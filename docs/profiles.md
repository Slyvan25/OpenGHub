# Profiles, games and scripting

Per-game profiles, the application database, the Games tab, community profiles, G HUB imports and Lua scripting.

## Per-game profiles and the application database

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

## The Games tab

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

## Community profiles

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

## Importing your G HUB profiles

Settings → *Import profiles from settings.db* reads `%LOCALAPPDATA%\LGHUB\settings.db`
(one SQLite table whose latest row is a JSON document). Profiles map to ours by Logitech
application id (the Desktop profile to *Desktop: Default*); per device, slot ids such as
`g502wireless_mouse_settings`, `…_lighting_setting_firmware` and `…_g7_m1[_shifted]` become the
DPI table with shift stage, report rate, per-zone lighting and `button-N[:gshift]`
assignments. Built-in mouse/device functions are synthetic card ids
(`0f82f693-…-TTNN00000000`) decoded from the G502's factory table; keystroke cards carry a HID
usage plus modifier usages; integration actions (OBS, Discord, Overwolf) have no Linux
equivalent and are listed as skipped.

## Lua scripting

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
