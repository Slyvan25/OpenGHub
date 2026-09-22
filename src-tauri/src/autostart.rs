//! Launch at login — G HUB's "Launch at startup", done the freedesktop way:
//! a `.desktop` entry in `~/.config/autostart/`, which every desktop (GNOME,
//! KDE, XFCE, …) starts with the session. No daemon, no systemd unit; the
//! toggle in Settings writes or removes one file, and its presence is the
//! state — so an entry the user made or removed by hand is respected.
//!
//! The entry launches the app with `--minimized`, so a boot goes straight to
//! the tray; opening the window is a click away. For an AppImage the
//! `$APPIMAGE` path is used instead of the mounted binary, which vanishes
//! when the image is unmounted.

use std::path::PathBuf;

use crate::hidpp::{Error, Result};

const FILE_NAME: &str = "openghub.desktop";
/// Passed by the autostart entry; the app starts hidden in the tray.
pub const MINIMIZED_FLAG: &str = "--minimized";

fn entry_path() -> Result<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or_else(|| Error::other("neither XDG_CONFIG_HOME nor HOME is set"))?;
    Ok(config.join("autostart").join(FILE_NAME))
}

/// What the entry should run: the AppImage if that is how we were started,
/// otherwise this executable.
fn exec_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Ok(p);
    }
    std::env::current_exe().map_err(|e| Error::other(format!("cannot find the executable: {e}")))
}

pub fn is_enabled() -> bool {
    entry_path().map(|p| p.is_file()).unwrap_or(false)
}

/// Writes or removes the autostart entry.
pub fn set_enabled(on: bool) -> Result<bool> {
    let path = entry_path()?;
    if !on {
        match std::fs::remove_file(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::other(format!("could not remove {}: {e}", path.display()))),
        }
        return Ok(false);
    }
    let exe = exec_path()?;
    let contents = entry(&exe.to_string_lossy());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::other(format!("could not create {}: {e}", dir.display())))?;
    }
    std::fs::write(&path, contents).map_err(|e| Error::other(format!("could not write {}: {e}", path.display())))?;
    Ok(true)
}

/// The `.desktop` text. `Exec` is quoted so a path with spaces survives.
fn entry(exec: &str) -> String {
    let quoted = format!("\"{}\"", exec.replace('"', "\\\""));
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=OpenGHub\n\
         GenericName=Logitech G Configuration\n\
         Comment=Configure Logitech gaming peripherals\n\
         Exec={quoted} {MINIMIZED_FLAG}\n\
         Icon=openghub\n\
         Terminal=false\n\
         Categories=Utility;Settings;HardwareSettings;\n\
         StartupWMClass=OpenGHub\n\
         X-GNOME-Autostart-enabled=true\n\
         X-KDE-autostart-after=panel\n"
    )
}

/// `true` when this process was started by the autostart entry (or anyone
/// passing the flag), in which case the window stays hidden in the tray.
pub fn started_minimized() -> bool {
    std::env::args().skip(1).any(|a| a == MINIMIZED_FLAG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_writes_and_removes_the_entry() {
        let dir = std::env::temp_dir().join(format!("openghub-autostart-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        assert!(!is_enabled());
        assert!(set_enabled(true).unwrap());
        let path = dir.join("autostart").join(FILE_NAME);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(MINIMIZED_FLAG));
        assert!(is_enabled());
        assert!(!set_enabled(false).unwrap());
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn entry_quotes_the_path_and_starts_minimized() {
        let e = entry("/opt/Open GHub/openghub");
        assert!(e.contains("Exec=\"/opt/Open GHub/openghub\" --minimized"));
        assert!(e.starts_with("[Desktop Entry]\n"));
        assert!(e.contains("X-GNOME-Autostart-enabled=true"));
    }
}
