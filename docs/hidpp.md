# HID++ and the device layer

How OpenGHub talks to the hardware: the protocol, onboard memory, button remapping, device settings and firmware.

## How HID++ works here

Messages are 7-byte (`0x10`) or 20-byte (`0x11`) HID reports shaped
`[report_id, device_index, feature_index, function<<4 | software_id, params…]`.

`feature_index` is *not* the well-known feature id — it is assigned per device, so every
feature is resolved at runtime through the Root feature (`0x0000`), which is fixed at index 0.
Requests are tagged with software id `0x0A` and correlated against replies, so unsolicited
notifications and traffic from sibling devices on a shared receiver are discarded instead of
desynchronising the exchange.

Receivers are bridges, not devices: each of their six child slots (`device_index` 1–6) is
pinged and probed independently.

## Probing real hardware

`probe` is a read-only diagnostic that dumps everything OpenGHub can read from the devices on
the machine — endpoints, whether each one opens, and the full HID++ feature list. It never
writes to a device. Run it first when something is not detected, or when adding support for
hardware that is not in the registry:

```sh
cd src-tauri && cargo run --example probe      # devices, features, DPI, rate, battery
cd src-tauri && cargo run --example lighting   # per-zone lighting capabilities
```

Stop the app first, so the two are not driving the same device at once.

## Onboard profiles vs host control

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

## Onboard memory and macros

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

## On-board memory mode

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

## Button remapping is device-specific

There are two mechanisms, and which one applies depends on the hardware:

- **`0x1b04` (Special Keys & Buttons)** — remap at runtime, no flash writes. Common on
  keyboards and older mice.
- **`0x8100` (Onboard Profiles)** — the device stores button descriptors and macros in its own
  flash. This is what a G502 LIGHTSPEED uses; it does **not** expose `0x1b04` at all.

Do not assume `0x1b04`: run `cargo run --example onboard` (read-only) to see which a device has.
A G502 reports 5 profiles, 11 buttons, 16 sectors, profile format 3, macro format 1, and its
button descriptors are legible 4-byte records — `80 01 00 01` is "mouse button 1", `90 07 ff 00`
is a special action.

## G-Shift

Assign *Actions → G-Shift* to a button and flip the DEFAULT / G-SHIFT switch under the render
to bind the second layer (stored as `button-N:gshift`). In software mode the button pump keeps
a second `0x8110` remapping table and loads it while G-Shift is held, so the device's own
actions follow the layer too; each press remembers the layer it was resolved in so a release
never sticks a key. In onboard mode the layer is written to the profile's second button table
at offset 96 (unassigned = `ff`, as the factory table has it), with the shift button itself as
special `0x0b`.

## Device settings

The gear tab carries G HUB's per-device settings, kept outside the profiles
(`settings.deviceSettings[deviceId]`): **firmware version** from `0x0003 getFwInfo` (main
`MPM17.00_B0008` and bootloader `BOT92.00_B0008` on the G502), **power management** — the
auto-sleep and inactivity-lighting timeouts written into the onboard profile's header words at
offsets 28/30 (seconds, big-endian, `0xffff` = firmware default) — a software **low-battery
mode** that dims the profile's lighting below a threshold and restores it once charged past it,
and the **left-handed** layout, which swaps the two clicks through the assignment plan (and
therefore the onboard table too).

## Firmware updates

Logitech publishes firmware the same way it publishes artwork: as `*_dfu` depots on the public
CDN, listed in the depository. Each holds a `dfu.json` (version, the USB interface ids it
applies to — the bootloader's marked `force` — and start blockers such as *connect over USB*),
release notes in twenty languages, and the `.dfu` image with its SHA-256. *Device settings →
Firmware → Check for updates* fetches every such depot once (27 packages, ~3.5 MB; the Blue
microphones and two keyboards use other containers and are skipped), caches them under the data
directory, and matches connected devices by product id. The catalogue can only be as new as the
imported depository, since `current.json` itself is not downloadable.

An update runs the HID++ 2.0 DFU sequence fwupd's `logitech-hidpp` plugin uses: the device is
sent into its bootloader with feature `0x00C2` (it reboots itself) or `0x00C1` (unplug and
reconnect), the bootloader enumerates under its own product id and exposes feature `0x00D0`,
and the `.dfu` image — which is literally the packet stream, 16-byte commands starting with
`dfuStart` and the firmware's magic name — goes out as `dfuStart` then `dfuCmdData1..3,0` in a
sliding window, each packet acknowledged with a status byte (busy statuses wait for the
device's notification). `restart` ends it and the device comes back on the bus. A failed
transfer leaves the device in its bootloader, from where the update can simply be run again.
The confirmation dialog says so, and says that this path has not been exercised on every
device family — the G502 LIGHTSPEED here has no package published, so it could not be run on
this machine's hardware.
