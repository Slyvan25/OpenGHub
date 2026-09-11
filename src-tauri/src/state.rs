//! Device manager: owns the HID handles, caches what we know about each device
//! and keeps the frontend's view of the world in sync.

use std::collections::HashMap;

use hidapi::HidApi;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::hidpp::features::{self, BatteryState, DpiState, ReportRateState};
use crate::hidpp::registry::{self, DeviceKind};
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
            protocol_version: String::new(),
            demo: false,
            last_error: None,
        }
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
        let found_endpoints = !endpoints.is_empty();
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

        // Drop anything that disappeared, preserving the order of what remains.
        self.handles.retain(|id, _| seen.contains(id));
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
