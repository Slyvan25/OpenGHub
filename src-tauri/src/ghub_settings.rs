//! Imports a G HUB `settings.db` (Windows: `%LOCALAPPDATA%\LGHUB\settings.db`).
//!
//! The database is one SQLite table, `data`, whose latest row holds a JSON
//! document. What matters for us:
//!
//! - `profiles.profiles[]` — id, `applicationId` (Logitech's game id; the
//!   Desktop profile uses its own id), and `assignments[] { cardId, slotId }`.
//!   Slot ids look like `g502wireless_g7_m1` (`_shifted` for the G-Shift
//!   layer), `<model>_mouse_settings`, `<model>_lighting_setting_firmware`.
//! - `cards.cards[]` — the settings referenced by those assignments:
//!   `MOUSE_SETTINGS` (DPI table, shift DPI, report rate),
//!   `FIRMWARE_LIGHTING_SETTINGS` (effect per zone type), `MACRO_PLAYBACK`
//!   (keystroke: HID usage + modifier usages; or an integration action).
//! - Built-in mouse/device functions are not stored as cards; their ids are
//!   synthetic (`0f82f693-5b78-4cf5-867e-TTNN00000000`, TT = type, NN = n),
//!   decoded from the G502's factory defaults: 02 = mouse button n,
//!   04 01/02/03 = DPI up / down / shift, 04 0b/0c = scroll left / right,
//!   09 06 = battery level, 09 07 = unassigned.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::hidpp::{Error, Result};
use crate::keymap;
use crate::profiles::{Assignment, DeviceProfile, LightingSettings};

/// Everything found for one G HUB profile, keyed for our config.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedProfile {
    pub ghub_id: String,
    pub name: String,
    /// Logitech application id, `None` for the Desktop profile.
    pub application_id: Option<String>,
    /// Device slot prefix (`g502wireless`) → settings for that device.
    pub devices: HashMap<String, DeviceProfile>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub profiles: Vec<ImportedProfile>,
}

/// Reads the newest document out of `settings.db`.
pub fn read_document(path: &Path) -> Result<Value> {
    let conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| Error::other(format!("could not open {}: {e}", path.display())))?;
    let blob: Vec<u8> = conn
        .query_row("SELECT file FROM data ORDER BY _id DESC LIMIT 1", [], |r| r.get(0))
        .map_err(|e| Error::other(format!("not a G HUB settings database: {e}")))?;
    serde_json::from_slice(&blob).map_err(|e| Error::other(format!("settings document is not JSON: {e}")))
}

/// Parses the document into per-profile device settings.
pub fn parse(doc: &Value) -> ImportReport {
    let cards: HashMap<String, &Value> = doc
        .pointer("/cards/cards")
        .and_then(|c| c.as_array())
        .map(|a| a.iter().filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(|i| (i.to_string(), c))).collect())
        .unwrap_or_default();
    let app_names: HashMap<String, String> = doc
        .pointer("/applications/applications")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| Some((x.get("applicationId")?.as_str()?.to_string(), x.get("name")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for p in doc.pointer("/profiles/profiles").and_then(|p| p.as_array()).into_iter().flatten() {
        let ghub_id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let app = p.get("applicationId").and_then(|v| v.as_str()).map(str::to_owned);
        let app_name = app.as_ref().and_then(|a| app_names.get(a)).cloned().unwrap_or_default();
        let is_desktop = app_name == "APPLICATION_NAME_DESKTOP" || app.as_deref() == Some(ghub_id.as_str());
        let raw_name = p.get("name").and_then(|v| v.as_str()).unwrap_or("PROFILE_NAME_DEFAULT");
        let name = if raw_name == "PROFILE_NAME_DEFAULT" { "Default".to_string() } else { raw_name.to_string() };
        let mut imported = ImportedProfile {
            ghub_id,
            name,
            application_id: if is_desktop { None } else { app },
            devices: HashMap::new(),
            skipped: Vec::new(),
        };

        for a in p.get("assignments").and_then(|a| a.as_array()).into_iter().flatten() {
            let (Some(slot), Some(card_id)) = (a.get("slotId").and_then(|v| v.as_str()), a.get("cardId").and_then(|v| v.as_str())) else {
                continue;
            };
            let Some((device, rest)) = slot.split_once('_') else { continue };
            let dp = imported.devices.entry(device.to_string()).or_default();
            let card = cards.get(card_id).copied();

            if rest == "mouse_settings" {
                if let Some(ms) = card.and_then(|c| c.get("mouseSettings")) {
                    apply_mouse_settings(dp, ms);
                } else {
                    imported.skipped.push(format!("{slot}: card missing"));
                }
            } else if rest == "lighting_setting_firmware" {
                if let Some(fw) = card.and_then(|c| c.get("firmwareLightingSettings")) {
                    apply_lighting(dp, fw);
                } else {
                    imported.skipped.push(format!("{slot}: card missing"));
                }
            } else if let Some(control) = control_for_slot(rest) {
                let button: u8 = control.trim_start_matches("button-").split(':').next().and_then(|n| n.parse().ok()).unwrap_or(0);
                match assignment_for_card(card_id, card, button) {
                    Ok(Some((category, label, value))) => {
                        dp.assignments.retain(|x| x.control != control);
                        dp.assignments.push(Assignment { control, category, label, value });
                    }
                    Ok(None) => {} // factory default; nothing to store
                    Err(why) => imported.skipped.push(format!("{slot}: {why}")),
                }
            } else {
                imported.skipped.push(format!("{slot}: unknown slot"));
            }
        }
        out.push(imported);
    }
    ImportReport { profiles: out }
}

/// `g7_m1` → `button-7`, `g7_m1_shifted` → `button-7:gshift`.
fn control_for_slot(rest: &str) -> Option<String> {
    let mut parts = rest.split('_');
    let g = parts.next()?;
    let n: u8 = g.strip_prefix('g')?.parse().ok()?;
    let shifted = rest.ends_with("_shifted");
    Some(if shifted { format!("button-{n}:gshift") } else { format!("button-{n}") })
}

fn apply_mouse_settings(dp: &mut DeviceProfile, ms: &Value) {
    if let Some(t) = ms.get("dpiTable") {
        let levels: Vec<u16> = t
            .get("levels")
            .and_then(|l| l.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as u16).collect())
            .unwrap_or_default();
        if !levels.is_empty() {
            let active = t.get("activeDpi").and_then(|v| v.as_u64()).map(|v| v as u16);
            let shift = t.get("shiftDpi").and_then(|v| v.as_u64()).map(|v| v as u16);
            dp.active_stage = active.and_then(|a| levels.iter().position(|l| *l == a)).unwrap_or(0);
            dp.shift_stage = shift.and_then(|s| levels.iter().position(|l| *l == s));
            dp.dpi_stages = levels;
        }
    }
    if let Some(rate) = ms.pointer("/reportRate/value").and_then(|v| v.as_u64()) {
        dp.report_rate_hz = Some(rate as u32);
    }
}

fn colour_of(v: Option<&Value>) -> String {
    let c = |k: &str| v.and_then(|c| c.get(k)).and_then(|x| x.as_u64()).unwrap_or(0) as u8;
    format!("#{:02x}{:02x}{:02x}", c("red"), c("green"), c("blue"))
}

fn apply_lighting(dp: &mut DeviceProfile, fw: &Value) {
    for e in fw.get("effects").and_then(|e| e.as_array()).into_iter().flatten() {
        let zone = match e.get("zoneType").and_then(|z| z.as_str()) {
            Some("ZONE_PRIMARY") | Some("ZONE_KEYS") | Some("ZONE_MAIN") => 0u8,
            Some("ZONE_BRANDING") | Some("ZONE_LOGO") => 1,
            Some(z) => z.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0),
            None => 0,
        };
        let id = e.get("id").and_then(|i| i.as_str()).unwrap_or("OFF");
        let mut s = LightingSettings::default();
        match id {
            "FIXED" => {
                s.effect = "fixed".into();
                s.color = colour_of(e.pointer("/fixedParams/color"));
                s.brightness = intensity(e.pointer("/fixedParams/intensity"));
            }
            "CYCLE" => {
                s.effect = "cycle".into();
                s.rate_ms = e.pointer("/cycleParams/periodInMs").and_then(|v| v.as_u64()).unwrap_or(8000) as u16;
                s.brightness = intensity(e.pointer("/cycleParams/intensity"));
            }
            "BREATHING" => {
                s.effect = "breathing".into();
                s.color = colour_of(e.pointer("/breathingParams/color"));
                s.rate_ms = e.pointer("/breathingParams/periodInMs").and_then(|v| v.as_u64()).unwrap_or(5000) as u16;
                s.brightness = intensity(e.pointer("/breathingParams/intensity"));
            }
            _ => s.effect = "off".into(),
        }
        if zone == 0 {
            dp.lighting = Some(s.clone());
        }
        dp.lighting_zones.insert(zone.to_string(), s);
    }
}

/// G HUB stores intensity as a 0-1 ratio.
fn intensity(v: Option<&Value>) -> u8 {
    v.and_then(|x| x.as_f64()).map(|f| (f * 100.0).round().clamp(0.0, 100.0) as u8).unwrap_or(100)
}

/// Translates a card into `(category, label, value)`; `None` for factory
/// defaults that need no assignment; `Err` for things we cannot express.
fn assignment_for_card(
    card_id: &str,
    card: Option<&Value>,
    button: u8,
) -> std::result::Result<Option<(String, String, String)>, String> {
    if let Some(builtin) = card_id.strip_prefix("0f82f693-5b78-4cf5-867e-") {
        let kind = &builtin[0..2];
        let n = &builtin[2..4];
        // A mouse button doing its own job is the default, not an assignment.
        if kind == "02" && n.parse::<u8>().ok() == Some(button) {
            return Ok(None);
        }
        return Ok(match (kind, n) {
            ("02", "01") => Some(("action".into(), "Primary Click".into(), "mouse-left".into())),
            ("02", "02") => Some(("action".into(), "Secondary Click".into(), "mouse-right".into())),
            ("02", "03") => Some(("action".into(), "Middle Click".into(), "mouse-middle".into())),
            ("02", "04") => Some(("action".into(), "Back".into(), "mouse-back".into())),
            ("02", "05") => Some(("action".into(), "Forward".into(), "mouse-forward".into())),
            ("04", "01") => Some(("action".into(), "DPI Up".into(), "dpi-up".into())),
            ("04", "02") => Some(("action".into(), "DPI Down".into(), "dpi-down".into())),
            ("04", "03") => Some(("action".into(), "DPI Shift".into(), "dpi-shift".into())),
            ("04", "04") => Some(("action".into(), "DPI Cycle".into(), "dpi-cycle".into())),
            ("04", "05") => Some(("action".into(), "DPI Default".into(), "dpi-default".into())),
            // 09 07 = unassigned, scroll tilt and battery indicator are the
            // device's own defaults: nothing to store.
            _ => None,
        });
    }
    let Some(card) = card else {
        return Err("card not in database".into());
    };
    let name = card.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let Some(m) = card.get("macro") else {
        return Err(format!("{name}: not a macro card"));
    };
    match m.get("type").and_then(|t| t.as_str()) {
        Some("KEYSTROKE") => {
            let code = m.pointer("/keystroke/code").and_then(|c| c.as_u64()).unwrap_or(0) as u8;
            let mods: Vec<u8> = m
                .pointer("/keystroke/modifiers")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as u8).collect())
                .unwrap_or_default();
            let mut parts: Vec<String> = Vec::new();
            for md in mods {
                parts.push(
                    match md {
                        224 => "ctrl",
                        225 => "shift",
                        226 => "alt",
                        227 => "super",
                        228 => "rightctrl",
                        229 => "rightshift",
                        230 => "rightalt",
                        231 => "rightwin",
                        _ => continue,
                    }
                    .to_string(),
                );
            }
            let key = keymap::usage_code(code).and_then(key_name).ok_or_else(|| format!("{name}: unknown key usage {code}"))?;
            parts.push(key);
            let label = m.get("actionName").and_then(|a| a.as_str()).map(str::to_owned).unwrap_or(name);
            Ok(Some(("command".into(), label, parts.join("+"))))
        }
        Some("ACTION") => Err(format!("{name}: integration action (OBS/Discord/Overwolf) — no Linux equivalent")),
        Some(other) => Err(format!("{name}: macro type {other} not imported")),
        None => Err(format!("{name}: malformed macro")),
    }
}

/// A key code back to the library's spelling, so the stored value parses.
fn key_name(code: u16) -> Option<String> {
    use keymap::*;
    Some(
        match code {
            KEY_ENTER => "Return",
            KEY_ESC => "Escape",
            KEY_SPACE => "space",
            KEY_TAB => "Tab",
            KEY_BACKSPACE => "BackSpace",
            KEY_DELETE => "Delete",
            KEY_INSERT => "Insert",
            KEY_HOME => "Home",
            KEY_END => "End",
            KEY_PAGEUP => "PageUp",
            KEY_PAGEDOWN => "PageDown",
            KEY_UP => "Up",
            KEY_DOWN => "Down",
            KEY_LEFT => "Left",
            KEY_RIGHT => "Right",
            KEY_SYSRQ => "Print",
            KEY_CAPSLOCK => "CapsLock",
            KEY_MINUS => "minus",
            KEY_EQUAL => "equal",
            KEY_LEFTBRACE => "bracketleft",
            KEY_RIGHTBRACE => "bracketright",
            KEY_SEMICOLON => "semicolon",
            KEY_APOSTROPHE => "apostrophe",
            KEY_GRAVE => "grave",
            KEY_BACKSLASH => "backslash",
            KEY_COMMA => "comma",
            KEY_DOT => "period",
            KEY_SLASH => "slash",
            c if (KEY_F1..=KEY_F1 + 9).contains(&c) => return Some(format!("F{}", c - KEY_F1 + 1)),
            KEY_F11 => "F11",
            KEY_F12 => "F12",
            c => {
                // letters and digits
                for ch in ('a'..='z').chain('0'..='9') {
                    if keymap::char_code(ch) == Some(c) {
                        return Some(ch.to_string());
                    }
                }
                return None;
            }
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_desktop_profile() {
        let doc: Value = serde_json::json!({
            "applications": {"applications": [{"applicationId": "d", "name": "APPLICATION_NAME_DESKTOP"}]},
            "cards": {"cards": [
                {"id": "ms", "attribute": "MOUSE_SETTINGS", "mouseSettings": {"dpiTable": {"activeDpi": 800, "defaultDpi": 800, "levels": [400, 800, 1600], "shiftDpi": 400}, "reportRate": {"value": 1000}}},
                {"id": "fw", "attribute": "FIRMWARE_LIGHTING_SETTINGS", "firmwareLightingSettings": {"effects": [
                    {"id": "CYCLE", "zoneType": "ZONE_PRIMARY", "cycleParams": {"periodInMs": 8000, "intensity": 1}},
                    {"id": "FIXED", "zoneType": "ZONE_BRANDING", "fixedParams": {"color": {"red": 255, "green": 0, "blue": 16}, "intensity": 0.5}}]}},
                {"id": "copy", "attribute": "MACRO_PLAYBACK", "name": "Copy", "macro": {"type": "KEYSTROKE", "actionName": "CTRL + C", "keystroke": {"code": 6, "modifiers": [224]}}}
            ]},
            "profiles": {"profiles": [{"id": "d", "applicationId": "d", "name": "PROFILE_NAME_DEFAULT", "assignments": [
                {"cardId": "ms", "slotId": "g502wireless_mouse_settings"},
                {"cardId": "fw", "slotId": "g502wireless_lighting_setting_firmware"},
                {"cardId": "copy", "slotId": "g502wireless_g5_m1"},
                {"cardId": "0f82f693-5b78-4cf5-867e-040100000000", "slotId": "g502wireless_g8_m1"},
                {"cardId": "0f82f693-5b78-4cf5-867e-090700000000", "slotId": "g502wireless_g4_m1_shifted"}
            ]}]}
        });
        let r = parse(&doc);
        assert_eq!(r.profiles.len(), 1);
        let p = &r.profiles[0];
        assert_eq!(p.name, "Default");
        assert!(p.application_id.is_none());
        let dp = &p.devices["g502wireless"];
        assert_eq!(dp.dpi_stages, vec![400, 800, 1600]);
        assert_eq!(dp.active_stage, 1);
        assert_eq!(dp.shift_stage, Some(0));
        assert_eq!(dp.report_rate_hz, Some(1000));
        assert_eq!(dp.lighting_zones["0"].effect, "cycle");
        assert_eq!(dp.lighting_zones["1"].color, "#ff0010");
        assert_eq!(dp.lighting_zones["1"].brightness, 50);
        let copy = dp.assignments.iter().find(|a| a.control == "button-5").unwrap();
        assert_eq!(copy.value, "ctrl+c");
        assert!(dp.assignments.iter().any(|a| a.control == "button-8" && a.value == "dpi-up"));
        assert!(!dp.assignments.iter().any(|a| a.control == "button-4:gshift"), "unassigned defaults are not stored");
    }
}
