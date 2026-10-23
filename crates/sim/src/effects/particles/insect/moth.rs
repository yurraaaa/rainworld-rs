//! `Moth`: flaps about, rests on walls for a while and takes off when something comes
//! close.
//!
//! Mirrors: Moth.cs

use super::{CreatureAvoider, InsectBase, InsectBehavior, InsectCx};
use crate::{FloatExt, Vec2};

/// `Moth`.
#[derive(Debug, Clone, PartialEq)]
pub struct Moth {
    pub sit_pos: Option<Vec2>,
    pub rot: Vec2,
    pub last_rot: Vec2,
    fly_dir: Vec2,
    wall_dir: Vec2,
    pub wings_out: f32,
    pub last_wings_out: f32,
    pub flap: f32,
    pub last_flap: f32,
    sit_still: i32,
}

impl Moth {
    /// The constructor after the base's (three draws): sitting when a wall is within 5 px.
    pub fn new(b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) -> Self {
        b.avoider = Some(CreatureAvoider::new(30, 80.0, 0.1, cx));
        let (sit_pos, wings_out, wall_dir) = InsectBase::sitting_start(pos, cx);
        if b.burrow_pos.is_some()
            && let Some(sit) = sit_pos
        {
            b.burrow_pos = Some(sit);
        }
        let rot = cx.rnv();
        let sit_still = cx.rng.range_i32(20, 90);
        Self {
            sit_pos,
            rot,
            last_rot: rot,
            fly_dir: Vec2::ZERO,
            wall_dir,
            wings_out,
            last_wings_out: wings_out,
            flap: 0.0,
            last_flap: 0.0,
            sit_still,
        }
    }

    /// `Update`.
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.last_rot = self.rot;
        self.last_wings_out = self.wings_out;
        self.last_flap = self.flap;
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

impl InsectBehavior for Moth {
    fn reset(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.reset(pos, cx.rng);
        self.sit_pos = None;
    }

    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
        if b.want_to_burrow {
            self.sit_pos = None;
        }
        if let Some(sit) = self.sit_pos {
            b.pos = sit;
            b.vel *= 0.0;
            self.wings_out = (self.wings_out - 1.0 / 20.0_f32.lerp(40.0, cx.value())).max(0.0);
            if self.flap == 1.0 {
                self.flap = 0.0;
            } else {
                self.flap = (self.flap + 1.0 / (5.0 - 2.0 * self.wings_out)).min(1.0);
            }
            if b.worst_danger_pos()
                .is_some_and(|d| d.dist_less(b.pos, 70.0))
            {
                self.take_off(b, cx);
            } else if self.sit_still > 0 {
                self.sit_still -= 1;
            } else if cx.value() < 1.0 / 60.0 {
                self.take_off(b, cx);
            }
            return;
        }
        b.vel.y += 0.4;
        b.vel *= 0.82;
        let r = cx.rnv();
        self.fly_dir += r * (cx.value() * 0.6);
        if b.want_to_burrow {
            self.fly_dir.y -= 0.5;
        } else if b.submerged {
            self.fly_dir.y += 1.0;
            b.vel.y += 0.3;
        } else if let Some(s) = b.swarm.filter(|_| b.out_of_bounds()) {
            self.fly_dir += b.pos.direction_to(s.center) * (cx.value() * 0.075);
        } else {
            if let Some(danger) = b.worst_danger_pos() {
                let flee = b.flee_speed();
                self.fly_dir += danger.direction_to(b.pos) * (flee * cx.value() * 3.0);
            }
            let (w, h) = (cx.room.pixel_width(), cx.room.pixel_height());
            if b.pos.x < 0.0 {
                self.fly_dir.x += cx.value() * 0.05;
            } else if b.pos.x > w {
                self.fly_dir.x -= cx.value() * 0.05;
            }
            if b.pos.y < 0.0 {
                self.fly_dir.y += cx.value() * 0.05;
            } else if b.pos.y > h {
                self.fly_dir.y -= cx.value() * 0.05;
            }
        }
        self.fly_dir = self.fly_dir.normalized();
        let r = cx.rnv();
        b.vel += self.fly_dir * 0.5 + r * (0.5 * cx.value());
        let to = (-b.vel - self.fly_dir + Vec2::new(0.0, -8.0)).normalized();
        self.rot = self.rot.slerp(to, 0.2);
        self.wings_out = 1.0;
        if self.flap == 1.0 {
            self.flap = 0.0;
            b.vel.y += 2.4;
            self.rot = self.rot.slerp(Vec2::new(0.0, -1.0), 0.5);
        } else {
            self.flap = (self.flap + 1.0 / 3.0).min(1.0);
        }
    }

    fn wall_collision(
        &mut self,
        b: &mut InsectBase,
        dir: (i32, i32),
        _first: bool,
        cx: &mut InsectCx,
    ) {
        if b.want_to_burrow {
            return;
        }
        if self.sit_pos.is_none() && cx.value() < 0.5 {
            self.sit_pos = Some(b.pos);
            (self.wall_dir, self.rot) = b.landing(cx);
            if cx.value() < 0.5 {
                self.sit_still = cx.rng.range_i32(20, 90);
            }
        } else {
            b.vel -= Vec2::new(dir.0 as f32, dir.1 as f32) * cx.value();
        }
    }

    fn emerge_from_ground(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.pos = pos;
        self.sit_pos = Some(pos + Vec2::new(0.0, 4.0));
        self.take_off(b, cx);
    }
}
