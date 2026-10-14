//! Shortcut travel on the installed `SmallRoom` test level (read-only). Set `RW_DIR` to
//! the game install directory; skipped if absent. Expected values are hand-traced from
//! an ASCII dump of `levels/smallroom.txt` with the rules in
//! `rainworld-re/notes/shortcuts.md`.

use std::path::PathBuf;

use rw_assets::Room;
use rw_sim::{HEAD, HIPS, Input, Player, ShortcutHandler, Vec2};

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
