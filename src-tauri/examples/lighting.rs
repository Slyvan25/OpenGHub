//! Dumps what feature 0x8070 (Color LED Effects) reports for each zone.
//!
//! Read-only: it queries capabilities and never sets an effect. Use it to find
//! out which effect IDs a device actually accepts before implementing against
//! them — the parameter layout varies between device generations.
//!
//!     cargo run --example lighting            # read-only capability dump
//!     cargo run --example lighting -- --set    # also set a colour, then restore

use openghub_lib::hidpp::features::{self, lighting, LightEffect};
use openghub_lib::hidpp::{Packet, ReportKind};
use openghub_lib::state::DeviceManager;

fn hex(p: &Packet) -> String {
    p.params.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let manager = DeviceManager::new();
    let devices = manager.refresh();

    for device in devices.iter().filter(|d| d.capabilities.lighting) {
        println!("=== {} ({}) ===", device.name, device.id);

        // getInfo — reply layout differs by version, so print it raw.
        let info = manager.with_handle(&device.id, |h| {
            h.call_feature(lighting::ID_COLOR_LED_EFFECTS, lighting::FN_GET_INFO,
                           &[0xff, 0x00, 0x00], ReportKind::Long)
        });
        match &info {
            Ok(p) => println!("getInfo([ff,00,00]) -> {}", hex(p)),
            Err(e) => println!("getInfo failed: {e}"),
        }

        // Some versions want the zone index rather than 0xff.
        for probe in [0x00u8, 0x01] {
            let r = manager.with_handle(&device.id, |h| {
                h.call_feature(lighting::ID_COLOR_LED_EFFECTS, lighting::FN_GET_INFO,
                               &[probe, 0x00, 0x00], ReportKind::Long)
            });
            match r {
                Ok(p) => println!("getInfo([{probe:02x},00,00]) -> {}", hex(&p)),
                Err(e) => println!("getInfo([{probe:02x},..]) failed: {e}"),
            }
        }

        let zones = info.as_ref().map(|p| p.param(0)).unwrap_or(0);
        println!("\nzone count from getInfo param0: {zones}");

        for zone in 0..zones.min(8) {
            let zi = manager.with_handle(&device.id, |h| {
                h.call_feature(lighting::ID_COLOR_LED_EFFECTS, lighting::FN_GET_ZONE_INFO,
                               &[zone], ReportKind::Long)
            });
            match &zi {
                Ok(p) => {
                    // location BE u16, then effect count, then persistency caps.
                    println!(
                        "\nzone {zone}: raw [{}]  location={:#06x} num_effects={} persist={:#04x}",
                        hex(p), p.param_u16(1), p.param(3), p.param(4)
                    );
                    let n = p.param(3);
                    for effect in 0..n.min(16) {
                        let ei = manager.with_handle(&device.id, |h| {
                            h.call_feature(lighting::ID_COLOR_LED_EFFECTS, 0x02,
                                           &[zone, effect], ReportKind::Long)
                        });
                        match ei {
                            Ok(p) => println!(
                                "    effect idx {effect}: id={:#06x}  raw [{}]",
                                p.param_u16(2), hex(&p)
                            ),
                            Err(e) => println!("    effect idx {effect}: failed: {e}"),
                        }
                    }
                }
                Err(e) => println!("zone {zone}: getZoneInfo failed: {e}"),
            }
        }
        if std::env::args().any(|a| a == "--set") {
            set_test(&manager, &device.id, zones);
        }
        println!();
    }
}

/// Writes a colour, reads it back, then puts the original effect back.
fn set_test(manager: &DeviceManager, id: &str, zones: u8) {
    println!("\n--- set test (RAM only, original restored afterwards) ---");

    // Save what is there now so the mouse is left as we found it.
    let saved: Vec<(u8, (u8, Vec<u8>))> = (0..zones)
        .filter_map(|z| {
            manager.with_handle(id, |h| features::read_zone_effect(h, z)).ok().map(|e| (z, e))
        })
        .collect();
    for (z, (mode, payload)) in &saved {
        println!("saved zone {z}: mode={mode:#04x} payload=[{}]",
                 payload.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
    }

    for (label, effect, rgb) in [
        ("fixed magenta", LightEffect::Fixed, [0xff, 0x00, 0x80]),
        ("breathing cyan", LightEffect::Breathing { rate_ms: 3000, brightness: 100 }, [0x00, 0xb5, 0xe2]),
        ("colour cycle", LightEffect::Cycle { rate_ms: 5000, brightness: 100 }, [0, 0, 0]),
        ("off", LightEffect::Off, [0, 0, 0]),
    ] {
        let r = manager.set_lighting(id, lighting::ALL_ZONES, rgb, effect, false);
        match r {
            Ok(()) => {
                let back = manager.with_handle(id, |h| features::read_zone_effect(h, 0));
                match back {
                    Ok((mode, payload)) => println!(
                        "  {label:<16} OK   -> device reports mode={mode:#04x} payload=[{}]",
                        payload.iter().take(6).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
                    ),
                    Err(e) => println!("  {label:<16} OK   (read-back failed: {e})"),
                }
            }
            Err(e) => println!("  {label:<16} FAILED: {e}"),
        }
        std::thread::sleep(std::time::Duration::from_millis(700));
    }

    // Restore.
    for (z, (mode, payload)) in &saved {
        let mut params = vec![0u8; 13];
        params[0] = *z;
        params[1] = *mode;
        let n = payload.len().min(10);
        params[2..2 + n].copy_from_slice(&payload[..n]);
        params[12] = lighting::PERSIST_RAM;
        let r = manager.with_handle(id, |h| {
            h.call_feature(lighting::ID_COLOR_LED_EFFECTS, lighting::FN_SET_ZONE_EFFECT,
                           &params, ReportKind::Long)
        });
        println!("restored zone {z}: {}", if r.is_ok() { "OK" } else { "FAILED" });
    }
}
