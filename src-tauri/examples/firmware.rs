//! Fetches the firmware catalogue and lists what applies to each connected device.
//!     cargo run --example firmware
use openghub_lib::{firmware, state::DeviceManager};
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    match firmware::refresh_catalog() {
        Ok(c) => {
            println!("catalogue: {} package(s), fetched {:?}", c.packages.len(), c.fetched);
            for p in &c.packages {
                println!(
                    "  {:32} v{:<10} runtime {:?} bootloader {:?} {} bytes blockers {:?}",
                    p.depot,
                    p.version,
                    p.product_ids.iter().map(|i| format!("{i:04x}")).collect::<Vec<_>>(),
                    p.bootloader_ids.iter().map(|i| format!("{i:04x}")).collect::<Vec<_>>(),
                    p.image_size,
                    p.start_blockers
                );
            }
            for f in &c.failed {
                println!("  FAILED {f}");
            }
        }
        Err(e) => println!("catalogue: {e}"),
    }
    let manager = DeviceManager::new();
    for snap in manager.refresh() {
        let check = firmware::check(&snap);
        println!("{}: {:?} installed {:?} ({:?}) package {:?}", snap.name, check.state, check.installed, check.installed_ghub, check.package.map(|p| p.version));
    }
}
