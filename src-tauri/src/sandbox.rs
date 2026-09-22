//! Running inside Flatpak.
//!
//! The sandbox has no `pkexec`, `gst-launch-1.0`, `parec`, `steam` or
//! `xdg-open` of its own, and its `/tmp` and `PATH` are not the host's. With
//! `--talk-name=org.freedesktop.Flatpak` in the manifest, `flatpak-spawn
//! --host` runs a command on the host instead; that is what every process
//! the app starts goes through here. Outside Flatpak this is a no-op.

use std::process::Command;

pub const APP_ID: &str = "io.github.slyvan25.OpenGHub";

/// `/.flatpak-info` exists only inside a Flatpak sandbox.
pub fn in_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// A `Command` for `program`, on the host when sandboxed.
/// `forward_fds` are file descriptors the child must inherit (the screen
/// sampler's PipeWire fd); `flatpak-spawn --forward-fd` carries them across.
pub fn host_command(program: &str, forward_fds: &[i32]) -> Command {
    if in_flatpak() {
        let mut c = Command::new("flatpak-spawn");
        c.arg("--host");
        for fd in forward_fds {
            c.arg(format!("--forward-fd={fd}"));
        }
        c.arg(program);
        c
    } else {
        Command::new(program)
    }
}

/// Whether `program` can be run — on the host when sandboxed, else on PATH.
pub fn host_has(program: &str) -> bool {
    if in_flatpak() {
        return Command::new("flatpak-spawn")
            .args(["--host", "sh", "-c", &format!("command -v {program} >/dev/null 2>&1")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
        .unwrap_or(false)
}

/// What an autostart entry or a shell should run to start this app.
pub fn launch_command() -> Option<String> {
    if in_flatpak() {
        return Some(format!("flatpak run {APP_ID}"));
    }
    None
}
