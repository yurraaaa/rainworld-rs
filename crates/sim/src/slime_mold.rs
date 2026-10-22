//! `SlimeMold.CosmeticSlimeMold`: a patch of slime-mold dots on the walls around a placed
//! point, made by `Room.Loaded` for `CosmeticSlimeMold`, `CosmeticSlimeMold2` (through
//! walls) and, past `SeededRandom`'s gate, the plain `SlimeMold` placed object. The dots
//! are fixed at construction, whose draws run under `InitState` with the global state
//! restored (no shared-stream draws, D1); `Update` does nothing. Sprites:
//! `rw-graphics` `slime_mold.rs`.
//!
//! Mirrors: SlimeMold.cs (CosmeticSlimeMold), Room.cs (Loaded: CosmeticSlimeMold, CosmeticSlimeMold2, SlimeMold)

use rw_assets::Room;
use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_random::UnityRandom;

use crate::shared_physics::ExactRayTrace;
use crate::{FloatExt, RoomExt, Vec2};

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

/// One `CosmeticSlimeMold`.
#[derive(Debug, Clone, PartialEq)]
pub struct CosmeticSlimeMold {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    pub positions: Vec<Vec2>,
    pub rads: Vec<f32>,
    pub brightnesses: Vec<f32>,
}

impl CosmeticSlimeMold {
    /// The constructor: dots on the wall faces within `rad / 20` tiles of `pos`, drawn
    /// from `InitState((int)(pos.x + pos.y + rad))`, `rng`'s state restored.
    #[must_use]
    pub fn new(
        update_order: u64,
        room: &Room,
        pos: Vec2,
        rad: f32,
        through_walls: bool,
        rng: &mut UnityRandom,
    ) -> Self {
        let saved = rng.state();
        rng.init_state((pos.x + pos.y + rad).to_int_cs());
        let mold = Self::generate(update_order, room, pos, rad, through_walls, rng);
        rng.set_state(saved);
        mold
    }

    /// The constructor's dots, drawn from `rng` as it stands (the seeded stream).
    pub(crate) fn generate(
        update_order: u64,
        room: &Room,
        pos: Vec2,
        rad: f32,
        through_walls: bool,
        rng: &mut UnityRandom,
    ) -> Self {
        let mut mold = Self {
            update_order,
            positions: Vec::new(),
            rads: Vec::new(),
            brightnesses: Vec::new(),
        };
        let centre = pos.tile();
        let reach = (rad / 20.0).to_int_cs();
        for i in centre.0 - reach..=centre.0 + reach {
            for j in centre.1 - reach..=centre.1 + reach {
                if i <= 0 || i >= room.width() || j <= 0 || j >= room.height() {
                    continue;
                }
                if room.is_solid_tile(i, j) {
                    continue;
                }
                if through_walls {
                    mold.through_walls_tile(room, pos, rad, (i, j), rng);
                } else {
                    mold.open_tile(room, pos, rad, centre, (i, j), rng);
                }
            }
        }
        mold
    }

    /// `CosmeticSlimeMold2`: dots in the face of each solid neighbour (four directions),
    /// pushed out of a tile walled in on both sides.
    fn through_walls_tile(
        &mut self,
        room: &Room,
        pos: Vec2,
        rad: f32,
        (i, j): (i32, i32),
        rng: &mut UnityRandom,
    ) {
        let middle = Vec2::middle_of_tile((i, j));
        for (dx, dy) in FOUR_DIRECTIONS {
            if !room.is_solid_tile(i + dx, j + dy) {
                continue;
            }
            let opposite_solid = room.is_solid_tile(i - dx, j - dy);
            let most = if opposite_solid {
                6.0_f32.lerp(8.0, rng.value())
            } else {
                4.0_f32.lerp(7.0, rng.value())
            };
            let count = middle
                .distance(pos)
                .lerp_map(rad / 3.0, rad, most, 0.0)
                .to_int_cs();
            for _ in 0..count {
                let mut dot = middle + Vec2::new(dx as f32, dy as f32) * 10.0;
                if dx != 0 {
                    dot.y += (-9.0_f32).lerp(9.0, rng.value());
                } else {
                    dot.x += (-9.0_f32).lerp(9.0, rng.value());
                }
                let dot_rad = self.add_dot(dot, pos, rad, rng);
                if opposite_solid {
                    let wall = Vec2::middle_of_tile((i + dx, j + dy));
                    let last = self.positions.len() - 1;
                    self.positions[last] += wall.direction_to(dot) * (dot_rad - 1.0).max(0.0);
                }
            }
        }
    }

    /// `CosmeticSlimeMold`: dots ray-traced 25 px onto the wall of an open tile with a
    /// solid tile opposite an open one, seen from the centre tile.
    fn open_tile(
        &mut self,
        room: &Room,
        pos: Vec2,
        rad: f32,
        centre: (i32, i32),
        (i, j): (i32, i32),
        rng: &mut UnityRandom,
    ) {
        let mut wall_dir = Vec2::ZERO;
        for (dx, dy) in EIGHT_DIRECTIONS {
            if !room.is_solid_tile(i + dx, j + dy) && room.is_solid_tile(i - dx, j - dy) {
                wall_dir += Vec2::new(dx as f32, dy as f32).normalized();
            }
        }
        if wall_dir.length() <= 0.0 || !room.ray_trace_tiles_for_terrain(centre, (i, j)) {
            return;
        }
        let middle = Vec2::middle_of_tile((i, j));
        let count = middle
            .distance(pos)
            .lerp_map(rad / 3.0, rad, 2.0_f32.lerp(4.0, rng.value()), 0.0)
            .to_int_cs();
        let wall_dir = wall_dir.normalized();
        for _ in 0..count {
            let x = (-9.0_f32).lerp(9.0, rng.value());
            let y = (-9.0_f32).lerp(9.0, rng.value());
            let from = middle + Vec2::new(x, y);
            if let Some(hit) = room.exact_terrain_ray_trace_pos(from, from - wall_dir * 25.0) {
                self.add_dot(hit, pos, rad, rng);
            }
        }
    }

    /// A dot at `at`: its radius (both of `LerpMap`'s ends drawn) and brightness;
    /// returns the radius.
    fn add_dot(&mut self, at: Vec2, pos: Vec2, rad: f32, rng: &mut UnityRandom) -> f32 {
        let near = 1.5 + 3.0 * rng.value();
        let far = 0.75 + 0.25 * rng.value();
        let dot_rad = at.distance(pos).lerp_map(rad / 3.0, rad, near, far);
        let top = 2.0_f32.lerp(4.0, rng.value().mathf_pow(1.5));
        self.positions.push(at);
        self.rads.push(dot_rad);
        self.brightnesses.push(dot_rad.inverse_lerp(0.75, top));
        dot_rad
    }

    /// `Room.Loaded`'s gate for a plain `SlimeMold` placed object:
    /// `game.SeededRandom((int)(pos.x + pos.y))` above 0.3 makes a patch of radius
    /// `LerpMap(r, 0.3, 1, 30, 70)`; `None` otherwise. No shared-stream draws.
    #[must_use]
    pub fn placed_item_rad(pos: Vec2, rng: &mut UnityRandom) -> Option<f32> {
        let saved = rng.state();
        rng.init_state((pos.x + pos.y).to_int_cs());
        let r = rng.value();
        rng.set_state(saved);
        (r > 0.3).then(|| r.lerp_map(0.3, 1.0, 30.0, 70.0))
    }
}
