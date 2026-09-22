# Racing wheels

The G923 and its relatives: the classic command channel, the built-in force-feedback driver, pedals, TrueForce and Proton.

## Racing wheels and the built-in force-feedback driver

Verified on a G923 Racing Wheel for PlayStation 4 and PC (046d:c267). Older wheels and the
G923 in PlayStation mode do **not** speak HID++ — G HUB drives them through a raw-HID class
(`hidio_g923_ps4`, `hidio_g29`) with the classic 7-byte commands the `new-lg4ff` kernel driver
documents. OpenGHub sends the same bytes from `src-tauri/src/wheel/`:

| command | bytes |
| --- | --- |
| operating range | `f8 81 lo hi` |
| RPM LEDs (5-bit mask) | `f8 12 mask` |
| centering spring | `fe 0d k k mag` then `14` (off: `f5`) |
| force slots 0–3 | see `wheel/ffb.rs` |

On the G923 (PS mode) they travel as output report `0x30` on the joystick interface; native
wheels use the id-less report. The steering angle is a 16-bit field the kernel doesn't map to
evdev, so inputs are read from hidraw too (`u16` at bytes 43–44, pedals at 45/47/49). The
Driving Force Shifter plugged into the wheel arrives in byte 51 — bits 0–5 are gears 1–6, bit 7
is reverse — and is exposed as buttons 21–26 and 28 of the virtual wheel (`BTN_TRIGGER_HAPPY+4…`);
the Steering Wheel page shows the gear next to the pedal bars. The wheel's own extras are in
byte 54 (Enter, dial left, dial right, −, +) and become buttons 14–18.

For assignments the driver re-numbers all of that the way G HUB does (its `gN` slots, read off
the depot's marker positions): 1 ✕, 2 □, 3 ○, 4 △, 5/6 right/left paddle, 7 R2, 8 L2, 9 Share,
10 Options, 11 R3, 12 L3, 13–18 gears, 19 reverse, 20 +, 21 −, 22/23 dial right/left, 24 Enter,
25 PS, 26–33 D-pad. The Assignments page shows the wheel, the shifter and the pedals as separate
views with those names; the pedals are axes and are shaped on the Pedals tab instead.

**Force feedback without a kernel module.** Games upload effects to an event device; only a
driver can answer. OpenGHub is that driver, in userspace: it creates a virtual wheel on
`/dev/uinput` with FF capability (`OpenGHub G923 Racing Wheel`), mirrors the real wheel's axes
and buttons onto it, grabs the real evdev node so games see one wheel, and turns every
uploaded effect — constant, ramp, periodic, spring, damper, friction, inertia, with envelopes —
into the wheel's four force slots on a 2 ms tick. The effect engine is a port of new-lg4ff's.
Sensitivity and the "calibrate wheel centre" offset are applied to the virtual axis, so they
are real on Linux rather than Windows-driver-only. `/dev/uinput` access comes from the udev
rule in `packaging/` (Steam's rule grants it too).

TrueForce is different: it is an audio/physics stream that the game's Logitech SDK writes
straight to the wheel's third HID interface (usage page `0xFFFD`), not through G HUB. Proton
passes that hidraw interface through, so TrueForce works on Linux — confirmed in Assetto
Corsa — and OpenGHub's driver leaves that interface alone. G HUB's *Torque* / *Audio Effects*
sliders are not wheel settings either: G HUB hands them to Logitech's `trueforce_manager`
service over a named pipe, and that service scales the stream in software (`GAIN_TF_IN_SW` /
`GAIN_KF_IN_SW` in the binary) before it reaches the wheel. Without that service the game's own
TrueForce gain is the only one applied, so here *Torque* is wired to the driver's force-feedback
gain and *Audio Effects* is only stored with the profile. Making it real would mean proxying
the `0xFFFD` stream and scaling it, which needs the (undocumented) packet format first.

**What the PS-mode wheel's HID++ interface offers.** Interface 1 is a HID++ 4.2 endpoint
(short/long reports `0x10`/`0x11`, device index `0xFF`). Its feature set has the mouse-style
basics plus `0x8120` gaming attachments (reports pedals and shifter as connected), `0x8127`
dual clutch, `0x807a` RPM indicator, `0x80a3` axis response curve (four axes X/Y/Z/Rz, each
`getSensitivity` → `(100, 50)`, `setSensitivity(axis, a, b)` with both ≤ 100) and `0x80d0`
combined pedals — but not the Xbox/PC edition's `0x8123`/`0x8131`/`0x8136`/`0x8138`/`0x8139`.
Tested on hardware: the wheel stores the response-curve and combined-pedals values but the
PS-mode input report is unchanged by them (full travel still reads 100 %, a held pedal reads
the same under every setting), so they are consumed by Logitech's Windows driver rather than
by the firmware. OpenGHub therefore keeps pedal curves on its virtual wheel.

Two hardware gotchas that cost an evening: the wheel must be on **mains power** (unpowered it
enumerates but resets constantly), and on one of the AMD xHCI controllers here the joystick
interface's interrupt-OUT endpoint never comes up (`xhci_hcd: WARN urb submitted to disabled
ep`, every write fails `ENOENT`). Another USB port fixed it; the alternative is
`options usbhid quirks=0x046d:0xc267:0x00040000` in `/etc/modprobe.d/`, which routes output
reports over the control endpoint.

**Pedals.** The third rail item is G HUB's pedal panel: per pedal a sensitivity preset (Low /
Medium / High, or the slider behind them), dead zones at both ends and inversion, plus
"combined pedals" (brake and accelerator on one axis). All of it is applied on the virtual
wheel's axes in `wheel::apply_settings`, so it works in any game that reads the OpenGHub wheel.

**Proton games.** Proton routes Logitech wheels over hidraw by default (that is what lets the
TrueForce DLL reach the wheel), and drops the SDL duplicate of anything it hidraw-routes — so a
Proton game sees the raw wheel and not the OpenGHub one. Over hidraw Wine can only do
DirectInput force feedback for HID *Physical Interface* devices, and the PS-mode G923 has no
PID descriptor: the game's FF (self-aligning torque, curbs, its gain slider) goes nowhere, and
what you feel is TrueForce plus OpenGHub's centering spring. Two launch options fix that:

| launch option | result |
| --- | --- |
| `PROTON_DISABLE_HIDRAW=0x046d/0xc266 %command%` | the OpenGHub wheel (it presents as the native G923, `c266`) comes through SDL with full FF, the real wheel stays on hidraw for TrueForce — bind axes to *OpenGHub G923 Racing Wheel* |
| `PROTON_PREFER_SDL=1 %command%` | everything through the OpenGHub driver (calibration and sensitivity apply); no TrueForce |

Keep the game's steer lock equal to OpenGHub's operating range — a game on the hidraw path
cannot set the wheel's range itself.

**Games without wheel support (Rocket League and friends).** *Gamepad mode* on the Steering
Wheel page adds a second virtual device next to the wheel: an Xbox 360 controller
(`045e:028e`, which SDL and Proton map with no setup) with steering on the left stick,
accelerator and brake on the right and left triggers, ✕ ○ □ △ as A B X Y, the paddles (or
L2 / R2) as the bumpers, Share / Options / PS as Back / Start / Guide, L3 / R3 as the stick
clicks and the D-pad as the D-pad. It has no force feedback of its own; the wheel's spring and
the driver keep running. The community repository carries a *Rocket League: Wheel as
controller* profile for the G923 that turns this on with a 360° range and a firm spring, bound
to the game so it activates when Rocket League runs.

G HUB's HID++ wheel features (`0x8123` force feedback, `0x812c` centre calibration, `0x8131`
spring, `0x8138` range, `0x8139` TrueForce) apply to the G920 / G923 Xbox editions; those are
recognised but not driven yet. `cargo run --example wheeldrive -- 30` runs the driver
standalone for testing with `fftest`.
