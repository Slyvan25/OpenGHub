//! Exercises the software-lighting sources without a device.
//!
//!     cargo run --example lightsync -- audio     # prints band levels for 6 s
//!     cargo run --example lightsync -- screen    # portal dialog, then region colours
use std::time::Duration;
use openghub_lib::lightsync::{AudioSource, Region, ScreenSource};

fn main() {
    let what = std::env::args().nth(1).unwrap_or_else(|| "audio".into());
    let rt = tokio::runtime::Runtime::new().unwrap();
    if what == "audio" {
        let src = AudioSource::start().expect("parec");
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(500));
            let l = src.levels();
            println!("rms {:.2}  low {:.2}  mid {:.2}  high {:.2}", l.rms, l.low, l.mid, l.high);
        }
    } else {
        rt.block_on(async {
            let src = ScreenSource::start(None).await.expect("portal");
            println!("restore token: {:?}", src.restore_token.as_ref().map(|t| &t[..8.min(t.len())]));
            for _ in 0..12 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                match src.frame() {
                    Some(f) => println!(
                        "frame {}x{}  whole {:?}  left {:?}  right {:?}",
                        f.w, f.h,
                        f.average(&Region::default()),
                        f.average(&Region { x: 0.0, y: 0.0, w: 0.5, h: 1.0 }),
                        f.average(&Region { x: 0.5, y: 0.0, w: 0.5, h: 1.0 })
                    ),
                    None => println!("no frame yet"),
                }
            }
        });
    }
}
