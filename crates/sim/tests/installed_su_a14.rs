//! Corridor crawling in the installed Outskirts room `SU_A14` (read-only). Set `RW_DIR`
//! to the game install directory; skipped if absent. Expected values come from
//! `rainworld-re/notes/corridors.md` and an ASCII dump of `world/su-rooms/su_a14.txt`:
//! row 20 is a one-tile tunnel (Solid rows 19 and 21) from column 15 to 32; at column 33
//! its ceiling becomes a slope and from column 34 the room above is open.

use rw_sim::Sounds;
use std::path::PathBuf;

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::{BodyMode, HEAD, HIPS, Input, Player, Vec2};

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

fn mean_x(p: &Player) -> f32 {
    (p.head().pos.x + p.hips().pos.x) / 2.0
}

#[test]
fn su_a14_tunnel_crawl_speed_and_exit() {
    let Some(room) = su_a14() else { return };
    // Lying in the tunnel, hips in (16, 20), head to the right.
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(330.0, 410.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(347.0, 410.0));
    let right = Input::new(1, 0, false);
    // Three ticks of counting, then CorridorClimb.
    p.update(
        &room,
        None,
        right,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    p.update(
        &room,
        None,
        right,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_ne!(p.body_mode(), BodyMode::CorridorClimb);
    p.update(
        &room,
        None,
        right,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_eq!(p.body_mode(), BodyMode::CorridorClimb);
    // Steady speed: the mean end velocity m solves m = 0.75 * 0.999 m + 1.32 / 2, and
    // physics moves by 0.999 m.
    let steady = 0.999 * 0.66 / (1.0 - 0.75 * 0.999);
    for _ in 0..40 {
        p.update(
            &room,
            None,
            right,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
    }
    let x0 = mean_x(&p);
    for _ in 0..10 {
        p.update(
            &room,
            None,
            right,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
    }
    let speed = (mean_x(&p) - x0) / 10.0;
    assert!(
        (speed - steady).abs() < 1e-3,
        "speed {speed}, expected {steady}"
    );
    assert_eq!(p.body_mode(), BodyMode::CorridorClimb);
    // Out under the open room: no opposite pairs left, so classification is back.
    let mut ticks = 0;
    while p.hips().pos.x < 680.0 {
        p.update(
            &room,
            None,
            right,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        ticks += 1;
        assert!(ticks < 200, "stuck at {:?}", p.hips().pos);
    }
    p.update(
        &room,
        None,
        right,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_ne!(p.body_mode(), BodyMode::CorridorClimb);
}
