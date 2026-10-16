//! `spawn_creature` and room screenshots, on SU_A07 of the installed game (skipped
//! without it; the screenshot test skips without a Vulkan adapter).

use rw_assets::{Room, Terrain};

use super::Session;
use super::params::{InputParam, ScreenshotParams, SpawnCreatureParams, StepParams, TilesParams};

/// The first air tile standing on solid ground, scanning from the bottom left.
trait FloorTile {
    fn floor_tile(&self) -> (i32, i32);
}

impl FloorTile for Room {
    fn floor_tile(&self) -> (i32, i32) {
        (2..self.height())
            .flat_map(|y| (2..self.width() - 2).map(move |x| (x, y)))
            .find(|&(x, y)| {
                self.get(x, y).is_some_and(|t| t.terrain == Terrain::Air)
                    && self
                        .get(x, y + 1)
                        .is_some_and(|t| t.terrain == Terrain::Air)
                    && self
                        .get(x, y - 1)
                        .is_some_and(|t| t.terrain == Terrain::Solid)
            })
            .expect("a floor tile")
    }
}

impl Session {
    fn su_a07() -> Option<Self> {
        let mut s = Self::installed()?;
        s.load("SU_A07", 1);
        s.step(&StepParams {
            ticks: Some(40),
            input: Some(InputParam::default()),
        })
        .expect("steps");
        Some(s)
    }

    fn camera_floor_tile(&mut self) -> (i32, i32) {
        self.scene_mut().expect("game").game().room().floor_tile()
    }
}

/// A green lizard and a fly are realized on the asked tile, with IDs from the issuer.
#[test]
fn spawned_creatures_are_realized_on_their_tile() {
    let Some(mut s) = Session::su_a07() else {
        return;
    };
    let (x, y) = s.camera_floor_tile();
    let next = s.scene_mut().expect("game").game().sim().next_issued_id();
    for (n, kind) in ["GreenLizard", "Fly"].into_iter().enumerate() {
        let reply = s
            .spawn_creature(&SpawnCreatureParams {
                kind: kind.to_owned(),
                tile: [x, y],
                ..Default::default()
            })
            .expect("spawns");
        assert!(reply.realized, "{reply:?}");
        assert_eq!(reply.tile, [x, y]);
        assert_eq!(reply.id, [-1, next + 1 + i32::try_from(n).unwrap_or(0)]);
        let state = s.state().expect("state");
        let c = state
            .creatures
            .iter()
            .find(|c| c.id == reply.id)
            .expect("in get_state");
        assert_eq!((c.kind.as_str(), c.room.as_str()), (kind, "SU_A07"));
        assert!(c.realized && c.alive && !c.in_den);
        assert_eq!(c.tile, [x, y]);
        let centre = rw_assets::tile_centre(x, y);
        let pos = match (&c.lizard, &c.fly) {
            (Some(l), _) => l.chunks[0].pos,
            (None, Some(f)) => f.pos,
            _ => panic!("no realized body reported: {c:?}"),
        };
        // `PlaceInRoom`: within a pixel of the tile's middle.
        assert!(
            (pos[0] - centre[0]).hypot(pos[1] - centre[1]) <= 1.01,
            "{pos:?}"
        );
    }
    // A type without a realized body in the port stays abstract.
    let reply = s
        .spawn_creature(&SpawnCreatureParams {
            kind: "PinkLizard".to_owned(),
            tile: [x, y],
            ..Default::default()
        })
        .expect("spawns");
    assert!(!reply.realized && reply.note.is_some(), "{reply:?}");
    assert!(
        s.spawn_creature(&SpawnCreatureParams {
            kind: "greenlizard".to_owned(),
            tile: [x, y],
            ..Default::default()
        })
        .is_err()
    );
}

/// A screenshot of another realized room draws it and leaves the game as it was.
#[test]
fn another_rooms_screenshot_leaves_the_game_untouched() {
    let Some(mut s) = Session::su_a07() else {
        return;
    };
    let before = s.state().expect("state");
    let other = before
        .realized_rooms
        .iter()
        .map(|r| r.room.clone())
        .find(|r| *r != before.room)
        .expect("a neighbour realized");
    let tiles = s
        .tiles(&TilesParams {
            room: Some(other.clone()),
            ..Default::default()
        })
        .expect("tiles");
    assert!(tiles.starts_with(&format!("room {other}")), "{tiles}");
    let rng = s.scene_mut().expect("game").game().cosmetic_rng().clone();
    let shot = match s.screenshot(&ScreenshotParams {
        room: Some(other.clone()),
        ..Default::default()
    }) {
        Ok(shot) => shot,
        Err(e) if e.starts_with("no GPU renderer") => {
            eprintln!("skipping: {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    assert!(shot.png.starts_with(b"\x89PNG") && shot.width > 0);
    let after = s.state().expect("state");
    assert_eq!(
        serde_json::to_string(&before).expect("json"),
        serde_json::to_string(&after).expect("json")
    );
    assert_eq!(&rng, s.scene_mut().expect("game").game().cosmetic_rng());
    // The camera's room still draws.
    assert!(s.screenshot(&ScreenshotParams::default()).is_ok());
}
