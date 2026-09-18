//! Community profiles: an open counterpart to G HUB's Community tab.
//!
//! G HUB's community is a closed Logitech service. OpenGHub instead reads
//! profiles from a plain Git repository: one JSON file per profile under
//! `profiles/<modelId>/`, plus an `index.json` that CI regenerates on every
//! push. Anyone can contribute with a pull request, and the client needs
//! nothing but HTTPS.
//!
//! ```text
//! <repo>/index.json                         ← what the client lists
//! <repo>/profiles/g502_wireless/cs2.json    ← one shared profile
//! ```
//!
//! A shared profile carries no Logitech data — only the user's own settings.
//! It can contain macros, which are keystroke sequences, so the UI shows
//! exactly what a macro does before anything is imported and never writes to
//! a device without the user's explicit action.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::hidpp::Error;
use crate::profiles::{DeviceProfile, Profile};

/// Current shared-profile format. Bump when the shape changes incompatibly.
pub const FORMAT_VERSION: u32 = 1;

/// Default repository. Overridable in settings so forks and private mirrors work.
pub const DEFAULT_REPO: &str = "https://raw.githubusercontent.com/Slyvan25/openghub-community/main";

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

/// Which hardware a shared profile is for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetDevice {
    /// G HUB's model id, e.g. `g502_wireless`; the human-stable key.
    pub model_id: String,
    /// Product ids the profile was made on, so OpenGHub can match a connected
    /// device without a G HUB device database.
    #[serde(default)]
    pub product_ids: Vec<u16>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub kind: String,
}

/// The game a profile is meant for, if any.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetApplication {
    /// Id from Logitech's public application database.
    pub id: String,
    pub name: String,
}

/// One shareable profile — the file that lives in the repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProfile {
    pub format: u32,
    /// Stable id, lowercase kebab-case, unique within the repository.
    pub id: String,
    pub name: String,
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// SPDX identifier. Contributions are expected to be `CC0-1.0`.
    #[serde(default = "default_license")]
    pub license: String,
    pub device: TargetDevice,
    #[serde(default)]
    pub application: Option<TargetApplication>,
    /// The settings themselves: DPI stages, lighting, assignments, macros.
    pub profile: DeviceProfile,
}

fn default_license() -> String {
    "CC0-1.0".into()
}

/// Summary row in `index.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub device: TargetDevice,
    #[serde(default)]
    pub application: Option<TargetApplication>,
    /// Path relative to the repository root.
    pub path: String,
    /// Counts, so the list can say "3 macros" without fetching the file.
    #[serde(default)]
    pub macro_count: usize,
    #[serde(default)]
    pub assignment_count: usize,
    #[serde(default)]
    pub dpi_stages: Vec<u16>,
    #[serde(default)]
    pub has_lighting: bool,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityIndex {
    pub version: u32,
    #[serde(default)]
    pub generated: String,
    pub profiles: Vec<IndexEntry>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Rejects anything that could not have come from a well-formed export.
/// Keeps the import path from being a way to smuggle odd data into the config.
pub fn validate(p: &SharedProfile) -> Result<(), Error> {
    if p.format != FORMAT_VERSION {
        return Err(Error::other(format!(
            "profile format {} is not supported (this build reads {})",
            p.format, FORMAT_VERSION
        )));
    }
    if p.id.is_empty()
        || p.id.len() > 80
        || !p.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(Error::other("profile id must be lowercase kebab-case"));
    }
    if p.name.trim().is_empty() || p.name.len() > 80 {
        return Err(Error::other("profile name is missing or too long"));
    }
    if p.device.model_id.is_empty() {
        return Err(Error::other("profile has no target device"));
    }
    if p.profile.dpi_stages.len() > 5 || p.profile.dpi_stages.iter().any(|d| *d == 0 || *d > 50_000) {
        return Err(Error::other("DPI stages are out of range"));
    }
    if p.profile.macros.len() > 64 {
        return Err(Error::other("too many macros"));
    }
    for m in &p.profile.macros {
        if m.steps.len() > 512 {
            return Err(Error::other(format!("macro '{}' is too long", m.name)));
        }
    }
    Ok(())
}

/// Builds the shareable file from a local profile and the device it is for.
pub fn export(
    profile: &Profile,
    device_id: &str,
    device: TargetDevice,
    author: &str,
    description: &str,
) -> Result<SharedProfile, Error> {
    let settings = profile
        .devices
        .get(device_id)
        .cloned()
        .ok_or_else(|| Error::other("this profile has no settings for that device yet"))?;
    let base = profile.name.split(':').next().unwrap_or(&profile.name).trim();
    let id = slugify(&format!("{}-{}", base, device.model_id));
    let shared = SharedProfile {
        format: FORMAT_VERSION,
        id,
        name: profile.name.clone(),
        author: author.trim().to_string(),
        description: description.trim().to_string(),
        license: default_license(),
        device,
        application: profile.application_id.as_ref().map(|id| TargetApplication {
            id: id.clone(),
            name: base.to_string(),
        }),
        profile: settings,
    };
    validate(&shared)?;
    Ok(shared)
}

/// `"CS2 Competitive / G502"` → `cs2-competitive-g502`.
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').chars().take(80).collect()
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

fn cache_dir() -> PathBuf {
    crate::artwork::dir()
        .parent()
        .map(|d| d.join("community"))
        .unwrap_or_else(|| PathBuf::from("openghub-community"))
}

fn get_text(url: &str) -> Result<String, Error> {
    ureq::get(url)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| Error::other(format!("{url}: {e}")))?
        .into_string()
        .map_err(|e| Error::other(format!("{url}: {e}")))
}

/// Downloads the index, caching it; falls back to the cache when offline.
pub fn fetch_index(repo: &str, refresh: bool) -> Result<CommunityIndex, Error> {
    let cache = cache_dir();
    let path = cache.join("index.json");
    if !refresh {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(idx) = serde_json::from_str::<CommunityIndex>(&text) {
                return Ok(idx);
            }
        }
    }
    let url = format!("{}/index.json", repo.trim_end_matches('/'));
    match get_text(&url) {
        Ok(text) => {
            let idx: CommunityIndex = serde_json::from_str(&text)
                .map_err(|e| Error::other(format!("community index is malformed: {e}")))?;
            let _ = std::fs::create_dir_all(&cache);
            let _ = std::fs::write(&path, &text);
            Ok(idx)
        }
        Err(e) => {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(idx) = serde_json::from_str::<CommunityIndex>(&text) {
                    log::warn!("community index refresh failed ({e}); using cached copy");
                    return Ok(idx);
                }
            }
            Err(e)
        }
    }
}

/// Downloads and validates one shared profile.
pub fn fetch_profile(repo: &str, path: &str) -> Result<SharedProfile, Error> {
    // Paths come from the index; keep them inside the repository.
    if path.contains("..") || path.starts_with('/') || path.contains("://") {
        return Err(Error::other("refusing a profile path outside the repository"));
    }
    let url = format!("{}/{}", repo.trim_end_matches('/'), path);
    let text = get_text(&url)?;
    let shared: SharedProfile = serde_json::from_str(&text)
        .map_err(|e| Error::other(format!("shared profile is malformed: {e}")))?;
    validate(&shared)?;
    Ok(shared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::LightingSettings;

    fn sample() -> SharedProfile {
        SharedProfile {
            format: FORMAT_VERSION,
            id: "cs2-competitive-g502-wireless".into(),
            name: "Counter-Strike 2: Competitive".into(),
            author: "silvan".into(),
            description: "Low sens, logo off".into(),
            license: "CC0-1.0".into(),
            device: TargetDevice {
                model_id: "g502_wireless".into(),
                product_ids: vec![0x407f, 0xc08d],
                display_name: "G502 LIGHTSPEED".into(),
                kind: "mouse".into(),
            },
            application: Some(TargetApplication { id: "ca572e6a".into(), name: "Counter-Strike 2".into() }),
            profile: DeviceProfile {
                dpi_stages: vec![400, 800, 1600],
                active_stage: 1,
                report_rate_hz: Some(1000),
                lighting: Some(LightingSettings::default()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn a_well_formed_profile_validates_and_round_trips() {
        let p = sample();
        validate(&p).unwrap();
        let text = serde_json::to_string_pretty(&p).unwrap();
        let back: SharedProfile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.device, p.device);
        assert_eq!(back.profile.dpi_stages, vec![400, 800, 1600]);
    }

    #[test]
    fn validation_rejects_bad_shapes() {
        let mut p = sample();
        p.format = 99;
        assert!(validate(&p).is_err(), "unknown format");

        let mut p = sample();
        p.id = "Has Spaces".into();
        assert!(validate(&p).is_err(), "id must be kebab-case");

        let mut p = sample();
        p.profile.dpi_stages = vec![0];
        assert!(validate(&p).is_err(), "zero DPI");

        let mut p = sample();
        p.device.model_id.clear();
        assert!(validate(&p).is_err(), "no device");
    }

    #[test]
    fn slugs_are_stable_and_safe() {
        assert_eq!(slugify("CS2 Competitive / G502"), "cs2-competitive-g502");
        assert_eq!(slugify("  Überprofil!!  "), "berprofil");
        assert_eq!(slugify("a--b"), "a-b");
    }

    #[test]
    fn profile_paths_stay_inside_the_repo() {
        assert!(fetch_profile("https://x", "../etc/passwd").is_err());
        assert!(fetch_profile("https://x", "/abs").is_err());
        assert!(fetch_profile("https://x", "https://evil/x.json").is_err());
    }

    #[test]
    fn export_uses_the_profiles_device_settings() {
        let mut profile = Profile {
            id: "p1".into(),
            name: "Counter-Strike 2: Default".into(),
            kind: "game".into(),
            application_id: Some("ca572e6a".into()),
            poster_url: None,
            disabled: false,
            devices: Default::default(),
            script: None,
        };
        profile.devices.insert(
            "046d:407f:255".into(),
            DeviceProfile { dpi_stages: vec![800], ..Default::default() },
        );
        let device = sample().device;
        let shared = export(&profile, "046d:407f:255", device, "silvan", "").unwrap();
        assert_eq!(shared.id, "counter-strike-2-g502-wireless");
        assert_eq!(shared.application.unwrap().name, "Counter-Strike 2");
        assert_eq!(shared.profile.dpi_stages, vec![800]);

        assert!(export(&profile, "other-device", sample().device, "s", "").is_err());
    }
}
