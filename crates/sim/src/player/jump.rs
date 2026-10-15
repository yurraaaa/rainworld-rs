//! `Player.Jump` and `Player.WallJump` (Survivor values, no adrenaline or exhaustion).
//!
//! Behaviour: `slugcat-movement.md` section 5, `slope-collision.md` section 5,
//! `vertical-poles.md` section 6, `ledge-grab.md` section 3, `corridors.md` section 5,
//! `advanced-moves.md`
//! sections 1-3 (the roll, belly-slide and flip jumps are in `advanced.rs`).

use rw_assets::Room;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::SoundId;

impl Player {
    /// A jump from the ground, a beam or pole top, a wall-climb, a ledge, a pole, a
    /// roll or a belly slide; or the start of a belly slide.
    pub(super) fn jump(&mut self, room: &Room) {
        self.feet_stuck_pos = None;
        self.aerobic_increase(1.0);
        if self.body_mode == BodyMode::WallClimb {
            let head_contact = i32::from(self.body.chunks[HEAD].contact().x);
            let dir = if self.can_wall_jump != 0 {
                self.can_wall_jump.signum()
            } else if head_contact == 0 {
                -self.flip_direction
            } else {
                -head_contact
            };
            self.wall_jump(room, dir);
            return;
        }
        if self.body_mode == BodyMode::CorridorClimb {
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y = 6.0;
            hips.vel.y = 5.0;
            self.standing = true;
            self.jump_boost = 8.0;
            return;
        }
        if self.animation == Animation::LedgeGrab {
            if self.input_x() != 0 {
                self.wall_jump(room, -self.input_x());
            }
            return;
        }
        if self.animation == Animation::ClimbOnBeam {
            self.jump_off_pole();
            return;
        }
        if self.animation == Animation::Roll {
            self.roll_jump();
            return;
        }
        if self.animation == Animation::BellySlide {
            self.belly_slide_jump(room);
            return;
        }
        if self.starts_belly_slide() {
            return;
        }
        let mut x_dir = self.input_x();
        self.animation = Animation::None;
        if self.standing {
            if !self.flip_jump() {
                let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
                head.vel.y = 4.0;
                hips.vel.y = 3.0;
                self.jump_boost = 8.0;
                let id = if self.body_mode == BodyMode::ClimbingOnBeam {
                    SoundId::SlugcatFromHorizontalPoleJump
                } else {
                    SoundId::SlugcatNormalJump
                };
                self.play_sound(id, HEAD, 1.0, 1.0);
            }
        } else {
            let mut hop = 1.5;
            if self.super_launch_jump >= 20 {
                self.super_launch_jump = 0;
                hop = 9.0;
                let [head, hips] = [&self.body.chunks[HEAD], &self.body.chunks[HIPS]];
                x_dir = if head.pos.x > hips.pos.x { 1 } else { -1 };
                self.simulate_hold_jump_button = 6;
            }
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.pos.y += 6.0;
            if head.contact().y == -1 {
                head.vel.y += 3.0;
                if x_dir == 0 {
                    head.vel.y += 3.0;
                }
            }
            hips.vel.y += 4.0;
            self.jump_boost = 6.0;
            if x_dir != 0 && (head.pos.x > hips.pos.x) == (x_dir > 0) {
                let push = x_dir as f32 * hop;
                head.vel.x += push;
                hips.vel.x += push;
                let id = if hop >= 9.0 {
                    SoundId::SlugcatSuperJump
                } else {
                    SoundId::SlugcatCrouchJump
                };
                self.play_sound(id, HEAD, 1.0, 1.0);
            }
        }

        let downhill = i32::from(self.body.chunks[HIPS].on_slope());
        if downhill != 0 {
            let side = downhill as f32;
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            if x_dir == -downhill {
                hips.vel.x += side * 8.0;
            } else {
                head.vel.x += side * 1.8;
                hips.vel.x += side * 1.2;
            }
        }
    }

    /// From a vertical pole: sideways with x held, else the slide-up hop (up held) or a
    /// drop.
    fn jump_off_pole(&mut self) {
        self.jump_boost = 0.0;
        let (x, y) = (self.input_x(), self.input_y());
        let flip = self.flip_direction as f32;
        if x == 0 && y > 0 {
            if self.slow_movement_stun < 1 && self.slide_up_pole < 1 {
                self.blink(7);
                for c in &mut self.body.chunks {
                    c.pos.y += 4.5;
                    c.vel.y += 2.0;
                }
                self.slide_up_pole = 17;
                self.play_sound(SoundId::SlugcatFromVerticalPoleJump, HEAD, 0.8, 1.0);
            }
            return;
        }
        self.animation = Animation::None;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        let volume = if x == 0 {
            head.vel.y = 2.0;
            if y > -1 {
                head.vel.x = 2.0 * flip;
            }
            0.3
        } else {
            head.vel = crate::Vec2::new(6.0 * flip, 8.0);
            hips.vel = crate::Vec2::new(5.0 * flip, 7.0);
            1.0
        };
        self.play_sound(SoundId::SlugcatFromVerticalPoleJump, HEAD, volume, 1.0);
    }

    /// Jump away from a wall toward `dir`, or a straight hop when there is footing;
    /// weaker while exhausted (`1 - 0.5 * aerobicLevel`).
    pub(super) fn wall_jump(&mut self, room: &Room, dir: i32) {
        let mut num = 1.0;
        if self.exhausted {
            num *= 1.0 - 0.5 * self.aerobic_level;
        }
        let x = self.input_x();
        let head_contact = i32::from(self.body.chunks[HEAD].contact().x);
        let at_ledge = x != 0
            && head_contact == x
            && self.is_tile_solid(room, HEAD, x, 0)
            && !self.is_tile_solid(room, HEAD, x, 1);
        let has_footing = self.is_tile_solid(room, HIPS, 0, -1)
            || self.is_tile_solid(room, HEAD, 0, -1)
            || at_ledge;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if has_footing {
            if hips.contact().y > -1 && head.contact().y > -1 {
                num *= 0.7;
            }
            head.vel.y = 8.0 * num;
            hips.vel.y = 7.0 * num;
            head.pos.y += 10.0 * num.min(1.0);
            hips.pos.y += 10.0 * num.min(1.0);
            self.play_sound(SoundId::SlugcatNormalJump, HEAD, 1.0, 1.0);
            self.jump_boost = 0.0;
        } else {
            let d = dir as f32;
            head.vel.y = 8.0 * num;
            hips.vel.y = 7.0 * num;
            head.vel.x = 6.0 * num * d;
            hips.vel.x = 5.0 * num * d;
            self.play_sound(SoundId::SlugcatWallJump, HEAD, 1.0, 1.0);
            self.standing = true;
            self.jump_boost = 0.0;
            self.jump_stun = 8 * dir;
        }
        self.can_wall_jump = 0;
    }
}
