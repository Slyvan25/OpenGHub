//! User-supplied device artwork.
//!
//! Logitech's product renders are their copyright, so none ship with OpenGHub.
//! Instead the app looks for images the user has put on their own machine and
//! falls back to the built-in SVG drawings when there is nothing to show.
//!
//! Files are matched on product id, lowercase hex, four digits:
//!
//! ```text
//! ~/.local/share/openghub/devices/c08d.png    → G502 LIGHTSPEED
//! ```
//!
//! Note the lowercase directory: that is what `directories` produces on Linux,
//! and `scripts/fetch-artwork.sh` must agree with it exactly.
//!
//! `scripts/fetch-artwork.sh` populates this directory from Logitech's CDN.

use std::collections::HashMap;
use std::path::PathBuf;

/// Extensions we will load, in preference order.
const EXTENSIONS: [&str; 4] = ["png", "webp", "jpg", "jpeg"];

/// `$XDG_DATA_HOME/openghub/devices`, matching the config store's layout.
pub fn dir() -> PathBuf {
    directories::ProjectDirs::from("com", "openghub", "OpenGHub")
        .map(|d| d.data_dir().join("devices"))
        .unwrap_or_else(|| PathBuf::from("openghub-devices"))
}

/// Creates the directory so the user has somewhere obvious to drop files.
pub fn ensure_dir() -> std::io::Result<PathBuf> {
    let path = dir();
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// The key a file must be named after, e.g. `c08d`.
pub fn key(product_id: u16) -> String {
    format!("{product_id:04x}")
}

/// Every artwork file present, keyed by product id.
///
/// Scanned in one pass rather than probed per device: the frontend asks once and
/// then renders from the map, so a card never waits on the filesystem.
pub fn scan() -> HashMap<String, PathBuf> {
    let mut found = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return found;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
            continue;
        }
        // Tolerate `046d_c08d.png` as well as `c08d.png`.
        let key = stem.rsplit(['_', '-']).next().unwrap_or(stem).to_ascii_lowercase();
        if key.len() == 4 && key.chars().all(|c| c.is_ascii_hexdigit()) {
            found.entry(key).or_insert(path);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_four_digit_lowercase_hex() {
        assert_eq!(key(0xc08d), "c08d");
        assert_eq!(key(0x407f), "407f");
        // Leading zeros matter — 0x0afe must not become "afe".
        assert_eq!(key(0x0afe), "0afe");
    }

    #[test]
    fn artwork_dir_matches_what_the_fetch_script_writes() {
        // The script hardcodes this path; if `directories` ever changes the
        // casing, the two silently stop agreeing and no artwork is ever found.
        let path = dir();
        assert!(path.ends_with("openghub/devices"), "got {}", path.display());
    }

    #[test]
    fn scan_ignores_files_that_are_not_product_ids() {
        // Guards the filter, not the filesystem: readme.txt, notes.png etc.
        for stem in ["readme", "notes", "g502", "12345"] {
            let key = stem.rsplit(['_', '-']).next().unwrap().to_ascii_lowercase();
            let ok = key.len() == 4 && key.chars().all(|c| c.is_ascii_hexdigit());
            assert!(!ok, "'{stem}' should not be treated as a product id");
        }
        for stem in ["c08d", "046d_c08d", "g502-407f"] {
            let key = stem.rsplit(['_', '-']).next().unwrap().to_ascii_lowercase();
            assert!(key.len() == 4 && key.chars().all(|c| c.is_ascii_hexdigit()), "{stem}");
        }
    }
}
