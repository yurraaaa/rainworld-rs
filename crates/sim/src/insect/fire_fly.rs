//! `FireFly`: a yellow dash that wanders like a mini fly and, where the room is dark,
//! carries a pulsing `LightSource`.
//!
//! Mirrors: FireFly.cs

use super::{FOUR_DIRECTIONS, InsectBase, InsectBehavior, InsectCx};
use crate::{FloatExt, Vec2};

/// What a firefly's update asks of its light (`room.AddObject(light)`, its setters,
/// `light.Destroy()`); the game applies it before the next object updates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FireFlyLight {
    /// Make the light at `pos` (then set it as [`FireFlyLight::Set`]).
    Create {
        pos: Vec2,
        alpha: f32,
        rad: f32,
    },
    Set {
        pos: Vec2,
        alpha: f32,
        rad: f32,
    },
    Destroy,
}

/// `FireFly`.
#[derive(Debug, Clone, PartialEq)]
pub struct FireFly {
    dir: Vec2,
    pub last_last_pos: Vec2,
    /// The id of its `LightSource`, while it has one.
    pub light: Option<u64>,
    /// `col`.
    pub color: [f32; 3],
    pub sin: f32,
    /// What this update asked of the light.
    pub light_step: Option<FireFlyLight>,
}

impl FireFly {
    /// The constructor after the base's (two draws).
    pub fn new(pos: Vec2, cx: &mut InsectCx) -> Self {
        let g = 0.6.lerp(0.8, cx.value());
        let sin = cx.value();
        Self {
            dir: Vec2::ZERO,
            last_last_pos: pos,
            light: None,
            color: [1.0, g, 0.0],
            sin,
            light_step: None,
        }
    }

    /// `Update`.
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.light_step = None;
        b.vel *= 0.95;
        b.vel.x += self.dir.x * 0.3;
        b.vel.y += self.dir.y * 0.2;
        let to = Vec2::from_degrees(cx.value() * 360.0) * cx.value().mathf_pow(0.75);
        self.dir = self.dir.lerp(to, 0.4).normalized();
        if b.want_to_burrow {
            self.dir = self.dir.lerp(Vec2::new(0.0, -1.0), 0.1);
        } else if let Some(s) = b.swarm.filter(|_| b.out_of_bounds()) {
            let t = b.pos.distance(s.center).inverse_lerp(s.rad, s.rad + 100.0);
            self.dir = self.dir.lerp(b.pos.direction_to(s.center), t);
        }
        let tile = b.pos.tile();
        let mut best = Self::tile_score(tile, cx);
        let mut step = (0, 0);
        for (dx, dy) in FOUR_DIRECTIONS {
            let far = (tile.0 + dx * 3, tile.1 + dy * 3);
            if !cx.solid_tile((tile.0 + dx, tile.1 + dy)) && Self::tile_score(far, cx) > best {
                best = Self::tile_score(far, cx);
                step = (dx, dy);
            }
        }
        b.vel += Vec2::new(step.0 as f32, step.1 as f32) * 0.4;
        if cx.water.submerged(b.pos) {
            b.pos.y = cx.water.level(b.pos);
        }
        self.sin += 1.0 / 20.0.lerp(80.0, cx.value());
        if cx.darkness > 0.0 {
            let wave = f64::from(self.sin * std::f32::consts::PI * 2.0).sin() as f32;
            let (alpha, rad) = (0.15 - 0.1 * wave, 60.0 + 20.0 * wave);
            self.light_step = Some(if self.light.is_none() {
                FireFlyLight::Create {
                    pos: b.pos,
                    alpha,
                    rad,
                }
            } else {
                FireFlyLight::Set {
                    pos: b.pos,
                    alpha,
                    rad,
                }
            });
        } else if self.light.is_some() {
            self.light_step = Some(FireFlyLight::Destroy);
        }
        self.last_last_pos = b.last_pos;
        self.base_update(b, cx);
        if !cx.viewed {
            b.destroy();
        }
    }

    /// `TileScore`: one draw while `readyForAI` and the tile is in the room.
    fn tile_score(tile: (i32, i32), cx: &mut InsectCx) -> f32 {
        let Some(map) = cx.ai_map else {
            return 0.0;
        };
        if !cx.inside(tile) {
            return 0.0;
        }
        let v = cx.rng.value();
        v / (map.floor_altitude(tile.0, tile.1) - 4).abs() as f32
            / (map.terrain_proximity(tile.0, tile.1) - 4).abs() as f32
    }
}

impl InsectBehavior for FireFly {
    fn reset(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.reset(pos, cx.rng);
        self.last_last_pos = pos;
    }

    fn emerge_from_ground(&mut self, _b: &mut InsectBase, _pos: Vec2, _cx: &mut InsectCx) {
        self.dir = Vec2::new(0.0, 1.0);
    }
}
