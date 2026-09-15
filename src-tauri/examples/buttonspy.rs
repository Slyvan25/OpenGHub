//! Probes HID++ 0x8110 (Mouse Button Spy) on the first mouse: button count,
//! current remapping, then starts the spy and prints reports while you click.
//!
//!     cargo run --example buttonspy -- 8
use std::time::{Duration, Instant};
use openghub_lib::hidpp::{self, Handle, ReportKind};

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    let api = hidapi::HidApi::new().unwrap();
    for ep in hidpp::enumerate(&api) {
        let indices: Vec<u8> = if ep.is_receiver { (1..=6).collect() } else { vec![0xff] };
        for idx in indices {
            let mut addr = ep.address.clone();
            addr.device_index = idx;
            let Ok(mut h) = Handle::open(&api, addr) else { continue };
            if h.ping().is_err() { continue; }
            let Ok(spy) = h.feature_index(0x8110) else { continue };
            println!("device {:04x} index {idx}: 0x8110 at feature index {spy}", ep.address.product_id);
            let n = h.call(spy, 0, &[], ReportKind::Short).map(|p| p.param(0));
            println!("  getNbOfButtons -> {:?}", n);
            let remap = h.call(spy, 3, &[], ReportKind::Long).map(|p| p.params.clone());
            println!("  getRemapping   -> {:?}", remap.as_ref().map(|v| v.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")));
            println!("  startSpy       -> {:?}", h.call(spy, 1, &[], ReportKind::Short).map(|_| ()));
            println!("  click buttons now ({secs} s)…");
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(secs) {
                for ev in h.poll_events() {
                    println!("  event feature {} fn {} params {}", ev.feature_index, ev.function_id(), ev.params.iter().take(6).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            println!("  stopSpy        -> {:?}", h.call(spy, 2, &[], ReportKind::Short).map(|_| ()));
            return;
        }
    }
    println!("no device with 0x8110 answered (mouse asleep?)");
}
