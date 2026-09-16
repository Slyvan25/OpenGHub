//! Dry-runs the G HUB settings.db importer: prints what would be imported.
//!
//!     cargo run --example ghubsettings -- "/path/to/settings.db"
fn main() {
    let path = std::env::args().nth(1).expect("path to settings.db");
    let doc = openghub_lib::ghub_settings::read_document(std::path::Path::new(&path)).expect("read");
    let report = openghub_lib::ghub_settings::parse(&doc);
    for p in report.profiles {
        println!("profile {:?} (app {:?})", p.name, p.application_id);
        for (dev, dp) in &p.devices {
            println!("  device {dev}: dpi {:?} active {} shift {:?} rate {:?}", dp.dpi_stages, dp.active_stage, dp.shift_stage, dp.report_rate_hz);
            for (z, l) in &dp.lighting_zones {
                println!("    zone {z}: {} {} {}% {}ms", l.effect, l.color, l.brightness, l.rate_ms);
            }
            for a in &dp.assignments {
                println!("    {} -> {} [{}] {}", a.control, a.label, a.category, a.value);
            }
        }
        for s in p.skipped {
            println!("  skipped: {s}");
        }
    }
}
