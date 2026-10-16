//! The plain abstract AI on a three-room test world. Expected values follow the
//! game's code (`AbstractCreatureAI` 302062, `AbstractSpacePathFinder` 16334,
//! `AbstractCreature.InDenUpdate` 300649); each derivation is in the comments.
//!
//! The world, for the green lizard's layer (2), costs and lengths per node pair:
//! - A (0), 40 x 30: node 0 Exit to B (tube 4), node 1 Den; 0-1 cost 10, length 8.
//! - B (1), 60 x 40: node 0 Exit to A (tube 5), node 1 Exit to C (tube 6), node 2 Den;
//!   0-1 cost 30 length 25, 0-2 cost 12 length 9, 1-2 cost 20 length 15.
//! - C (2), 30 x 30: node 0 Exit to B (tube 7), node 1 Den; 0-1 cost 8, length 6.
//! - The off-screen den (3).
//!
//! No attractions: every room is Neutral (0.5), so `offscreenSpeedFac` is
//! `Lerp(0.1, 0.9, 0.25)`, 0.29999998 in floats, and a green's speed about 0.09 tiles a
//! tick.

use std::sync::Arc;

use rw_assets::world::{AbstractRoomData, FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, ConnMap, CreatureType, RoomSettings, SlugcatName, Timeline};
use rw_random::UnityRandom;

use crate::creature::{
    AbstractCreatureAi, AiKind, CreatureId, CreatureTemplate, EntityId, IdIssuer, PlayerGuideState,
    WorldCoordinate,
};
use crate::item::Entity;
use crate::room_script::StorySession;
use crate::tests::room;
use crate::world::{AbstractTick, World};

const LAYERS: usize = 15;
const GREEN_LAYER: usize = 2;

/// One room's node map line: `(type, tube length)` per node and the green's
/// `[cost, length]` from each node to each.
fn conn_map(nodes: &[(i32, i32)], green: &[&[[i32; 2]]]) -> ConnMap {
    let count = nodes.len();
    let mut line = format!("20|{count}|{LAYERS}|");
    for (i, (kind, tube)) in nodes.iter().enumerate() {
        line.push_str(&format!("{kind},{tube},0,0,1,"));
        for layer in 0..LAYERS {
            for to in green[i] {
                let cell = if layer == GREEN_LAYER { *to } else { [-1, -1] };
                line.push_str(&format!("{} {},", cell[0], cell[1]));
            }
        }
        line.push('|');
    }
    ConnMap::parse(&line).expect("node map")
}

fn abstract_room(
    template: &AbstractRoomData,
    name: &str,
    index: i32,
    size: (i32, i32),
    connections: Vec<i32>,
    map: ConnMap,
) -> AbstractRoomData {
    AbstractRoomData {
        name: name.to_owned(),
        index,
        connections,
        nodes: map.nodes,
        size,
        swarm_room_index: -1,
        shelter_index: -1,
        gate_index: -1,
        tags: Vec::new(),
        offscreen_den: false,
        ..template.clone()
    }
}

/// The test world, rain far off (a cycle of 100000 ticks).
fn world() -> World {
    let base = WorldData::from_room(&room(&["#"]));
    let mut data = base.clone();
    let template = &base.rooms[0];
    let n = [-1, -1];
    data.name = "TT".to_owned();
    data.single_room = false;
    data.swarm_rooms = Vec::new();
    data.rooms = vec![
        abstract_room(
            template,
            "TT_A",
            0,
            (40, 30),
            vec![1],
            conn_map(&[(0, 4), (1, 3)], &[&[n, [10, 8]], &[[10, 8], n]]),
        ),
        abstract_room(
            template,
            "TT_B",
            1,
            (60, 40),
            vec![0, 2],
            conn_map(
                &[(0, 5), (0, 6), (1, 2)],
                &[
                    &[n, [30, 25], [12, 9]],
                    &[[30, 25], n, [20, 15]],
                    &[[12, 9], [20, 15], n],
                ],
            ),
        ),
        abstract_room(
            template,
            "TT_C",
            2,
            (30, 30),
            vec![1],
            conn_map(&[(0, 7), (1, 2)], &[&[n, [8, 6]], &[[8, 6], n]]),
        ),
        AbstractRoomData {
            index: 3,
            ..base.rooms[1].clone()
        },
    ];
    let rooms = Arc::new(FixedRoom(LoadedRoom {
        room: room(&["#"]),
        settings: RoomSettings::ANCESTOR,
    }));
    let mut world = World::new(data, rooms, Timeline::White, ActiveMods::NONE, true);
    world.rain_cycle.cycle_length = 100_000;
    world
}

/// A green lizard made at `pos` (a den node with no tile draws its den counter).
fn green(world: &mut World, pos: WorldCoordinate, rng: &mut UnityRandom) -> CreatureId {
    let session = StorySession {
        character: SlugcatName::White,
        cycle_number: 0,
        den_position: String::new(),
    };
    let mut ids = IdIssuer {
        next_issued_id: 1000,
    };
    let template = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
    let c = world.new_creature(template, pos, EntityId::new(-1, 1), &session, &mut ids, rng);
    world.insert_creature(c)
}

/// A green lizard standing at `pos`, out of any den.
fn green_out(world: &mut World, pos: WorldCoordinate) -> CreatureId {
    let mut rng = UnityRandom::new(0);
    let id = green(world, pos, &mut rng);
    world.add_entity(id);
    id
}

fn at(room: i32, node: i32) -> WorldCoordinate {
    WorldCoordinate::new(room, -1, -1, node)
}

/// Runs `f` with an abstract tick on `rng`.
struct Ticker {
    guide: PlayerGuideState,
    ids: IdIssuer,
    session: StorySession,
}

impl Ticker {
    fn new() -> Self {
        Self {
            guide: PlayerGuideState::default(),
            ids: IdIssuer {
                next_issued_id: 1000,
            },
            session: StorySession {
                character: SlugcatName::White,
                cycle_number: 0,
                den_position: String::new(),
            },
        }
    }

    fn tick<'a>(&'a mut self, rng: &'a mut UnityRandom) -> AbstractTick<'a> {
        AbstractTick {
            rng,
            guide: &self.guide,
            player: None,
            player_karma: 0,
            cycle: 0,
            ids: &mut self.ids,
            session: &self.session,
        }
    }
}

#[test]
fn a_green_in_its_den_counts_down_then_comes_out_without_drawing() {
    let mut world = world();
    let mut rng = UnityRandom::new(5);
    let id = green(&mut world, at(0, 1), &mut rng);
    world.move_entity_to_den(id, &mut rng);
    // The constructor's `Range(100, 1000)`, the den and spawn den.
    let mut model = UnityRandom::new(5);
    let counter = model.range_i32(100, 1000);
    assert_eq!(rng.state(), model.state());
    let c = world.creature(id);
    assert_eq!(c.remain_in_den_counter, counter);
    assert_eq!(c.ai.as_ref().and_then(|a| a.den_position), Some(at(0, 1)));
    assert_eq!(c.distance_to_my_node, 0, "an int field's default");
    // Each room update subtracts `NumberOfRooms` (4); out when below 0.
    let updates = counter / 4 + 1;
    let mut ticker = Ticker::new();
    for k in 1..=updates {
        assert!(world.creature(id).in_den, "in at update {k}");
        world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));
    }
    let c = world.creature(id);
    assert!(!c.in_den);
    assert_eq!(c.remain_in_den_counter, -1);
    assert_eq!(c.time_spent_here, 0, "IsExitingDen resets it");
    assert!(world.room_creatures(0).contains(&id));
    assert!(world.room_dens(0).is_empty());
    assert_eq!(rng.state(), model.state(), "no draws in the den");
}

#[test]
fn rain_pre_cycle_and_night_keep_a_green_in() {
    let mut ticker = Ticker::new();
    // Rain within 60 s: `WantToStayInDenUntilEndOfCycle`, counter -1 for good.
    let mut world = world();
    let mut rng = UnityRandom::new(5);
    let id = green(&mut world, at(0, 1), &mut rng);
    world.move_entity_to_den(id, &mut rng);
    world.rain_cycle.timer = 100_000 - 2399;
    world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));
    assert_eq!(world.creature(id).remain_in_den_counter, -1);
    for _ in 0..300 {
        world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));
    }
    assert!(world.creature(id).in_den);

    // `PreCycle` in the spawn data: no precycles in the base game, so it stays.
    let mut world = world_with_flags("{PreCycle}");
    let id = CreatureId(0);
    world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));
    assert!(world.creature(id).pre_cycle);
    assert_eq!(world.creature(id).remain_in_den_counter, -1);

    // `Night`: the counter waits for the day-night counter to pass 600.
    let mut world = world_with_flags("{Night}");
    let before = world.creature(id).remain_in_den_counter;
    world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));
    assert_eq!(world.creature(id).remain_in_den_counter, before);
    world.rain_cycle.day_night_counter = 601;
    world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));
    assert_eq!(world.creature(id).remain_in_den_counter, before - 4);
}

/// A world with one green in A's den whose spawn data is `data` (`setCustomFlags`).
fn world_with_flags(data: &str) -> World {
    let mut world = world();
    let mut rng = UnityRandom::new(5);
    let id = green(&mut world, at(0, 1), &mut rng);
    let c = world.creature_mut(id);
    c.night_creature = true;
    c.spawn_data = Some(data.to_owned());
    c.set_custom_flags();
    world.move_entity_to_den(id, &mut rng);
    world
}

#[test]
fn spawn_flags_reset_the_night_flag() {
    // `setCustomFlags` clears `nightCreature` before reading the list: a lineage's
    // night field alone does not survive it.
    let world = world_with_flags("{0.3}");
    let c = world.creature(CreatureId(0));
    assert!(!c.night_creature && !c.ignore_cycle && !c.pre_cycle);
    let world = world_with_flags("{Ignorecycle,Night}");
    let c = world.creature(CreatureId(0));
    assert!(c.night_creature && !c.ignore_cycle);
}

#[test]
fn an_idle_green_rolls_to_migrate_then_to_roam() {
    // In A at node 1, no path, its destination where it stands: `MigrationBehavior`
    // draws `Range(0, abstractedLaziness = 50)` against `timeSpentHere` (4 after this
    // update), then (when that passes) `TimeInfluencedRandomRoll(0.1 * Lerp(2, 0.1,
    // 0.5) = 0.105, 4)`; not leaving, `TimeInfluencedRandomRoll(roamInRoomChance 0.1,
    // 4)`, and on success `RandomMoveWithinRoom`'s `Range(0, 2)`.
    let mut ticker = Ticker::new();
    for seed in 0..40 {
        let mut world = world();
        let id = green_out(&mut world, at(0, 1));
        let mut rng = UnityRandom::new(seed);
        world.update_abstract_room(0, 4, &mut ticker.tick(&mut rng));

        let mut m = UnityRandom::new(seed);
        let lazy = m.range_i32(0, 50);
        let leave = 4 > lazy && (0..4).any(|_| m.value() < 0.105);
        if leave {
            // Leaving draws on: skip, the other tests pin those draws.
            continue;
        }
        let roam = (0..4).any(|_| m.value() < 0.1);
        let ai = world.creature(id).ai.as_ref().expect("AI");
        if roam {
            let node = m.range_i32(0, 2);
            if node == 0 {
                // A new destination in the room (`FindPath`: same room, connected).
                assert_eq!(ai.destination, at(0, 0), "seed {seed}");
                assert_eq!(ai.path, vec![at(0, 0)], "seed {seed}");
            } else {
                assert!(ai.path.is_empty(), "seed {seed}");
            }
        } else {
            assert!(ai.path.is_empty(), "seed {seed}");
        }
        assert_eq!(ai.migration_destination, None);
        assert_eq!(rng.state(), m.state(), "seed {seed}");
    }
}

#[test]
fn follow_path_moves_once_the_buffer_covers_the_way() {
    // A node 1 -> node 0: length 8 / 0.09 = 88.9; `timeBuffer > 88.9` moves and pays
    // `(int)88.9 = 88`.
    let mut ticker = Ticker::new();
    let mut world = world();
    let id = green_out(&mut world, at(0, 1));
    let ai = world.creature_mut(id).ai.as_mut().expect("AI");
    // A path whose goal is not the destination would be dropped by `Update`'s check.
    ai.path = vec![at(0, 0)];
    ai.destination = at(0, 0);
    ai.time_buffer = 88;
    let mut rng = UnityRandom::new(1);
    let before = rng.state();
    world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
    assert_eq!(world.creature(id).pos, at(0, 1));
    world.creature_mut(id).ai.as_mut().expect("AI").time_buffer = 89;
    world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
    let c = world.creature(id);
    assert_eq!(c.pos, at(0, 0));
    let ai = c.ai.as_ref().expect("AI");
    assert_eq!(ai.time_buffer, 1);
    assert!(
        ai.path.is_empty(),
        "Moved cuts the path at the node reached"
    );
    // At its destination with no path left: nothing to reset, and no draws.
    assert_eq!(rng.state(), before);
}

#[test]
fn follow_path_takes_the_shortcut_into_the_next_room() {
    // A's exit (node 0) to B's entrance (node 0): both tubes, 4 + 5 = 9. In floats
    // `Lerp(0.1, 0.9, 0.25)` is 0.29999998 and 9 / (0.3 * 0.29999998) = 100.00001, so a
    // buffer of 100 waits, 101 moves and pays 100.
    let mut ticker = Ticker::new();
    let mut world = world();
    let id = green_out(&mut world, at(0, 0));
    let ai = world.creature_mut(id).ai.as_mut().expect("AI");
    ai.path = vec![at(1, 0)];
    ai.destination = at(1, 0);
    ai.time_buffer = 100;
    let mut rng = UnityRandom::new(1);
    world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
    assert_eq!(world.creature(id).pos, at(0, 0));
    world.creature_mut(id).ai.as_mut().expect("AI").time_buffer = 101;
    world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
    let c = world.creature(id);
    assert_eq!(c.pos, at(1, 0));
    assert_eq!(c.ai.as_ref().expect("AI").time_buffer, 1);
    assert_eq!(c.ai.as_ref().expect("AI").last_room, 0);
    assert!(world.room_creatures(1).contains(&id));
    assert!(!world.room_creatures(0).contains(&id));
}

#[test]
fn a_stuck_green_jumps_to_a_random_reachable_node() {
    // With no distance to its node (-1) and over 200 ticks buffered, `FollowPath` rolls
    // `TimeInfluencedRandomRoll(0.5, 4)`; on success `Range(0, 2)` over the mapped
    // nodes it can reach from node 1 (node 0 only: a node's path to itself is -1, so
    // the list is [0]), then `SetDestination(pos)` and the distance is 0.
    let mut ticker = Ticker::new();
    for seed in 0..10 {
        let mut world = world();
        let id = green_out(&mut world, at(0, 1));
        let c = world.creature_mut(id);
        c.distance_to_my_node = -1;
        let ai = c.ai.as_mut().expect("AI");
        ai.path = vec![at(0, 0)];
        ai.time_buffer = 201;
        let mut rng = UnityRandom::new(seed);
        world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
        let mut m = UnityRandom::new(seed);
        let stuck = (0..4).any(|_| m.value() < 0.5);
        let c = world.creature(id);
        if stuck {
            assert_eq!(m.range_i32(0, 1), 0);
            assert_eq!(c.pos, at(0, 0), "seed {seed}");
            assert_eq!(c.distance_to_my_node, 0);
        } else {
            assert_eq!(c.pos, at(0, 1), "seed {seed}");
        }
        assert_eq!(rng.state(), m.state(), "seed {seed}");
    }
}

#[test]
fn the_path_finder_floods_from_the_goal() {
    // A 0 to C 1 (no direct neighbour): from C 1 (0), C 0 at 8; from C 0 the tube to
    // B 1 at (7 + 6) * 1.5 + 10 = 29.5 -> 29, so 37 (and C 1 again at 16); from B 1,
    // B 2 at 57 and B 0 at 67; from B 0 the tube to A 0 at (5 + 4) * 1.5 + 10 = 23.5
    // -> 23: the start, at 90. The path runs goal first.
    let world = world();
    let template = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
    let finder = super::super::abstract_path::AbstractSpacePathFinder {
        world: &world,
        template: &template,
    };
    let mut rng = UnityRandom::new(1);
    let before = rng.state();
    let path = finder.path(at(0, 0), at(2, 1), &mut rng);
    assert_eq!(
        path,
        Some(vec![at(2, 1), at(2, 0), at(1, 1), at(1, 0), at(0, 0)])
    );
    // Into the next room: goal, entrance, then the exit when it starts elsewhere.
    assert_eq!(
        finder.path(at(0, 1), at(1, 2), &mut rng),
        Some(vec![at(1, 2), at(1, 0), at(0, 0)])
    );
    // In the room, connected: the goal alone.
    assert_eq!(
        finder.path(at(1, 0), at(1, 2), &mut rng),
        Some(vec![at(1, 2)])
    );
    assert_eq!(rng.state(), before, "defined, mapped nodes: no draws");
    // An undefined goal node draws `Range(0, nodes)`.
    let mut model = UnityRandom::new(1);
    let node = model.range_i32(0, 3);
    let path = finder.path(at(0, 0), at(1, -1), &mut rng).expect("path");
    assert_eq!(path[0], at(1, node));
    assert_eq!(rng.state(), model.state());
}

#[test]
fn set_destination_finds_the_path_and_migrates() {
    let mut world = world();
    let id = green_out(&mut world, at(0, 0));
    let mut rng = UnityRandom::new(1);
    world.set_destination(id, at(2, 1), &mut rng);
    let ai = world.creature(id).ai.as_ref().expect("AI");
    assert_eq!(ai.destination, at(2, 1));
    assert_eq!(ai.migration_destination, Some(at(2, 1)));
    assert_eq!(ai.path.len(), 5);
    // Already on the path's goal: no new search.
    world.set_destination(id, at(2, 1), &mut rng);
    assert_eq!(world.creature(id).ai.as_ref().expect("AI").path.len(), 5);
}

#[test]
fn a_random_walk_of_rooms_makes_two_draws_a_step() {
    // From A 1: step 1 has one option (B's entrance 0): `Range(0, 1)` and the weighted
    // `value`; step 2 from B 0 has C 0 (A is on the walk): two more; C's only neighbour
    // is on the walk, so it stops. Walk [C 0, B 1, B 0, A 0]; C (size-dependent 0.4946)
    // is less attractive than A (0.4962), so it is cut back while the first is less
    // attractive than the second or in the same room: C 0 (< B's 0.5035), B 1 (same
    // room as B 0) go, leaving [B 0, A 0]. The destination is B 0, a migration.
    let mut world = world();
    let id = green_out(&mut world, at(0, 1));
    let mut rng = UnityRandom::new(3);
    let mut model = UnityRandom::new(3);
    for _ in 0..2 {
        model.range_i32(0, 1);
        model.value();
    }
    world.random_move_to_other_room(id, 250, &mut rng);
    assert_eq!(rng.state(), model.state());
    let ai = world.creature(id).ai.as_ref().expect("AI");
    assert_eq!(ai.path, vec![at(1, 0), at(0, 0)]);
    assert_eq!(ai.destination, at(1, 0));
    assert_eq!(ai.migration_destination, Some(at(1, 0)));
}

#[test]
fn moving_to_a_more_attractive_neighbour_draws_per_candidate_and_once_more() {
    // From B 0: A (exit 0, its own node) and C (exit 1, connected) are candidates up to
    // the `value < 0.025` roll, as neither is more attractive (all Neutral): a draw
    // each, then the weighted pick's `value`.
    for seed in 0..20 {
        let mut world = world();
        let id = green_out(&mut world, at(1, 0));
        let mut rng = UnityRandom::new(seed);
        world.move_to_more_attractive_neighbor_room(id, &mut rng);
        let mut m = UnityRandom::new(seed);
        let a = m.value() < 0.025;
        let c = m.value() < 0.025;
        m.value();
        assert_eq!(rng.state(), m.state(), "seed {seed}");
        let ai = world.creature(id).ai.as_ref().expect("AI");
        if !a && !c {
            assert_eq!(ai.destination, at(1, 0), "seed {seed}: none taken");
        } else {
            assert_ne!(ai.destination.room, 1, "seed {seed}");
        }
    }
}

#[test]
fn a_green_passing_its_den_late_in_the_cycle_goes_in() {
    // `FollowPath` at a den node calls `OpportunityToEnterDen`: with the rain within
    // 60 s it wants to stay, so it goes in with counter -1.
    let mut ticker = Ticker::new();
    let mut world = world();
    let id = green_out(&mut world, at(0, 0));
    world.creature_mut(id).remain_in_den_counter = -1;
    let ai = world.creature_mut(id).ai.as_mut().expect("AI");
    ai.path = vec![at(0, 1)];
    ai.time_buffer = 100;
    let mut rng = UnityRandom::new(1);
    world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
    assert!(
        !world.creature(id).in_den,
        "early in the cycle it walks past"
    );
    let c = world.creature_mut(id);
    c.pos = at(0, 0);
    let ai = c.ai.as_mut().expect("AI");
    ai.path = vec![at(0, 1)];
    ai.time_buffer = 100;
    world.rain_cycle.timer = 100_000 - 100;
    world.abstract_ai_update(id, 4, &mut ticker.tick(&mut rng));
    let c = world.creature(id);
    assert!(c.in_den);
    assert_eq!(c.remain_in_den_counter, -1);
    assert!(world.room_dens(0).contains(&id));
}

/// A lantern mouse (a green's prey) with ID number `number`, standing at `pos`.
fn mouse_out(world: &mut World, pos: WorldCoordinate, number: i32) -> CreatureId {
    let session = StorySession {
        character: SlugcatName::White,
        cycle_number: 0,
        den_position: String::new(),
    };
    let mut ids = IdIssuer {
        next_issued_id: 1000,
    };
    let template = CreatureTemplate::of(CreatureType::LanternMouse, ActiveMods::NONE);
    let mut rng = UnityRandom::new(0);
    let c = world.new_creature(
        template,
        pos,
        EntityId::new(-1, number),
        &session,
        &mut ids,
        &mut rng,
    );
    let id = world.insert_creature(c);
    world.add_entity(id);
    id
}

/// A green out at A's den carrying a mouse (grasp 0, a carry stick).
fn green_with_prey(world: &mut World) -> (CreatureId, CreatureId) {
    let green = green_out(world, at(0, 1));
    let mouse = mouse_out(world, at(0, 1), 7);
    world.add_grip_stick(green, Entity::Creature(mouse), 0, true);
    (green, mouse)
}

#[test]
fn a_green_stows_its_prey_kills_it_and_drops_it_in_the_den() {
    // `OpportunityToEnterDen` (301182) with a counter of -1 and no rain: the stow branch
    // (a grip stick on a creature) sets 200 and goes in. `IsEnteringDen` (303528 then
    // 301221): the mouse comes into the den first, then dies (`AbstractCreature.Die`:
    // spawner -1, no respawn) and the plain AI drops it (`DoIwantToDropThisItemInDen`
    // true). No draws.
    let mut world = world();
    let (green, mouse) = green_with_prey(&mut world);
    world.creature_mut(green).remain_in_den_counter = -1;
    let mut rng = UnityRandom::new(4);
    let before = rng.state();
    world.opportunity_to_enter_den(green, &mut rng);
    assert_eq!(rng.state(), before);
    let g = world.creature(green);
    assert!(g.in_den);
    assert_eq!(g.remain_in_den_counter, 200);
    assert!(g.sticks.is_empty());
    let m = world.creature(mouse);
    assert!(m.in_den && !m.state.alive && m.sticks.is_empty());
    assert_eq!(world.room_dens(0), [mouse, green]);
    assert!(world.room_creatures(0).is_empty());

    // Coming out (`MoveEntityOutOfDen`), the green leaves the dropped mouse in the den.
    world.move_entity_out_of_den(Entity::Creature(green));
    assert!(!world.creature(green).in_den);
    assert_eq!(world.room_dens(0), [mouse]);
    assert_eq!(world.room_creatures(0), [green]);
}

#[test]
fn prey_killed_in_the_den_keeps_a_soon_leaving_creature_in_for_200_ticks() {
    // Counter 50 (> 0): in without stowing; the kill raises a counter in (-1, 200) to
    // 200. With the rain within 60 s it wants to stay: -1, and the kill keeps it.
    let mut world = world();
    let (green, _) = green_with_prey(&mut world);
    world.creature_mut(green).remain_in_den_counter = 50;
    let mut rng = UnityRandom::new(4);
    world.opportunity_to_enter_den(green, &mut rng);
    assert_eq!(world.creature(green).remain_in_den_counter, 200);

    let mut world = self::world();
    let (green, mouse) = green_with_prey(&mut world);
    world.rain_cycle.timer = 100_000 - 100;
    world.opportunity_to_enter_den(green, &mut rng);
    assert_eq!(world.creature(green).remain_in_den_counter, -1);
    assert!(!world.creature(mouse).state.alive);
}

#[test]
fn an_ai_that_keeps_its_prey_brings_it_out_of_the_den() {
    // A scavenger's `DoIwantToDropThisItemInDen` is false: the dead mouse stays stuck,
    // and `IsExitingDen` takes it out with the carrier (whose own `AddEntity` comes
    // after). Drop bugs drop creatures only.
    let mut world = world();
    let (green, mouse) = green_with_prey(&mut world);
    let plain = world.creature(green).ai.clone().expect("AI");
    world.creature_mut(green).ai.as_mut().expect("AI").kind = AiKind::Scavenger {
        carry_rocks: 0,
        dont_migrate: 0,
    };
    let mut rng = UnityRandom::new(4);
    world.move_entity_to_den(green, &mut rng);
    assert!(!world.creature(mouse).state.alive);
    assert_eq!(world.creature(green).sticks.len(), 1);
    world.move_entity_out_of_den(Entity::Creature(green));
    assert!(!world.creature(mouse).in_den);
    assert_eq!(world.room_creatures(0), [mouse, green]);
    assert!(world.room_dens(0).is_empty());

    let drop_bug = AbstractCreatureAi {
        kind: AiKind::DropBug,
        ..plain.clone()
    };
    assert!(drop_bug.wants_to_drop_in_den(true));
    assert!(!drop_bug.wants_to_drop_in_den(false));
    assert!(plain.wants_to_drop_in_den(false));
}

/// Gives creature `id` a relationship with ID number `number` of like `like`.
fn befriend(world: &mut World, id: CreatureId, number: i32, like: f32) {
    let m = world
        .creature_mut(id)
        .state
        .social_memory
        .as_mut()
        .expect("lizards have a social memory");
    m.get_or_initiate(EntityId::new(3, number)).like = like;
}

#[test]
fn a_strong_like_drawn_from_the_social_memory_starts_following() {
    // `MigrationBehavior` (302457) with no follow creature: `Range(0, count)` picks a
    // relationship; above 0.95 (either way) the first creature with that ID number in a
    // room's `creatures` is followed, true. A weaker one goes on to the laziness
    // `Range(0, 50)` against `timeSpentHere` 0, which never passes: false.
    for seed in 0..20 {
        let mut world = world();
        let green = green_out(&mut world, at(0, 1));
        let friend = mouse_out(&mut world, at(2, 1), 7);
        befriend(&mut world, green, 6, 0.5);
        befriend(&mut world, green, 7, -0.97);
        let mut rng = UnityRandom::new(seed);
        let followed = world.migration_behavior(green, 4, &mut rng);
        let mut m = UnityRandom::new(seed);
        let strong = m.range_i32(0, 2) == 1;
        if !strong {
            m.range_i32(0, 50);
        }
        assert_eq!(rng.state(), m.state(), "seed {seed}");
        assert_eq!(followed, strong, "seed {seed}");
        let follow = world
            .creature(green)
            .ai
            .as_ref()
            .expect("AI")
            .follow_creature;
        assert_eq!(follow, strong.then_some(friend), "seed {seed}");
    }
}

#[test]
fn a_followed_creature_is_followed_until_the_like_fades_or_it_dies() {
    let mut world = world();
    let green = green_out(&mut world, at(0, 1));
    let friend = mouse_out(&mut world, at(2, 1), 7);
    befriend(&mut world, green, 7, 1.0);
    world
        .creature_mut(green)
        .ai
        .as_mut()
        .expect("AI")
        .follow_creature = Some(friend);
    // `MoveWithCreature` -> `TryToGoToRoom(C 1)`: a mapped den node, so
    // `SetDestination` there (the path found from defined nodes: no draws); true.
    let mut rng = UnityRandom::new(2);
    let before = rng.state();
    assert!(world.migration_behavior(green, 4, &mut rng));
    assert_eq!(rng.state(), before);
    let ai = world.creature(green).ai.as_ref().expect("AI");
    assert_eq!(ai.destination, at(2, 1));
    assert_eq!(
        ai.path,
        vec![at(2, 1), at(2, 0), at(1, 1), at(1, 0), at(0, 0), at(0, 1)]
    );
    assert_eq!(ai.follow_creature, Some(friend));
    // `offscreenSpeedFac` is 1 while following.
    assert_eq!(world.offscreen_speed_fac(green), 1.0);

    // A like under 0.9 stops it after this move.
    befriend(&mut world, green, 7, 0.89);
    assert!(world.migration_behavior(green, 4, &mut rng));
    assert_eq!(
        world
            .creature(green)
            .ai
            .as_ref()
            .expect("AI")
            .follow_creature,
        None
    );

    // A dead one: the relationship is discarded and the roaming part follows, which
    // draws nothing while it still migrates to C (`MigrationDestination` elsewhere).
    world
        .creature_mut(green)
        .ai
        .as_mut()
        .expect("AI")
        .follow_creature = Some(friend);
    world.creature_mut(friend).state.alive = false;
    let before = rng.state();
    assert!(!world.migration_behavior(green, 4, &mut rng));
    assert_eq!(rng.state(), before);
    let g = world.creature(green);
    assert_eq!(g.ai.as_ref().expect("AI").follow_creature, None);
    let memory = g.state.social_memory.as_ref().expect("memory");
    assert!(memory.relationships.is_empty());
}
