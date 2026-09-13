//! Dumps the games library the way the Games tab will see it.
//!
//!     cargo run --example games
fn main() {
    let db = openghub_lib::apps::AppDatabase::new();
    let games = openghub_lib::games::scan(&[], &db);
    println!("{} game(s)", games.len());
    for g in games {
        println!(
            "{:<8} {:<40} played={} mins={} cover={} app={}",
            format!("{:?}", g.source).to_lowercase(),
            g.name,
            g.last_played,
            g.playtime_minutes,
            g.cover.as_deref().map(|_| "local").unwrap_or(if g.cover_url.is_some() { "remote" } else { "none" }),
            g.application_id.as_deref().unwrap_or("-"),
        );
    }
}
