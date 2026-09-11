//! Synthetic device catalogue.
//!
//! Used when no HID++ hardware is reachable — either because the machine has no
//! Logitech gear or because the hidraw nodes are not readable by this user (see
//! `packaging/70-openghub.rules`). Every snapshot here is flagged `demo: true`
//! so the UI can say so; nothing in this module ever touches the bus.

use crate::hidpp::features::{BatteryState, ChargeStatus, DpiState, ReportRateState};
use crate::hidpp::registry::DeviceKind;
use crate::state::{Capabilities, Connection, DeviceSnapshot};

fn battery(pct: u8, status: ChargeStatus) -> Option<BatteryState> {
    Some(BatteryState { percentage: pct, approximate: false, status, voltage_mv: None })
}

/// A range-type sensor, like most modern Logitech mice: bounds plus a step,
/// with no enumerated list. Matches what `parse_dpi_list` produces.
fn dpi(current: u16, min: u16, max: u16, step: u16) -> Option<DpiState> {
    Some(DpiState { sensor: 0, current, default: current, steps: Vec::new(), min, max, step })
}

fn rate(current: u32, available: &[u32]) -> Option<ReportRateState> {
    Some(ReportRateState {
        current_hz: current,
        available_hz: available.to_vec(),
        extended: available.iter().any(|r| *r > 1000),
    })
}

struct Spec {
    id: &'static str,
    name: &'static str,
    kind: DeviceKind,
    product_id: u16,
    connection: Connection,
    caps: Capabilities,
    battery: Option<BatteryState>,
    dpi: Option<DpiState>,
    rate: Option<ReportRateState>,
    zones: u8,
}

/// Mirrors a typical G HUB dashboard so every screen has something to render.
pub fn catalogue() -> Vec<DeviceSnapshot> {
    let specs = vec![
        Spec {
            // Two RGB zones, so the per-zone lighting UI has something to show.
            id: "demo-g502",
            name: "G502 LIGHTSPEED",
            kind: DeviceKind::Mouse,
            product_id: 0x407f,
            connection: Connection::Receiver,
            caps: Capabilities {
                dpi: true,
                report_rate: true,
                battery: true,
                lighting: true,
                onboard_memory: true,
            },
            battery: battery(74, ChargeStatus::Discharging),
            dpi: dpi(1600, 100, 25600, 50),
            rate: rate(1000, &[125, 250, 500, 1000]),
            zones: 2,
        },
        Spec {
            id: "demo-prox60",
            name: "PRO X 60",
            kind: DeviceKind::Keyboard,
            product_id: 0x4097,
            connection: Connection::Wireless,
            caps: Capabilities {
                dpi: false,
                report_rate: true,
                battery: true,
                lighting: true,
                onboard_memory: true,
            },
            battery: battery(100, ChargeStatus::Full),
            dpi: None,
            rate: rate(1000, &[125, 250, 500, 1000]),
            zones: 1,
        },
        Spec {
            id: "demo-prox2",
            name: "PRO X 2 LIGHTSPEED",
            kind: DeviceKind::Headset,
            product_id: 0x0afe,
            connection: Connection::Wireless,
            caps: Capabilities {
                dpi: false,
                report_rate: false,
                battery: true,
                lighting: false,
                onboard_memory: false,
            },
            battery: battery(17, ChargeStatus::Discharging),
            dpi: None,
            rate: None,
            zones: 0,
        },
        Spec {
            id: "demo-a50x",
            name: "A50 X Party Time",
            kind: DeviceKind::Headset,
            product_id: 0x0b02,
            connection: Connection::Wireless,
            caps: Capabilities {
                dpi: false,
                report_rate: false,
                battery: true,
                lighting: false,
                onboard_memory: false,
            },
            battery: battery(100, ChargeStatus::ChargingFull),
            dpi: None,
            rate: None,
            zones: 0,
        },
        Spec {
            id: "demo-litra-1",
            name: "LITRA BEAM",
            kind: DeviceKind::Light,
            product_id: 0xc901,
            connection: Connection::Wired,
            caps: Capabilities {
                dpi: false,
                report_rate: false,
                battery: false,
                lighting: true,
                onboard_memory: false,
            },
            battery: None,
            dpi: None,
            rate: None,
            zones: 1,
        },
        Spec {
            id: "demo-litra-2",
            name: "LITRA BEAM",
            kind: DeviceKind::Light,
            product_id: 0xc901,
            connection: Connection::Wired,
            caps: Capabilities {
                dpi: false,
                report_rate: false,
                battery: false,
                lighting: true,
                onboard_memory: false,
            },
            battery: None,
            dpi: None,
            rate: None,
            zones: 1,
        },
        Spec {
            id: "demo-yeti-gx",
            name: "Yeti GX",
            kind: DeviceKind::Microphone,
            product_id: 0x0ade,
            connection: Connection::Wired,
            caps: Capabilities {
                dpi: false,
                report_rate: false,
                battery: false,
                lighting: true,
                onboard_memory: false,
            },
            battery: None,
            dpi: None,
            rate: None,
            zones: 1,
        },
    ];

    specs
        .into_iter()
        .map(|s| DeviceSnapshot {
            id: s.id.to_string(),
            name: s.name.to_string(),
            kind: s.kind,
            vendor_id: crate::hidpp::LOGITECH_VID,
            product_id: s.product_id,
            serial: None,
            model_ids: vec![s.product_id],
            connection: s.connection,
            online: true,
            capabilities: s.caps,
            battery: s.battery,
            dpi: s.dpi,
            report_rate: s.rate,
            lighting_zones: s.zones,
            protocol_version: "4.5".to_string(),
            demo: true,
            last_error: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_all_demo_flagged() {
        let devices = catalogue();
        assert_eq!(devices.len(), 7);
        assert!(devices.iter().all(|d| d.demo));
    }

    #[test]
    fn ids_are_unique() {
        let devices = catalogue();
        let mut ids: Vec<_> = devices.iter().map(|d| d.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), devices.len());
    }
}
