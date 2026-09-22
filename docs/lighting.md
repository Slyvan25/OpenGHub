# Lighting and artwork

Per-zone LIGHTSYNC, the software effects, and where the device renders and zone geometry come from.

## Per-zone lighting

Devices expose their zones through `getZoneInfo`, including a *location* code that OpenGHub
turns into the tab names G HUB uses — a G502 reports `0x0001` and `0x0002`, shown as **PRIMARY**
and **LOGO**. Each zone keeps its own effect and colour in the profile, and "sync lighting
zones" copies the active one across.

## Screen sampler and audio visualizer

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

## Lighting notes

Two things about `0x8070` that cost real debugging time:

- Byte 1 of `setZoneEffect` is the **zone-local effect index**, not the global effect id. A
  G502 advertises ids `0x00, 0x01, 0x03, 0x0a` at indices `0..=3` and accepts only `0..=3`.
  Sending the id `0x0a` gets "invalid argument"; sending `0x03` quietly selects index 3
  (breathing) instead of cycle. OpenGHub reads each zone's effect list and translates.
- A device running its **onboard profile owns its own LEDs** — `0x8070` writes are accepted and
  then ignored. OpenGHub switches to host mode via `0x8100` first, as G HUB does. Use
  `cargo run --example lighting` to dump what a zone actually supports.

## Where G HUB's artwork really comes from

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

## Zone positions without a depot

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

## Device artwork

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
