//! Exercises the community client against a repository URL.
//!     cargo run --example community -- http://127.0.0.1:4711
use openghub_lib::community;
fn main() {
    let repo = std::env::args().nth(1).unwrap_or_else(|| community::DEFAULT_REPO.to_string());
    match community::fetch_index(&repo, true) {
        Ok(idx) => {
            println!("index v{} — {} profile(s)", idx.version, idx.profiles.len());
            for e in &idx.profiles {
                println!("  {} by {} for {} ({} macros, {} bindings)", e.name, e.author, e.device.display_name, e.macro_count, e.assignment_count);
                match community::fetch_profile(&repo, &e.path) {
                    Ok(p) => println!("    fetched + validated: dpi {:?}, zones {}, format {}", p.profile.dpi_stages, p.profile.lighting_zones.len(), p.format),
                    Err(err) => println!("    FAILED: {err}"),
                }
            }
        }
        Err(e) => println!("index FAILED: {e}"),
    }
}
