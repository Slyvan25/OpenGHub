# OpenGHub

An open source, Linux-native reimplementation of Logitech G HUB, built with **Tauri v2**,
**Svelte 5** and **Rust**. It talks to Logitech peripherals directly over **HID++ 2.0** — no
kernel module, no proprietary daemon.

> Not affiliated with or endorsed by Logitech. "G HUB", "LIGHTSPEED" and "LIGHTSYNC" are
> trademarks of Logitech.

## What works

| Area | Status |
| --- | --- |
| Device discovery (direct USB, Bluetooth, Unifying/LIGHTSPEED receivers) | ✅ |
| Battery level & charge state (`0x1000`, `0x1001`, `0x1004`) | ✅ |
| Adjustable DPI, incl. stage lists and range sensors (`0x2201`) | ✅ |
| Report/polling rate, 125 Hz – 8 kHz (`0x8060`, `0x8061`) | ✅ |
| Feature enumeration & diagnostics (`0x0001`) | ✅ |
| Profiles, per-device settings, persistence | ✅ |
| Per-game profiles: detection, auto-switch, game command sets | ✅ via Logitech's public application database |
| Community profiles: browse, preview, import, share | ✅ from an open Git repository ([openghub-community](https://github.com/Slyvan25/openghub-community)) |
| Games library & launcher | ✅ Steam, Heroic (Epic / GOG), Lutris and manually added executables |
| Racing wheels: range, centering spring, RPM LEDs, centre calibration | ✅ G923 (PS4/PC), G29, G27, G25, DFGT, DFP, MOMO — classic command channel |
| Force feedback for games (wheels) | ✅ OpenGHub's own userspace driver on uinput, no kernel module |
| TrueForce | ✅ works: the game streams it straight to the wheel (confirmed in Assetto Corsa under Proton); G HUB's gain sliders need HID++ `0x8139`, so on PS-mode wheels the in-game TrueForce settings apply |
| Device renders, thumbnails, exact zone & button geometry | ✅ imported from a G HUB install, or fetched from Logitech's CDN per device |
| RGB lighting (`0x8070`) | ✅ per zone: off / fixed / breathing / colour cycle |
| Screen sampler & audio visualizer | ✅ software effects streamed to each zone (desktop portal + PipeWire / PulseAudio monitor) |
| Macros on onboard memory (`0x8100`) | ✅ recorded, written to device flash, backed up first |
| Button remapping (non-macro) | ⚠️ UI and profile storage only; not written to the device yet |
| On-board memory mode (`0x8100`) | ✅ toggle per device; DPI ladder, report rate, lighting and button table written into the onboard profile |
| Import from G HUB `settings.db` | ✅ profiles, DPI/shift, report rate, per-zone lighting, button assignments |
| Lua scripting (G HUB API: `OnEvent`, `PressKey`, `OutputLogMessage`, …) | ✅ per profile, Lua 5.4 in-process; scripts written for G HUB load unchanged |

## Requirements

- Rust 1.77+, Node 20+
- WebKitGTK 4.1, libudev (`webkit2gtk4.1-devel libsoup3-devel libudev-devel` on Fedora,
  `libwebkit2gtk-4.1-dev libudev-dev` on Debian/Ubuntu)

`hidapi` is compiled from the vendored hidraw backend, so no system hidapi package is needed.

## Device permissions

HID++ needs read/write access to the device's `hidraw` node, which desktop users do not get
by default — on most distributions `/dev/hidraw*` is `root:root 0600`.

```sh
sudo cp packaging/70-openghub.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger --action=add --subsystem-match=hidraw
# then unplug and reconnect the device (or its receiver)
```

The `70-` prefix is load-bearing. `TAG+="uaccess"` grants nothing by itself — systemd's
`/usr/lib/udev/rules.d/73-seat-late.rules` is what applies the ACL, and it does so with
`TAG=="uaccess", RUN{builtin}+="uaccess"`. udev evaluates rule files in lexical order, so a
tag set by a `99-` file lands *after* 73 has already tested for it: `udevadm info` shows
`CURRENT_TAGS=:uaccess:` and yet the node stays `root:root 0600`. Anything below 73 works.

Until that is done OpenGHub falls back to demo devices, and the dashboard distinguishes the
cases rather than showing one vague message:

- **hardware present but unopenable** — names the devices it can see and prints the exact
  commands above (this is the usual first-run state);
- **no Logitech hardware** — nothing on the bus;
- **found but silent** — a wireless device is off or asleep;
- **hidapi unavailable** — the HID backend itself would not start.

## Running

```sh
npm install
npm run tauri dev      # desktop app against real hardware
npm run dev            # browser only, served by the mock backend in src/lib/mock.ts
npm run tauri build    # produces .deb and .rpm in src-tauri/target/release/bundle
```

AppImage is not built by default: its `linuxdeploy` step downloads helper binaries at bundle
time and fails on some hosts. Build it explicitly with
`npm run tauri build -- --bundles appimage` if you want one.

Note that plain `cargo build` produces a *dev* binary that expects the Vite dev server on port
1420 — use `npm run tauri dev` or `npm run tauri build` instead.

## Tests

```sh
cd src-tauri && cargo test    # protocol framing, DPI list parsing, config round-trips
npm run check                 # svelte-check + TypeScript
```

### Onboard profiles vs host control

A device running its **onboard profile** owns its own settings, and that shows up in two
different disguises:

- `setReportRate` and `setSensorDpi` are refused with **"invalid argument (0x02)"**, which
  reads like a bad parameter but means "not yours to change";
- `0x8070` lighting writes are **accepted and then silently ignored**.

Host mode (`0x8100` fn 1, value `0x02`) hands control to software. It is **not persistent** —
the device reverts to its onboard profile on reconnect — so it cannot be taken once at startup.

OpenGHub therefore takes host mode lazily: lighting always claims it up front, because a
silently-dropped write gives nothing to react to, while DPI and report rate only claim it after
a write is actually refused. A device that is happy in onboard mode is left alone.

### Community profiles

G HUB's Community tab is a closed Logitech service. OpenGHub's is an open Git repository —
[`Slyvan25/openghub-community`](https://github.com/Slyvan25/openghub-community) by default,
changeable in Settings — with one JSON file per profile and an `index.json` that CI
regenerates on every push. The client needs nothing but HTTPS to `raw.githubusercontent.com`.

- **Browse** the index under Community, filtered to your connected devices by product id.
- **Preview** shows everything a profile contains before import — every macro step included,
  because a macro is a keystroke sequence and nobody should import one blind.
- **Import** creates a new local profile applied to the matching device(s). Nothing touches
  hardware until the user applies it through the normal screens.
- **Share** (Profiles → ⋮ → Share…) exports a profile for one device as the same JSON, ready
  for a pull request. Contributions are CC0.

`src-tauri/src/community.rs` holds the format (`format: 1`), validation, and the fetcher;
`cargo run --example community -- <repo-url>` exercises it against any repository.

### The Games tab

G HUB's Games tab is a launcher: every installed game as a poster tile, filtered by store.
OpenGHub reads the launchers that exist on Linux, all read-only:

- **Steam** — `steamapps/libraryfolders.vdf` lists the library folders and every
  `appmanifest_*.acf` in them is an installed app. Last-played and playtime come from
  `userdata/<id>/config/localconfig.vdf`. Portrait covers are Steam's own
  `appcache/librarycache/<appid>/library_600x900.jpg`, with the public CDN as fallback.
  Proton, the Steam Linux Runtime and Steamworks redistributables are hidden.
- **Epic Games / GOG** — through Heroic's `store_cache/{legendary,gog}_library.json`.
- **Lutris** — `lutris --list-games --installed --json`; entries whose runner is Steam are
  skipped because Steam already lists them.
- **Manually installed** — any executable, added with **+** or under *Manage*; stored in the
  config with an optional cover.

Covers are copied into `~/.local/share/openghub/games/` so the webview can load them without
the asset protocol being opened to every launcher's data directory. Games are matched to
Logitech's application database by Steam app id (or exact name), which is what links a tile to
its profile: *Profile* jumps to it, *Add profile* creates and binds one. Launching goes through
the owning launcher (`steam://rungameid/…`, `heroic://launch/…`, `lutris:rungame/…`), so
profile auto-switching then works the usual way. `cargo run --example games` dumps the scan.

### Racing wheels and the built-in force-feedback driver

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
Corsa — and OpenGHub's driver leaves that interface alone. What G HUB's Torque / Audio Effects
sliders do on Windows is scale the stream via HID++ `0x8139`, which the PS-mode G923 does not
expose; here *Torque* is wired to the driver's force-feedback gain and *Audio Effects* is kept
per profile, while the game's own TrueForce settings set the stream's strength.

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

G HUB's HID++ wheel features (`0x8123` force feedback, `0x812c` centre calibration, `0x8131`
spring, `0x8138` range, `0x8139` TrueForce) apply to the G920 / G923 Xbox editions; those are
recognised but not driven yet. `cargo run --example wheeldrive -- 30` runs the driver
standalone for testing with `fftest`.

### Per-game profiles and the application database

G HUB keys profiles on the running game and shows each game's own keybinds in the COMMANDS
tab. Both come from a **public, unauthenticated** channel that G HUB itself uses — verified
live, no credentials involved:

```
https://gamesapps-assets.ghub.logitechg.com/v1/channels/public/update_apps.json
  → { version, applicationPath }
https://gamesapps-assets.ghub.logitechg.com/assets/<version>/applications.json
  → 846 applications: detection rules, command sets, category colours
https://gamesapps-assets.ghub.logitechg.com/images/<hash>/<name>_poster.jpg
  → posters, loaded at runtime (never bundled)
```

`src-tauri/src/apps.rs` fetches this on startup, caches it under
`$XDG_DATA_HOME/openghub/apps/`, and refreshes after 24 hours. `cargo run --example appsdb`
exercises the whole chain.

**Detection on Linux** reads `/proc` rather than asking the compositor which window is
focused — Wayland will not say. 809 of the 846 entries carry a Steam app id, and Steam exports
`SteamAppId` into every game process it launches, so that is authoritative. Executable basenames
from the Windows-oriented rules serve as a weaker fallback for Proton, Lutris and Heroic. A
watcher polls every 3 s; when a bound game starts, its profile activates, and when it stops,
Desktop takes over. **Settings → "Switch profiles with games"** turns this off.

Note that none of this is Logitech's *device* depot channel, which is a separate, internal,
token-gated service (see the section on device artwork).

### Onboard memory and macros

Macros are written into the device's own flash, so they keep working with OpenGHub closed.
The format was established by reading a real G502 rather than from documentation:

- Memory is **sectors of 255 bytes** — not 256. A 16-byte read or write may not cross the end,
  so the last access of a sector is end-aligned and overlaps the previous one. `memoryAddrWrite`
  must declare *exactly* the sector size; rounding up to 256 is rejected as "invalid argument".
- Every sector ends with a **CRC-16/CCITT** (poly `0x1021`, init `0xFFFF`) over all preceding
  bytes, big-endian. `onboard::seal` applies it; nothing is written without it.
- Sector 0 is a **profile directory** of 4-byte entries (`sector`, `enabled`), terminated by
  `ffff`. Each enabled entry points at a profile sector.
- A profile holds the report rate, a five-step DPI ladder (**little-endian**, unlike HID++
  messages) and a button table at offset 32 of 4-byte descriptors: `80` mouse button + mask,
  `90` special action, `00` macro pointer (sector + offset), `ff` disabled.
- Macros live in their own sector, taken from the top of memory downwards so they can never
  collide with profiles. OpenGHub rebuilds that sector from its own config on every write, so
  the device never accumulates orphans.

**A backup is taken automatically before every write** to
`$XDG_DATA_HOME/openghub/backups/<pid>-<timestamp>.json`, containing every sector verbatim.
`restore_onboard_memory` puts one back. Use `cargo run --example onboard -- --backup` to take
one by hand.

The macro editor follows G HUB's three steps (name → type → build) and its four types — *no
repeat*, *repeat while holding*, *toggle*, *sequence* with on-press / while-holding / on-release
sections. The onboard macro format only has a flat keystroke list, so **the type is kept in the
profile for editing but the device always receives the flattened sequence**. Likewise G HUB's
*Action*, *Launch application* and *System* entries run on the host and are shown disabled;
recorded keystrokes, typed ASCII text and delays are what a device can play. "Use standard
delays" replaces the recorded timing with a fixed gap when the macro is flattened.

### Device settings

The gear tab carries G HUB's per-device settings, kept outside the profiles
(`settings.deviceSettings[deviceId]`): **firmware version** from `0x0003 getFwInfo` (main
`MPM17.00_B0008` and bootloader `BOT92.00_B0008` on the G502), **power management** — the
auto-sleep and inactivity-lighting timeouts written into the onboard profile's header words at
offsets 28/30 (seconds, big-endian, `0xffff` = firmware default) — a software **low-battery
mode** that dims the profile's lighting below a threshold and restores it once charged past it,
and the **left-handed** layout, which swaps the two clicks through the assignment plan (and
therefore the onboard table too).

### G-Shift

Assign *Actions → G-Shift* to a button and flip the DEFAULT / G-SHIFT switch under the render
to bind the second layer (stored as `button-N:gshift`). In software mode the button pump keeps
a second `0x8110` remapping table and loads it while G-Shift is held, so the device's own
actions follow the layer too; each press remembers the layer it was resolved in so a release
never sticks a key. In onboard mode the layer is written to the profile's second button table
at offset 96 (unassigned = `ff`, as the factory table has it), with the shift button itself as
special `0x0b`.

### Lua scripting

*Profiles → ⋮ → Scripting…* opens the same editor-over-console window G HUB has. A profile's
script runs on its own thread in a Lua 5.4 VM (`mlua`, vendored — nothing to install) for as
long as the profile is active; *Save & Run* (Ctrl+S) restarts it, *Stop* removes it, and
switching profile stops one script and starts the next. The API is G HUB's, so existing
scripts work as they are:

- `OnEvent(event, arg, family)` receives `PROFILE_ACTIVATED`, `MOUSE_BUTTON_PRESSED` /
  `MOUSE_BUTTON_RELEASED` (arg = button number, 1-based; button 1 only after
  `EnablePrimaryMouseButtonEvents(true)`) and `PROFILE_DEACTIVATED`.
- Input: `PressKey`, `ReleaseKey`, `PressAndReleaseKey` (names as in G HUB — `lctrl`, `f5`,
  `spacebar`, `num7`, or a scancode), `PressMouseButton` … `PressAndReleaseMouseButton`,
  `MoveMouseRelative`, `MoveMouseWheel`, `IsMouseButtonPressed`, `IsModifierPressed`. All of
  it goes through the same virtual keyboard the assignments use, so it works on Wayland.
  `MoveMouseTo` has no Wayland equivalent and logs instead.
- Device: `SetMouseDPITableIndex`, `SetMouseDPITable`, `SetBacklightColor`, `PlayMacro`
  (a macro of the active profile, by name).
- Console: `OutputLogMessage` (printf subset), `OutputDebugMessage`, `ClearLog`, `Sleep`,
  `GetRunningTime`, `GetDate`. `SetMKeyState` / `GetMKeyState` / `IsKeyLockOn` are accepted
  and do nothing.

Button events come from the `0x8110` button spy, which is switched on for every mouse while a
script runs even when nothing is assigned. A device in on-board memory mode keeps its buttons
to itself, so the console says so instead of silently seeing nothing.

### On-board memory mode

G HUB's card button. Off (default), OpenGHub drives the mouse live: host mode, software
assignments through the button spy, software lighting effects. On, the mouse runs the profile
in its own flash and OpenGHub writes into that profile instead — the DPI ladder / default /
shift stage and report period in the header (bytes 0–12), the LED blocks at offset 208 (and
their copy at 230; each is the effect id plus the same 10-byte union `0x8070` takes — the
factory sector read `03 … 1f 40` = Cycle 8000 ms), and the button table. Every write is
preceded by a backup and followed by a host→onboard round trip so the device reloads the
profile. The choice is remembered per device (`settings.onboardModeDevices`), re-applied after
a rescan, and the profile is rewritten on every profile switch. Software effects cannot live
on the device; they fall back to a fixed colour there.

### Importing your G HUB profiles

Settings → *Import profiles from settings.db* reads `%LOCALAPPDATA%\LGHUB\settings.db`
(one SQLite table whose latest row is a JSON document). Profiles map to ours by Logitech
application id (the Desktop profile to *Desktop: Default*); per device, slot ids such as
`g502wireless_mouse_settings`, `…_lighting_setting_firmware` and `…_g7_m1[_shifted]` become the
DPI table with shift stage, report rate, per-zone lighting and `button-N[:gshift]`
assignments. Built-in mouse/device functions are synthetic card ids
(`0f82f693-…-TTNN00000000`) decoded from the G502's factory table; keystroke cards carry a HID
usage plus modifier usages; integration actions (OBS, Discord, Overwolf) have no Linux
equivalent and are listed as skipped.

### Button remapping is device-specific

There are two mechanisms, and which one applies depends on the hardware:

- **`0x1b04` (Special Keys & Buttons)** — remap at runtime, no flash writes. Common on
  keyboards and older mice.
- **`0x8100` (Onboard Profiles)** — the device stores button descriptors and macros in its own
  flash. This is what a G502 LIGHTSPEED uses; it does **not** expose `0x1b04` at all.

Do not assume `0x1b04`: run `cargo run --example onboard` (read-only) to see which a device has.
A G502 reports 5 profiles, 11 buttons, 16 sectors, profile format 3, macro format 1, and its
button descriptors are legible 4-byte records — `80 01 00 01` is "mouse button 1", `90 07 ff 00`
is a special action.

### Per-zone lighting

Devices expose their zones through `getZoneInfo`, including a *location* code that OpenGHub
turns into the tab names G HUB uses — a G502 reports `0x0001` and `0x0002`, shown as **PRIMARY**
and **LOGO**. Each zone keeps its own effect and colour in the profile, and "sync lighting
zones" copies the active one across.

### Where G HUB's artwork really comes from

Reverse-engineered from a G HUB 39.1 `C:\ProgramData\LGHUB` tree, a Wireshark capture of a
first start, and cross-checked against the device's own HID++ replies. The whole chain:

1. **The depository** — `ProgramData\LGHUB\current.json` — lists every depot of a build
   (757 in build 824196, 322 of them devices) with a stable UUID URL, a SHA-256 `mac`, a size
   and an RSA signature:
   ```
   https://updates.ghub.logitechg.com/depots/<uuid>/<name>.depot
   ```
   This file is the one gated piece: it arrives via a bootstrap on `util.logitech.io`, which
   refuses plain requests. OpenGHub therefore imports it from an existing installation.
2. **Device depots are public and unencrypted.** Their `cipherSuite`/`iv`/`key` are empty;
   only some application depots are encrypted. `curl` on the URL above returns the same bytes
   G HUB has on disk — verified byte-for-byte.
3. **The `.depot` container** is `magic 10 01 17 20 | json_len (u32 LE) | json | [len (u32 LE) | bytes]*`,
   the JSON naming files in order. No compression. `src-tauri/src/depot.rs` unpacks it.
4. **The device database** — `depots/<build>/core/data/devices/devices_NNNN.json` — maps
   `046d_<pid>` to a `modelId` (`g502_wireless`), its depot, display name, thumbnail and
   lighting `typeMap` (`ZONE_PRIMARY → PRIMARY`, `ZONE_BRANDING → LOGO`). Files beginning
   `21 05 21 20` are encrypted, presumably unannounced hardware, and are skipped.
5. **`metadata.json` inside a device depot** gives, in image pixels, a **rectangle per
   lighting zone** and a **marker + label position per button**, for a front and a side view.
   That is exactly what G HUB draws — and it matches what the device reports over HID++
   (`0x0003` model ids, `0x8070` zone locations) in every field checked.

**Settings → G HUB data → "Import from G HUB folder"** reads such a tree, writes each device's
`<pid>.png`, `<pid>-side.png`, `<pid>-thumb.png` and `<pid>.layout.json` into the artwork
directory, and caches the depository. From then on any device OpenGHub sees for the first
time is fetched from Logitech's CDN automatically (SHA-256 verified), the way G HUB does —
switchable off with "Fetch artwork automatically". `cargo run --example ghubimport` does the
same from the command line.

Nothing from Logitech is bundled in this repository. The user supplies the depository from
their own installation; OpenGHub then does what the official client does.

### Zone positions without a depot

**A device cannot tell you where its zones are.** This was confirmed against G HUB's own
device schema, recovered from the protobuf descriptors embedded in `lghub_agent.exe`:

```protobuf
message MaskedZone {
  string id = 1;
  repeated string slot_ids = 2;
  string display_icon_key = 3;   // tab icon, suffixed `_on` / `_off`
  string render_icon_key = 4;    // image composited over the device render
  bool enabled = 5;
}
MaskedZones maskable_zones = 6;  // in Capabilities
```

No coordinates anywhere: each zone points at an **image**, resolved through
`manifest.Resource { key, src }` to a file in a per-device depot the app downloads at runtime.
The frontend then uses that image as a CSS mask (`-webkit-mask: url(...) no-repeat center` with
`mask-size: contain`) and fills it with the live colour.

OpenGHub supports the same thing: drop `<product-id>-zone<N>.png` next to the base render and
its alpha channel is used as a mask, giving pixel-accurate lighting. Without a mask it falls
back to a positioned glow: `getZoneInfo` returns a *location code* —
`0x0001` "Primary", `0x0002` "Logo" — which is semantic, not positional, and what it means
physically differs per model: a G502's Primary zone is the DPI indicator on its left flank, and
it has no scroll-wheel lighting at all. G HUB only knows because Logitech ships per-device
asset packs with zone masks.

So the position is data, resolved in layers (`src/lib/zones.ts`, `DeviceArt.svelte`):

0. **an imported depot layout** (`<pid>.layout.json`) — G HUB's own rectangles, exact;
1. **a per-zone mask image**, if one exists;
2. **the user's dragged position** for that device, stored in the profile;
3. **a per-product entry**, keyed by product id;
4. **a category fallback**, deliberately vague rather than claiming a component the device may
   not have.

Layers 2–4 are approximations, and the artwork being user-supplied makes that unavoidable: a
different photo of the same mouse puts everything somewhere else. **LIGHTSYNC → "Position zones on artwork"**
turns on drag handles, and **"Identify"** lights one zone at a time on the real device so you
can see which tab is which light.

The glow itself is composited over the photo with `mix-blend-mode: screen`, one blob per lit
zone, so light is added rather than painted over the product.

### Screen sampler and audio visualizer

G HUB's two software effects run in `src-tauri/src/lightsync.rs`. A source produces a colour
per zone ~20 times a second, and each is pushed as a *fixed* colour with `persist = false` —
RAM only, so the flash is untouched and the device falls back to its stored effect when the
app stops. Writes are skipped when the colour barely changed, so a static screen costs nothing.

- **Screen** — the desktop portal's ScreenCast (`ashpd`) provides a PipeWire stream; the
  portal asks once which monitor to share and hands back a restore token that is kept in the
  settings, so it never asks again. A `gst-launch-1.0 pipewiresrc … videoscale` helper turns
  the stream into 64×36 RGB thumbnails on a pipe (no PipeWire headers needed at build time), and
  each zone averages the region it is mapped to — drag the region on the Lighting page, or pick
  Full / Left / Right / Top / Bottom / Centre.
- **Audio** — `parec --device=@DEFAULT_MONITOR@` records whatever plays on the default output;
  a 1024-point FFT with a Hann window gives RMS and three bands (40–250 / 250–2000 /
  2000–9000 Hz), each with its own peak-following gain so quiet and loud sources both use the
  whole range. The zone colour is the bass / mids / treble colours blended by band energy and
  dimmed by level.

Both need the helpers on `PATH` (`gst-launch-1.0` with the pipewire plugin, `parec`); a missing
one shows up as a message under the effect. `cargo run --example lightsync -- audio|screen`
exercises the sources on their own.

### Lighting notes

Two things about `0x8070` that cost real debugging time:

- Byte 1 of `setZoneEffect` is the **zone-local effect index**, not the global effect id. A
  G502 advertises ids `0x00, 0x01, 0x03, 0x0a` at indices `0..=3` and accepts only `0..=3`.
  Sending the id `0x0a` gets "invalid argument"; sending `0x03` quietly selects index 3
  (breathing) instead of cycle. OpenGHub reads each zone's effect list and translates.
- A device running its **onboard profile owns its own LEDs** — `0x8070` writes are accepted and
  then ignored. OpenGHub switches to host mode via `0x8100` first, as G HUB does. Use
  `cargo run --example lighting` to dump what a zone actually supports.

### Probing real hardware

`probe` is a read-only diagnostic that dumps everything OpenGHub can read from the devices on
the machine — endpoints, whether each one opens, and the full HID++ feature list. It never
writes to a device. Run it first when something is not detected, or when adding support for
hardware that is not in the registry:

```sh
cd src-tauri && cargo run --example probe      # devices, features, DPI, rate, battery
cd src-tauri && cargo run --example lighting   # per-zone lighting capabilities
```

Stop the app first, so the two are not driving the same device at once.

### A dev-server race, and why `vite.config.js` warms the client graph

With Vite 8, SvelteKit 2 and vite-plugin-svelte 7, components would intermittently render
**unstyled** in `npm run tauri dev`, with this in the log:

```
[vite-plugin-svelte:load] failed to load virtual css module …/Foo.svelte?svelte&type=style&lang.css
```

The plugin serves a component's CSS from `getModuleInfo(...).meta.svelte.css` in the
*current* environment. Vite 8 keeps the client and SSR module graphs apart, and SvelteKit emits
`<link>` tags for those virtual CSS modules that the browser fetches **before** the component's
own JS. If the client graph has not transformed the component by then, the plugin finds no CSS
and falls back to serving the raw `.svelte` source as the stylesheet. Which components lose the
race depends on timing, so it looked random.

`server.warmup.clientFiles` in `vite.config.js` pre-transforms every component in the client
environment at startup, so the `<link>` requests always hit. Verified across repeated cold and
warm starts. (`emitCss: false` is not an option: SvelteKit needs the emitted CSS.) Production
builds were never affected.

### How HID++ works here

Messages are 7-byte (`0x10`) or 20-byte (`0x11`) HID reports shaped
`[report_id, device_index, feature_index, function<<4 | software_id, params…]`.

`feature_index` is *not* the well-known feature id — it is assigned per device, so every
feature is resolved at runtime through the Root feature (`0x0000`), which is fixed at index 0.
Requests are tagged with software id `0x0A` and correlated against replies, so unsolicited
notifications and traffic from sibling devices on a shared receiver are discarded instead of
desynchronising the exchange.

Receivers are bridges, not devices: each of their six child slots (`device_index` 1–6) is
pinged and probed independently.

### Design tokens

Colours, radii and type come from G HUB's own stylesheet — Logitech's `@logi/magnetite` design
system, token prefix `dls` — read out of the app bundle, so `src/app.css` matches the real
application rather than an approximation: surface `#212225`, primary `#1196ff`, brand cyan
`#00a9e0`, text `#a7a7a8` / labels `#afb1b4`, outlines at 10 % white, panels 310 px wide with a
12 px radius, titles 24 px / 700 / −0.96 px tracking. G HUB's font is Lineto's Brown
(`BrownPro`, `BrownLogitechPan`), which is commercial and therefore not bundled; the stack
lists it first so anyone who has it gets it.

### Device artwork

**No product images ship with OpenGHub.** Logitech's renders are their copyright, so bundling
them in this repository would be redistributing someone else's work. Devices are drawn as
inline SVG per category (mouse, keyboard, headset, light, microphone, …), and RGB state is
rendered live onto the drawing so the LIGHTSYNC page previews what the device will look like.

If you want the real renders on your own machine, OpenGHub loads them at runtime from:

```
$XDG_DATA_HOME/openghub/devices/<product-id>.png     e.g. c08d.png
```

Anything found there replaces the drawing on that device's card; anything missing falls back
to the SVG, and a file that fails to decode falls back too. `webp`, `jpg` and `jpeg` work as
well, and `046d_c08d.png` is accepted alongside the bare form.

```sh
./scripts/fetch-artwork.sh          # every Logitech device attached to this machine
./scripts/fetch-artwork.sh c08d     # a specific product id
```

The script pulls transparent PNGs from Logitech's own CDN (`resource.logitechg.com`, which
takes Cloudinary-style size parameters) into your data directory — the same thing you could do
by right-clicking the image on logitechg.com. The files stay on your machine.

It reads product ids from `/sys/class/hidraw/*/device/uevent` rather than `lsusb`, because a
device behind a Unifying/LIGHTSPEED receiver reports the *receiver's* id on the USB bus and its
own id only at the HID layer. For the same reason OpenGHub identifies devices by the `modelId`
from feature `0x0003` rather than the address's product id — a G502 on a LIGHTSPEED receiver
reports `0xc539` (the dongle) at the USB level but `407f c08d` as its own ids, and artwork is
matched against any of them. For anything not in the script's table, save a PNG named after
the product id by hand; **Settings → Device artwork** shows the exact path.

Photos cannot light up, so when a device has RGB state the colour is rendered as a backlight
glow behind the image instead.

## Adding a device

Nothing needs to be added for a device to work — names come from feature `0x0005` and the
category from the device-type byte. `src-tauri/src/hidpp/registry.rs` only improves the
fallback name and icon. Use **Settings → HID++ features** on the device page to dump what a
new device exposes.

## Licence

MIT.
