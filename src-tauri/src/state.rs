//! Device manager: owns the HID handles, caches what we know about each device
//! and keeps the frontend's view of the world in sync.

use std::collections::HashMap;

use hidapi::HidApi;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::hidpp::features::{self, BatteryState, DpiState, ReportRateState};
use crate::hidpp::registry::{self, DeviceKind};
use crate::wheel::{self, WheelHandle, WheelSettings, WheelState};
use crate::hidpp::{self, DeviceAddress, Error, Handle, Result, RECEIVER_CHILD_INDICES};

/// How the device is attached, which is what the card's status row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    Wired,
    Wireless,
    Receiver,
    Bluetooth,
}

/// What a device can actually do — the UI hides tabs it cannot drive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub dpi: bool,
    pub report_rate: bool,
    pub battery: bool,
    pub lighting: bool,
    pub onboard_memory: bool,
    /// A racing wheel driven through the classic command channel.
    #[serde(default)]
    pub wheel: bool,
}

/// Static facts about a wheel, for the Steering Wheel page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelInfo {
    pub range_min: u16,
    pub range_max: u16,
    pub rpm_leds: u8,
    pub protocol: String,
    /// The userspace force-feedback driver is running for this wheel.
    pub driver_running: bool,
    /// Whether the wheel's own centre calibration (HID++ 0x812c) exists; the
    /// classic wheels get a software offset instead.
    pub hardware_calibration: bool,
}

/// The full per-device payload sent over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSnapshot {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
    /// The device's own product ids from `0x0003`. Behind a receiver these are
    /// the only way to identify the model — `product_id` is the dongle's.
    #[serde(default)]
    pub model_ids: Vec<u16>,
    pub connection: Connection,
    pub online: bool,
    pub capabilities: Capabilities,
    pub battery: Option<BatteryState>,
    pub dpi: Option<DpiState>,
    pub report_rate: Option<ReportRateState>,
    pub lighting_zones: u8,
    #[serde(default)]
    pub wheel: Option<WheelInfo>,
    pub protocol_version: String,
    /// Set when this entry is synthesised rather than backed by real hardware.
    pub demo: bool,
    /// Last error seen while talking to the device, surfaced as a card badge.
    pub last_error: Option<String>,
}

impl DeviceSnapshot {
    fn placeholder(address: &DeviceAddress, name: String, kind: DeviceKind) -> Self {
        DeviceSnapshot {
            id: address.id(),
            name,
            kind,
            vendor_id: address.vendor_id,
            product_id: address.product_id,
            serial: None,
            model_ids: Vec::new(),
            connection: Connection::Wired,
            online: false,
            capabilities: Capabilities::default(),
            battery: None,
            dpi: None,
            report_rate: None,
            lighting_zones: 0,
            wheel: None,
            protocol_version: String::new(),
            demo: false,
            last_error: None,
        }
    }

    /// A classic wheel, which has no HID++ address to derive an id from.
    fn wheel_placeholder(handle: &WheelHandle, hid_product: Option<String>) -> Self {
        let mut snap = DeviceSnapshot::placeholder(
            &DeviceAddress {
                path: handle.path.clone(),
                vendor_id: crate::hidpp::LOGITECH_VID,
                product_id: handle.model.product_id,
                device_index: hidpp::DEVICE_INDEX_WIRED,
            },
            hid_product.unwrap_or_else(|| handle.model.name.to_string()),
            DeviceKind::Wheel,
        );
        snap.id = handle.id();
        snap.name = handle.model.name.to_string();
        snap.serial = handle.serial.clone();
        snap.model_ids = vec![handle.model.product_id];
        snap.online = true;
        snap.capabilities.wheel = true;
        snap.protocol_version = "classic".into();
        snap.wheel = Some(WheelInfo {
            range_min: handle.model.range.0,
            range_max: handle.model.range.1,
            rpm_leds: handle.model.rpm_leds,
            protocol: format!("{:?}", handle.model.protocol),
            driver_running: handle.bridge_running(),
            hardware_calibration: false,
        });
        snap
    }
}

/// Why the device list came back empty. The dashboard renders a different
/// message for each, because "no Logitech hardware" and "hardware is there but
/// this user cannot open it" need completely different fixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmptyReason {
    /// Nothing Logitech on the bus at all.
    NoHardware,
    /// HID++ endpoints were found but none could be opened — almost always the
    /// hidraw nodes being root-only.
    PermissionDenied { devices: Vec<String> },
    /// Endpoints opened but did not answer a HID++ ping (asleep, or not HID++).
    NoResponse,
    /// hidapi itself would not start.
    HidUnavailable { message: String },
}

/// Owns everything HID-related. Every command goes through a single lock, which
/// serialises access to the hidraw nodes — HID++ is strictly request/response so
/// concurrent writes to one device would interleave replies.
pub struct DeviceManager {
    inner: Mutex<Inner>,
}

struct Inner {
    api: Option<HidApi>,
    handles: HashMap<String, Handle>,
    /// Classic wheels, keyed like `snapshots`.
    wheels: HashMap<String, WheelHandle>,
    snapshots: HashMap<String, DeviceSnapshot>,
    order: Vec<String>,
    /// True when we are showing synthetic devices because no real ones were found.
    demo: bool,
    init_error: Option<String>,
    empty_reason: Option<EmptyReason>,
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager {
    pub fn new() -> Self {
        let (api, init_error) = match HidApi::new() {
            Ok(api) => (Some(api), None),
            Err(e) => {
                log::error!("could not initialise hidapi: {e}");
                (None, Some(e.to_string()))
            }
        };
        DeviceManager {
            inner: Mutex::new(Inner {
                api,
                handles: HashMap::new(),
                wheels: HashMap::new(),
                snapshots: HashMap::new(),
                order: Vec::new(),
                demo: false,
                empty_reason: init_error
                    .clone()
                    .map(|message| EmptyReason::HidUnavailable { message }),
                init_error,
            }),
        }
    }

    /// True when the list currently shown is synthetic.
    pub fn is_demo(&self) -> bool {
        self.inner.lock().demo
    }

    pub fn init_error(&self) -> Option<String> {
        self.inner.lock().init_error.clone()
    }

    /// Why the last scan found nothing, if it found nothing.
    pub fn empty_reason(&self) -> Option<EmptyReason> {
        self.inner.lock().empty_reason.clone()
    }

    /// Rescans the bus, opens anything new and drops anything that went away.
    pub fn refresh(&self) -> Vec<DeviceSnapshot> {
        let mut inner = self.inner.lock();
        inner.refresh();
        inner.ordered_snapshots()
    }

    /// Cached view — cheap, used by polling callers that do not want a rescan.
    pub fn snapshots(&self) -> Vec<DeviceSnapshot> {
        let inner = self.inner.lock();
        if inner.snapshots.is_empty() {
            drop(inner);
            return self.refresh();
        }
        inner.ordered_snapshots()
    }

    pub fn snapshot(&self, id: &str) -> Option<DeviceSnapshot> {
        self.inner.lock().snapshots.get(id).cloned()
    }

    /// Re-reads the live state (battery, DPI, rate) of one device.
    pub fn refresh_device(&self, id: &str) -> Result<DeviceSnapshot> {
        let mut inner = self.inner.lock();
        inner.refresh_one(id)
    }

    /// Runs `f` against an open handle, marking the device offline if it vanished.
    pub fn with_handle<T>(&self, id: &str, f: impl FnOnce(&mut Handle) -> Result<T>) -> Result<T> {
        let mut inner = self.inner.lock();
        inner.with_handle(id, f)
    }

    /// Reads battery for every device that reports one. Used by the background poller.
    pub fn poll_batteries(&self) -> Vec<(String, BatteryState)> {
        let mut inner = self.inner.lock();
        let ids: Vec<String> = inner
            .snapshots
            .values()
            .filter(|s| s.capabilities.battery && s.online)
            .map(|s| s.id.clone())
            .collect();

        let mut out = Vec::new();
        for id in ids {
            if inner.demo {
                if let Some(state) = inner.demo_battery(&id) {
                    out.push((id, state));
                }
                continue;
            }
            match inner.with_handle(&id, features::read_battery) {
                Ok(state) => {
                    if let Some(snap) = inner.snapshots.get_mut(&id) {
                        snap.battery = Some(state.clone());
                        snap.last_error = None;
                    }
                    out.push((id, state));
                }
                Err(Error::NotConnected) | Err(Error::Timeout) => {
                    // Wireless devices sleep; not an error worth surfacing.
                    if let Some(snap) = inner.snapshots.get_mut(&id) {
                        snap.online = false;
                    }
                }
                Err(e) => log::debug!("battery poll failed for {id}: {e}"),
            }
        }
        out
    }

    pub fn set_demo(&self, on: bool) -> Vec<DeviceSnapshot> {
        let mut inner = self.inner.lock();
        inner.demo = on;
        if on {
            inner.load_demo();
        } else {
            inner.snapshots.clear();
            inner.order.clear();
            inner.refresh();
        }
        inner.ordered_snapshots()
    }

    /// Applies a DPI change, updating the cached snapshot on success.
    // -- wheels ---------------------------------------------------------------

    fn with_wheel<T>(&self, id: &str, f: impl FnOnce(&mut WheelHandle) -> Result<T>) -> Result<T> {
        let mut inner = self.inner.lock();
        let handle = inner.wheels.get_mut(id).ok_or(Error::NotConnected)?;
        f(handle)
    }

    pub fn is_wheel(&self, id: &str) -> bool {
        self.inner.lock().wheels.contains_key(id)
    }

    /// Latest input state of a wheel (polled here unless the driver runs).
    pub fn wheel_state(&self, id: &str) -> Result<WheelState> {
        self.with_wheel(id, |w| {
            if !w.bridge_running() {
                w.poll();
            }
            Ok(*w.state.lock())
        })
    }

    pub fn wheel_settings(&self, id: &str) -> Result<WheelSettings> {
        self.with_wheel(id, |w| Ok(w.settings.lock().clone()))
    }

    /// Writes a settings block to the wheel; returns it with the range clamped.
    pub fn apply_wheel_settings(&self, id: &str, settings: &WheelSettings) -> Result<WheelSettings> {
        self.with_wheel(id, |w| {
            w.apply(settings)?;
            Ok(w.settings.lock().clone())
        })
    }

    pub fn set_wheel_leds(&self, id: &str, mask: u8) -> Result<()> {
        self.with_wheel(id, |w| w.set_leds(mask))
    }

    /// G HUB's "calibrate wheel centre": the current position becomes centre.
    /// Returns the new raw offset; refused beyond `max_degrees` from the
    /// wheel's own centre, like G HUB's restricted calibration.
    pub fn calibrate_wheel_center(&self, id: &str, max_degrees: f32) -> Result<i16> {
        self.with_wheel(id, |w| {
            if !w.bridge_running() {
                w.poll();
            }
            let raw = w.state.lock().steering_raw as i32;
            let offset = raw - 32768;
            let range = w.settings.lock().range_deg as f32;
            let degrees = offset as f32 / 32768.0 * range / 2.0;
            if degrees.abs() > max_degrees {
                return Err(Error::other(format!(
                    "the wheel is {degrees:.1}° from centre; calibration allows at most ±{max_degrees:.0}°"
                )));
            }
            w.settings.lock().center_offset = offset.clamp(-32768, 32767) as i16;
            Ok(offset as i16)
        })
    }

    /// Starts or stops the userspace force-feedback driver for a wheel.
    pub fn set_wheel_driver(&self, id: &str, enabled: bool) -> Result<bool> {
        let mut inner = self.inner.lock();
        let handle = inner.wheels.get_mut(id).ok_or(Error::NotConnected)?;
        if enabled && !handle.bridge_running() {
            let grabs = wheel_evdev_nodes(&handle.path);
            handle.start_bridge(grabs)?;
        } else if !enabled && handle.bridge_running() {
            handle.stop_bridge();
        }
        let running = handle.bridge_running();
        if let Some(info) = inner.snapshots.get_mut(id).and_then(|s| s.wheel.as_mut()) {
            info.driver_running = running;
        }
        Ok(running)
    }

    /// Ids of every connected wheel.
    pub fn wheel_ids(&self) -> Vec<String> {
        self.inner.lock().wheels.keys().cloned().collect()
    }

    pub fn set_dpi(&self, id: &str, dpi: u16) -> Result<DpiState> {
        let mut inner = self.inner.lock();
        if inner.demo {
            return inner.demo_set_dpi(id, dpi);
        }
        let sensor = inner
            .snapshots
            .get(id)
            .and_then(|s| s.dpi.as_ref())
            .map(|d| d.sensor)
            .unwrap_or(0);
        inner.with_handle(id, |h| features::write_dpi(h, sensor, dpi))?;
        let state = inner.with_handle(id, |h| features::read_dpi(h, sensor))?;
        if let Some(snap) = inner.snapshots.get_mut(id) {
            snap.dpi = Some(state.clone());
        }
        Ok(state)
    }

    pub fn set_report_rate(&self, id: &str, hz: u32) -> Result<ReportRateState> {
        let mut inner = self.inner.lock();
        if inner.demo {
            return inner.demo_set_rate(id, hz);
        }
        inner.with_handle(id, |h| features::write_report_rate(h, hz))?;
        let state = inner.with_handle(id, features::read_report_rate)?;
        if let Some(snap) = inner.snapshots.get_mut(id) {
            snap.report_rate = Some(state.clone());
        }
        Ok(state)
    }

    pub fn set_lighting(
        &self,
        id: &str,
        zone: u8,
        rgb: [u8; 3],
        effect: features::LightEffect,
        persist: bool,
    ) -> Result<()> {
        let mut inner = self.inner.lock();
        if inner.demo {
            return Ok(());
        }
        inner.with_handle(id, |h| features::write_lighting(h, zone, rgb, effect, persist))
    }

    /// Full zone description — index, location name and accepted effects.
    pub fn lighting_zones(&self, id: &str) -> Result<Vec<features::ZoneInfo>> {
        let mut inner = self.inner.lock();
        if inner.demo {
            // Mirror a two-zone mouse so the UI is exercisable without hardware.
            let count = inner.snapshots.get(id).map(|s| s.lighting_zones).unwrap_or(0);
            return Ok((0..count)
                .map(|index| features::ZoneInfo {
                    index,
                    location: index as u16 + 1,
                    location_name: features::zone_location_name(index as u16 + 1).to_string(),
                    effects: vec![0x00, 0x01, 0x03, 0x0a],
                })
                .collect());
        }
        inner.with_handle(id, features::read_zones)
    }

    /// Reads every sector of onboard memory and writes it to a timestamped file.
    ///
    /// Always taken before the first write to a device: a bad sector write can
    /// leave an onboard profile unusable, and this is what makes that reversible.
    pub fn backup_onboard(&self, id: &str) -> Result<std::path::PathBuf> {
        let mut inner = self.inner.lock();
        let (name, pid) = inner
            .snapshots
            .get(id)
            .map(|s| (s.name.clone(), s.product_id))
            .unwrap_or_default();
        let backup = inner.with_handle(id, |h| crate::hidpp::onboard::backup(h, &name, pid))?;

        let dir = crate::artwork::dir()
            .parent()
            .map(|d| d.join("backups"))
            .ok_or_else(|| Error::other("could not resolve the data directory"))?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::other(format!("could not create {}: {e}", dir.display())))?;

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("{:04x}-{stamp}.json", backup.product_id));

        let json = serde_json::to_string_pretty(&backup)
            .map_err(|e| Error::other(format!("could not serialise the backup: {e}")))?;
        std::fs::write(&path, json)
            .map_err(|e| Error::other(format!("could not write {}: {e}", path.display())))?;
        Ok(path)
    }

    /// The device's onboard profiles, decoded.
    pub fn read_onboard_profiles(
        &self,
        id: &str,
    ) -> Result<(crate::hidpp::onboard::OnboardInfo, Vec<crate::hidpp::onboard::Profile>)> {
        use crate::hidpp::onboard;
        let mut inner = self.inner.lock();
        inner.with_handle(id, |h| {
            let info = onboard::read_info(h)?;
            let size = info.sector_size as usize;
            let directory = onboard::parse_directory(&onboard::read_sector(h, 0, size, true)?);

            let mut profiles = Vec::new();
            for entry in directory.iter().filter(|e| e.enabled) {
                let raw = onboard::read_sector(h, entry.sector, size, true)?;
                profiles.push(onboard::parse_profile(entry.sector, &raw, info.button_count)?);
            }
            Ok((info, profiles))
        })
    }

    /// Renders the given macro assignments into the device's onboard memory.
    ///
    /// Rebuilds the macro sector from `assignments` in full, then repoints the
    /// listed buttons. Buttons not mentioned are left exactly as they were.
    pub fn apply_onboard_macros(
        &self,
        id: &str,
        assignments: Vec<crate::hidpp::onboard::MacroAssignment>,
    ) -> Result<()> {
        use crate::hidpp::onboard;
        let mut inner = self.inner.lock();
        if inner.demo {
            return Ok(());
        }
        inner.with_handle(id, |h| {
            let info = onboard::read_info(h)?;
            let size = info.sector_size as usize;
            let directory = onboard::parse_directory(&onboard::read_sector(h, 0, size, true)?);
            let profile = directory
                .iter()
                .position(|e| e.enabled)
                .ok_or_else(|| Error::other("the device has no enabled onboard profile"))?;
            let sector = directory[profile].sector;
            let macro_sector = onboard::macro_sector_for(&info, profile);
            onboard::apply_macros(h, sector, macro_sector, &assignments, &info)
        })
    }

    /// Puts every sector of a backup file back onto the device.
    pub fn restore_onboard(&self, id: &str, path: &std::path::Path) -> Result<usize> {
        use crate::hidpp::onboard;
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::other(format!("could not read {}: {e}", path.display())))?;
        let backup: onboard::MemoryBackup = serde_json::from_str(&text)
            .map_err(|e| Error::other(format!("{} is not a valid backup: {e}", path.display())))?;

        let mut inner = self.inner.lock();
        inner.with_handle(id, |h| {
            let mut written = 0;
            for (sector, hex) in backup.sectors.iter().enumerate() {
                // Sector 0 is the directory; restoring it last would be safer,
                // but the device rejects an inconsistent directory anyway.
                onboard::restore_sector(h, sector as u16, hex)?;
                written += 1;
            }
            Ok(written)
        })
    }

    pub fn enumerate_features(&self, id: &str) -> Result<Vec<(u16, u8, u8)>> {
        let mut inner = self.inner.lock();
        inner.with_handle(id, |h| h.enumerate_features())
    }
}

impl Inner {
    fn ordered_snapshots(&self) -> Vec<DeviceSnapshot> {
        self.order.iter().filter_map(|id| self.snapshots.get(id).cloned()).collect()
    }

    fn refresh(&mut self) {
        if self.demo {
            self.load_demo();
            return;
        }
        let Some(api) = self.api.as_mut() else {
            return;
        };
        if let Err(e) = api.refresh_devices() {
            log::warn!("hid refresh failed: {e}");
        }

        let endpoints = hidpp::enumerate(api);
        let found_endpoints = !endpoints.is_empty() || !wheel::enumerate(api).is_empty();
        // Names of devices we could see but not open, so the UI can name them.
        let mut unopenable: Vec<String> = Vec::new();
        let mut seen: Vec<String> = Vec::new();

        for endpoint in &endpoints {
            // A receiver is a bridge, not a device: probe its six child slots.
            let indices: Vec<u8> = if endpoint.is_receiver {
                RECEIVER_CHILD_INDICES.to_vec()
            } else {
                vec![hidpp::DEVICE_INDEX_WIRED]
            };

            for index in indices {
                let mut address = endpoint.address.clone();
                address.device_index = index;
                let id = address.id();

                let handle = match self.handles.entry(id.clone()) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        match Handle::open(api, address.clone()) {
                            Ok(h) => e.insert(h),
                            Err(err) => {
                                log::debug!("cannot open {}: {err}", address.path);
                                let label = endpoint
                                    .hid_product
                                    .clone()
                                    .or_else(|| {
                                        registry::name_for(address.product_id).map(str::to_owned)
                                    })
                                    .unwrap_or_else(|| format!("{:04x}", address.product_id));
                                if !unopenable.contains(&label) {
                                    unopenable.push(label);
                                }
                                continue;
                            }
                        }
                    }
                };

                // A receiver slot with nothing paired into it never answers.
                if handle.ping().is_err() {
                    self.handles.remove(&id);
                    continue;
                }

                let mut snapshot = self
                    .snapshots
                    .remove(&id)
                    .unwrap_or_else(|| DeviceSnapshot::placeholder(&address, String::new(), DeviceKind::Other));

                let handle = self.handles.get_mut(&id).expect("just inserted");
                probe(handle, &mut snapshot, endpoint);
                self.snapshots.insert(id.clone(), snapshot);
                seen.push(id);
            }
        }

        // Classic wheels: no HID++, no ping — open the joystick interface.
        for endpoint in wheel::enumerate(api) {
            let id = format!("046d:{:04x}:wheel", endpoint.model.product_id);
            if !self.wheels.contains_key(&id) {
                match WheelHandle::open(api, &endpoint) {
                    Ok(h) => {
                        log::info!("wheel {} on {}", endpoint.model.name, endpoint.path);
                        self.wheels.insert(id.clone(), h);
                    }
                    Err(err) => {
                        log::debug!("cannot open wheel {}: {err}", endpoint.path);
                        let label = endpoint.hid_product.clone().unwrap_or_else(|| endpoint.model.name.to_string());
                        if !unopenable.contains(&label) {
                            unopenable.push(label);
                        }
                        continue;
                    }
                }
            }
            let handle = &self.wheels[&id];
            let mut snapshot = self
                .snapshots
                .remove(&id)
                .unwrap_or_else(|| DeviceSnapshot::wheel_placeholder(handle, endpoint.hid_product.clone()));
            if let Some(info) = snapshot.wheel.as_mut() {
                info.driver_running = handle.bridge_running();
            }
            self.snapshots.insert(id.clone(), snapshot);
            seen.push(id);
        }

        // Drop anything that disappeared, preserving the order of what remains.
        self.handles.retain(|id, _| seen.contains(id));
        self.wheels.retain(|id, _| seen.contains(id));
        self.snapshots.retain(|id, _| seen.contains(id));
        self.order = seen;

        if self.order.is_empty() {
            // Nothing usable. Work out *why* — the fix is completely different
            // for "no Logitech gear" versus "gear is there but root-only".
            let reason = if !found_endpoints {
                EmptyReason::NoHardware
            } else if !unopenable.is_empty() {
                EmptyReason::PermissionDenied { devices: unopenable }
            } else {
                EmptyReason::NoResponse
            };
            log::info!("no HID++ devices available ({reason:?}) — falling back to demo devices");
            self.empty_reason = Some(reason);
            self.demo = true;
            self.load_demo();
        } else {
            self.empty_reason = None;
        }
    }

    fn refresh_one(&mut self, id: &str) -> Result<DeviceSnapshot> {
        if self.demo {
            return self.snapshots.get(id).cloned().ok_or(Error::NotConnected);
        }
        let mut snapshot = self.snapshots.get(id).cloned().ok_or(Error::NotConnected)?;
        let handle = self.handles.get_mut(id).ok_or(Error::NotConnected)?;
        read_dynamic_state(handle, &mut snapshot);
        self.snapshots.insert(id.to_string(), snapshot.clone());
        Ok(snapshot)
    }

    fn with_handle<T>(&mut self, id: &str, f: impl FnOnce(&mut Handle) -> Result<T>) -> Result<T> {
        let handle = self.handles.get_mut(id).ok_or(Error::NotConnected)?;
        let result = f(handle);
        if let Err(Error::NotConnected) = result {
            self.handles.remove(id);
            if let Some(snap) = self.snapshots.get_mut(id) {
                snap.online = false;
            }
        }
        if let Err(e) = &result {
            if let Some(snap) = self.snapshots.get_mut(id) {
                snap.last_error = Some(e.to_string());
            }
        }
        result
    }

    // -- demo -------------------------------------------------------------

    fn load_demo(&mut self) {
        if self.order.iter().any(|id| self.snapshots.get(id).map(|s| s.demo).unwrap_or(false)) {
            return; // already populated
        }
        let devices = crate::demo::catalogue();
        self.order = devices.iter().map(|d| d.id.clone()).collect();
        self.snapshots = devices.into_iter().map(|d| (d.id.clone(), d)).collect();
    }

    fn demo_battery(&mut self, id: &str) -> Option<BatteryState> {
        let snap = self.snapshots.get_mut(id)?;
        let battery = snap.battery.as_mut()?;
        // Drift slowly so the UI's live updates are visibly working.
        battery.percentage = match battery.status {
            features::ChargeStatus::Charging => (battery.percentage + 1).min(100),
            _ => battery.percentage.saturating_sub(1).max(1),
        };
        Some(battery.clone())
    }

    fn demo_set_dpi(&mut self, id: &str, dpi: u16) -> Result<DpiState> {
        let snap = self.snapshots.get_mut(id).ok_or(Error::NotConnected)?;
        let state = snap.dpi.as_mut().ok_or(Error::UnsupportedFeature(features::dpi::ID))?;
        state.current = dpi.clamp(state.min.max(1), state.max.max(1));
        Ok(state.clone())
    }

    fn demo_set_rate(&mut self, id: &str, hz: u32) -> Result<ReportRateState> {
        let snap = self.snapshots.get_mut(id).ok_or(Error::NotConnected)?;
        let state = snap
            .report_rate
            .as_mut()
            .ok_or(Error::UnsupportedFeature(features::report_rate::ID))?;
        if let Some(best) = state.available_hz.iter().copied().min_by_key(|v| v.abs_diff(hz)) {
            state.current_hz = best;
        }
        Ok(state.clone())
    }
}

/// Fills in everything about a device: identity first, then live state.
/// The evdev nodes the kernel created for the HID interface behind a hidraw
/// path (`/dev/hidrawN` → `/sys/class/hidraw/hidrawN/device/input/*/event*`).
fn wheel_evdev_nodes(hidraw_path: &str) -> Vec<String> {
    let Some(node) = std::path::Path::new(hidraw_path).file_name().and_then(|n| n.to_str()) else {
        return vec![];
    };
    let input_dir = std::path::PathBuf::from("/sys/class/hidraw").join(node).join("device/input");
    let mut out = Vec::new();
    if let Ok(inputs) = std::fs::read_dir(&input_dir) {
        for input in inputs.flatten() {
            if let Ok(entries) = std::fs::read_dir(input.path()) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.starts_with("event") {
                        out.push(format!("/dev/input/{name}"));
                    }
                }
            }
        }
    }
    out
}

fn probe(handle: &mut Handle, snapshot: &mut DeviceSnapshot, endpoint: &hidpp::Endpoint) {
    snapshot.model_ids = features::read_model_ids(handle).unwrap_or_default();

    // Through a receiver the address carries the dongle's product id, which
    // identifies neither the model nor its artwork. Prefer what the device says
    // about itself.
    let pid = if endpoint.is_receiver {
        snapshot.model_ids.first().copied().unwrap_or(handle.address.product_id)
    } else {
        handle.address.product_id
    };
    snapshot.product_id = pid;

    if snapshot.name.is_empty() {
        // Curated name first: devices report their full marketing string via
        // 0x0005 ("G502 LIGHTSPEED Wireless Gaming Mouse") where G HUB shows the
        // short form. Unknown hardware still gets a correct name from the device.
        // Try every id the device claims, so a model listed under its wired id
        // is still recognised when it turns up wirelessly.
        snapshot.name = std::iter::once(pid)
            .chain(snapshot.model_ids.iter().copied())
            .find_map(registry::name_for)
            .map(str::to_owned)
            .or_else(|| features::read_device_name(handle).ok())
            .or_else(|| endpoint.hid_product.clone())
            .unwrap_or_else(|| format!("Logitech {pid:04x}"));
    }

    snapshot.kind = features::read_device_type(handle)
        .ok()
        .map(DeviceKind::from_hidpp_type)
        .filter(|k| *k != DeviceKind::Other)
        .or_else(|| {
            std::iter::once(pid)
                .chain(snapshot.model_ids.iter().copied())
                .find_map(registry::kind_for)
        })
        .unwrap_or(DeviceKind::Other);

    snapshot.serial = endpoint.serial.clone();
    snapshot.connection = if endpoint.is_receiver {
        Connection::Receiver
    } else if handle.address.device_index == hidpp::DEVICE_INDEX_WIRED {
        Connection::Wired
    } else {
        Connection::Wireless
    };
    let (major, minor) = handle.protocol_version;
    snapshot.protocol_version = format!("{major}.{minor}");
    snapshot.online = true;
    snapshot.demo = false;

    snapshot.capabilities = Capabilities {
        dpi: handle.supports(features::dpi::ID),
        report_rate: handle.supports(features::report_rate::ID)
            || handle.supports(features::report_rate::ID_EXTENDED),
        battery: handle.supports(features::battery::ID_UNIFIED)
            || handle.supports(features::battery::ID_LEVEL_STATUS)
            || handle.supports(features::battery::ID_VOLTAGE),
        lighting: handle.supports(features::lighting::ID_COLOR_LED_EFFECTS)
            || handle.supports(features::lighting::ID_RGB_EFFECTS),
        onboard_memory: handle.supports(0x8100),
        wheel: false,
    };

    read_dynamic_state(handle, snapshot);
}

/// The parts that change while the device is plugged in.
fn read_dynamic_state(handle: &mut Handle, snapshot: &mut DeviceSnapshot) {
    snapshot.last_error = None;

    if snapshot.capabilities.battery {
        match features::read_battery(handle) {
            Ok(b) => snapshot.battery = Some(b),
            Err(e) => log::debug!("battery read failed for {}: {e}", snapshot.name),
        }
    }
    if snapshot.capabilities.dpi {
        match features::read_dpi(handle, 0) {
            Ok(d) => snapshot.dpi = Some(d),
            Err(e) => log::debug!("dpi read failed for {}: {e}", snapshot.name),
        }
    }
    if snapshot.capabilities.report_rate {
        match features::read_report_rate(handle) {
            Ok(r) => snapshot.report_rate = Some(r),
            Err(e) => log::debug!("report rate read failed for {}: {e}", snapshot.name),
        }
    }
    if snapshot.capabilities.lighting {
        snapshot.lighting_zones = features::lighting_zone_count(handle).unwrap_or(1);
    }
}
