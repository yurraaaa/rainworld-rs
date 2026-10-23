//! `spawn_creature`, `spawn_object` and room screenshots, on SU_A07 of the installed game (skipped
//! without it; the screenshot test skips without a Vulkan adapter).

use rw_assets::{Room, Terrain};

use super::Session;
use super::params::{
    InputParam, ScreenshotParams, SetRainTimerParams, SpawnCreatureParams, SpawnObjectParams,
    StepParams, TeleportPlayerParams, TilesParams,
};

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
    fn idle(&mut self, ticks: u32) -> super::reply::State {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(InputParam::default()),
        })
        .expect("steps")
    }

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

/// A rock and a spear are realized on the asked tile with new IDs, making the game's
/// draws: `Weapon`'s rotation and `Rock.SetRandomSpin`'s value (2); the rotation,
/// `Spear`'s `spearmasterNeedleType` and `Spear.SetRandomSpin`'s two values (4).
#[test]
fn spawned_objects_are_realized_on_their_tile() {
    let Some(mut s) = Session::su_a07() else {
        return;
    };
    let (x, y) = s.camera_floor_tile();
    for (kind, draws) in [("Rock", 2), ("Spear", 4)] {
        let mut expected = s.scene_mut().expect("game").game().sim().rng().clone();
        for _ in 0..draws {
            expected.value();
        }
        let reply = s
            .spawn_object(&SpawnObjectParams {
                kind: kind.to_owned(),
                tile: [x, y],
                ..Default::default()
            })
            .expect("spawns");
        assert!(reply.realized && reply.note.is_none(), "{reply:?}");
        assert_eq!((reply.kind.as_str(), reply.tile), (kind, [x, y]));
        assert_eq!(s.scene_mut().expect("game").game().sim().rng(), &expected);
        let state = s.state().expect("state");
        let o = state
            .objects
            .iter()
            .find(|o| o.id == reply.id)
            .expect("in get_state");
        assert_eq!((o.room.as_str(), o.tile), ("SU_A07", [x, y]));
        let w = o.realized.as_ref().expect("realized");
        let centre = rw_assets::tile_centre(x, y);
        assert!((w.pos[0] - centre[0]).hypot(w.pos[1] - centre[1]) < 0.01);
    }
    assert!(
        s.spawn_object(&SpawnObjectParams {
            kind: "Oracle".to_owned(),
            tile: [x, y],
            ..Default::default()
        })
        .is_err()
    );
}

/// An item the port has no body for is added abstract with a new ID and a note, and
/// makes no draws (`GetNewID` and the constructors draw nothing).
#[test]
fn spawned_items_without_a_body_stay_abstract() {
    let Some(mut s) = Session::su_a07() else {
        return;
    };
    let (x, y) = s.camera_floor_tile();
    let before = s.scene_mut().expect("game").game().sim().rng().clone();
    let reply = s
        .spawn_object(&SpawnObjectParams {
            kind: "DataPearl".to_owned(),
            tile: [x, y],
            ..Default::default()
        })
        .expect("spawns");
    assert!(!reply.realized, "{reply:?}");
    assert_eq!(
        reply.note.as_deref(),
        Some("the port does not realize DataPearl objects: added abstract")
    );
    assert_eq!(s.scene_mut().expect("game").game().sim().rng(), &before);
    let state = s.state().expect("state");
    let o = state
        .objects
        .iter()
        .find(|o| o.id == reply.id)
        .expect("in get_state");
    assert_eq!(
        (o.kind.as_str(), o.room.as_str(), o.tile),
        ("DataPearl", "SU_A07", [x, y])
    );
    assert!(o.realized.is_none());
}

/// The teleport keeps the world: the player goes through the lobby and comes out of
/// the target room's first exit, the camera following, every creature still there
/// with its ID, and the clock running on. To a room already realized the call itself
/// makes no draws.
#[test]
fn teleport_keeps_the_world_and_comes_out_of_an_exit() {
    let Some(mut s) = Session::su_a07() else {
        return;
    };
    let before = s.state().expect("state");
    let target = before
        .realized_rooms
        .iter()
        .map(|r| r.room.clone())
        .find(|r| *r != before.room)
        .expect("a realized neighbour");
    let rng = s.scene_mut().expect("game").game().sim().rng().clone();
    let params = TeleportPlayerParams {
        room: target.clone(),
        node: None,
    };
    let reply = s.teleport_player(&params).expect("teleports");
    assert_eq!(reply.room, target);
    assert_eq!(s.scene_mut().expect("game").game().sim().rng(), &rng);
    assert!(s.teleport_player(&params).is_err(), "already travelling");
    let mut arrived = None;
    for _ in 0..80 {
        let state = s
            .step(&StepParams {
                ticks: Some(1),
                input: Some(InputParam::default()),
            })
            .expect("steps");
        if state.room == target && state.player.as_ref().is_some_and(|p| !p.in_shortcut) {
            arrived = Some(state);
            break;
        }
    }
    let after = arrived.expect("arrives within 2 s");
    assert_eq!(
        (after.player_room.as_str(), after.player_node),
        (target.as_str(), reply.node)
    );
    assert!(after.tick > before.tick);
    let exit = s
        .exits()
        .expect("exits")
        .exits
        .into_iter()
        .find(|e| i32::try_from(e.node).ok() == Some(reply.node))
        .expect("the exit");
    let head = &after.player.as_ref().expect("player").chunks[0];
    // Spat out of the entrance (`SpitOutOfShortCut`), then moving out of the hole.
    let [ex, ey] = exit.entrance_tile.expect("an entrance");
    assert!(
        (head.tile[0] - ex).abs() <= 1 && (head.tile[1] - ey).abs() <= 1,
        "{:?} vs {:?}",
        head.tile,
        [ex, ey]
    );
    assert!(
        before
            .creatures
            .iter()
            .map(|c| c.id)
            .eq(after.creatures.iter().map(|c| c.id))
    );
}

/// `set_rain_timer`: set, hold (the clock runs on, the timer does not), restart, and
/// the state it reports; nothing is drawn from the random stream by the edit itself.
#[test]
fn set_rain_timer_sets_holds_and_restarts() {
    let Some(mut s) = Session::su_a07() else {
        return;
    };
    let length = s.state().expect("state").rain.cycle_length;
    let rng = s.scene_mut().expect("game").game().sim().rng().clone();
    let rain = s
        .set_rain_timer(&SetRainTimerParams {
            timer: Some(length - 1200),
            paused: Some(true),
            ..SetRainTimerParams::default()
        })
        .expect("sets");
    assert_eq!(rain.timer, length - 1200);
    assert_eq!(rain.time_until_rain, 1200);
    assert!(rain.dev_paused);
    assert_eq!(s.scene_mut().expect("game").game().sim().rng(), &rng);
    let state = s.idle(20);
    assert_eq!(state.rain.timer, length - 1200);
    let rain = s
        .set_rain_timer(&SetRainTimerParams {
            paused: Some(false),
            ..SetRainTimerParams::default()
        })
        .expect("resumes");
    assert_eq!(s.idle(10).rain.timer, rain.timer + 10);
    let rain = s
        .set_rain_timer(&SetRainTimerParams {
            restart: Some(true),
            ..SetRainTimerParams::default()
        })
        .expect("restarts");
    assert_eq!((rain.timer, rain.dev_paused), (0, false));
    let rain = s
        .set_rain_timer(&SetRainTimerParams {
            stop: Some(true),
            shift: Some(80),
            ..SetRainTimerParams::default()
        })
        .expect("stops");
    assert_eq!((rain.timer, rain.dev_paused), (80, true));
}
