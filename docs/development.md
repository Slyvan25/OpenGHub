# Developing OpenGHub

How the app is built, run and tested, plus the notes that only matter when you change it.

## Requirements

- Rust 1.77+, Node 20+
- WebKitGTK 4.1, libudev (`webkit2gtk4.1-devel libsoup3-devel libudev-devel` on Fedora,
  `libwebkit2gtk-4.1-dev libudev-dev` on Debian/Ubuntu)

`hidapi` is compiled from the vendored hidraw backend, so no system hidapi package is needed.

## Running

```sh
npm install
npm run tauri dev      # desktop app against real hardware
npm run dev            # browser only, served by the mock backend in src/lib/mock.ts
npm run tauri build    # produces .deb and .rpm in src-tauri/target/release/bundle
```

The release workflow (`.github/workflows/release.yml`) builds the .deb, .rpm and AppImage on
Ubuntu 22.04 for every `v*` tag and publishes them on the GitHub release, together with the
Arch PKGBUILD from `packaging/arch/`. The AppImage step downloads `linuxdeploy` at bundle time;
if that fails on your machine, build the other two with `npm run tauri build -- --bundles deb,rpm`.

To cut a release, bump the version **before** tagging — the workflow refuses a tag that does
not match the sources, because tauri-action publishes under the version in `tauri.conf.json`
while the other steps follow the tag, and a mismatch yields a half-filled release:

```sh
scripts/bump-version.sh 0.1.1
git commit -m "release: v0.1.1"
git tag -a v0.1.1 -m "OpenGHub 0.1.1"
git push origin master v0.1.1
```

Re-tagging a version that is already on GitHub needs the old one out of the way first
(`git push origin :refs/tags/v0.1.1`), and the half-finished release deleted in the web UI.

Note that plain `cargo build` produces a *dev* binary that expects the Vite dev server on port
1420 — use `npm run tauri dev` or `npm run tauri build` instead.

## Tests

```sh
cd src-tauri && cargo test    # protocol framing, DPI list parsing, config round-trips
npm run check                 # svelte-check + TypeScript
```

## A dev-server race, and why `vite.config.js` warms the client graph

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

## Design tokens

Colours, radii and type come from G HUB's own stylesheet — Logitech's `@logi/magnetite` design
system, token prefix `dls` — read out of the app bundle, so `src/app.css` matches the real
application rather than an approximation: surface `#212225`, primary `#1196ff`, brand cyan
`#00a9e0`, text `#a7a7a8` / labels `#afb1b4`, outlines at 10 % white, panels 310 px wide with a
12 px radius, titles 24 px / 700 / −0.96 px tracking. G HUB's font is Lineto's Brown
(`BrownPro`, `BrownLogitechPan`), which is commercial and therefore not bundled; the stack
lists it first so anyone who has it gets it.

## Adding a device

Nothing needs to be added for a device to work — names come from feature `0x0005` and the
category from the device-type byte. `src-tauri/src/hidpp/registry.rs` only improves the
fallback name and icon. Use **Settings → HID++ features** on the device page to dump what a
new device exposes.

## The splash screen

`src/app.html` carries a small framework-free splash: inline CSS and an inline copy of
`static/logo.svg`, so it paints with the first frame instead of the webview showing a black
window until SvelteKit has booted. Two bars turn a full circle and resolve into the mark, which
then breathes. `+layout.svelte` fades it out (`#splash.done`, then removes the node) as soon as
the config, device and artwork stores have loaded — and it removes itself after ten seconds
anyway, so a failure during boot can never leave the window covered. It honours
`prefers-reduced-motion`.
