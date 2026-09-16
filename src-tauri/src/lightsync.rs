//! Software lighting effects — G HUB's Screen Sampler and Audio Visualizer.
//!
//! Firmware effects (fixed, breathing, cycle) run on the device. These two
//! run here: a source produces colours ~20 times a second and each is pushed
//! to its zone as a fixed colour, RAM only (`persist = false`), so the flash
//! is never touched and the device falls back to its stored effect when the
//! app stops.
//!
//! - **Screen**: the desktop portal's ScreenCast gives a PipeWire stream (one
//!   consent dialog, remembered through a restore token). A `gst-launch-1.0`
//!   helper scales it to a 64×36 RGB thumbnail on a pipe; every zone averages
//!   the region of that thumbnail it is mapped to.
//! - **Audio**: `parec` records the default sink's monitor as raw PCM; RMS and
//!   three bands (low / mid / high) drive the zone colour.

use std::collections::HashMap;
use std::io::Read;
use std::os::fd::{AsRawFd, IntoRawFd};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::hidpp::{Error, Result};

/// A region of the screen, as fractions of its width and height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for Region {
    fn default() -> Self {
        Region { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }
    }
}

/// One zone's software effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SoftwareEffect {
    /// Average colour of `region`, brightness-scaled.
    Screen { region: Region, brightness: u8 },
    /// Colour follows the music: `low`/`mid`/`high` are `#rrggbb`, blended by
    /// band energy; `sensitivity` 0-100 scales the input.
    Audio { low: String, mid: String, high: String, sensitivity: u8, brightness: u8 },
}

/// Thumbnail size the screen helper produces.
const THUMB_W: usize = 64;
const THUMB_H: usize = 36;
const TICK: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Screen source
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub rgb: Arc<Vec<u8>>,
}

impl Frame {
    /// Mean colour of a region (fractions of the frame).
    pub fn average(&self, r: &Region) -> [u8; 3] {
        let x0 = ((r.x.clamp(0.0, 1.0)) * self.w as f32) as usize;
        let y0 = ((r.y.clamp(0.0, 1.0)) * self.h as f32) as usize;
        let x1 = (((r.x + r.w).clamp(0.0, 1.0)) * self.w as f32).ceil() as usize;
        let y1 = (((r.y + r.h).clamp(0.0, 1.0)) * self.h as f32).ceil() as usize;
        let (x1, y1) = (x1.max(x0 + 1).min(self.w), y1.max(y0 + 1).min(self.h));
        let mut sum = [0u64; 3];
        let mut n = 0u64;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * self.w + x) * 3;
                if i + 2 < self.rgb.len() {
                    sum[0] += self.rgb[i] as u64;
                    sum[1] += self.rgb[i + 1] as u64;
                    sum[2] += self.rgb[i + 2] as u64;
                    n += 1;
                }
            }
        }
        if n == 0 {
            return [0, 0, 0];
        }
        [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
    }
}

/// The portal session plus the helper that turns it into thumbnails.
pub struct ScreenSource {
    latest: Arc<Mutex<Option<Frame>>>,
    child: Mutex<Option<Child>>,
    stop: Arc<AtomicBool>,
    /// Keeps the portal session alive; dropping it ends the cast.
    _session_task: tauri::async_runtime::JoinHandle<()>,
    pub restore_token: Option<String>,
}

impl ScreenSource {
    /// Asks the portal for a monitor stream, then starts the helper.
    pub async fn start(restore_token: Option<String>) -> Result<Self> {
        use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
        use ashpd::desktop::PersistMode;

        let proxy = Screencast::new().await.map_err(|e| Error::other(format!("screen portal: {e}")))?;
        let session = proxy.create_session().await.map_err(|e| Error::other(format!("screen portal session: {e}")))?;
        proxy
            .select_sources(
                &session,
                CursorMode::Hidden,
                SourceType::Monitor.into(),
                false,
                restore_token.as_deref(),
                PersistMode::ExplicitlyRevoked,
            )
            .await
            .map_err(|e| Error::other(format!("screen portal: {e}")))?
            .response()
            .map_err(|e| Error::other(format!("screen portal: {e}")))?;
        let streams = proxy
            .start(&session, None)
            .await
            .map_err(|e| Error::other(format!("screen portal start: {e}")))?
            .response()
            .map_err(|e| Error::other(format!("screen sharing was refused: {e}")))?;
        let token = streams.restore_token().map(str::to_owned);
        let node = streams
            .streams()
            .first()
            .map(|s| s.pipe_wire_node_id())
            .ok_or_else(|| Error::other("the portal returned no screen stream"))?;
        let fd = proxy
            .open_pipe_wire_remote(&session)
            .await
            .map_err(|e| Error::other(format!("pipewire remote: {e}")))?;

        // The helper inherits the PipeWire fd; clear CLOEXEC on a dup for it.
        let raw = fd.as_raw_fd();
        let inherited = unsafe { libc::dup(raw) };
        if inherited < 0 {
            return Err(Error::other("could not duplicate the PipeWire fd"));
        }
        unsafe {
            let flags = libc::fcntl(inherited, libc::F_GETFD);
            libc::fcntl(inherited, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
        }
        let pipeline = format!(
            "pipewiresrc fd={inherited} path={node} do-timestamp=true ! videoconvert ! videoscale ! video/x-raw,format=RGB,width={THUMB_W},height={THUMB_H} ! videorate ! video/x-raw,framerate=20/1 ! fdsink fd=1 sync=false"
        );
        let mut child = Command::new("gst-launch-1.0")
            .arg("-q")
            .args(pipeline.split(' '))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::other(format!("gst-launch-1.0 is needed for the screen sampler: {e}")))?;
        unsafe {
            libc::close(inherited);
        }
        let mut stdout = child.stdout.take().expect("piped");

        let latest: Arc<Mutex<Option<Frame>>> = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        {
            let latest = Arc::clone(&latest);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("screen-sampler".into())
                .spawn(move || {
                    let mut buf = vec![0u8; THUMB_W * THUMB_H * 3];
                    while !stop.load(Ordering::SeqCst) {
                        if stdout.read_exact(&mut buf).is_err() {
                            break;
                        }
                        *latest.lock() = Some(Frame { w: THUMB_W, h: THUMB_H, rgb: Arc::new(buf.clone()) });
                    }
                })
                .expect("screen thread");
        }

        // Hold the session (and the fd) for as long as the source lives.
        let keep_fd = fd.into_raw_fd();
        let session_task = tauri::async_runtime::spawn(async move {
            let _proxy = proxy;
            let _session = session;
            std::future::pending::<()>().await;
            unsafe {
                libc::close(keep_fd);
            }
        });

        Ok(ScreenSource { latest, child: Mutex::new(Some(child)), stop, _session_task: session_task, restore_token: token })
    }

    pub fn frame(&self) -> Option<Frame> {
        self.latest.lock().clone()
    }
}

impl Drop for ScreenSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mut c) = self.child.lock().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self._session_task.abort();
    }
}

// ---------------------------------------------------------------------------
// Audio source
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioLevels {
    pub rms: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

pub struct AudioSource {
    latest: Arc<Mutex<AudioLevels>>,
    child: Mutex<Option<Child>>,
    stop: Arc<AtomicBool>,
}

const RATE: u32 = 22050;
const CHUNK: usize = 1024;

impl AudioSource {
    /// Records the default sink's monitor through `parec` (PulseAudio or
    /// PipeWire's compatibility layer).
    pub fn start() -> Result<Self> {
        let mut child = Command::new("parec")
            .args([
                "--device=@DEFAULT_MONITOR@",
                "--raw",
                "--format=s16le",
                &format!("--rate={RATE}"),
                "--channels=1",
                "--latency-msec=30",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::other(format!("parec is needed for the audio visualizer: {e}")))?;
        let mut stdout = child.stdout.take().expect("piped");
        let latest = Arc::new(Mutex::new(AudioLevels::default()));
        let stop = Arc::new(AtomicBool::new(false));
        {
            let latest = Arc::clone(&latest);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("audio-visualizer".into())
                .spawn(move || {
                    let mut buf = vec![0u8; CHUNK * 2];
                    // Automatic gain per band: each follows its own recent
                    // peak (slow decay), so quiet and loud sources both use
                    // the whole range and a bass-heavy mix still shows treble.
                    let mut peaks = [0.02f32; 4];
                    while !stop.load(Ordering::SeqCst) {
                        if stdout.read_exact(&mut buf).is_err() {
                            break;
                        }
                        let samples: Vec<f32> =
                            buf.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
                        let raw = analyse(&samples);
                        let values = [raw.rms, raw.low, raw.mid, raw.high];
                        let mut norm = [0.0f32; 4];
                        for i in 0..4 {
                            peaks[i] = (peaks[i] * 0.993).max(values[i]).max(0.004);
                            norm[i] = (values[i] / peaks[i]).min(1.0);
                        }
                        let mut l = latest.lock();
                        let smooth = |old: f32, new: f32| if new > old { old * 0.4 + new * 0.6 } else { old * 0.75 + new * 0.25 };
                        *l = AudioLevels {
                            rms: smooth(l.rms, norm[0]),
                            low: smooth(l.low, norm[1]),
                            mid: smooth(l.mid, norm[2]),
                            high: smooth(l.high, norm[3]),
                        };
                    }
                })
                .expect("audio thread");
        }
        Ok(AudioSource { latest, child: Mutex::new(Some(child)), stop })
    }

    pub fn levels(&self) -> AudioLevels {
        *self.latest.lock()
    }
}

impl Drop for AudioSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mut c) = self.child.lock().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// In-place radix-2 FFT on interleaved (re, im) pairs; `n` must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (ar, ai) = (re[start + k], im[start + k]);
                let (br, bi) = (re[start + k + len / 2], im[start + k + len / 2]);
                let (tr, ti) = (br * cr - bi * ci, br * ci + bi * cr);
                re[start + k] = ar + tr;
                im[start + k] = ai + ti;
                re[start + k + len / 2] = ar - tr;
                im[start + k + len / 2] = ai - ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// RMS plus the mean spectral magnitude in three bands (Hz): low 40–250,
/// mid 250–2000, high 2000–9000.
pub fn analyse(samples: &[f32]) -> AudioLevels {
    if samples.is_empty() {
        return AudioLevels::default();
    }
    let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
    let n = samples.len().next_power_of_two().min(samples.len());
    let n = if n.is_power_of_two() { n } else { 1 << (usize::BITS - n.leading_zeros() - 1) };
    let mut re: Vec<f32> = samples[..n]
        .iter()
        .enumerate()
        // Hann window, so tones between bins still register.
        .map(|(i, x)| x * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos()))
        .collect();
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);
    let bin_hz = RATE as f32 / n as f32;
    let band = |lo: f32, hi: f32| {
        let a = (lo / bin_hz).round().max(1.0) as usize;
        let b = ((hi / bin_hz).round() as usize).min(n / 2);
        if b <= a {
            return 0.0;
        }
        (a..b).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).sum::<f32>() / (b - a) as f32 / n as f32 * 4.0
    };
    AudioLevels { rms, low: band(40.0, 250.0), mid: band(250.0, 2000.0), high: band(2000.0, 9000.0) }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// (device id, zone) → effect.
pub type ZoneKey = (String, u8);

/// Owns the sources and the loop that streams colours to devices.
pub struct LightSync {
    effects: Mutex<HashMap<ZoneKey, SoftwareEffect>>,
    screen: Mutex<Option<Arc<ScreenSource>>>,
    audio: Mutex<Option<Arc<AudioSource>>>,
    /// Last colour sent per zone, to skip identical writes.
    sent: Mutex<HashMap<ZoneKey, ([u8; 3], Instant)>>,
    pub restore_token: Mutex<Option<String>>,
    pub last_error: Mutex<Option<String>>,
}

impl Default for LightSync {
    fn default() -> Self {
        Self::new()
    }
}

impl LightSync {
    pub fn new() -> Self {
        LightSync {
            effects: Mutex::new(HashMap::new()),
            screen: Mutex::new(None),
            audio: Mutex::new(None),
            sent: Mutex::new(HashMap::new()),
            restore_token: Mutex::new(None),
            last_error: Mutex::new(None),
        }
    }

    /// Replaces the whole effect table (what the active profile asks for).
    pub fn set_effects(&self, effects: HashMap<ZoneKey, SoftwareEffect>) {
        *self.effects.lock() = effects;
        self.sent.lock().clear();
    }

    pub fn set_zone(&self, key: ZoneKey, effect: Option<SoftwareEffect>) {
        let mut e = self.effects.lock();
        match effect {
            Some(fx) => {
                e.insert(key.clone(), fx);
            }
            None => {
                e.remove(&key);
            }
        }
        self.sent.lock().remove(&key);
    }

    pub fn effects(&self) -> HashMap<ZoneKey, SoftwareEffect> {
        self.effects.lock().clone()
    }

    fn needs_screen(&self) -> bool {
        self.effects.lock().values().any(|e| matches!(e, SoftwareEffect::Screen { .. }))
    }

    fn needs_audio(&self) -> bool {
        self.effects.lock().values().any(|e| matches!(e, SoftwareEffect::Audio { .. }))
    }

    /// Starts or stops the sources to match the effect table. Screen capture
    /// needs the portal, hence async.
    pub async fn reconcile_sources(&self) {
        if self.needs_screen() {
            if self.screen.lock().is_none() {
                let token = self.restore_token.lock().clone();
                match ScreenSource::start(token).await {
                    Ok(src) => {
                        if let Some(t) = &src.restore_token {
                            *self.restore_token.lock() = Some(t.clone());
                        }
                        *self.last_error.lock() = None;
                        *self.screen.lock() = Some(Arc::new(src));
                    }
                    Err(e) => {
                        log::warn!("screen sampler not started: {e}");
                        *self.last_error.lock() = Some(e.to_string());
                    }
                }
            }
        } else {
            self.screen.lock().take();
        }
        if self.needs_audio() {
            if self.audio.lock().is_none() {
                match AudioSource::start() {
                    Ok(src) => {
                        *self.last_error.lock() = None;
                        *self.audio.lock() = Some(Arc::new(src));
                    }
                    Err(e) => {
                        log::warn!("audio visualizer not started: {e}");
                        *self.last_error.lock() = Some(e.to_string());
                    }
                }
            }
        } else {
            self.audio.lock().take();
        }
    }

    /// One tick: the colours that should go out now, deduplicated.
    pub fn tick(&self) -> Vec<(ZoneKey, [u8; 3])> {
        let effects = self.effects.lock().clone();
        if effects.is_empty() {
            return vec![];
        }
        let frame = self.screen.lock().as_ref().and_then(|s| s.frame());
        let levels = self.audio.lock().as_ref().map(|a| a.levels());
        let mut out = Vec::new();
        let mut sent = self.sent.lock();
        let now = Instant::now();
        for (key, fx) in effects {
            let rgb = match &fx {
                SoftwareEffect::Screen { region, brightness } => {
                    let Some(f) = &frame else { continue };
                    scale(f.average(region), *brightness)
                }
                SoftwareEffect::Audio { low, mid, high, sensitivity, brightness } => {
                    let Some(l) = levels else { continue };
                    scale(audio_colour(l, low, mid, high, *sensitivity), *brightness)
                }
            };
            let changed = match sent.get(&key) {
                Some((last, at)) => distance(*last, rgb) >= 4 || now.duration_since(*at) > Duration::from_secs(2),
                None => true,
            };
            if changed {
                sent.insert(key.clone(), (rgb, now));
                out.push((key, rgb));
            }
        }
        out
    }

    pub fn active(&self) -> bool {
        !self.effects.lock().is_empty()
    }

    pub fn stop_all(&self) {
        self.effects.lock().clear();
        self.screen.lock().take();
        self.audio.lock().take();
        self.sent.lock().clear();
    }
}

fn scale(rgb: [u8; 3], brightness: u8) -> [u8; 3] {
    let b = brightness.min(100) as u32;
    [(rgb[0] as u32 * b / 100) as u8, (rgb[1] as u32 * b / 100) as u8, (rgb[2] as u32 * b / 100) as u8]
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u16 {
    (0..3).map(|i| a[i].abs_diff(b[i]) as u16).max().unwrap_or(0)
}

pub fn parse_hex(s: &str) -> [u8; 3] {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return [0, 0, 0];
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    [p(0), p(2), p(4)]
}

/// Blends the three band colours by their energy, scaled by overall level —
/// a quiet passage dims, bass pushes towards `low`, treble towards `high`.
pub fn audio_colour(l: AudioLevels, low: &str, mid: &str, high: &str, sensitivity: u8) -> [u8; 3] {
    let gain = 0.4 + sensitivity.min(100) as f32 / 100.0 * 1.6;
    let (lo, mi, hi) = ((l.low * gain).min(1.0), (l.mid * gain).min(1.0), (l.high * gain).min(1.0));
    let total = lo + mi + hi;
    if total < 1e-3 {
        return [0, 0, 0];
    }
    let (cl, cm, ch) = (parse_hex(low), parse_hex(mid), parse_hex(high));
    let level = (l.rms * gain).min(1.0);
    let mut out = [0u8; 3];
    for i in 0..3 {
        let v = (cl[i] as f32 * lo + cm[i] as f32 * mi + ch[i] as f32 * hi) / total;
        out[i] = (v * level).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Runs the streaming loop until the app exits.
pub fn spawn(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let sync = app.state::<Arc<LightSync>>().inner().clone();
        let manager_app = app.clone();
        loop {
            tokio::time::sleep(TICK).await;
            sync.reconcile_sources().await;
            if !sync.active() {
                continue;
            }
            let updates = sync.tick();
            if updates.is_empty() {
                continue;
            }
            let app2 = manager_app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let manager = app2.state::<crate::state::DeviceManager>();
                for ((device, zone), rgb) in updates {
                    if let Err(e) = manager.set_lighting(&device, zone, rgb, crate::hidpp::features::LightEffect::Fixed, false) {
                        log::debug!("lightsync write to {device} zone {zone} failed: {e}");
                    }
                }
            })
            .await;
        }
    });
}

use tauri::Manager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_average() {
        // 2×2 frame: red, green / blue, white.
        let rgb = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let f = Frame { w: 2, h: 2, rgb: Arc::new(rgb) };
        assert_eq!(f.average(&Region { x: 0.0, y: 0.0, w: 0.5, h: 0.5 }), [255, 0, 0]);
        assert_eq!(f.average(&Region::default()), [127, 127, 127]);
    }

    #[test]
    fn bands_pick_up_a_sine() {
        let n = 1024;
        let bass: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * 100.0 * i as f32 / RATE as f32).sin()).collect();
        let l = analyse(&bass);
        assert!(l.low > l.high * 5.0, "100 Hz lands in the low band: {l:?}");
        let treble: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * 6000.0 * i as f32 / RATE as f32).sin()).collect();
        let l = analyse(&treble);
        assert!(l.high > l.low * 5.0, "6 kHz lands in the high band: {l:?}");
    }

    #[test]
    fn audio_colour_follows_the_loud_band() {
        let l = AudioLevels { rms: 1.0, low: 1.0, mid: 0.0, high: 0.0 };
        assert_eq!(audio_colour(l, "#ff0000", "#00ff00", "#0000ff", 50), [255, 0, 0]);
        let silent = AudioLevels::default();
        assert_eq!(audio_colour(silent, "#ff0000", "#00ff00", "#0000ff", 50), [0, 0, 0]);
    }
}
