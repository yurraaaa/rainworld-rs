//! Shortcut travel and vertical poles on the installed `SmallRoom` test level
//! (read-only). Set `RW_DIR` to the game install directory; skipped if absent. Expected
//! values are hand-traced from an ASCII dump of `levels/smallroom.txt` with the rules in
//! `rainworld-re/notes/shortcuts.md` and `vertical-poles.md`.
//!
//! The room's 44 pole tiles: column 18 rows 10-14 and 17-30, column 20 rows 24-30,
//! column 27 rows 24-25, column 29 rows 10-25 ((18,10), (29,10), (18,27) and (20,27)
//! are Floor tiles; (18,21) and (29,21) also carry the row-21 horizontal beam that runs
//! between them). Above row 24 every pole sits in a one-tile shaft, which the game
//! climbs in `CorridorClimb` (not ported), so the tests stay below it.

use std::path::PathBuf;

use rw_assets::Room;
use rw_sim::{Animation, BodyMode, HEAD, HIPS, Input, Player, ShortcutHandler, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn smallroom() -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("levels").is_dir() {
        eprintln!("skipping: no levels dir under {}", sa.display());
        return None;
    }
    Some(Room::load_level(&sa, "SmallRoom").expect("smallroom loads"))
}

#[test]
fn smallroom_straight_pipe_end_to_end() {
    let Some(room) = smallroom() else { return };
    // Corridor y = 18 (floor top 360) ending in the entrance (8,18), which opens left;
    // the pipe runs straight to (14,18), which opens right. 6 steps.
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(153.0, 368.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(170.0, 369.0));
    let mut handler = ShortcutHandler::new();
    let right = Input::new(1, 0, false);

    let mut tick = 0;
    while !p.in_shortcut() {
        handler.update(&room, &mut p);
        p.update(&room, right);
        tick += 1;
        assert!(tick < 30, "never entered: {:?}", p.head());
    }
    assert_eq!(p.shortcut_vessel_tile(), Some((8, 18)));

    // One tile per third tick: each of (9,18)..(13,18) is held for exactly 3 ticks,
    // then (14,18) spits the player out.
    let mut seen = Vec::new();
    loop {
        handler.update(&room, &mut p);
        let Some(tile) = p.shortcut_vessel_tile() else {
            break;
        };
        seen.push(tile);
        p.update(&room, right);
        assert!(seen.len() < 40, "never came out");
    }
    // Before the first move the vessel waits 0-2 ticks on the entrance, by phase.
    let waited = seen.iter().take_while(|&&t| t == (8, 18)).count();
    assert!(waited < 3, "{seen:?}");
    let expected: Vec<_> = (9..=13).flat_map(|x| [(x, 18); 3]).collect();
    assert_eq!(seen[waited..], expected[..]);

    // Spit out at (14,18), middle (290, 370), hole (1,0): head 2.5 px toward the room,
    // hips 2.5 px into the tube, both moving (2, 0), shortcutDelay 20, no history.
    assert_eq!(p.head().pos, Vec2::new(292.5, 370.0));
    assert_eq!(p.hips().pos, Vec2::new(287.5, 370.0));
    for c in p.chunks() {
        assert_eq!(c.vel, Vec2::new(2.0, 0.0));
        assert_eq!(c.last_pos, c.pos);
    }
    assert_eq!(p.shortcut_delay(), 20);
    // The player's own update runs in the same tick and counts the delay down.
    p.update(&room, Input::NONE);
    assert_eq!(p.shortcut_delay(), 19);
}

const UP: Input = Input::new(0, 1, false);

trait PlayerExt {
    /// Body vertical, hips at `hips`, at rest.
    fn upright(hips: Vec2) -> Player;
}

impl PlayerExt for Player {
    fn upright(hips: Vec2) -> Player {
        let mut p = Player::new();
        p.body.chunks[HIPS].reset_at(hips);
        p.body.chunks[HEAD].reset_at(hips + Vec2::new(0.0, 17.0));
        p
    }
}

#[test]
fn smallroom_climb_the_right_pole_onto_the_beam() {
    let Some(room) = smallroom() else { return };
    // Standing in column 28 on the Solid row 10 (floor top 220), no x: the grab tries
    // columns 28, 27, 29; only (29, 12) has a pole. Head x := 590, facing -1.
    let mut p = Player::upright(Vec2::new(570.0, 228.0));
    for _ in 0..3 {
        p.update(&room, Input::NONE);
    }
    assert_eq!(p.body_mode(), BodyMode::Stand);
    p.update(&room, UP);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.head().pos.x, 590.0);
    assert_eq!(p.flip_direction(), -1);

    // Steady climb (vertical-poles.md section 2: v' = 0.999 (0.75 v + 0.5)).
    let steady = 0.999 * 0.5 / (1.0 - 0.999 * 0.75);
    for _ in 0..30 {
        p.update(&room, UP);
    }
    let y0 = p.hips().pos.y;
    for _ in 0..40 {
        p.update(&room, UP);
    }
    let speed = (p.hips().pos.y - y0) / 40.0;
    assert!((speed - steady).abs() < 1e-3, "climb speed {speed}");
    // Up through the beam crossing at (29, 21) (the grab is off while climbing) to
    // head row 24, the last row before the shaft.
    let mut ticks = 0;
    while p.head().pos.y < 480.0 {
        p.update(&room, UP);
        assert_eq!(p.animation(), Animation::ClimbOnBeam);
        ticks += 1;
        assert!(ticks < 100);
    }
    // Slide back down to head row 21 and hold still there.
    while p.head().pos.y >= 440.0 {
        p.update(&room, Input::new(0, -1, false));
        assert_eq!(p.animation(), Animation::ClimbOnBeam);
    }
    for _ in 0..30 {
        p.update(&room, Input::NONE);
    }
    assert_eq!(p.head().pos.y as i32 / 20, 21);
    // A left press toward the facing side with the head in the beam tile: hang from it,
    // then shuffle left along it.
    let left = Input::new(-1, 0, false);
    p.update(&room, left);
    assert_eq!(p.animation(), Animation::HangFromBeam);
    for _ in 0..60 {
        p.update(&room, left);
    }
    assert_eq!(p.animation(), Animation::HangFromBeam);
    assert_eq!(p.head().pos.y, 430.0);
    assert!(p.head().pos.x < 560.0);
}

#[test]
fn smallroom_climb_the_left_pole_to_the_ceiling() {
    let Some(room) = smallroom() else { return };
    // Column 17 on Solid (17, 10): the grab finds (18, 12) as the +1 candidate.
    let mut p = Player::upright(Vec2::new(350.0, 228.0));
    for _ in 0..3 {
        p.update(&room, Input::NONE);
    }
    p.update(&room, UP);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.head().pos.x, 370.0);
    // Pole rows 11-14 end under Solid (18, 15): the head stops against the ceiling at
    // 300 - 9 and the player stays on the pole.
    for _ in 0..80 {
        p.update(&room, UP);
    }
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    // Snapped to 291 by the ceiling, then the rod (applied after the chunks) moves it
    // a little back down.
    assert!(p.head().pos.y > 289.0 && p.head().pos.y <= 291.0);
    assert_eq!(p.head().contact().y, 1);
    // Facing -1 (grabbed from the left): offset 5 px left of the pole middle.
    assert!((p.head().pos.x - 365.0).abs() < 1e-3);
}

#[test]
fn smallroom_hang_under_the_short_pole() {
    let Some(room) = smallroom() else { return };
    // Column 20's pole starts at row 24 with open (20, 23) below: holding up with the
    // head in (20, 23) hangs under it, head at (410, 470) + (0, 5).
    let mut p = Player::upright(Vec2::new(410.0, 458.0));
    p.update(&room, UP);
    assert_eq!(p.animation(), Animation::HangUnderVerticalBeam);
    assert_eq!(p.head().pos, Vec2::new(410.0, 475.0));
    // Holding up (+2.5 per tick) lifts the head into (20, 24); the same tick the hang
    // block raises it to at least that tile's middle + 5 + vel.y (row 25) and, the head
    // tile having a pole, switches to ClimbOnBeam.
    let mut ticks = 0;
    while p.animation() != Animation::ClimbOnBeam {
        p.update(&room, UP);
        ticks += 1;
        assert!(ticks < 20, "{:?} {:?}", p.animation(), p.head().pos);
    }
    assert_eq!(p.head().pos.y as i32 / 20, 25);
    assert!(p.head().pos.y >= 495.0);
}
