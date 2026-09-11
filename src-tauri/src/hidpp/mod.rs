//! HID++ 2.0 transport, packet framing and feature discovery.
//!
//! Logitech peripherals speak a vendor protocol ("HID++") over a dedicated HID
//! interface (usage page `0xFF00`). Two report sizes exist:
//!
//! ```text
//!  short (report id 0x10, 7 bytes):  [0x10, device_index, feature_index, fn|sw_id, p0, p1, p2]
//!  long  (report id 0x11, 20 bytes): [0x11, device_index, feature_index, fn|sw_id, p0 .. p15]
//! ```
//!
//! `feature_index` is *not* the well known feature id (e.g. `0x2201`); it is a
//! per-device index that must be resolved at runtime through the Root feature
//! (`0x0000`), which is guaranteed to live at index `0x00`. See [`Handle::feature_index`].

pub mod features;
pub mod onboard;
pub mod registry;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice, HidError};
use serde::{Deserialize, Serialize};

pub const LOGITECH_VID: u16 = 0x046d;

/// Report id / total length of a short HID++ report.
pub const REPORT_ID_SHORT: u8 = 0x10;
pub const SHORT_LEN: usize = 7;
/// Report id / total length of a long HID++ report.
pub const REPORT_ID_LONG: u8 = 0x11;
pub const LONG_LEN: usize = 20;
/// Report id / total length of a very long HID++ report (rarely used, some 0x8071 devices).
pub const REPORT_ID_VERY_LONG: u8 = 0x12;
pub const VERY_LONG_LEN: usize = 64;

/// Device index used for devices we talk to directly over USB/Bluetooth.
pub const DEVICE_INDEX_WIRED: u8 = 0xff;
/// Device index of the receiver itself when going through a Unifying/Lightspeed dongle.
pub const DEVICE_INDEX_RECEIVER: u8 = 0x00;
/// Paired-device indices behind a receiver.
pub const RECEIVER_CHILD_INDICES: [u8; 6] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

/// Software id tag placed in the low nibble of byte 3. The device echoes it back,
/// which lets us tell our own replies apart from unsolicited event notifications
/// (which always carry software id `0`).
pub const SOFTWARE_ID: u8 = 0x0a;

/// Feature index reported by the device when a request failed.
const ERROR_FEATURE_INDEX: u8 = 0xff;
/// HID++ 1.0 style error sub-id.
const ERROR_SUB_ID_V1: u8 = 0x8f;

const IO_TIMEOUT: Duration = Duration::from_millis(1000);
/// How long we keep draining unrelated reports while waiting for our reply.
const EXCHANGE_DEADLINE: Duration = Duration::from_millis(1500);

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("hid error: {0}")]
    Hid(#[from] HidError),
    #[error("device is not connected")]
    NotConnected,
    #[error("timed out waiting for a reply from the device")]
    Timeout,
    #[error("device does not support feature {0:#06x}")]
    UnsupportedFeature(u16),
    #[error("device reported error: {0}")]
    Protocol(ProtocolError),
    #[error("malformed reply from device")]
    Malformed,
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}

/// Serialized as a plain string so the Svelte side can render it directly.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// HID++ 2.0 error codes (byte 5 of an error reply).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolError(pub u8);

impl ProtocolError {
    /// Returned when a setting is refused, including when the device's onboard
    /// profile owns it and software is not allowed to change it.
    pub const INVALID_ARGUMENT: u8 = 0x02;

    pub const fn is_invalid_argument(self) -> bool {
        self.0 == Self::INVALID_ARGUMENT
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.0 {
            0x00 => "no error",
            0x01 => "unknown",
            0x02 => "invalid argument",
            0x03 => "out of range",
            0x04 => "hardware error",
            0x05 => "logitech internal",
            0x06 => "invalid feature index",
            0x07 => "invalid function id",
            0x08 => "busy",
            0x09 => "unsupported",
            _ => "unspecified",
        };
        write!(f, "{name} ({:#04x})", self.0)
    }
}

// ---------------------------------------------------------------------------
// Packets
// ---------------------------------------------------------------------------

/// Which report size a message uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportKind {
    Short,
    Long,
    VeryLong,
}

impl ReportKind {
    pub const fn report_id(self) -> u8 {
        match self {
            ReportKind::Short => REPORT_ID_SHORT,
            ReportKind::Long => REPORT_ID_LONG,
            ReportKind::VeryLong => REPORT_ID_VERY_LONG,
        }
    }

    pub const fn len(self) -> usize {
        match self {
            ReportKind::Short => SHORT_LEN,
            ReportKind::Long => LONG_LEN,
            ReportKind::VeryLong => VERY_LONG_LEN,
        }
    }

    pub const fn from_report_id(id: u8) -> Option<Self> {
        match id {
            REPORT_ID_SHORT => Some(ReportKind::Short),
            REPORT_ID_LONG => Some(ReportKind::Long),
            REPORT_ID_VERY_LONG => Some(ReportKind::VeryLong),
            _ => None,
        }
    }

    /// Number of parameter bytes carried after the 4 byte header.
    pub const fn params_len(self) -> usize {
        self.len() - 4
    }
}

/// A single HID++ message, in either direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub kind: ReportKind,
    pub device_index: u8,
    pub feature_index: u8,
    /// High nibble = function id, low nibble = software id.
    pub func_and_sw: u8,
    pub params: Vec<u8>,
}

impl Packet {
    /// Builds a request. `params` is zero-padded / truncated to the report size.
    pub fn request(
        kind: ReportKind,
        device_index: u8,
        feature_index: u8,
        function_id: u8,
        params: &[u8],
    ) -> Self {
        let mut buf = vec![0u8; kind.params_len()];
        let n = params.len().min(buf.len());
        buf[..n].copy_from_slice(&params[..n]);
        Packet {
            kind,
            device_index,
            feature_index,
            func_and_sw: (function_id << 4) | (SOFTWARE_ID & 0x0f),
            params: buf,
        }
    }

    pub fn parse(raw: &[u8]) -> Option<Self> {
        if raw.len() < 4 {
            return None;
        }
        let kind = ReportKind::from_report_id(raw[0])?;
        // Some kernels hand back fewer bytes than the nominal report size.
        let end = raw.len().min(kind.len());
        Some(Packet {
            kind,
            device_index: raw[1],
            feature_index: raw[2],
            func_and_sw: raw[3],
            params: raw[4..end].to_vec(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.kind.len()];
        out[0] = self.kind.report_id();
        out[1] = self.device_index;
        out[2] = self.feature_index;
        out[3] = self.func_and_sw;
        let n = self.params.len().min(out.len() - 4);
        out[4..4 + n].copy_from_slice(&self.params[..n]);
        out
    }

    pub const fn function_id(&self) -> u8 {
        self.func_and_sw >> 4
    }

    pub const fn software_id(&self) -> u8 {
        self.func_and_sw & 0x0f
    }

    /// An unsolicited event (button press, battery change, …) carries software id 0.
    pub const fn is_notification(&self) -> bool {
        self.software_id() == 0
    }

    /// Error replies shift the whole message one slot left:
    ///
    /// ```text
    /// [0x10, device, 0xff, errored_feature_index, errored_fn|sw, code, 0x00]
    /// ```
    ///
    /// so the feature index we sent lands in `func_and_sw` and the code in
    /// `params[1]`. HID++ 1.0 errors use the same shape under sub-id `0x8f`.
    pub fn as_error(&self) -> Option<ProtocolError> {
        if self.is_error_reply() {
            return Some(ProtocolError(self.param(1)));
        }
        None
    }

    pub const fn is_error_reply(&self) -> bool {
        self.feature_index == ERROR_FEATURE_INDEX || self.feature_index == ERROR_SUB_ID_V1
    }

    /// For an error reply, the feature index of the request that failed.
    pub const fn errored_feature_index(&self) -> u8 {
        self.func_and_sw
    }

    pub fn param(&self, i: usize) -> u8 {
        self.params.get(i).copied().unwrap_or(0)
    }

    /// Big-endian u16 read from two consecutive parameter bytes.
    pub fn param_u16(&self, i: usize) -> u16 {
        u16::from_be_bytes([self.param(i), self.param(i + 1)])
    }
}

// ---------------------------------------------------------------------------
// Device identity
// ---------------------------------------------------------------------------

/// Everything needed to reopen a device later, plus how to address it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceAddress {
    /// hidraw path, e.g. `/dev/hidraw3`. Unique per HID interface.
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// `0xff` when talking straight to the device, `1..=6` behind a receiver.
    pub device_index: u8,
}

impl DeviceAddress {
    /// Stable identifier handed to the frontend. Paths can change across replug,
    /// so we key on PID + index and keep the path as a reopen hint.
    pub fn id(&self) -> String {
        format!("{:04x}:{:04x}:{}", self.vendor_id, self.product_id, self.device_index)
    }
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

/// An opened HID++ endpoint with a cached feature-index table.
pub struct Handle {
    device: HidDevice,
    pub address: DeviceAddress,
    /// feature id -> (index, version)
    feature_cache: HashMap<u16, (u8, u8)>,
    /// Features the device has already told us it does not have. Without this,
    /// every battery poll would re-ask for `0x1004` on a device that only has
    /// `0x1000`, costing a round trip each time.
    unsupported: HashSet<u16>,
    /// Largest report the device accepted so far; long is the safe default.
    pub protocol_version: (u8, u8),
}

impl Handle {
    /// Opens the HID++ endpoint described by `address`.
    pub fn open(api: &HidApi, address: DeviceAddress) -> Result<Self> {
        let cpath = std::ffi::CString::new(address.path.as_str())
            .map_err(|_| Error::other("hid path contained a NUL byte"))?;
        let device = api.open_path(&cpath)?;
        device.set_blocking_mode(false)?;
        let mut handle = Handle {
            device,
            address,
            feature_cache: HashMap::new(),
            unsupported: HashSet::new(),
            protocol_version: (0, 0),
        };
        handle.protocol_version = handle.ping().unwrap_or((0, 0));
        Ok(handle)
    }

    /// Root feature function 1: returns the HID++ protocol version and proves the
    /// device index is actually alive (a sleeping wireless device will time out).
    pub fn ping(&mut self) -> Result<(u8, u8)> {
        const MARKER: u8 = 0x5a;
        let reply = self.exchange(Packet::request(
            ReportKind::Short,
            self.address.device_index,
            features::root::INDEX,
            features::root::FN_GET_PROTOCOL_VERSION,
            &[0x00, 0x00, MARKER],
        ))?;
        if reply.param(2) != MARKER {
            return Err(Error::Malformed);
        }
        Ok((reply.param(0), reply.param(1)))
    }

    /// Resolves a feature id to its runtime index, caching the result.
    ///
    /// Returns [`Error::UnsupportedFeature`] when the device answers with index 0,
    /// which is the protocol's way of saying "I don't have that feature".
    pub fn feature_index(&mut self, feature_id: u16) -> Result<u8> {
        if let Some((idx, _)) = self.feature_cache.get(&feature_id) {
            return Ok(*idx);
        }
        if self.unsupported.contains(&feature_id) {
            return Err(Error::UnsupportedFeature(feature_id));
        }
        let [hi, lo] = feature_id.to_be_bytes();
        let reply = self.exchange(Packet::request(
            ReportKind::Short,
            self.address.device_index,
            features::root::INDEX,
            features::root::FN_GET_FEATURE,
            &[hi, lo],
        ))?;
        let index = reply.param(0);
        let version = reply.param(2);
        if index == 0 {
            self.unsupported.insert(feature_id);
            return Err(Error::UnsupportedFeature(feature_id));
        }
        self.feature_cache.insert(feature_id, (index, version));
        Ok(index)
    }

    pub fn feature_version(&mut self, feature_id: u16) -> Result<u8> {
        self.feature_index(feature_id)?;
        Ok(self.feature_cache.get(&feature_id).map(|(_, v)| *v).unwrap_or(0))
    }

    pub fn supports(&mut self, feature_id: u16) -> bool {
        self.feature_index(feature_id).is_ok()
    }

    /// Enumerates every feature the device exposes via the FeatureSet feature (`0x0001`).
    /// Primarily a debugging aid — normal calls go through [`Handle::feature_index`].
    pub fn enumerate_features(&mut self) -> Result<Vec<(u16, u8, u8)>> {
        let idx = self.feature_index(features::feature_set::ID)?;
        let count = self
            .call(idx, features::feature_set::FN_GET_COUNT, &[], ReportKind::Short)?
            .param(0);

        let mut out = Vec::with_capacity(count as usize + 1);
        out.push((features::root::ID, 0u8, 0u8));
        for i in 1..=count {
            let reply =
                self.call(idx, features::feature_set::FN_GET_FEATURE_ID, &[i], ReportKind::Short)?;
            let id = reply.param_u16(0);
            let ftype = reply.param(2);
            let version = reply.param(3);
            self.unsupported.remove(&id);
            self.feature_cache.entry(id).or_insert((i, version));
            out.push((id, i, ftype));
        }
        Ok(out)
    }

    /// Calls `function_id` on an already-resolved feature index.
    pub fn call(
        &mut self,
        feature_index: u8,
        function_id: u8,
        params: &[u8],
        kind: ReportKind,
    ) -> Result<Packet> {
        self.exchange(Packet::request(
            kind,
            self.address.device_index,
            feature_index,
            function_id,
            params,
        ))
    }

    /// Resolves the feature id then calls it. The common path.
    pub fn call_feature(
        &mut self,
        feature_id: u16,
        function_id: u8,
        params: &[u8],
        kind: ReportKind,
    ) -> Result<Packet> {
        let index = self.feature_index(feature_id)?;
        self.call(index, function_id, params, kind)
    }

    /// Writes a request and waits for the matching reply.
    ///
    /// Replies are correlated on `(device_index, feature_index, function|software id)`.
    /// Anything else — notifications, traffic for a sibling device on the same
    /// receiver — is discarded, so an idle mouse moving does not desync us.
    pub fn exchange(&mut self, request: Packet) -> Result<Packet> {
        let bytes = request.to_bytes();
        // Drop anything stale that arrived since the last call.
        self.drain();

        self.device.write(&bytes).map_err(|e| match e {
            HidError::HidApiError { message } if message.contains("No such device") => {
                Error::NotConnected
            }
            other => Error::Hid(other),
        })?;

        let deadline = Instant::now() + EXCHANGE_DEADLINE;
        let mut buf = [0u8; VERY_LONG_LEN];
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let timeout_ms = remaining.min(IO_TIMEOUT).as_millis() as i32;
            let n = self.device.read_timeout(&mut buf, timeout_ms.max(1))?;
            if n == 0 {
                continue;
            }
            let Some(reply) = Packet::parse(&buf[..n]) else {
                continue;
            };
            if reply.device_index != request.device_index {
                continue;
            }
            if let Some(err) = reply.as_error() {
                // Only claim the error if it refers to the request we just sent.
                if reply.errored_feature_index() == request.feature_index
                    || reply.feature_index == ERROR_SUB_ID_V1
                {
                    return Err(Error::Protocol(err));
                }
                continue;
            }
            if reply.feature_index == request.feature_index
                && reply.func_and_sw == request.func_and_sw
            {
                return Ok(reply);
            }
            // Otherwise: a notification or another device's traffic — keep waiting.
        }
        Err(Error::Timeout)
    }

    /// Non-blocking read of one pending notification, if any.
    pub fn poll_notification(&mut self) -> Result<Option<Packet>> {
        let mut buf = [0u8; VERY_LONG_LEN];
        let n = self.device.read_timeout(&mut buf, 0)?;
        if n == 0 {
            return Ok(None);
        }
        Ok(Packet::parse(&buf[..n]).filter(|p| p.is_notification()))
    }

    fn drain(&mut self) {
        let mut buf = [0u8; VERY_LONG_LEN];
        // Bounded so a chatty device can never trap us here.
        for _ in 0..32 {
            match self.device.read_timeout(&mut buf, 0) {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

/// A HID interface that looks like it speaks HID++.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub address: DeviceAddress,
    /// Product string reported by the HID descriptor, when present.
    pub hid_product: Option<String>,
    pub serial: Option<String>,
    /// True when this endpoint is a Unifying/Lightspeed receiver rather than a
    /// directly attached device.
    pub is_receiver: bool,
}

/// Finds every Logitech HID++ endpoint on the system.
///
/// Logitech devices expose several HID interfaces (mouse, keyboard, consumer
/// control, HID++). Only the vendor one — usage page `0xFF00` — accepts HID++
/// traffic, so we filter on that, falling back to interface number 1/2 on
/// backends that do not report usage pages.
pub fn enumerate(api: &HidApi) -> Vec<Endpoint> {
    let mut out = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() != LOGITECH_VID {
            continue;
        }
        let usage_page = info.usage_page();
        let iface = info.interface_number();
        let looks_vendor = usage_page == 0xff00 || (usage_page == 0 && (iface == 1 || iface == 2));
        if !looks_vendor {
            continue;
        }
        let Some(path) = info.path().to_str().ok().map(str::to_owned) else {
            continue;
        };
        // A given hidraw node may be listed once per usage; keep the first.
        if out.iter().any(|e: &Endpoint| e.address.path == path) {
            continue;
        }
        let pid = info.product_id();
        out.push(Endpoint {
            address: DeviceAddress {
                path,
                vendor_id: info.vendor_id(),
                product_id: pid,
                device_index: DEVICE_INDEX_WIRED,
            },
            hid_product: info.product_string().map(str::to_owned),
            serial: info.serial_number().map(str::to_owned),
            is_receiver: registry::is_receiver(pid),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_request_round_trips() {
        let p = Packet::request(ReportKind::Short, 0xff, 0x00, 0x00, &[0x22, 0x01]);
        let bytes = p.to_bytes();
        assert_eq!(bytes.len(), SHORT_LEN);
        assert_eq!(bytes[0], REPORT_ID_SHORT);
        assert_eq!(bytes[3], SOFTWARE_ID); // function 0, software id in low nibble
        assert_eq!(&bytes[4..], &[0x22, 0x01, 0x00]);
        assert_eq!(Packet::parse(&bytes).unwrap(), p);
    }

    #[test]
    fn long_request_is_padded() {
        let p = Packet::request(ReportKind::Long, 0x01, 0x0a, 0x03, &[0x00, 0x1f, 0x40]);
        let bytes = p.to_bytes();
        assert_eq!(bytes.len(), LONG_LEN);
        assert_eq!(bytes[3], (0x03 << 4) | SOFTWARE_ID);
        assert!(bytes[7..].iter().all(|b| *b == 0));
    }

    #[test]
    fn function_and_software_id_split() {
        let p = Packet::request(ReportKind::Short, 0xff, 0x05, 0x0b, &[]);
        assert_eq!(p.function_id(), 0x0b);
        assert_eq!(p.software_id(), SOFTWARE_ID);
        assert!(!p.is_notification());
    }

    #[test]
    fn detects_protocol_errors() {
        // [0x10, dev, 0xff, errored_feature_idx, errored_fn|sw, code, 0]
        let raw = [REPORT_ID_SHORT, 0xff, 0xff, 0x0a, 0x1a, 0x09, 0x00];
        let packet = Packet::parse(&raw).unwrap();
        let err = packet.as_error().unwrap();
        assert_eq!(err, ProtocolError(0x09));
        assert_eq!(err.to_string(), "unsupported (0x09)");
        assert_eq!(packet.errored_feature_index(), 0x0a);

        // A successful reply from the same feature is not mistaken for an error.
        let ok = Packet::parse(&[REPORT_ID_SHORT, 0xff, 0x0a, 0x1a, 0x00, 0x06, 0x40]).unwrap();
        assert!(ok.as_error().is_none());
    }

    #[test]
    fn notifications_have_software_id_zero() {
        let raw = [REPORT_ID_SHORT, 0x01, 0x06, 0x00, 0x50, 0x00, 0x00];
        let p = Packet::parse(&raw).unwrap();
        assert!(p.is_notification());
        assert!(p.as_error().is_none());
    }

    #[test]
    fn parses_truncated_reports() {
        let raw = [REPORT_ID_LONG, 0xff, 0x0a, 0x2a, 0x00, 0x06, 0x40];
        let p = Packet::parse(&raw).unwrap();
        assert_eq!(p.kind, ReportKind::Long);
        assert_eq!(p.param_u16(1), 1600);
    }
}
