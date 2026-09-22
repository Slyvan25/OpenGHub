//! Device permissions: the udev rule that lets the desktop user open Logitech
//! hidraw nodes and `/dev/uinput`, and a way to install it from the app.
//!
//! The rule is `packaging/70-openghub.rules`, compiled in so the app can write
//! exactly the version it was built with. Installing needs root: we hand the
//! job to `pkexec`, which shows the desktop's own polkit password dialog —
//! the app never sees the password. The stale `99-openghub.rules` from early
//! builds is removed at the same time (a `99-` file is tagged too late for
//! systemd's `73-seat-late.rules` to apply the ACL).

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::hidpp::{Error, Result};

/// The rule as shipped with this build.
pub const RULE: &str = include_str!("../../packaging/70-openghub.rules");
pub const RULE_PATH: &str = "/etc/udev/rules.d/70-openghub.rules";
/// Earlier builds installed this name; its number is wrong (see the module docs).
pub const STALE_RULE_PATH: &str = "/etc/udev/rules.d/99-openghub.rules";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UdevRuleStatus {
    /// A rule file exists at `RULE_PATH`.
    pub installed: bool,
    /// …and its content is exactly this build's rule.
    pub current: bool,
    /// A `99-openghub.rules` left over from an early build is present.
    pub stale: bool,
    /// `pkexec` is available, so the app can do the install itself.
    pub can_install: bool,
    pub path: String,
}

/// Compares what is installed with what this build ships.
pub fn status() -> UdevRuleStatus {
    let installed = std::fs::read_to_string(RULE_PATH).ok();
    UdevRuleStatus {
        installed: installed.is_some(),
        current: installed.as_deref().map(|s| normalise(s) == normalise(RULE)).unwrap_or(false),
        stale: Path::new(STALE_RULE_PATH).exists(),
        can_install: which("pkexec"),
        path: RULE_PATH.into(),
    }
}

/// Everything that needs root, done in one authentication: install the rule,
/// drop the stale one, reload udev and re-trigger the nodes so the ACL is
/// applied to devices that are already plugged in.
pub fn install() -> Result<UdevRuleStatus> {
    if !which("pkexec") {
        return Err(Error::other("pkexec (polkit) is not available; install the rule by hand — see the README"));
    }
    let tmp = std::env::temp_dir().join(format!("openghub-udev-{}.rules", std::process::id()));
    std::fs::write(&tmp, RULE).map_err(|e| Error::other(format!("could not write {}: {e}", tmp.display())))?;
    let script = format!(
        "install -m 0644 '{src}' '{dst}' && rm -f '{stale}' && udevadm control --reload-rules && \
         udevadm trigger --action=add --subsystem-match=hidraw && \
         udevadm trigger --action=add --subsystem-match=misc --attr-match=name=uinput; \
         true",
        src = tmp.display(),
        dst = RULE_PATH,
        stale = STALE_RULE_PATH,
    );
    let out = Command::new("pkexec").args(["sh", "-c", &script]).output();
    let _ = std::fs::remove_file(&tmp);
    let out = out.map_err(|e| Error::other(format!("could not run pkexec: {e}")))?;
    match out.status.code() {
        Some(0) => {}
        // polkit: 126 = the user dismissed the dialog, 127 = not authorised.
        Some(126) => return Err(Error::other("authentication was cancelled")),
        Some(127) => return Err(Error::other("authentication failed or was refused")),
        code => {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(Error::other(format!("installing the udev rule failed ({code:?}): {}", err.trim())));
        }
    }
    let s = status();
    if !s.current {
        return Err(Error::other("the rule was written but does not read back as expected"));
    }
    Ok(s)
}

fn normalise(s: &str) -> String {
    s.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_rule_has_both_lines_and_the_right_number() {
        assert!(RULE.contains(r#"KERNEL=="hidraw*""#));
        assert!(RULE.contains(r#"KERNEL=="uinput""#));
        assert!(RULE_PATH.ends_with("/70-openghub.rules"));
    }

    #[test]
    fn normalise_ignores_whitespace_only_differences() {
        assert_eq!(normalise("a  \n\nb\n"), normalise("a\nb"));
    }
}
