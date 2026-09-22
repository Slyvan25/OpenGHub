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

use serde::Serialize;

use crate::hidpp::{Error, Result};

/// The rule as shipped with this build.
pub const RULE: &str = include_str!("../../packaging/70-openghub.rules");
pub const RULE_PATH: &str = "/etc/udev/rules.d/70-openghub.rules";
/// Where the .deb / .rpm put it; a rule there counts just the same.
pub const PACKAGED_RULE_PATH: &str = "/usr/lib/udev/rules.d/70-openghub.rules";
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
    // /etc overrides /usr/lib for the same file name, so check it first.
    let found = [RULE_PATH, PACKAGED_RULE_PATH]
        .into_iter()
        .find_map(|p| std::fs::read_to_string(p).ok().map(|s| (p, s)));
    UdevRuleStatus {
        installed: found.is_some(),
        current: found.as_ref().map(|(_, s)| normalise(s) == normalise(RULE)).unwrap_or(false),
        stale: Path::new(STALE_RULE_PATH).exists(),
        can_install: crate::sandbox::host_has("pkexec"),
        path: found.as_ref().map(|(p, _)| p.to_string()).unwrap_or_else(|| RULE_PATH.into()),
    }
}

/// Everything that needs root, done in one authentication: install the rule,
/// drop the stale one, reload udev and re-trigger the nodes so the ACL is
/// applied to devices that are already plugged in.
pub fn install() -> Result<UdevRuleStatus> {
    if !crate::sandbox::host_has("pkexec") {
        return Err(Error::other("pkexec (polkit) is not available; install the rule by hand — see the README"));
    }
    // The rule travels inline, base64-encoded: no temp file, so this works
    // from inside a Flatpak sandbox (whose /tmp the host cannot see) too.
    let encoded = base64_encode(RULE.as_bytes());
    let script = format!(
        "printf '%s' '{encoded}' | base64 -d > '{dst}' && chmod 0644 '{dst}' && rm -f '{stale}' && \
         udevadm control --reload-rules && \
         udevadm trigger --action=add --subsystem-match=hidraw && \
         udevadm trigger --action=add --subsystem-match=misc --attr-match=name=uinput; \
         true",
        dst = RULE_PATH,
        stale = STALE_RULE_PATH,
    );
    let out = crate::sandbox::host_command("pkexec", &[])
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| Error::other(format!("could not run pkexec: {e}")))?;
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
    if !s.current && !crate::sandbox::in_flatpak() {
        return Err(Error::other("the rule was written but does not read back as expected"));
    }
    Ok(s)
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn normalise(s: &str) -> String {
    s.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
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
    fn base64_matches_the_standard() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn normalise_ignores_whitespace_only_differences() {
        assert_eq!(normalise("a  \n\nb\n"), normalise("a\nb"));
    }
}
