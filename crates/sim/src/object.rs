//! A physical object: body chunks joined by distance constraints (`PhysicalObject`).
//!
//! Behaviour: `rainworld-re/notes/physics-core.md` sections 1-2.
//!
//! Mirrors: PhysicalObject.cs

use rw_assets::Room;

use crate::{BodyChunk, Contact, PhysicsParams, TerrainImpact, Vec2, Water};

/// The owner of a [`PhysicalObject`]: receives terrain impacts during physics
/// (`PhysicalObject.TerrainImpact`, overridden by creatures).
pub trait TerrainImpactHandler {
    /// Called for each impact mid-collision, with all of the object's chunks.
    fn terrain_impact(&mut self, chunks: &mut [BodyChunk], impact: TerrainImpact);
}

/// No owner: impacts are only recorded.
impl TerrainImpactHandler for () {
    fn terrain_impact(&mut self, _chunks: &mut [BodyChunk], _impact: TerrainImpact) {}
}

/// When a [`BodyChunkConnection`] acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConnectionKind {
    /// Always: a rod.
    #[default]
    Normal,
    /// Only when the chunks are farther apart than `distance`: a rope.
    Pull,
    /// Only when they are closer than `distance`.
    Push,
}

/// Distance constraint between two chunks of the same object (`BodyChunkConnection`).
///
/// Each tick it moves both chunks along their axis by the length error times
/// `elasticity`, split `weight_symmetry` / `1 - weight_symmetry`, and adds the same
/// correction to their velocities.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyChunkConnection {
    /// Index of the first chunk in the owner's chunk list.
    pub chunk1: usize,
    /// Index of the second chunk.
    pub chunk2: usize,
    pub distance: f32,
    pub kind: ConnectionKind,
    pub elasticity: f32,
    /// Share of the correction applied to `chunk1`.
    pub weight_symmetry: f32,
    pub active: bool,
}

impl BodyChunkConnection {
    /// An active connection.
    #[must_use]
    pub fn new(
        chunk1: usize,
        chunk2: usize,
        distance: f32,
        kind: ConnectionKind,
        elasticity: f32,
        weight_symmetry: f32,
    ) -> Self {
        Self {
            chunk1,
            chunk2,
            distance,
            kind,
            elasticity,
            weight_symmetry,
            active: true,
        }
    }

    /// Applies the constraint once.
    ///
    /// # Panics
    /// If either chunk index is out of range for `chunks`.
    pub fn apply(&self, chunks: &mut [BodyChunk]) {
        if !self.active {
            return;
        }
        let (p1, p2) = (chunks[self.chunk1].pos, chunks[self.chunk2].pos);
        let dist = p1.distance(p2);
        let acts = match self.kind {
            ConnectionKind::Normal => true,
            ConnectionKind::Pull => dist > self.distance,
            ConnectionKind::Push => dist < self.distance,
        };
        if !acts {
            return;
        }
        let dir = p1.direction_to(p2);
        let err = self.distance - dist;
        let d1 = dir * (err * self.weight_symmetry * self.elasticity);
        let d2 = dir * (err * (1.0 - self.weight_symmetry) * self.elasticity);
        let c1 = &mut chunks[self.chunk1];
        c1.pos -= d1;
        c1.vel -= d1;
        let c2 = &mut chunks[self.chunk2];
        c2.pos += d2;
        c2.vel += d2;
    }
}

/// Body chunks, their connections and the shared physics scalars.
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalObject {
    pub chunks: Vec<BodyChunk>,
    pub connections: Vec<BodyChunkConnection>,
    pub params: PhysicsParams,
    /// `grabbedBy.Count > 0`: a held object's chunks may leave the room's range.
    pub held: bool,
    impacts: Vec<TerrainImpact>,
}

/// Slugcat (Survivor) values from the `Player` constructor.
pub mod slugcat {
    use crate::PhysicsParams;

    /// Head (chunk 0) radius.
    pub const HEAD_RAD: f32 = 9.0;
    /// Hips (chunk 1) radius.
    pub const HIPS_RAD: f32 = 8.0;
    /// Each chunk's mass: `0.7 * bodyWeightFac / 2` with `bodyWeightFac = 1`.
    pub const CHUNK_MASS: f32 = 0.7 / 2.0;
    /// Rod length between head and hips (adult, not rolling).
    pub const ROD_LENGTH: f32 = 17.0;
    /// Player physics scalars (conscious).
    pub const PARAMS: PhysicsParams = PhysicsParams {
        gravity: 0.9,
        air_friction: 0.999,
        bounce: 0.1,
        surface_friction: 0.5,
        impact_threshold: 1.0,
        buoyancy: 0.95,
        water_friction: 0.96,
        water_retardation_immunity: 0.0,
    };
}

impl PhysicalObject {
    /// An object with no connections.
    #[must_use]
    pub fn new(chunks: Vec<BodyChunk>, params: PhysicsParams) -> Self {
        Self {
            chunks,
            connections: Vec::new(),
            params,
            held: false,
            impacts: Vec::new(),
        }
    }

    /// The slugcat body: head (r 9) and hips (r 8) at the origin, joined by a rigid
    /// rod of 17 (Normal, elasticity 1, symmetry 0.5), with the player's scalars.
    /// Place it with [`crate::place_in_room`].
    #[must_use]
    pub fn slugcat() -> Self {
        use slugcat::{CHUNK_MASS, HEAD_RAD, HIPS_RAD, PARAMS, ROD_LENGTH};
        Self {
            chunks: vec![
                BodyChunk::new(Vec2::ZERO, HEAD_RAD, CHUNK_MASS),
                BodyChunk::new(Vec2::ZERO, HIPS_RAD, CHUNK_MASS),
            ],
            connections: vec![BodyChunkConnection::new(
                0,
                1,
                ROD_LENGTH,
                ConnectionKind::Normal,
                1.0,
                0.5,
            )],
            params: PARAMS,
            held: false,
            impacts: Vec::new(),
        }
    }

    /// `PhysicalObject.Submersion`: the chunks' mean submersion.
    #[must_use]
    pub fn submersion(&self, water: Option<&Water>) -> f32 {
        let sum: f32 = self.chunks.iter().map(|c| c.submersion(water)).sum();
        sum / self.chunks.len() as f32
    }

    /// Mutable references to two different chunks.
    ///
    /// # Panics
    /// If `a == b` or either index is out of range.
    pub fn chunk_pair_mut(&mut self, a: usize, b: usize) -> [&mut BodyChunk; 2] {
        self.chunks
            .get_disjoint_mut([a, b])
            .expect("two distinct chunk indices in range")
    }

    /// Sets `go_through_floors` on every chunk (the game's `GoThroughFloors` setter).
    pub fn set_go_through_floors(&mut self, value: bool) {
        for c in &mut self.chunks {
            c.go_through_floors = value;
        }
    }

    /// One physics tick in a room without water: every chunk in index order, then
    /// every connection. Terrain impacts are only recorded ([`PhysicalObject::impacts`]).
    ///
    /// Chunk `i` sees the already-updated `last_pos` of chunks before it and the
    /// previous tick's of chunks after it, as in the game.
    pub fn update(&mut self, room: &Room) {
        self.update_with(room, None, &mut ());
    }

    /// [`PhysicalObject::update`] with an owner that receives each terrain impact at the
    /// moment the game calls `TerrainImpact`: inside the chunk's collision, after the
    /// position snap and before the bounce, so changes it makes to any chunk act on the
    /// rest of the tick's physics. `water` is the room's water (buoyancy and drag).
    pub fn update_with(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        owner: &mut dyn TerrainImpactHandler,
    ) {
        self.update_chunks(room, water, owner);
        self.update_connections();
    }

    /// The chunk half of [`PhysicalObject::update_with`] (`PhysicalObject.Update` sets
    /// the abstract tile between the two halves).
    pub fn update_chunks(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        owner: &mut dyn TerrainImpactHandler,
    ) {
        self.impacts.clear();
        for i in 0..self.chunks.len() {
            self.chunks[i].integrate(&self.params, water);
            if self.chunks[i].collide_with_terrain {
                let lowest = self.lowest_last_y(room.pixel_height());
                if let Some(side) = self.chunks[i].sweep_vertical(room, lowest) {
                    self.hit(i, side, owner);
                }
                if self.chunks[i].collide_with_slopes {
                    self.chunks[i].collide_slopes(room, &self.params);
                }
                if let Some(side) = self.chunks[i].sweep_horizontal(room) {
                    self.hit(i, side, owner);
                }
            } else {
                self.chunks[i].clear_contact();
            }
            if !self.held {
                self.chunks[i].restrict_to_room(room);
            }
            self.chunks[i].count_splash_stop(water);
        }
    }

    /// Every connection in order (`BodyChunkConnection.Update`).
    pub fn update_connections(&mut self) {
        for c in &self.connections {
            c.apply(&mut self.chunks);
        }
    }

    /// Chunk `i` hit the surface on `side`: report the impact, then bounce.
    fn hit(&mut self, i: usize, side: Contact, owner: &mut dyn TerrainImpactHandler) {
        if let Some(impact) = self.chunks[i].impact(side, &self.params, i) {
            self.impacts.push(impact);
            owner.terrain_impact(&mut self.chunks, impact);
        }
        self.chunks[i].bounce_off(side, &self.params);
    }

    /// Terrain impacts of the last update, in the order the game calls `TerrainImpact`
    /// (chunk index, then ceiling/floor before walls).
    #[must_use]
    pub fn impacts(&self) -> &[TerrainImpact] {
        &self.impacts
    }

    /// Lowest `last_pos.y` of all chunks, but no higher than `ceiling`.
    fn lowest_last_y(&self, ceiling: f32) -> f32 {
        self.chunks
            .iter()
            .map(|c| c.last_pos.y)
            .fold(ceiling, |low, y| if y < low { y } else { low })
    }
}
