//! `RedSwarmer`: a red hovering bug that keeps moving its hover spot, low over the
//! terrain and above water, and always flees what scares it.
//!
//! Mirrors: RedSwarmer.cs

use super::{CreatureAvoider, EIGHT_DIRECTIONS, InsectBase, InsectBehavior, InsectCx};
use crate::physics::shared_physics::ExactRayTrace;
use crate::{FloatExt, Vec2};

/// `RedSwarmer`.
#[derive(Debug, Clone, PartialEq)]
pub struct RedSwarmer {
    pub hover_pos: Vec2,
    pub hover_timer: i32,
    pub dir: Vec2,
    pub last_dir: Vec2,
    pub hue: f32,
}

impl RedSwarmer {
    /// The constructor after the base's (three draws).
    pub fn new(b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) -> Self {
        let hover_timer = cx.rng.range_i32(20, 120);
        b.avoider = Some(CreatureAvoider::new(10, 200.0, 0.2, cx));
        let hue = cx.value();
        Self {
            hover_pos: pos,
            hover_timer,
            dir: Vec2::ZERO,
            last_dir: Vec2::ZERO,
            hue,
        }
    }

    /// `Update`.
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.last_dir = self.dir;
        self.base_update(b, cx);
        b.vel.y -= 0.8;
        b.vel *= 0.87;
        self.dir -= b.vel * 0.3;
        self.dir = self.dir.normalized();
    }

    /// `TileScore`: one draw before the room is ready for AI (past the wall and water
    /// tests).
    fn tile_score(tile: (i32, i32), cx: &mut InsectCx) -> f32 {
        if cx.solid_tile(tile) {
            return f32::MAX;
        }
        if let Some(row) = cx.water.row
            && (tile.1 as f32) < row as f32 + 5.0
        {
            return f32::MAX;
        }
        let Some(map) = cx.ai_map else {
            return cx.value();
        };
        (3.0 - map.terrain_proximity(tile.0, tile.1) as f32).abs()
            + (3.0 - map.floor_altitude(tile.0, tile.1) as f32).abs() * 2.0
    }
}

impl InsectBehavior for RedSwarmer {
    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
        b.vel.y += 0.8;
        b.vel += (self.hover_pos - b.pos).clamp_magnitude(20.0) / 20.0 * 0.5;
        let r = cx.rnv();
        b.vel += r * (cx.value() * 0.8);
        self.dir = self.dir.slerp(Vec2::new(0.0, -1.0), 0.2);
        if b.want_to_burrow {
            let d = Vec2::from_degrees((-20.0_f32).lerp(20.0, cx.value()));
            self.hover_pos -= d * (cx.value() * 30.0);
            return;
        }
        self.hover_timer -= 1;
        if self.hover_timer < 0 {
            let r = cx.rnv();
            let v = b.pos + r * 20.0_f32.lerp(120.0, cx.value());
            if !cx.solid_at(v)
                && cx.inside(v.tile())
                && !cx.water.submerged(v + Vec2::new(0.0, -60.0))
                && cx.room.ray_trace_tiles_for_terrain_exact(b.pos, v)
            {
                self.hover_pos = v;
                self.hover_timer = cx.rng.range_i32(0, 6);
            }
        }
        let (dx, dy) = EIGHT_DIRECTIONS[cx.rng.range_i32(0, 8) as usize];
        let at = self.hover_pos.tile();
        if Self::tile_score((at.0 + dx, at.1 + dy), cx) < Self::tile_score(at, cx) {
            self.hover_pos += Vec2::new(dx as f32, dy as f32).normalized() * (cx.value() * 20.0);
        }
        if let Some(danger) = b.worst_danger_pos() {
            self.hover_pos = b.pos + danger.direction_to(b.pos) * (b.flee_speed() * 200.0);
        }
        if let Some(s) = b.swarm.filter(|_| b.out_of_bounds()) {
            self.hover_pos += self.hover_pos.direction_to(s.center);
        }
        if b.submerged || cx.water.submerged(b.pos + Vec2::new(0.0, -40.0)) {
            b.vel.y += 0.5;
        }
    }

    fn wall_collision(
        &mut self,
        b: &mut InsectBase,
        dir: (i32, i32),
        _first: bool,
        cx: &mut InsectCx,
    ) {
        let r = cx.rnv();
        b.vel -= Vec2::new(dir.0 as f32, dir.1 as f32) * 6.0 + r * 2.0_f32.lerp(5.0, cx.value());
        if let Some(a) = &mut b.avoider {
            a.reset();
        }
    }

    fn emerge_from_ground(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.pos = pos + Vec2::new(0.0, 3.0);
        let d = Vec2::from_degrees((-20.0_f32).lerp(20.0, cx.value()));
        b.vel = d * 2.0_f32.lerp(6.0, cx.value());
        self.hover_pos = pos + b.vel * 2.0;
    }
}
