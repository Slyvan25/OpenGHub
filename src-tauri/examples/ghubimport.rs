//! Imports a G HUB `ProgramData\LGHUB` tree into OpenGHub's artwork directory.
//!
//!     cargo run --example ghubimport -- /path/to/LGHUBProgramData
//!     cargo run --example ghubimport -- --fetch 407f     # download a depot by product id

use openghub_lib::depot;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(String::as_str) == Some("--fetch") {
        let pids: Vec<u16> = args[1..].iter().filter_map(|a| u16::from_str_radix(a, 16).ok()).collect();
        match depot::fetch_for_product_ids(&pids) {
            Ok(d) => println!("fetched {} ({}) views {:?} thumb {}", d.display_name, d.model_id, d.views, d.has_thumbnail),
            Err(e) => println!("FAILED: {e}"),
        }
        return;
    }

    let Some(root) = args.first() else {
        println!("usage: ghubimport <ProgramData dir> | --fetch <pid hex>...");
        return;
    };
    match depot::import_program_data(std::path::Path::new(root)) {
        Ok(r) => {
            println!("build {}  depots {}  device defs {}", r.build_id, r.depots_in_depository, r.device_definitions);
            println!("artwork → {}", r.artwork_dir);
            for d in &r.imported {
                println!("  {:<28} pids {:?}  views {:?}  thumb {}",
                         d.display_name,
                         d.product_ids.iter().map(|p| format!("{p:04x}")).collect::<Vec<_>>(),
                         d.views, d.has_thumbnail);
            }
        }
        Err(e) => println!("FAILED: {e}"),
    }
}
