//! Logitech's public application database.
//!
//! G HUB keys profiles on the running game and ships per-game command sets
//! (the COMMANDS tab). Both come from a public, unauthenticated channel:
//!
//! ```text
//! v1/channels/public/update_apps.json   → { version, applicationPath }
//! assets/<version>/applications.json    → 846 applications
//! images/<hash>/<name>_poster.jpg       → poster artwork
//! ```
//!
//! This module fetches and caches it, and detects the running game. On Linux
//! Steam covers 816 of the 846 entries and exports `SteamAppId` into every game
//! process it launches, so detection reads `/proc/*/environ` rather than asking
//! the compositor which window is focused — which Wayland will not tell us.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::hidpp::Error;

const CHANNEL_HOST: &str = "https://gamesapps-assets.ghub.logitechg.com";
const CHANNEL_MANIFEST: &str = "v1/channels/public/update_apps.json";
/// How stale the on-disk copy may be before it is refreshed on startup.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

// ---------------------------------------------------------------------------
// Wire format (only the fields we use)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelManifest {
    version: String,
    application_path: String,
}

#[derive(Debug, Deserialize)]
struct Database {
    applications: Vec<RawApplication>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawApplication {
    application_id: String,
    name: String,
    // The feed mixes conventions: everything is camelCase except this one.
    #[serde(default, rename = "poster_url")]
    poster_url: Option<String>,
    #[serde(default)]
    detection: Vec<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    commands: Vec<RawCommand>,
    #[serde(default)]
    category_colors: Vec<CategoryColor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawCommand {
    pub category: String,
    pub name: String,
    #[serde(default)]
    pub keystroke: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CategoryColor {
    pub hex: String,
    pub tag: String,
}

// ---------------------------------------------------------------------------
// What the app works with
// ---------------------------------------------------------------------------

/// Compact entry for the picker; commands are fetched separately per app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: String,
    pub name: String,
    /// Absolute poster URL, when the entry has one.
    pub poster_url: Option<String>,
    pub steam_app_ids: Vec<String>,
    /// Executable basenames from Windows-oriented rules, lowercase, used as a
    /// weaker fallback for non-Steam launches (Proton, Lutris, Heroic).
    pub executables: Vec<String>,
    pub command_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationCommands {
    pub id: String,
    pub name: String,
    pub commands: Vec<RawCommand>,
    pub category_colors: Vec<CategoryColor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub version: String,
    pub application_count: usize,
    pub fetched_at: u64,
    pub cache_path: String,
}

struct Loaded {
    version: String,
    fetched_at: SystemTime,
    apps: Vec<Application>,
    commands: HashMap<String, ApplicationCommands>,
    by_steam_id: HashMap<String, String>,
    by_executable: HashMap<String, String>,
}

/// In-memory database plus its on-disk cache.
pub struct AppDatabase {
    inner: RwLock<Option<Loaded>>,
}

fn cache_dir() -> PathBuf {
    crate::artwork::dir()
        .parent()
        .map(|d| d.join("apps"))
        .unwrap_or_else(|| PathBuf::from("openghub-apps"))
}

impl Default for AppDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl AppDatabase {
    pub fn new() -> Self {
        AppDatabase { inner: RwLock::new(None) }
    }

    /// Loads the cached copy if fresh enough, otherwise fetches. Never fails
    /// hard: with no cache and no network the database is simply empty.
    pub fn load(&self) -> Result<DatabaseInfo, Error> {
        if let Some(info) = self.load_cache()? {
            let age = SystemTime::now().duration_since(info.1).unwrap_or(MAX_AGE);
            if age < MAX_AGE {
                return Ok(info.0);
            }
        }
        match self.refresh() {
            Ok(info) => Ok(info),
            Err(e) => {
                // Stale is better than nothing.
                if let Some((info, _)) = self.load_cache()? {
                    log::warn!("could not refresh application database ({e}); using cached copy");
                    return Ok(info);
                }
                Err(e)
            }
        }
    }

    /// Downloads the current database and writes it to the cache.
    pub fn refresh(&self) -> Result<DatabaseInfo, Error> {
        let manifest: ChannelManifest = ureq::get(&format!("{CHANNEL_HOST}/{CHANNEL_MANIFEST}"))
            .timeout(Duration::from_secs(20))
            .call()
            .map_err(|e| Error::other(format!("channel manifest: {e}")))?
            .into_json()
            .map_err(|e| Error::other(format!("channel manifest is not JSON: {e}")))?;

        let url = format!("{CHANNEL_HOST}/{}", manifest.application_path);
        let body = ureq::get(&url)
            .timeout(Duration::from_secs(60))
            .call()
            .map_err(|e| Error::other(format!("application database: {e}")))?
            .into_string()
            .map_err(|e| Error::other(format!("application database body: {e}")))?;

        let dir = cache_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::other(format!("could not create {}: {e}", dir.display())))?;
        std::fs::write(dir.join("applications.json"), &body)
            .map_err(|e| Error::other(format!("could not cache database: {e}")))?;
        std::fs::write(dir.join("version"), &manifest.version)
            .map_err(|e| Error::other(format!("could not cache version: {e}")))?;

        self.install(&body, manifest.version, SystemTime::now())
    }

    fn load_cache(&self) -> Result<Option<(DatabaseInfo, SystemTime)>, Error> {
        let dir = cache_dir();
        let path = dir.join("applications.json");
        let Ok(meta) = std::fs::metadata(&path) else {
            return Ok(None);
        };
        let fetched_at = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let body = std::fs::read_to_string(&path)
            .map_err(|e| Error::other(format!("could not read {}: {e}", path.display())))?;
        let version = std::fs::read_to_string(dir.join("version")).unwrap_or_default();
        Ok(Some((self.install(&body, version, fetched_at)?, fetched_at)))
    }

    fn install(&self, body: &str, version: String, fetched_at: SystemTime) -> Result<DatabaseInfo, Error> {
        let db: Database = serde_json::from_str(body)
            .map_err(|e| Error::other(format!("application database is malformed: {e}")))?;

        let mut apps = Vec::with_capacity(db.applications.len());
        let mut commands = HashMap::new();
        let mut by_steam_id = HashMap::new();
        let mut by_executable = HashMap::new();

        let mut seen_ids = std::collections::HashSet::new();
        for raw in db.applications {
            // The feed carries a couple of quirks: untranslated placeholder
            // entries (`APPLICATION_NAME_DESKTOP` — G HUB's built-in Desktop
            // pseudo-app) and an id shared by CS:GO and CS2. Keyed UI lists
            // choke on duplicates, so keep the first of each id and drop the
            // placeholders; Desktop is built into OpenGHub anyway.
            if raw.name.starts_with("APPLICATION_NAME_") || !seen_ids.insert(raw.application_id.clone()) {
                continue;
            }
            let mut steam_app_ids = Vec::new();
            let mut executables = Vec::new();
            for rule in &raw.detection {
                if let Some(steam) = rule.get("steam") {
                    if let Some(id) = steam.get("appId").and_then(|v| v.as_str()) {
                        steam_app_ids.push(id.to_string());
                        by_steam_id.insert(id.to_string(), raw.application_id.clone());
                    }
                }
                // Windows-flavoured rules still name the executable, which
                // Proton launches under the same basename.
                for key in ["winRegistry", "glob"] {
                    let exe = match rule.get(key) {
                        Some(serde_json::Value::Object(o)) => {
                            o.get("executable").and_then(|v| v.as_str()).map(str::to_string)
                        }
                        Some(serde_json::Value::String(s)) => {
                            s.rsplit(['\\', '/']).next().map(str::to_string)
                        }
                        _ => None,
                    };
                    if let Some(exe) = exe {
                        let exe = exe.to_ascii_lowercase();
                        if !exe.contains('*') {
                            by_executable.insert(exe.clone(), raw.application_id.clone());
                            executables.push(exe);
                        }
                    }
                }
            }

            commands.insert(
                raw.application_id.clone(),
                ApplicationCommands {
                    id: raw.application_id.clone(),
                    name: raw.name.clone(),
                    commands: raw.commands.clone(),
                    category_colors: raw.category_colors.clone(),
                },
            );
            apps.push(Application {
                id: raw.application_id,
                name: raw.name,
                poster_url: raw.poster_url.map(|p| format!("{CHANNEL_HOST}/{p}")),
                steam_app_ids,
                executables,
                command_count: raw.commands.len(),
            });
        }
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        let info = DatabaseInfo {
            version: version.clone(),
            application_count: apps.len(),
            fetched_at: fetched_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            cache_path: cache_dir().join("applications.json").display().to_string(),
        };
        *self.inner.write() =
            Some(Loaded { version, fetched_at, apps, commands, by_steam_id, by_executable });
        Ok(info)
    }

    pub fn applications(&self) -> Vec<Application> {
        self.inner.read().as_ref().map(|l| l.apps.clone()).unwrap_or_default()
    }

    pub fn commands(&self, id: &str) -> Option<ApplicationCommands> {
        self.inner.read().as_ref().and_then(|l| l.commands.get(id).cloned())
    }

    pub fn info(&self) -> Option<DatabaseInfo> {
        let guard = self.inner.read();
        let l = guard.as_ref()?;
        Some(DatabaseInfo {
            version: l.version.clone(),
            application_count: l.apps.len(),
            fetched_at: l
                .fetched_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            cache_path: cache_dir().join("applications.json").display().to_string(),
        })
    }

    /// Which known application is running right now, if any.
    ///
    /// Steam first: it exports `SteamAppId` into every process it launches, and
    /// that is authoritative. Then executable basename, for games launched some
    /// other way. Returns the application id.
    pub fn detect_running(&self) -> Option<String> {
        let guard = self.inner.read();
        let l = guard.as_ref()?;
        let mut by_exe: Option<String> = None;

        for entry in std::fs::read_dir("/proc").ok()?.flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().filter(|s| s.chars().all(|c| c.is_ascii_digit())) else {
                continue;
            };
            let base = PathBuf::from("/proc").join(pid);

            if let Ok(environ) = std::fs::read(base.join("environ")) {
                for var in environ.split(|b| *b == 0) {
                    if let Some(id) = var.strip_prefix(b"SteamAppId=") {
                        if let Ok(id) = std::str::from_utf8(id) {
                            if let Some(app) = l.by_steam_id.get(id) {
                                return Some(app.clone());
                            }
                        }
                    }
                }
            }

            if by_exe.is_none() {
                if let Ok(comm) = std::fs::read_to_string(base.join("comm")) {
                    let comm = comm.trim().to_ascii_lowercase();
                    // `comm` is truncated to 15 chars; match on prefix of the exe name.
                    if let Some((_, app)) = l
                        .by_executable
                        .iter()
                        .find(|(exe, _)| exe.starts_with(&comm) && comm.len() >= 6)
                    {
                        by_exe = Some(app.clone());
                    }
                }
            }
        }
        by_exe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{"applications":[
      {"applicationId":"cs2","name":"Counter-Strike 2","poster_url":"images/x/cs2.jpg",
       "detection":[{"steam":{"appId":"730"}}],
       "commands":[{"category":"Interface","keystroke":["I"],"name":"Toggle Inventory"}],
       "categoryColors":[{"hex":"#fff","tag":"Interface"}]},
      {"applicationId":"tf2","name":"Titanfall 2",
       "detection":[{"winRegistry":{"executable":"TITANFALL2.EXE","registryKey":"x","registryPath":"y"}}],
       "commands":[]},
      {"applicationId":"r6","name":"Rainbow Six",
       "detection":[{"glob":"%LOCALAPPDATA%\\\\Ubisoft\\\\r6*\\\\RainbowSix.exe"}],
       "commands":[]}
    ]}"##;

    #[test]
    fn parses_the_public_database_shape() {
        let db = AppDatabase::new();
        let info = db.install(SAMPLE, "test".into(), SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(info.application_count, 3);

        let apps = db.applications();
        let cs2 = apps.iter().find(|a| a.id == "cs2").unwrap();
        assert_eq!(cs2.steam_app_ids, vec!["730"]);
        assert_eq!(cs2.command_count, 1);
        assert_eq!(
            cs2.poster_url.as_deref(),
            Some("https://gamesapps-assets.ghub.logitechg.com/images/x/cs2.jpg")
        );

        // Executable rules are lowercased and stripped to a basename.
        let tf2 = apps.iter().find(|a| a.id == "tf2").unwrap();
        assert_eq!(tf2.executables, vec!["titanfall2.exe"]);
        let r6 = apps.iter().find(|a| a.id == "r6").unwrap();
        assert_eq!(r6.executables, vec!["rainbowsix.exe"]);
    }

    #[test]
    fn duplicate_ids_and_placeholders_are_dropped() {
        // Mirrors the real feed: CS:GO and CS2 share an id, and the Desktop
        // pseudo-app appears twice under an untranslated name.
        let json = r##"{"applications":[
          {"applicationId":"cs","name":"Counter-Strike 2","detection":[],"commands":[]},
          {"applicationId":"cs","name":"Counter-Strike: Global Offensive","detection":[],"commands":[]},
          {"applicationId":"dt","name":"APPLICATION_NAME_DESKTOP","detection":[],"commands":[]},
          {"applicationId":"dt","name":"APPLICATION_NAME_DESKTOP","detection":[],"commands":[]},
          {"applicationId":"ow","name":"Overwatch 2","detection":[],"commands":[]}
        ]}"##;
        let db = AppDatabase::new();
        db.install(json, "t".into(), SystemTime::UNIX_EPOCH).unwrap();
        let apps = db.applications();
        let ids: Vec<_> = apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["cs", "ow"]);
        assert_eq!(apps[0].name, "Counter-Strike 2", "first occurrence wins");
    }

    #[test]
    fn commands_are_available_per_application() {
        let db = AppDatabase::new();
        db.install(SAMPLE, "test".into(), SystemTime::UNIX_EPOCH).unwrap();
        let cmds = db.commands("cs2").unwrap();
        assert_eq!(cmds.commands[0].name, "Toggle Inventory");
        assert_eq!(cmds.commands[0].keystroke, vec!["I"]);
        assert!(db.commands("nope").is_none());
    }

    #[test]
    fn applications_are_sorted_by_name() {
        let db = AppDatabase::new();
        db.install(SAMPLE, "test".into(), SystemTime::UNIX_EPOCH).unwrap();
        let names: Vec<_> = db.applications().into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["Counter-Strike 2", "Rainbow Six", "Titanfall 2"]);
    }
}
