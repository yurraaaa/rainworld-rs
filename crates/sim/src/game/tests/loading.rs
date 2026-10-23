//! The loading stages of a Survivor story's start room under the game's minimum schedule:
//! `ShortCutsReady` on the preparer's second call (objects, then the creatures without
//! `AI` when none needs the AI map), `ReadyForAI` on the third (creatures in list order,
//! then the `AIMapReady` listeners). Expected values from `Room.cs` (`ShortCutsReady`,
//! `ReadyForAI`), `RoomPreparer.cs` (`Update`, `UpdateThread`) and
//! `AbstractCreature.cs` (`AllowedToExistInRoom`).
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{AbstractRoomNode, NodeType};
use rw_assets::{ActiveMods, CreatureType, PlacedObject, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::{CreatureId, EntityId, WorldCoordinate};
use crate::item::{AbstractObject, ObjectId};
use crate::tests::room;
use crate::world::{LoadingSchedule, PreparerStatus, World};
use crate::{Input, RainWorldGame, Sounds, Start};

/// A 40 x 20 room with three rows of floor (top at y 3).
fn rows() -> Vec<String> {
    let mut rows: Vec<String> = vec![".".repeat(40); 20];
    for row in &mut rows[17..] {
        *row = "#".repeat(40);
    }
    rows
}

/// A room with a den under the floor at (4, 1): one node.
const DEN: [&str; 6] = [
    "##############",
    "#............#",
    "#............#",
    "#............#",
    "####E#########",
    "####D#########",
];

/// What a test room starts with.
#[derive(Default)]
struct Setup<'a> {
    placed: &'a str,
    rock: bool,
    counted_fly: bool,
    /// A side exit node after the shortcut nodes (test rooms have no baked nodes).
    side_exit: bool,
}

/// A Survivor story in `rows` under `schedule`, the player at (2, 3), no tick run: the
/// constructor made the preparer's first call. Returns the rock's ID when there is one.
fn game(
    rows: &[&str],
    setup: &Setup<'_>,
    schedule: LoadingSchedule,
) -> (RainWorldGame, Option<ObjectId>) {
    let room = room(rows);
    let mut data = WorldData::from_room(&room);
    if setup.side_exit {
        data.rooms[0].nodes.push(AbstractRoomNode {
            kind: NodeType::SideExit,
            shortcut_length: -1,
            submerged: false,
            viewed_by_camera: -1,
            entrance_width: 1,
            connectivity: Default::default(),
        });
    }
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(setup.placed),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let mut world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    world.loading_schedule = schedule;
    let rock = setup.rock.then(|| {
        world.add_object(AbstractObject::new_rock(
            EntityId::new(0, 1),
            WorldCoordinate::new(0, 6, 3, -1),
        ))
    });
    if setup.counted_fly {
        world.add_quantified_at(0, 0, CreatureType::Fly);
    }
    let g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 3)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut (),
    );
    (g, rock)
}

impl RainWorldGame {
    fn progress(&self) -> u8 {
        self.world.realized(0).expect("room").loading_progress()
    }

    /// A green lizard dev-spawned at (20, 3), still abstract while the room loads.
    fn loading_lizard(&mut self) -> CreatureId {
        let l = self
            .dev_spawn_creature(CreatureType::GreenLizard, 0, (20, 3), None, &mut ())
            .expect("spawns");
        assert!(!self.world.creature(l).is_realized());
        l
    }
}

fn refs(rows: &[String]) -> Vec<&str> {
    rows.iter().map(String::as_str).collect()
}

/// Immediate (the port's synchronous loading until the schedule is switched on): the
/// constructor's call runs both stages and drops the preparer.
#[test]
fn the_immediate_schedule_loads_the_start_room_in_the_constructor() {
    let rows = rows();
    let (g, _) = game(&refs(&rows), &Setup::default(), LoadingSchedule::Immediate);
    assert_eq!(g.progress(), 3);
    assert!(g.player.is_some());
    assert!(g.world.room_preparer(0).is_none());
}

/// The game's minimum: the constructor's call starts the thread; the first tick's call
/// takes stage 1 (the player, which has no `AI`, comes in with it in a room where no
/// creature needs the AI map), the second's stage 2; the third sees the thread done and
/// the fourth removes the preparer.
#[test]
fn stage_one_comes_on_the_second_call_and_stage_two_on_the_third() {
    let rows = rows();
    let (mut g, _) = game(
        &refs(&rows),
        &Setup::default(),
        LoadingSchedule::GAME_MINIMUM,
    );
    let p = g.world.room_preparer(0).expect("loading");
    assert_eq!(
        (p.calls(), p.status()),
        (1, PreparerStatus::MappingShortcuts)
    );
    assert_eq!(g.progress(), 0);
    assert!(g.player.is_none());
    assert!(g.world.realized(0).unwrap().shortcuts_blinking.is_none());
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.progress(), 1);
    assert!(g.player.is_some());
    let r = g.world.realized(0).unwrap();
    assert!(r.shortcuts_blinking.is_some() && r.ai_map.is_none());
    assert!(r.ready_for_player() && !r.ready_for_ai());
    assert_eq!(
        g.world.room_preparer(0).unwrap().status(),
        PreparerStatus::MappingAi
    );
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.progress(), 3);
    assert!(g.world.realized(0).unwrap().ai_map.is_some());
    let p = g.world.room_preparer(0).expect("still listed");
    assert_eq!((p.status(), p.done()), (PreparerStatus::Finishing, false));
    g.tick(Input::NONE, &mut ());
    assert!(g.world.room_preparer(0).unwrap().done());
    g.tick(Input::NONE, &mut ());
    assert!(g.world.room_preparer(0).is_none());
}

/// `ShortCutsReady` realizes the room's objects before its creatures: the rock takes its
/// place in the update list before the player.
#[test]
fn stage_one_realizes_the_objects_before_the_player() {
    let rows = rows();
    let setup = Setup {
        rock: true,
        ..Setup::default()
    };
    let (mut g, rock) = game(&refs(&rows), &setup, LoadingSchedule::GAME_MINIMUM);
    let rock = rock.unwrap();
    g.update_loading_rooms(&mut ());
    let order = g.world.realized_weapon(rock, 0).expect("rock").update_order;
    assert!(g.player.is_some());
    assert!(order < g.player_seq);
}

/// A lizard (`requireAImap`) in the room: stage 1 realizes only the rock (its two draws,
/// rotation and spin) and leaves the room closed to creatures without `AI`; stage 2
/// realizes the player and the lizard in list order.
#[test]
fn an_ai_creature_keeps_every_creature_for_stage_two() {
    let rows = rows();
    let setup = Setup {
        rock: true,
        ..Setup::default()
    };
    let (mut g, rock) = game(&refs(&rows), &setup, LoadingSchedule::GAME_MINIMUM);
    let l = g.loading_lizard();
    let mut expect = g.rng.clone();
    expect.value();
    expect.value();
    g.update_loading_rooms(&mut ());
    assert_eq!(g.rng.state(), expect.state());
    assert!(g.world.realized_weapon(rock.unwrap(), 0).is_some());
    assert!(g.player.is_none() && !g.world.creature(l).is_realized());
    assert_eq!(g.progress(), 1);
    assert!(!g.world.realized(0).unwrap().ready_for_player());
    g.update_loading_rooms(&mut ());
    assert_eq!(g.progress(), 3);
    assert!(g.player.is_some() && g.world.creature(l).is_realized());
    assert!(g.player_seq < g.world.lizard_ref(l).update_order);
}

/// A counted fly (`requireAImap`, quantified) also keeps the player for stage 2.
#[test]
fn a_counted_fly_keeps_the_player_for_stage_two() {
    let (mut g, _) = game(
        &DEN,
        &Setup {
            counted_fly: true,
            ..Setup::default()
        },
        LoadingSchedule::GAME_MINIMUM,
    );
    assert!(g.world.room_creatures_require_ai_map(0));
    g.update_loading_rooms(&mut ());
    assert_eq!(g.progress(), 1);
    assert!(g.player.is_none());
    g.update_loading_rooms(&mut ());
    assert!(g.player.is_some());
}

/// `AIMapReady` runs after the creatures: the wall mycelia the coral system makes there
/// come after the lizard in the update list (the camera's first view, at the head of the
/// first tick, came before).
#[test]
fn ai_map_ready_comes_after_the_creatures() {
    let rows = rows();
    let setup = Setup {
        placed: "WallMycelia><300><100><0~150",
        ..Setup::default()
    };
    let (mut g, _) = game(&refs(&rows), &setup, LoadingSchedule::GAME_MINIMUM);
    let l = g.loading_lizard();
    g.tick(Input::NONE, &mut ());
    assert!(
        g.world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .wall_mycelia
            .is_empty()
    );
    g.tick(Input::NONE, &mut ());
    let system = g.world.realized(0).unwrap().coral.as_ref().unwrap();
    assert!(system.built);
    let lizard = g.world.lizard_ref(l).update_order;
    assert!(system.wall_mycelia.iter().all(|w| w.update_order > lizard));
}

/// A room open at its left edge from y 3 up: the `ShortcutMapper` constructor made its
/// border exits at realization, so during stage 1 (no AI map yet) `LocalCoordinateOfNode`
/// of the side exit node draws `Range(0, tiles)` and gives one of its edge tiles.
#[test]
fn a_border_exit_node_has_its_tiles_before_the_ai_map() {
    let mut rows = rows();
    for row in &mut rows[..17] {
        row.replace_range(..1, ".");
    }
    let setup = Setup {
        side_exit: true,
        ..Setup::default()
    };
    let (mut g, _) = game(&refs(&rows), &setup, LoadingSchedule::GAME_MINIMUM);
    g.update_loading_rooms(&mut ());
    let r = g.world.realized(0).unwrap();
    assert_eq!(r.loading_progress(), 1);
    assert!(r.ai_map.is_none());
    let tiles = r.border_exits[0].tiles.clone();
    assert_eq!(r.border_exits[0].kind, NodeType::SideExit);
    assert_eq!(tiles.len(), 17);
    assert!(tiles.iter().all(|&(x, y)| x == 0 && y >= 3));
    let mut expect = g.rng.clone();
    let i = expect.range_i32(0, 17) as usize;
    let at = g.world.local_coordinate_of_node(0, 0, &mut g.rng);
    assert_eq!(at, tiles[i]);
    assert_eq!(g.rng.state(), expect.state());
}
