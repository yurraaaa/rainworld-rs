//! `CoralCircuit`: a cluster of overlapping placed `CoralCircuit` objects as one grid of
//! `CircuitBit`s (flat maze tiles that sit on their tiles, are knocked loose by bodies, break
//! their links and repair), with the `CircuitConnector`s and `CircuitCrab`s on them. The
//! constructor is seeded from the first object's place (the shared stream is restored after);
//! each bit has its own turn in the update list, and the circuit's own pushes every nearby
//! body chunk (layer 1 and up) off the bits. The player's side of a push is
//! [`crate::Player::collide_with_coral_circuit_bit`]; the sprites are `rw-graphics`'.
//! Behaviour: `rainworld-re/notes/superstructure-coral.md`.
//!
//! Mirrors: CoralBrain/CoralCircuit.cs (CoralCircuit, CircuitBit)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::math::float_ext::FloatExt;
use crate::physics::shared_physics::TerrainCollisionData;
use crate::superstructure::coral::mycelium::{MyceliumPool, SystemId};
use crate::world::AiMap;
use crate::{BodyChunk, CircuitConnector, CircuitCrab, SoundId, Sounds, Vec2};

/// A tile `(x, y)`.
type Tile = (i32, i32);

/// `Custom.fourDirectionsAndZero`.
const FOUR_DIRECTIONS_AND_ZERO: [(i32, i32); 5] = [(0, 0), (-1, 0), (0, -1), (1, 0), (0, 1)];

/// `Custom.fourDirections`.
const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// A placed `CoralCircuit`: where, and its `ResizableObjectData.Rad`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircuitPlace {
    pub pos: Vec2,
    pub rad: f32,
}

/// Which bit of which circuit of a room (the game compares the objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitRef {
    pub room: i32,
    pub circuit: u32,
    pub bit: u32,
}

/// The part of a body chunk a push reads and moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PushChunk {
    pub pos: Vec2,
    pub vel: Vec2,
    pub rad: f32,
    pub mass: f32,
}

impl From<&BodyChunk> for PushChunk {
    fn from(c: &BodyChunk) -> Self {
        Self {
            pos: c.pos,
            vel: c.vel,
            rad: c.rad,
            mass: c.mass,
        }
    }
}

/// What a bit's turn reads of the room and plays into it.
pub struct BitCx<'a> {
    pub room: &'a Room,
    pub ai_map: &'a AiMap,
    pub sounds: &'a mut Sounds,
    pub rng: &'a mut UnityRandom,
}

/// What the constructor reads of the room and makes in it.
pub struct CircuitCx<'a> {
    pub room: &'a Room,
    pub ai_map: &'a AiMap,
    pub system: SystemId,
    pub pool: &'a mut MyceliumPool,
    /// The room's update-list counter (`AddObject`).
    pub seq: &'a mut u64,
}

impl CircuitCx<'_> {
    pub(crate) fn next_seq(&mut self) -> u64 {
        *self.seq += 1;
        *self.seq
    }
}

/// One `CircuitBit`.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitBit {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub tile: (i32, i32),
    /// `neighbors`, as indices into the circuit's `bits`.
    pub neighbors: Vec<usize>,
    pub neighbor_dists: Vec<f32>,
    pub neighbor_dirs: Vec<f32>,
    pub active_connections: Vec<bool>,
    pub last_pos: Vec2,
    pub pos: Vec2,
    pub vel: Vec2,
    pub stuck_pos: Vec2,
    pub size: Vec2,
    pub rotat: Vec2,
    pub last_rotat: Vec2,
    pub d_rotat: Vec2,
    pub last_d_rotat: Vec2,
    pub rotat_vel: f32,
    pub d_rotat_vel: f32,
    pub lonely: bool,
    pub last_lonely: bool,
    pub repair_counter: i32,
    pub no_repair_needed_counter: i32,
    /// `shader`: the sprite shows `CoralCircuitBothSides` (the bit was in position at the
    /// last turn) rather than `DeadCoralCircuitBothSides`.
    pub shader: bool,
}

impl CircuitBit {
    /// The constructor: `stuckPos` a tile's middle off by up to 9 px (two draws), then the
    /// size (two draws; larger near the middle of the places).
    fn new(
        tile: (i32, i32),
        places: &[CircuitPlace],
        update_order: u64,
        rng: &mut UnityRandom,
    ) -> Self {
        let dx = (-9.0f32).lerp(9.0, rng.value());
        let dy = (-9.0f32).lerp(9.0, rng.value());
        let stuck_pos = Vec2::middle_of_tile(tile) + Vec2::new(dx, dy);
        let mut near = 0.0f32;
        for p in places {
            near = near.max(stuck_pos.distance(p.pos).inverse_lerp(p.rad, p.rad - 220.0));
        }
        let (min, max) = if near > 0.5 {
            (15.0, 35.0)
        } else {
            (5.0, 25.0)
        };
        let width = min.lerp(max, rng.value());
        let height = min.lerp(max, rng.value());
        Self {
            update_order,
            tile,
            neighbors: Vec::new(),
            neighbor_dists: Vec::new(),
            neighbor_dirs: Vec::new(),
            active_connections: Vec::new(),
            last_pos: stuck_pos,
            pos: stuck_pos,
            vel: Vec2::ZERO,
            stuck_pos,
            size: Vec2::new(width.floor(), height.floor()),
            rotat: Vec2::new(0.0, 1.0),
            last_rotat: Vec2::ZERO,
            d_rotat: Vec2::new(0.0, 1.0),
            last_d_rotat: Vec2::ZERO,
            rotat_vel: 0.0,
            d_rotat_vel: 0.0,
            lonely: false,
            last_lonely: false,
            repair_counter: 0,
            no_repair_needed_counter: 0,
            shader: false,
        }
    }

    /// `OverLap`: the boxes round the two stuck positions overlap.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.stuck_pos.x - self.size.x < other.stuck_pos.x + other.size.x
            && self.stuck_pos.x + self.size.x > other.stuck_pos.x - other.size.x
            && self.stuck_pos.y - self.size.y < other.stuck_pos.y + other.size.y
            && self.stuck_pos.y + self.size.y > other.stuck_pos.y - other.size.y
    }

    /// `InPosition`: within 1 px of its stuck position and not moving.
    #[must_use]
    pub fn in_position(&self) -> bool {
        self.pos.dist_less(self.stuck_pos, 1.0) && self.vel.length() == 0.0
    }

    /// `OnCircuitPos` of a connector or crab: `rel` (from the box's middle) squashed by the
    /// flip, turned by the rotation and moved to the bit, at `time_stacker`.
    #[must_use]
    pub fn on_circuit_pos(&self, rel: Vec2, time_stacker: f32) -> Vec2 {
        let squash = self.last_d_rotat.slerp(self.d_rotat, time_stacker).y;
        let turned = Vec2::new(rel.x * squash, rel.y)
            .rotate_around_origo(self.last_rotat.slerp(self.rotat, time_stacker).to_degrees());
        turned + self.last_pos.lerp(self.pos, time_stacker.clamp(0.0, 1.0))
    }
}

/// A `CoralCircuit` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct CoralCircuit {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub places: Vec<CircuitPlace>,
    pub tiles: Vec<(i32, i32)>,
    pub rotation: Vec2,
    pub bottom_left: (i32, i32),
    pub top_right: (i32, i32),
    /// The grid's size in tiles (the game's array lengths; 2 x 2 when no tile was found).
    pub dims: (i32, i32),
    /// `bits[x, y, n]` as an index into `bits` (`(x * height + y) * 2 + n`).
    grid: Vec<Option<usize>>,
    /// The bits in the order the constructor added them.
    pub bits: Vec<CircuitBit>,
    pub connectors: Vec<CircuitConnector>,
    pub crabs: Vec<CircuitCrab>,
    pub displaced_counter: i32,
    pub disconnect_chance: f32,
}

impl CoralCircuit {
    /// Layers of bits a tile holds.
    const DEPTH: i32 = 2;

    /// The clusters `AIMapReady` makes of the placed `CoralCircuit` objects: the first
    /// object, then every object within `Rad + Rad` of one already in the cluster, found
    /// by scanning the rest from its end until a pass adds nothing.
    pub fn clusters(mut rest: Vec<CircuitPlace>) -> Vec<Vec<CircuitPlace>> {
        let mut clusters = Vec::new();
        while !rest.is_empty() {
            let mut cluster = vec![rest.remove(0)];
            loop {
                let mut quiet = true;
                let mut j = 0;
                while j < cluster.len() {
                    let found = (0..rest.len()).rev().find(|&k| {
                        cluster[j]
                            .pos
                            .dist_less(rest[k].pos, cluster[j].rad + rest[k].rad)
                    });
                    if let Some(k) = found {
                        cluster.push(rest.remove(k));
                        quiet = false;
                    }
                    j += 1;
                }
                if quiet {
                    break;
                }
            }
            clusters.push(cluster);
        }
        clusters
    }

    /// The constructor (`room.AddObject` of the bits, connectors and crabs along the way,
    /// then of the circuit).
    pub fn build(places: Vec<CircuitPlace>, cx: &mut CircuitCx<'_>, rng: &mut UnityRandom) -> Self {
        let saved = rng.state();
        let first = places[0].pos;
        rng.init_state(first.x.to_int_cs().wrapping_add(first.y.to_int_cs()));
        let rotation = Vec2::random_unit(rng);
        let (tiles, [bottom_left, top_right]) = Self::find_tiles(&places, cx);
        let dims = (
            top_right.0.wrapping_sub(bottom_left.0).wrapping_add(1),
            top_right.1.wrapping_sub(bottom_left.1).wrapping_add(1),
        );
        let cells = usize::try_from(dims.0).unwrap_or(0)
            * usize::try_from(dims.1).unwrap_or(0)
            * Self::DEPTH as usize;
        let mut circuit = Self {
            update_order: 0,
            places,
            tiles,
            rotation,
            bottom_left,
            top_right,
            dims,
            grid: vec![None; cells],
            bits: Vec::new(),
            connectors: Vec::new(),
            crabs: Vec::new(),
            displaced_counter: 0,
            disconnect_chance: 0.0,
        };
        circuit.make_bits(cx, rng);
        circuit.connect_bits();
        circuit.make_connectors_and_crabs(cx, rng);
        circuit.update_order = cx.next_seq();
        rng.set_state(saved);
        circuit
    }

    /// The free tiles within a place's radius, away from walls, with the box they span.
    fn find_tiles(places: &[CircuitPlace], cx: &CircuitCx<'_>) -> (Vec<Tile>, [Tile; 2]) {
        let mut tiles: Vec<(i32, i32)> = Vec::new();
        let mut bottom_left = (i32::MAX, i32::MAX);
        let mut top_right = (i32::MIN, i32::MIN);
        for place in places {
            let centre = place.pos.tile();
            let reach = (place.rad / 20.0).to_int_cs();
            for j in centre.0 - reach - 1..=centre.0 + reach + 1 {
                for k in centre.1 - reach - 1..=centre.1 + reach + 1 {
                    if !cx.room.tile(j, k).is_solid()
                        && cx.ai_map.terrain_proximity(j, k) > 2
                        && Vec2::middle_of_tile((j, k)).dist_less(place.pos, place.rad)
                        && !tiles.contains(&(j, k))
                    {
                        tiles.push((j, k));
                        bottom_left = (bottom_left.0.min(j), bottom_left.1.min(k));
                        top_right = (top_right.0.max(j), top_right.1.max(k));
                    }
                }
            }
        }
        (tiles, [bottom_left, top_right])
    }

    /// One bit (two near the places' edges) on every tile, each added to the room.
    fn make_bits(&mut self, cx: &mut CircuitCx<'_>, rng: &mut UnityRandom) {
        for l in 0..self.tiles.len() {
            let tile = self.tiles[l];
            let middle = Vec2::middle_of_tile(tile);
            let mut near = 0.0f32;
            for p in &self.places {
                near = near.max(middle.distance(p.pos).inverse_lerp(p.rad, p.rad - 220.0));
            }
            let count = if f64::from(near) < 0.5 { 2 } else { 1 };
            for n in 0..count {
                let order = cx.next_seq();
                let bit = CircuitBit::new(tile, &self.places, order, rng);
                let cell = self.cell(tile.0, tile.1, n);
                self.grid[cell.unwrap_or(0)] = Some(self.bits.len());
                self.bits.push(bit);
            }
        }
    }

    /// `Connect` of every bit: the bits of its own and the four next tiles whose boxes
    /// overlap its box.
    fn connect_bits(&mut self) {
        for i in 0..self.bits.len() {
            let tile = self.bits[i].tile;
            let (mut neighbors, mut dists, mut dirs) = (Vec::new(), Vec::new(), Vec::new());
            for (dx, dy) in FOUR_DIRECTIONS_AND_ZERO {
                for j in 0..Self::DEPTH {
                    let Some(other) = self.bit_at(tile.0 + dx, tile.1 + dy, j) else {
                        continue;
                    };
                    if other != i && self.bits[i].overlaps(&self.bits[other]) {
                        neighbors.push(other);
                        dists.push(self.bits[i].stuck_pos.distance(self.bits[other].stuck_pos));
                        dirs.push(self.bits[i].stuck_pos.to_aim(self.bits[other].stuck_pos));
                    }
                }
            }
            let bit = &mut self.bits[i];
            bit.active_connections = vec![true; neighbors.len()];
            bit.neighbors = neighbors;
            bit.neighbor_dists = dists;
            bit.neighbor_dirs = dirs;
        }
    }

    /// `Custom.IntClamp(tiles / 15, 1, 40)` connectors, then `IntClamp(tiles / 30, 1, 20)`
    /// crabs, each on a random tile's random layer, if a bit is there.
    fn make_connectors_and_crabs(&mut self, cx: &mut CircuitCx<'_>, rng: &mut UnityRandom) {
        if self.tiles.is_empty() {
            return;
        }
        let count = i32::try_from(self.tiles.len()).unwrap_or(i32::MAX);
        for _ in 0..(count / 15).clamp(1, 40) {
            if let Some(bit) = self.random_bit(rng) {
                let connector = CircuitConnector::new(bit, &self.bits, cx, rng);
                self.connectors.push(connector);
            }
        }
        for _ in 0..(count / 30).clamp(1, 20) {
            if let Some(bit) = self.random_bit(rng) {
                let crab = CircuitCrab::new(bit, &self.bits, cx, rng);
                self.crabs.push(crab);
            }
        }
    }

    /// `tiles[Random.Range(0, tiles.Count)]`'s bit at `Random.Range(0, depth)`.
    fn random_bit(&self, rng: &mut UnityRandom) -> Option<usize> {
        let count = i32::try_from(self.tiles.len()).unwrap_or(i32::MAX);
        let tile = self.tiles[usize::try_from(rng.range_i32(0, count)).unwrap_or(0)];
        let layer = rng.range_i32(0, Self::DEPTH);
        self.bit_at(tile.0, tile.1, layer)
    }

    /// The grid cell of `Bit(x, y, i)`, if it is inside the grid.
    fn cell(&self, x: i32, y: i32, i: i32) -> Option<usize> {
        if x < self.bottom_left.0
            || x > self.top_right.0
            || y < self.bottom_left.1
            || y > self.top_right.1
            || !(0..Self::DEPTH).contains(&i)
        {
            return None;
        }
        let (cx, cy) = (x - self.bottom_left.0, y - self.bottom_left.1);
        usize::try_from((cx * self.dims.1 + cy) * Self::DEPTH + i).ok()
    }

    /// `Bit(x, y, i)`: the index of the bit, if there is one.
    #[must_use]
    pub fn bit_at(&self, x: i32, y: i32, i: i32) -> Option<usize> {
        self.cell(x, y, i)
            .and_then(|c| self.grid.get(c).copied().flatten())
    }

    /// `CircuitBit.TryRepair` of bit `i`: a bit within 2 px of its place and nearly upright
    /// snaps into it and reconnects the links whose other end is in place.
    pub fn try_repair(&mut self, i: usize) {
        let bit = &self.bits[i];
        let upright = bit.rotat.y > 0.9;
        if !bit.pos.dist_less(bit.stuck_pos, 2.0) || !upright {
            return;
        }
        let bit = &mut self.bits[i];
        bit.rotat = Vec2::new(0.0, 1.0);
        bit.rotat_vel = 0.0;
        bit.d_rotat = Vec2::new(0.0, 1.0);
        bit.d_rotat_vel = 0.0;
        bit.pos = bit.stuck_pos;
        for k in 0..self.bits[i].neighbors.len() {
            let other = self.bits[i].neighbors[k];
            if !self.bits[i].active_connections[k]
                && self.bits[other].in_position()
                && self.bits[i].repair_counter < 200
            {
                self.bits[i].active_connections[k] = true;
            }
        }
    }

    /// `CircuitBit.Update` of bit `i`.
    pub fn update_bit(&mut self, i: usize, cx: &mut BitCx<'_>) {
        self.bit_move(i);
        self.bits[i].last_lonely = self.bits[i].lonely;
        self.try_repair(i);
        if self.bits[i].in_position() {
            let bit = &mut self.bits[i];
            bit.lonely = false;
            bit.no_repair_needed_counter += 1;
            if bit.no_repair_needed_counter > 40 {
                bit.repair_counter = 0;
            }
        } else {
            if self.bits[i].repair_counter > 100 {
                self.bit_return(i, cx);
            }
            self.bits[i].no_repair_needed_counter = 0;
            self.bits[i].repair_counter = (self.bits[i].repair_counter + 1).min(600);
            let (rotation_sum, linked) = if self.bits[i].repair_counter < 550 {
                self.bit_links(i, cx)
            } else {
                (Vec2::ZERO, 0)
            };
            self.try_repair(i);
            if linked > 0 {
                let bit = &mut self.bits[i];
                bit.rotat_vel = 0.0;
                bit.d_rotat_vel = 0.0;
                bit.d_rotat = bit.d_rotat.slerp(Vec2::new(0.0, 1.0), 0.5);
                bit.rotat = rotation_sum.normalized();
                bit.lonely = false;
            } else {
                self.bits[i].lonely = true;
            }
            if self.bits[i].repair_counter < 550
                && cx.ai_map.terrain_proximity_at(self.bits[i].pos) < 2
            {
                self.bit_terrain(i, cx);
            }
        }
        if self.bits[i].lonely && !self.bits[i].last_lonely {
            let speed = self.bits[i].vel.length().clamp(1.0, 5.0);
            self.bits[i].rotat_vel = (-1.0f32).lerp(1.0, cx.rng.value()) * speed * 0.02;
            self.bits[i].d_rotat_vel = (-1.0f32).lerp(1.0, cx.rng.value()) * speed * 0.04;
        }
        if !self.bits[i].in_position() {
            self.displaced_counter += 1;
        }
    }

    /// The start of `Update`: position, spin and flip advance.
    fn bit_move(&mut self, i: usize) {
        let bit = &mut self.bits[i];
        bit.last_pos = bit.pos;
        bit.pos += bit.vel;
        bit.vel *= 0.999;
        bit.last_rotat = bit.rotat;
        bit.rotat += bit.rotat.perpendicular() * bit.rotat_vel;
        bit.rotat = bit.rotat.normalized();
        bit.last_d_rotat = bit.d_rotat;
        bit.d_rotat += bit.rotat.perpendicular() * bit.d_rotat_vel;
        bit.d_rotat = bit.d_rotat.normalized();
        let in_position = bit.in_position();
        if in_position != bit.shader {
            bit.shader = in_position;
        }
    }

    /// A displaced bit that has waited over 100 ticks is pulled home, more strongly the
    /// longer it has waited, and sounds when it settles.
    fn bit_return(&mut self, i: usize, cx: &mut BitCx<'_>) {
        let bit = &self.bits[i];
        let mut pull = (bit.repair_counter as f32)
            .inverse_lerp(50.0, 600.0)
            .mathf_pow(5.0)
            * 0.2;
        if bit.pos.dist_less(bit.stuck_pos, 2.0) {
            pull = pull.mathf_pow(0.5);
        }
        for (k, &other) in bit.neighbors.iter().enumerate() {
            let n = &self.bits[other];
            if bit.active_connections[k] && n.pos.dist_less(n.stuck_pos, 2.0) {
                pull = pull.mathf_pow(0.5);
            }
        }
        let bit = &mut self.bits[i];
        let away = bit.pos.distance(bit.stuck_pos);
        bit.rotat_vel *= 1.0f32.lerp(away.lerp_map(40.0, 140.0, 0.8, 1.0), pull);
        bit.d_rotat_vel *= 1.0f32.lerp(away.lerp_map(40.0, 140.0, 0.8, 1.0), pull);
        bit.rotat = bit.rotat.slerp(Vec2::new(0.0, 1.0), 0.1 * pull);
        bit.vel *= 1.0f32.lerp(away.lerp_map(40.0, 140.0, 0.5, 0.9), pull);
        bit.vel += (bit.pos.direction_to(bit.stuck_pos) * pull).clamp_magnitude(away);
        if bit.pos.dist_less(bit.stuck_pos, (10.0 * pull).max(1.0)) {
            bit.vel *= 0.5;
            bit.pos = bit.pos.lerp(bit.stuck_pos, 0.2);
            bit.rotat_vel *= 0.5;
            bit.d_rotat_vel *= 0.5;
            bit.rotat = bit.rotat.slerp(Vec2::new(0.0, 1.0), 0.1);
            // The game slerps `rotat` here, not `dRotat`.
            bit.d_rotat = bit.rotat.slerp(Vec2::new(0.0, 1.0), 0.1);
            self.try_repair(i);
            if self.bits[i].in_position() {
                cx.sounds.play_at(
                    SoundId::CoralCircuitReactivate,
                    self.bits[i].pos,
                    1.0,
                    1.0,
                    cx.rng,
                );
            }
        }
    }

    /// The links of a displaced bit: one that the other end let go breaks, one breaks by
    /// chance (`disconnectChance`), a stretched one pulls the two together, a fast
    /// relative motion snaps it. Returns the sum of the links' directions turned back by
    /// their rest direction, and how many pulled.
    fn bit_links(&mut self, i: usize, cx: &mut BitCx<'_>) -> (Vec2, i32) {
        let mut sum = Vec2::ZERO;
        let mut linked = 0;
        for j in 0..self.bits[i].neighbors.len() {
            if !self.bits[i].active_connections[j] {
                continue;
            }
            let other = self.bits[i].neighbors[j];
            for k in 0..self.bits[other].neighbors.len() {
                if !self.bits[other].active_connections[k] && self.bits[other].neighbors[k] == i {
                    self.break_link(i, j, cx);
                }
            }
            if cx.rng.value() < self.disconnect_chance {
                self.break_link(i, j, cx);
            }
            let pos = self.bits[i].pos;
            if self.bits[i].active_connections[j]
                && self.bits[other]
                    .pos
                    .dist_less(pos, self.bits[i].neighbor_dists[j] * 1.2)
            {
                let away = (self.bits[other].pos - pos).normalized();
                let gap = self.bits[other].pos.distance(pos);
                let stiffness = 0.45
                    * (self.bits[i]
                        .repair_counter
                        .max(self.bits[other].repair_counter) as f32)
                        .inverse_lerp(550.0, 300.0);
                let pull = away * ((gap - self.bits[i].neighbor_dists[j]) * stiffness);
                self.bits[i].pos += pull;
                self.bits[i].vel += pull;
                self.bits[other].pos -= pull;
                self.bits[other].vel -= pull;
                sum += away.rotate_around_origo(-self.bits[i].neighbor_dirs[j]);
                self.try_repair(other);
                linked += 1;
            }
            if cx.rng.value() < 0.2 && self.bits[i].vel.distance(self.bits[other].vel) > 7.0 {
                self.break_link(i, j, cx);
            }
        }
        (sum, linked)
    }

    /// `Coral_Circuit_Break` at the bit, and link `j` of it is inactive.
    fn break_link(&mut self, i: usize, j: usize, cx: &mut BitCx<'_>) {
        cx.sounds.play_at(
            SoundId::CoralCircuitBreak,
            self.bits[i].pos,
            1.0,
            1.0,
            cx.rng,
        );
        self.bits[i].active_connections[j] = false;
    }

    /// A free bit near terrain bounces off it (one draw on a hit) and drifts away from
    /// the nearest walls.
    fn bit_terrain(&mut self, i: usize, cx: &mut BitCx<'_>) {
        let bit = &self.bits[i];
        let rad = bit.size.x.min(bit.size.y);
        let hit = TerrainCollisionData::new(bit.pos, bit.last_pos, bit.vel, rad, true)
            .vertical(cx.room)
            .horizontal(cx.room);
        let bit = &mut self.bits[i];
        bit.pos = hit.pos;
        bit.vel = hit.vel;
        if hit.contact_point.0 != 0 {
            bit.vel.x = bit.vel.x.abs() * -(hit.contact_point.0 as f32);
        }
        if hit.contact_point.1 != 0 {
            bit.vel.y = bit.vel.y.abs() * -(hit.contact_point.1 as f32);
        }
        if hit.contact_point.0 != 0 || hit.contact_point.1 != 0 {
            bit.rotat_vel *= 1.0 + (-1.0f32).lerp(1.0, cx.rng.value()) * bit.vel.length() * 0.1;
        }
        if cx.ai_map.terrain_proximity_at(bit.pos) < 2 {
            let tile = bit.pos.tile();
            let mut open = Vec2::ZERO;
            for (dx, dy) in FOUR_DIRECTIONS {
                let (x, y) = (tile.0 + dx, tile.1 + dy);
                if !cx.room.tile(x, y).is_solid() && !cx.ai_map.narrow_space(x, y) {
                    let mut room_around = 0.0f32;
                    for (mx, my) in FOUR_DIRECTIONS {
                        room_around += cx.ai_map.terrain_proximity(x + mx, y + my).min(4) as f32;
                    }
                    open += Vec2::new(dx as f32, dy as f32) * room_around;
                }
            }
            bit.vel += open.normalized() * 0.1;
        }
    }

    /// How far the circuit's places push on a chunk at `pos`: the largest
    /// `InverseLerp(rad + 100, rad, distance)`, 0 if none reaches it.
    #[must_use]
    pub fn push_strength(&self, pos: Vec2) -> f32 {
        let mut strength = 0.0f32;
        for p in &self.places {
            if strength >= 1.0 {
                break;
            }
            strength = strength.max(pos.distance(p.pos).inverse_lerp(p.rad + 100.0, p.rad));
        }
        strength
    }

    /// The bits `Update` visits for a chunk, in its order: the tiles round the chunk, the
    /// bit layers of each.
    #[must_use]
    pub fn push_candidates(&self, chunk: &PushChunk) -> Vec<usize> {
        let reach = Vec2::new(chunk.rad, chunk.rad);
        let low = (chunk.pos - reach).tile();
        let high = (chunk.pos + reach).tile();
        let mut bits = Vec::new();
        for x in low.0 - 2..=high.0 + 2 {
            for y in low.1 - 2..=high.1 + 2 {
                for layer in 0..Self::DEPTH {
                    bits.extend(self.bit_at(x, y, layer));
                }
            }
        }
        bits
    }

    /// One bit's push on a chunk (`Update`'s inner body) with the circuit's `strength` on
    /// it: a bit near its place and the chunk close to it are moved apart by their masses
    /// and trade some of the chunk's velocity. Returns the overlap factor the player's
    /// `CollideWithCoralCircuitBit` takes, when the bit was touched.
    pub fn push_bit(&mut self, i: usize, chunk: &mut PushChunk, strength: f32) -> Option<f32> {
        let bit = &mut self.bits[i];
        let reach = chunk.rad + 10.0;
        if !(bit.pos.dist_less(bit.stuck_pos, 20.0) && bit.pos.dist_less(chunk.pos, reach)) {
            return None;
        }
        let away = chunk.pos.direction_to(bit.pos);
        let gap = chunk.pos.distance(bit.pos);
        let loose = bit.pos.distance(bit.stuck_pos).inverse_lerp(50.0, 10.0);
        let fac = loose * chunk.vel.length().inverse_lerp(1.0, 3.0) * strength;
        let share = chunk.mass / (chunk.mass + 0.01);
        let to_bit = away * ((reach - gap) * fac * share);
        bit.pos += to_bit;
        bit.vel += to_bit;
        bit.vel += chunk.vel * (fac * share * 0.3);
        let to_chunk = away * ((reach - gap) * fac * (1.0 - share));
        chunk.pos -= to_chunk;
        chunk.vel -= to_chunk;
        let drag = chunk.vel * (fac * (1.0 - share) * 0.3);
        chunk.vel -= drag;
        Some(bit.pos.distance(bit.stuck_pos).inverse_lerp(50.0, 10.0) * strength)
    }

    /// The end of `Update`: the next ticks' chance for a link to break follows how many
    /// bits were displaced.
    pub fn finish_update(&mut self) {
        self.disconnect_chance = (self.displaced_counter as f32).inverse_lerp(300.0, 1600.0);
        self.displaced_counter = 0;
    }

    /// `Explosion`: every bit within `rad` of `pos` is thrown (more the nearer); more than
    /// five of them sound `Coral_Circuit_Jump_Explosion` at `pos`.
    pub fn explosion(
        &mut self,
        pos: Vec2,
        rad: f32,
        force: f32,
        dir_vec: Vec2,
        sounds: &mut Sounds,
        rng: &mut UnityRandom,
    ) {
        let mut hit = 0;
        for bit in &mut self.bits {
            if bit.pos.dist_less(pos, rad) {
                let near = pos.distance(bit.pos).inverse_lerp(rad, rad / 2.0);
                bit.vel += (pos.direction_to(bit.pos) * force + dir_vec) * near;
                hit += 1;
            }
        }
        if hit > 5 {
            sounds.play_at(
                SoundId::CoralCircuitJumpExplosion,
                pos,
                (hit as f32).inverse_lerp(4.0, 19.0),
                1.0,
                rng,
            );
        }
    }
}

#[cfg(test)]
#[path = "tests/circuit.rs"]
mod tests;
