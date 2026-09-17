//! G HUB depots: the per-device asset packages and the manifest that lists them.
//!
//! Established from a G HUB 39.1 `C:\ProgramData\LGHUB` tree and a matching
//! network capture, and cross-checked against the device's own HID++ replies:
//!
//! - `current.json` (the *depository*) lists every depot of a build with a
//!   stable UUID URL on `updates.ghub.logitechg.com`, a SHA-256 `mac` and a
//!   size. Device depots have empty `cipherSuite`/`iv`/`key`: they are served
//!   **unencrypted from a public CDN**, verified by hash and RSA signature.
//! - A `.depot` file is `magic(4) | json_len(u32 LE) | json | [len(u32 LE) | bytes]*`,
//!   with the JSON naming the files in order. No compression.
//! - `core/data/devices/devices_NNNN.json` maps `046d_<pid>` interface ids to a
//!   `modelId` (`g502_wireless`), its depot name, display name and lighting
//!   zone types. Files starting with `21 05 21 20` are encrypted — presumably
//!   unannounced hardware — and are skipped.
//! - A device depot's `metadata.json` gives, in image pixels, a rectangle per
//!   lighting zone and a marker + label position per button. That is the data
//!   G HUB draws its render from, and what OpenGHub needs to be pixel-accurate.
//!
//! Nothing here is bundled with OpenGHub: it reads a G HUB installation the
//! user already has, or fetches a device's own depot the way G HUB does.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hidpp::Error;

pub const DEPOT_MAGIC: [u8; 4] = [0x10, 0x01, 0x17, 0x20];
const ENCRYPTED_MAGIC: [u8; 4] = [0x21, 0x05, 0x21, 0x20];

// ---------------------------------------------------------------------------
// .depot container
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DepotIndex {
    files: Vec<DepotIndexEntry>,
}

#[derive(Debug, Deserialize)]
struct DepotIndexEntry {
    name: String,
    #[allow(dead_code)]
    mode: u32,
}

/// Splits a `.depot` into its files. Names are as recorded; contents verbatim.
pub fn unpack(blob: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Error> {
    if blob.len() < 8 || blob[..4] != DEPOT_MAGIC {
        return Err(Error::other("not a .depot file (bad magic)"));
    }
    let json_len = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) as usize;
    let index: DepotIndex = serde_json::from_slice(
        blob.get(8..8 + json_len).ok_or_else(|| Error::other("truncated depot header"))?,
    )
    .map_err(|e| Error::other(format!("depot index is not JSON: {e}")))?;

    let mut pos = 8 + json_len;
    let mut out = Vec::with_capacity(index.files.len());
    for entry in index.files {
        let len_bytes = blob
            .get(pos..pos + 4)
            .ok_or_else(|| Error::other(format!("truncated before {}", entry.name)))?;
        let len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
        pos += 4;
        let data = blob
            .get(pos..pos + len)
            .ok_or_else(|| Error::other(format!("truncated inside {}", entry.name)))?;
        pos += len;
        out.push((entry.name, data.to_vec()));
    }
    if pos != blob.len() {
        return Err(Error::other(format!("{} trailing bytes after last file", blob.len() - pos)));
    }
    Ok(out)
}

/// Builds a `.depot` — used by tests, and handy for round-tripping.
pub fn pack(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let index = serde_json::json!({
        "files": files.iter().map(|(n, _)| serde_json::json!({"name": n, "mode": 33206})).collect::<Vec<_>>()
    });
    let json = serde_json::to_vec(&index).expect("static json");
    let mut out = Vec::new();
    out.extend_from_slice(&DEPOT_MAGIC);
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    for (_, data) in files {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

// ---------------------------------------------------------------------------
// Depository (current.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepotEntry {
    pub name: String,
    pub url: String,
    /// SHA-256 of the `.depot` file, hex.
    #[serde(default)]
    pub mac: String,
    #[serde(default, deserialize_with = "string_or_number")]
    pub size: u64,
    #[serde(default)]
    pub cipher_suite: String,
    #[serde(default)]
    pub files: Vec<String>,
}

impl DepotEntry {
    /// Device depots are plain; only some application depots are encrypted.
    pub fn is_encrypted(&self) -> bool {
        !self.cipher_suite.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Depository {
    pub version: String,
    #[serde(default, deserialize_with = "string_or_number_str")]
    pub build_id: String,
    #[serde(default)]
    pub branch: String,
    pub depots: Vec<DepotEntry>,
}

impl Depository {
    pub fn depot(&self, name: &str) -> Option<&DepotEntry> {
        self.depots.iter().find(|d| d.name == name)
    }
}

fn string_or_number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        S(String),
        N(u64),
    }
    Ok(match V::deserialize(d)? {
        V::S(s) => s.parse().unwrap_or(0),
        V::N(n) => n,
    })
}

fn string_or_number_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        S(String),
        N(u64),
    }
    Ok(match V::deserialize(d)? {
        V::S(s) => s,
        V::N(n) => n.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Device database (core/data/devices/*.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDef {
    pub model_id: String,
    pub display_name: String,
    #[serde(default)]
    pub depot: String,
    #[serde(default)]
    pub slot_prefix: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    /// `pipeline://core_assets/thumbnails/<file>` — the dashboard card image.
    #[serde(default)]
    pub thumbnail: String,
    /// Product ids this definition covers, e.g. `[0xc08d, 0x407f]`.
    #[serde(default)]
    pub product_ids: Vec<u16>,
    /// `ZONE_PRIMARY` → `PRIMARY`, `ZONE_BRANDING` → `LOGO`.
    #[serde(default)]
    pub zone_type_map: HashMap<String, String>,
}

/// Parses one `devices_NNNN.json`. Encrypted files yield an empty list.
pub fn parse_device_file(bytes: &[u8]) -> Vec<DeviceDef> {
    if bytes.len() < 4 || bytes[..4] == ENCRYPTED_MAGIC || bytes[0] != b'{' {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(bytes);
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(list) = root.get("devices").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    list.iter()
        .filter_map(|d| {
            let model_id = d.get("modelId")?.as_str()?.to_string();
            let mut pids = Vec::new();
            for mode in d.get("modes").and_then(|m| m.as_array()).into_iter().flatten() {
                for iface in mode.get("interfaces").and_then(|i| i.as_array()).into_iter().flatten() {
                    if let Some(id) = iface.get("id").and_then(|v| v.as_str()) {
                        // "046d_407f" → 0x407f
                        if let Some(pid) = id.strip_prefix("046d_") {
                            if let Ok(n) = u16::from_str_radix(pid, 16) {
                                if !pids.contains(&n) {
                                    pids.push(n);
                                }
                            }
                        }
                    }
                }
            }
            let zone_type_map = d
                .pointer("/capabilities/lightingSupport/typeMap")
                .and_then(|m| m.as_object())
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            Some(DeviceDef {
                model_id,
                display_name: d.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                depot: d.get("depot").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                slot_prefix: d.get("slotPrefix").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                kind: d.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                thumbnail: d.get("thumbnail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                product_ids: pids,
                zone_type_map,
            })
        })
        .collect()
}

/// Loads every readable device definition under `core/data/devices/`.
pub fn load_device_db(core_dir: &Path) -> Vec<DeviceDef> {
    let dir = core_dir.join("data").join("devices");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if let Ok(bytes) = std::fs::read(entry.path()) {
            out.extend(parse_device_file(&bytes));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Device image metadata (metadata.json in a device depot)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct RawMetadata {
    images: Vec<RawImage>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawImage {
    key: String,
    origin: RawSize,
    #[serde(default)]
    assignments: Vec<RawAssignment>,
    #[serde(default)]
    zones: Vec<RawZone>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSize {
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAssignment {
    slot_id: String,
    marker: RawPoint,
    label: RawPoint,
}

#[derive(Debug, Clone, Deserialize)]
struct RawPoint {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct RawZone {
    id: String,
    #[serde(default)]
    components: Vec<RawComponent>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawComponent {
    #[serde(default)]
    renderings: Vec<RawRendering>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRendering {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    rect: Option<RawRect>,
}

// The rect values are strings in the wild ("405"), so accept both.
#[derive(Debug, Clone, Deserialize)]
struct RawRect {
    #[serde(deserialize_with = "num_str")]
    x: f32,
    #[serde(deserialize_with = "num_str")]
    y: f32,
    #[serde(deserialize_with = "num_str")]
    width: f32,
    #[serde(deserialize_with = "num_str")]
    height: f32,
}

fn num_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        S(String),
        N(f64),
    }
    Ok(match V::deserialize(d)? {
        V::S(s) => s.parse().unwrap_or(0.0),
        V::N(n) => n as f32,
    })
}

/// What OpenGHub keeps: everything normalised to 0–1 of the image, so it
/// applies at any render size.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkLayout {
    pub model_id: String,
    pub display_name: String,
    /// `front` and, when present, `side`.
    pub views: Vec<ArtworkView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkView {
    /// `device_image` → `front`, `device_side` → `side`.
    pub view: String,
    pub width: f32,
    pub height: f32,
    pub zones: Vec<ZoneRect>,
    pub controls: Vec<ControlMarker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZoneRect {
    /// G HUB zone type, e.g. `ZONE_PRIMARY`.
    pub id: String,
    /// The HID++ location name it maps to: `Primary`, `Logo`, … from the
    /// device definition's `typeMap`, so it lines up with `getZoneInfo`.
    pub location_name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControlMarker {
    /// G HUB slot id, e.g. `g502wireless_g7_m1`.
    pub slot_id: String,
    /// OpenGHub's control id, e.g. `button-7`, derived from the `gN` in the slot.
    pub control: String,
    pub marker_x: f32,
    pub marker_y: f32,
    /// Label anchor, normalised; may lie outside 0–1, which is how G HUB puts
    /// labels in the margins beside the render.
    pub label_x: f32,
    pub label_y: f32,
    pub side: String,
}

/// Turns a depot's `metadata.json` into a normalised layout.
pub fn parse_metadata(
    bytes: &[u8],
    model_id: &str,
    display_name: &str,
    zone_type_map: &HashMap<String, String>,
) -> Result<ArtworkLayout, Error> {
    let raw: RawMetadata = serde_json::from_slice(bytes)
        .map_err(|e| Error::other(format!("metadata.json is malformed: {e}")))?;

    let views = raw
        .images
        .into_iter()
        .map(|img| {
            let (w, h) = (img.origin.width.max(1.0), img.origin.height.max(1.0));
            let zones = img
                .zones
                .iter()
                .flat_map(|z| {
                    z.components.iter().flat_map(|c| c.renderings.iter()).filter_map(|r| {
                        let rect = r.rect.as_ref().filter(|_| r.kind == "rect")?;
                        Some(ZoneRect {
                            id: z.id.clone(),
                            location_name: zone_location_name(&z.id, zone_type_map),
                            x: rect.x / w,
                            y: rect.y / h,
                            width: rect.width / w,
                            height: rect.height / h,
                        })
                    })
                })
                .collect();
            let controls = img
                .assignments
                .iter()
                .map(|a| {
                    let label_x = a.label.x / w;
                    ControlMarker {
                        slot_id: a.slot_id.clone(),
                        control: control_id_for_slot(&a.slot_id),
                        marker_x: a.marker.x / w,
                        marker_y: a.marker.y / h,
                        label_x,
                        label_y: a.label.y / h,
                        side: if label_x < 0.0 {
                            "left".into()
                        } else if label_x > 1.0 {
                            "right".into()
                        } else {
                            "top".into()
                        },
                    }
                })
                .collect();
            ArtworkView {
                view: match img.key.as_str() {
                    "device_image" => "front".into(),
                    "device_side" => "side".into(),
                    other => other.to_string(),
                },
                width: w,
                height: h,
                zones,
                controls,
            }
        })
        .collect();

    Ok(ArtworkLayout { model_id: model_id.into(), display_name: display_name.into(), views })
}

/// `ZONE_PRIMARY` → `Primary` via the typeMap (`PRIMARY`), title-cased to match
/// `features::zone_location_name`.
fn zone_location_name(zone_id: &str, type_map: &HashMap<String, String>) -> String {
    let mapped = type_map.get(zone_id).map(String::as_str).unwrap_or(zone_id);
    let base = mapped.strip_prefix("ZONE_").unwrap_or(mapped).to_ascii_lowercase();
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// `g502wireless_g7_m1` → `button-7`; anything unrecognised keeps the slot id.
pub fn control_id_for_slot(slot: &str) -> String {
    slot.split('_')
        .find_map(|part| part.strip_prefix('g').and_then(|n| n.parse::<u32>().ok()))
        .map(|n| format!("button-{n}"))
        .unwrap_or_else(|| slot.to_string())
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// Downloads a depot, verifies it against the depository's SHA-256, and unpacks it.
pub fn fetch_and_unpack(entry: &DepotEntry) -> Result<Vec<(String, Vec<u8>)>, Error> {
    if entry.is_encrypted() {
        return Err(Error::other(format!(
            "depot '{}' is encrypted ({}); only plain device depots are supported",
            entry.name, entry.cipher_suite
        )));
    }
    let mut reader = ureq::get(&entry.url)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| Error::other(format!("download of {}: {e}", entry.name)))?
        .into_reader();
    let mut blob = Vec::with_capacity(entry.size as usize);
    std::io::Read::read_to_end(&mut reader, &mut blob)
        .map_err(|e| Error::other(format!("reading {}: {e}", entry.name)))?;

    if !entry.mac.is_empty() {
        let digest = sha256_hex(&blob);
        if !digest.eq_ignore_ascii_case(&entry.mac) {
            return Err(Error::other(format!(
                "depot '{}' failed its checksum — refusing to use it",
                entry.name
            )));
        }
    }
    unpack(&blob)
}

/// Plain SHA-256; kept local so the crate does not pull in a hashing stack.
pub fn sha256_hex(data: &[u8]) -> String {
    // FIPS 180-4, straightforward implementation — the inputs are a few MB.
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (i, v) in [a, b, c, d, e, f, g, hh].iter().enumerate() {
            h[i] = h[i].wrapping_add(*v);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// Where OpenGHub keeps its copy of the depository and device database, so
/// depots can be fetched for devices that were never seen by the imported
/// G HUB installation.
pub fn cache_dir() -> PathBuf {
    crate::artwork::dir()
        .parent()
        .map(|d| d.join("ghub"))
        .unwrap_or_else(|| PathBuf::from("openghub-ghub"))
}

// ---------------------------------------------------------------------------
// Import into OpenGHub's artwork directory
// ---------------------------------------------------------------------------

/// One device whose artwork and layout were written out.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedDevice {
    pub model_id: String,
    pub display_name: String,
    pub product_ids: Vec<u16>,
    pub views: Vec<String>,
    pub has_thumbnail: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub build_id: String,
    pub depots_in_depository: usize,
    pub device_definitions: usize,
    pub imported: Vec<ImportedDevice>,
    pub artwork_dir: String,
}

/// Writes one unpacked device depot into the artwork directory, keyed by
/// every product id the definition lists. Returns what was written.
pub fn import_device_files(
    files: &[(String, Vec<u8>)],
    def: &DeviceDef,
    thumbnail: Option<&[u8]>,
) -> Result<ImportedDevice, Error> {
    let dir = crate::artwork::ensure_dir()
        .map_err(|e| Error::other(format!("artwork directory: {e}")))?;
    let get = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_slice());

    // The manifest names the resources; fall back to the conventional names.
    let mut front = "front.png".to_string();
    let mut side = "side.png".to_string();
    // Wheels ship the rim (`wheel_image_front`) over a static base.
    let mut base: Option<String> = None;
    let mut pedals: Option<String> = None;
    let mut metadata = "metadata.json".to_string();
    if let Some(manifest) = get("manifest.json") {
        if let Ok(m) = serde_json::from_slice::<serde_json::Value>(manifest) {
            if let Some(dev) = m
                .get("devices")
                .and_then(|d| d.as_array())
                .and_then(|a| a.iter().find(|d| d.get("modelId").and_then(|v| v.as_str()) == Some(&def.model_id)))
            {
                for r in dev.get("resources").and_then(|r| r.as_array()).into_iter().flatten() {
                    match (r.get("key").and_then(|v| v.as_str()), r.get("src").and_then(|v| v.as_str())) {
                        (Some("device_image"), Some(src)) => front = src.to_string(),
                        (Some("device_side"), Some(src)) => side = src.to_string(),
                        (Some("wheel_image_base"), Some(src)) => base = Some(src.to_string()),
                        (Some("pedals_image"), Some(src)) => pedals = Some(src.to_string()),
                        (Some("image_metadata"), Some(src)) => metadata = src.to_string(),
                        _ => {}
                    }
                }
            }
        }
    }

    // Wheel depots use a different metadata schema (zoom regions and slot
    // markers); no layout is not a failure.
    let layout = match get(&metadata) {
        Some(bytes) => match parse_metadata(bytes, &def.model_id, &def.display_name, &def.zone_type_map) {
            Ok(l) => Some(l),
            Err(e) => {
                log::info!("{}: no zone layout in metadata ({e})", def.display_name);
                None
            }
        },
        None => None,
    };
    /// Keeps the source's image format: the wheels ship WebP.
    fn ext_of(name: &str) -> &str {
        match name.rsplit('.').next() {
            Some(e) if matches!(e, "png" | "webp" | "jpg" | "jpeg") => e,
            _ => "png",
        }
    }

    let mut views = Vec::new();
    for pid in &def.product_ids {
        let key = crate::artwork::key(*pid);
        let write = |suffix: &str, data: &[u8]| -> Result<(), Error> {
            let path = dir.join(format!("{key}{suffix}"));
            std::fs::write(&path, data)
                .map_err(|e| Error::other(format!("could not write {}: {e}", path.display())))
        };
        if let Some(png) = get(&front) {
            write(&format!(".{}", ext_of(&front)), png)?;
            if !views.contains(&"front".to_string()) {
                views.push("front".into());
            }
        }
        if let Some(png) = get(&side) {
            write(&format!("-side.{}", ext_of(&side)), png)?;
            if !views.contains(&"side".to_string()) {
                views.push("side".into());
            }
        }
        if let Some(name) = &base {
            if let Some(img) = get(name) {
                write(&format!("-base.{}", ext_of(name)), img)?;
            }
        }
        if let Some(name) = &pedals {
            if let Some(img) = get(name) {
                write(&format!("-pedals.{}", ext_of(name)), img)?;
            }
        }
        if let Some(png) = thumbnail {
            write("-thumb.png", png)?;
        }
        if let Some(l) = &layout {
            let json = serde_json::to_vec_pretty(l)
                .map_err(|e| Error::other(format!("could not serialise layout: {e}")))?;
            write(".layout.json", &json)?;
        }
    }

    Ok(ImportedDevice {
        model_id: def.model_id.clone(),
        display_name: def.display_name.clone(),
        product_ids: def.product_ids.clone(),
        views,
        has_thumbnail: thumbnail.is_some(),
    })
}

/// Reads a whole `C:\ProgramData\LGHUB` tree: caches the depository and the
/// device database, then imports every device depot already present on disk.
pub fn import_program_data(root: &Path) -> Result<ImportReport, Error> {
    // Accept either the ProgramData root or the LGHUB folder inside it.
    let lghub = if root.join("current.json").exists() { root.to_path_buf() } else { root.join("LGHUB") };
    let current = lghub.join("current.json");
    let text = std::fs::read_to_string(&current)
        .map_err(|e| Error::other(format!("{} — is this the ProgramData\\LGHUB folder? ({e})", current.display())))?;
    let depository: Depository = serde_json::from_str(&text)
        .map_err(|e| Error::other(format!("current.json is malformed: {e}")))?;

    let build_dir = lghub.join("depots").join(&depository.build_id);
    let defs = load_device_db(&build_dir.join("core"));
    if defs.is_empty() {
        return Err(Error::other(format!(
            "no readable device definitions under {}",
            build_dir.join("core/data/devices").display()
        )));
    }

    // Cache the depository + device DB so depots can be fetched later for
    // devices this installation never saw.
    let cache = cache_dir();
    std::fs::create_dir_all(&cache).map_err(|e| Error::other(format!("{}: {e}", cache.display())))?;
    std::fs::write(cache.join("depository.json"), &text)
        .map_err(|e| Error::other(format!("could not cache depository: {e}")))?;
    std::fs::write(
        cache.join("devices.json"),
        serde_json::to_vec_pretty(&defs).map_err(|e| Error::other(e.to_string()))?,
    )
    .map_err(|e| Error::other(format!("could not cache device db: {e}")))?;

    let thumbs = build_dir.join("core_assets").join("thumbnails");
    let mut imported = Vec::new();
    for def in &defs {
        if def.depot.is_empty() || def.product_ids.is_empty() {
            continue;
        }
        let depot_dir = build_dir.join(&def.depot);
        if !depot_dir.is_dir() {
            continue; // not downloaded by G HUB on that machine
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&depot_dir).into_iter().flatten().flatten() {
            if entry.path().is_file() {
                if let Ok(data) = std::fs::read(entry.path()) {
                    files.push((entry.file_name().to_string_lossy().to_string(), data));
                }
            }
        }
        let thumb_name = def.thumbnail.rsplit('/').next().unwrap_or("");
        let thumb = std::fs::read(thumbs.join(thumb_name)).ok();
        match import_device_files(&files, def, thumb.as_deref()) {
            Ok(d) => imported.push(d),
            Err(e) => log::warn!("skipping {}: {e}", def.model_id),
        }
    }

    Ok(ImportReport {
        build_id: depository.build_id.clone(),
        depots_in_depository: depository.depots.len(),
        device_definitions: defs.len(),
        imported,
        artwork_dir: crate::artwork::dir().display().to_string(),
    })
}

/// Loads the cached depository and device database written by an import.
pub fn load_cache() -> Result<(Depository, Vec<DeviceDef>), Error> {
    let cache = cache_dir();
    let dep = std::fs::read_to_string(cache.join("depository.json"))
        .map_err(|_| Error::other("no G HUB data imported yet — import a ProgramData\\LGHUB folder first"))?;
    let depository: Depository =
        serde_json::from_str(&dep).map_err(|e| Error::other(format!("cached depository: {e}")))?;
    let db = std::fs::read_to_string(cache.join("devices.json"))
        .map_err(|_| Error::other("cached device database missing"))?;
    let defs: Vec<DeviceDef> =
        serde_json::from_str(&db).map_err(|e| Error::other(format!("cached device db: {e}")))?;
    Ok((depository, defs))
}

/// Devices whose definitions sit in the encrypted part of G HUB's device
/// database. Their depots are public like every other device depot; only the
/// mapping from product id to depot name had to be read off the depository.
pub fn builtin_defs() -> Vec<DeviceDef> {
    let wheel = |model: &str, name: &str, depot: &str, pids: &[u16]| DeviceDef {
        model_id: model.into(),
        display_name: name.into(),
        depot: depot.into(),
        slot_prefix: model.replace('_', "-"),
        kind: "wheel".into(),
        thumbnail: String::new(),
        product_ids: pids.to_vec(),
        zone_type_map: Default::default(),
    };
    vec![
        wheel("g923_ps4", "G923 Racing Wheel", "g923_ps4", &[0xc267, 0xc266]),
        wheel("g923_xbox", "G923 Racing Wheel", "g923_xbox", &[0xc26e, 0xc26d]),
    ]
}

/// Fetches and imports the depot for a device, found by any of its product ids.
/// This is what G HUB does when it first sees a device.
pub fn fetch_for_product_ids(product_ids: &[u16]) -> Result<ImportedDevice, Error> {
    let (depository, mut defs) = load_cache()?;
    defs.extend(builtin_defs());
    let def = defs
        .iter()
        .find(|d| d.product_ids.iter().any(|p| product_ids.contains(p)))
        .ok_or_else(|| {
            Error::other(format!(
                "no G HUB device definition for product id(s) {}",
                product_ids.iter().map(|p| format!("{p:04x}")).collect::<Vec<_>>().join(", ")
            ))
        })?;
    let entry = depository
        .depot(&def.depot)
        .ok_or_else(|| Error::other(format!("depot '{}' is not in the depository", def.depot)))?;

    log::info!("fetching depot {} ({} bytes) for {}", def.depot, entry.size, def.display_name);
    let files = fetch_and_unpack(entry)?;

    // The thumbnail lives in core_assets; fetch that depot's file if we can.
    let thumb = fetch_thumbnail(&depository, &def.thumbnail);
    import_device_files(&files, def, thumb.as_deref())
}

/// `pipeline://core_assets/thumbnails/x.png` → the bytes, via the core_assets depot.
fn fetch_thumbnail(depository: &Depository, uri: &str) -> Option<Vec<u8>> {
    let rel = uri.strip_prefix("pipeline://core_assets/")?;
    let entry = depository.depot("core_assets")?;
    if entry.is_encrypted() {
        return None;
    }
    // core_assets is ~21 MB; cache it so repeated fetches are free.
    let cache_path = cache_dir().join("core_assets.depot");
    let blob = match std::fs::read(&cache_path) {
        Ok(b) => b,
        Err(_) => {
            let b = ureq::get(&entry.url)
                .timeout(std::time::Duration::from_secs(180))
                .call()
                .ok()?
                .into_reader();
            let mut v = Vec::new();
            std::io::Read::read_to_end(&mut { b }, &mut v).ok()?;
            if !entry.mac.is_empty() && !sha256_hex(&v).eq_ignore_ascii_case(&entry.mac) {
                return None;
            }
            let _ = std::fs::write(&cache_path, &v);
            v
        }
    };
    unpack(&blob).ok()?.into_iter().find(|(n, _)| n == rel).map(|(_, d)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depot_round_trips() {
        let files = vec![
            ("front.png".to_string(), vec![0x89, b'P', b'N', b'G', 1, 2, 3]),
            ("metadata.json".to_string(), b"{}".to_vec()),
            ("empty".to_string(), Vec::new()),
        ];
        let blob = pack(&files);
        assert_eq!(&blob[..4], &DEPOT_MAGIC);
        assert_eq!(unpack(&blob).unwrap(), files);
    }

    #[test]
    fn unpack_rejects_bad_input() {
        assert!(unpack(b"not a depot").is_err());
        let mut blob = pack(&[("a".into(), vec![1, 2, 3])]);
        blob.push(0xff); // trailing garbage
        assert!(unpack(&blob).is_err());
        blob.truncate(blob.len() - 3); // truncated inside a file
        assert!(unpack(&blob).is_err());
    }

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Multi-block input.
        let long = vec![b'a'; 1000];
        assert_eq!(
            sha256_hex(&long),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    #[test]
    fn device_definitions_expose_pids_and_zone_map() {
        let json = br#"{"devices":[{"modelId":"g502_wireless","displayName":"G502 LIGHTSPEED",
          "depot":"g502_wireless","slotPrefix":"g502wireless","type":"MOUSE",
          "thumbnail":"pipeline://core_assets/thumbnails/g502_wireless.png",
          "modes":[{"interfaces":[{"type":"DEVIO","id":"046d_c08d"}]},
                   {"interfaces":[{"type":"DEVIO","id":"046d_407f"}]},
                   {"interfaces":[{"type":"DEVIO","id":"046d_aaef"}]}],
          "capabilities":{"lightingSupport":{"typeMap":{"ZONE_PRIMARY":"PRIMARY","ZONE_BRANDING":"LOGO"}}}}]}"#;
        let defs = parse_device_file(json);
        assert_eq!(defs.len(), 1);
        let g = &defs[0];
        assert_eq!(g.product_ids, vec![0xc08d, 0x407f, 0xaaef]);
        assert_eq!(g.zone_type_map["ZONE_BRANDING"], "LOGO");
        assert_eq!(g.depot, "g502_wireless");
    }

    #[test]
    fn encrypted_device_files_are_skipped_not_crashed() {
        let mut bytes = ENCRYPTED_MAGIC.to_vec();
        bytes.extend_from_slice(&[0x82, 0x00, 0xde, 0xad]);
        assert!(parse_device_file(&bytes).is_empty());
    }

    #[test]
    fn metadata_is_normalised_and_mapped_to_hidpp_names() {
        // Trimmed from a real G502 LIGHTSPEED depot.
        let json = br#"{"images":[{"key":"device_image","origin":{"width":1391,"height":2700},
          "assignments":[{"slotId":"g502wireless_g7_m1","marker":{"x":295,"y":989},"label":{"x":-1400,"y":1140}},
                         {"slotId":"g502wireless_g2_m1","marker":{"x":1110,"y":614},"label":{"x":2840,"y":214}},
                         {"slotId":"g502wireless_g3_m1","marker":{"x":800,"y":869},"label":{"x":800,"y":-300}}],
          "zones":[{"id":"ZONE_BRANDING","components":[{"renderings":[{"type":"rect","rect":{"x":"405","y":"1785","width":"300","height":"300"}}]}]},
                   {"id":"ZONE_PRIMARY","components":[{"renderings":[{"type":"rect","rect":{"x":"190","y":"1346","width":"140","height":"300"}}]}]}]},
          {"key":"device_side","origin":{"width":936,"height":2700},
          "assignments":[{"slotId":"g502wireless_g4_m1","marker":{"x":580,"y":1800},"label":{"x":-1200,"y":1800}}]}]}"#;
        let mut map = HashMap::new();
        map.insert("ZONE_PRIMARY".into(), "PRIMARY".into());
        map.insert("ZONE_BRANDING".into(), "LOGO".into());

        let layout = parse_metadata(json, "g502_wireless", "G502 LIGHTSPEED", &map).unwrap();
        assert_eq!(layout.views.len(), 2);

        let front = &layout.views[0];
        assert_eq!(front.view, "front");
        // Zone names line up with what the device reports over HID++.
        let logo = front.zones.iter().find(|z| z.id == "ZONE_BRANDING").unwrap();
        assert_eq!(logo.location_name, "Logo");
        assert!((logo.x - 405.0 / 1391.0).abs() < 1e-5);
        assert!((logo.height - 300.0 / 2700.0).abs() < 1e-5);
        let primary = front.zones.iter().find(|z| z.id == "ZONE_PRIMARY").unwrap();
        assert_eq!(primary.location_name, "Primary");
        // The primary zone is on the left flank, well left of centre.
        assert!(primary.x < 0.2);

        // Controls: slot → OpenGHub id, label side from where the label falls.
        let g7 = front.controls.iter().find(|c| c.slot_id == "g502wireless_g7_m1").unwrap();
        assert_eq!(g7.control, "button-7");
        assert_eq!(g7.side, "left");
        let g2 = front.controls.iter().find(|c| c.control == "button-2").unwrap();
        assert_eq!(g2.side, "right");
        let g3 = front.controls.iter().find(|c| c.control == "button-3").unwrap();
        assert_eq!(g3.side, "top");

        assert_eq!(layout.views[1].view, "side");
        assert_eq!(layout.views[1].controls[0].control, "button-4");
    }

    #[test]
    fn slot_ids_map_to_control_ids() {
        assert_eq!(control_id_for_slot("g502wireless_g11_m1"), "button-11");
        assert_eq!(control_id_for_slot("g502wireless_g9_m1_shifted"), "button-9");
        assert_eq!(control_id_for_slot("g502wireless_mouse_settings"), "g502wireless_mouse_settings");
    }

    #[test]
    fn depository_tolerates_string_numbers() {
        let json = r#"{"version":"2026.5","buildId":824196,"branch":"staging",
          "depots":[{"name":"g502_wireless","url":"https://x/y.depot","mac":"ab","size":"15035151","cipherSuite":""},
                    {"name":"core","url":"https://x/c.depot","mac":"cd","size":5,"cipherSuite":"AES-256-CBC"}]}"#;
        let d: Depository = serde_json::from_str(json).unwrap();
        assert_eq!(d.build_id, "824196");
        assert_eq!(d.depot("g502_wireless").unwrap().size, 15035151);
        assert!(!d.depot("g502_wireless").unwrap().is_encrypted());
        assert!(d.depot("core").unwrap().is_encrypted());
    }
}
