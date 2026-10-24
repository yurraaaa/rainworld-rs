//! `RoomRain`'s `rainReach` and `splashTiles` on the installed Outskirts room `SU_A14`
//! (read-only). Set `RW_DIR` to the game install directory; skipped if absent. The
//! expectation is read from `rw_assets::RainMask`, the other output of `GenerateShelterTex`:
//! a column's sky texels (red 255) are the tiles above its first solid tile, so the lowest
//! one is `rainReach + 1`; a column without any has its top row solid, one with the whole
//! column has no solid tile and `rainReach` 0. The room's tunnel (row 20, solid rows 19
//! and 21, columns 15 to 32; `corridors.md`) is under a ceiling: reach 21 or more.

use std::path::PathBuf;

use rw_assets::{DangerType, RainMask, Room};
use rw_sim::world::RoomRain;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn su_a14() -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let path = root
        .join("RainWorld_Data")
        .join("StreamingAssets")
        .join("world")
        .join("su-rooms")
        .join("su_a14.txt");
    if !path.is_file() {
        eprintln!("skipping: no {}", path.display());
        return None;
    }
    Some(Room::load(&path).expect("su_a14 loads"))
}

#[test]
fn su_a14_rain_reach_and_splash_tiles_match_the_sky_texels() {
    let Some(room) = su_a14() else { return };
    let rain = RoomRain::new(0, &room, DangerType::Rain, false);
    let mask = RainMask::generate(&room);
    let (w, h) = (room.width(), room.height());
    assert_eq!(rain.rain_reach.len(), w as usize);
    let mut splash = Vec::new();
    for x in 0..w {
        let lowest_sky = (0..h).find(|&y| mask.texel(x, y)[0] == 255);
        let want = match lowest_sky {
            None => h - 1,
            Some(0) => 0,
            Some(y) => y - 1,
        };
        assert_eq!(rain.rain_reach[x as usize], want, "column {x}");
        if lowest_sky != Some(0) && want < h - 1 {
            splash.push((x, want));
        }
    }
    assert_eq!(rain.splash_tiles, splash);
    assert!(!splash.is_empty());
    for x in 15..=32 {
        assert!(rain.rain_reach[x] >= 21, "tunnel column {x}");
    }
    assert_eq!(
        rain.splashes,
        splash.len() as i32 * 2 / room.cameras().len() as i32
    );
}
