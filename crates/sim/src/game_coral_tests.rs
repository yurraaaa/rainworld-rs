//! The coral in a test room: `Room.Loaded` making the system, `AIMapReady` making the wall
//! mycelia from the placed objects (after the camera's first view), their turns in the
//! update list with their draws, the strands' cull while the system is frozen.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, PlacedObject, Room, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use super::super::lights::tests::WithGraphics;
use rw_assets::{ClipNames, SoundTable};

use super::CoralRef;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::game::OverseerView;
use crate::tests::room_with_water;
use crate::world::World;
use crate::{
    CosmeticKind, Input, MyceliaCx, MyceliumId, NeuronSpark, OwnerKey, Player, PlayerObserver,
    PlayerSurroundings, PointOwner, RainWorldGame, SoundEvent, SoundId, Sounds, Start, SystemId,
    Vec2, Water,
};

/// A 40 x 20 room with three rows of floor, the player standing on it at (2, 3).
fn coral_game(placed: &str, effects: &[(&str, f32)], ticks: usize) -> RainWorldGame {
    coral_game_with(placed, effects, ticks, Sounds::default())
}

/// [`coral_game`] with the room's `Sounds`.
fn coral_game_with(
    placed: &str,
    effects: &[(&str, f32)],
    ticks: usize,
    sounds: Sounds,
) -> RainWorldGame {
    coral_game_observed(placed, effects, ticks, sounds, false, &mut WithGraphics)
}

/// [`coral_game_with`] under an observer, with an overseer dev-spawned right after the game
/// is made (the room is still loading, so it is realized when the room finishes loading).
fn coral_game_observed(
    placed: &str,
    effects: &[(&str, f32)],
    ticks: usize,
    sounds: Sounds,
    overseer: bool,
    observer: &mut dyn PlayerObserver,
) -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(40); 20];
    for row in &mut rows[17..] {
        *row = "#".repeat(40);
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let room: Room = room_with_water(&refs, -1);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(placed),
        effects: effects
            .iter()
            .map(|&(kind, amount)| RoomEffect {
                kind: kind.to_owned(),
                amount,
            })
            .collect(),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 3)),
        },
        UnityRandom::new(5),
        sounds,
        observer,
    );
    if overseer {
        g.dev_spawn_creature(CreatureType::Overseer, 0, (10, 10), None, observer)
            .expect("spawns");
    }
    for _ in 0..ticks {
        g.tick(Input::NONE, observer);
    }
    g
}

/// Over the floor's open side: a place of radius 150 at (300, 100) reaches the floor's
/// top at y 60 and the walls of the room's bottom rows.
const WALL: &str = "WallMycelia><300><100><0~150";

/// A coral placed object or the `SSSwarmers` effect gives the room one system, made when the
/// room loads (its two wind draws), with its registry; `AIMapReady` (the first tick of the
/// session's first room) builds the wall mycelia in a later place of the update list.
#[test]
fn the_system_is_made_at_load_and_its_owners_at_ai_map_ready() {
    let g = coral_game(WALL, &[], 0);
    let r = g.world.realized(0).expect("room");
    let system = r.coral.as_ref().expect("system");
    assert!(!system.built && system.wall_mycelia.is_empty());
    assert_eq!(g.world.mycelia().registry(SystemId(1)).len(), 0);
    let g = coral_game(WALL, &[], 1);
    let system = g.world.realized(0).unwrap().coral.as_ref().unwrap();
    assert!(system.built);
    assert_eq!(system.wall_mycelia.len(), 1);
    let wall = &system.wall_mycelia[0];
    assert!(system.update_order < wall.update_order);
    assert!(!wall.mycelia.is_empty());
    assert_eq!(
        g.world.mycelia().registry(SystemId(1)).len(),
        wall.mycelia.len()
    );
    assert_eq!(wall.places.len(), 1);
    assert_eq!(wall.places[0].pos, Vec2::new(300.0, 100.0));
    assert_eq!(wall.places[0].rad, 150.0);
    // The system's own turn: `age` counts ticks (one a tick; the first room is viewed).
    assert_eq!(system.age, 1);
    assert_eq!(system.not_viewed_counter, 0);
}

/// The `SSSwarmers` effect makes the system too (its swarmers are a later step's), with no
/// owners without placed objects.
#[test]
fn the_ss_swarmers_effect_makes_a_system() {
    let g = coral_game("", &[("SSSwarmers", 0.5)], 2);
    let system = g.world.realized(0).unwrap().coral.as_ref().expect("system");
    assert!(system.built);
    assert!(system.wall_mycelia.is_empty());
    assert_eq!(system.age, 2);
    let none = coral_game("", &[], 1);
    assert!(none.world.realized(0).unwrap().coral.is_none());
}

/// A second coral object adds no second system and no draws.
#[test]
fn a_second_coral_object_makes_no_second_system() {
    let mut g = coral_game(WALL, &[], 0);
    let order = g
        .world
        .realized(0)
        .unwrap()
        .coral
        .as_ref()
        .unwrap()
        .update_order;
    let state = g.rng.state();
    g.add_coral_system(0);
    assert_eq!(g.rng.state(), state);
    assert_eq!(
        g.world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .update_order,
        order
    );
}

/// The system's turn draws two (the wind); the wall mycelia's second turn, the camera having
/// seen the strands in the first, resets each strand (`2 + 2 * points` draws), picks its
/// registry partner (one) and drifts (one).
#[test]
fn the_turns_draw_as_the_game_does() {
    let mut g = coral_game(WALL, &[], 1);
    let mut expect = g.rng.clone();
    g.run_coral(0, CoralRef::System);
    expect.value();
    expect.value();
    assert_eq!(g.rng.state(), expect.state());
    let (order, ids) = {
        let w = &g
            .world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .wall_mycelia[0];
        (w.update_order, w.mycelia.clone())
    };
    let mut draws = 0;
    for &id in &ids {
        let m = g.world.mycelia().get(id).unwrap();
        assert!(m.culled && m.viewed_by_camera, "strand {id:?}");
        draws += 2 + 2 * m.points.len() + 1 + 1;
    }
    for _ in 0..draws {
        expect.value();
    }
    g.run_coral(0, CoralRef::Wall(order));
    assert_eq!(g.rng.state(), expect.state());
    assert!(
        ids.iter()
            .all(|&id| !g.world.mycelia().get(id).unwrap().culled)
    );
}

/// While the system is frozen its strands are culled: they draw only for their drift and do
/// not move; thawed, the camera sees them and they are drawn again.
#[test]
fn a_frozen_system_stops_its_strands() {
    let mut g = coral_game(WALL, &[], 3);
    let (order, ids) = {
        let w = &g
            .world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .wall_mycelia[0];
        (w.update_order, w.mycelia.clone())
    };
    // The room is viewed and the strands in the camera's reach are live.
    let live = ids
        .iter()
        .filter(|&&id| !g.world.mycelia().get(id).unwrap().culled)
        .count();
    assert!(live > 0, "no strand in view");
    {
        let r = g.world.realized_mut(0).unwrap();
        let s = r.coral.as_mut().unwrap();
        s.age = 400;
        s.not_viewed_counter = 100;
    }
    let before: Vec<_> = ids
        .iter()
        .map(|&id| g.world.mycelia().get(id).unwrap().points.clone())
        .collect();
    let mut expect = g.rng.clone();
    g.run_coral(0, CoralRef::Wall(order));
    // One drift draw each; nothing else.
    for _ in &ids {
        Vec2::random_unit(&mut expect);
    }
    assert_eq!(g.rng.state(), expect.state());
    for (&id, was) in ids.iter().zip(&before) {
        let m = g.world.mycelia().get(id).unwrap();
        assert!(m.culled);
        assert_eq!(&m.points, was);
    }
}

/// The installed `sounds.txt` line of `SS_Mycelia_Spark`, one clip.
fn spark_sounds() -> Sounds {
    let text = "Volume: 6\nVolume exponent: 1.8\n\
                SS_Mycelia_Spark/DopplerFac=0 : MyceliaSparkA/vol=0.25/RangeFac=0.5\n";
    let names = ClipNames::new(["MyceliaSparkA".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()))
}

/// A strand of another owner (an overseer's) put beside a live wall strand's base links with
/// a wall strand within a few hundred ticks (the registry pick lands on it or on the
/// other): both ends hold the one connection.
#[test]
fn another_owners_strand_links_with_a_wall_strand() {
    let mut g = coral_game(WALL, &[], 3);
    let (wall_owner, target) = {
        let w = &g
            .world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .wall_mycelia[0];
        let live = w
            .mycelia
            .iter()
            .position(|&id| !g.world.mycelia().get(id).unwrap().culled)
            .expect("a live strand");
        (w.owner, w.roots.root_positions[live])
    };
    let at = PointOwner(target + Vec2::new(0.0, 10.0));
    let foreign = g.world.mycelia.new_owner();
    let id = g.world.mycelia.add(
        Some(SystemId(1)),
        (foreign, &at),
        0,
        60.0,
        at.0,
        &mut UnityRandom::new(2),
    );
    g.world.mycelia.get_mut(id).unwrap().use_static_culling = false;
    for _ in 0..600 {
        g.tick(Input::NONE, &mut WithGraphics);
        let mut pool = std::mem::take(&mut g.world.mycelia);
        let mut rng = g.rng.clone();
        let env = g
            .world
            .realized(0)
            .unwrap()
            .mycelium_env(0, g.camera_view());
        pool.update(id, &at, &env, &mut rng);
        g.world.mycelia = pool;
        g.rng = rng;
        if g.world.mycelia().get(id).unwrap().connection.is_some() {
            break;
        }
    }
    let m = g.world.mycelia().get(id).unwrap();
    let connection = m.connection.expect("linked");
    let other = g.world.mycelia().get(connection.other(id)).unwrap();
    assert_eq!(other.owner, wall_owner);
    assert_eq!(other.connection, Some(connection));
    assert!(
        m.base()
            .dist_less(other.base(), (m.length + other.length) * 0.75 + 5.0)
    );
}

/// The sparks strands strike join the room's update list as cosmetics; the first update
/// plays `SS_Mycelia_Spark` at the spark (to the camera's room's sounds).
#[test]
fn a_spark_joins_the_room_and_plays_its_sound() {
    let mut g = coral_game_with(WALL, &[], 2, spark_sounds());
    let at = Vec2::new(120.0, 80.0);
    let spark = NeuronSpark::new(at, &mut g.rng);
    g.world.mycelia.sparks.push(spark);
    g.flush_mycelium_sparks(0);
    assert!(g.world.mycelia().sparks.is_empty());
    let (id, order) = {
        let c = g
            .world
            .room_cosmetics(0)
            .iter()
            .find(|c| matches!(c.kind, CosmeticKind::NeuronSpark(_)))
            .expect("spark");
        (c.id, c.update_order)
    };
    assert!(
        order
            > g.world
                .realized(0)
                .unwrap()
                .coral
                .as_ref()
                .unwrap()
                .wall_mycelia[0]
                .update_order
    );
    g.sounds.take_events();
    g.run_cosmetic(0, id);
    let events = g.sounds.take_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        SoundEvent::Play(p) => assert_eq!(p.id, SoundId::SsMyceliaSpark),
        other => panic!("{other:?}"),
    }
    g.run_cosmetic(0, id);
    assert!(g.sounds.take_events().is_empty());
}

/// An observer whose overseers' modules are one strand each, made and moved as
/// `OverseerGraphics` does (no system at the constructor, then `UpdateNeuronSystemForMycelia`).
#[derive(Default)]
struct StrandObserver {
    owner: Option<OwnerKey>,
    strands: Vec<MyceliumId>,
    removed: usize,
}

impl PlayerObserver for StrandObserver {
    fn created(&mut self, _player: &Player, _rng: &mut UnityRandom) {}
    fn updated(
        &mut self,
        _player: &Player,
        _room: &Room,
        _water: Option<&Water>,
        _around: &PlayerSurroundings<'_>,
        _rng: &mut UnityRandom,
    ) {
    }
    fn spat_out(&mut self, _player: &Player, _rng: &mut UnityRandom) {}
    fn has_player_graphics(&self) -> bool {
        true
    }
    fn overseer_added(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        rng: &mut UnityRandom,
    ) -> bool {
        let owner = mx.pool.new_owner();
        let at = PointOwner(view.body.chunk().pos);
        let id = mx
            .pool
            .add(view.neuron_system, (owner, &at), 0, 60.0, at.0, rng);
        self.owner = Some(owner);
        self.strands.push(id);
        true
    }
    fn overseer_new_room(
        &mut self,
        _view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        new: Option<SystemId>,
    ) {
        for &id in &self.strands {
            mx.pool.move_to_system(id, new);
        }
    }
    fn overseer_removed(&mut self, _id: crate::creature::EntityId) -> Option<OwnerKey> {
        self.removed += 1;
        self.owner
    }
}

/// The session's first room, whichever of the camera's first view (`NowViewed`, the head of
/// the first tick) and the loading step (`ReadyForAI` realizing the overseer, then
/// `AIMapReady`) comes first: the overseer's strand joins the system in `NewRoom` before
/// the wall mycelia are made, so the registry is the overseer's strand, then the wall's.
#[test]
fn the_first_rooms_overseer_registers_before_the_wall() {
    for view_first in [true, false] {
        let mut observer = StrandObserver::default();
        let mut g = coral_game_observed(WALL, &[], 0, Sounds::default(), true, &mut observer);
        if view_first {
            g.apply_insect_view_change();
            g.finish_loading(&mut observer);
        } else {
            g.finish_loading(&mut observer);
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
            g.apply_insect_view_change();
        }
        let system = g.world.realized(0).unwrap().coral.as_ref().unwrap();
        let registry = g.world.mycelia().registry(system.id);
        let wall = &system.wall_mycelia[0];
        assert_eq!(registry.len(), 1 + wall.mycelia.len(), "{view_first}");
        assert_eq!(registry[0], observer.strands[0]);
        assert_eq!(&registry[1..], &wall.mycelia[..]);
        let owner = g.world.mycelia().get(registry[0]).unwrap().owner;
        assert_eq!(Some(owner), observer.owner);
        assert_ne!(owner, wall.owner);
    }
}

/// Overseers' modules made and dropped again and again (`AddObject`/`NowViewed`, then
/// `NoLongerViewed`): in a room without a system their strands are freed with the module, so
/// the pool stays as it was; in a room with one each stays as an entry of its registry (as the
/// game's do), until the room goes.
#[test]
fn dropped_modules_do_not_pile_up_strands() {
    for (placed, kept) in [("", false), (WALL, true)] {
        let mut observer = StrandObserver::default();
        let mut g = coral_game_observed(placed, &[], 0, Sounds::default(), true, &mut observer);
        g.apply_insect_view_change();
        g.finish_loading(&mut observer);
        let cid = g
            .world
            .creatures()
            .find(|(_, c)| c.kind() == CreatureType::Overseer)
            .map(|(id, _)| id)
            .expect("overseer");
        let before = g.world.mycelia().live();
        for _ in 0..50 {
            assert!(g.overseer_module_added(cid, 0, &mut observer));
            g.overseer_module_removed(cid, &mut observer);
        }
        assert_eq!(observer.removed, 50);
        let grown = g.world.mycelia().live() - before;
        assert_eq!(grown, if kept { 50 } else { 0 }, "{placed}");
        if kept {
            let system = g.world.realized(0).unwrap().coral.as_ref().unwrap().id;
            g.world.mycelia.drop_registry(system);
            assert_eq!(g.world.mycelia().live(), before);
        }
    }
}

/// Every camera move bumps `camerasChangedTick` of the room it shows: the app's cut to
/// another camera position, also to the one it already shows (`camera_moved`).
#[test]
fn every_camera_move_bumps_the_tick() {
    let mut g = coral_game(WALL, &[], 1);
    let first = g.world.realized(0).unwrap().cameras_changed_tick;
    g.camera_moved();
    g.camera_moved();
    assert_eq!(g.world.realized(0).unwrap().cameras_changed_tick, first + 2);
    g.set_camera_view(0, 0);
    assert_eq!(g.world.realized(0).unwrap().cameras_changed_tick, first + 2);
}
