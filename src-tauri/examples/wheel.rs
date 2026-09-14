//! Raw classic-protocol probe for a Logitech wheel's joystick interface.
//!
//!     cargo run --example wheel -- /dev/hidraw16 leds 0x1f      # light RPM LEDs
//!     cargo run --example wheel -- /dev/hidraw16 range 540       # rotation range
//!     cargo run --example wheel -- /dev/hidraw16 native          # PS -> PC mode switch
//!
//! Optional last argument: the report id to prefix (default 0x30 for PS mode,
//! use 0 for native mode where the command report has no id).
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let what = args.get(2).map(String::as_str).unwrap_or("leds");
    let arg = args.get(3).map(|s| parse(s)).unwrap_or(0);
    let rid: u8 = args.get(4).map(|s| parse(s) as u8).unwrap_or(0x30);

    let api = hidapi::HidApi::new().unwrap();
    let dev = api.open_path(&std::ffi::CString::new(path.clone()).unwrap()).unwrap();
    dev.set_blocking_mode(false).unwrap();

    let cmd: [u8; 7] = match what {
        "leds" => [0xf8, 0x12, arg as u8, 0, 0, 0, 0],
        "range" => [0xf8, 0x81, (arg & 0xff) as u8, (arg >> 8) as u8, 0, 0, 0],
        "native" => [0xf8, 0x09, 0x07, 0x01, 0x01, 0x00, 0x00],
        "autocenter-off" => [0xf5, 0, 0, 0, 0, 0, 0],
        "autocenter" => {
            // Same maths as new-lg4ff for a 0..65535 magnitude on non-MOMO wheels.
            let m = arg.min(65535) as u32;
            let (a, b) = if m <= 0xaaaa { (0x0c * m, 0x80 * m) } else { (0x0c * 0xaaaa + 0x06 * (m - 0xaaaa), 0x80 * 0xaaaa + 0xff * (m - 0xaaaa)) };
            let a = a >> 1;
            let out = [0xfe, 0x0d, (a / 0xaaaa) as u8, (a / 0xaaaa) as u8, (b / 0xaaaa) as u8, 0, 0];
            send(&dev, rid, &out);
            [0x14, 0, 0, 0, 0, 0, 0]
        }
        _ => panic!("unknown command"),
    };
    send(&dev, rid, &cmd);

    let mut buf = [0u8; 64];
    let deadline = std::time::Instant::now() + Duration::from_millis(400);
    while std::time::Instant::now() < deadline {
        let n = dev.read_timeout(&mut buf, 100).unwrap_or(0);
        if n > 0 { println!("<< {}", hex(&buf[..n.min(24)])); break; }
    }
}

fn send(dev: &hidapi::HidDevice, rid: u8, cmd: &[u8; 7]) {
    let mut bytes = vec![rid];
    bytes.extend_from_slice(cmd);
    println!(">> {}", hex(&bytes));
    match dev.write(&bytes) {
        Ok(n) => println!("   wrote {n} bytes"),
        Err(e) => println!("   write failed: {e}"),
    }
}

fn parse(s: &str) -> u32 {
    if let Some(h) = s.strip_prefix("0x") { u32::from_str_radix(h, 16).unwrap() } else { s.parse().unwrap() }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}
