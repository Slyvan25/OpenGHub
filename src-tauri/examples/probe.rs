//! Dumps everything OpenGHub can read from the Logitech devices on this machine.
//!
//! A read-only diagnostic — it never writes to a device. Useful when adding
//! support for hardware that is not in the registry, and for checking that the
//! udev rule took effect.
//!
//!     cargo run --example probe

use openghub_lib::artwork;
use openghub_lib::hidpp::{self, features, Handle};
use openghub_lib::state::DeviceManager;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("hidapi would not start: {e}");
            std::process::exit(1);
        }
    };

    let endpoints = hidpp::enumerate(&api);
    println!("HID++ endpoints: {}", endpoints.len());
    for endpoint in &endpoints {
        println!(
            "  {}  {:04x}:{:04x}  {}{}",
            endpoint.address.path,
            endpoint.address.vendor_id,
            endpoint.address.product_id,
            endpoint.hid_product.as_deref().unwrap_or("?"),
            if endpoint.is_receiver { "  [receiver]" } else { "" },
        );
        // Prove the permissions are right before blaming the protocol.
        match Handle::open(&api, endpoint.address.clone()) {
            Ok(_) => println!("      opened OK"),
            Err(e) => println!("      CANNOT OPEN: {e}"),
        }
    }

    println!("\n--- discovery (the path the app uses) ---");
    let manager = DeviceManager::new();
    let devices = manager.refresh();
    println!("devices: {} (demo: {})\n", devices.len(), manager.is_demo());

    for device in &devices {
        println!("{} [{}]", device.name, device.id);
        println!("  kind            {:?}", device.kind);
        println!("  connection      {:?}", device.connection);
        if !device.model_ids.is_empty() {
            let ids: Vec<String> = device.model_ids.iter().map(|i| artwork::key(*i)).collect();
            println!("  model ids       {}", ids.join(", "));
        }
        println!("  HID++           {}", device.protocol_version);
        println!("  capabilities    {:?}", device.capabilities);
        if let Some(b) = &device.battery {
            println!(
                "  battery         {}% {:?}{}",
                b.percentage,
                b.status,
                if b.approximate { " (approximate)" } else { "" }
            );
        }
        if let Some(d) = &device.dpi {
            println!(
                "  dpi             current {} default {} range {}..={} step {} ({} steps)",
                d.current,
                d.default,
                d.min,
                d.max,
                d.step,
                d.steps.len()
            );
        }
        if let Some(r) = &device.report_rate {
            println!(
                "  report rate     {} Hz of {:?}{}",
                r.current_hz,
                r.available_hz,
                if r.extended { " [0x8061]" } else { " [0x8060]" }
            );
        }
        if device.lighting_zones > 0 {
            println!("  lighting zones  {}", device.lighting_zones);
        }
        if let Some(e) = &device.last_error {
            println!("  last error      {e}");
        }

        match manager.enumerate_features(&device.id) {
            Ok(list) => {
                println!("  features        {}", list.len());
                for (id, index, kind) in list {
                    println!(
                        "      {id:#06x}  idx {index:>2}{}",
                        if kind & 0x80 != 0 { "  (obsolete)" } else { "" }
                    );
                }
            }
            Err(e) => println!("  features        unavailable: {e}"),
        }
        println!();
    }

    // Artwork: which id, if any, has a file the UI can show.
    let files = artwork::scan();
    println!("--- artwork ({} file(s) in {}) ---", files.len(), artwork::dir().display());
    for device in &devices {
        let ids: Vec<u16> =
            device.model_ids.iter().copied().chain(std::iter::once(device.product_id)).collect();
        match ids.iter().find(|id| files.contains_key(&artwork::key(**id))) {
            Some(id) => println!("  {:<26} {}", device.name, artwork::key(*id)),
            None => println!("  {:<26} none — drawing will be used", device.name),
        }
    }
    println!();

    // Exercise the raw feature helpers too, so a failure points at the layer.
    for device in &devices {
        if !device.capabilities.dpi {
            continue;
        }
        let sensors = manager.with_handle(&device.id, features::sensor_count);
        println!("{}: sensor count = {:?}", device.name, sensors);
    }
}
