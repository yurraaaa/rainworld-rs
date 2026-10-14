//! Prints one line per room in `rw_room.py --survey` format, for diffing against it.
//! Usage: `cargo run --example room_survey -- <dir>`.

use std::path::PathBuf;

use rw_assets::{Room, Terrain};

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: room_survey <dir>");
        std::process::exit(2);
    };
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(e) => {
            eprintln!("{}: {e}", dir.display());
            std::process::exit(1);
        }
    };
    paths.sort();
    for path in paths {
        let Some(b) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !b.ends_with(".txt") || b.ends_with("_settings.txt") || b.ends_with("_arena.txt") {
            continue;
        }
        let room = match Room::load(&path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{b}: {e}");
                continue;
            }
        };
        let pv = room.tiles().iter().filter(|t| t.vertical_pole()).count();
        let ph = room.tiles().iter().filter(|t| t.horizontal_pole()).count();
        println!(
            "{b:34} {:3}x{:<3} cams={} water={:3} slope={:3} floor={:3} pole_v={pv:3} pole_h={ph:3} sc={:2} solid={}",
            room.width(),
            room.height(),
            room.cameras().len(),
            room.water_level().unwrap_or(-1),
            room.count_terrain(Terrain::Slope),
            room.count_terrain(Terrain::Floor),
            room.count_terrain(Terrain::ShortcutEntrance),
            room.count_terrain(Terrain::Solid),
        );
    }
}
