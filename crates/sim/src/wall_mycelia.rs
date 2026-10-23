//! `WallMycelia`: the room's placed `WallMycelia` objects as one object of hundreds of
//! mycelia rooted on wall surfaces within each object's radius. The constructor is seeded
//! from the first object's place (the shared stream untouched); each tick every strand
//! updates, drifts by its own random movement (one draw) and, unless culled, bends its
//! first points. The sprites are `rw-graphics`' `wall_mycelia_draw.rs`.
//!
//! Mirrors: CoralBrain/WallMycelia.cs (constructor, Update, ConnectionPos, ResetDir)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::float_ext::MathfSin;
use crate::mycelium::{MyceliumEnv, MyceliumId, MyceliumOwner, MyceliumPool, OwnerKey, SystemId};
use crate::room_ext::RoomExt;
use crate::shared_physics::ExactRayTrace;
use crate::{FloatExt, Vec2};

/// `Custom.eightDirections`.
const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// A placed `WallMycelia`: where, and its `ResizableObjectData.Rad`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallPlace {
    pub pos: Vec2,
    pub rad: f32,
}

/// `rootPositions` and `directions`: what the strands ask of their owner.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WallRoots {
    pub root_positions: Vec<Vec2>,
    pub directions: Vec<Vec2>,
}

impl MyceliumOwner for WallRoots {
    fn connection_pos(&self, index: usize) -> Vec2 {
        self.root_positions[index]
    }

    fn reset_dir(&self, index: usize, _rng: &mut UnityRandom) -> Vec2 {
        self.directions[index]
    }
}

/// The `WallMycelia` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct WallMycelia {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub owner: OwnerKey,
    pub places: Vec<WallPlace>,
    pub roots: WallRoots,
    pub random_movements: Vec<Vec2>,
    pub mycelia: Vec<MyceliumId>,
}

impl WallMycelia {
    /// The constructor, seeded by `(int)x + (int)y` of the first place (the shared stream
    /// is restored after): per free tile with a wall beside it within a place's radius,
    /// `(int)LerpMap(...)` strands, each rooted where a ray from a point near the tile
    /// meets the wall, with a length and a direction away from the place; then a
    /// `Mycelium` for each, registered in `room_index`'s system.
    pub fn new(
        update_order: u64,
        places: Vec<WallPlace>,
        room: &Room,
        system: SystemId,
        pool: &mut MyceliumPool,
        rng: &mut UnityRandom,
    ) -> Self {
        let saved = rng.state();
        let first = places[0].pos;
        rng.init_state(first.x.to_int_cs().wrapping_add(first.y.to_int_cs()));
        let (roots, lengths) = Self::find_roots(&places, room, rng);
        let owner = pool.new_owner();
        let random_movements = vec![Vec2::ZERO; roots.root_positions.len()];
        let mut mycelia = Vec::with_capacity(roots.root_positions.len());
        for (n, &length) in lengths.iter().enumerate() {
            let id = pool.add(
                Some(system),
                (owner, &roots),
                n,
                length,
                roots.root_positions[n],
                rng,
            );
            mycelia.push(id);
        }
        rng.set_state(saved);
        Self {
            update_order,
            owner,
            places,
            roots,
            random_movements,
            mycelia,
        }
    }

    /// The constructor's first part: every strand's root, direction and length.
    fn find_roots(
        places: &[WallPlace],
        room: &Room,
        rng: &mut UnityRandom,
    ) -> (WallRoots, Vec<f32>) {
        let mut roots = WallRoots::default();
        let mut lengths = Vec::new();
        for place in places {
            let tile = place.pos.tile();
            let reach = (place.rad / 20.0).to_int_cs();
            for j in tile.0 - reach..=tile.0 + reach {
                for k in tile.1 - reach..=tile.1 + reach {
                    if j <= 0
                        || j >= room.width()
                        || k <= 0
                        || k >= room.height()
                        || room.is_solid_tile(j, k)
                    {
                        continue;
                    }
                    let mut vector = Self::wall_side(room, j, k);
                    if vector.length() <= 0.0 {
                        continue;
                    }
                    let middle = Vec2::middle_of_tile((j, k));
                    let from_b = 1.0f32.lerp(4.0, rng.value());
                    let strands = middle
                        .distance(place.pos)
                        .lerp_map(place.rad / 3.0, place.rad, from_b, rng.value())
                        .to_int_cs();
                    for _ in 0..strands {
                        let dx = (-9.0f32).lerp(9.0, rng.value());
                        let dy = (-9.0f32).lerp(9.0, rng.value());
                        let at = middle + Vec2::new(dx, dy);
                        vector += place.pos.direction_to(at);
                        vector = vector.normalized();
                        if let Some(hit) = room.exact_terrain_ray_trace_pos(at, at - vector * 20.0)
                        {
                            roots.root_positions.push(hit);
                            roots.directions.push(vector);
                            lengths.push(Self::strand_length(place, hit, rng));
                        }
                    }
                }
            }
        }
        (roots, lengths)
    }

    /// The sum of the unit directions toward which tile `(j, k)` is open and the opposite
    /// side is wall.
    fn wall_side(room: &Room, j: i32, k: i32) -> Vec2 {
        let mut vector = Vec2::ZERO;
        for (dx, dy) in EIGHT_DIRECTIONS {
            if !room.is_solid_tile(j + dx, k + dy) && room.is_solid_tile(j - dx, k - dy) {
                vector += Vec2::new(dx as f32, dy as f32).normalized();
            }
        }
        vector
    }

    /// `LerpMap(Distance(root, place), 0, Rad, 20 + LerpMap(Rad, 50, 500, 20, 180, 0.5) *
    /// Pow(value, 0.25), 20)`: one draw.
    fn strand_length(place: &WallPlace, root: Vec2, rng: &mut UnityRandom) -> f32 {
        let reach = place.rad.lerp_map_pow(50.0, 500.0, 20.0, 180.0, 0.5);
        let from_b = 20.0 + reach * rng.value().mathf_pow(0.25);
        root.distance(place.pos)
            .lerp_map(0.0, place.rad, from_b, 20.0)
    }

    /// `Update`: each strand's turn, then its drift (`RNV() * 0.1`, clamped to 1) and, if
    /// not culled, the bend of its first five points along `directions + randomMovements`.
    pub fn update(
        &mut self,
        pool: &mut MyceliumPool,
        env: &MyceliumEnv<'_>,
        rng: &mut UnityRandom,
    ) {
        for i in 0..self.mycelia.len() {
            let id = self.mycelia[i];
            pool.update(id, &self.roots, env, rng);
            let nudge = Vec2::random_unit(rng) * 0.1;
            self.random_movements[i] = (self.random_movements[i] + nudge).clamp_magnitude(1.0);
            let Some(m) = pool.get_mut(id).filter(|m| !m.culled) else {
                continue;
            };
            let bent = m.points.len().min(5);
            for j in 1..bent {
                let num = j as f32 / (bent - 1) as f32;
                m.points[j][2] += (self.roots.directions[i] + self.random_movements[i])
                    * ((num * std::f32::consts::PI).mathf_sin() * 0.75);
            }
        }
    }
}

#[cfg(test)]
#[path = "wall_mycelia_tests.rs"]
mod tests;
