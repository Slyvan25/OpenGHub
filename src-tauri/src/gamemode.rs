//! Game Mode and function-key G-keys, done by OpenGHub itself.
//!
//! The G915's `0x4522` accepts key lists but disables nothing we could
//! observe, so Game Mode grabs the keyboard's key-bearing evdev nodes
//! (`EVIOCGRAB`: only OpenGHub receives their events) and passes every key on
//! through the virtual keyboard — except the disabled ones.
//!
//! The same grab serves keyboards whose G-keys are plain function keys: a
//! G915 X has no `0x8010`, its G1–G9 simply type F13–F21. With a binding on
//! such a key the press is swallowed here and handed to the button pump as a
//! G-key edge, where the assignment plan resolves it; keys with no binding
//! pass through as what they are.
//!
//! The kernel drops the grab when the process exits, so a crash can never
//! leave the keyboard dead. Known limits: while grabbed, lock-key LEDs (Caps
//! Lock) follow the virtual keyboard, not the physical one; and a grab is
//! exclusive, so a keyboard another remapper holds (keyd, kmonad) cannot be
//! taken — that is logged, and the keys keep doing what that program says.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::keymap;
use crate::remap::Injector;

/// `_IOW('E', 0x90, int)`.
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;
const EV_KEY: u16 = 1;

/// HID keyboard usages → evdev key codes (modifiers included).
pub fn codes_for_usages(usages: &[u8]) -> Vec<u16> {
    usages
        .iter()
        .flat_map(|&u| {
            if (0xe0..=0xe7).contains(&u) {
                keymap::modifier_codes(1 << (u - 0xe0))
            } else {
                keymap::usage_code(u).into_iter().collect()
            }
        })
        .collect()
}

/// Keyboards whose G-keys are function keys: product id → how many, counted
/// from F13. The G915 X reports G1–G9 as F13–F21 and has no `0x8010`.
const FKEY_GKEYS: &[(u16, u8)] = &[(0xc359, 9)];

/// The G-key count of a keyboard whose G-keys arrive as F13 onwards.
pub fn fkey_gkey_count(product_id: u16) -> Option<u8> {
    FKEY_GKEYS.iter().find(|(pid, _)| *pid == product_id).map(|(_, n)| *n)
}

/// The evdev code of such a keyboard's G-key `n` (0-based).
pub fn fkey_gkey_code(n: u8) -> u16 {
    keymap::KEY_F13 + n as u16
}

/// A G-key press or release taken off the keyboard, for the button pump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GKeyEdge {
    pub device: String,
    /// 0-based G-key index.
    pub button: u8,
    pub pressed: bool,
}

struct Grab {
    run: Arc<AtomicBool>,
    blocked: Arc<Mutex<Vec<u16>>>,
    /// evdev code → G-key index, for keys the plan has a binding for.
    gkeys: Arc<Mutex<HashMap<u16, u8>>>,
}

#[derive(Default)]
pub struct GameMode {
    grabs: Mutex<HashMap<String, Grab>>,
    edges: Arc<Mutex<Vec<GKeyEdge>>>,
}

impl GameMode {
    /// Grabs `device` (evdev name containing `name`) to block these key codes
    /// and to divert these G-keys, or updates both lists when already held.
    /// With nothing to block or divert the grab is released. Returns whether
    /// the keyboard is held afterwards.
    pub fn enable(
        &self,
        device: &str,
        name: &str,
        blocked: Vec<u16>,
        gkeys: HashMap<u16, u8>,
        injector: Arc<Injector>,
    ) -> bool {
        if blocked.is_empty() && gkeys.is_empty() {
            self.disable(device);
            return false;
        }
        let mut grabs = self.grabs.lock();
        if let Some(g) = grabs.get(device) {
            *g.blocked.lock() = blocked;
            *g.gkeys.lock() = gkeys;
            return true;
        }
        let run = Arc::new(AtomicBool::new(true));
        let list = Arc::new(Mutex::new(blocked));
        let gmap = Arc::new(Mutex::new(gkeys));
        let mut opened = 0;
        let mut found = 0;
        for path in key_nodes(name) {
            found += 1;
            let Ok(file) = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&path) else {
                continue;
            };
            // SAFETY: plain ioctl on an fd we own.
            if unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB, 1 as libc::c_int) } != 0 {
                log::warn!("keyboard grab: could not grab {path} — another program (keyd, kmonad, …) may hold it");
                continue;
            }
            opened += 1;
            let (run, list, gmap, edges, injector) =
                (Arc::clone(&run), Arc::clone(&list), Arc::clone(&gmap), Arc::clone(&self.edges), Arc::clone(&injector));
            let id = device.to_string();
            std::thread::Builder::new()
                .name("keyboard-grab".into())
                .spawn(move || forward(file, &run, &list, &gmap, &edges, &id, &injector))
                .ok();
        }
        if opened == 0 {
            if found == 0 {
                log::warn!("keyboard grab: no keyboard node found for '{name}'");
            }
            return false;
        }
        log::info!("keyboard grabbed for {device} ({opened} node(s))");
        grabs.insert(device.to_string(), Grab { run, blocked: list, gkeys: gmap });
        true
    }

    /// G-key edges taken since the last call.
    pub fn drain_gkeys(&self) -> Vec<GKeyEdge> {
        std::mem::take(&mut *self.edges.lock())
    }

    pub fn disable(&self, device: &str) {
        if let Some(g) = self.grabs.lock().remove(device) {
            g.run.store(false, Ordering::SeqCst);
            log::info!("game mode off for {device}");
        }
    }

    pub fn disable_all(&self) {
        for (_, g) in self.grabs.lock().drain() {
            g.run.store(false, Ordering::SeqCst);
        }
    }
}

/// The evdev nodes of a keyboard that carry ordinary keys (KEY_A), by name.
fn key_nodes(name: &str) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else { return Vec::new() };
    dir.flatten()
        .filter_map(|e| {
            let node = e.file_name().to_string_lossy().into_owned();
            if !node.starts_with("event") {
                return None;
            }
            let dev_name = std::fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
            let caps = std::fs::read_to_string(e.path().join("device/capabilities/key")).unwrap_or_default();
            (dev_name.contains(name) && has_key(&caps, 30)).then(|| format!("/dev/input/{node}"))
        })
        .collect()
}

/// Whether a sysfs key-capability bitmap (hex words, most significant first)
/// has bit `code` set.
fn has_key(caps: &str, code: usize) -> bool {
    let words: Vec<&str> = caps.split_whitespace().collect();
    let bits = std::mem::size_of::<libc::c_ulong>() * 8;
    let (word, bit) = (code / bits, code % bits);
    words
        .len()
        .checked_sub(word + 1)
        .and_then(|i| u64::from_str_radix(words[i], 16).ok())
        .is_some_and(|w| w >> bit & 1 == 1)
}

fn forward(
    mut file: File,
    run: &AtomicBool,
    blocked: &Mutex<Vec<u16>>,
    gkeys: &Mutex<HashMap<u16, u8>>,
    edges: &Mutex<Vec<GKeyEdge>>,
    device: &str,
    injector: &Injector,
) {
    // struct input_event on 64-bit: timeval (16), type u16, code u16, value i32.
    let mut buf = [0u8; 24];
    while run.load(Ordering::SeqCst) {
        match file.read(&mut buf) {
            Ok(24) => {
                let kind = u16::from_ne_bytes([buf[16], buf[17]]);
                let code = u16::from_ne_bytes([buf[18], buf[19]]);
                let value = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);
                // Presses and releases pass on; the compositor makes its own
                // repeats, so the kernel's (value 2) are not needed.
                if kind != EV_KEY || value == 2 {
                    continue;
                }
                if let Some(button) = gkeys.lock().get(&code).copied() {
                    edges.lock().push(GKeyEdge { device: device.to_string(), button, pressed: value == 1 });
                    continue;
                }
                if !blocked.lock().contains(&code) {
                    if let Err(e) = injector.key(code, value == 1) {
                        log::warn!("keyboard grab: key not passed on: {e}");
                    }
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Sleep until the keyboard sends something (or 100 ms pass, to
                // notice being switched off).
                let mut pfd = libc::pollfd { fd: file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
                // SAFETY: one valid pollfd.
                unsafe { libc::poll(&mut pfd, 1, 100) };
            }
            Err(_) => break, // unplugged
        }
    }
    // SAFETY: as above; the grab also ends when the fd closes.
    unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB, 0 as libc::c_int) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usages_map_to_evdev_codes() {
        assert_eq!(codes_for_usages(&[0x04, 0xe3, 0x65]), vec![30, keymap::KEY_LEFTMETA, 127]);
    }

    #[test]
    fn function_key_gkeys() {
        assert_eq!(fkey_gkey_count(0xc359), Some(9));
        assert_eq!(fkey_gkey_count(0xc33e), None, "a G915 has 0x8010 instead");
        assert_eq!(fkey_gkey_code(0), keymap::KEY_F13);
        assert_eq!(fkey_gkey_code(8), keymap::KEY_F13 + 8, "G9 is F21");
    }

    #[test]
    fn reads_key_capability_bitmaps() {
        // Bit 30 (KEY_A) in the lowest word.
        assert!(has_key("0 0 40000000", 30));
        assert!(!has_key("0 0 0", 30));
    }
}
