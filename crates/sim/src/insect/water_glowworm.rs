//! `WaterGlowworm`: a segmented worm with a glowing back that swims in deep water,
//! away from scary creatures and from the surface.
//!
//! Mirrors: WaterGlowworm.cs

use super::{CreatureAvoider, InsectBase, InsectBehavior, InsectCx};
use crate::{FloatExt, Vec2};

/// `WaterGlowworm`.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterGlowworm {
    pub stressed: f32,
    pub rot: Vec2,
    pub last_rot: Vec2,
    swim_dir: Vec2,
    pub breath: f32,
    pub last_breath: f32,
    /// Per segment: position, last position, velocity (`segments[i, 0..3]`).
    pub segments: Vec<[Vec2; 3]>,
}

impl WaterGlowworm {
    /// The constructor after the base's: the avoider, `breath`, the segment count, then
    /// `Reset(pos)`.
    pub fn new(b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) -> Self {
        b.avoider = Some(CreatureAvoider::new(10, 300.0, 0.3, cx));
        let breath = cx.value();
        let n = cx.rng.range_i32(2, 4) as usize;
        let mut worm = Self {
            stressed: 0.0,
            rot: Vec2::ZERO,
            last_rot: Vec2::ZERO,
            swim_dir: Vec2::ZERO,
            breath,
            last_breath: 0.0,
            segments: vec![[Vec2::ZERO; 3]; n],
        };
        worm.reset(b, pos, cx);
        worm
    }

    /// `Update` (`PointDeferred` is false without More Slugcats).
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.last_rot = self.rot;
        self.last_breath = self.breath;
        self.base_update(b, cx);
        if b.submerged {
            b.vel *= 0.8;
        } else {
            b.vel.y -= 0.9;
        }
        for i in 0..self.segments.len() {
            self.segments[i][1] = self.segments[i][0];
            let v = self.segments[i][2];
            self.segments[i][0] += v;
            if cx.water.submerged(self.segments[i][0]) {
                self.segments[i][2] *= 0.8;
            } else {
                self.segments[i][2].y -= 0.9;
            }
            let pull = 0.8.lerp(1.2, self.stressed);
            if i == 0 {
                let dir = self.segments[i][0].direction_to(b.pos);
                let dist = self.segments[i][0].distance(b.pos);
                let push = dir * ((4.0 - dist) * 0.5);
                b.pos += push;
                b.vel += push;
                self.segments[i][0] -= push;
                self.segments[i][2] -= push;
                b.vel += dir * pull;
            } else {
                let dir = self.segments[i][0].direction_to(self.segments[i - 1][0]);
                let dist = self.segments[i][0].distance(self.segments[i - 1][0]);
                let push = dir * ((4.0 - dist) * 0.5);
                self.segments[i - 1][0] += push;
                self.segments[i - 1][2] += push;
                self.segments[i][0] -= push;
                self.segments[i][2] -= push;
                self.segments[i - 1][2] += dir * pull;
            }
        }
    }
}

impl InsectBehavior for WaterGlowworm {
    /// The base reset, then each segment around `pos` (three draws a segment).
    fn reset(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.reset(pos, cx.rng);
        for s in &mut self.segments {
            s[0] = pos + cx.rnv();
            s[1] = pos;
            let dir = cx.rnv();
            s[2] = dir * cx.value();
        }
    }

    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
        self.breath -= 1.0 / 60.0.lerp(10.0, self.stressed);
        let fear = b.flee_speed().mathf_pow(0.3);
        self.stressed = if fear > self.stressed {
            self.stressed.lerp_and_tick(fear, 0.05, 1.0 / 60.0)
        } else {
            self.stressed.lerp_and_tick(fear, 0.02, 0.005)
        };
        if b.submerged {
            let dir = cx.rnv();
            self.swim_dir += dir * (cx.value() * 0.5);
            if b.want_to_burrow {
                self.swim_dir.y -= 0.5;
            }
            if b.pos.x < 0.0 {
                self.swim_dir.x += 1.0;
            } else if b.pos.x > cx.room.pixel_width() {
                self.swim_dir.x -= 1.0;
            }
            if b.pos.y < 0.0 {
                self.swim_dir.y += 1.0;
            }
            if let Some(danger) = b.worst_danger_pos() {
                self.swim_dir -= b.pos.direction_to(danger) * b.flee_speed();
            }
            if cx.water.present() {
                let level = cx.water.level(b.pos);
                let t = b.pos.y.inverse_lerp(level - 100.0, level) * 0.5;
                self.swim_dir = self.swim_dir.slerp(Vec2::new(0.0, -1.0), t);
            }
            self.swim_dir = self.swim_dir.normalized();
            let dir = cx.rnv();
            b.vel += self.swim_dir * 0.8.lerp(1.1, self.stressed) + dir * (cx.value() * 0.1);
        }
        self.rot = self.rot.slerp((-b.vel - self.swim_dir).normalized(), 0.2);
    }

    fn wall_collision(
        &mut self,
        _b: &mut InsectBase,
        dir: (i32, i32),
        _first: bool,
        cx: &mut InsectCx,
    ) {
        let r = cx.rnv();
        self.swim_dir -= r * cx.value() + Vec2::new(dir.0 as f32, dir.1 as f32);
        self.swim_dir = self.swim_dir.normalized();
    }

    fn emerge_from_ground(&mut self, b: &mut InsectBase, pos: Vec2, _cx: &mut InsectCx) {
        b.pos = pos;
        self.swim_dir = Vec2::new(0.0, 1.0);
    }
}
