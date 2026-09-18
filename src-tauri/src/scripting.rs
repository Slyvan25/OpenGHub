//! Lua scripting — G HUB's per-profile scripts, with the G HUB Lua API.
//!
//! A profile's script runs on its own thread in a Lua 5.4 VM while the
//! profile is active. The script defines `OnEvent(event, arg, family)` and
//! gets `"PROFILE_ACTIVATED"`, `"MOUSE_BUTTON_PRESSED"` / `"…_RELEASED"`
//! (arg = button number), and `"PROFILE_DEACTIVATED"`. The API it can call is
//! the familiar G HUB set: `PressKey`, `ReleaseKey`, `PressAndReleaseKey`,
//! `PressMouseButton` … `MoveMouseRelative`, `MoveMouseWheel`, `Sleep`,
//! `OutputLogMessage`, `IsMouseButtonPressed`, `IsModifierPressed`,
//! `GetRunningTime`, `GetDate`, `PlayMacro`, `SetMouseDPITableIndex`,
//! `SetBacklightColor`, `EnablePrimaryMouseButtonEvents`, `ClearLog`.
//!
//! Input is injected through the same virtual keyboard the assignments use.
//! Anything Windows-only (`OutputDebugMessage` to a debugger, M-key states)
//! is accepted and does nothing, so scripts written for G HUB load unchanged.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mlua::{Lua, Value};
use parking_lot::Mutex;

use crate::keymap;
use crate::remap::Injector;

/// Something the script's `OnEvent` should hear about.
#[derive(Debug, Clone)]
pub enum ScriptEvent {
    MouseButton { button: u8, pressed: bool },
    Stop,
}

/// What the script can ask the app to do that needs more than the injector.
pub enum HostRequest {
    SetDpiIndex(usize),
    SetDpiTable(Vec<u16>, usize),
    SetBacklight([u8; 3]),
    PlayMacro(String),
}

const LOG_LINES: usize = 400;

struct Running {
    tx: Sender<ScriptEvent>,
    thread: JoinHandle<()>,
    profile_id: String,
}

/// Owns the running script and its log.
pub struct Scripting {
    running: Mutex<Option<Running>>,
    log: Arc<Mutex<VecDeque<String>>>,
    /// Buttons currently pressed on the mouse, kept up to date by the pump so
    /// `IsMouseButtonPressed` can answer without asking the device.
    pressed: Arc<Mutex<u16>>,
    /// `EnablePrimaryMouseButtonEvents(true)` — off by default, as in G HUB.
    primary_events: Arc<Mutex<bool>>,
    host_tx: Mutex<Option<Sender<HostRequest>>>,
}

impl Default for Scripting {
    fn default() -> Self {
        Self::new()
    }
}

impl Scripting {
    pub fn new() -> Self {
        Scripting {
            running: Mutex::new(None),
            log: Arc::new(Mutex::new(VecDeque::new())),
            pressed: Arc::new(Mutex::new(0)),
            primary_events: Arc::new(Mutex::new(false)),
            host_tx: Mutex::new(None),
        }
    }

    pub fn active_profile(&self) -> Option<String> {
        self.running.lock().as_ref().map(|r| r.profile_id.clone())
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().is_some()
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.log.lock().iter().cloned().collect()
    }

    pub fn clear_log(&self) {
        self.log.lock().clear();
    }

    fn push_log(log: &Mutex<VecDeque<String>>, line: String) {
        let mut l = log.lock();
        if l.len() >= LOG_LINES {
            l.pop_front();
        }
        l.push_back(line);
    }

    /// Stops the running script, sending `PROFILE_DEACTIVATED` first.
    pub fn stop(&self) {
        if let Some(r) = self.running.lock().take() {
            let _ = r.tx.send(ScriptEvent::Stop);
            // The script may be inside Sleep(); give it a moment, then move on.
            let deadline = Instant::now() + Duration::from_millis(1500);
            while !r.thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    /// Forwards a button edge to the script, if one is running.
    pub fn mouse_button(&self, button: u8, pressed: bool) {
        {
            let mut p = self.pressed.lock();
            if pressed {
                *p |= 1 << button;
            } else {
                *p &= !(1 << button);
            }
        }
        if button == 0 && !*self.primary_events.lock() {
            return;
        }
        if let Some(r) = self.running.lock().as_ref() {
            let _ = r.tx.send(ScriptEvent::MouseButton { button: button + 1, pressed });
        }
    }

    /// Starts `source` for `profile_id`, replacing whatever ran before.
    /// `host` receives requests the script makes that need the device layer.
    pub fn start(&self, profile_id: &str, source: &str, injector: Arc<Injector>, host: Sender<HostRequest>) {
        self.stop();
        *self.host_tx.lock() = Some(host.clone());
        let (tx, rx) = mpsc::channel::<ScriptEvent>();
        let log = Arc::clone(&self.log);
        let pressed = Arc::clone(&self.pressed);
        let primary = Arc::clone(&self.primary_events);
        let source = source.to_string();
        let name = profile_id.to_string();
        Self::push_log(&log, format!("[{}] script started", stamp()));
        log::info!("lua script for profile {profile_id} started");
        let thread = std::thread::Builder::new()
            .name("lua-script".into())
            .spawn(move || run(source, rx, log, pressed, primary, injector, host))
            .expect("script thread");
        *self.running.lock() = Some(Running { tx, thread, profile_id: name });
    }
}

fn stamp() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() % 86400;
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

/// G HUB key names (`lctrl`, `enter`, `f5`, `spacebar`, `a`, `1`, or a
/// numeric scancode) to key codes.
fn key_for(name: &str) -> Option<u16> {
    let n = name.trim().to_ascii_lowercase();
    match n.as_str() {
        "lctrl" => Some(keymap::KEY_LEFTCTRL),
        "rctrl" => Some(keymap::KEY_RIGHTCTRL),
        "lshift" => Some(keymap::KEY_LEFTSHIFT),
        "rshift" => Some(keymap::KEY_RIGHTSHIFT),
        "lalt" => Some(keymap::KEY_LEFTALT),
        "ralt" => Some(keymap::KEY_RIGHTALT),
        "lgui" => Some(keymap::KEY_LEFTMETA),
        "rgui" => Some(keymap::KEY_RIGHTMETA),
        "spacebar" => Some(keymap::KEY_SPACE),
        "enter" => Some(keymap::KEY_ENTER),
        "escape" => Some(keymap::KEY_ESC),
        "printscreen" => Some(keymap::KEY_SYSRQ),
        "scrolllock" => Some(keymap::KEY_SCROLLLOCK),
        "numlock" => Some(keymap::KEY_NUMLOCK),
        "capslock" => Some(keymap::KEY_CAPSLOCK),
        "numenter" => Some(keymap::KEY_KPENTER),
        "numplus" => Some(keymap::KEY_KPPLUS),
        "numminus" => Some(keymap::KEY_KPMINUS),
        "nummultiply" => Some(keymap::KEY_KPASTERISK),
        "numslash" => Some(keymap::KEY_KPSLASH),
        "numperiod" => Some(keymap::KEY_KPDOT),
        _ => {
            if let Ok(scancode) = n.parse::<u16>() {
                // G HUB scancodes are Linux key codes for the main block.
                return Some(scancode);
            }
            if let Some(d) = n.strip_prefix("num").and_then(|d| d.parse::<u16>().ok()) {
                return keymap::key_code(&format!("kp{d}"));
            }
            keymap::key_code(&n)
        }
    }
}

fn modifier_pressed(name: &str, injector: &Injector) -> bool {
    let n = name.trim().to_ascii_lowercase();
    let codes: Vec<u16> = match n.as_str() {
        "ctrl" => vec![keymap::KEY_LEFTCTRL, keymap::KEY_RIGHTCTRL],
        "shift" => vec![keymap::KEY_LEFTSHIFT, keymap::KEY_RIGHTSHIFT],
        "alt" => vec![keymap::KEY_LEFTALT, keymap::KEY_RIGHTALT],
        other => key_for(other).into_iter().collect(),
    };
    injector.is_held(&codes)
}

#[allow(clippy::too_many_arguments)]
fn run(
    source: String,
    rx: Receiver<ScriptEvent>,
    log: Arc<Mutex<VecDeque<String>>>,
    pressed: Arc<Mutex<u16>>,
    primary: Arc<Mutex<bool>>,
    injector: Arc<Injector>,
    host: Sender<HostRequest>,
) {
    let lua = Lua::new();
    let started = Instant::now();
    let logln = {
        let log = Arc::clone(&log);
        move |line: String| Scripting::push_log(&log, line)
    };

    // -- the G HUB API ---------------------------------------------------------
    let api = lua.globals();
    macro_rules! reg {
        ($name:literal, $f:expr) => {
            if let Ok(f) = lua.create_function($f) {
                let _ = api.set($name, f);
            }
        };
    }

    {
        let logln = logln.clone();
        reg!("OutputLogMessage", move |_, args: mlua::Variadic<Value>| {
            logln(format_args(&args));
            Ok(())
        });
    }
    {
        let logln = logln.clone();
        reg!("OutputDebugMessage", move |_, args: mlua::Variadic<Value>| {
            logln(format!("[debug] {}", format_args(&args)));
            Ok(())
        });
    }
    {
        let log = Arc::clone(&log);
        reg!("ClearLog", move |_, ()| {
            log.lock().clear();
            Ok(())
        });
    }
    reg!("Sleep", |_, ms: u64| {
        std::thread::sleep(Duration::from_millis(ms.min(60_000)));
        Ok(())
    });
    reg!("GetRunningTime", move |_, ()| Ok(started.elapsed().as_millis() as u64));
    reg!("GetDate", |_, fmt: Option<String>| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = now.as_secs();
        let _ = fmt;
        Ok(format!("{} ({})", stamp(), secs))
    });

    // Keys.
    {
        let inj = Arc::clone(&injector);
        let logln = logln.clone();
        reg!("PressKey", move |_, keys: mlua::Variadic<Value>| {
            for k in keys.iter() {
                match key_for(&value_to_string(k)) {
                    Some(c) => {
                        let _ = inj.key(c, true);
                    }
                    None => logln(format!("PressKey: unknown key {}", value_to_string(k))),
                }
            }
            Ok(())
        });
    }
    {
        let inj = Arc::clone(&injector);
        reg!("ReleaseKey", move |_, keys: mlua::Variadic<Value>| {
            for k in keys.iter() {
                if let Some(c) = key_for(&value_to_string(k)) {
                    let _ = inj.key(c, false);
                }
            }
            Ok(())
        });
    }
    {
        let inj = Arc::clone(&injector);
        reg!("PressAndReleaseKey", move |_, keys: mlua::Variadic<Value>| {
            let codes: Vec<u16> = keys.iter().filter_map(|k| key_for(&value_to_string(k))).collect();
            let _ = inj.chord(&codes, true);
            std::thread::sleep(Duration::from_millis(8));
            let _ = inj.chord(&codes, false);
            Ok(())
        });
    }
    {
        let inj = Arc::clone(&injector);
        reg!("IsModifierPressed", move |_, name: String| Ok(modifier_pressed(&name, &inj)));
    }
    reg!("IsKeyLockOn", |_, _name: String| Ok(false));

    // Mouse.
    {
        let inj = Arc::clone(&injector);
        reg!("PressMouseButton", move |_, n: u8| {
            let _ = inj.mouse_button(n, true);
            Ok(())
        });
    }
    {
        let inj = Arc::clone(&injector);
        reg!("ReleaseMouseButton", move |_, n: u8| {
            let _ = inj.mouse_button(n, false);
            Ok(())
        });
    }
    {
        let inj = Arc::clone(&injector);
        reg!("PressAndReleaseMouseButton", move |_, n: u8| {
            let _ = inj.mouse_button(n, true);
            std::thread::sleep(Duration::from_millis(8));
            let _ = inj.mouse_button(n, false);
            Ok(())
        });
    }
    {
        let pressed = Arc::clone(&pressed);
        reg!("IsMouseButtonPressed", move |_, n: u8| Ok(n >= 1 && (*pressed.lock() >> (n - 1)) & 1 == 1));
    }
    {
        let inj = Arc::clone(&injector);
        reg!("MoveMouseRelative", move |_, (x, y): (i32, i32)| {
            let _ = inj.move_rel(x, y);
            Ok(())
        });
    }
    {
        let inj = Arc::clone(&injector);
        reg!("MoveMouseWheel", move |_, clicks: i32| {
            let _ = inj.scroll(clicks, 0);
            Ok(())
        });
    }
    {
        let logln = logln.clone();
        reg!("MoveMouseTo", move |_, (_x, _y): (i32, i32)| {
            logln("MoveMouseTo: absolute pointer positioning is not available on Wayland; use MoveMouseRelative".into());
            Ok(())
        });
    }
    {
        let primary = Arc::clone(&primary);
        reg!("EnablePrimaryMouseButtonEvents", move |_, on: bool| {
            *primary.lock() = on;
            Ok(())
        });
    }

    // Device.
    {
        let host = host.clone();
        reg!("SetMouseDPITableIndex", move |_, i: usize| {
            let _ = host.send(HostRequest::SetDpiIndex(i.saturating_sub(1)));
            Ok(())
        });
    }
    {
        let host = host.clone();
        reg!("SetMouseDPITable", move |_, (table, index): (Vec<u16>, Option<usize>)| {
            let _ = host.send(HostRequest::SetDpiTable(table, index.unwrap_or(1).saturating_sub(1)));
            Ok(())
        });
    }
    {
        let host = host.clone();
        reg!("SetBacklightColor", move |_, (r, g, b): (u8, u8, u8)| {
            let _ = host.send(HostRequest::SetBacklight([r, g, b]));
            Ok(())
        });
    }
    {
        let host = host.clone();
        reg!("PlayMacro", move |_, name: String| {
            let _ = host.send(HostRequest::PlayMacro(name));
            Ok(())
        });
    }
    reg!("AbortMacro", |_, ()| Ok(()));
    reg!("SetMKeyState", |_, _args: mlua::Variadic<Value>| Ok(()));
    reg!("GetMKeyState", |_, _args: mlua::Variadic<Value>| Ok(1));

    // -- load and run ----------------------------------------------------------
    if let Err(e) = lua.load(&source).set_name("script").exec() {
        log::warn!("lua script failed to load: {e}");
        logln(format!("error: {e}"));
        return;
    }
    let call = |event: &str, arg: i64| {
        if let Ok(f) = lua.globals().get::<_, mlua::Function>("OnEvent") {
            if let Err(e) = f.call::<_, ()>((event, arg, "mouse")) {
                log::warn!("lua OnEvent({event}, {arg}) failed: {e}");
                logln(format!("error in OnEvent({event}, {arg}): {e}"));
            }
        }
    };
    call("PROFILE_ACTIVATED", 0);
    for ev in rx {
        match ev {
            ScriptEvent::MouseButton { button, pressed } => {
                call(if pressed { "MOUSE_BUTTON_PRESSED" } else { "MOUSE_BUTTON_RELEASED" }, button as i64);
            }
            ScriptEvent::Stop => break,
        }
    }
    call("PROFILE_DEACTIVATED", 0);
    injector.release_all();
    logln(format!("[{}] script stopped", stamp()));
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_str().unwrap_or("").to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Nil => "nil".into(),
        other => format!("{other:?}"),
    }
}

/// `OutputLogMessage("x = %d\n", x)`-style formatting: a printf-ish subset.
fn format_args(args: &mlua::Variadic<Value>) -> String {
    let Some(first) = args.first() else { return String::new() };
    let fmt = value_to_string(first);
    let mut out = String::new();
    let mut rest = args.iter().skip(1);
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // skip flags/width
        let mut spec = String::new();
        while let Some(&n) = chars.peek() {
            chars.next();
            if n.is_ascii_alphabetic() || n == '%' {
                spec.push(n);
                break;
            }
            spec.push(n);
        }
        match spec.chars().last() {
            Some('%') => out.push('%'),
            Some('d') | Some('i') | Some('u') | Some('s') | Some('f') | Some('g') | Some('x') => {
                out.push_str(&rest.next().map(value_to_string).unwrap_or_default())
            }
            _ => out.push_str(&spec),
        }
    }
    out.trim_end_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_resolve() {
        assert_eq!(key_for("lctrl"), Some(keymap::KEY_LEFTCTRL));
        assert_eq!(key_for("a"), Some(keymap::KEY_A));
        assert_eq!(key_for("spacebar"), Some(keymap::KEY_SPACE));
        assert_eq!(key_for("f5"), Some(keymap::KEY_F1 + 4));
        assert_eq!(key_for("30"), Some(30));
    }

    #[test]
    fn printf_subset() {
        let lua = Lua::new();
        let s = lua.create_string("event %d fired\n").unwrap();
        let args = mlua::Variadic::from_iter([Value::String(s), Value::Integer(7)]);
        assert_eq!(format_args(&args), "event 7 fired");
    }

    #[test]
    fn script_receives_events_and_logs() {
        let sc = Scripting::new();
        let inj = Arc::new(Injector::new());
        let (tx, _rx) = mpsc::channel();
        sc.start("p", r#"
            function OnEvent(event, arg)
              OutputLogMessage("%s %d", event, arg)
            end
        "#, inj, tx);
        std::thread::sleep(Duration::from_millis(100));
        *sc.primary_events.lock() = true;
        sc.mouse_button(3, true);
        std::thread::sleep(Duration::from_millis(100));
        sc.stop();
        let log = sc.log_lines().join("\n");
        assert!(log.contains("PROFILE_ACTIVATED 0"), "{log}");
        assert!(log.contains("MOUSE_BUTTON_PRESSED 4"), "{log}");
        assert!(log.contains("PROFILE_DEACTIVATED 0"), "{log}");
    }
}
