//! `Beetle`: sits on walls with its wings folded and flies off when stressed or by
//! chance, landing on the next wall it hits.
//!
//! Mirrors: Beetle.cs

use super::{
    CreatureAvoider, EIGHT_DIRECTIONS, FOUR_DIRECTIONS, InsectBase, InsectBehavior, InsectCx,
};
use crate::{FloatExt, Vec2};

/// `Beetle`.
#[derive(Debug, Clone, PartialEq)]
pub struct Beetle {
    pub sit_pos: Option<Vec2>,
    pub stressed: f32,
    pub rot: Vec2,
    pub last_rot: Vec2,
    wall_dir: Vec2,
    fly_dir: Vec2,
    pub wings_out: f32,
    pub last_wings_out: f32,
}

impl Beetle {
    /// The constructor after the base's (two draws): sitting when a wall is within 5 px.
    pub fn new(b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) -> Self {
        b.avoider = Some(CreatureAvoider::new(20, 300.0, 0.3, cx));
        let (sit_pos, wings_out, wall_dir) = InsectBase::sitting_start(pos, cx);
        if b.burrow_pos.is_some()
            && let Some(sit) = sit_pos
        {
            b.burrow_pos = Some(sit);
        }
        let rot = cx.rnv();
        Self {
            sit_pos,
            stressed: 0.0,
            rot,
            last_rot: rot,
            wall_dir,
            fly_dir: Vec2::ZERO,
            wings_out,
            last_wings_out: wings_out,
        }
    }

    /// `Update`.
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.last_rot = self.rot;
        self.last_wings_out = self.wings_out;
        self.base_update(b, cx);
        if self.sit_pos.is_none() {
            b.vel.y -= 0.8;
        }
        if b.submerged {
            b.vel *= 0.8;
            let r = cx.rnv();
            self.rot = self.rot.slerp(r, 0.5);
        }
    }

    /// `TakeOff`.
    fn take_off(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        let Some(sit) = self.sit_pos else {
            return;
        };
        if b.burrow_pos.is_some() {
            return;
        }
        let r = cx.rnv();
        self.fly_dir = (-self.wall_dir + r * cx.value()).normalized();
        b.pos = sit + self.fly_dir * 2.0;
        b.vel = self.fly_dir * 6.0;
        self.sit_pos = None;
    }
}

impl InsectBase {
    /// The constructors of `Beetle` and `Moth`: sitting at `pos`, wings folded, when one of
    /// the four tiles 5 px away is Solid; else flying with wings out.
    pub(super) fn sitting_start(pos: Vec2, cx: &InsectCx) -> (Option<Vec2>, f32, Vec2) {
        for (dx, dy) in FOUR_DIRECTIONS {
            let d = Vec2::new(dx as f32, dy as f32);
            if cx.solid_at(pos + d * 5.0) {
                return (Some(pos), 0.0, -d);
            }
        }
        (None, 1.0, Vec2::ZERO)
    }

    /// `WallCollision`'s landing of `Beetle`, `Wasp` and `Moth`: away from the Solid tiles
    /// around (one draw), then the body along the wall (three draws).
    pub(super) fn landing(&self, cx: &mut InsectCx) -> (Vec2, Vec2) {
        let pos = self.pos;
        let mut wall_dir = cx.rnv() * 0.1;
        for (dx, dy) in EIGHT_DIRECTIONS {
            let d = Vec2::new(dx as f32, dy as f32);
            if cx.solid_at(pos + d * 20.0) {
                wall_dir += d.normalized();
            }
        }
        wall_dir = wall_dir.normalized();
        let side = if cx.value() < 0.5 { -1.0 } else { 1.0 };
        let rot = (wall_dir.perpendicular() * side + cx.rnv() * 0.2).normalized();
        (wall_dir, rot)
    }
}

impl InsectBehavior for Beetle {
    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
        if b.want_to_burrow {
            self.sit_pos = None;
        }
        let fear = b.flee_speed().mathf_pow(0.3);
        self.stressed = if fear > self.stressed {
            self.stressed.lerp_and_tick(fear, 0.05, 1.0 / 60.0)
        } else {
            self.stressed.lerp_and_tick(fear, 0.02, 0.005)
        };
        if let Some(sit) = self.sit_pos {
            b.pos = sit;
            b.vel *= 0.0;
            self.wings_out = (self.wings_out - 1.0 / 10.0_f32.lerp(200.0, cx.value())).max(0.0);
            let chance = 1.0 / 400.0_f32.lerp(40.0, self.stressed);
            let close = b
                .worst_danger_pos()
                .is_some_and(|d| b.pos.dist_less(d, 30.0));
            if cx.value() < chance || close {
                self.take_off(b, cx);
            }
            return;
        }
        b.vel.y += 0.7;
        b.vel *= 0.8;
        let r = cx.rnv();
        self.fly_dir += r * (cx.value() * 0.3);
        if let Some(s) = b.swarm.filter(|_| b.out_of_bounds()) {
            self.fly_dir += b.pos.direction_to(s.center) * (cx.value() * 0.1);
        } else if b.swarm.is_none() {
            let (w, h) = (cx.room.pixel_width(), cx.room.pixel_height());
            if b.pos.x < 0.0 {
                self.fly_dir.x += 0.05 * cx.value();
            } else if b.pos.x > w {
                self.fly_dir.x -= 0.05 * cx.value();
            }
            if b.pos.y < 0.0 {
                self.fly_dir.y += 0.05 * cx.value();
            } else if b.pos.y > h {
                self.fly_dir.y -= 0.05 * cx.value();
            }
        }
        if b.want_to_burrow {
            self.fly_dir.y -= 0.5;
        } else if b.submerged {
            self.fly_dir.y += 1.0;
            b.vel.y += 0.3;
        }
        self.fly_dir = self.fly_dir.normalized();
        b.vel += self.fly_dir * 0.9 + cx.rnv() * 0.3;
        let to = (-b.vel - self.fly_dir + Vec2::new(0.0, -8.0)).normalized();
        self.rot = self.rot.slerp(to, 0.2);
        self.wings_out = 1.0;
    }

    fn wall_collision(
        &mut self,
        b: &mut InsectBase,
        _dir: (i32, i32),
        _first: bool,
        cx: &mut InsectCx,
    ) {
        if self.sit_pos.is_some() || b.want_to_burrow {
            return;
        }
        self.sit_pos = Some(b.pos);
        (self.wall_dir, self.rot) = b.landing(cx);
    }

    fn emerge_from_ground(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.pos = pos;
        self.sit_pos = Some(pos + Vec2::new(0.0, 4.0));
        self.take_off(b, cx);
    }
}
