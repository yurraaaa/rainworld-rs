//! The seed-1 spawn in the installed Outskirts room `SU_A22` (read-only). Set `RW_DIR`
//! to the game install directory; skipped if absent. The spawn tile (12, 16) is a
//! shortcut entrance at the end of a one-tile tunnel: Solid at (11, 16), (11..=13, 15)
//! and (11..=13, 17), open from column 14. A ShortcutEntrance is not solid to
//! `IsTileSolid`, so the game's `MovementUpdate` counts 6 opposite pairs there (S/N,
//! SW/NE, SE/NW per chunk) and 4 once the head is in (13, 16): `CorridorClimb` from
//! the third tick, clinging with the gravity compensation (`corridors.md` section 1).

use rw_sim::Sounds;
use std::path::PathBuf;

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::{BodyMode, Input, Player, determine_safe_spawn_tile};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn su_a22() -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let path = root
        .join("RainWorld_Data")
        .join("StreamingAssets")
        .join("world")
        .join("su-rooms")
        .join("su_a22.txt");
    if !path.is_file() {
        eprintln!("skipping: no {}", path.display());
        return None;
    }
    Some(Room::load(&path).expect("su_a22 loads"))
}

#[test]
fn su_a22_seed_1_spawn_clings_in_the_shortcut_mouth_tunnel() {
    let Some(room) = su_a22() else { return };
    // Spawn tile, facing and chunk offsets from one `UnityRandom` stream, in the game's
    // order.
    let mut rng = UnityRandom::new(1);
    let tile = determine_safe_spawn_tile(&room, &mut rng);
    let mut p = Player::create(&mut rng);
    p.place(tile, &mut rng);
    assert_eq!(p.head().pos.tile(), (12, 16));
    assert_eq!(p.hips().pos.tile(), (12, 16));
    let still = Input::new(0, 0, false);
    p.update(
        &room,
        None,
        still,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    p.update(
        &room,
        None,
        still,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_ne!(p.body_mode(), BodyMode::CorridorClimb);
    p.update(
        &room,
        None,
        still,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_eq!(p.body_mode(), BodyMode::CorridorClimb);
    for _ in 0..57 {
        p.update(
            &room,
            None,
            still,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
    }
    assert_eq!(p.body_mode(), BodyMode::CorridorClimb);
    assert_eq!(p.head().pos.tile(), (13, 16));
    assert_eq!(p.hips().pos.tile(), (12, 16));
    // Damping 0.75 then +0.9 lift each tick: the end-of-tick vel.y settles at 0.9, which
    // the next tick's gravity cancels, so the body hangs still off the floor.
    for c in [p.head(), p.hips()] {
        assert!((c.vel.y - 0.9).abs() < 1e-4, "vel {:?}", c.vel);
        assert!((c.pos.y - c.last_pos.y).abs() < 1e-3, "moving {:?}", c.pos);
        assert_eq!((c.contact().x, c.contact().y), (0, 0));
    }
}
