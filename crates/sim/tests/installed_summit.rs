//! Physics on the installed `Summit` test room (read-only). Set `RW_DIR` to the game
//! install directory (the one containing `RainWorld_Data`); skipped if absent.
//! Expected slope classes come from `rainworld-re/notes/slope-collision.md` section 8.

use std::path::PathBuf;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rw_assets::{Room, Slope, Terrain, tile_position};
use rw_sim::{PhysicalObject, determine_safe_spawn_tile, place_in_room};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn summit() -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("levels").is_dir() {
        eprintln!("skipping: no levels dir under {}", sa.display());
        return None;
    }
    Some(Room::load_level(&sa, "Summit").expect("summit loads"))
}

#[test]
fn summit_slopes_identify_by_neighbour_rule() {
    let Some(room) = summit() else { return };
    let mut slopes: Vec<((i32, i32), Option<Slope>)> = Vec::new();
    for x in 0..room.width() {
        for y in 0..room.height() {
            if room.tile(x, y).terrain == Terrain::Slope {
                slopes.push(((x, y), room.identify_slope(x, y)));
            }
        }
    }
    // Hand-applied rule (slope-collision.md section 1) to the ASCII dump of summit.txt
    // (`rw_room.py --ascii`): (10,13) has right (11,13) and above (10,14) Solid, below
    // Air => DownLeft; (11,12) has right (12,12) and above (11,13) Solid, below (11,11)
    // Air => DownLeft. Both are the diagonal underside of the left block; the right
    // block mirrors them (x -> 47 - x) with the wall on the left => DownRight.
    assert_eq!(
        slopes,
        vec![
            ((10, 13), Some(Slope::DownLeft)),
            ((11, 12), Some(Slope::DownLeft)),
            ((36, 12), Some(Slope::DownRight)),
            ((37, 13), Some(Slope::DownRight)),
        ]
    );
}

#[test]
fn summit_spawned_slugcat_comes_to_rest_on_block_top() {
    let Some(room) = summit() else { return };
    // These seeds spawn on the open top of a block ((11,17) or (36,17), row 16 Solid).
    // Surface friction is the player's not-conscious 0.3: with the conscious 0.5 a
    // flat landing multiplies vel.x by clamp(2 * 0.5, 0, 1) = 1, so without the
    // Player ground friction (out of scope) the body slides off the block.
    for seed in [1, 3, 7] {
        let mut rng = StdRng::seed_from_u64(seed);
        let tile = determine_safe_spawn_tile(&room, &mut rng);
        // The spawn predicate: inside, y > 0, not Solid, three Solid tiles below.
        assert!(room.contains(tile.0, tile.1) && tile.1 > 0);
        assert!(!room.tile(tile.0, tile.1).is_solid());
        assert!((-1..=1).all(|dx| room.tile(tile.0 + dx, tile.1 - 1).is_solid()));
        assert_eq!(
            room.tile(tile.0, tile.1).terrain,
            Terrain::Air,
            "seed {seed}"
        );
        assert_eq!(tile.1, 17, "seed {seed}");

        let mut body = PhysicalObject::slugcat();
        body.params.surface_friction = 0.3;
        place_in_room(&mut body, tile, &mut rng);
        for _ in 0..400 {
            body.update(&room);
        }
        for (i, c) in body.chunks.iter().enumerate() {
            let ctx = format!("seed {seed} spawn {tile:?} chunk {i}: {c:?}");
            assert_eq!(c.contact().y, -1, "{ctx}");
            assert_eq!(c.on_slope(), 0, "{ctx}");
            // Flat-floor snap on row 16: (16 + 1) * 20 + rad.
            assert_eq!(c.pos.y, 340.0 + c.rad, "{ctx}");
            // Supported: the vertical sweep covers columns under x +/- (rad - 1).
            let (x0, ty) = tile_position([c.pos.x - c.rad + 1.0, c.pos.y - c.rad - 1.0]);
            let (x1, _) = tile_position([c.pos.x + c.rad - 1.0, 0.0]);
            assert_eq!(ty, 16, "{ctx}");
            assert!((x0..=x1).any(|x| room.tile(x, ty).is_solid()), "{ctx}");
            assert!(c.vel.length() < 1e-3, "{ctx}");
        }
        let d = body.chunks[0].pos.distance(body.chunks[1].pos);
        assert!((d - 17.0).abs() < 1e-3, "seed {seed}: rod length {d}");
    }
}
