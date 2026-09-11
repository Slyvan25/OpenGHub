//! Onboard profile memory (HID++ feature `0x8100`).
//!
//! Devices like the G502 LIGHTSPEED do not expose `0x1b04`; their button
//! bindings, DPI ladder and macros live in the device's own flash, organised as
//! fixed-size sectors. A profile directory in sector 0 points at one sector per
//! profile.
//!
//! **Every sector ends with a CRC-16/CCITT** (polynomial `0x1021`, init
//! `0xFFFF`) over all preceding bytes, stored big-endian. A sector written with
//! a wrong CRC is rejected or leaves the profile corrupt, so [`crc16`] is
//! verified against sectors the device wrote itself before anything is written
//! back — see `cargo run --example onboard`.
//!
//! Reads are always 16 bytes and must not cross the end of a sector. The G502's
//! sectors are 255 bytes, *not* 256, so the final read of a sector is aligned to
//! its end and overlaps the previous one.

use serde::{Deserialize, Serialize};

use super::{Error, Handle, ReportKind, Result};

pub const ID: u16 = 0x8100;

pub const FN_GET_INFO: u8 = 0x00;
pub const FN_SET_MODE: u8 = 0x01;
pub const FN_GET_MODE: u8 = 0x02;
pub const FN_SET_CURRENT_PROFILE: u8 = 0x03;
pub const FN_GET_CURRENT_PROFILE: u8 = 0x04;
pub const FN_MEMORY_READ: u8 = 0x05;
pub const FN_MEMORY_ADDR_WRITE: u8 = 0x06;
pub const FN_MEMORY_WRITE: u8 = 0x07;
pub const FN_MEMORY_WRITE_END: u8 = 0x08;

/// The device runs its own stored profile.
pub const MODE_ONBOARD: u8 = 0x01;
/// Software drives the device.
pub const MODE_HOST: u8 = 0x02;

/// Button descriptor types, as found in profile format 3.
pub const BUTTON_MACRO: u8 = 0x00;
pub const BUTTON_MOUSE: u8 = 0x80;
pub const BUTTON_SPECIAL: u8 = 0x90;
/// An unprogrammed button reads as all `0xff`.
pub const BUTTON_DISABLED: u8 = 0xff;

/// Capabilities from `getOnboardProfilesInfo`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardInfo {
    pub memory_model: u8,
    pub profile_format: u8,
    pub macro_format: u8,
    pub profile_count: u8,
    pub profile_count_oob: u8,
    pub button_count: u8,
    pub sector_count: u8,
    pub sector_size: u16,
    pub mechanical_layout: u8,
    pub various_info: u8,
}

pub fn read_info(h: &mut Handle) -> Result<OnboardInfo> {
    let idx = h.feature_index(ID)?;
    let p = h.call(idx, FN_GET_INFO, &[], ReportKind::Long)?;
    Ok(OnboardInfo {
        memory_model: p.param(0),
        profile_format: p.param(1),
        macro_format: p.param(2),
        profile_count: p.param(3),
        profile_count_oob: p.param(4),
        button_count: p.param(5),
        sector_count: p.param(6),
        sector_size: p.param_u16(7),
        mechanical_layout: p.param(9),
        various_info: p.param(10),
    })
}

/// CRC-16/CCITT (poly `0x1021`, init `0xFFFF`) — the checksum the device stores
/// in the last two bytes of every sector.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

/// The 16-byte-aligned read offsets that exactly cover a sector.
///
/// A read may not run past the sector end, so for a 255-byte sector the last
/// read starts at 239 and overlaps the previous one by a byte.
pub fn read_offsets(sector_size: usize) -> Vec<usize> {
    if sector_size < 16 {
        return Vec::new();
    }
    let mut offsets: Vec<usize> = (0..=sector_size - 16).step_by(16).collect();
    if offsets.last().map(|o| o + 16) != Some(sector_size) {
        offsets.push(sector_size - 16);
    }
    offsets
}

/// Reads one whole sector, verifying its checksum.
///
/// An erased sector (all `0xff`) has no valid CRC; `verify` lets the caller
/// accept that when taking a backup.
pub fn read_sector(h: &mut Handle, sector: u16, size: usize, verify: bool) -> Result<Vec<u8>> {
    let idx = h.feature_index(ID)?;
    let mut buf = vec![0u8; size];

    for offset in read_offsets(size) {
        let [sh, sl] = sector.to_be_bytes();
        let [oh, ol] = (offset as u16).to_be_bytes();
        let reply = h.call(idx, FN_MEMORY_READ, &[sh, sl, oh, ol], ReportKind::Long)?;
        buf[offset..offset + 16].copy_from_slice(&reply.params[..16]);
    }

    if verify && !is_erased(&buf) && !checksum_ok(&buf) {
        return Err(Error::other(format!(
            "sector {sector} failed its checksum — refusing to trust it"
        )));
    }
    Ok(buf)
}

/// True when every byte is `0xff`, i.e. the sector has never been written.
pub fn is_erased(sector: &[u8]) -> bool {
    sector.iter().all(|b| *b == 0xff)
}

/// Checks the stored CRC in the last two bytes against the rest of the sector.
pub fn checksum_ok(sector: &[u8]) -> bool {
    if sector.len() < 3 {
        return false;
    }
    let split = sector.len() - 2;
    let stored = u16::from_be_bytes([sector[split], sector[split + 1]]);
    crc16(&sector[..split]) == stored
}

/// Writes the correct CRC into the last two bytes, in place.
pub fn seal(sector: &mut [u8]) {
    let split = sector.len() - 2;
    let crc = crc16(&sector[..split]);
    sector[split..].copy_from_slice(&crc.to_be_bytes());
}

/// One entry of the profile directory in sector 0.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileEntry {
    pub sector: u16,
    pub enabled: bool,
}

/// Parses the profile directory. The list ends at an `0xffff` sector id.
pub fn parse_directory(sector0: &[u8]) -> Vec<ProfileEntry> {
    let mut out = Vec::new();
    for chunk in sector0.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let sector = u16::from_be_bytes([chunk[0], chunk[1]]);
        if sector == 0xffff || sector == 0x0000 {
            break;
        }
        out.push(ProfileEntry { sector, enabled: chunk[2] != 0 });
    }
    out
}

/// A button binding, decoded from its 4-byte descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Button {
    /// Standard mouse button; `mask` has one bit set per physical button.
    Mouse { mask: u16 },
    /// Built-in action such as DPI up/down or profile cycle.
    Special { action: u8 },
    /// Runs a macro stored at `sector`/`offset`.
    Macro { sector: u8, offset: u16 },
    Disabled,
    /// Anything we do not recognise, kept verbatim so a round trip is lossless.
    Raw { bytes: [u8; 4] },
}

impl Button {
    pub fn decode(b: [u8; 4]) -> Self {
        match b[0] {
            BUTTON_MOUSE => Button::Mouse { mask: u16::from_be_bytes([b[2], b[3]]) },
            BUTTON_SPECIAL => Button::Special { action: b[1] },
            BUTTON_MACRO => Button::Macro { sector: b[1], offset: u16::from_be_bytes([b[2], b[3]]) },
            BUTTON_DISABLED => Button::Disabled,
            _ => Button::Raw { bytes: b },
        }
    }

    pub fn encode(self) -> [u8; 4] {
        match self {
            Button::Mouse { mask } => {
                let [h, l] = mask.to_be_bytes();
                [BUTTON_MOUSE, 0x01, h, l]
            }
            Button::Special { action } => [BUTTON_SPECIAL, action, 0xff, 0x00],
            Button::Macro { sector, offset } => {
                let [h, l] = offset.to_be_bytes();
                [BUTTON_MACRO, sector, h, l]
            }
            Button::Disabled => [0xff; 4],
            Button::Raw { bytes } => bytes,
        }
    }
}

/// Offset of the button descriptor table within a profile sector.
pub const BUTTONS_OFFSET: usize = 32;

/// A decoded profile. Only the fields OpenGHub understands are broken out; the
/// raw sector is kept so a write can preserve everything else untouched.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub sector: u16,
    /// Report rate in milliseconds, as stored.
    pub report_rate_ms: u8,
    pub default_dpi_index: u8,
    pub switched_dpi_index: u8,
    /// The five DPI steps. Stored little-endian, unlike HID++ messages.
    pub dpi: [u16; 5],
    pub buttons: Vec<Button>,
    #[serde(skip)]
    pub raw: Vec<u8>,
}

pub fn parse_profile(sector: u16, raw: &[u8], button_count: u8) -> Result<Profile> {
    if raw.len() < BUTTONS_OFFSET + button_count as usize * 4 {
        return Err(Error::Malformed);
    }
    let dpi_at = |i: usize| u16::from_le_bytes([raw[3 + i * 2], raw[4 + i * 2]]);

    let buttons = (0..button_count as usize)
        .map(|i| {
            let at = BUTTONS_OFFSET + i * 4;
            Button::decode([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]])
        })
        .collect();

    Ok(Profile {
        sector,
        report_rate_ms: raw[0],
        default_dpi_index: raw[1],
        switched_dpi_index: raw[2],
        dpi: [dpi_at(0), dpi_at(1), dpi_at(2), dpi_at(3), dpi_at(4)],
        buttons,
        raw: raw.to_vec(),
    })
}

/// Writes one sector, sealing it with the correct checksum first.
///
/// The sequence is `memoryAddrWrite` (declare sector, offset and length), then
/// `memoryWrite` in 16-byte chunks, then `memoryWriteEnd` to commit. The device
/// only commits on the final call, so an interrupted write leaves the old
/// contents intact.
///
/// `data` must be exactly one sector; the last two bytes are overwritten with
/// the CRC, so callers never compute it themselves.
pub fn write_sector(h: &mut Handle, sector: u16, data: &[u8]) -> Result<()> {
    let idx = h.feature_index(ID)?;
    let info = read_info(h)?;
    let size = info.sector_size as usize;

    if data.len() != size {
        return Err(Error::other(format!(
            "sector write must be exactly {size} bytes, got {}",
            data.len()
        )));
    }
    if sector >= info.sector_count as u16 {
        return Err(Error::other(format!("sector {sector} is out of range")));
    }

    let mut payload = data.to_vec();
    seal(&mut payload);

    // The declared length must be exactly the sector size. Rounding it up to a
    // whole number of 16-byte writes (256 for a 255-byte sector) is rejected as
    // "invalid argument"; the final write simply carries a partial chunk.
    let declared = payload.len();
    let [sh, sl] = sector.to_be_bytes();
    let [oh, ol] = 0u16.to_be_bytes();
    let [lh, ll] = (declared as u16).to_be_bytes();
    h.call(idx, FN_MEMORY_ADDR_WRITE, &[sh, sl, oh, ol, lh, ll], ReportKind::Long)?;

    let mut chunk = [0u8; 16];
    for offset in (0..declared).step_by(16) {
        chunk.fill(0xff);
        let end = (offset + 16).min(payload.len());
        chunk[..end - offset].copy_from_slice(&payload[offset..end]);
        h.call(idx, FN_MEMORY_WRITE, &chunk, ReportKind::Long)?;
    }

    h.call(idx, FN_MEMORY_WRITE_END, &[], ReportKind::Short)?;
    Ok(())
}

/// A full copy of every sector, for backup before any write.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBackup {
    pub device: String,
    pub product_id: u16,
    pub info: OnboardInfo,
    /// Sector index → bytes, hex-encoded so the file is diffable.
    pub sectors: Vec<String>,
}

/// Reads every sector. Used before writing anything, so a mistake is always
/// recoverable.
pub fn backup(h: &mut Handle, device: &str, product_id: u16) -> Result<MemoryBackup> {
    let info = read_info(h)?;
    let size = info.sector_size as usize;

    let mut sectors = Vec::with_capacity(info.sector_count as usize);
    for sector in 0..info.sector_count as u16 {
        // Do not verify: erased and vendor sectors legitimately fail the CRC.
        let bytes = read_sector(h, sector, size, false)?;
        sectors.push(bytes.iter().map(|b| format!("{b:02x}")).collect());
    }

    Ok(MemoryBackup {
        device: device.to_string(),
        product_id,
        info,
        sectors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_what_the_device_stores() {
        // Sector 0 of a real G502 LIGHTSPEED: directory, then erased, CRC 0x1b6f.
        let mut sector = vec![0xffu8; 255];
        sector[..24].copy_from_slice(&[
            0x00, 0x01, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04,
            0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00,
        ]);
        sector[253] = 0x1b;
        sector[254] = 0x6f;
        assert!(checksum_ok(&sector), "CRC-16/CCITT init 0xFFFF must reproduce the stored value");

        // And sealing an unchanged sector must be a no-op.
        let original = sector.clone();
        seal(&mut sector);
        assert_eq!(sector, original);
    }

    #[test]
    fn sealing_makes_a_sector_self_consistent() {
        let mut sector = vec![0u8; 255];
        sector[..4].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        assert!(!checksum_ok(&sector));
        seal(&mut sector);
        assert!(checksum_ok(&sector), "a sealed sector must verify");
        // Sealing twice is stable.
        let once = sector.clone();
        seal(&mut sector);
        assert_eq!(sector, once);
    }

    #[test]
    fn read_offsets_never_cross_the_sector_end() {
        // 255 is the awkward real-world case: not a multiple of 16.
        let offsets = read_offsets(255);
        assert_eq!(offsets.first(), Some(&0));
        assert_eq!(offsets.last(), Some(&239), "final read must be end-aligned");
        assert!(offsets.iter().all(|o| o + 16 <= 255));
        // Every byte is covered.
        let mut covered = vec![false; 255];
        for o in offsets {
            for i in o..o + 16 {
                covered[i] = true;
            }
        }
        assert!(covered.into_iter().all(|c| c));

        // A round sector needs no overlapping tail read.
        assert_eq!(read_offsets(256).last(), Some(&240));
    }

    #[test]
    fn directory_parses_and_stops_at_the_terminator() {
        let sector = [
            0x00, 0x01, 0x01, 0x00, // profile 1, sector 1, enabled
            0x00, 0x02, 0x00, 0x00, // profile 2, sector 2, disabled
            0xff, 0xff, 0x00, 0x00, // terminator
            0x00, 0x09, 0x01, 0x00, // must not be read
        ];
        let entries = parse_directory(&sector);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sector, 1);
        assert!(entries[0].enabled);
        assert!(!entries[1].enabled);
    }

    #[test]
    fn button_descriptors_round_trip() {
        // Real descriptors read off a G502.
        assert_eq!(Button::decode([0x80, 0x01, 0x00, 0x01]), Button::Mouse { mask: 1 });
        assert_eq!(Button::decode([0x80, 0x01, 0x00, 0x10]), Button::Mouse { mask: 16 });
        assert_eq!(Button::decode([0x90, 0x07, 0xff, 0x00]), Button::Special { action: 7 });
        assert_eq!(Button::decode([0xff; 4]), Button::Disabled);

        for raw in [[0x80, 0x01, 0x00, 0x08], [0x90, 0x04, 0xff, 0x00], [0xff; 4]] {
            assert_eq!(Button::decode(raw).encode(), raw, "round trip must be lossless");
        }

        let m = Button::Macro { sector: 2, offset: 0x0040 };
        assert_eq!(m.encode(), [0x00, 0x02, 0x00, 0x40]);
        assert_eq!(Button::decode(m.encode()), m);
    }

    #[test]
    fn profile_parses_the_g502_dpi_ladder() {
        // DPI values are little-endian in profile memory, unlike HID++ messages.
        let mut raw = vec![0xffu8; 255];
        raw[..13].copy_from_slice(&[
            0x01, 0x01, 0x00, 0x90, 0x01, 0x20, 0x03, 0x40, 0x06, 0x80, 0x0c, 0x00, 0x19,
        ]);
        raw[32..48].copy_from_slice(&[
            0x80, 0x01, 0x00, 0x01, 0x80, 0x01, 0x00, 0x02, 0x80, 0x01, 0x00, 0x04, 0x80, 0x01,
            0x00, 0x08,
        ]);
        let profile = parse_profile(1, &raw, 4).unwrap();
        assert_eq!(profile.report_rate_ms, 1);
        assert_eq!(profile.dpi, [400, 800, 1600, 3200, 6400]);
        assert_eq!(profile.buttons[0], Button::Mouse { mask: 1 });
        assert_eq!(profile.buttons[3], Button::Mouse { mask: 8 });
    }
}
