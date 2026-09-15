//! Read-only onboard profile inspector, and the backup tool.
//!
//!     cargo run --example onboard             # decode profiles
//!     cargo run --example onboard -- --backup # also dump every sector to a file

use openghub_lib::hidpp::onboard::Button;
use openghub_lib::state::DeviceManager;

fn main() {
    let manager = DeviceManager::new();
    for d in manager.refresh() {
        if !d.capabilities.onboard_memory {
            continue;
        }
        println!("=== {} ===", d.name);

        match manager.read_onboard_profiles(&d.id) {
            Ok((info, profiles)) => {
                println!(
                    "profiles={} buttons={} sectors={} sectorSize={} profileFormat={} macroFormat={}",
                    info.profile_count,
                    info.button_count,
                    info.sector_count,
                    info.sector_size,
                    info.profile_format,
                    info.macro_format
                );
                for p in &profiles {
                    println!("\n-- profile in sector {} --", p.sector);
                    println!("  report rate   {} ms ({} Hz)", p.report_rate_ms,
                             1000 / p.report_rate_ms.max(1) as u32);
                    println!("  dpi ladder    {:?}  (default index {})", p.dpi, p.default_dpi_index);
                    println!("  buttons:");
                    for (i, b) in p.buttons.iter().enumerate() {
                        let text = match b {
                            Button::Mouse { mask } => format!("mouse button {}", mask.trailing_zeros() + 1),
                            Button::Special { action } => format!("special action {action}"),
                            Button::Macro { sector, offset } => format!("macro @ sector {sector} +{offset}"),
                            Button::Key { modifiers, usage } => format!("key usage {usage:#04x} modifiers {modifiers:#04x}"),
                            Button::Consumer { usage } => format!("consumer usage {usage:#06x}"),
                            Button::Disabled => "disabled".into(),
                            Button::Raw { bytes } => format!("raw {bytes:02x?}"),
                        };
                        println!("    {:>2}. {text}", i + 1);
                    }
                }
            }
            Err(e) => println!("could not read profiles: {e}"),
        }

        if std::env::args().any(|a| a == "--backup") {
            match manager.backup_onboard(&d.id) {
                Ok(path) => println!("\nbackup written to {}", path.display()),
                Err(e) => println!("\nbackup FAILED: {e}"),
            }
        }
        println!();
    }
}
