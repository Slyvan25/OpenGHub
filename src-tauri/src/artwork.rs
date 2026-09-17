//! User-supplied device artwork.
//!
//! Logitech's product renders are their copyright, so none ship with OpenGHub.
//! Instead the app looks for images the user has put on their own machine and
//! falls back to the built-in SVG drawings when there is nothing to show.
//!
//! Files are matched on product id, lowercase hex, four digits:
//!
//! ```text
//! ~/.local/share/openghub/devices/c08d.png          → base render
//! ~/.local/share/openghub/devices/c08d-zone0.png    → mask for zone 0
//! ~/.local/share/openghub/devices/c08d-zone1.png    → mask for zone 1
//! ```
//!
//! The per-zone files are **masks**: their alpha channel marks where that zone's
//! light falls, and the UI fills them with the live colour. This is the same
//! model G HUB uses — its device descriptors carry a `render_icon_key` per zone
//! pointing at an image, never coordinates — so a mask gives pixel-accurate
//! lighting instead of an approximate blob.
//!
//! Note the lowercase directory: that is what `directories` produces on Linux,
//! and `scripts/fetch-artwork.sh` must agree with it exactly.
//!
//! `scripts/fetch-artwork.sh` populates this directory from Logitech's CDN.

use std::collections::HashMap;
use std::path::PathBuf;

/// Extensions we will load, in preference order. `json` only for `.layout.json`.
const EXTENSIONS: [&str; 5] = ["png", "webp", "jpg", "jpeg", "json"];

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

/// True for `c08d`, `c08d-side`, `c08d-thumb`, `c08d-zone2`, `c08d.layout`.
fn is_artwork_key(key: &str) -> bool {
    let product = key
        .split_once('-')
        .map(|(p, _)| p)
        .or_else(|| key.strip_suffix(".layout"))
        .unwrap_or(key);
    if !(product.len() == 4 && product.chars().all(|c| c.is_ascii_hexdigit())) {
        return false;
    }
    match key.split_once('-') {
        None => true,
        Some((_, "side")) | Some((_, "thumb")) | Some((_, "base")) | Some((_, "pedals")) => true,
        Some((_, rest)) => rest
            .strip_prefix("zone")
            .map(|z| !z.is_empty() && z.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false),
    }
}

/// The `.layout.json` for a product id, parsed, if one was imported.
pub fn layout_for(product_ids: &[u16]) -> Option<crate::depot::ArtworkLayout> {
    let files = scan();
    for pid in product_ids {
        if let Some(path) = files.get(&format!("{}.layout", key(*pid))) {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(layout) = serde_json::from_str(&text) {
                    return Some(layout);
                }
            }
        }
    }
    None
}

/// The key a zone mask must be filed under.
pub fn zone_key(product_id: u16, zone: u8) -> String {
    format!("{}-zone{zone}", key(product_id))
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
        let stem = stem.to_ascii_lowercase();
        // Recognised forms, all keyed on the product id:
        //   c08d.png          base render (front)      c08d-side.png    side view
        //   c08d-thumb.png    dashboard thumbnail      c08d-zone1.png   zone mask
        //   c08d.layout.json  zones + button positions from a G HUB depot
        //   046d_c08d.png     tolerated alongside the bare form
        let key = if let Some((product, suffix)) = stem.split_once('-') {
            let product = product.rsplit('_').next().unwrap_or(product);
            let suffix_ok = suffix == "side"
                || suffix == "thumb"
                || suffix == "base"
                || suffix == "pedals"
                || suffix.strip_prefix("zone").map(|z| !z.is_empty() && z.chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
            if !suffix_ok {
                continue;
            }
            format!("{product}-{suffix}")
        } else if stem.ends_with(".layout") {
            let product = stem.trim_end_matches(".layout");
            format!("{}.layout", product.rsplit('_').next().unwrap_or(product))
        } else {
            stem.rsplit('_').next().unwrap_or(&stem).to_string()
        };
        if is_artwork_key(&key) {
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
    fn zone_masks_get_their_own_keys() {
        assert_eq!(zone_key(0xc08d, 1), "c08d-zone1");
        assert!(is_artwork_key("c08d"));
        assert!(is_artwork_key("c08d-zone0"));
        assert!(is_artwork_key("407f-zone12"));
        assert!(is_artwork_key("407f-side"));
        assert!(is_artwork_key("407f-thumb"));
        assert!(is_artwork_key("407f.layout"));
        assert!(!is_artwork_key("notes"));
        assert!(!is_artwork_key("c08d-zoneX"));
        assert!(!is_artwork_key("c08d-back"));
        assert!(!is_artwork_key("12345-zone1"));
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
