//! Firmware updates — what G HUB's "Firmware update" does, from the same source.
//!
//! **Where the packages come from.** Logitech publishes firmware as `*_dfu`
//! depots on the same public CDN as the device artwork (`depot.rs`), listed
//! in the depository. Each one holds a `dfu.json` — the version, the USB
//! interface ids it applies to (`046d_c08b`, with the bootloader id marked
//! `force`), start blockers such as *connect over USB* — release notes in
//! twenty languages, and the `.dfu` image whose SHA-256 the JSON carries.
//! We fetch every such depot once (about 3.5 MB in all), keep them under the
//! data directory, and match connected devices by product id. The catalogue
//! is only as fresh as the imported depository, since `current.json` itself
//! is not publicly downloadable.
//!
//! **How flashing works.** The `.dfu` image is literally the packet stream:
//! 16-byte HID++ 2.0 DFU commands, the first being `dfuStart` with the
//! firmware entity and its magic name (`U127_D1`). The device is first put
//! into its bootloader with feature `0x00C2` (DFU control signed: it reboots
//! itself) or `0x00C1` (DFU control: the user must replug); the bootloader
//! enumerates under its own product id and exposes feature `0x00D0`, to which
//! the packets go as function `dfuStart` (4) and then `dfuCmdData1..3,0`
//! in a sliding window, each answered with a status byte. `restart` (5) ends
//! it. This is the sequence fwupd's `logitech-hidpp` plugin uses for these
//! devices, ported here; a failed transfer leaves the device in its
//! bootloader, from where the update can simply be run again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::depot;
use crate::hidpp::{self, features, Error, Handle, ReportKind, Result};

// ---------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------

/// `dfu.json` as shipped in a `*_dfu` depot.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DfuJson {
    #[serde(default)]
    contents: Vec<DfuContent>,
    #[serde(default)]
    start_blockers: Vec<String>,
    #[serde(default)]
    start_warnings: Vec<String>,
    #[serde(default)]
    update_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DfuContent {
    #[serde(default)]
    interface_infos: Vec<DfuInterface>,
    version: String,
    binary_file_key: DfuBinaryKey,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DfuInterface {
    interface_id: String,
    #[serde(default)]
    updatable: bool,
    /// The bootloader's own id — "force" because a device stuck there must
    /// always be offered the image.
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct DfuBinaryKey {
    key: String,
    #[serde(default)]
    hash: String,
}

/// One firmware package, as cached on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub depot: String,
    /// `127.03.10` — G HUB's number.revision.build.
    pub version: String,
    /// Product ids this applies to (runtime ids).
    pub product_ids: Vec<u16>,
    /// Product ids of the bootloader, where the transfer happens.
    pub bootloader_ids: Vec<u16>,
    /// `BLOCKER_CONNECT_USB` and friends.
    pub start_blockers: Vec<String>,
    pub start_warnings: Vec<String>,
    pub update_required: bool,
    /// Release notes, HTML fragment, English.
    pub release_notes: String,
    /// Path of the `.dfu` image and its SHA-256 from `dfu.json`.
    pub image: String,
    pub image_sha256: String,
    pub image_size: u64,
    /// Depository hash of the depot the package came from, to notice changes.
    pub depot_mac: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub fetched: Option<String>,
    pub packages: Vec<Package>,
    /// Depots that could not be fetched or parsed, with the reason.
    #[serde(default)]
    pub failed: Vec<String>,
}

pub fn dir() -> PathBuf {
    crate::artwork::dir().parent().map(|d| d.join("firmware")).unwrap_or_else(|| PathBuf::from("openghub-firmware"))
}

fn catalog_path() -> PathBuf {
    dir().join("catalog.json")
}

pub fn load_catalog() -> Catalog {
    std::fs::read_to_string(catalog_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_catalog(c: &Catalog) -> Result<()> {
    std::fs::create_dir_all(dir()).map_err(|e| Error::other(format!("{}: {e}", dir().display())))?;
    let json = serde_json::to_string_pretty(c).map_err(|e| Error::other(e.to_string()))?;
    std::fs::write(catalog_path(), json).map_err(|e| Error::other(format!("could not write the firmware catalogue: {e}")))
}

fn parse_interface_id(s: &str) -> Option<u16> {
    let (vid, pid) = s.split_once('_')?;
    if u16::from_str_radix(vid, 16).ok()? != hidpp::LOGITECH_VID {
        return None;
    }
    u16::from_str_radix(pid, 16).ok()
}

/// Fetches every `*_dfu` depot the depository lists (skipping ones whose
/// hash has not changed since the last run) and rebuilds the catalogue.
pub fn refresh_catalog() -> Result<Catalog> {
    let (depository, _) = depot::load_cache().map_err(|e| Error::other(format!("no G HUB depository imported yet ({e}); firmware packages are listed there")))?;
    let previous = load_catalog();
    let mut catalog = Catalog { fetched: Some(now_iso()), packages: Vec::new(), failed: Vec::new() };
    for entry in depository.depots.iter().filter(|d| d.name.ends_with("_dfu")) {
        if entry.is_encrypted() {
            catalog.failed.push(format!("{}: encrypted", entry.name));
            continue;
        }
        if let Some(p) = previous.packages.iter().find(|p| p.depot == entry.name && p.depot_mac == entry.mac && Path::new(&p.image).is_file()) {
            catalog.packages.push(p.clone());
            continue;
        }
        match fetch_package(entry) {
            Ok(p) => catalog.packages.push(p),
            Err(e) => {
                log::warn!("firmware depot {}: {e}", entry.name);
                catalog.failed.push(format!("{}: {e}", entry.name));
            }
        }
    }
    save_catalog(&catalog)?;
    Ok(catalog)
}

fn fetch_package(entry: &depot::DepotEntry) -> Result<Package> {
    let files = depot::fetch_and_unpack(entry)?;
    let get = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, b)| b.as_slice());
    let dfu: DfuJson = serde_json::from_slice(get("dfu.json").ok_or_else(|| Error::other("no dfu.json"))?)
        .map_err(|e| Error::other(format!("dfu.json: {e}")))?;
    let content = dfu.contents.first().ok_or_else(|| Error::other("dfu.json lists no firmware"))?;
    // The manifest maps the binary key to its file name.
    let manifest: serde_json::Value = get("manifest.json")
        .and_then(|b| serde_json::from_slice(b).ok())
        .ok_or_else(|| Error::other("no manifest.json"))?;
    let resource = |key: &str| -> Option<String> {
        manifest
            .get("resources")?
            .as_array()?
            .iter()
            .find(|r| r.get("key").and_then(|k| k.as_str()) == Some(key))
            .and_then(|r| r.get("src").and_then(|s| s.as_str()).map(str::to_owned))
    };
    let image_name = resource(&content.binary_file_key.key).ok_or_else(|| Error::other("manifest names no firmware image"))?;
    let image = get(&image_name).ok_or_else(|| Error::other(format!("{image_name} missing from the depot")))?;
    let digest = depot::sha256_hex(image);
    if !content.binary_file_key.hash.is_empty() && !digest.eq_ignore_ascii_case(&content.binary_file_key.hash) {
        return Err(Error::other("firmware image hash does not match dfu.json"));
    }
    if image.len() % 16 != 0 || image.is_empty() {
        return Err(Error::other(format!("firmware image is {} bytes, not a whole number of DFU packets", image.len())));
    }
    let notes_name = manifest
        .pointer("/releaseNotes/content")
        .and_then(|s| s.as_str())
        .unwrap_or("release_notes/dfu_release_notes.html");
    let release_notes = get(notes_name).map(|b| String::from_utf8_lossy(b).trim_start_matches('\u{feff}').to_string()).unwrap_or_default();

    let pkg_dir = dir().join(&entry.name);
    std::fs::create_dir_all(&pkg_dir).map_err(|e| Error::other(format!("{}: {e}", pkg_dir.display())))?;
    let image_path = pkg_dir.join(&image_name);
    std::fs::write(&image_path, image).map_err(|e| Error::other(format!("could not write {}: {e}", image_path.display())))?;
    for (name, bytes) in &files {
        if name.ends_with(".json") || name.ends_with(".html") {
            let p = pkg_dir.join(name.replace('/', "_"));
            let _ = std::fs::write(p, bytes);
        }
    }

    let mut product_ids = Vec::new();
    let mut bootloader_ids = Vec::new();
    for i in &content.interface_infos {
        let Some(pid) = parse_interface_id(&i.interface_id) else { continue };
        if !i.updatable {
            continue;
        }
        if i.force {
            bootloader_ids.push(pid);
        } else {
            product_ids.push(pid);
        }
    }
    Ok(Package {
        depot: entry.name.clone(),
        version: content.version.clone(),
        product_ids,
        bootloader_ids,
        start_blockers: dfu.start_blockers.clone(),
        start_warnings: dfu.start_warnings.clone(),
        update_required: dfu.update_required,
        release_notes,
        image: image_path.to_string_lossy().into_owned(),
        image_sha256: digest,
        image_size: image.len() as u64,
        depot_mac: entry.mac.clone(),
    })
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Date only is plenty for "last checked".
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Matching a device
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateState {
    /// No package in the catalogue applies to this device.
    NoPackage,
    /// The package's version is what the device already runs.
    UpToDate,
    /// The package is newer than what is installed.
    UpdateAvailable,
    /// Versions cannot be compared with confidence; both are shown.
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareCheck {
    pub state: UpdateState,
    /// Installed main firmware as the device names it (`MPM17.00_B0008`).
    pub installed: Option<String>,
    /// …and in G HUB's notation (`17.00.8`), for the comparison.
    pub installed_ghub: Option<String>,
    pub package: Option<Package>,
    /// Why the update cannot start right now (`BLOCKER_CONNECT_USB` → wired).
    pub blockers: Vec<String>,
    pub catalog_fetched: Option<String>,
}

/// G HUB's `number.revision.build` from the device's `PPPnn.rr_Bbbbb`.
pub fn ghub_version(installed: &str) -> Option<(u32, u32, u32)> {
    let (name, build) = installed.split_once("_B")?;
    let (num, rev) = name.rsplit_once('.')?;
    let digits: String = num.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    Some((digits.parse().ok()?, rev.parse().ok()?, u32::from_str_radix(build, 16).ok()?))
}

fn parse_package_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.split('.');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// Compares what a device runs with a package version.
pub fn compare(installed: Option<&str>, package: &str) -> UpdateState {
    let (Some(inst), Some(pkg)) = (installed.and_then(ghub_version), parse_package_version(package)) else {
        return UpdateState::Unknown;
    };
    // The firmware number on the wire is two BCD digits; the package may
    // carry a longer one (U127 → "127"). Compare on the digits both have.
    let inst_num = inst.0 % 100;
    let pkg_num = pkg.0 % 100;
    if inst_num != pkg_num {
        return UpdateState::Unknown;
    }
    match (pkg.1, pkg.2).cmp(&(inst.1, inst.2)) {
        std::cmp::Ordering::Greater => UpdateState::UpdateAvailable,
        _ => UpdateState::UpToDate,
    }
}

/// The catalogue entry for a device, by any of its product ids.
pub fn package_for(catalog: &Catalog, product_ids: &[u16]) -> Option<Package> {
    catalog
        .packages
        .iter()
        .find(|p| product_ids.iter().any(|id| p.product_ids.contains(id) || p.bootloader_ids.contains(id)))
        .cloned()
}

pub fn check(snapshot: &crate::state::DeviceSnapshot) -> FirmwareCheck {
    let catalog = load_catalog();
    let mut ids = snapshot.model_ids.clone();
    ids.push(snapshot.product_id);
    let installed = snapshot.firmware.iter().find(|f| f.kind == "main").or(snapshot.firmware.first()).map(|f| f.version.clone());
    let installed_ghub = installed.as_deref().and_then(ghub_version).map(|(n, r, b)| format!("{n}.{r:02}.{b}"));
    let package = package_for(&catalog, &ids);
    let state = match &package {
        None => UpdateState::NoPackage,
        Some(p) => compare(installed.as_deref(), &p.version),
    };
    let mut blockers = Vec::new();
    if let Some(p) = &package {
        for b in &p.start_blockers {
            let blocked = match b.as_str() {
                "BLOCKER_CONNECT_USB" => !matches!(snapshot.connection, crate::state::Connection::Wired),
                _ => false,
            };
            if blocked {
                blockers.push(b.clone());
            }
        }
    }
    FirmwareCheck { state, installed, installed_ghub, package, blockers, catalog_fetched: catalog.fetched }
}

// ---------------------------------------------------------------------------
// Flashing (HID++ 2.0 DFU, as fwupd does it)
// ---------------------------------------------------------------------------

pub mod dfu {
    pub const CONTROL: u16 = 0x00c1;
    pub const CONTROL_SIGNED: u16 = 0x00c2;
    pub const CONTROL_BOLT: u16 = 0x00c3;
    pub const ID: u16 = 0x00d0;
    pub const FN_CMD_DATA0: u8 = 0;
    pub const FN_START: u8 = 4;
    pub const FN_RESTART: u8 = 5;
    pub const PACKET: usize = 16;

    // Status byte (low 7 bits) in every reply.
    pub const PACKET_SUCCESS: u8 = 0x01;
    pub const DFU_SUCCESS: u8 = 0x02;
    pub const WAIT_FOR_EVENT: u8 = 0x03;
    pub const DFU_SUCCESS_ENTITY_RESTART: u8 = 0x05;
    pub const DFU_SUCCESS_SYSTEM_RESTART: u8 = 0x06;
    pub const COMMAND_IN_PROGRESS: u8 = 0x12;
    pub const BLOCKED_COMMAND: u8 = 0x1c;

    pub fn status_name(s: u8) -> &'static str {
        match s & 0x7f {
            0x00 => "invalid",
            0x01 => "packet success",
            0x02 => "DFU success",
            0x03 => "wait for event",
            0x04 => "generic error",
            0x05 => "DFU success, entity restart required",
            0x06 => "DFU success, system restart required",
            0x10 => "generic error",
            0x11 => "bad voltage",
            0x12 => "unknown",
            0x13 => "unsupported encryption mode",
            0x14 => "bad magic string",
            0x15 => "erase failure",
            0x16 => "DFU not started",
            0x17 => "bad sequence number",
            0x18 => "unsupported command",
            0x19 => "command in progress",
            0x1a => "address out of range",
            0x1b => "unaligned address",
            0x1c => "bad size",
            0x1d => "missing program data",
            0x1e => "missing check data",
            0x1f => "program failed to write",
            0x20 => "program failed to verify",
            0x21 => "bad firmware",
            0x22 => "firmware check failure",
            0x23 => "blocked command",
            _ => "unknown status",
        }
    }

    /// Whether a status means "carry on", "wait for the device", or "stop".
    pub enum Outcome {
        Ok,
        Busy,
        Failed,
    }

    pub fn classify(status: u8) -> Outcome {
        match status & 0x7f {
            0x01 | 0x02 | 0x05 | 0x06 => Outcome::Ok,
            0x03 | 0x19 | 0x23 => Outcome::Busy,
            _ => Outcome::Failed,
        }
    }
}

/// Progress of a running update, sent to the UI as it goes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub device_id: String,
    pub stage: String,
    /// 0-100 across the whole update.
    pub percent: u8,
    pub done: bool,
    pub error: Option<String>,
}

/// Asks a running device to reboot into its bootloader. Returns whether the
/// user has to replug it (`0x00C1`) or it restarts by itself (`0x00C2`).
pub fn enter_bootloader(h: &mut Handle) -> Result<bool> {
    const MAGIC: [u8; 7] = [0x01, 0x00, 0x00, 0x00, b'D', b'F', b'U'];
    if h.supports(dfu::CONTROL_SIGNED) {
        let idx = h.feature_index(dfu::CONTROL_SIGNED)?;
        // startDfu: the device answers, then drops off the bus.
        match h.call(idx, 1, &MAGIC, ReportKind::Long) {
            Ok(_) | Err(Error::Timeout) | Err(Error::NotConnected) => {}
            Err(e) => return Err(e),
        }
        return Ok(false);
    }
    if h.supports(dfu::CONTROL) || h.supports(dfu::CONTROL_BOLT) {
        let feature = if h.supports(dfu::CONTROL_BOLT) { dfu::CONTROL_BOLT } else { dfu::CONTROL };
        let idx = h.feature_index(feature)?;
        match h.call(idx, 1, &MAGIC, ReportKind::Long) {
            Ok(_) | Err(Error::Timeout) | Err(Error::NotConnected) => {}
            Err(e) => return Err(e),
        }
        return Ok(true);
    }
    Err(Error::other("this device has no DFU control feature; it cannot be updated over HID++"))
}

/// Streams the image into a bootloader that exposes feature 0x00D0.
/// `on_progress(sent, total)` is called per packet.
pub fn flash(h: &mut Handle, image: &[u8], mut on_progress: impl FnMut(usize, usize)) -> Result<()> {
    if image.is_empty() || image.len() % dfu::PACKET != 0 {
        return Err(Error::other("the firmware image is not a whole number of DFU packets"));
    }
    let idx = h.feature_index(dfu::ID).map_err(|_| Error::other("the bootloader does not expose the DFU feature"))?;
    let total = image.len() / dfu::PACKET;
    let mut cmd = dfu::FN_START;
    for (i, chunk) in image.chunks(dfu::PACKET).enumerate() {
        send_packet(h, idx, cmd, chunk).map_err(|e| Error::other(format!("packet {} of {total} (function {cmd}): {e}", i + 1)))?;
        // Sliding window: dfuStart, then dfuCmdData1, 2, 3, 0, 1, …
        cmd = (cmd + 1) % 4;
        on_progress(i + 1, total);
    }
    Ok(())
}

/// One DFU packet: send, read the status, and if the device says "wait",
/// wait for its notification (flash erases take a while).
fn send_packet(h: &mut Handle, idx: u8, function: u8, data: &[u8]) -> Result<()> {
    let reply = h.call(idx, function, data, ReportKind::Long)?;
    let status = reply.param(4);
    match dfu::classify(status) {
        dfu::Outcome::Ok => return Ok(()),
        dfu::Outcome::Failed => return Err(Error::other(format!("device reported: {}", dfu::status_name(status)))),
        dfu::Outcome::Busy => {}
    }
    // The device continues with a notification carrying the same layout.
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        for ev in h.poll_events() {
            if ev.feature_index != idx {
                continue;
            }
            let s = ev.param(4);
            match dfu::classify(s) {
                dfu::Outcome::Ok => return Ok(()),
                dfu::Outcome::Busy => {}
                dfu::Outcome::Failed => return Err(Error::other(format!("device reported: {}", dfu::status_name(s)))),
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(Error::other("timed out waiting for the device to finish the packet"))
}

/// Ends the transfer: the bootloader starts the new firmware.
pub fn restart(h: &mut Handle, entity: u8) -> Result<()> {
    let idx = h.feature_index(dfu::ID)?;
    match h.call(idx, dfu::FN_RESTART, &[entity], ReportKind::Long) {
        Ok(_) | Err(Error::Timeout) | Err(Error::NotConnected) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Finds the bootloader on the bus after `enter_bootloader`: any HID++
/// endpoint whose product id is one the package names as its bootloader.
pub fn find_bootloader(api: &hidapi::HidApi, bootloader_ids: &[u16]) -> Option<hidpp::Endpoint> {
    hidpp::enumerate(api).into_iter().find(|e| bootloader_ids.contains(&e.address.product_id))
}

/// Reads the firmware entity byte the image's first packet names.
pub fn image_entity(image: &[u8]) -> u8 {
    image.first().copied().unwrap_or(0)
}

/// The image's magic string (`U127_D1`) from the start packet, for display.
pub fn image_magic(image: &[u8]) -> String {
    image.get(2..16).map(|b| b.iter().take_while(|c| **c != 0).map(|c| *c as char).collect()).unwrap_or_default()
}

/// Feature list of a bootloader: enough to say whether it can take an image.
pub fn bootloader_ready(h: &mut Handle) -> bool {
    h.supports(dfu::ID)
}

pub fn read_installed(h: &mut Handle) -> Vec<features::FirmwareInfo> {
    features::read_firmware(h).unwrap_or_default()
}

/// Product ids → package, for the UI's "N updates available" summary.
pub fn index_by_product(catalog: &Catalog) -> HashMap<u16, Package> {
    let mut m = HashMap::new();
    for p in &catalog.packages {
        for id in p.product_ids.iter().chain(p.bootloader_ids.iter()) {
            m.insert(*id, p.clone());
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_versions_map_to_ghub_notation() {
        assert_eq!(ghub_version("MPM17.00_B0008"), Some((17, 0, 8)));
        assert_eq!(ghub_version("U1227.03_B0010"), Some((1227, 3, 16)));
        assert_eq!(ghub_version("BOT92.00_B0008"), Some((92, 0, 8)));
        assert_eq!(ghub_version("garbage"), None);
    }

    #[test]
    fn comparison_uses_revision_and_build() {
        assert_eq!(compare(Some("MPM17.00_B0008"), "17.00.8"), UpdateState::UpToDate);
        assert_eq!(compare(Some("MPM17.00_B0008"), "17.01.2"), UpdateState::UpdateAvailable);
        assert_eq!(compare(Some("MPM17.02_B0001"), "17.01.9"), UpdateState::UpToDate);
        assert_eq!(compare(Some("MPM17.00_B0008"), "127.00.8"), UpdateState::Unknown);
        assert_eq!(compare(None, "17.00.8"), UpdateState::Unknown);
    }

    #[test]
    fn interface_ids_parse() {
        assert_eq!(parse_interface_id("046d_c08b"), Some(0xc08b));
        assert_eq!(parse_interface_id("046d_AAE6"), Some(0xaae6));
        assert_eq!(parse_interface_id("1234_c08b"), None);
    }

    #[test]
    fn dfu_window_and_status() {
        let mut cmd = dfu::FN_START;
        let seq: Vec<u8> = (0..6)
            .map(|_| {
                let c = cmd;
                cmd = (cmd + 1) % 4;
                c
            })
            .collect();
        assert_eq!(seq, vec![4, 1, 2, 3, 0, 1]);
        assert!(matches!(dfu::classify(0x81), dfu::Outcome::Ok));
        assert!(matches!(dfu::classify(0x03), dfu::Outcome::Busy));
        assert!(matches!(dfu::classify(0x14), dfu::Outcome::Failed));
        let img = b"\x00\x01U127_D1\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(image_entity(img), 0);
        assert_eq!(image_magic(img), "U127_D1");
    }

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }
}
