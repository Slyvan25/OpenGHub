//! Read-only diagnostic: lists every device, its HID++ features, lighting zones
//! and onboard-memory info. Never writes to a device.
use openghub_lib::state::DeviceManager;

fn main() {
    let m = DeviceManager::new();
    for d in m.refresh() {
        println!("== {} ({:04x}) id={} kind={:?} conn={:?} onboard_mode={:?} zones={}",
            d.name, d.product_id, d.id, d.kind, d.connection, d.onboard_mode, d.lighting_zones);
        match m.enumerate_features(&d.id) {
            Ok(fs) => for (id, idx, kind) in fs {
                println!("   feature {id:#06x} idx={idx} kind={kind:#04x}");
            },
            Err(e) => println!("   features: {e}"),
        }
        match m.lighting_zones(&d.id) {
            Ok(z) => println!("   zones: {z:?}"),
            Err(e) => println!("   zones: {e}"),
        }
    }
}
