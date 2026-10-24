//! `Room.Loaded`'s `RoomRain` and its turn: which rooms get one, the intensity, and
//! `ThrowAroundObjects` over a room's player, lizard and rock (draw order and count, the
//! velocities, the stun and kill branches, `rainDeath` on the main chunk only). Expected
//! values from `Room.cs` (`Loaded`) and `RoomRain.cs` (`Update`, `ThrowAroundObjects`),
//! replayed by hand on a second generator.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::{AbstractObjectType, DangerType, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::creature::lizard::perception_turn::tests::game_seeded;
use crate::item::Entity;
use crate::item::ObjectId;
use crate::world::RoomRain;
use crate::{ChunkOwner, FloatExt, RainWorldGame, Vec2};

/// A 30 x 6 room open at the top: every interior column's rain reach is its floor (0).
const OPEN: [&str; 6] = [
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##############################",
];

fn open_game() -> RainWorldGame {
    game_seeded(&OPEN, (3, 1), 5)
}

fn rain(g: &RainWorldGame) -> &RoomRain {
    g.world().realized(0).unwrap().room_rain.as_ref().unwrap()
}

fn rock(g: &mut RainWorldGame) -> ObjectId {
    let id = g
        .dev_spawn_object(AbstractObjectType::Rock, 0, (10, 1), false)
        .expect("spawns");
    for _ in 0..3 {
        g.tick(crate::Input::NONE, &mut ());
    }
    id
}

/// `Room.Loaded` makes a `RoomRain` for `Rain`, `FloodAndRain`, `Flood` and
/// `AerieBlizzard` rooms and none for `None` or `Thunder`; it joins the update list.
#[test]
fn only_the_rain_danger_types_make_a_room_rain() {
    for (danger, made) in [
        (DangerType::Rain, true),
        (DangerType::FloodAndRain, true),
        (DangerType::Flood, true),
        (DangerType::AerieBlizzard, true),
        (DangerType::None, false),
        (DangerType::Thunder, false),
    ] {
        let mut g = open_game();
        g.world_mut().realized_mut(0).unwrap().room_rain = None;
        g.world_mut().realized_mut(0).unwrap().settings = RoomSettings {
            danger_type: Some(danger),
            ..RoomSettings::ANCESTOR
        };
        g.add_room_rain(0);
        let has = g.world().realized(0).unwrap().room_rain.is_some();
        assert_eq!(has, made, "{danger:?}");
        assert_eq!(g.room_rain_objects(0).len(), usize::from(made));
    }
}

/// The ancestor settings' `Rain` room has one from the start, with `rainReach` 0 on the
/// open interior and 5 (the top row) on the two walls.
#[test]
fn a_rain_room_has_its_rain_from_load() {
    let g = open_game();
    let r = rain(&g);
    assert_eq!(r.danger_type, DangerType::Rain);
    assert_eq!(r.reach_at(0), 5);
    assert_eq!(r.reach_at(29), 5);
    assert!((1..29).all(|x| r.reach_at(x) == 0));
    assert_eq!(r.splash_tiles.len(), 28);
}

/// `RoomRain.Update` follows `globalRain.Intensity` at 0.2 a tick and caps at the room's
/// `RainIntensity`.
#[test]
fn the_turn_updates_the_intensity() {
    let mut g = open_game();
    let order = rain(&g).update_order;
    g.world_mut().rain_cycle.global_rain.intensity = 0.2;
    g.run_room_rain(0, order);
    assert!((rain(&g).intensity - 0.04).abs() < 1e-6);
    g.world_mut()
        .realized_mut(0)
        .unwrap()
        .settings
        .rain_intensity = 0.01;
    g.run_room_rain(0, order);
    assert_eq!(rain(&g).intensity, 0.01);
}

/// What `ThrowAroundObjects` reads of a chunk before pushing it.
#[derive(Clone)]
struct Probe {
    pos: Vec2,
    rad: f32,
    mass: f32,
    vel: Vec2,
    on_ground: bool,
    creature: bool,
    main: bool,
    owner_chunks: usize,
}

/// What a chunk ends with in the replay.
struct Replayed {
    vel: Vec2,
    rain_death: f32,
}

impl RainWorldGame {
    /// Room 0's chunks in `physicalObjects` order with the owner of each.
    fn probes(&self) -> Vec<(ChunkOwner, Probe)> {
        let mut out = Vec::new();
        for (_, _, owner) in self.world().physical_objects(0, Some(self.player_seq)) {
            let creature = self.rain_creature_probe(owner);
            let n = (0..).map_while(|i| self.electric_chunk(owner, i)).count();
            for i in 0..n {
                let c = self.electric_chunk(owner, i).unwrap();
                out.push((
                    owner,
                    Probe {
                        pos: c.pos,
                        rad: c.rad,
                        mass: c.mass,
                        vel: c.vel,
                        on_ground: c.contact().y < 0,
                        creature,
                        main: i == 0,
                        owner_chunks: n,
                    },
                ));
            }
        }
        out
    }

    fn rain_creature_probe(&self, owner: ChunkOwner) -> bool {
        match owner {
            ChunkOwner::Player => true,
            ChunkOwner::Entity(e) => matches!(e, Entity::Creature(_)),
            ChunkOwner::Segment(_) => false,
        }
    }
}

trait Reach {
    /// The first solid row from the top of column `x` (clamped), 0 if none.
    fn reach(&self, x: i32) -> i32;
}

impl Reach for Room {
    fn reach(&self, x: i32) -> i32 {
        let x = x.clamp(0, self.width() - 1);
        (0..self.height())
            .rev()
            .find(|&y| self.tile(x, y).is_solid())
            .unwrap_or(0)
    }
}

/// `ThrowAroundObjects` on `chunks` in an `OPEN` room at `outside`/`inside` push, replayed
/// from the decompile on `e`. `None` when a stun or kill roll would fire.
fn replay(
    e: &mut UnityRandom,
    room: &Room,
    push: (f32, f32),
    chunks: &[Probe],
) -> Option<Vec<Replayed>> {
    let (inside, outside) = push;
    let mut out = Vec::new();
    for c in chunks {
        let x = (-c.rad).lerp(c.rad, e.value());
        let y = (-c.rad).lerp(c.rad, e.value());
        let tile = (c.pos + Vec2::new(x, y)).tile();
        let mut num = inside;
        let mut flag = false;
        if room.reach(tile.0) < tile.1 {
            flag = true;
            num = outside.max(inside);
        }
        let mut vel = c.vel;
        let mut rain_death = 0.0;
        if num > 0.0 {
            if c.on_ground {
                let mut side = 0;
                if room.reach(tile.0 - 1) >= tile.1 && !room.tile(tile.0 - 1, tile.1).is_solid() {
                    side -= 1;
                }
                if room.reach(tile.0 + 1) >= tile.1 && !room.tile(tile.0 + 1, tile.1).is_solid() {
                    side += 1;
                }
                let angle = (-30.0_f32).lerp(30.0, e.value()) + (side * 16) as f32;
                let size = e.value() * if flag { 9.0 } else { 4.0 } * num;
                vel += Vec2::from_degrees(angle) * size / c.mass;
            } else {
                vel.y -= e.value().mathf_pow(5.0) * 16.5 * num / c.mass;
            }
            if c.creature {
                if e.value().mathf_pow(1.2) * 2.0 * (c.owner_chunks as f32) < num {
                    return None;
                }
                if c.main {
                    rain_death += num / 20.0;
                }
                if num > 0.5 && rain_death > 1.0 {
                    return None;
                }
            }
            let dir = Vec2::from_degrees(90.0_f32.lerp(270.0, e.value()));
            vel += dir * (e.value() * 5.0 * inside);
        }
        out.push(Replayed { vel, rain_death });
    }
    Some(out)
}

/// The first seed for which `replay` finds no stun and no kill, and its result.
fn quiet_seed(probes: &[Probe], push: (f32, f32)) -> (i32, Vec<Replayed>) {
    let room = crate::tests::room(&OPEN);
    (0..500)
        .find_map(|s| replay(&mut UnityRandom::new(s), &room, push, probes).map(|r| (s, r)))
        .expect("a seed without a stun")
}

/// A player, a green lizard and a rock in an open room at `Intensity` 0.9: every chunk
/// gets the draws and the velocity `ThrowAroundObjects` gives it, in `physicalObjects`
/// order, and the main chunks of the two creatures add `num / 20` to `rainDeath`.
#[test]
fn every_chunk_of_the_room_is_pushed_in_list_order() {
    let mut g = open_game();
    let lizard = g.still_lizard((20, 1), 1.0, 1);
    rock(&mut g);
    g.world_mut().rain_cycle.global_rain.intensity = 0.9;
    let push = {
        let gr = &g.world().rain_cycle.global_rain;
        (gr.inside_push_around(), gr.outside_push_around())
    };
    let probes = g.probes();
    assert!(probes.len() >= 6, "{} chunks", probes.len());
    assert!(probes.iter().any(|(_, p)| !p.creature), "the rock");
    let plain: Vec<Probe> = probes.iter().map(|(_, p)| p.clone()).collect();
    let (seed, want) = quiet_seed(&plain, push);
    let order = rain(&g).update_order;
    g.rng = UnityRandom::new(seed);
    g.run_room_rain(0, order);

    for (i, (owner, _)) in probes.iter().enumerate() {
        let got = g.electric_chunk(
            *owner,
            probes.iter().take(i).filter(|(o, _)| o == owner).count(),
        );
        assert_eq!(got.unwrap().vel, want[i].vel, "chunk {i}");
    }
    let mut e = UnityRandom::new(seed);
    let room = crate::tests::room(&OPEN);
    replay(&mut e, &room, push, &plain).unwrap();
    assert_eq!(
        g.rng.value(),
        e.value(),
        "the stream is where the replay left it"
    );

    let player_death: f32 = want
        .iter()
        .zip(&probes)
        .filter(|(_, (o, _))| *o == ChunkOwner::Player)
        .map(|(w, _)| w.rain_death)
        .sum();
    assert_eq!(g.rain_death(g.player_id), player_death);
    assert!(player_death > 0.0);
    assert_eq!(g.rain_death(lizard), {
        let lizard_owner = ChunkOwner::Entity(Entity::Creature(lizard));
        want.iter()
            .zip(&probes)
            .filter(|(_, (o, _))| *o == lizard_owner)
            .map(|(w, _)| w.rain_death)
            .sum::<f32>()
    });
}

/// With `RainIntensity` 0 the room does not push: no draws.
#[test]
fn a_room_without_rain_intensity_pushes_nothing() {
    let mut g = open_game();
    g.world_mut().rain_cycle.global_rain.intensity = 0.9;
    g.world_mut()
        .realized_mut(0)
        .unwrap()
        .settings
        .rain_intensity = 0.0;
    let order = rain(&g).update_order;
    g.rng = UnityRandom::new(3);
    g.run_room_rain(0, order);
    assert_eq!(g.rng.value(), UnityRandom::new(3).value());
}

/// Below `AnyPushAround` (`Intensity` 0.35) the turn only follows the intensity.
#[test]
fn a_calm_rain_draws_nothing() {
    let mut g = open_game();
    g.world_mut().rain_cycle.global_rain.intensity = 0.3;
    let order = rain(&g).update_order;
    g.rng = UnityRandom::new(3);
    g.run_room_rain(0, order);
    assert_eq!(g.rng.value(), UnityRandom::new(3).value());
}

/// A stun roll that fires stuns the player for `Range(1, 1 + (int)(9 * num))` ticks.
#[test]
fn a_creature_chunk_can_be_stunned() {
    let mut g = open_game();
    g.world_mut().rain_cycle.global_rain.intensity = 0.9;
    let probes = g.probes();
    let head = probes[0].1.clone();
    assert!(head.creature && head.main);
    let gr = g.world().rain_cycle.global_rain;
    let num = gr.outside_push_around().max(gr.inside_push_around());
    // The head's first chunk: 2 jitter draws, 1 or 2 push draws, then the stun roll.
    let pushes = if head.on_ground { 2 } else { 1 };
    let seed = (0..2000)
        .find(|&s| {
            let mut e = UnityRandom::new(s);
            for _ in 0..2 + pushes {
                e.value();
            }
            e.value().mathf_pow(1.2) * 2.0 * 2.0 < num
        })
        .expect("a stunning seed");
    let mut e = UnityRandom::new(seed);
    for _ in 0..2 + pushes + 1 {
        e.value();
    }
    let ticks = e.range_i32(1, 1 + (9.0 * num) as i32);
    let order = rain(&g).update_order;
    g.rng = UnityRandom::new(seed);
    g.run_room_rain(0, order);
    // The hips' own roll may stun it longer.
    assert!(g.player().unwrap().stun_ticks() >= ticks);
}

/// With `rainDeath` over 1 after the main chunk's share and `num > 0.5`, a draw under 0.025
/// kills; the kill roll's draw is the one after the stun roll.
#[test]
fn rain_death_over_one_can_kill() {
    let mut g = open_game();
    g.world_mut().rain_cycle.global_rain.intensity = 0.9;
    g.player.as_mut().unwrap().rain_death = 1.0;
    let head = g.probes()[0].1.clone();
    let gr = g.world().rain_cycle.global_rain;
    let num = gr.outside_push_around().max(gr.inside_push_around());
    assert!(num > 0.5);
    let pushes = if head.on_ground { 2 } else { 1 };
    let seed = (0..200_000)
        .find(|&s| {
            let mut e = UnityRandom::new(s);
            for _ in 0..2 + pushes {
                e.value();
            }
            let stun = e.value().mathf_pow(1.2) * 2.0 * 2.0 < num;
            !stun && e.value() < 0.025
        })
        .expect("a killing seed");
    let order = rain(&g).update_order;
    g.rng = UnityRandom::new(seed);
    g.run_room_rain(0, order);
    assert!(g.player().unwrap().is_dead());
    assert!(g.rain_death(g.player_id) > 1.0);
}

/// From `rainDeath` 0.9 the main chunk's 0.05 stays under 1: no kill roll, whatever the seed.
#[test]
fn rain_death_at_or_under_one_does_not_kill() {
    let mut g = open_game();
    g.world_mut().rain_cycle.global_rain.intensity = 0.9;
    g.player.as_mut().unwrap().rain_death = 0.9;
    let order = rain(&g).update_order;
    for seed in 0..300 {
        g.rng = UnityRandom::new(seed);
        g.run_room_rain(0, order);
        assert!(!g.player().unwrap().is_dead(), "seed {seed}");
        g.player.as_mut().unwrap().rain_death = 0.9;
    }
}

/// `Creature.TerrainImpact` of a lizard or fly: a first floor contact in a pushing rain
/// makes `CreatureSmashedInGround`'s three draws; a slow hit, a ceiling or a repeated contact
/// make none. A speed is positive, so `InverseLerp(-2.5, -15, speed)` is 0 and `rainDeath`
/// is unchanged.
#[test]
fn a_lizards_floor_impact_makes_the_smash_draws() {
    let mut g = open_game();
    let lizard = g.still_lizard((20, 1), 1.0, 1);
    g.world_mut().rain_cycle.global_rain.intensity = 0.9;
    let chunks = [(Vec2::new(410.0, 30.0), 6.0), (Vec2::new(390.0, 30.0), 6.0)];
    for (i, draws) in [
        (crate::TerrainImpact::floor(0, 5.0, true, -1), 3),
        (crate::TerrainImpact::floor(0, 2.4, true, -1), 0),
        (crate::TerrainImpact::floor(0, 5.0, true, 1), 0),
        (crate::TerrainImpact::floor(0, 5.0, false, -1), 0),
    ] {
        let mut rng = UnityRandom::new(30);
        g.world_mut()
            .creature_smashed_in_ground(0, lizard, &i, &chunks, &mut rng);
        let mut e = UnityRandom::new(30);
        for _ in 0..draws {
            e.value();
        }
        assert_eq!(rng.value(), e.value(), "{i:?}");
        assert_eq!(g.rain_death(lizard), 0.0);
    }
}
