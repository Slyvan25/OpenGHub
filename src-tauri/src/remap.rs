//! Button assignments, both ways G HUB does them.
//!
//! **Software mode** (the app is running): the device is asked to report raw
//! button presses (`0x8110` Mouse Button Spy for mice; the wheel bridge for
//! wheels), any button with a non-mouse assignment is remapped to "no HID
//! action", and OpenGHub performs the assignment itself — keys and chords
//! through a virtual keyboard on uinput, DPI/profile actions through the
//! device, macros by playing their steps.
//!
//! **Onboard mode** (the app is not running): the same assignments are also
//! written into the device's onboard profile where the format can express
//! them (mouse buttons, keys with modifiers, media keys, DPI/profile specials,
//! macros), so a mouse switched back to its onboard profile keeps behaving.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::hidpp::onboard::{Button, MacroStep};
use crate::keymap;
use crate::profiles::{Assignment, MacroDef};
use crate::uinput::{self as ui, VirtualDevice};

/// What pressing a button should do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Hold these key codes (modifiers first) while the button is held.
    Keys(Vec<u16>),
    /// Act as physical mouse button `n` (1 = left … 5 = forward).
    MouseButton(u8),
    DpiUp,
    DpiDown,
    DpiCycle,
    /// Jump to the shift stage while held.
    DpiShift,
    DpiDefault,
    ProfileNext,
    /// Hold to use the G-Shift layer's assignments.
    GShift,
    /// Play these steps once per press.
    Macro(Vec<MacroStep>),
    LockScreen,
    Disabled,
}

impl Action {
    /// Whether the device may keep its own HID behaviour for this button.
    pub fn keeps_device_button(&self) -> Option<u8> {
        match self {
            Action::MouseButton(n) => Some(*n),
            _ => None,
        }
    }

    /// The onboard-profile descriptor for this action, when the format has one.
    pub fn onboard_button(&self, macro_slot: Option<(u8, u16)>) -> Option<Button> {
        Some(match self {
            Action::MouseButton(n) => Button::Mouse { mask: 1u16 << (n.saturating_sub(1)) },
            Action::Keys(codes) => {
                let mut modifiers = 0u8;
                let mut key = None;
                for c in codes {
                    if let Some(bit) = keymap::modifier_bit_for_code(*c) {
                        modifiers |= bit;
                    } else if let Some(usage) = keymap::consumer_usage_for_code(*c) {
                        if codes.len() == 1 {
                            return Some(Button::Consumer { usage });
                        }
                    } else if key.is_none() {
                        key = keymap::usage_for_code(*c);
                    }
                }
                Button::Key { modifiers, usage: key.unwrap_or(0) }
            }
            Action::DpiUp => Button::Special { action: 0x03 },
            Action::DpiDown => Button::Special { action: 0x04 },
            Action::DpiCycle => Button::Special { action: 0x05 },
            Action::DpiDefault => Button::Special { action: 0x06 },
            Action::DpiShift => Button::Special { action: 0x07 },
            Action::ProfileNext => Button::Special { action: 0x0a },
            Action::GShift => Button::Special { action: 0x0b },
            Action::Macro(_) => {
                let (sector, offset) = macro_slot?;
                Button::Macro { sector, offset }
            }
            Action::LockScreen => return None,
            Action::Disabled => Button::Disabled,
        })
    }
}

/// Physical button index (0-based) for an assignment's control id.
/// `button-7` → 6; wheel tilts and keyboard G-keys have no spy index.
pub fn button_index(control: &str) -> Option<u8> {
    let base = control.split(':').next().unwrap_or(control);
    base.strip_prefix("button-").and_then(|n| n.parse::<u8>().ok()).and_then(|n| n.checked_sub(1))
}

/// Interprets one stored assignment.
pub fn action_for(a: &Assignment, macros: &[MacroDef]) -> Option<Action> {
    let value = a.value.trim();
    Some(match a.category.as_str() {
        "action" => match value {
            "dpi-up" => Action::DpiUp,
            "dpi-down" => Action::DpiDown,
            "dpi-cycle" => Action::DpiCycle,
            "dpi-shift" => Action::DpiShift,
            "dpi-default" => Action::DpiDefault,
            "profile-next" => Action::ProfileNext,
            "gshift" | "g-shift" => Action::GShift,
            "mouse-back" => Action::MouseButton(4),
            "mouse-forward" => Action::MouseButton(5),
            "mouse-left" => Action::MouseButton(1),
            "mouse-right" => Action::MouseButton(2),
            "mouse-middle" => Action::MouseButton(3),
            "" | "unassign" | "disabled" => Action::Disabled,
            other => Action::Keys(keymap::parse_chord(other)?.codes),
        },
        "macro" => {
            let def = macros.iter().find(|m| m.id == value)?;
            Action::Macro(def.steps.clone())
        }
        "system" if value == "lock-screen" => Action::LockScreen,
        "command" | "key" | "system" => {
            if value.is_empty() {
                Action::Disabled
            } else {
                Action::Keys(keymap::parse_chord(value)?.codes)
            }
        }
        _ => return None,
    })
}

/// Everything a device needs for software-mode assignments.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Button index → action, for buttons the app handles itself.
    pub actions: HashMap<u8, Action>,
    /// The same for the G-Shift layer.
    pub shift_actions: HashMap<u8, Action>,
    /// The `0x8110` remapping table (16 entries; 0 = no HID action).
    pub remapping: [u8; 16],
    /// The table to load while G-Shift is held.
    pub shift_remapping: [u8; 16],
    /// Buttons currently pressed, from the last spy report (bit n = button n+1).
    pub pressed: u32,
    /// G-Shift is held.
    pub shift_held: bool,
    /// Which layer each pressed button was resolved in, so its release runs
    /// the same action even if the layer changed meanwhile.
    pub pressed_in_shift: u32,
}

impl Plan {
    /// Builds the plan for `button_count` physical buttons.
    pub fn build(assignments: &[Assignment], macros: &[MacroDef], button_count: u8) -> Self {
        let mut plan = Plan::default();
        for i in 0..16u8 {
            plan.remapping[i as usize] = if i < button_count { i + 1 } else { 0 };
        }
        plan.shift_remapping = plan.remapping;
        for a in assignments {
            let shifted = a.control.contains(":gshift");
            let Some(index) = button_index(&a.control) else { continue };
            let Some(action) = action_for(a, macros) else { continue };
            let (table, actions) = if shifted {
                (&mut plan.shift_remapping, &mut plan.shift_actions)
            } else {
                (&mut plan.remapping, &mut plan.actions)
            };
            if index < 16 {
                table[index as usize] = action.keeps_device_button().unwrap_or(0);
            }
            if action.keeps_device_button().is_none() {
                actions.insert(index, action);
            }
        }
        // The G-Shift button itself is silent in both layers.
        for (i, a) in plan.actions.clone() {
            if a == Action::GShift && (i as usize) < 16 {
                plan.shift_remapping[i as usize] = 0;
                plan.shift_actions.insert(i, Action::GShift);
            }
        }
        // Never leave a mouse without a primary click: if nothing emits
        // button 1 any more, button 1 keeps its own job whatever was assigned.
        // (Wheels report more buttons than the 16-entry table; only the table
        // matters here.)
        let n = (button_count as usize).min(plan.remapping.len());
        if n > 0 && !plan.remapping[..n].contains(&1) {
            log::warn!("assignments leave no primary click; keeping button 1 as the primary click");
            plan.remapping[0] = 1;
            plan.actions.remove(&0);
        }
        if n > 0 && !plan.shift_remapping[..n].contains(&1) {
            plan.shift_remapping[0] = 1;
            plan.shift_actions.remove(&0);
        }
        plan
    }

    /// True when at least one button needs the spy.
    pub fn needs_spy(&self) -> bool {
        !self.actions.is_empty() || !self.shift_actions.is_empty()
    }

    pub fn has_gshift(&self) -> bool {
        self.actions.values().any(|a| *a == Action::GShift)
    }

    /// Updates the pressed mask and returns the transitions as (button, pressed).
    pub fn transitions(&mut self, mask: u32) -> Vec<(u8, bool)> {
        let changed = self.pressed ^ mask;
        self.pressed = mask;
        (0..32u8).filter(|b| changed & (1 << b) != 0).map(|b| (b, mask & (1 << b) != 0)).collect()
    }

    /// Resolves an edge to its action, honouring the layer it was pressed in.
    pub fn resolve(&mut self, button: u8, pressed: bool) -> Option<Action> {
        let bit = 1u32 << button;
        let shifted = if pressed {
            if self.shift_held {
                self.pressed_in_shift |= bit;
            } else {
                self.pressed_in_shift &= !bit;
            }
            self.shift_held
        } else {
            self.pressed_in_shift & bit != 0
        };
        let table = if shifted { &self.shift_actions } else { &self.actions };
        table.get(&button).cloned()
    }
}

/// The virtual keyboard everything injects through. One per app.
pub struct Injector {
    device: Mutex<Option<VirtualDevice>>,
    /// Keys currently held by assignments, so a lost release cannot stick.
    held: Mutex<Vec<u16>>,
}

impl Default for Injector {
    fn default() -> Self {
        Self::new()
    }
}

impl Injector {
    pub fn new() -> Self {
        Injector { device: Mutex::new(None), held: Mutex::new(Vec::new()) }
    }

    fn with_device<T>(&self, f: impl FnOnce(&VirtualDevice) -> std::io::Result<T>) -> std::io::Result<T> {
        let mut guard = self.device.lock();
        if guard.is_none() {
            *guard = Some(VirtualDevice::create_keyboard("OpenGHub Virtual Keyboard", crate::hidpp::LOGITECH_VID, 0xc0de)?);
            // Give udev a moment to create the node before the first event.
            std::thread::sleep(Duration::from_millis(80));
        }
        f(guard.as_ref().unwrap())
    }

    pub fn key(&self, code: u16, down: bool) -> std::io::Result<()> {
        {
            let mut held = self.held.lock();
            if down && !held.contains(&code) {
                held.push(code);
            } else if !down {
                held.retain(|c| *c != code);
            }
        }
        self.with_device(|d| d.emit(&[(ui::EV_KEY, code, down as i32)]))
    }

    /// Presses a chord (modifiers first) or releases it in reverse.
    pub fn chord(&self, codes: &[u16], down: bool) -> std::io::Result<()> {
        if down {
            for c in codes {
                self.key(*c, true)?;
                std::thread::sleep(Duration::from_millis(2));
            }
        } else {
            for c in codes.iter().rev() {
                self.key(*c, false)?;
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Ok(())
    }

    pub fn mouse_button(&self, n: u8, down: bool) -> std::io::Result<()> {
        let code = match n {
            1 => ui::BTN_LEFT,
            2 => ui::BTN_RIGHT,
            3 => ui::BTN_MIDDLE,
            4 => ui::BTN_SIDE,
            5 => ui::BTN_EXTRA,
            6 => ui::BTN_FORWARD,
            7 => ui::BTN_BACK,
            _ => ui::BTN_TASK,
        };
        self.with_device(|d| d.emit(&[(ui::EV_KEY, code, down as i32)]))
    }

    /// Relative pointer motion, for scripts.
    pub fn move_rel(&self, dx: i32, dy: i32) -> std::io::Result<()> {
        self.with_device(|d| d.emit(&[(ui::EV_REL, ui::REL_X, dx), (ui::EV_REL, ui::REL_Y, dy)]))
    }

    /// Scroll wheel notches (positive = up) and horizontal scroll.
    pub fn scroll(&self, vertical: i32, horizontal: i32) -> std::io::Result<()> {
        self.with_device(|d| d.emit(&[(ui::EV_REL, ui::REL_WHEEL, vertical), (ui::EV_REL, ui::REL_HWHEEL, horizontal)]))
    }

    /// Plays macro steps on a separate thread so delays never block the pump.
    pub fn play_macro(self: &Arc<Self>, steps: Vec<MacroStep>) {
        let me = Arc::clone(self);
        std::thread::spawn(move || {
            for step in steps {
                let r = match step {
                    MacroStep::KeyDown { usage } => keymap::usage_code(usage).map(|c| me.key(c, true)).unwrap_or(Ok(())),
                    MacroStep::KeyUp { usage } => keymap::usage_code(usage).map(|c| me.key(c, false)).unwrap_or(Ok(())),
                    MacroStep::ModifiersDown { mask } => keymap::modifier_codes(mask).into_iter().try_for_each(|c| me.key(c, true)),
                    MacroStep::ModifiersUp { mask } => keymap::modifier_codes(mask).into_iter().try_for_each(|c| me.key(c, false)),
                    MacroStep::MouseDown { mask } => (1..=8u8).filter(|b| mask & (1 << (b - 1)) != 0).try_for_each(|b| me.mouse_button(b, true)),
                    MacroStep::MouseUp { mask } => (1..=8u8).filter(|b| mask & (1 << (b - 1)) != 0).try_for_each(|b| me.mouse_button(b, false)),
                    MacroStep::Delay { ms } => {
                        std::thread::sleep(Duration::from_millis(ms as u64));
                        Ok(())
                    }
                };
                if let Err(e) = r {
                    log::warn!("macro step failed: {e}");
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
    }

    /// Whether any of `codes` is currently held through this keyboard.
    pub fn is_held(&self, codes: &[u16]) -> bool {
        let held = self.held.lock();
        codes.iter().any(|c| held.contains(c))
    }

    /// Releases anything still held — on device loss or shutdown.
    pub fn release_all(&self) {
        let held: Vec<u16> = self.held.lock().drain(..).collect();
        for c in held {
            let _ = self.with_device(|d| d.emit(&[(ui::EV_KEY, c, 0)]));
        }
    }
}

/// Locks the desktop session, the way G HUB's "Lock computer" system action does.
pub fn lock_screen() {
    let attempts: [&[&str]; 3] = [
        &["loginctl", "lock-session"],
        &["xdg-screensaver", "lock"],
        &["dbus-send", "--session", "--dest=org.freedesktop.ScreenSaver", "/ScreenSaver", "org.freedesktop.ScreenSaver.Lock"],
    ];
    for cmd in attempts {
        if std::process::Command::new(cmd[0]).args(&cmd[1..]).status().map(|s| s.success()).unwrap_or(false) {
            return;
        }
    }
    log::warn!("no screen locker answered");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(control: &str, category: &str, value: &str) -> Assignment {
        Assignment { control: control.into(), category: category.into(), label: String::new(), value: value.into() }
    }

    #[test]
    fn builds_a_remapping_table() {
        let plan = Plan::build(
            &[a("button-4", "command", "ctrl+c"), a("button-5", "action", "mouse-forward"), a("button-6", "action", "dpi-up")],
            &[],
            11,
        );
        assert_eq!(&plan.remapping[..11], &[1, 2, 3, 0, 5, 0, 7, 8, 9, 10, 11]);
        assert_eq!(plan.actions.get(&3), Some(&Action::Keys(vec![keymap::KEY_LEFTCTRL, 46])));
        assert_eq!(plan.actions.get(&5), Some(&Action::DpiUp));
        assert!(plan.actions.get(&4).is_none(), "a mouse-button assignment stays on the device");
        assert!(plan.needs_spy());
    }

    #[test]
    fn primary_click_is_never_lost() {
        let plan = Plan::build(&[a("button-1", "command", "ctrl+c")], &[], 11);
        assert_eq!(plan.remapping[0], 1);
        assert!(plan.actions.get(&0).is_none());
        // Moving the primary click to another button is allowed.
        let plan = Plan::build(&[a("button-1", "command", "ctrl+c"), a("button-4", "action", "mouse-left")], &[], 11);
        assert_eq!(plan.remapping[0], 0);
        assert_eq!(plan.remapping[3], 1);
        assert!(plan.actions.get(&0).is_some());
    }

    #[test]
    fn wheel_button_counts_exceed_the_table() {
        // A G923 has 28 buttons; the plan must not index past the 16-entry table.
        let plan = Plan::build(&[a("button-20", "command", "ctrl+c")], &[], 28);
        assert!(plan.actions.contains_key(&19));
    }

    #[test]
    fn gshift_layer_has_its_own_table() {
        let mut plan = Plan::build(
            &[a("button-6", "action", "gshift"), a("button-4:gshift", "command", "ctrl+c"), a("button-4", "action", "dpi-up")],
            &[],
            11,
        );
        assert_eq!(plan.remapping[5], 0, "the shift button is silent");
        assert_eq!(plan.shift_remapping[5], 0);
        assert_eq!(plan.remapping[3], 0);
        assert_eq!(plan.shift_remapping[3], 0);
        assert!(plan.has_gshift());
        // Base layer press resolves to DPI up; with shift held, to the chord.
        assert_eq!(plan.resolve(3, true), Some(Action::DpiUp));
        assert_eq!(plan.resolve(3, false), Some(Action::DpiUp));
        plan.shift_held = true;
        assert_eq!(plan.resolve(3, true), Some(Action::Keys(vec![keymap::KEY_LEFTCTRL, 46])));
        plan.shift_held = false;
        assert_eq!(plan.resolve(3, false), Some(Action::Keys(vec![keymap::KEY_LEFTCTRL, 46])), "release follows the press layer");
    }

    #[test]
    fn transitions_report_edges() {
        let mut plan = Plan::default();
        assert_eq!(plan.transitions(0b0001), vec![(0, true)]);
        assert_eq!(plan.transitions(0b0011), vec![(1, true)]);
        assert_eq!(plan.transitions(0b0010), vec![(0, false)]);
        assert!(plan.transitions(0b0010).is_empty());
    }

    #[test]
    fn onboard_descriptors() {
        let keys = Action::Keys(vec![keymap::KEY_LEFTCTRL, 46]);
        assert_eq!(keys.onboard_button(None), Some(Button::Key { modifiers: 0x01, usage: 0x06 }));
        assert_eq!(Action::Keys(vec![keymap::KEY_VOLUMEUP]).onboard_button(None), Some(Button::Consumer { usage: 0xe9 }));
        assert_eq!(Action::MouseButton(5).onboard_button(None), Some(Button::Mouse { mask: 0x10 }));
        assert_eq!(Action::Macro(vec![]).onboard_button(Some((9, 0))), Some(Button::Macro { sector: 9, offset: 0 }));
        assert_eq!(Action::Macro(vec![]).onboard_button(None), None);
    }
}
