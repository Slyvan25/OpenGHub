//! Profile storage.
//!
//! G HUB keys settings on an application ("Desktop", a game, …). We mirror that:
//! a profile holds one [`DeviceProfile`] per device id, and the active profile is
//! what the UI edits. Everything lives in a single JSON file under the user's
//! config directory so it is trivial to inspect, back up or hand-edit.

use std::fs;
use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::hidpp::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingSettings {
    /// `off` | `fixed` | `breathing` | `cycle` | `screen` | `audio`
    pub effect: String,
    /// `#rrggbb`
    pub color: String,
    pub brightness: u8,
    pub rate_ms: u16,
    /// Parameters of a software effect (`screen` / `audio`), run by the app.
    #[serde(default)]
    pub software: Option<crate::lightsync::SoftwareEffect>,
}

impl Default for LightingSettings {
    fn default() -> Self {
        LightingSettings {
            effect: "fixed".into(),
            color: "#00b8fc".into(),
            brightness: 100,
            rate_ms: 5000,
            software: None,
        }
    }
}

/// A macro the user has recorded. Stored here rather than on the device, so
/// OpenGHub can rebuild the device's macro sector from scratch every time and
/// never leaves orphaned macros behind.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroDef {
    pub id: String,
    pub name: String,
    /// What the device plays: the flattened sequence.
    pub steps: Vec<crate::hidpp::onboard::MacroStep>,
    /// G HUB's macro type: `noRepeat`, `repeatWhileHolding`, `toggle` or
    /// `sequence`. Only affects how the editor presents the steps; the device
    /// always receives `steps`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Sequence macros keep their three sections for editing.
    #[serde(default)]
    pub sections: Option<MacroSections>,
    #[serde(default)]
    pub use_standard_delays: Option<bool>,
    #[serde(default)]
    pub standard_delay_ms: Option<u32>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroSections {
    #[serde(default)]
    pub on_press: Vec<crate::hidpp::onboard::MacroStep>,
    #[serde(default)]
    pub while_holding: Vec<crate::hidpp::onboard::MacroStep>,
    #[serde(default)]
    pub on_release: Vec<crate::hidpp::onboard::MacroStep>,
}

/// A zone glow's placement on the artwork, 0-1 in each axis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonePosition {
    pub x: f32,
    pub y: f32,
    /// Blob radius as a fraction of the art width.
    pub r: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    /// Physical control id, e.g. `button-4`.
    pub control: String,
    /// Command category: `command` | `key` | `action` | `macro` | `system`.
    pub category: String,
    pub label: String,
    /// Payload interpreted by the category, e.g. a keysym name.
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProfile {
    /// DPI stages shown on the sensitivity screen, in ascending order.
    #[serde(default)]
    pub dpi_stages: Vec<u16>,
    #[serde(default)]
    pub active_stage: usize,
    /// G HUB's "DPI shift" speed: the stage a DPI-shift button jumps to while
    /// held. `None` when no stage is marked.
    #[serde(default)]
    pub shift_stage: Option<usize>,
    #[serde(default)]
    pub report_rate_hz: Option<u32>,
    /// Fallback used for devices with a single zone, and as the seed for new
    /// zones. Kept separate from `lighting_zones` so older configs still load.
    #[serde(default)]
    pub lighting: Option<LightingSettings>,
    /// Per-zone settings, keyed by zone index as a string (JSON object keys).
    #[serde(default)]
    pub lighting_zones: std::collections::HashMap<String, LightingSettings>,
    /// HID++ location names per zone index (`["Primary", "Logo"]`), cached from
    /// the device so the dashboard can draw per-zone lighting without asking
    /// the hardware for every card.
    #[serde(default)]
    pub zone_names: Vec<String>,
    /// Where each zone's glow sits on this device's artwork, as fractions of the
    /// art box. Only the user can know this: the device reports no position, and
    /// a different product photo moves everything.
    #[serde(default)]
    pub zone_positions: std::collections::HashMap<String, ZonePosition>,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
    /// Recorded macros, referenced by assignments whose category is `macro`.
    #[serde(default)]
    pub macros: Vec<MacroDef>,
    /// Steering wheel settings, for wheels.
    #[serde(default)]
    pub wheel: Option<crate::wheel::WheelSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// `desktop` | `game` | `app` — drives the icon in the profile picker.
    pub kind: String,
    /// Application id from Logitech's database. When that game is detected
    /// running, this profile is activated automatically.
    #[serde(default)]
    pub application_id: Option<String>,
    /// Poster for the picker, cached from the database at bind time.
    #[serde(default)]
    pub poster_url: Option<String>,
    /// A disabled game never has its profile activated, even when detected.
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub devices: std::collections::HashMap<String, DeviceProfile>,
}

impl Profile {
    fn desktop_default() -> Self {
        Profile {
            id: "default".into(),
            name: "Desktop: Default".into(),
            kind: "desktop".into(),
            application_id: None,
            poster_url: None,
            disabled: false,
            devices: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_true")]
    pub start_minimised: bool,
    #[serde(default = "default_true")]
    pub show_battery_notifications: bool,
    #[serde(default = "default_poll_secs")]
    pub battery_poll_seconds: u64,
    #[serde(default)]
    pub illumination_follows_profile: bool,
    /// Switch profiles automatically when a bound game starts or stops.
    #[serde(default = "default_true")]
    pub auto_switch_profiles: bool,
    /// Fetch a device's render and layout from Logitech's CDN on first sight,
    /// as G HUB does. Only possible once a depository has been imported.
    #[serde(default = "default_true")]
    pub auto_fetch_artwork: bool,
    /// The profile used when no bound game is running — G HUB's "persistent
    /// profile". Defaults to the Desktop profile.
    #[serde(default = "default_profile_id")]
    pub persistent_profile: String,
    /// Base URL of the community profile repository (raw file access).
    #[serde(default = "default_community_repo")]
    pub community_repo: String,
    /// Name written into profiles the user shares.
    #[serde(default)]
    pub author_name: String,
    /// Run the userspace force-feedback driver for classic wheels, so games
    /// get force feedback without a kernel module.
    #[serde(default = "default_true")]
    pub wheel_driver: bool,
    /// The desktop portal's restore token for the screen sampler, so the
    /// "share your screen" dialog is shown once.
    #[serde(default)]
    pub screen_restore_token: Option<String>,
    /// Devices the user switched to on-board memory mode: they run their own
    /// stored profile and OpenGHub writes settings into it instead of driving
    /// them live.
    #[serde(default)]
    pub onboard_mode_devices: Vec<String>,
    /// Per-device settings that are not part of a profile (G HUB's device
    /// settings screen): power management, low-battery mode, button layout.
    #[serde(default)]
    pub device_settings: std::collections::HashMap<String, DeviceSettings>,
}

/// G HUB's per-device settings, kept outside the profiles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    /// Minutes of inactivity before the device powers off; 0 = never / default.
    #[serde(default)]
    pub auto_sleep_min: u16,
    /// Minutes of inactivity before the lighting goes to sleep; 0 = default.
    #[serde(default)]
    pub inactivity_lighting_min: u16,
    /// Dim the lighting when the battery drops to `low_battery_threshold`.
    #[serde(default)]
    pub low_battery_mode: bool,
    #[serde(default = "default_low_threshold")]
    pub low_battery_threshold: u8,
    /// Brightness (0-100) used while in low-battery mode.
    #[serde(default)]
    pub low_battery_brightness: u8,
    /// Swap primary and secondary click.
    #[serde(default)]
    pub left_handed: bool,
}

fn default_low_threshold() -> u8 {
    15
}

fn default_community_repo() -> String {
    crate::community::DEFAULT_REPO.into()
}

fn default_profile_id() -> String {
    "default".into()
}

fn default_true() -> bool {
    true
}
fn default_poll_secs() -> u64 {
    60
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            start_minimised: false,
            show_battery_notifications: true,
            battery_poll_seconds: default_poll_secs(),
            illumination_follows_profile: false,
            auto_switch_profiles: true,
            auto_fetch_artwork: true,
            persistent_profile: "default".into(),
            community_repo: crate::community::DEFAULT_REPO.into(),
            author_name: String::new(),
            wheel_driver: true,
            screen_restore_token: None,
            onboard_mode_devices: Vec::new(),
            device_settings: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub profiles: Vec<Profile>,
    pub active_profile: String,
    #[serde(default)]
    pub settings: Settings,
    /// Games the user added to the library by hand (any executable).
    #[serde(default)]
    pub manual_games: Vec<crate::games::ManualGame>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            profiles: vec![Profile::desktop_default()],
            active_profile: "default".into(),
            settings: Settings::default(),
            manual_games: Vec::new(),
        }
    }
}

pub struct Store {
    path: PathBuf,
    config: Mutex<Config>,
}

impl Store {
    /// Loads the config, falling back to defaults if it is missing or corrupt.
    /// A corrupt file is kept aside rather than overwritten.
    pub fn load() -> Self {
        let path = config_path();
        let config = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::error!("config at {} is not valid JSON ({e}); keeping a backup", path.display());
                    let _ = fs::rename(&path, path.with_extension("json.bak"));
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        };
        Store { path, config: Mutex::new(config) }
    }

    pub fn get(&self) -> Config {
        self.config.lock().clone()
    }

    pub fn replace(&self, config: Config) -> Result<Config, Error> {
        {
            let mut guard = self.config.lock();
            *guard = config;
        }
        self.persist()?;
        Ok(self.get())
    }

    /// Mutates the config under the lock and writes it out.
    pub fn update<T>(&self, f: impl FnOnce(&mut Config) -> T) -> Result<T, Error> {
        let out = {
            let mut guard = self.config.lock();
            f(&mut guard)
        };
        self.persist()?;
        Ok(out)
    }

    pub fn device_profile(&self, device_id: &str) -> DeviceProfile {
        let cfg = self.config.lock();
        cfg.profiles
            .iter()
            .find(|p| p.id == cfg.active_profile)
            .and_then(|p| p.devices.get(device_id))
            .cloned()
            .unwrap_or_default()
    }

    /// Writes atomically — a truncated config on a crash would lose every profile.
    fn persist(&self) -> Result<(), Error> {
        let cfg = self.config.lock().clone();
        let text = serde_json::to_string_pretty(&cfg)
            .map_err(|e| Error::other(format!("could not serialise config: {e}")))?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| Error::other(format!("could not create {}: {e}", dir.display())))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, text)
            .map_err(|e| Error::other(format!("could not write {}: {e}", tmp.display())))?;
        fs::rename(&tmp, &self.path)
            .map_err(|e| Error::other(format!("could not replace {}: {e}", self.path.display())))?;
        Ok(())
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

/// `$XDG_CONFIG_HOME/openghub/config.json`, per the XDG base directory spec.
fn config_path() -> PathBuf {
    directories::ProjectDirs::from("com", "openghub", "OpenGHub")
        .map(|d| d.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from("openghub-config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_desktop_profile() {
        let cfg = Config::default();
        assert_eq!(cfg.active_profile, "default");
        assert_eq!(cfg.profiles[0].name, "Desktop: Default");
    }

    #[test]
    fn config_round_trips_through_json() {
        let mut cfg = Config::default();
        cfg.profiles[0].devices.insert(
            "046d:c08b:255".into(),
            DeviceProfile {
                dpi_stages: vec![400, 800, 1600],
                active_stage: 1,
                shift_stage: Some(0),
                report_rate_hz: Some(1000),
                lighting: Some(LightingSettings::default()),
                lighting_zones: Default::default(),
                zone_positions: Default::default(),
                zone_names: vec![],
                assignments: vec![],
                macros: vec![],
                wheel: None,
            },
        );
        let text = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&text).unwrap();
        let profile = &back.profiles[0].devices["046d:c08b:255"];
        assert_eq!(profile.dpi_stages, vec![400, 800, 1600]);
        assert_eq!(profile.active_stage, 1);
        assert_eq!(profile.shift_stage, Some(0));
    }

    #[test]
    fn per_zone_lighting_round_trips() {
        let mut profile = DeviceProfile::default();
        profile.lighting_zones.insert("0".into(), LightingSettings::default());
        profile.lighting_zones.insert(
            "1".into(),
            LightingSettings { color: "#ff0080".into(), ..LightingSettings::default() },
        );
        let text = serde_json::to_string(&profile).unwrap();
        let back: DeviceProfile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.lighting_zones.len(), 2);
        assert_eq!(back.lighting_zones["1"].color, "#ff0080");
    }

    #[test]
    fn configs_without_zone_lighting_still_load() {
        // Anything written before per-zone support must keep working.
        let json = r##"{"dpiStages":[800],"activeStage":0,"lighting":{"effect":"fixed","color":"#00b5e2","brightness":100,"rateMs":5000}}"##;
        let profile: DeviceProfile = serde_json::from_str(json).unwrap();
        assert!(profile.lighting_zones.is_empty());
        assert_eq!(profile.lighting.unwrap().color, "#00b5e2");
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let json = r#"{"profiles":[],"activeProfile":"x"}"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.settings.battery_poll_seconds, 60);
        assert!(cfg.settings.show_battery_notifications);
    }
}
