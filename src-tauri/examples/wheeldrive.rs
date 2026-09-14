//! Runs the userspace force-feedback driver for the first classic wheel for a
//! while, so it can be tested with `fftest` or the Python client in the repo.
//!
//!     cargo run --example wheeldrive -- 30
use std::time::Duration;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let api = hidapi::HidApi::new().unwrap();
    let endpoints = openghub_lib::wheel::enumerate(&api);
    let Some(ep) = endpoints.first() else {
        eprintln!("no classic wheel found");
        std::process::exit(1);
    };
    println!("{} on {}", ep.model.name, ep.path);
    let mut handle = openghub_lib::wheel::WheelHandle::open(&api, ep).unwrap();
    handle.set_range(900).unwrap();
    handle.start_bridge(vec![]).expect("bridge");
    println!("virtual wheel up for {secs}s — upload effects to the 'OpenGHub {}' event device", ep.model.name);
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        std::thread::sleep(Duration::from_millis(500));
        let s = *handle.state.lock();
        println!("steering {:+.3}  accel {:.2}  brake {:.2}  clutch {:.2}  buttons {:#x}", s.steering, s.accelerator, s.brake, s.clutch, s.buttons);
    }
    handle.stop_bridge();
}
