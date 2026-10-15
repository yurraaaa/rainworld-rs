//! Room-exit shortcuts in the installed game's worlds (read-only), as the Survivor. Set
//! `RW_DIR` to the game install directory; skipped if absent.
//!
//! Each transition starts with the player put on the exit's entrance and the direction
//! into the hole held, as `ShortcutHandler` carries it: the vessel reaches the exit node,
//! waits in `betweenRoomsWaitingLobby` until the destination is `ReadyForPlayer`, restarts
//! on the destination's node leading back (`ExitIndex`), and the camera moves to that
//! node's `viewedByCamera`.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{ActiveMods, StreamingAssets, tile_centre};
use rw_random::UnityRandom;
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::world::World;
use rw_sim::{CameraMove, Input, RainWorldGame, Sounds, Start, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed(StreamingAssets);

/// What one transition did.
#[derive(Debug)]
struct Transit {
    /// Most shortcut updates the vessel spent in the lobby.
    lobby_updates: u32,
    arrival_node: i32,
    camera: CameraMove,
    /// Chunk tiles on the tick the player came out.
    tiles: Vec<(i32, i32)>,
    /// The destination's entrance for the arrival node.
    entrance: (i32, i32),
    /// The destination's `waitToEnterAfterFullyLoaded` when the vessel reached the exit.
    wait_at_exit: i32,
}

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(Self(StreamingAssets::new(sa)))
        } else {
            eprintln!("skipping: no world under {}", sa.display());
            None
        }
    }

    /// The region world of `room` with the player in it, its first `settle` ticks run.
    fn game(&self, room: &str, settle: u32) -> RainWorldGame {
        let campaign = &SlugcatCampaign::SURVIVOR;
        let filter = LineFilter {
            timeline: Some(campaign.timeline),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        };
        let regions = Regions::load(&self.0).expect("regions");
        let request = WorldRequest::for_first_room(&self.0, room, filter);
        let data = WorldData::load(&self.0, &regions, &request).expect("world loads");
        let first = data.room_named(room).expect("room in world").index;
        let rooms = Arc::new(GameRooms {
            assets: self.0.clone(),
            region: Some(request.name.to_lowercase()),
            timeline: campaign.timeline,
        });
        let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
        let start = Start {
            room: first,
            tile: None,
        };
        let mut game = RainWorldGame::new(
            world,
            campaign,
            start,
            UnityRandom::new(1),
            Sounds::default(),
            &mut (),
        );
        assert_eq!(game.take_camera_move().map(|m| m.camera), Some(0));
        for _ in 0..settle {
            game.tick(Input::default(), &mut ());
        }
        game
    }

    /// Puts the player on `from`'s exit to `to` and holds the way in until it leaves
    /// the destination's entrance.
    fn transit(&self, from: &str, to: &str, settle: u32) -> Transit {
        let mut game = self.game(from, settle);
        let world = game.world();
        let src = world.room_index(from).expect("from");
        let dest = world.room_index(to).expect("to");
        let src_data = world.abstract_room(src).expect("from data");
        let node = src_data.exit_index(dest);
        assert!(node >= 0, "{from} has an exit to {to}");
        let room = &world.realized(src).expect("realized").room;
        let entrance = room
            .shortcut_map()
            .leading_to_node(usize::try_from(node).expect("node"))
            .expect("the exit's shortcut")
            .start;
        let hole = room.shortcut_hole_direction(entrance.0, entrance.1);
        let input = Input::new(-hole.0 as i8, -hole.1 as i8, false);
        game.set_player(
            Vec2::from(tile_centre(entrance.0, entrance.1)),
            Vec2::ZERO,
            &mut (),
        );
        game.take_camera_move();
        let mut lobby_updates = 0;
        let mut camera = None;
        let mut wait_at_exit = None;
        for _ in 0..600 {
            game.tick(input, &mut ());
            if let Some(lobby) = game.lobby() {
                lobby_updates = lobby_updates.max(lobby.updates);
            }
            if game.player_pos().node >= 0 && wait_at_exit.is_none() {
                wait_at_exit = game.world().realized(dest).map(|r| r.wait_to_enter());
            }
            if let Some(mv) = game.take_camera_move() {
                camera = Some(mv);
            }
            let pos = game.player_pos();
            let player = game.player().expect("player");
            if pos.room == dest && !player.in_shortcut() {
                let dest_room = &game.world().realized(dest).expect("realized").room;
                let entrance = dest_room
                    .shortcut_map()
                    .leading_to_node(usize::try_from(pos.node).expect("arrival node"))
                    .expect("arrival shortcut")
                    .start;
                return Transit {
                    lobby_updates,
                    arrival_node: pos.node,
                    camera: camera.expect("the camera moved"),
                    tiles: player.chunks().iter().map(|c| c.pos.tile()).collect(),
                    entrance,
                    wait_at_exit: wait_at_exit.unwrap_or(-1),
                };
            }
        }
        panic!("{from} -> {to}: never arrived");
    }
}

/// The four transitions from the Survivor's start to the first shelter, and back into
/// the start room (arriving on its third camera): the arrival node
/// is the destination's exit back (`ExitIndex`), the player comes out at that node's
/// entrance, and the camera moves to the node's baked `viewedByCamera`, which is also
/// the camera nearest the node (`CameraViewingNode`).
#[test]
fn outskirts_start_to_shelter() {
    let Some(game) = Installed::open() else {
        return;
    };
    for (from, to) in [
        ("SU_C04", "SU_A41"),
        ("SU_A41", "SU_A43"),
        ("SU_A43", "SU_A22"),
        ("SU_A22", "SU_S01"),
        ("SU_A41", "SU_C04"),
    ] {
        let t = game.transit(from, to, 60);
        println!("{from} -> {to}: {t:?}");
        let world = game.game(to, 0);
        let dest = world
            .world()
            .abstract_room(world.camera_room())
            .expect("dest");
        let src = world.world().room_index(from).expect("from");
        assert_eq!(t.arrival_node, dest.exit_index(src), "{from} -> {to}");
        let node = usize::try_from(t.arrival_node).expect("node");
        let baked = dest.nodes[node].viewed_by_camera;
        assert_eq!(t.camera.room, dest.index);
        assert_eq!(
            i32::try_from(t.camera.camera).expect("camera"),
            baked.max(0)
        );
        let realized = world.world().realized(dest.index).expect("realized");
        assert_eq!(realized.room.camera_viewing_node(node), baked);
        assert!(
            t.tiles
                .iter()
                .any(|&(x, y)| (x - t.entrance.0).abs() + (y - t.entrance.1).abs() <= 2),
            "{from} -> {to}: out at the entrance {:?}",
            t.entrance
        );
        // Base Outskirts rooms make no one wait; the realizer had the room loaded.
        assert_eq!(t.wait_at_exit, -1);
        assert_eq!(t.lobby_updates, 0);
    }
}

/// `SS_AI` (story, Five Pebbles' room) makes creatures wait 80 ticks after loading:
/// entered at once from `SS_D07`, the vessel waits in the lobby until the count is
/// below 1, then moves in.
#[test]
fn oracle_room_entry_waits_in_the_lobby() {
    let Some(game) = Installed::open() else {
        return;
    };
    let t = game.transit("SS_D07", "SS_AI", 0);
    println!("SS_D07 -> SS_AI: {t:?}");
    assert!(t.wait_at_exit > 0, "the room still counts down");
    // One lobby update per shortcut update (every 3rd tick) until the count is out.
    let expected = u32::try_from(t.wait_at_exit).expect("positive").div_ceil(3);
    assert!(
        t.lobby_updates.abs_diff(expected) <= 1,
        "lobby {} updates for a wait of {}",
        t.lobby_updates,
        t.wait_at_exit
    );
}
