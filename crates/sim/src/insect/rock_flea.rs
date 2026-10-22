//! `RockFlea`: a flea that sits on walls and jumps off them, faster when a creature
//! scares it, and swims under water.
//!
//! Mirrors: RockFlea.cs

use super::{CreatureAvoider, EIGHT_DIRECTIONS, InsectBase, InsectBehavior, InsectCx};
use crate::color::RandomVariation;
use crate::{FloatExt, Vec2};

/// `RockFlea`.
#[derive(Debug, Clone, PartialEq)]
pub struct RockFlea {
    pub sit_pos: Option<Vec2>,
    pub stressed: f32,
    pub body_move: f32,
    pub last_body_move: f32,
    pub size: f32,
    pub rot: Vec2,
    pub last_rot: Vec2,
    /// `legs[i, 0]` (now) and `legs[i, 1]` (last tick).
    pub legs: [[Vec2; 2]; 2],
    pub color_fac: f32,
    swim: f32,
}

impl RockFlea {
    /// The constructor after the base's (four draws).
    pub fn new(b: &mut InsectBase, cx: &mut InsectCx) -> Self {
        b.avoider = Some(CreatureAvoider::new(20, 300.0, 0.2, cx));
        let size = cx.rng.clamped_random_variation(0.75, 0.25, 0.5);
        let color_fac = cx.value();
        Self {
            sit_pos: None,
            stressed: 0.0,
            body_move: 0.0,
            last_body_move: 0.0,
            size,
            rot: Vec2::ZERO,
            last_rot: Vec2::ZERO,
            legs: [[Vec2::ZERO; 2]; 2],
            color_fac,
            swim: 0.0,
        }
    }

    fn side(i: usize) -> f32 {
        if i == 0 { -1.0 } else { 1.0 }
    }

    /// `Update` (`PointDeferred` is false without More Slugcats).
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.last_body_move = self.body_move;
        self.last_rot = self.rot;
        self.base_update(b, cx);
        if self.sit_pos.is_none() && !b.submerged {
            b.vel.y -= 0.8;
        }
        if b.submerged {
            b.vel *= 0.9;
            for i in 0..2 {
                self.legs[i][1] = self.legs[i][0];
                if self.sit_pos.is_none() {
                    let deg = self.rot.to_degrees() + 80.0.lerp(170.0, self.swim) * Self::side(i);
                    self.legs[i][0] = self.legs[i][0].slerp(Vec2::from_degrees(deg), 0.8);
                }
            }
            return;
        }
        for j in 0..2 {
            self.legs[j][1] = self.legs[j][0];
            if self.sit_pos.is_none() {
                let to = (-b.vel
                    + b.vel.normalized().perpendicular() * (Self::side(j) * -5.0)
                    + Vec2::new(Self::side(j), 0.7))
                .normalized();
                self.legs[j][0] = self.legs[j][0].slerp(to, 0.2);
            }
        }
    }

    /// `Jump`.
    fn jump(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        let Some(sit) = self.sit_pos else {
            return;
        };
        if b.burrow_pos.is_some() {
            return;
        }
        let mut dir = cx.value().mathf_pow(0.75) * if cx.value() < 0.5 { -1.0 } else { 1.0 };
        if let Some(s) = b.swarm.filter(|_| b.out_of_bounds()) {
            dir = cx.value().mathf_pow(0.75) * if s.center.x > b.pos.x { 1.0 } else { -1.0 };
        } else if let Some(danger) = b.worst_danger_pos() {
            dir = cx.value().mathf_pow(0.75) * if danger.x < b.pos.x { 1.0 } else { -1.0 };
        }
        let (tx, ty) = sit.tile();
        let sign = if dir >= 0.0 { 1 } else { -1 };
        if cx.solid_tile((tx + sign, ty)) {
            dir *= -1.0;
        }
        let a = (-20.0).lerp(20.0, cx.value());
        let v = Vec2::from_degrees(a + 30.0.lerp(60.0, cx.value()) * dir);
        b.pos = sit + v * 2.0;
        let speed = 11.0.lerp(19.0, cx.value().mathf_pow(1.2.lerp(0.6, self.stressed)));
        b.vel = v * (speed * self.size);
        self.sit_pos = None;
    }
}

impl InsectBehavior for RockFlea {
    fn reset(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.reset(pos, cx.rng);
        self.sit_pos = None;
    }

    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
        if b.want_to_burrow {
            self.sit_pos = None;
            if b.submerged {
                self.rot = self.rot.slerp(Vec2::new(0.0, -1.0), 0.3);
            }
            return;
        }
        let fear = b.flee_speed().mathf_pow(0.3);
        self.stressed = if fear > self.stressed {
            self.stressed.lerp_and_tick(fear, 0.1, 0.05)
        } else {
            self.stressed.lerp_and_tick(fear, 0.02, 0.005)
        };
        if b.submerged {
            self.swim += 1.0 / 40.0.lerp(10.0, self.stressed);
            if self.swim > 1.0 {
                self.swim = 0.0;
            }
            b.vel += self.rot
                * (1.2.lerp(3.0, self.stressed)
                    * self.size
                    * self.swim.mathf_pow(2.0 - self.stressed));
            self.rot += cx.rnv() * 0.2;
            if let Some(danger) = b.worst_danger_pos() {
                self.rot -= b.pos.direction_to(danger) * b.flee_speed();
            }
            if b.pos.x < 0.0 {
                self.rot.x += 1.0;
            } else if b.pos.x > cx.room.pixel_width() {
                self.rot.x -= 1.0;
            }
            if b.pos.y < 0.0 {
                self.rot.y += 1.0;
            }
            if b.want_to_burrow {
                self.rot.y -= 0.5;
            }
            self.rot = self.rot.normalized();
            return;
        }
        self.body_move = if self.body_move < 0.5 {
            0.5.lerp(1.0, cx.value())
        } else {
            0.0.lerp(0.5, cx.value())
        };
        let (tx, ty) = b.pos.tile();
        if cx.value() < 0.025 && cx.solid_tile((tx - 1, ty)) && cx.solid_tile((tx + 1, ty)) {
            self.stressed = 1.0;
        }
        if let Some(sit) = self.sit_pos {
            b.pos = sit;
            b.vel *= 0.0;
            let chance = 1.0 / 400.0.lerp(15.0, self.stressed.mathf_pow(0.5));
            let close = b
                .worst_danger_pos()
                .is_some_and(|d| b.pos.dist_less(d, 30.0));
            if cx.value() < chance || close {
                self.jump(b, cx);
            }
        } else {
            self.rot = b.vel.normalized();
        }
    }

    fn wall_collision(
        &mut self,
        b: &mut InsectBase,
        dir: (i32, i32),
        _first: bool,
        cx: &mut InsectCx,
    ) {
        if b.submerged {
            self.rot -= Vec2::new(dir.0 as f32, dir.1 as f32);
            self.rot = self.rot.normalized();
            return;
        }
        if self.sit_pos.is_some() || dir.1 >= 1 || b.want_to_burrow {
            return;
        }
        self.sit_pos = Some(b.pos);
        let mut v = cx.rnv() * 0.1;
        for (dx, dy) in EIGHT_DIRECTIONS {
            let d = Vec2::new(dx as f32, dy as f32);
            if cx.solid_at(b.pos + d * 20.0) {
                v += d.normalized();
            }
        }
        self.legs[0][0] = (v + Vec2::new(-1.0, 0.7)).normalized();
        self.legs[1][0] = (v + Vec2::new(1.0, 0.7)).normalized();
        let side = if cx.value() < 0.5 { -1.0 } else { 1.0 };
        self.rot = (v.perpendicular() * side + cx.rnv() * 0.2).normalized();
    }

    fn emerge_from_ground(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.pos = pos;
        self.sit_pos = Some(pos + Vec2::new(0.0, 4.0));
        self.jump(b, cx);
    }
}
