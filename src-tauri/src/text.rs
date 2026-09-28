//! Typing text through the virtual keyboard (G HUB's "TEXT" macro entries).
//!
//! uinput sends key codes, and the compositor turns them into characters with
//! the user's layout — so "@" is a different key on a Swedish keyboard than on
//! a US one. This module asks libxkbcommon, for the configured layout, which
//! key and modifiers produce each character. libxkbcommon is loaded at run
//! time; without it, or for characters the layout cannot type directly
//! (emoji, dead-key combinations), those characters are skipped.

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::OnceLock;

use xkbcommon_dl::{self as xkb, xkb_context_flags, xkb_keymap_compile_flags, xkb_rule_names};

/// Modifiers a character needs on top of its key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    /// Linux input key code (evdev), as the virtual keyboard takes it.
    pub code: u16,
    pub shift: bool,
    /// AltGr (ISO Level 3), e.g. `@` on Nordic layouts.
    pub altgr: bool,
}

/// Character → stroke for the user's layout, built once.
pub fn strokes() -> &'static HashMap<char, Stroke> {
    static MAP: OnceLock<HashMap<char, Stroke>> = OnceLock::new();
    MAP.get_or_init(|| {
        let (layout, variant) = configured_layout();
        let map = build(&layout, &variant).unwrap_or_default();
        log::info!("text typing: layout '{layout}' variant '{variant}', {} characters", map.len());
        map
    })
}

/// The desktop's first keyboard layout code, e.g. `se`.
pub fn layout_name() -> String {
    configured_layout().0
}

/// The first configured layout: environment, then KDE, then systemd-localed.
fn configured_layout() -> (String, String) {
    let first = |s: &str| s.split(',').next().unwrap_or("").trim().to_string();
    if let Ok(l) = std::env::var("XKB_DEFAULT_LAYOUT") {
        return (first(&l), first(&std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default()));
    }
    let home = std::env::var("HOME").unwrap_or_default();
    if let Ok(text) = std::fs::read_to_string(format!("{home}/.config/kxkbrc")) {
        let get = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
                .map(first)
                .unwrap_or_default()
        };
        let layout = get("LayoutList");
        if !layout.is_empty() {
            return (layout, get("VariantList"));
        }
    }
    if let Ok(out) = std::process::Command::new("localectl").arg("status").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        let get = |key: &str| {
            text.lines()
                .find_map(|l| l.trim().strip_prefix(key).map(|v| first(v.trim())))
                .unwrap_or_default()
        };
        let layout = get("X11 Layout:");
        if !layout.is_empty() {
            return (layout, get("X11 Variant:"));
        }
    }
    ("us".into(), String::new())
}

fn build(layout: &str, variant: &str) -> Option<HashMap<char, Stroke>> {
    let x = xkb::xkbcommon_option()?;
    let rules = CString::new("evdev").ok()?;
    let model = CString::new("pc105").ok()?;
    let layout_c = CString::new(layout).ok()?;
    let variant_c = CString::new(variant).ok()?;
    let names = xkb_rule_names {
        rules: rules.as_ptr(),
        model: model.as_ptr(),
        layout: layout_c.as_ptr(),
        variant: variant_c.as_ptr(),
        options: std::ptr::null(),
    };
    let mut map = HashMap::new();
    unsafe {
        let ctx = (x.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
        if ctx.is_null() {
            return None;
        }
        let keymap = (x.xkb_keymap_new_from_names)(ctx, &names, xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS);
        if keymap.is_null() {
            (x.xkb_context_unref)(ctx);
            return None;
        }
        let state = (x.xkb_state_new)(keymap);
        let index = |name: &str| {
            let c = CString::new(name).unwrap();
            (x.xkb_keymap_mod_get_index)(keymap, c.as_ptr())
        };
        let bit = |i: u32| if i == xkb::XKB_MOD_INVALID { 0 } else { 1u32 << i };
        let shift = bit(index("Shift"));
        // AltGr is Mod5 on the standard evdev layouts.
        let altgr = bit(index("Mod5"));
        let min = (x.xkb_keymap_min_keycode)(keymap).max(8);
        let max = (x.xkb_keymap_max_keycode)(keymap).min(255 + 8);
        // Plain first, so a character reachable several ways gets the simplest.
        for (mask, s, a) in [(0, false, false), (shift, true, false), (altgr, false, true), (shift | altgr, true, true)] {
            if (s && shift == 0) || (a && altgr == 0) {
                continue;
            }
            (x.xkb_state_update_mask)(state, mask, 0, 0, 0, 0, 0);
            for keycode in min..=max {
                let cp = (x.xkb_state_key_get_utf32)(state, keycode);
                let Some(ch) = char::from_u32(cp).filter(|c| *c != '\0' && !c.is_control() || *c == '\n' || *c == '\t') else {
                    continue;
                };
                map.entry(ch).or_insert(Stroke { code: (keycode - 8) as u16, shift: s, altgr: a });
            }
        }
        (x.xkb_state_unref)(state);
        (x.xkb_keymap_unref)(keymap);
        (x.xkb_context_unref)(ctx);
    }
    // Keys whose symbol is not a printable character.
    map.insert('\n', Stroke { code: crate::keymap::KEY_ENTER, shift: false, altgr: false });
    map.insert('\t', Stroke { code: crate::keymap::KEY_TAB, shift: false, altgr: false });
    Some(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swedish_layout_puts_at_on_altgr_2() {
        // Skipped where libxkbcommon or its data is missing (minimal CI images).
        let Some(map) = build("se", "") else { return };
        if map.is_empty() {
            return;
        }
        assert_eq!(map[&'a'], Stroke { code: 30, shift: false, altgr: false });
        assert_eq!(map[&'A'], Stroke { code: 30, shift: true, altgr: false });
        assert_eq!(map[&'@'], Stroke { code: 3, shift: false, altgr: true });
        assert_eq!(map[&'å'].code, 26);
    }
}
