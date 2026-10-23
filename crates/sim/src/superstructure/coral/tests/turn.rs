//! The coral in a test room: `Room.Loaded` making the system, `AIMapReady` making the wall
//! mycelia from the placed objects (after the camera's first view), their turns in the
//! update list with their draws, the strands' cull while the system is frozen.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, PlacedObject, Room, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use crate::effects::lights::turn::tests::WithGraphics;
use rw_assets::{ClipNames, SoundTable};

use super::{CoralRef, RoomObject};
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::overseer::OverseerView;
use crate::tests::room_with_water;
use crate::world::World;
use crate::{
    CosmeticKind, Input, MyceliaCx, MyceliumId, NeuronSpark, OwnerKey, Player, PlayerObserver,
    PlayerSurroundings, PointOwner, RainWorldGame, SoundEvent, SoundId, Sounds, Start, SystemId,
    Vec2, Water,
};

/// A 40 x 20 room with three rows of floor, the player standing on it at (2, 3).
pub(crate) fn coral_game(placed: &str, effects: &[(&str, f32)], ticks: usize) -> RainWorldGame {
    coral_game_with(placed, effects, ticks, Sounds::default())
}

/// [`coral_game`] with the room's `Sounds`.
pub(crate) fn coral_game_with(
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

/// Two neurons, one wall place: `AIMapReady` makes the neurons as the placed loop meets them
/// (settings order), each with its vine registered, then the `WallMycelia`, so the
/// update-list places grow neuron, neuron, wall; all draw from their own seeds.
#[test]
fn neurons_are_made_in_the_placed_loop_before_the_wall_mycelia() {
    let placed = format!("CoralNeuron><200><120><0~100, {WALL}, CoralNeuron><500><120><0~160");
    let g = coral_game(&placed, &[], 1);
    let r = g.world.realized(0).unwrap();
    let system = r.coral.as_ref().unwrap();
    assert_eq!(system.neurons.len(), 2);
    let (a, b) = (&system.neurons[0], &system.neurons[1]);
    assert_eq!(a.geometry.segments.len(), 5);
    assert_eq!(b.geometry.segments.len(), 8);
    let wall = &system.wall_mycelia[0];
    assert!(system.update_order < a.update_order);
    assert!(a.update_order < b.update_order && b.update_order < wall.update_order);
    // The vines are registered in order, the first making the room's system, which takes
    // its place in the update list at that point (before the neuron's own).
    let made = r.climbable_vines.as_ref().unwrap();
    assert!(system.update_order < made.update_order && made.update_order < a.update_order);
    let vines = made.vines();
    assert_eq!(
        vines,
        [
            crate::VineRef::CoralNeuron {
                room: 0,
                order: a.update_order
            },
            crate::VineRef::CoralNeuron {
                room: 0,
                order: b.update_order
            },
        ]
    );
    // Both neurons' strands (3 and 6 pairs) and the wall's are in the one registry.
    assert_eq!(
        g.world.mycelia().registry(SystemId(1)).len(),
        6 + 12 + wall.mycelia.len()
    );
    // Update list, newest first: the wall, the neurons.
    let list: Vec<RoomObject> = g
        .room_update_list(0)
        .into_iter()
        .map(|(_, o)| o)
        .filter(|o| matches!(o, RoomObject::Coral(_)))
        .collect();
    assert_eq!(
        list,
        [
            RoomObject::Coral(CoralRef::Wall(wall.update_order)),
            RoomObject::Coral(CoralRef::Neuron(b.update_order)),
            RoomObject::Coral(CoralRef::Neuron(a.update_order)),
            RoomObject::Coral(CoralRef::System),
        ]
    );
}

/// A neuron's turn in the update list: the drift of its first tick moves the segments and a
/// frozen system stops it.
#[test]
fn a_neuron_updates_in_its_turn_until_the_system_freezes() {
    let mut g = coral_game("CoralNeuron><200><120><0~100", &[], 1);
    let order = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0].update_order;
    let before = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0]
        .geometry
        .segments
        .clone();
    g.run_coral(0, CoralRef::Neuron(order));
    let after = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0]
        .geometry
        .segments
        .clone();
    assert_ne!(after, before);
    {
        let s = g.world.realized_mut(0).unwrap().coral.as_mut().unwrap();
        s.age = 400;
        s.not_viewed_counter = 100;
    }
    let state = g.rng.state();
    g.run_coral(0, CoralRef::Neuron(order));
    assert_eq!(g.rng.state(), state);
    let frozen = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0]
        .geometry
        .segments
        .clone();
    assert_eq!(frozen, after);
}

/// A room with the `SuperStructureProjector` effect gets the neuron's circles in its
/// update list (before the neuron), turned by their own `Update`; one without gets none.
#[test]
fn a_projector_room_adds_circles_to_the_update_list() {
    let g = coral_game(
        "CoralNeuron><200><120><0~100",
        &[("SuperStructureProjector", 0.5)],
        1,
    );
    let r = g.world.realized(0).unwrap();
    let neuron = &r.coral.as_ref().unwrap().neurons[0];
    // Free ends, 100 long: circle 1 only.
    assert_eq!(r.projections.circles.len(), 1);
    let circle = &r.projections.circles[0];
    assert_eq!(circle.index, 1);
    assert!(circle.update_order < neuron.update_order);
    assert_eq!(
        circle.owner,
        crate::CircleOwner::Neuron {
            order: neuron.update_order
        }
    );
    let without = coral_game("CoralNeuron><200><120><0~100", &[], 1);
    assert!(
        without
            .world
            .realized(0)
            .unwrap()
            .projections
            .circles
            .is_empty()
    );
}

/// The circles' depth update runs at the first draw after each of their updates.
#[test]
fn circles_read_their_depth_when_drawn() {
    let mut g = coral_game(
        "CoralNeuron><200><120><0~100",
        &[("SuperStructureProjector", 0.5)],
        2,
    );
    assert!(g.world.realized(0).unwrap().projections.circles[0].update_depth);
    g.update_circle_depths();
    let c = &g.world.realized(0).unwrap().projections.circles[0];
    assert!(!c.update_depth);
    // No camera picture: sky (1), `Lerp(0, 1, 0.5)`.
    assert_eq!(c.depth_offset, 0.5);
}

/// The grab: the player under a neuron holding up takes hold of it (the vine, the
/// `Grab_Neuron` sound), and climbing pushes the neuron's segments live.
#[test]
fn the_player_climbs_a_neuron() {
    let text = "Volume: 6\nVolume exponent: 1.8\nGrab_Neuron : clip\n";
    let names = ClipNames::new(["clip".to_owned()], None);
    let sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    let mut g = coral_game_with("CoralNeuron><50><100><0~100", &[], 1, sounds);
    // The neuron runs up from (50, 100) to (50, 200); the player stands right under it.
    {
        let p = g.player.as_mut().unwrap();
        for c in &mut p.body.chunks {
            c.pos = Vec2::new(50.0, 105.0);
            c.last_pos = c.pos;
            c.vel = Vec2::ZERO;
        }
    }
    g.sounds.take_events();
    let before: Vec<Vec2> = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0]
        .geometry
        .segments
        .iter()
        .map(|s| s[0])
        .collect();
    g.tick(Input::new(0, 1, false), &mut WithGraphics);
    g.tick(Input::new(0, 1, false), &mut WithGraphics);
    let p = g.player.as_ref().unwrap();
    assert_eq!(p.animation(), crate::Animation::VineGrab);
    let order = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0].update_order;
    assert_eq!(
        p.vine_pos().unwrap().vine,
        crate::VineRef::CoralNeuron { room: 0, order }
    );
    let heard = g
        .sounds
        .take_events()
        .into_iter()
        .any(|e| matches!(e, SoundEvent::Play(p) if p.id == SoundId::GrabNeuron));
    assert!(heard);
    // The climber's weight reached the neuron (`ConnectChunkToVine` pushes the points).
    let after: Vec<Vec2> = g.world.realized(0).unwrap().coral.as_ref().unwrap().neurons[0]
        .geometry
        .segments
        .iter()
        .map(|s| s[0])
        .collect();
    assert_ne!(after, before);
}

/// A room leaving realization takes its neurons' strands with it (their owners are released
/// and the registry is dropped), as the wall mycelia's.
#[test]
fn abstracting_the_room_frees_the_neurons_strands() {
    let mut g = coral_game(&format!("CoralNeuron><200><120><0~100, {WALL}"), &[], 1);
    assert!(g.world.mycelia().live() > 12);
    g.world.abstractize(0, &mut UnityRandom::new(1));
    assert_eq!(g.world.mycelia().live(), 0);
}

/// A stem 120 px wide (size 0.2) in the open above the floor.
const STEM: &str = "CoralStem><300><100><0~120";

/// `AIMapReady` makes the stems after the wall mycelia (the neurons came in the placed loop):
/// the climbable vines system is registered by the first vine, the stem next and its
/// neurons after it; the segments take their places in the update list in the order they
/// are made, the stem's circle after the last; the stem itself is not in the update list.
#[test]
fn stems_are_made_after_the_wall_and_neurons() {
    let placed = format!("CoralNeuron><200><120><0~100, {WALL}, {STEM}");
    let g = coral_game(&placed, &[("SuperStructureProjector", 0.5)], 1);
    let r = g.world.realized(0).unwrap();
    let system = r.coral.as_ref().unwrap();
    assert_eq!(system.stems.len(), 1);
    let stem = &system.stems[0];
    assert!(stem.segments.len() > 3);
    assert_eq!(stem.size, 0.2);
    let wall = &system.wall_mycelia[0];
    assert!(wall.update_order < stem.segments[0].update_order);
    let orders: Vec<u64> = stem.segments.iter().map(|s| s.update_order).collect();
    assert!(orders.windows(2).all(|w| w[0] < w[1]));
    let vines = r.climbable_vines.as_ref().unwrap().vines();
    assert_eq!(
        vines,
        [
            crate::VineRef::CoralNeuron {
                room: 0,
                order: system.neurons[0].update_order
            },
            crate::VineRef::CoralStem { room: 0, stem: 0 },
        ]
    );
    // The neuron's circle, then the stem's (after its last segment).
    let circles = &r.projections.circles;
    assert_eq!(circles.len(), 2);
    assert_eq!(circles[1].owner, crate::CircleOwner::Stem { stem: 0 });
    assert!(*orders.last().unwrap() < circles[1].update_order);
    // One entity ID a segment, from the game's counter.
    let first = stem.segments[0].id.number;
    assert!(
        stem.segments
            .iter()
            .enumerate()
            .all(|(k, s)| s.id.number == first + k as i32)
    );
    let segments: Vec<RoomObject> = g
        .room_update_list(0)
        .into_iter()
        .map(|(_, o)| o)
        .filter(|o| matches!(o, RoomObject::Coral(CoralRef::Segment(_))))
        .collect();
    let newest_first: Vec<RoomObject> = orders
        .iter()
        .rev()
        .map(|&o| RoomObject::Coral(CoralRef::Segment(o)))
        .collect();
    assert_eq!(segments, newest_first);
}

/// A `CoralStemWithNeurons` makes a neuron in some of its segments: each neuron is added to
/// the system and the vines after the stem, and its place in the update list is before the
/// segment that made it.
#[test]
fn a_stem_with_neurons_registers_its_neurons_after_itself() {
    let g = coral_game("CoralStemWithNeurons><300><100><0~360", &[], 1);
    let r = g.world.realized(0).unwrap();
    let system = r.coral.as_ref().unwrap();
    let stem = &system.stems[0];
    let made: Vec<(u64, u64)> = stem
        .segments
        .iter()
        .filter_map(|s| Some((s.neuron?, s.update_order)))
        .collect();
    assert!(!made.is_empty());
    assert_eq!(system.neurons.len(), made.len());
    for ((neuron, segment), n) in made.iter().zip(&system.neurons) {
        assert_eq!(*neuron, n.update_order);
        assert!(neuron < segment);
    }
    let vines = r.climbable_vines.as_ref().unwrap().vines();
    assert_eq!(vines[0], crate::VineRef::CoralStem { room: 0, stem: 0 });
    assert_eq!(vines.len(), 1 + made.len());
    assert_eq!(
        vines[1],
        crate::VineRef::CoralNeuron {
            room: 0,
            order: made[0].0
        }
    );
}

/// The segments are in `physicalObjects[0]`: the water, the electric fixtures and the
/// disruptor walk them with the other bodies, in the order they joined the layer.
#[test]
fn the_segments_are_layer_zero_physical_objects() {
    let g = coral_game(STEM, &[], 1);
    let stem = &g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0];
    let list = g.world.physical_objects(0, Some(g.player_seq));
    let segments: Vec<(i32, u64)> = list
        .iter()
        .filter(|(_, _, o)| matches!(o, crate::ChunkOwner::Segment(_)))
        .map(|&(layer, seq, _)| (layer, seq))
        .collect();
    let expected: Vec<(i32, u64)> = stem.segments.iter().map(|s| (0, s.update_order)).collect();
    assert_eq!(segments, expected);
    let segment = list
        .iter()
        .find_map(|&(_, _, o)| match o {
            crate::ChunkOwner::Segment(r) => Some(r),
            _ => None,
        })
        .expect("a segment");
    assert_eq!(g.world.segment_chunks(segment).unwrap().len(), 2);
}

/// A segment's turn in the update list moves its chunks (gravity and the rod); a frozen
/// system stops it, with no draws.
#[test]
fn a_segment_updates_in_its_turn_until_the_system_freezes() {
    let mut g = coral_game(STEM, &[], 1);
    let order =
        g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].segments[2].update_order;
    let before = g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].segments[2]
        .body
        .chunks
        .clone();
    g.run_coral(0, CoralRef::Segment(order));
    let after = g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].segments[2]
        .body
        .chunks
        .clone();
    assert_ne!(after, before);
    {
        let s = g.world.realized_mut(0).unwrap().coral.as_mut().unwrap();
        s.age = 400;
        s.not_viewed_counter = 100;
    }
    let state = g.rng.state();
    g.run_coral(0, CoralRef::Segment(order));
    assert_eq!(g.rng.state(), state);
    let frozen = g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].segments[2]
        .body
        .chunks
        .clone();
    assert_eq!(frozen, after);
}

/// The grab: the player under a stem holding up takes hold of it (the vine, the
/// `Grab_Coral_Stem` sound) and its weight reaches the chunks.
#[test]
fn the_player_climbs_a_stem() {
    let text = "Volume: 6\nVolume exponent: 1.8\nGrab_Coral_Stem : clip\n";
    let names = ClipNames::new(["clip".to_owned()], None);
    let sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    let mut g = coral_game_with(STEM, &[], 1, sounds);
    let at = g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].segments[2]
        .body
        .chunks[0]
        .pos;
    {
        let p = g.player.as_mut().unwrap();
        for c in &mut p.body.chunks {
            c.pos = at - Vec2::new(0.0, 5.0);
            c.last_pos = c.pos;
            c.vel = Vec2::ZERO;
        }
    }
    g.sounds.take_events();
    let before = g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].clone();
    g.tick(Input::new(0, 1, false), &mut WithGraphics);
    g.tick(Input::new(0, 1, false), &mut WithGraphics);
    let p = g.player.as_ref().unwrap();
    assert_eq!(p.animation(), crate::Animation::VineGrab);
    assert_eq!(
        p.vine_pos().unwrap().vine,
        crate::VineRef::CoralStem { room: 0, stem: 0 }
    );
    let heard = g
        .sounds
        .take_events()
        .into_iter()
        .any(|e| matches!(e, SoundEvent::Play(p) if p.id == SoundId::GrabCoralStem));
    assert!(heard);
    let after = &g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0];
    assert_ne!(
        after.segments[2].body.chunks,
        before.segments[2].body.chunks
    );
}

/// A room leaving realization takes its segments' strands with it.
#[test]
fn abstracting_the_room_frees_the_segments_strands() {
    let mut g = coral_game(STEM, &[], 1);
    assert!(g.world.mycelia().live() > 0);
    g.world.abstractize(0, &mut UnityRandom::new(1));
    assert_eq!(g.world.mycelia().live(), 0);
}
