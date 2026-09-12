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
| RGB lighting (`0x8070`) | ✅ per zone: off / fixed / breathing / colour cycle |
| Macros on onboard memory (`0x8100`) | ✅ recorded, written to device flash, backed up first |
| Button remapping (non-macro) | ⚠️ UI and profile storage only; not written to the device yet |
| Onboard profile memory (`0x8100`) | ❌ detected but not edited |

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

Zone positions on the artwork live in `src/lib/zones.ts`, as fractions of the art box per
device category. The device reports *which* zone is lit but not *where* it is in the picture,
so a photo gets the lighting composited on top: one screen-blended blob per lit zone. That is
an approximation, but it puts a G502's logo glow on the palm and its primary glow on the wheel,
which is what the real mouse does.

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

### Known dev-server quirk

On a cold start — the first run after `node_modules/.vite` is cleared, or after dependencies
change — Vite re-optimises deps and forces a page reload mid-load. That can race
`vite-plugin-svelte`'s one-shot CSS cache, and you get:

```
[vite-plugin-svelte:load] failed to load virtual css module …/Foo.svelte?svelte&type=style&lang.css
```

Any component caught by the race loads its raw source in place of its stylesheet, so it renders
unstyled. Restart the dev server; the second run has a warm cache and is clean. Production
builds are unaffected.

## Architecture

```
src-tauri/src/
  hidpp/mod.rs        packet framing, transport, feature discovery, enumeration
  hidpp/features.rs   typed wrappers: battery, DPI, report rate, lighting
  hidpp/registry.rs   product-id → name/category table (display hints only)
  state.rs            DeviceManager: open handles, cached snapshots, receiver probing
  commands.rs         Tauri IPC surface
  profiles.rs         JSON config store (XDG config dir)
  demo.rs             synthetic devices used when no hardware is reachable
  lib.rs              app setup, tray, background battery poller

src/
  routes/             SvelteKit SPA: dashboard, /device/[id]/[tab], /profiles, /settings
  lib/components/     chrome and controls (device cards, sliders, colour picker, DPI track)
  lib/views/          Sensitivity, Assignments, LIGHTSYNC, Settings
  lib/stores/         Svelte 5 rune stores for devices, config and UI state
  lib/api.ts          typed IPC bindings, with a browser fallback
```

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
