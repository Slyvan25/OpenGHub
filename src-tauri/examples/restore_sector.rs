//! Restores single onboard sectors from a backup, only where they differ.
//!
//!     cargo run --example restore_sector -- <product-id-hex> <backup.json>          # dry run
//!     cargo run --example restore_sector -- <product-id-hex> <backup.json> --write  # write
//!
//! Erased sectors in the backup (all 0xff) and the directory (sector 0) are
//! never written. Each written sector is read back and compared.
use openghub_lib::hidpp::onboard;
use openghub_lib::state::DeviceManager;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pid = u16::from_str_radix(&args[1], 16).expect("product id");
    let text = std::fs::read_to_string(&args[2]).expect("backup file");
    let write = args.iter().any(|a| a == "--write");
    let backup: onboard::MemoryBackup = serde_json::from_str(&text).expect("backup json");

    let m = DeviceManager::new();
    let dev = m.refresh().into_iter().find(|d| d.product_id == pid).expect("device not connected");
    println!("device: {} ({:04x})", dev.name, dev.product_id);

    let result = m.with_handle(&dev.id, |h| {
        let info = onboard::read_info(h)?;
        let size = info.sector_size as usize;
        for (i, hex) in backup.sectors.iter().enumerate().skip(1) {
            let want = unhex(hex);
            if want.len() != size || onboard::is_erased(&want) {
                continue;
            }
            let crc_ok = onboard::crc16(&want[..size - 2]) == u16::from_be_bytes([want[size - 2], want[size - 1]]);
            let have = onboard::read_sector(h, i as u16, size, false)?;
            if have == want {
                continue;
            }
            let diff: Vec<usize> = (0..size).filter(|k| have[*k] != want[*k]).collect();
            println!("sector {i}: {} byte(s) differ at {diff:?}; backup crc ok: {crc_ok}", diff.len());
            if !crc_ok {
                println!("  skipped: backup sector has a bad checksum");
                continue;
            }
            if write {
                onboard::write_sector(h, i as u16, &want)?;
                let back = onboard::read_sector(h, i as u16, size, true)?;
                println!("  written, read back {}", if back == want { "identical" } else { "DIFFERENT" });
            }
        }
        Ok(())
    });
    if let Err(e) = result {
        println!("error: {e}");
    }
    if !write {
        println!("(dry run — add --write to write)");
    }
}
