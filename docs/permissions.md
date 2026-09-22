# Device permissions and startup

The .deb and .rpm packages install the udev rule and reload udev in their post-install step, so nothing below is needed there. The AppImage and source builds get the in-app installer described first.

## Device permissions

HID++ needs read/write access to the device's `hidraw` node, which desktop users do not get
by default — on most distributions `/dev/hidraw*` is `root:root 0600`.

**The app does this for you.** On launch OpenGHub checks `/etc/udev/rules.d/70-openghub.rules`
against the rule it was built with; if it is missing or out of date (older builds shipped a
`99-` file, and the rule has since grown a `/dev/uinput` line) it offers to install it. The
install runs through polkit (`pkexec`), so your desktop shows its own password prompt and the
app never handles the password; it writes the rule, removes a stale `99-openghub.rules`,
reloads udev and re-triggers the nodes, then rescans. "Not now" skips it for this launch,
"Don't ask again" for good — the Devices page keeps an *Install device access* button either
way. Without polkit, or by hand:

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

## Launch at startup

*Settings → General → Launch at startup* writes `~/.config/autostart/openghub.desktop` (the
freedesktop autostart entry every desktop honours), pointing at the running executable — or
the `$APPIMAGE` when started from one — with `--minimized`, so a login goes straight to the
tray with profiles and lighting applied. Turning it off removes the file; the toggle reflects
the file, so an entry added or removed by hand is respected. *Start minimised to tray* does
the same for manual launches.

## Flatpak

The Flatpak (`packaging/flatpak/com.openghub.app.yml`) is built from the release .deb, so it is
the same binary. The sandbox gets `--device=all` (there is no portal for `hidraw` or `uinput`)
and talks to `org.freedesktop.Flatpak`, through which every helper process runs **on the host**
via `flatpak-spawn --host`: `pkexec` for the udev rule (the rule travels inline, base64-encoded,
because the host cannot see the sandbox's `/tmp`), `gst-launch-1.0` with the portal's PipeWire
fd forwarded and `parec` for LIGHTSYNC, `steam` / `lutris` / `xdg-open` for the Games tab, and
`loginctl` for "lock computer". "Launch at startup" writes `Exec=flatpak run com.openghub.app`
and the launchers' folders are mounted read-only for the Games tab. Everything else — HID++,
the force-feedback driver, the virtual keyboard — runs inside the sandbox as it does natively.

## AppImage

The AppImage bundles WebKitGTK. That WebKit cannot start its bubblewrap sandbox from inside the
squashfs mount (`WebKitWebProcess has encountered a fatal error`, black window), and its DMA-BUF
renderer misbehaves with some host Mesa builds, so the app sets `WEBKIT_FORCE_SANDBOX=0` and
`WEBKIT_DISABLE_DMABUF_RENDERER=1` for itself when started from an AppImage. Set either in the
environment to override.
