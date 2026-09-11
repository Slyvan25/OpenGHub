//! Integrity check: is the real profile still intact after the write tests?
use openghub_lib::hidpp::onboard::{self, Button};
use openghub_lib::state::DeviceManager;

fn main() {
    let manager = DeviceManager::new();
    let devices = manager.refresh();
    let Some(d) = devices.iter().find(|d| d.capabilities.onboard_memory) else { return };
    let id = &d.id;

    let (info, profiles) = match manager.read_onboard_profiles(id) {
        Ok(v) => v,
        Err(e) => {
            println!("PROFILE READ FAILED: {e}");
            return;
        }
    };
    let size = info.sector_size as usize;

    for p in &profiles {
        println!("profile sector {}: {} Hz, dpi {:?}",
                 p.sector, 1000 / p.report_rate_ms.max(1) as u32, p.dpi);
        let mice = p.buttons.iter().filter(|b| matches!(b, Button::Mouse { .. })).count();
        let special = p.buttons.iter().filter(|b| matches!(b, Button::Special { .. })).count();
        println!("  buttons: {mice} mouse + {special} special = {}", p.buttons.len());
        let raw = manager
            .with_handle(id, |h| onboard::read_sector(h, p.sector, size, false))
            .expect("reread");
        println!("  checksum valid: {}", onboard::checksum_ok(&raw));
    }

    // And the scratch sector we used.
    let target = info.sector_count as u16 - 1;
    let s = manager
        .with_handle(id, |h| onboard::read_sector(h, target, size, false))
        .expect("scratch");
    println!("\nscratch sector {target}: erased={} checksum_valid={}",
             onboard::is_erased(&s), onboard::checksum_ok(&s));
}
