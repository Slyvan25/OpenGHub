//! Display metadata for known Logitech product ids.
//!
//! This table decides which artwork and friendly name a card shows. Listed
//! devices use the short marketing name G HUB displays, in preference to the
//! longer string the hardware reports via feature `0x0005`. Anything not listed
//! still works: the name then comes from `0x0005` and the category from the
//! device-type byte, so an unknown PID degrades to a correct, generic card.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Mouse,
    Keyboard,
    Headset,
    Speaker,
    Microphone,
    Light,
    Webcam,
    Wheel,
    Receiver,
    Other,
}

impl DeviceKind {
    /// Maps the device-type byte returned by feature `0x0005` function 2.
    pub fn from_hidpp_type(byte: u8) -> Self {
        match byte {
            0x00 => DeviceKind::Keyboard,
            0x01 => DeviceKind::Other,       // remote control
            0x02 => DeviceKind::Other,       // numpad
            0x03 => DeviceKind::Mouse,
            0x04 => DeviceKind::Mouse,       // touchpad
            0x05 => DeviceKind::Mouse,       // trackball
            0x06 => DeviceKind::Other,       // presenter
            0x07 => DeviceKind::Receiver,
            0x08 => DeviceKind::Headset,
            0x09 => DeviceKind::Webcam,
            0x0a => DeviceKind::Speaker,
            0x0b => DeviceKind::Microphone,
            0x0c => DeviceKind::Light,
            _ => DeviceKind::Other,
        }
    }
}

pub struct KnownDevice {
    pub product_id: u16,
    pub name: &'static str,
    pub kind: DeviceKind,
    /// True when the PID belongs to a Unifying / Lightspeed / Bolt receiver,
    /// which must be probed for paired child devices instead of queried directly.
    pub receiver: bool,
}

const fn dev(product_id: u16, name: &'static str, kind: DeviceKind) -> KnownDevice {
    KnownDevice { product_id, name, kind, receiver: false }
}

const fn rcv(product_id: u16, name: &'static str) -> KnownDevice {
    KnownDevice { product_id, name, kind: DeviceKind::Receiver, receiver: true }
}

/// Curated subset of the Logitech G catalogue. Extend freely — nothing else
/// depends on an entry existing.
pub static KNOWN_DEVICES: &[KnownDevice] = &[
    // ---- Receivers -------------------------------------------------------
    rcv(0xc52b, "Unifying Receiver"),
    rcv(0xc532, "Unifying Receiver"),
    rcv(0xc534, "Unifying Receiver"),
    rcv(0xc539, "LIGHTSPEED Receiver"),
    rcv(0xc53a, "PowerPlay Receiver"),
    rcv(0xc53f, "LIGHTSPEED Receiver"),
    rcv(0xc541, "LIGHTSPEED Receiver"),
    rcv(0xc545, "LIGHTSPEED Receiver"),
    rcv(0xc547, "LIGHTSPEED Receiver"),
    rcv(0xc548, "LIGHTSPEED Receiver"),
    rcv(0xc52f, "Nano Receiver"),
    rcv(0xc548, "Bolt Receiver"),

    // ---- Mice ------------------------------------------------------------
    dev(0xc081, "G900 CHAOS SPECTRUM", DeviceKind::Mouse),
    dev(0xc083, "G403 PRODIGY", DeviceKind::Mouse),
    dev(0xc084, "G203 PRODIGY", DeviceKind::Mouse),
    dev(0xc085, "G402 HYPERION FURY", DeviceKind::Mouse),
    dev(0xc086, "G903 LIGHTSPEED", DeviceKind::Mouse),
    dev(0xc087, "G703 LIGHTSPEED", DeviceKind::Mouse),
    dev(0xc088, "PRO WIRELESS", DeviceKind::Mouse),
    dev(0xc08b, "G502 HERO", DeviceKind::Mouse),
    dev(0xc08d, "G502 LIGHTSPEED", DeviceKind::Mouse),
    dev(0xc08f, "G403 HERO", DeviceKind::Mouse),
    dev(0xc090, "G203 LIGHTSYNC", DeviceKind::Mouse),
    dev(0xc092, "G102 LIGHTSYNC", DeviceKind::Mouse),
    dev(0xc094, "PRO X SUPERLIGHT", DeviceKind::Mouse),
    dev(0xc095, "G502 X PLUS", DeviceKind::Mouse),
    dev(0xc097, "G502 X", DeviceKind::Mouse),
    dev(0xc098, "G502 X LIGHTSPEED", DeviceKind::Mouse),
    dev(0xc09b, "PRO X SUPERLIGHT 2", DeviceKind::Mouse),
    dev(0x4074, "G305 LIGHTSPEED", DeviceKind::Mouse),
    dev(0x4079, "PRO WIRELESS", DeviceKind::Mouse),
    dev(0x407f, "G502 LIGHTSPEED", DeviceKind::Mouse),
    dev(0x4082, "MX MASTER 3", DeviceKind::Mouse),
    dev(0x4086, "G903 LIGHTSPEED", DeviceKind::Mouse),
    dev(0x4093, "PRO X SUPERLIGHT", DeviceKind::Mouse),
    dev(0x4099, "G502 X PLUS", DeviceKind::Mouse),

    // ---- Keyboards -------------------------------------------------------
    dev(0xc32b, "G910 ORION SPARK", DeviceKind::Keyboard),
    dev(0xc330, "G410 ATLAS SPECTRUM", DeviceKind::Keyboard),
    dev(0xc331, "G810 ORION SPECTRUM", DeviceKind::Keyboard),
    dev(0xc333, "G610 ORION", DeviceKind::Keyboard),
    dev(0xc336, "G213 PRODIGY", DeviceKind::Keyboard),
    dev(0xc339, "PRO MECHANICAL", DeviceKind::Keyboard),
    dev(0xc33c, "G512 CARBON", DeviceKind::Keyboard),
    dev(0xc33e, "G915 LIGHTSPEED", DeviceKind::Keyboard),
    dev(0xc33f, "G815 LIGHTSYNC", DeviceKind::Keyboard),
    dev(0xc343, "PRO X TKL", DeviceKind::Keyboard),
    dev(0xc545, "G915 TKL", DeviceKind::Keyboard),
    dev(0x407c, "G915 LIGHTSPEED", DeviceKind::Keyboard),
    dev(0x4097, "PRO X 60", DeviceKind::Keyboard),

    // ---- Headsets --------------------------------------------------------
    dev(0x0a5b, "G933 ARTEMIS SPECTRUM", DeviceKind::Headset),
    dev(0x0a66, "G533", DeviceKind::Headset),
    dev(0x0a78, "G560 LIGHTSYNC", DeviceKind::Speaker),
    dev(0x0a87, "G935", DeviceKind::Headset),
    dev(0x0a8f, "G733 LIGHTSPEED", DeviceKind::Headset),
    dev(0x0aaa, "PRO X 2 LIGHTSPEED", DeviceKind::Headset),
    dev(0x0ab5, "G733 LIGHTSPEED", DeviceKind::Headset),
    dev(0x0afe, "PRO X 2 LIGHTSPEED", DeviceKind::Headset),
    dev(0x0b02, "A50 X", DeviceKind::Headset),

    // ---- Streaming gear --------------------------------------------------
    dev(0x0990, "YETI GX", DeviceKind::Microphone),
    dev(0x0995, "YETI ORB", DeviceKind::Microphone),
    dev(0x0ade, "YETI GX", DeviceKind::Microphone),
    dev(0xc900, "LITRA GLOW", DeviceKind::Light),
    dev(0xc901, "LITRA BEAM", DeviceKind::Light),
    dev(0xc903, "LITRA BEAM LX", DeviceKind::Light),
    dev(0x0893, "STREAMCAM", DeviceKind::Webcam),

    // ---- Sim racing ------------------------------------------------------
    dev(0xc262, "G920 DRIVING FORCE", DeviceKind::Wheel),
    dev(0xc266, "G923 RACING WHEEL", DeviceKind::Wheel),
];

pub fn lookup(product_id: u16) -> Option<&'static KnownDevice> {
    KNOWN_DEVICES.iter().find(|d| d.product_id == product_id)
}

pub fn is_receiver(product_id: u16) -> bool {
    lookup(product_id).map(|d| d.receiver).unwrap_or(false)
}

pub fn name_for(product_id: u16) -> Option<&'static str> {
    lookup(product_id).map(|d| d.name)
}

pub fn kind_for(product_id: u16) -> Option<DeviceKind> {
    lookup(product_id).map(|d| d.kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receivers_are_flagged() {
        assert!(is_receiver(0xc539));
        assert!(!is_receiver(0xc08b));
    }

    #[test]
    fn known_lookup() {
        assert_eq!(name_for(0xc08b), Some("G502 HERO"));
        assert_eq!(kind_for(0xc901), Some(DeviceKind::Light));
        assert_eq!(name_for(0xffff), None);
    }

    #[test]
    fn device_type_byte_mapping() {
        assert_eq!(DeviceKind::from_hidpp_type(0x00), DeviceKind::Keyboard);
        assert_eq!(DeviceKind::from_hidpp_type(0x03), DeviceKind::Mouse);
        assert_eq!(DeviceKind::from_hidpp_type(0x0c), DeviceKind::Light);
    }
}
