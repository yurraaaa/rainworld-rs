//! `Wasp`: flies in long sweeps away from terrain, rests on walls it hits and takes off
//! again by chance.
//!
//! Mirrors: Wasp.cs

use super::{InsectBase, InsectBehavior, InsectCx};
use crate::{FloatExt, Vec2};

/// `Wasp`.
#[derive(Debug, Clone, PartialEq)]
pub struct Wasp {
    pub sit_pos: Option<Vec2>,
    pub rot: Vec2,
    pub last_rot: Vec2,
    fly_dir: Vec2,
    wall_dir: Vec2,
    big_fly_dir: Vec2,
    pub wings_out: f32,
    pub last_wings_out: f32,
    pub color_fac: f32,
}

impl Wasp {
    /// The constructor after the base's (two draws).
    pub fn new(cx: &mut InsectCx) -> Self {
        let color_fac = cx.value();
        let big_fly_dir = cx.rnv();
        Self {
            sit_pos: None,
            rot: Vec2::ZERO,
            last_rot: Vec2::ZERO,
            fly_dir: Vec2::ZERO,
            wall_dir: Vec2::ZERO,
            big_fly_dir,
            wings_out: 0.0,
            last_wings_out: 0.0,
            color_fac,
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
        self.big_fly_dir = cx.rnv();
    }

    /// The pull toward open air: each tile of an 11 by 7 box around the spot 40 px
    /// ahead whose terrain proximity (up to 5) beats the spot's adds a draw.
    fn avoid_terrain(&mut self, b: &InsectBase, cx: &mut InsectCx) {
        let (tx, ty) = (b.pos + self.fly_dir * 40.0).tile();
        let here = cx.terrain_proximity((tx, ty)).min(5);
        for i in -5..=5 {
            for j in -3..=3 {
                if (i != 0 || j != 0) && cx.terrain_proximity((tx + i, ty + j)).min(5) > here {
                    let d = Vec2::new(i as f32, j as f32);
                    self.fly_dir +=
                        d.normalized() * (0.48 * cx.value()) / d.length().mathf_pow(1.4);
                }
            }
        }
    }
}

impl InsectBehavior for Wasp {
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
            self.wings_out = (self.wings_out - 1.0 / 10.0_f32.lerp(200.0, cx.value())).max(0.0);
            if cx.value() < 1.0 / 120.0 {
                self.take_off(b, cx);
            }
            return;
        }
        b.vel.y += 0.7;
        b.vel *= 0.8;
        let r = cx.rnv();
        self.big_fly_dir += r * (cx.value() * 0.1);
        self.big_fly_dir = self.big_fly_dir.normalized();
        let r = cx.rnv();
        self.fly_dir += r * (cx.value() * 0.6);
        self.fly_dir += self.big_fly_dir * (2.0 * cx.value());
        if b.want_to_burrow {
            self.fly_dir.y -= 0.5;
        } else {
            if b.submerged {
                self.fly_dir.y += 1.0;
                b.vel.y += 0.3;
            } else if cx.ready_for_ai() {
                self.avoid_terrain(b, cx);
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
        b.vel += self.fly_dir * 1.8;
        self.big_fly_dir += b.vel * 0.02;
        let to = (-b.vel - self.fly_dir + Vec2::new(0.0, -1.0)).normalized();
        self.rot = self.rot.slerp(to, 0.3);
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
