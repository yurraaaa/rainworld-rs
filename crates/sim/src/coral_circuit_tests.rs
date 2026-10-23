//! `CoralCircuit`, `CircuitBit`, `CircuitConnector` and `CircuitCrab` against
//! `CoralCircuit.cs`, `CircuitConnector.cs`, `CircuitCrab.cs` and `CrabLeg.cs`: the seeded
//! constructor (tiles, bits, neighbours, connectors, crabs and the draws between them), a
//! bit's turn (in place, displaced and alone, breaking a link, snapping home), the push on a
//! chunk and the circuit's explosion, the connectors' and crabs' turns with their draws.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};
use rw_random::UnityRandom;

use super::{BitCx, CircuitCx, CircuitPlace, CoralCircuit, PushChunk};
use crate::coral_circuit::CircuitBit;
use crate::mycelium::{MyceliumEnv, MyceliumPool, SystemId, SystemState};
use crate::tests::room;
use crate::world::AiMap;
use crate::{CameraView, FloatExt, SoundEvent, SoundId, Sounds, Vec2};

/// A 60 x 40 tile room with a solid border.
fn big_room() -> rw_assets::Room {
    let mut rows = vec!["#".repeat(60)];
    rows.extend((0..38).map(|_| format!("#{}#", ".".repeat(58))));
    rows.push("#".repeat(60));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// The installed `sounds.txt` lines of the circuit's sounds.
fn circuit_sounds() -> Sounds {
    let text = "Volume: 6\nVolume exponent: 1.8\n\
                Coral_Circuit_Break : CoralBreakA/vol=0.3\n\
                Coral_Circuit_Jump_Explosion : CoralSwimA/vol=0.4\n";
    let names = ClipNames::new(["CoralBreakA".to_owned(), "CoralSwimA".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()))
}

fn camera() -> CameraView {
    CameraView {
        room: 0,
        current: 0,
        next: 0,
    }
}

struct Setup {
    room: rw_assets::Room,
    map: AiMap,
    pool: MyceliumPool,
    seq: u64,
}

impl Setup {
    fn new() -> Self {
        let room = big_room();
        let map = AiMap::new(&room, 0, false, false);
        let mut pool = MyceliumPool::default();
        pool.create_registry();
        Self {
            room,
            map,
            pool,
            seq: 10,
        }
    }

    fn circuit(&mut self, places: Vec<CircuitPlace>, seed: i32) -> CoralCircuit {
        let mut rng = UnityRandom::new(seed);
        CoralCircuit::build(
            places,
            &mut CircuitCx {
                room: &self.room,
                ai_map: &self.map,
                system: SystemId(1),
                pool: &mut self.pool,
                seq: &mut self.seq,
            },
            &mut rng,
        )
    }
}

const PLACE: CircuitPlace = CircuitPlace {
    pos: Vec2::new(610.0, 410.0),
    rad: 60.0,
};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn near_vec(a: Vec2, b: Vec2) -> bool {
    near(a.x, b.x) && near(a.y, b.y)
}

/// The tiles are the free ones within the radius whose terrain proximity is over 2, in the
/// loop's order; the box they span has the grid's size (2 x 2 with none).
#[test]
fn the_tiles_are_the_free_ones_away_from_walls() {
    let mut s = Setup::new();
    let c = s.circuit(vec![PLACE], 7);
    let mut expect = Vec::new();
    for j in 30 - 3 - 1..=30 + 3 + 1 {
        for k in 20 - 3 - 1..=20 + 3 + 1 {
            let middle = Vec2::middle_of_tile((j, k));
            if middle.distance(PLACE.pos) < 60.0 && s.map.terrain_proximity(j, k) > 2 {
                expect.push((j, k));
            }
        }
    }
    assert!(expect.len() > 20);
    assert_eq!(c.tiles, expect);
    let (xs, ys): (Vec<i32>, Vec<i32>) = expect.iter().copied().unzip();
    let low = (*xs.iter().min().unwrap(), *ys.iter().min().unwrap());
    let high = (*xs.iter().max().unwrap(), *ys.iter().max().unwrap());
    assert_eq!((c.bottom_left, c.top_right), (low, high));
    assert_eq!(c.dims, (high.0 - low.0 + 1, high.1 - low.1 + 1));
    let far = s.circuit(
        vec![CircuitPlace {
            pos: Vec2::new(10.0, 10.0),
            rad: 15.0,
        }],
        7,
    );
    assert!(far.tiles.is_empty() && far.bits.is_empty() && far.connectors.is_empty());
    assert_eq!(far.dims, (2, 2));
    assert!(far.crabs.is_empty());
}

/// The constructor is seeded by `(int)x + (int)y` of the first place and puts the shared
/// stream back; replaying the game's draws in its order gives the same rotation, stuck
/// positions and sizes of every bit (two where a tile is over half way from a place's
/// edge, one near the edge), the same connector places and strand counts, the same crab
/// places, leg counts and leg points.
#[test]
fn the_constructor_draws_in_the_games_order() {
    let mut s = Setup::new();
    let mut shared = UnityRandom::new(5);
    let before = shared.state();
    let c = CoralCircuit::build(
        vec![PLACE],
        &mut CircuitCx {
            room: &s.room,
            ai_map: &s.map,
            system: SystemId(1),
            pool: &mut s.pool,
            seq: &mut s.seq,
        },
        &mut shared,
    );
    assert_eq!(shared.state(), before);
    let mut e = UnityRandom::new(0);
    e.init_state(610 + 410);
    assert!(near_vec(c.rotation, Vec2::random_unit(&mut e)));
    let mut at = 0;
    for &tile in &c.tiles {
        let near_edge = Vec2::middle_of_tile(tile)
            .distance(PLACE.pos)
            .inverse_lerp(60.0, 60.0 - 220.0);
        let count = if f64::from(near_edge) < 0.5 { 2 } else { 1 };
        for _ in 0..count {
            let bit = &c.bits[at];
            at += 1;
            assert_eq!(bit.tile, tile);
            let x = (-9.0f32).lerp(9.0, e.value());
            let y = (-9.0f32).lerp(9.0, e.value());
            let stuck = Vec2::middle_of_tile(tile) + Vec2::new(x, y);
            assert!(near_vec(bit.stuck_pos, stuck), "{tile:?}");
            let from_edge = stuck.distance(PLACE.pos).inverse_lerp(60.0, 60.0 - 220.0);
            let (lo, hi) = if from_edge > 0.5 {
                (15.0, 35.0)
            } else {
                (5.0, 25.0)
            };
            let w = lo.lerp(hi, e.value()).floor();
            let h = lo.lerp(hi, e.value()).floor();
            assert_eq!(bit.size, Vec2::new(w, h), "{tile:?}");
        }
    }
    assert_eq!(at, c.bits.len());
    let tiles = c.tiles.len() as i32;
    let mut expect_connectors = 0;
    for _ in 0..(tiles / 15).clamp(1, 40) {
        let tile = c.tiles[e.range_i32(0, tiles) as usize];
        let layer = e.range_i32(0, 2);
        let Some(bit) = c.bit_at(tile.0, tile.1, layer) else {
            continue;
        };
        let size = c.bits[bit].size;
        let cx = e.value() * size.x;
        let cy = e.value() * size.y;
        let strands = e.range_i32(5, 10) as usize;
        let connector = &c.connectors[expect_connectors];
        expect_connectors += 1;
        assert_eq!(connector.bit, bit);
        assert!(near_vec(connector.c_pos, Vec2::new(cx, cy) - size * 0.5));
        assert_eq!(connector.connections.len(), strands);
        for _ in 0..strands * 6 {
            e.value();
        }
    }
    assert_eq!(c.connectors.len(), expect_connectors);
    let mut expect_crabs = 0;
    for _ in 0..(tiles / 30).clamp(1, 20) {
        let tile = c.tiles[e.range_i32(0, tiles) as usize];
        let layer = e.range_i32(0, 2);
        let Some(bit) = c.bit_at(tile.0, tile.1, layer) else {
            continue;
        };
        let size = c.bits[bit].size;
        let cx = e.value() * size.x;
        let cy = e.value() * size.y;
        let legs = e.range_i32(4, 8) as usize;
        let crab = &c.crabs[expect_crabs];
        expect_crabs += 1;
        assert!(near_vec(crab.c_pos, Vec2::new(cx, cy) - size * 0.5));
        assert_eq!(crab.legs.len(), legs);
        let init = c.bits[bit].on_circuit_pos(crab.c_pos, 1.0);
        for leg in &crab.legs {
            assert_eq!(leg.points.len(), 3);
            assert!(near(leg.con_rad, 10.0));
            for p in &leg.points {
                let pos = init + Vec2::random_unit(&mut e);
                assert!(near_vec(p[0], pos) && near_vec(p[1], pos));
                assert!(near_vec(p[2], Vec2::random_unit(&mut e)));
            }
        }
    }
    assert_eq!(c.crabs.len(), expect_crabs);
    assert!(expect_connectors > 0 && expect_crabs > 0);
}

/// A bit's neighbours are the other bits of its own and the four next tiles whose boxes
/// overlap its box, as `Connect` lists them (tile offsets (0,0), (-1,0), (0,-1), (1,0),
/// (0,1), each over the layers).
#[test]
fn neighbours_are_the_overlapping_bits_of_the_five_tiles() {
    let mut s = Setup::new();
    let c = s.circuit(vec![PLACE], 7);
    for (i, bit) in c.bits.iter().enumerate() {
        let mut expect = Vec::new();
        for (dx, dy) in [(0, 0), (-1, 0), (0, -1), (1, 0), (0, 1)] {
            for layer in 0..2 {
                if let Some(j) = c.bit_at(bit.tile.0 + dx, bit.tile.1 + dy, layer) {
                    let other: &CircuitBit = &c.bits[j];
                    let overlap = bit.stuck_pos.x - bit.size.x < other.stuck_pos.x + other.size.x
                        && bit.stuck_pos.x + bit.size.x > other.stuck_pos.x - other.size.x
                        && bit.stuck_pos.y - bit.size.y < other.stuck_pos.y + other.size.y
                        && bit.stuck_pos.y + bit.size.y > other.stuck_pos.y - other.size.y;
                    if j != i && overlap {
                        expect.push(j);
                    }
                }
            }
        }
        assert_eq!(bit.neighbors, expect, "bit {i}");
        assert_eq!(bit.active_connections, vec![true; expect.len()]);
        for (k, &j) in expect.iter().enumerate() {
            assert!(near(
                bit.neighbor_dists[k],
                bit.stuck_pos.distance(c.bits[j].stuck_pos)
            ));
            assert!(near(
                bit.neighbor_dirs[k],
                bit.stuck_pos.to_aim(c.bits[j].stuck_pos)
            ));
        }
    }
}

/// The bits, connectors and crabs take the next places of the update list in that order,
/// the circuit last.
#[test]
fn the_update_list_runs_bits_then_connectors_and_crabs_then_the_circuit() {
    let mut s = Setup::new();
    let c = s.circuit(vec![PLACE], 7);
    let orders: Vec<u64> = c
        .bits
        .iter()
        .map(|b| b.update_order)
        .chain(c.connectors.iter().map(|x| x.update_order))
        .chain(c.crabs.iter().map(|x| x.update_order))
        .chain([c.update_order])
        .collect();
    let expect: Vec<u64> = (11..11 + orders.len() as u64).collect();
    assert_eq!(orders, expect);
    assert_eq!(s.seq, 10 + orders.len() as u64);
}

/// A circuit of one bit alone, set 30 px off its place: its turn (no neighbours) makes it
/// lonely, with the two spin draws `Lerp(-1, 1, value) * Clamp(speed, 1, 5) * 0.02` and
/// `* 0.04`, counts it displaced and starts counting its repairs.
#[test]
fn a_displaced_bit_alone_turns_lonely_and_spins() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let (mut rng, mut sounds) = (UnityRandom::new(3), Sounds::default());
    let mut mirror = rng.clone();
    let bit = &mut c.bits[0];
    bit.neighbors.clear();
    bit.active_connections.clear();
    bit.neighbor_dists.clear();
    bit.neighbor_dirs.clear();
    bit.pos = bit.stuck_pos + Vec2::new(30.0, 0.0);
    bit.last_pos = bit.pos;
    c.update_bit(
        0,
        &mut BitCx {
            room: &s.room,
            ai_map: &s.map,
            sounds: &mut sounds,
            rng: &mut rng,
        },
    );
    let bit = &c.bits[0];
    assert!(bit.lonely && !bit.in_position() && !bit.shader);
    assert_eq!((bit.repair_counter, bit.no_repair_needed_counter), (1, 0));
    assert_eq!(c.displaced_counter, 1);
    let spin = (-1.0f32).lerp(1.0, mirror.value()) * 1.0 * 0.02;
    let flip = (-1.0f32).lerp(1.0, mirror.value()) * 1.0 * 0.04;
    assert!(near(bit.rotat_vel, spin) && near(bit.d_rotat_vel, flip));
    assert_eq!(rng.state(), mirror.state());
    // The next turn turns it by the spin.
    c.update_bit(
        0,
        &mut BitCx {
            room: &s.room,
            ai_map: &s.map,
            sounds: &mut sounds,
            rng: &mut rng,
        },
    );
    let bit = &c.bits[0];
    let turned = Vec2::new(0.0, 1.0) + Vec2::new(0.0, 1.0).perpendicular() * spin;
    assert!(near_vec(bit.rotat, turned.normalized()));
    assert!(near_vec(bit.last_rotat, Vec2::new(0.0, 1.0)));
    // Lonely twice running draws nothing.
    assert_eq!(rng.state(), mirror.state());
}

/// A bit in place counts the ticks it needed no repair; after 40 its repair counter is
/// cleared. Nothing is drawn.
#[test]
fn a_bit_in_place_forgets_its_repairs() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let (mut rng, mut sounds) = (UnityRandom::new(3), Sounds::default());
    let before = rng.state();
    c.bits[0].repair_counter = 300;
    for tick in 1..=41 {
        c.update_bit(
            0,
            &mut BitCx {
                room: &s.room,
                ai_map: &s.map,
                sounds: &mut sounds,
                rng: &mut rng,
            },
        );
        assert_eq!(c.bits[0].no_repair_needed_counter, tick);
        assert_eq!(c.bits[0].repair_counter, if tick > 40 { 0 } else { 300 });
    }
    assert!(c.bits[0].shader && !c.bits[0].lonely);
    assert_eq!(c.displaced_counter, 0);
    assert_eq!(rng.state(), before);
}

/// A bit within 2 px of its place and upright snaps home at once (rotation, spin and flip
/// reset) and reconnects the links to neighbours in place while its repair counter is
/// under 200.
#[test]
fn a_bit_near_home_snaps_into_place_and_relinks() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let other = c.bits[0].neighbors[0];
    c.bits[0].active_connections[0] = false;
    c.bits[0].pos = c.bits[0].stuck_pos + Vec2::new(1.5, 0.0);
    c.bits[0].rotat_vel = 0.3;
    c.bits[0].repair_counter = 199;
    c.try_repair(0);
    let bit = &c.bits[0];
    assert_eq!(bit.pos, bit.stuck_pos);
    assert_eq!(
        (bit.rotat, bit.d_rotat),
        (Vec2::new(0.0, 1.0), Vec2::new(0.0, 1.0))
    );
    assert_eq!((bit.rotat_vel, bit.d_rotat_vel), (0.0, 0.0));
    assert!(c.bits[other].in_position() && bit.active_connections[0]);
    c.bits[0].active_connections[0] = false;
    c.bits[0].repair_counter = 200;
    c.bits[0].pos = c.bits[0].stuck_pos + Vec2::new(1.5, 0.0);
    c.try_repair(0);
    assert!(!c.bits[0].active_connections[0]);
    // Tilted past 0.9: no repair.
    c.bits[0].pos = c.bits[0].stuck_pos + Vec2::new(1.5, 0.0);
    c.bits[0].rotat = Vec2::new(1.0, 0.0);
    c.try_repair(0);
    assert!(!near(c.bits[0].pos.x, c.bits[0].stuck_pos.x));
}

/// A displaced bit's link breaks when `value < disconnectChance` (here 1): the sound, then
/// the `0.2` draw for a snap, then the lonely spin draws; the draws and the sound's equal
/// a mirror making them in that order.
#[test]
fn a_link_breaks_by_chance_with_its_sound_and_draws() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let bit = &mut c.bits[0];
    for a in bit.active_connections.iter_mut().skip(1) {
        *a = false;
    }
    bit.pos = bit.stuck_pos + Vec2::new(30.0, 0.0);
    bit.last_pos = bit.pos;
    let at = bit.pos;
    c.disconnect_chance = 1.0;
    let (mut rng, mut sounds) = (UnityRandom::new(3), circuit_sounds());
    let (mut mirror, mut mirror_sounds) = (rng.clone(), circuit_sounds());
    c.update_bit(
        0,
        &mut BitCx {
            room: &s.room,
            ai_map: &s.map,
            sounds: &mut sounds,
            rng: &mut rng,
        },
    );
    assert!(!c.bits[0].active_connections[0]);
    mirror.value();
    mirror_sounds.play_at(SoundId::CoralCircuitBreak, at, 1.0, 1.0, &mut mirror);
    mirror.value();
    // No link pulls, so the bit is lonely: two spin draws.
    mirror.value();
    mirror.value();
    assert_eq!(rng.state(), mirror.state());
    let events = sounds.take_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], SoundEvent::Play(p) if p.id == SoundId::CoralCircuitBreak));
}

/// A link to a neighbour that dropped its own link to this bit breaks with the sound; the
/// checks `value < disconnectChance` and `value < 0.2` still draw.
#[test]
fn a_link_the_other_end_dropped_breaks_with_a_sound() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let other = c.bits[0].neighbors[0];
    for a in c.bits[0].active_connections.iter_mut().skip(1) {
        *a = false;
    }
    let back = c.bits[other]
        .neighbors
        .iter()
        .position(|&n| n == 0)
        .expect("symmetric");
    c.bits[other].active_connections[back] = false;
    c.bits[0].pos = c.bits[0].stuck_pos + Vec2::new(30.0, 0.0);
    c.bits[0].last_pos = c.bits[0].pos;
    c.disconnect_chance = 0.0;
    let (mut rng, mut sounds) = (UnityRandom::new(3), circuit_sounds());
    let (mut mirror, mut mirror_sounds) = (rng.clone(), circuit_sounds());
    let at = c.bits[0].pos;
    c.update_bit(
        0,
        &mut BitCx {
            room: &s.room,
            ai_map: &s.map,
            sounds: &mut sounds,
            rng: &mut rng,
        },
    );
    assert!(!c.bits[0].active_connections[0]);
    mirror_sounds.play_at(SoundId::CoralCircuitBreak, at, 1.0, 1.0, &mut mirror);
    // The chance check, the snap check, then the lonely spin's two.
    for _ in 0..4 {
        mirror.value();
    }
    assert_eq!(rng.state(), mirror.state());
}

/// `disconnectChance` follows the ticks' displaced bits: `InverseLerp(300, 1600, n)`, the
/// counter cleared.
#[test]
fn the_disconnect_chance_follows_the_displaced_count() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    c.displaced_counter = 950;
    c.finish_update();
    assert!(near(c.disconnect_chance, 0.5));
    assert_eq!(c.displaced_counter, 0);
    c.displaced_counter = 100;
    c.finish_update();
    assert_eq!(c.disconnect_chance, 0.0);
}

fn chunk(pos: Vec2, vel: Vec2) -> PushChunk {
    PushChunk {
        pos,
        vel,
        rad: 5.0,
        mass: 1.0,
    }
}

/// A chunk 5 px left of a bit in place, moving right at 2 (radius 5, mass 1; strength 1):
/// reach 15, `fac = InverseLerp(50, 10, 0) * InverseLerp(1, 3, 2) * 1 = 0.5`, the bit takes
/// `(15 - 5) * 0.5 * 1/1.01` along (1, 0) as position and velocity plus 0.3 of the
/// chunk's velocity scaled by `fac * share`; the chunk is moved back by the rest
/// (`(15 - 5) * 0.5 * (1 - 1/1.01)`) and slowed by 0.3 of `fac * (1 - share)` of what is left.
#[test]
fn a_chunk_pushes_a_bit_by_their_masses() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let at = c.bits[0].stuck_pos;
    let mut ch = chunk(at - Vec2::new(5.0, 0.0), Vec2::new(2.0, 0.0));
    let share = 1.0f32 / 1.01;
    let to_bit = 10.0 * 0.5 * share;
    let to_chunk = 10.0 * 0.5 * (1.0 - share);
    let overlap = c.push_bit(0, &mut ch, 1.0).expect("touched");
    let bit = &c.bits[0];
    assert!(near_vec(bit.pos, at + Vec2::new(to_bit, 0.0)));
    assert!(near_vec(
        bit.vel,
        Vec2::new(to_bit + 2.0 * (0.5 * share * 0.3), 0.0)
    ));
    assert!(near(ch.pos.x, at.x - 5.0 - to_chunk));
    let slowed = 2.0 - to_chunk;
    assert!(near(
        ch.vel.x,
        slowed - slowed * (0.5 * (1.0 - share) * 0.3)
    ));
    // `InverseLerp(50, 10, distance after the push) * strength`.
    assert!(near(overlap, 1.0));
    // A bit away from its place (over 20 px) or a chunk beyond `rad + 10` is not touched.
    let mut ch2 = chunk(at - Vec2::new(16.0, 0.0), Vec2::new(2.0, 0.0));
    let mut c2 = s.circuit(vec![PLACE], 7);
    assert!(c2.push_bit(0, &mut ch2, 1.0).is_none());
    c2.bits[0].pos += Vec2::new(25.0, 0.0);
    let mut ch3 = chunk(c2.bits[0].pos - Vec2::new(5.0, 0.0), Vec2::new(2.0, 0.0));
    assert!(c2.push_bit(0, &mut ch3, 1.0).is_none());
}

/// The circuit's strength on a chunk is the places' largest `InverseLerp(rad + 100, rad,
/// distance)`; the candidates are the bits of the tiles two beyond the chunk's box.
#[test]
fn the_strength_and_candidates_follow_the_places_and_the_chunk_box() {
    let mut s = Setup::new();
    let c = s.circuit(vec![PLACE], 7);
    assert_eq!(c.push_strength(PLACE.pos), 1.0);
    assert!(near(
        c.push_strength(PLACE.pos + Vec2::new(110.0, 0.0)),
        0.5
    ));
    assert_eq!(c.push_strength(PLACE.pos + Vec2::new(170.0, 0.0)), 0.0);
    let ch = chunk(PLACE.pos, Vec2::ZERO);
    let mut expect = Vec::new();
    for x in 30 - 2..=30 + 2 {
        for y in 20 - 2..=20 + 2 {
            for layer in 0..2 {
                expect.extend(c.bit_at(x, y, layer));
            }
        }
    }
    assert_eq!(c.push_candidates(&ch), expect);
    assert!(expect.len() >= 25);
}

/// `Explosion`: bits within `rad` get `(DirVec(pos, bit) * frc + dirVec) * InverseLerp(rad,
/// rad / 2, distance)`; more than five of them sound once (volume `InverseLerp(4, 19, n)`),
/// five or fewer are silent.
#[test]
fn an_explosion_throws_the_bits_and_sounds_over_five() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let at = c.bits[0].stuck_pos + Vec2::new(1.0, 1.0);
    let dir = Vec2::new(0.0, -6.0);
    let hit: Vec<usize> = (0..c.bits.len())
        .filter(|&i| c.bits[i].pos.dist_less(at, 40.0))
        .collect();
    assert!(hit.len() > 5, "{}", hit.len());
    let (mut rng, mut sounds) = (UnityRandom::new(3), circuit_sounds());
    let (mut mirror, mut mirror_sounds) = (rng.clone(), circuit_sounds());
    c.explosion(at, 40.0, 12.0, dir, &mut sounds, &mut rng);
    for &i in &hit {
        let d = c.bits[i].stuck_pos.distance(at);
        let want = (at.direction_to(c.bits[i].stuck_pos) * 12.0 + dir) * d.inverse_lerp(40.0, 20.0);
        assert!(near_vec(c.bits[i].vel, want), "bit {i}");
    }
    mirror_sounds.play_at(
        SoundId::CoralCircuitJumpExplosion,
        at,
        (hit.len() as f32).inverse_lerp(4.0, 19.0),
        1.0,
        &mut mirror,
    );
    assert_eq!(rng.state(), mirror.state());
    let events = sounds.take_events();
    assert!(
        matches!(&events[..], [SoundEvent::Play(p)] if p.id == SoundId::CoralCircuitJumpExplosion)
    );
    // A small radius reaches at most five.
    let mut c = s.circuit(vec![PLACE], 7);
    let few = (0..c.bits.len())
        .filter(|&i| c.bits[i].pos.dist_less(at, 12.0))
        .count();
    assert!(few <= 5);
    let (mut rng, mut sounds) = (UnityRandom::new(3), circuit_sounds());
    let before = rng.state();
    c.explosion(at, 12.0, 12.0, dir, &mut sounds, &mut rng);
    assert_eq!(rng.state(), before);
    assert!(sounds.take_events().is_empty());
}

/// A connector's turn with its system frozen: every strand is culled (no static culling,
/// frozen only), so no draws, and the spoke `DegToVec(i / n * 360)` is added to the second
/// point's velocity.
#[test]
fn a_connector_nudges_each_strand_along_its_spoke() {
    let mut s = Setup::new();
    let c = s.circuit(vec![PLACE], 7);
    let x = &c.connectors[0];
    for &id in &x.connections {
        assert!(!s.pool.get(id).unwrap().use_static_culling);
    }
    let was: Vec<Vec2> = x
        .connections
        .iter()
        .map(|&id| s.pool.get(id).unwrap().points[1][2])
        .collect();
    let mut rng = UnityRandom::new(3);
    let before = rng.state();
    let Setup {
        room, map, pool, ..
    } = &mut s;
    let env = MyceliumEnv {
        room,
        room_index: 0,
        ai_map: Some(map),
        camera: camera(),
        cameras_changed_tick: 1,
        system: Some(SystemState {
            id: SystemId(1),
            frozen: true,
        }),
    };
    x.update(&c.bits, pool, &env, &mut rng);
    assert_eq!(rng.state(), before);
    let n = x.connections.len();
    for (i, &id) in x.connections.iter().enumerate() {
        let spoke = Vec2::from_degrees(i as f32 / n as f32 * 360.0);
        let now = s.pool.get(id).unwrap().points[1][2];
        assert!(near_vec(now, was[i] + spoke), "strand {i}");
    }
}

/// A connector's strands are made at the dot (`OnCircuitPos`), 30 px long: two points, so
/// `2 + 4` draws each (the reach direction is zero), registered in the system's registry
/// with the bits' other strands.
#[test]
fn a_connector_makes_its_strands_at_the_dot() {
    let mut s = Setup::new();
    let c = s.circuit(vec![PLACE], 7);
    let total: usize = c.connectors.iter().map(|x| x.connections.len()).sum();
    assert_eq!(s.pool.registry(SystemId(1)).len(), total);
    let x = &c.connectors[0];
    let dot = x.dot_pos(&c.bits, 1.0);
    let bit = &c.bits[x.bit];
    assert!(dot.dist_less(bit.pos, bit.size.length()));
    for &id in &x.connections {
        let m = s.pool.get(id).unwrap();
        assert_eq!((m.points.len(), m.length), (2, 30.0));
        assert!(m.points[0][0].dist_less(dot, 3.0));
    }
}

/// A crab at rest draws `value < 0.05` once and, per leg, `value < 0.025` and, if that
/// fails, `value < 1/60`; with none of them passing it stays on its bit and the legs hang
/// from its body.
#[test]
fn a_crab_at_rest_draws_per_leg() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let seed = (0..200)
        .find(|&seed| {
            let mut r = UnityRandom::new(seed);
            let legs = c.crabs[0].legs.len();
            r.value() >= 0.05 && (0..legs).all(|_| r.value() >= 0.025 && r.value() >= 1.0 / 60.0)
        })
        .expect("a quiet seed");
    let mut rng = UnityRandom::new(seed);
    let mut mirror = rng.clone();
    let legs = c.crabs[0].legs.len();
    let CoralCircuit { bits, crabs, .. } = &mut c;
    crabs[0].update(bits, &mut rng);
    mirror.value();
    for _ in 0..legs * 2 {
        mirror.value();
    }
    assert_eq!(rng.state(), mirror.state());
    let crab = &crabs[0];
    assert_eq!((crab.prog, crab.last_prog), (1.0, 1.0));
    let body = crab.body_pos(bits, 1.0);
    for (i, leg) in crab.legs.iter().enumerate() {
        assert!(near_vec(leg.points[0][0], body));
        let spoke = Vec2::from_degrees(i as f32 / legs as f32 * 360.0);
        assert!(near_vec(leg.points[0][2], spoke));
    }
}

/// A crab whose first draw is under 0.05 chooses a bit (own or a neighbour), a spot on it
/// and hops if it is under 50 px away: `prog` 0 and a speed of `0.5 / distance`; the next
/// turns walk it, and on arrival it stands on the new bit.
#[test]
fn a_crab_hops_to_a_near_spot_and_arrives() {
    let mut s = Setup::new();
    let mut c = s.circuit(vec![PLACE], 7);
    let CoralCircuit { bits, crabs, .. } = &mut c;
    let mut hopped = false;
    for seed in 0..400 {
        let mut crab = crabs[0].clone();
        let mut rng = UnityRandom::new(seed);
        crab.update(bits, &mut rng);
        if crab.prog < 1.0 || crab.last_prog == 0.0 {
            assert_eq!(crab.last_prog, 0.0);
            assert!(crab.move_speed > 0.0);
            let from = bits[crab.bit].on_circuit_pos(crab.c_pos, 1.0);
            let to = bits[crab.next_bit].on_circuit_pos(crab.next_c_pos, 1.0);
            assert!(from.distance(to) < 50.0);
            assert!(near(crab.move_speed, 0.5 / from.distance(to)));
            let (target, spot) = (crab.next_bit, crab.next_c_pos);
            let mut rng = UnityRandom::new(1000);
            let mut turns = 0;
            while crab.prog < 1.0 {
                crab.update(bits, &mut rng);
                turns += 1;
                assert!(turns < 10_000);
            }
            assert_eq!((crab.bit, crab.c_pos), (target, spot));
            hopped = true;
            break;
        }
    }
    assert!(hopped);
}
