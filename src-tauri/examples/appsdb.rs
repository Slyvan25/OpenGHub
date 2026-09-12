//! Fetches Logitech's public application database and reports on it.
use openghub_lib::apps::AppDatabase;
fn main() {
    let db = AppDatabase::new();
    match db.load() {
        Ok(info) => {
            println!("version {}  apps {}  cache {}", info.version, info.application_count, info.cache_path);
            let apps = db.applications();
            let steam = apps.iter().filter(|a| !a.steam_app_ids.is_empty()).count();
            let posters = apps.iter().filter(|a| a.poster_url.is_some()).count();
            println!("steam-detectable {steam}, with posters {posters}");
            if let Some(cs) = apps.iter().find(|a| a.name == "Counter-Strike 2") {
                let c = db.commands(&cs.id).unwrap();
                println!("CS2: steam {:?}, {} commands, first: {} = {:?}",
                         cs.steam_app_ids, c.commands.len(), c.commands[0].name, c.commands[0].keystroke);
            }
            println!("running now: {:?}", db.detect_running());
        }
        Err(e) => println!("FAILED: {e}"),
    }
}
