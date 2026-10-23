//! The abstract AI subclasses on small test worlds. Expected values follow the game's
//! code (line numbers in `creature/abstract_ai/kinds_world.rs`); each test says which draws it expects.
//!
//! Every layer of a test room's node map gets the same costs, so every template sees
//! the same connectivity. The off-screen den is the last room (its nodes: Den,
//! RegionTransportation, SideExit, SkyExit, SeaExit, all 100 wide).

use std::sync::Arc;

use rw_assets::world::{AbstractRoomData, FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, ConnMap, CreatureType, NodeType, RoomSettings, SlugcatName, Timeline};
use rw_random::UnityRandom;

use crate::FloatExt;
use crate::creature::abstract_ai::kinds::MirosBirdAbstractAi;
use crate::creature::{
    AiKind, CreatureId, CreatureTemplate, EntityId, IdIssuer, PlayerGuideState, WorldCoordinate,
};
use crate::item::Entity;
use crate::room_script::StorySession;
use crate::tests::room;
use crate::world::{AbstractTick, World};

const LAYERS: usize = 15;
const EXIT: i32 = 0;
const DEN: i32 = 1;
const SIDE: i32 = 3;
const SKY: i32 = 4;

/// A node map line: `(type, tube length, entrance width)` per node, and `[cost,
/// length]` from each node to each, on every layer.
fn conn_map(nodes: &[(i32, i32, i32)], costs: &[&[[i32; 2]]]) -> ConnMap {
    let count = nodes.len();
    let mut line = format!("20|{count}|{LAYERS}|");
    for (i, (kind, tube, width)) in nodes.iter().enumerate() {
        line.push_str(&format!("{kind},{tube},0,0,{width},"));
        for _ in 0..LAYERS {
            for to in costs[i] {
                line.push_str(&format!("{} {},", to[0], to[1]));
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
    connections: Vec<i32>,
    map: ConnMap,
) -> AbstractRoomData {
    AbstractRoomData {
        name: name.to_owned(),
        index,
        connections,
        nodes: map.nodes,
        size: (40, 30),
        swarm_room_index: -1,
        shelter_index: -1,
        gate_index: -1,
        tags: Vec::new(),
        offscreen_den: false,
        ..template.clone()
    }
}

/// Two rooms and the off-screen den (2). Room 0 (`SH_E01`, a Miros room): node 0 Exit
/// to room 1, node 1 Den, node 2 a side exit 10 wide, node 3 a sky exit 10 wide.
/// Room 1 (`TT_B`): node 0 Exit to room 0, node 1 Den. Every pair of nodes of a room
/// is connected (cost 10, length 8). The rain is far off.
fn world() -> World {
    let base = WorldData::from_room(&room(&["#"]));
    let mut data = base.clone();
    let template = &base.rooms[0];
    let c = [10, 8];
    let n = [-1, -1];
    data.name = "TT".to_owned();
    data.single_room = false;
    data.swarm_rooms = Vec::new();
    data.rooms = vec![
        abstract_room(
            template,
            "SH_E01",
            0,
            vec![1, -1, -1, -1],
            conn_map(
                &[(EXIT, 4, 1), (DEN, 3, 1), (SIDE, 0, 10), (SKY, 0, 10)],
                &[&[n, c, c, c], &[c, n, c, c], &[c, c, n, c], &[c, c, c, n]],
            ),
        ),
        abstract_room(
            template,
            "TT_B",
            1,
            vec![0],
            conn_map(&[(EXIT, 5, 1), (DEN, 2, 1)], &[&[n, c], &[c, n]]),
        ),
        AbstractRoomData {
            index: 2,
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

fn at(room: i32, node: i32) -> WorldCoordinate {
    WorldCoordinate::new(room, -1, -1, node)
}

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

    /// A creature of `kind` made at `pos` with ID `(-1, number)` and put in its room
    /// (out of any den). Returns it and the constructor's draws' end state.
    fn spawn(
        &mut self,
        world: &mut World,
        kind: CreatureType,
        pos: WorldCoordinate,
        number: i32,
        rng: &mut UnityRandom,
    ) -> CreatureId {
        let template = CreatureTemplate::of(kind, ActiveMods::NONE);
        let id = EntityId::new(-1, number);
        let c = world.new_creature(template, pos, id, &self.session, &mut self.ids, rng);
        let cid = world.insert_creature(c);
        world.add_entity(cid);
        cid
    }
}

fn kind_of(world: &World, id: CreatureId) -> &AiKind {
    &world.creature(id).ai.as_ref().expect("AI").kind
}

#[test]
fn a_vulture_spends_620_ticks_on_each_check_room_without_drawing() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(3);
    let id = ticker.spawn(&mut world, CreatureType::Vulture, at(0, 3), 1, &mut rng);
    // The check rooms are set by hand; the destination is the first one.
    let first = at(0, 3);
    if let Some(ai) = world.creature_mut(id).ai.as_mut() {
        ai.destination = first;
        if let AiKind::Vulture(v) = &mut ai.kind {
            v.check_rooms = vec![first, at(2, 3)];
            v.time_in_room = 0;
        }
    }
    let start = rng.state();
    for _ in 0..7 {
        world.abstract_ai_update(id, 100, &mut ticker.tick(&mut rng));
    }
    // 700 > 620: the first check room is done, 80 ticks carried over; no draws.
    let AiKind::Vulture(v) = kind_of(&world, id) else {
        panic!("a vulture's AI");
    };
    assert_eq!(v.check_rooms, vec![at(2, 3)]);
    assert_eq!(v.time_in_room, 80);
    assert_eq!(rng.state(), start);
}

/// A big eel with nothing to go to looks for a sea exit: `value > 0.2` takes its own
/// room's cluster (room 0 and, for each unconnected exit, room 0 again, then room 1):
/// no sea exit there, no draw. Else a sea access node is drawn (`Range(0, 1)`: only
/// the off-screen den's) and its cluster is that room alone, whose sea exit is 100
/// wide: `Range(0, 1)` picks it. A path there is not found (the eel stands on an exit
/// it does not map), so no more draws.
#[test]
fn a_big_eel_looks_for_a_wide_sea_exit() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(8);
    let id = ticker.spawn(&mut world, CreatureType::BigEel, at(0, 0), 1, &mut rng);
    let mut model = rng.clone();
    world.abstract_ai_update(id, 55, &mut ticker.tick(&mut rng));
    let far = model.value() <= 0.2;
    if far {
        model.range_i32(0, 1);
        model.range_i32(0, 1);
    }
    assert_eq!(rng.state(), model.state());
    let AiKind::BigEel(e) = kind_of(&world, id) else {
        panic!("an eel's AI");
    };
    assert_eq!(e.go_to, far.then_some(at(2, 4)));
    assert_eq!(e.time_in_room, 55);
    assert_eq!(e.last_room, 0);
}

#[test]
fn a_deer_outside_lf_has_no_allowed_nodes_and_waits() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(4);
    let id = ticker.spawn(&mut world, CreatureType::Deer, at(0, 2), 1, &mut rng);
    let AiKind::Deer(d) = kind_of(&world, id) else {
        panic!("a deer's AI");
    };
    assert!(d.allowed_nodes.is_empty(), "the six LF rooms are not here");
    let start = rng.state();
    world.abstract_ai_update(id, 55, &mut ticker.tick(&mut rng));
    assert_eq!(rng.state(), start);
}

#[test]
fn miros_birds_take_their_allowed_side_exits_from_the_hard_coded_rooms() {
    let world = world();
    // `SH_E01` is in the list: its side exit (node 2) is 10 wide (>= 5).
    let m = MirosBirdAbstractAi::new(&world);
    assert_eq!(m.allowed_nodes, vec![at(0, 2)]);
}

#[test]
fn a_waiting_miros_bird_counts_down_without_drawing() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(4);
    let id = ticker.spawn(&mut world, CreatureType::MirosBird, at(2, 0), 1, &mut rng);
    if let Some(ai) = world.creature_mut(id).ai.as_mut()
        && let AiKind::MirosBird(m) = &mut ai.kind
    {
        m.wait = 100;
    }
    let start = rng.state();
    world.abstract_ai_update(id, 55, &mut ticker.tick(&mut rng));
    let AiKind::MirosBird(m) = kind_of(&world, id) else {
        panic!("a Miros bird's AI");
    };
    assert_eq!(m.wait, 45);
    assert_eq!(rng.state(), start);
}

/// Three Miros birds in the off-screen den: the most dominant one raids. Draws:
/// `Range(0, allowed)` (one allowed node: 0), `Range(2, 5)`, then for each ready bird
/// in list order while the count lasts `Range(0, Range(0, 50))`. Every one gets the path
/// den, the target's other wide side exit (none here: the sky exit is not a side exit),
/// the target.
#[test]
fn the_most_dominant_miros_bird_leads_a_raid() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(11);
    let birds: Vec<CreatureId> = (1..=3)
        .map(|n| ticker.spawn(&mut world, CreatureType::MirosBird, at(2, 0), n, &mut rng))
        .collect();
    let leader = *birds
        .iter()
        .max_by(|&&a, &&b| {
            let da = world.creature(a).personality.dominance;
            let db = world.creature(b).personality.dominance;
            da.total_cmp(&db)
        })
        .expect("birds");
    let mut model = rng.clone();
    world.abstract_ai_update(leader, 55, &mut ticker.tick(&mut rng));
    assert_eq!(model.range_i32(0, 1), 0);
    let mut count = model.range_i32(2, 5);
    let mut waits = Vec::new();
    for _ in &birds {
        if count <= 0 {
            waits.push(0);
            continue;
        }
        let inner = model.range_i32(0, 50);
        waits.push(model.range_i32(0, inner));
        count -= 1;
    }
    assert_eq!(rng.state(), model.state());
    for (k, &b) in birds.iter().enumerate() {
        let c = world.creature(b);
        let ai = c.ai.as_ref().expect("AI");
        assert_eq!(ai.path, vec![at(2, 0), at(0, 2)], "bird {k}'s path");
        let AiKind::MirosBird(m) = &ai.kind else {
            panic!("a Miros bird's AI");
        };
        assert_eq!(m.wait, waits[k], "bird {k}'s wait");
    }
}

/// A big needle worm with `migrateActive` false in a room it likes only flips the flag
/// when its counter (0 at first) runs out: one `Range(1200, 3000)`.
#[test]
fn a_big_needle_worm_flips_migrate_active_on_its_counter() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(21);
    let id = ticker.spawn(
        &mut world,
        CreatureType::BigNeedleWorm,
        at(1, 1),
        1,
        &mut rng,
    );
    if let Some(ai) = world.creature_mut(id).ai.as_mut()
        && let AiKind::NeedleWorm(w) = &mut ai.kind
    {
        w.migrate_active = false;
    }
    let mut model = rng.clone();
    world.abstract_ai_update(id, 55, &mut ticker.tick(&mut rng));
    let counter = model.range_i32(1200, 3000);
    assert_eq!(rng.state(), model.state());
    let AiKind::NeedleWorm(w) = kind_of(&world, id) else {
        panic!("a needle worm's AI");
    };
    assert!(w.migrate_active);
    assert_eq!(w.migrate_active_counter, counter);
}

/// A small needle worm with a mother in the next room heads there
/// (`MoveWithCreature` -> `TryToGoToRoom`: the mother's node, an exit, is mapped).
#[test]
fn a_small_needle_worm_follows_its_mother() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(2);
    let mother = ticker.spawn(
        &mut world,
        CreatureType::BigNeedleWorm,
        at(1, 0),
        1,
        &mut rng,
    );
    let small = ticker.spawn(
        &mut world,
        CreatureType::SmallNeedleWorm,
        at(0, 1),
        2,
        &mut rng,
    );
    if let Some(ai) = world.creature_mut(small).ai.as_mut()
        && let AiKind::NeedleWorm(w) = &mut ai.kind
    {
        w.mother = Some(mother);
        w.migrate_active_counter = 10_000;
    }
    world.abstract_ai_update(small, 55, &mut ticker.tick(&mut rng));
    let ai = world.creature(small).ai.as_ref().expect("AI");
    assert_eq!(ai.follow_creature, Some(mother));
    assert_eq!(ai.destination, at(1, 0));
}

/// A cicada's catch: a seed where both rolls pass (`value < 0.1`, `value > 1/55`), then
/// `Range(0, 4)` picks a node holding flies and `value < 1/count` passes. The new fly
/// (`GetNewID`) is at the cicada's position, carried, the count one lower; the cicada
/// heads for its den.
#[test]
fn a_cicada_catches_a_counted_fly_and_carries_it_home() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(1);
    let cicada = ticker.spawn(&mut world, CreatureType::CicadaA, at(0, 1), 1, &mut rng);
    if let Some(ai) = world.creature_mut(cicada).ai.as_mut() {
        ai.den_position = Some(at(0, 1));
    }
    for node in 0..4 {
        world.add_quantified_at(0, node, CreatureType::Fly);
    }
    let mut seed = 0;
    let caught_node = loop {
        seed += 1;
        let mut m = UnityRandom::new(seed);
        if m.value() < 0.1 && m.value() > 1.0 / 55.0 {
            let node = m.range_i32(0, 4);
            if m.value() < 1.0 {
                break node;
            }
        }
    };
    let mut rng = UnityRandom::new(seed);
    let next_id = ticker.ids.next_issued_id;
    world.abstract_ai_update(cicada, 55, &mut ticker.tick(&mut rng));
    assert_eq!(
        world.number_of_quantified_creature_in_node(0, CreatureType::Fly, caught_node),
        0
    );
    let fly = world
        .creatures()
        .find(|(_, c)| c.kind() == CreatureType::Fly)
        .map(|(id, _)| id)
        .expect("a caught fly");
    let f = world.creature(fly);
    assert_eq!(f.id, EntityId::new(-1, next_id + 1));
    assert_eq!(f.pos, at(0, 1));
    assert!(f.pacified_because_carried(fly));
    assert!(world.have_prey(cicada));
    assert_eq!(world.creature(cicada).sticks[0].b, Entity::Creature(fly));
}

/// A carried fly follows its cicada's `Move`; into an unrealized room it `Quantify`s:
/// it leaves the room's lists, is destroyed and counted at the new node.
#[test]
fn a_carried_fly_turns_back_into_a_count_when_its_carrier_moves() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(1);
    let cicada = ticker.spawn(&mut world, CreatureType::CicadaB, at(0, 1), 1, &mut rng);
    let fly = ticker.spawn(&mut world, CreatureType::Fly, at(0, 1), 2, &mut rng);
    world.add_grip_stick(cicada, Entity::Creature(fly), 0, true);
    world.move_creature(cicada, at(1, 0));
    assert_eq!(world.creature(cicada).pos.room, 1);
    assert!(world.creature(fly).slated_for_deletion);
    assert!(world.creature(cicada).sticks.is_empty());
    assert!(!world.room_creatures(0).contains(&fly));
    assert_eq!(
        world.number_of_quantified_creature_in_node(1, CreatureType::Fly, 0),
        1
    );
}

/// `InDenUpdate` runs for every creature but the scavengers (not ported): a vulture grub
/// (no AI) comes out when its counter runs out; a scavenger stays in.
#[test]
fn only_scavengers_are_held_in_their_dens() {
    let mut world = world();
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(6);
    let grub = ticker.spawn(&mut world, CreatureType::VultureGrub, at(1, 1), 1, &mut rng);
    let scav = ticker.spawn(&mut world, CreatureType::Scavenger, at(1, 1), 2, &mut rng);
    for id in [grub, scav] {
        world.creature_mut(id).remain_in_den_counter = 10;
        world.move_entity_to_den(id, &mut rng);
    }
    world.update_abstract_room(1, 55, &mut ticker.tick(&mut rng));
    assert!(!world.creature(grub).in_den);
    assert!(world.creature(scav).in_den);
}

#[test]
fn csharp_int_casts_give_int_min_for_nan_and_infinity() {
    assert_eq!(f32::NAN.to_int_cs(), i32::MIN);
    assert_eq!(f32::INFINITY.to_int_cs(), i32::MIN);
    assert_eq!((-2.7_f32).to_int_cs(), -2);
    assert_eq!(1e10_f32.to_int_cs(), i32::MIN);
}

/// `Move` first puts a garbage worm on its new room's garbage-hole node; moving onto
/// that node then finds it already there and returns (`timeSpentHere` is kept).
#[test]
fn a_garbage_worm_moving_takes_the_garbage_hole_node() {
    let mut world = world();
    world.data.rooms[1].nodes[1].kind = NodeType::GarbageHoles;
    let mut ticker = Ticker::new();
    let mut rng = UnityRandom::new(6);
    let worm = ticker.spawn(&mut world, CreatureType::GarbageWorm, at(1, 0), 1, &mut rng);
    world.creature_mut(worm).pos = at(1, 0);
    world.creature_mut(worm).time_spent_here = 77;
    world.move_creature(worm, at(1, 1));
    let c = world.creature(worm);
    assert_eq!(c.pos, at(1, 1));
    assert_eq!(c.time_spent_here, 77);
}
