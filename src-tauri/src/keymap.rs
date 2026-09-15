//! Names → Linux input key codes.
//!
//! Assignments carry keys in three shapes: OpenGHub's own library uses X
//! keysym-style names (`ctrl+c`, `Return`, `XF86AudioRaiseVolume`), Logitech's
//! game database uses display names (`Left Windows + ]`, `BACKSPACE`, `Space`),
//! and macros carry HID keyboard usages. All three end up as `KEY_*` codes
//! from `<linux/input-event-codes.h>` here.

/// A parsed key chord: modifiers first, then the key, pressed in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    pub codes: Vec<u16>,
}

// A subset of <linux/input-event-codes.h>.
pub const KEY_ESC: u16 = 1;
pub const KEY_1: u16 = 2;
pub const KEY_MINUS: u16 = 12;
pub const KEY_EQUAL: u16 = 13;
pub const KEY_BACKSPACE: u16 = 14;
pub const KEY_TAB: u16 = 15;
pub const KEY_Q: u16 = 16;
pub const KEY_LEFTBRACE: u16 = 26;
pub const KEY_RIGHTBRACE: u16 = 27;
pub const KEY_ENTER: u16 = 28;
pub const KEY_LEFTCTRL: u16 = 29;
pub const KEY_A: u16 = 30;
pub const KEY_SEMICOLON: u16 = 39;
pub const KEY_APOSTROPHE: u16 = 40;
pub const KEY_GRAVE: u16 = 41;
pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_BACKSLASH: u16 = 43;
pub const KEY_Z: u16 = 44;
pub const KEY_COMMA: u16 = 51;
pub const KEY_DOT: u16 = 52;
pub const KEY_SLASH: u16 = 53;
pub const KEY_RIGHTSHIFT: u16 = 54;
pub const KEY_KPASTERISK: u16 = 55;
pub const KEY_LEFTALT: u16 = 56;
pub const KEY_SPACE: u16 = 57;
pub const KEY_CAPSLOCK: u16 = 58;
pub const KEY_F1: u16 = 59;
pub const KEY_NUMLOCK: u16 = 69;
pub const KEY_SCROLLLOCK: u16 = 70;
pub const KEY_KP7: u16 = 71;
pub const KEY_KPMINUS: u16 = 74;
pub const KEY_KPPLUS: u16 = 78;
pub const KEY_KP0: u16 = 82;
pub const KEY_KPDOT: u16 = 83;
pub const KEY_F11: u16 = 87;
pub const KEY_F12: u16 = 88;
pub const KEY_KPENTER: u16 = 96;
pub const KEY_RIGHTCTRL: u16 = 97;
pub const KEY_KPSLASH: u16 = 98;
pub const KEY_SYSRQ: u16 = 99;
pub const KEY_RIGHTALT: u16 = 100;
pub const KEY_HOME: u16 = 102;
pub const KEY_UP: u16 = 103;
pub const KEY_PAGEUP: u16 = 104;
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const KEY_END: u16 = 107;
pub const KEY_DOWN: u16 = 108;
pub const KEY_PAGEDOWN: u16 = 109;
pub const KEY_INSERT: u16 = 110;
pub const KEY_DELETE: u16 = 111;
pub const KEY_MUTE: u16 = 113;
pub const KEY_VOLUMEDOWN: u16 = 114;
pub const KEY_VOLUMEUP: u16 = 115;
pub const KEY_PAUSE: u16 = 119;
pub const KEY_LEFTMETA: u16 = 125;
pub const KEY_RIGHTMETA: u16 = 126;
pub const KEY_COMPOSE: u16 = 127;
pub const KEY_STOP: u16 = 128;
pub const KEY_CALC: u16 = 140;
pub const KEY_SLEEP: u16 = 142;
pub const KEY_WWW: u16 = 150;
pub const KEY_MAIL: u16 = 155;
pub const KEY_BACK: u16 = 158;
pub const KEY_FORWARD: u16 = 159;
pub const KEY_NEXTSONG: u16 = 163;
pub const KEY_PLAYPAUSE: u16 = 164;
pub const KEY_PREVIOUSSONG: u16 = 165;
pub const KEY_STOPCD: u16 = 166;
pub const KEY_HOMEPAGE: u16 = 172;
pub const KEY_REFRESH: u16 = 173;
pub const KEY_F13: u16 = 183;
pub const KEY_F24: u16 = 194;
pub const KEY_SEARCH: u16 = 217;
pub const KEY_BRIGHTNESSDOWN: u16 = 224;
pub const KEY_BRIGHTNESSUP: u16 = 225;
pub const KEY_MICMUTE: u16 = 248;

/// Resolves one key name (any of the three shapes) to a code.
pub fn key_code(name: &str) -> Option<u16> {
    let raw = name.trim();
    if raw.is_empty() {
        return None;
    }
    // Single printable characters map by themselves.
    if raw.chars().count() == 1 {
        return char_code(raw.chars().next().unwrap());
    }
    let n: String = raw
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
        .collect();
    let code = match n.as_str() {
        "ctrl" | "control" | "controll" | "leftctrl" | "leftcontrol" | "lctrl" => KEY_LEFTCTRL,
        "rightctrl" | "rightcontrol" | "rctrl" | "controlr" => KEY_RIGHTCTRL,
        "shift" | "leftshift" | "lshift" | "shiftl" => KEY_LEFTSHIFT,
        "rightshift" | "rshift" | "shiftr" => KEY_RIGHTSHIFT,
        "alt" | "leftalt" | "lalt" | "altl" | "option" => KEY_LEFTALT,
        "rightalt" | "ralt" | "altr" | "altgr" => KEY_RIGHTALT,
        "super" | "win" | "windows" | "leftwindows" | "leftwin" | "meta" | "superl" | "metal" | "cmd" | "command" | "leftcommand" => KEY_LEFTMETA,
        "rightwindows" | "rightwin" | "superr" | "metar" | "rightcommand" => KEY_RIGHTMETA,
        "menu" | "apps" | "compose" => KEY_COMPOSE,
        "enter" | "return" | "ret" => KEY_ENTER,
        "esc" | "escape" => KEY_ESC,
        "space" | "spacebar" => KEY_SPACE,
        "tab" => KEY_TAB,
        "backspace" | "bksp" | "back space" => KEY_BACKSPACE,
        "delete" | "del" => KEY_DELETE,
        "insert" | "ins" => KEY_INSERT,
        "home" => KEY_HOME,
        "end" => KEY_END,
        "pageup" | "pgup" | "prior" => KEY_PAGEUP,
        "pagedown" | "pgdn" | "pgdown" | "next" => KEY_PAGEDOWN,
        "up" | "uparrow" | "arrowup" => KEY_UP,
        "down" | "downarrow" | "arrowdown" => KEY_DOWN,
        "left" | "leftarrow" | "arrowleft" => KEY_LEFT,
        "right" | "rightarrow" | "arrowright" => KEY_RIGHT,
        "capslock" | "caps" => KEY_CAPSLOCK,
        "numlock" => KEY_NUMLOCK,
        "scrolllock" => KEY_SCROLLLOCK,
        "print" | "printscreen" | "prtsc" | "sysrq" => KEY_SYSRQ,
        "pause" | "break" => KEY_PAUSE,
        "kpenter" | "numpadenter" => KEY_KPENTER,
        "kpplus" | "numpadplus" | "numpadadd" => KEY_KPPLUS,
        "kpminus" | "numpadminus" | "numpadsubtract" => KEY_KPMINUS,
        "kpmultiply" | "numpadmultiply" | "kpasterisk" => KEY_KPASTERISK,
        "kpdivide" | "numpaddivide" | "kpslash" => KEY_KPSLASH,
        "kpdecimal" | "numpaddecimal" | "kpdot" => KEY_KPDOT,
        "xf86audioraisevolume" | "volumeup" | "volup" => KEY_VOLUMEUP,
        "xf86audiolowervolume" | "volumedown" | "voldown" => KEY_VOLUMEDOWN,
        "xf86audiomute" | "mute" | "volumemute" => KEY_MUTE,
        "xf86audiomicmute" | "micmute" => KEY_MICMUTE,
        "xf86audioplay" | "playpause" | "play" | "mediaplaypause" => KEY_PLAYPAUSE,
        "xf86audiostop" | "mediastop" => KEY_STOPCD,
        "xf86audionext" | "nexttrack" | "medianext" => KEY_NEXTSONG,
        "xf86audioprev" | "prevtrack" | "previoustrack" | "mediaprev" => KEY_PREVIOUSSONG,
        "xf86back" | "browserback" => KEY_BACK,
        "xf86forward" | "browserforward" => KEY_FORWARD,
        "xf86homepage" | "browserhome" => KEY_HOMEPAGE,
        "xf86reload" | "browserrefresh" => KEY_REFRESH,
        "xf86search" | "browsersearch" => KEY_SEARCH,
        "xf86mail" | "mail" => KEY_MAIL,
        "xf86www" | "browser" => KEY_WWW,
        "xf86calculator" | "calculator" => KEY_CALC,
        "xf86sleep" | "sleep" => KEY_SLEEP,
        "xf86monbrightnessup" | "brightnessup" => KEY_BRIGHTNESSUP,
        "xf86monbrightnessdown" | "brightnessdown" => KEY_BRIGHTNESSDOWN,
        "minus" | "hyphen" => KEY_MINUS,
        "equal" | "equals" | "plus" => KEY_EQUAL,
        "bracketleft" | "leftbracket" => KEY_LEFTBRACE,
        "bracketright" | "rightbracket" => KEY_RIGHTBRACE,
        "semicolon" => KEY_SEMICOLON,
        "apostrophe" | "quote" => KEY_APOSTROPHE,
        "grave" | "tilde" | "backquote" => KEY_GRAVE,
        "backslash" => KEY_BACKSLASH,
        "comma" => KEY_COMMA,
        "period" | "dot" => KEY_DOT,
        "slash" => KEY_SLASH,
        _ => {
            // F1..F24, numpad digits, "key a"/"digit 1" style names.
            if let Some(f) = n.strip_prefix('f').and_then(|d| d.parse::<u16>().ok()) {
                return match f {
                    1..=10 => Some(KEY_F1 + f - 1),
                    11 => Some(KEY_F11),
                    12 => Some(KEY_F12),
                    13..=24 => Some(KEY_F13 + f - 13),
                    _ => None,
                };
            }
            if let Some(d) = n.strip_prefix("numpad").or_else(|| n.strip_prefix("kp")).and_then(|d| d.parse::<u16>().ok()) {
                return numpad_code(d);
            }
            if let Some(rest) = n.strip_prefix("key").or_else(|| n.strip_prefix("digit")) {
                if rest.chars().count() == 1 {
                    return char_code(rest.chars().next().unwrap());
                }
            }
            return None;
        }
    };
    Some(code)
}

fn numpad_code(d: u16) -> Option<u16> {
    match d {
        0 => Some(KEY_KP0),
        7..=9 => Some(KEY_KP7 + d - 7),
        4..=6 => Some(KEY_KP7 + 4 + d - 4), // KP4 = 75
        1..=3 => Some(KEY_KP7 + 8 + d - 1), // KP1 = 79
        _ => None,
    }
}

/// Letters, digits and punctuation on a US layout.
pub fn char_code(c: char) -> Option<u16> {
    let c = c.to_ascii_lowercase();
    Some(match c {
        'a' => KEY_A, 'b' => 48, 'c' => 46, 'd' => 32, 'e' => 18, 'f' => 33, 'g' => 34, 'h' => 35,
        'i' => 23, 'j' => 36, 'k' => 37, 'l' => 38, 'm' => 50, 'n' => 49, 'o' => 24, 'p' => 25,
        'q' => KEY_Q, 'r' => 19, 's' => 31, 't' => 20, 'u' => 22, 'v' => 47, 'w' => 17, 'x' => 45,
        'y' => 21, 'z' => KEY_Z,
        '1'..='9' => KEY_1 + (c as u16 - '1' as u16),
        '0' => 11,
        '-' => KEY_MINUS, '=' => KEY_EQUAL, '[' => KEY_LEFTBRACE, ']' => KEY_RIGHTBRACE,
        ';' => KEY_SEMICOLON, '\'' => KEY_APOSTROPHE, '`' => KEY_GRAVE, '\\' => KEY_BACKSLASH,
        ',' => KEY_COMMA, '.' => KEY_DOT, '/' => KEY_SLASH, ' ' => KEY_SPACE,
        _ => return None,
    })
}

/// Parses `ctrl+shift+z`, `Left Windows + ]` or `ALT + LEFT` into codes.
/// A trailing `+` (as in `ctrl++`) means the plus key.
pub fn parse_chord(value: &str) -> Option<Chord> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    let mut rest = v;
    while let Some(i) = rest.find('+') {
        let head = rest[..i].trim();
        if head.is_empty() {
            // "++" → a literal plus key
            parts.push("+");
            rest = rest[i + 1..].trim_start_matches('+');
            continue;
        }
        parts.push(head);
        rest = &rest[i + 1..];
    }
    let tail = rest.trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    let mut codes = Vec::with_capacity(parts.len());
    for p in parts {
        let code = if p == "+" { KEY_EQUAL } else { key_code(p)? };
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    if codes.is_empty() {
        return None;
    }
    Some(Chord { codes })
}

/// HID keyboard usage (as stored in macros) → key code.
pub fn usage_code(usage: u8) -> Option<u16> {
    Some(match usage {
        0x04..=0x1d => char_code((b'a' + (usage - 0x04)) as char)?,
        0x1e..=0x26 => KEY_1 + (usage - 0x1e) as u16,
        0x27 => 11,
        0x28 => KEY_ENTER,
        0x29 => KEY_ESC,
        0x2a => KEY_BACKSPACE,
        0x2b => KEY_TAB,
        0x2c => KEY_SPACE,
        0x2d => KEY_MINUS,
        0x2e => KEY_EQUAL,
        0x2f => KEY_LEFTBRACE,
        0x30 => KEY_RIGHTBRACE,
        0x31 => KEY_BACKSLASH,
        0x33 => KEY_SEMICOLON,
        0x34 => KEY_APOSTROPHE,
        0x35 => KEY_GRAVE,
        0x36 => KEY_COMMA,
        0x37 => KEY_DOT,
        0x38 => KEY_SLASH,
        0x39 => KEY_CAPSLOCK,
        0x3a..=0x43 => KEY_F1 + (usage - 0x3a) as u16,
        0x44 => KEY_F11,
        0x45 => KEY_F12,
        0x46 => KEY_SYSRQ,
        0x47 => KEY_SCROLLLOCK,
        0x48 => KEY_PAUSE,
        0x49 => KEY_INSERT,
        0x4a => KEY_HOME,
        0x4b => KEY_PAGEUP,
        0x4c => KEY_DELETE,
        0x4d => KEY_END,
        0x4e => KEY_PAGEDOWN,
        0x4f => KEY_RIGHT,
        0x50 => KEY_LEFT,
        0x51 => KEY_DOWN,
        0x52 => KEY_UP,
        0x53 => KEY_NUMLOCK,
        0x54 => KEY_KPSLASH,
        0x55 => KEY_KPASTERISK,
        0x56 => KEY_KPMINUS,
        0x57 => KEY_KPPLUS,
        0x58 => KEY_KPENTER,
        0x59..=0x61 => numpad_code((usage - 0x59 + 1) as u16)?,
        0x62 => KEY_KP0,
        0x63 => KEY_KPDOT,
        0x65 => KEY_COMPOSE,
        _ => return None,
    })
}

/// HID modifier bitmask → key codes (left ctrl/shift/alt/gui, then right).
pub fn modifier_codes(mask: u8) -> Vec<u16> {
    let table = [
        KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_LEFTMETA, KEY_RIGHTCTRL, KEY_RIGHTSHIFT, KEY_RIGHTALT, KEY_RIGHTMETA,
    ];
    (0..8).filter(|b| mask & (1 << b) != 0).map(|b| table[b]).collect()
}

/// The reverse: a key code → HID usage / modifier bit, for onboard profiles.
pub fn usage_for_code(code: u16) -> Option<u8> {
    (0x04..=0x65u8).find(|u| usage_code(*u) == Some(code))
}

pub fn modifier_bit_for_code(code: u16) -> Option<u8> {
    let table = [
        KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_LEFTMETA, KEY_RIGHTCTRL, KEY_RIGHTSHIFT, KEY_RIGHTALT, KEY_RIGHTMETA,
    ];
    table.iter().position(|c| *c == code).map(|i| 1u8 << i)
}

/// Consumer-control usages for media keys, for onboard profiles.
pub fn consumer_usage_for_code(code: u16) -> Option<u16> {
    Some(match code {
        KEY_VOLUMEUP => 0x00e9,
        KEY_VOLUMEDOWN => 0x00ea,
        KEY_MUTE => 0x00e2,
        KEY_PLAYPAUSE => 0x00cd,
        KEY_NEXTSONG => 0x00b5,
        KEY_PREVIOUSSONG => 0x00b6,
        KEY_STOPCD => 0x00b7,
        KEY_CALC => 0x0192,
        KEY_HOMEPAGE => 0x0223,
        KEY_BACK => 0x0224,
        KEY_FORWARD => 0x0225,
        KEY_REFRESH => 0x0227,
        KEY_SEARCH => 0x0221,
        KEY_MAIL => 0x018a,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_style_chords() {
        assert_eq!(parse_chord("ctrl+c").unwrap().codes, vec![KEY_LEFTCTRL, 46]);
        assert_eq!(parse_chord("ctrl+shift+z").unwrap().codes, vec![KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_Z]);
        assert_eq!(parse_chord("alt+Left").unwrap().codes, vec![KEY_LEFTALT, KEY_LEFT]);
        assert_eq!(parse_chord("super+]").unwrap().codes, vec![KEY_LEFTMETA, KEY_RIGHTBRACE]);
        assert_eq!(parse_chord("XF86AudioRaiseVolume").unwrap().codes, vec![KEY_VOLUMEUP]);
        assert_eq!(parse_chord("Return").unwrap().codes, vec![KEY_ENTER]);
        assert_eq!(parse_chord("Shift_L").unwrap().codes, vec![KEY_LEFTSHIFT]);
    }

    #[test]
    fn parses_logitech_database_names() {
        assert_eq!(parse_chord("Left Windows + ]").unwrap().codes, vec![KEY_LEFTMETA, KEY_RIGHTBRACE]);
        assert_eq!(parse_chord("ALT + LEFT").unwrap().codes, vec![KEY_LEFTALT, KEY_LEFT]);
        assert_eq!(parse_chord("BACKSPACE").unwrap().codes, vec![KEY_BACKSPACE]);
        assert_eq!(parse_chord("Space").unwrap().codes, vec![KEY_SPACE]);
        assert_eq!(parse_chord("CTRL + Left Windows + F").unwrap().codes, vec![KEY_LEFTCTRL, KEY_LEFTMETA, 33]);
        assert_eq!(parse_chord("F5").unwrap().codes, vec![KEY_F1 + 4]);
        assert_eq!(parse_chord("ctrl++").unwrap().codes, vec![KEY_LEFTCTRL, KEY_EQUAL]);
    }

    #[test]
    fn usages_round_trip() {
        for u in 0x04..=0x63u8 {
            if let Some(code) = usage_code(u) {
                assert_eq!(usage_for_code(code), Some(u), "usage {u:#x}");
            }
        }
        assert_eq!(modifier_codes(0x03), vec![KEY_LEFTCTRL, KEY_LEFTSHIFT]);
        assert_eq!(modifier_bit_for_code(KEY_LEFTALT), Some(0x04));
    }
}
