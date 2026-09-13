//! The Games library: every installed game the launchers on this machine know
//! about, gathered the way G HUB scans Steam / Epic / GOG on Windows.
//!
//! Sources, all read-only:
//! - **Steam** — `libraryfolders.vdf` lists the library folders, each
//!   `steamapps/appmanifest_*.acf` is one installed app, and
//!   `userdata/<id>/config/localconfig.vdf` carries last-played / playtime.
//!   Portrait covers come from Steam's own `appcache/librarycache`.
//! - **Epic Games / GOG** — via Heroic's `store_cache/*_library.json`.
//! - **Lutris** — `lutris --list-games --installed --json`.
//! - **Manually installed** — executables the user added, kept in the config.
//!
//! Covers are copied into `~/.local/share/openghub/games/` so the webview can
//! load them through the asset protocol without opening every launcher's data
//! directory to it.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::hidpp::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Gog,
    Lutris,
    Manual,
}

impl Source {
    fn prefix(self) -> &'static str {
        match self {
            Source::Steam => "steam",
            Source::Epic => "epic",
            Source::Gog => "gog",
            Source::Lutris => "lutris",
            Source::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// `<source>:<launcher key>`, stable across scans.
    pub id: String,
    pub source: Source,
    pub name: String,
    /// Local cover file inside our cache, if the launcher had one.
    pub cover: Option<String>,
    /// Remote cover to fall back on when there is no local file.
    pub cover_url: Option<String>,
    /// Unix seconds; 0 when never played (or unknown).
    pub last_played: u64,
    pub playtime_minutes: u64,
    pub install_dir: Option<String>,
    /// Matching entry in Logitech's application database, when there is one —
    /// that is what links a game to a profile.
    pub application_id: Option<String>,
}

/// A game the user added by hand: any executable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManualGame {
    pub id: String,
    pub name: String,
    pub exec: String,
    #[serde(default)]
    pub args: String,
    /// Path to an image the user picked, copied into the cache on add.
    #[serde(default)]
    pub cover: Option<String>,
}

/// Cached scan result, so tab switches don't re-read every launcher.
#[derive(Default)]
pub struct Library {
    games: Mutex<Option<Vec<Game>>>,
}

impl Library {
    pub fn cached(&self) -> Option<Vec<Game>> {
        self.games.lock().clone()
    }

    pub fn set(&self, games: Vec<Game>) {
        *self.games.lock() = Some(games);
    }

    pub fn invalidate(&self) {
        *self.games.lock() = None;
    }
}

/// Where covers are cached. Must stay inside the asset protocol scope.
pub fn covers_dir() -> PathBuf {
    crate::artwork::dir()
        .parent()
        .map(|d| d.join("games"))
        .unwrap_or_else(|| PathBuf::from("openghub-games"))
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Copies a launcher's cover into our cache, skipping when the cached copy is
/// already the same size (covers change rarely; a full compare is not worth it).
fn cache_cover(src: &Path, name: &str) -> Option<String> {
    let src_len = fs::metadata(src).ok()?.len();
    if src_len == 0 {
        return None;
    }
    let dir = covers_dir();
    fs::create_dir_all(&dir).ok()?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    let dst = dir.join(format!("{name}.{ext}"));
    let fresh = fs::metadata(&dst).map(|m| m.len() == src_len).unwrap_or(false);
    if !fresh {
        fs::copy(src, &dst).ok()?;
    }
    Some(dst.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Steam
// ---------------------------------------------------------------------------

/// Minimal parser for Valve's KeyValues text format (`.vdf` / `.acf`): quoted
/// keys, quoted string values or `{ }` blocks, `//` comments.
#[derive(Debug, Clone)]
pub enum Vdf {
    Str(String),
    Map(Vec<(String, Vdf)>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(entries) => entries
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Vdf::Str(s) => Some(s.as_str()),
            Vdf::Map(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Map(e) => e,
            Vdf::Str(_) => &[],
        }
    }

    /// Follows a path of keys through nested maps.
    pub fn path(&self, keys: &[&str]) -> Option<&Vdf> {
        keys.iter().try_fold(self, |node, k| node.get(k))
    }
}

pub fn parse_vdf(text: &str) -> Vdf {
    let mut tokens = tokenize_vdf(text).into_iter().peekable();
    let mut root = Vec::new();
    parse_vdf_block(&mut tokens, &mut root);
    Vdf::Map(root)
}

#[derive(Debug, PartialEq)]
enum Tok {
    Str(String),
    Open,
    Close,
}

fn tokenize_vdf(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '"' => {
                let mut s = String::new();
                while let Some(ch) = chars.next() {
                    match ch {
                        '\\' => {
                            if let Some(e) = chars.next() {
                                s.push(match e {
                                    'n' => '\n',
                                    't' => '\t',
                                    other => other,
                                });
                            }
                        }
                        '"' => break,
                        other => s.push(other),
                    }
                }
                out.push(Tok::Str(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            // Unquoted tokens (rare, but Valve allows them).
            other => {
                let mut s = String::from(other);
                while let Some(&ch) = chars.peek() {
                    if ch.is_whitespace() || ch == '{' || ch == '}' || ch == '"' {
                        break;
                    }
                    s.push(ch);
                    chars.next();
                }
                out.push(Tok::Str(s));
            }
        }
    }
    out
}

fn parse_vdf_block(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Tok>>,
    out: &mut Vec<(String, Vdf)>,
) {
    while let Some(tok) = tokens.next() {
        match tok {
            Tok::Close => return,
            Tok::Open => {
                // Stray block without a key: consume it.
                let mut ignored = Vec::new();
                parse_vdf_block(tokens, &mut ignored);
            }
            Tok::Str(key) => match tokens.next() {
                Some(Tok::Str(value)) => out.push((key, Vdf::Str(value))),
                Some(Tok::Open) => {
                    let mut inner = Vec::new();
                    parse_vdf_block(tokens, &mut inner);
                    out.push((key, Vdf::Map(inner)));
                }
                Some(Tok::Close) | None => return,
            },
        }
    }
}

/// Candidate Steam roots; the first that exists wins per canonical path.
fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return vec![];
    };
    let candidates = [
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/debian-installation"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join("snap/steam/common/.local/share/Steam"),
    ];
    let mut seen = Vec::new();
    for c in candidates {
        if let Ok(canon) = fs::canonicalize(&c) {
            if canon.join("steamapps").is_dir() && !seen.contains(&canon) {
                seen.push(canon);
            }
        }
    }
    seen
}

/// Steam's runtime tooling shows up as installed apps; G HUB lists games only.
fn is_steam_tool(name: &str, installdir: &str) -> bool {
    let n = name.trim();
    n.starts_with("Proton")
        || n.starts_with("Steam Linux Runtime")
        || n.starts_with("Steamworks Common")
        || n == "SteamVR"
        || installdir == "Steamworks Shared"
        || installdir.starts_with("SteamLinuxRuntime")
}

fn steam_play_stats(root: &Path) -> HashMap<String, (u64, u64)> {
    let mut stats: HashMap<String, (u64, u64)> = HashMap::new();
    let Ok(users) = fs::read_dir(root.join("userdata")) else {
        return stats;
    };
    for user in users.flatten() {
        let cfg = user.path().join("config/localconfig.vdf");
        let Ok(text) = fs::read_to_string(&cfg) else {
            continue;
        };
        let vdf = parse_vdf(&text);
        let Some(apps) = vdf.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])
        else {
            continue;
        };
        for (appid, entry) in apps.entries() {
            let last = entry.str("LastPlayed").and_then(|s| s.parse().ok()).unwrap_or(0);
            let mins = entry.str("Playtime").and_then(|s| s.parse().ok()).unwrap_or(0);
            let slot = stats.entry(appid.clone()).or_default();
            slot.0 = slot.0.max(last);
            slot.1 = slot.1.max(mins);
        }
    }
    stats
}

fn steam_cover(root: &Path, appid: &str) -> Option<PathBuf> {
    let cache = root.join("appcache/librarycache");
    // Newer clients keep one directory per app, older ones flat files.
    let candidates = [
        cache.join(appid).join("library_600x900.jpg"),
        cache.join(format!("{appid}_library_600x900.jpg")),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

pub fn scan_steam() -> Vec<Game> {
    let mut games = Vec::new();
    let mut seen_ids = Vec::new();
    for root in steam_roots() {
        let stats = steam_play_stats(&root);
        let folders_file = root.join("steamapps/libraryfolders.vdf");
        let mut libraries: Vec<PathBuf> = vec![root.join("steamapps")];
        if let Ok(text) = fs::read_to_string(&folders_file) {
            let vdf = parse_vdf(&text);
            if let Some(folders) = vdf.get("libraryfolders") {
                for (_, folder) in folders.entries() {
                    if let Some(path) = folder.str("path") {
                        let apps = PathBuf::from(path).join("steamapps");
                        if apps.is_dir() && !libraries.contains(&apps) {
                            libraries.push(apps);
                        }
                    }
                }
            }
        }
        for lib in libraries {
            let Ok(entries) = fs::read_dir(&lib) else {
                continue; // unmounted external drive
            };
            for entry in entries.flatten() {
                let file = entry.file_name().to_string_lossy().into_owned();
                if !file.starts_with("appmanifest_") || !file.ends_with(".acf") {
                    continue;
                }
                let Ok(text) = fs::read_to_string(entry.path()) else {
                    continue;
                };
                let vdf = parse_vdf(&text);
                let Some(state) = vdf.get("AppState") else {
                    continue;
                };
                let (Some(appid), Some(name)) = (state.str("appid"), state.str("name")) else {
                    continue;
                };
                let installdir = state.str("installdir").unwrap_or("");
                if is_steam_tool(name, installdir) || seen_ids.contains(&appid.to_string()) {
                    continue;
                }
                // StateFlags 4 = fully installed; anything else is mid-download.
                let flags: u32 = state.str("StateFlags").and_then(|s| s.parse().ok()).unwrap_or(4);
                if flags & 4 == 0 {
                    continue;
                }
                seen_ids.push(appid.to_string());
                let (mut last, mins) = stats.get(appid).copied().unwrap_or((0, 0));
                if let Some(v) = state.str("LastPlayed").and_then(|s| s.parse::<u64>().ok()) {
                    last = last.max(v);
                }
                let cover = steam_cover(&root, appid)
                    .and_then(|p| cache_cover(&p, &format!("steam-{appid}")));
                games.push(Game {
                    id: format!("steam:{appid}"),
                    source: Source::Steam,
                    name: name.to_string(),
                    cover,
                    cover_url: Some(format!(
                        "https://shared.steamstatic.com/store_item_assets/steam/apps/{appid}/library_600x900.jpg"
                    )),
                    last_played: last,
                    playtime_minutes: mins,
                    install_dir: Some(lib.join("common").join(installdir).to_string_lossy().into_owned()),
                    application_id: None,
                });
            }
        }
    }
    games
}

// ---------------------------------------------------------------------------
// Heroic (Epic Games + GOG)
// ---------------------------------------------------------------------------

fn heroic_config_dirs() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return vec![];
    };
    [
        home.join(".config/heroic"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
    ]
    .into_iter()
    .filter(|p| p.is_dir())
    .collect()
}

#[derive(Deserialize)]
struct HeroicEntry {
    app_name: String,
    title: String,
    #[serde(default)]
    is_installed: bool,
    #[serde(default)]
    art_cover: Option<String>,
    #[serde(default)]
    art_square: Option<String>,
    #[serde(default)]
    install: Option<HeroicInstall>,
}

#[derive(Deserialize)]
struct HeroicInstall {
    #[serde(default)]
    install_path: Option<String>,
}

fn heroic_entries(file: &Path, key: &str) -> Vec<HeroicEntry> {
    let Ok(text) = fs::read_to_string(file) else {
        return vec![];
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return vec![];
    };
    json.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

pub fn scan_heroic() -> Vec<Game> {
    let mut games = Vec::new();
    for dir in heroic_config_dirs() {
        let sources = [
            (Source::Epic, dir.join("store_cache/legendary_library.json"), "library"),
            (Source::Gog, dir.join("store_cache/gog_library.json"), "games"),
        ];
        for (source, file, key) in sources {
            for e in heroic_entries(&file, key) {
                if !e.is_installed || games.iter().any(|g: &Game| g.id == format!("{}:{}", source.prefix(), e.app_name)) {
                    continue;
                }
                games.push(Game {
                    id: format!("{}:{}", source.prefix(), e.app_name),
                    source,
                    name: e.title,
                    cover: None,
                    cover_url: e.art_cover.or(e.art_square),
                    last_played: 0,
                    playtime_minutes: 0,
                    install_dir: e.install.and_then(|i| i.install_path),
                    application_id: None,
                });
            }
        }
    }
    games
}

// ---------------------------------------------------------------------------
// Lutris
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LutrisEntry {
    slug: String,
    name: String,
    #[serde(default)]
    runner: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    playtime_seconds: Option<f64>,
    #[serde(default)]
    lastplayed: Option<String>,
    #[serde(default)]
    cover_path: Option<String>,
}

/// Lutris prints `YYYY-MM-DD HH:MM:SS` in local time; treated as UTC, which is
/// close enough for ordering.
fn parse_lutris_time(s: &str) -> u64 {
    let mut parts = s.split(|c| c == '-' || c == ' ' || c == ':');
    let mut next = || parts.next().and_then(|p| p.trim().parse::<u64>().ok());
    let (Some(y), Some(m), Some(d)) = (next(), next(), next()) else {
        return 0;
    };
    let (h, mi, sec) = (next().unwrap_or(0), next().unwrap_or(0), next().unwrap_or(0));
    // Days from civil, Howard Hinnant's algorithm.
    let (y, m) = if m <= 2 { (y as i64 - 1, m as i64 + 12) } else { (y as i64, m as i64) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86400 + (h * 3600 + mi * 60 + sec) as i64).max(0) as u64
}

fn lutris_command() -> Option<Command> {
    if which("lutris") {
        return Some(Command::new("lutris"));
    }
    if which("flatpak") {
        let mut c = Command::new("flatpak");
        c.args(["run", "net.lutris.Lutris"]);
        return Some(c);
    }
    None
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

pub fn scan_lutris() -> Vec<Game> {
    let Some(mut cmd) = lutris_command() else {
        return vec![];
    };
    let output = cmd
        .args(["--list-games", "--installed", "--json"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return vec![];
    };
    let entries: Vec<LutrisEntry> = serde_json::from_slice(&output.stdout).unwrap_or_default();
    entries
        .into_iter()
        // Lutris can mirror the Steam library; those are already listed.
        .filter(|e| e.runner.as_deref() != Some("steam"))
        .map(|e| {
            let cover = e
                .cover_path
                .as_deref()
                .map(Path::new)
                .filter(|p| p.is_file())
                .and_then(|p| cache_cover(p, &format!("lutris-{}", e.slug)));
            Game {
                id: format!("lutris:{}", e.slug),
                source: Source::Lutris,
                name: e.name,
                cover,
                cover_url: None,
                last_played: e.lastplayed.as_deref().map(parse_lutris_time).unwrap_or(0),
                playtime_minutes: (e.playtime_seconds.unwrap_or(0.0) / 60.0) as u64,
                install_dir: e.directory.filter(|d| !d.is_empty()),
                application_id: None,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Manual
// ---------------------------------------------------------------------------

pub fn manual_games(list: &[ManualGame]) -> Vec<Game> {
    list.iter()
        .map(|m| Game {
            id: format!("manual:{}", m.id),
            source: Source::Manual,
            name: m.name.clone(),
            cover: m.cover.clone().filter(|c| Path::new(c).is_file()),
            cover_url: None,
            last_played: 0,
            playtime_minutes: 0,
            install_dir: Path::new(&m.exec)
                .parent()
                .map(|p| p.to_string_lossy().into_owned()),
            application_id: None,
        })
        .collect()
}

/// Builds a manual entry, copying the chosen cover into the cache.
pub fn new_manual_game(name: &str, exec: &str, args: &str, cover: Option<&str>) -> Result<ManualGame> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("the game needs a name"));
    }
    let exec_path = Path::new(exec);
    if !exec_path.is_file() {
        return Err(Error::other(format!("{exec} is not a file")));
    }
    let id = format!("{}-{}", slug(name), now());
    let cover = cover
        .map(Path::new)
        .filter(|p| p.is_file())
        .and_then(|p| cache_cover(p, &format!("manual-{id}")));
    Ok(ManualGame {
        id,
        name: name.to_string(),
        exec: exec.to_string(),
        args: args.trim().to_string(),
        cover,
    })
}

fn slug(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s.trim_matches('-').chars().take(40).collect()
}

// ---------------------------------------------------------------------------
// Scan + launch
// ---------------------------------------------------------------------------

/// Everything installed, newest-played first. Logitech application ids are
/// matched by Steam app id where possible, otherwise by exact name.
pub fn scan(manual: &[ManualGame], db: &crate::apps::AppDatabase) -> Vec<Game> {
    let mut games = scan_steam();
    games.extend(scan_heroic());
    games.extend(scan_lutris());
    games.extend(manual_games(manual));

    // The database loads in the background at startup; a scan that runs before
    // it lands would leave every game unlinked, so make sure it is there.
    if db.info().is_none() {
        if let Err(e) = db.load() {
            log::warn!("application database unavailable for game matching: {e}");
        }
    }
    let apps = db.applications();
    let by_steam: HashMap<&str, &str> = apps
        .iter()
        .flat_map(|a| a.steam_app_ids.iter().map(move |s| (s.as_str(), a.id.as_str())))
        .collect();
    let by_name: HashMap<String, &str> = apps
        .iter()
        .map(|a| (a.name.to_lowercase(), a.id.as_str()))
        .collect();
    for g in &mut games {
        let steam_id = g.id.strip_prefix("steam:");
        g.application_id = steam_id
            .and_then(|s| by_steam.get(s))
            .or_else(|| by_name.get(&g.name.to_lowercase()))
            .map(|s| s.to_string());
    }

    games.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.name.cmp(&b.name)));
    games
}

fn spawn_detached(mut cmd: Command) -> Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| Error::other(format!("could not start launcher: {e}")))
}

/// Opens a URL with the desktop's handler; the launchers register their own
/// schemes (`steam://`, `heroic://`, `lutris:`).
fn open_url(url: &str) -> Result<()> {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(url);
    spawn_detached(cmd)
}

/// Starts a game through the launcher that owns it.
pub fn launch(game_id: &str, manual: &[ManualGame]) -> Result<()> {
    let (source, key) = game_id
        .split_once(':')
        .ok_or_else(|| Error::other("malformed game id"))?;
    match source {
        "steam" => {
            if !key.chars().all(|c| c.is_ascii_digit()) {
                return Err(Error::other("malformed Steam app id"));
            }
            if which("steam") {
                let mut cmd = Command::new("steam");
                cmd.arg(format!("steam://rungameid/{key}"));
                spawn_detached(cmd)
            } else {
                open_url(&format!("steam://rungameid/{key}"))
            }
        }
        "lutris" => {
            let mut cmd = lutris_command().ok_or_else(|| Error::other("Lutris is not installed"))?;
            cmd.arg(format!("lutris:rungame/{key}"));
            spawn_detached(cmd)
        }
        "epic" => open_url(&format!("heroic://launch/legendary/{key}")),
        "gog" => open_url(&format!("heroic://launch/gog/{key}")),
        "manual" => {
            let m = manual
                .iter()
                .find(|m| m.id == key)
                .ok_or_else(|| Error::other("unknown game"))?;
            let mut cmd = Command::new(&m.exec);
            if !m.args.is_empty() {
                cmd.args(m.args.split_whitespace());
            }
            if let Some(dir) = Path::new(&m.exec).parent() {
                cmd.current_dir(dir);
            }
            spawn_detached(cmd)
        }
        _ => Err(Error::other("unknown game source")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_appmanifest() {
        let text = r#"
"AppState"
{
	"appid"		"250820"
	"name"		"SteamVR"
	"StateFlags"		"4"
	"installdir"		"SteamVR"
	"UserConfig"
	{
		"language"		"english"
	}
	// trailing comment
}
"#;
        let vdf = parse_vdf(text);
        let state = vdf.get("AppState").unwrap();
        assert_eq!(state.str("appid"), Some("250820"));
        assert_eq!(state.str("name"), Some("SteamVR"));
        assert_eq!(state.path(&["UserConfig", "language"]).is_some(), true);
        assert_eq!(state.get("UserConfig").unwrap().str("language"), Some("english"));
    }

    #[test]
    fn parses_libraryfolders() {
        let text = r#""libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
		"apps"
		{
			"228980"		"443244413"
		}
	}
	"1"
	{
		"path"		"/run/media/u/gaming/SteamLibrary"
	}
}"#;
        let vdf = parse_vdf(text);
        let folders = vdf.get("libraryfolders").unwrap();
        let paths: Vec<_> = folders.entries().iter().filter_map(|(_, f)| f.str("path")).collect();
        assert_eq!(paths, ["/home/u/.local/share/Steam", "/run/media/u/gaming/SteamLibrary"]);
    }

    #[test]
    fn filters_steam_tooling() {
        assert!(is_steam_tool("Proton 9.0 (Beta)", "Proton 9.0"));
        assert!(is_steam_tool("Steam Linux Runtime 3.0 (sniper)", "SteamLinuxRuntime_sniper"));
        assert!(is_steam_tool("Steamworks Common Redistributables", "Steamworks Shared"));
        assert!(!is_steam_tool("Counter-Strike 2", "Counter-Strike Global Offensive"));
    }

    #[test]
    fn lutris_timestamps() {
        assert_eq!(parse_lutris_time("1970-01-01 00:00:00"), 0);
        assert_eq!(parse_lutris_time("2024-07-05 21:14:10"), 1720214050);
        assert_eq!(parse_lutris_time("garbage"), 0);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Tempest Rising!"), "tempest-rising");
        assert_eq!(slug("  --A  B-- "), "a-b");
    }
}
