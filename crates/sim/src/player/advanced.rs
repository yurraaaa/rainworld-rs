//! Advanced moves: the Stand skid and flip jump, `Flip`, `Roll`, `RocketJump` and
//! `BellySlide` with their jumps. The landing roll that starts a roll is in `falls.rs`.
//!
//! Behaviour: `rainworld-re/notes/advanced-moves.md`, Survivor values, no adrenaline,
//! no held items (docs/DIVERGENCES.md D7).

use std::f32::consts::PI;

use rw_assets::{Room, Terrain};

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::Vec2;
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;

/// Rod length while `roll_direction != 0` (roll, belly slide, whiplash flip).
pub(super) const ROLL_ROD_LENGTH: f32 = 10.0;

impl Player {
    /// `allowRoll`: counts down, and is held at 15 while the tile 20 px below the hips
    /// is Air without solid ground on both sides below.
    pub(super) fn update_allow_roll(&mut self, room: &Room) {
        if self.allow_roll > 0 {
            self.allow_roll -= 1;
        }
        let below = self.body.chunks[HIPS].pos + Vec2::new(0.0, -20.0);
        if room.tile_at_pos(below).terrain == Terrain::Air
            && !(self.is_tile_solid(room, HIPS, -1, -1) && self.is_tile_solid(room, HIPS, 1, -1))
        {
            self.allow_roll = 15;
        }
    }

    /// Roll bookkeeping after `UpdateBodyMode`: count, shorten the rod, and drop the
    /// direction once the body left `Default` mode or after 200 ticks.
    pub(super) fn count_roll(&mut self) {
        if self.roll_direction == 0 {
            self.body.connections[0].distance = crate::slugcat::ROD_LENGTH;
            return;
        }
        self.roll_counter += 1;
        self.body.connections[0].distance = ROLL_ROD_LENGTH;
        if self.body_mode != BodyMode::Default || self.roll_counter > 200 {
            self.roll_counter = 0;
            self.roll_direction = 0;
        }
    }

    /// The turn-around skid at the end of Stand mode: reversing after running the same
    /// way for more than 10 ticks skids for up to 20 ticks, first drifting along the old
    /// direction, then braking.
    pub(super) fn skid(&mut self) {
        let x = self.input_x();
        let sd = self.slide_direction;
        if self.slide_counter > 0 {
            self.slide_counter += 1;
            if self.slide_counter > 20 || x != -sd {
                self.slide_counter = 0;
            }
            // Evaluated after the reset, so the last tick pushes with n = 0.5.
            let n = 0.5 - (self.slide_counter as f32 / 20.0 * PI * 0.5).sin();
            let (n_sd, sd) = (n * 3.5 * sd as f32, sd as f32);
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.x += n_sd - sd * if n < 0.0 { 0.8 } else { 0.5 };
            hips.vel.x += n_sd + sd * 0.5;
        } else if x != 0 {
            if x == sd {
                self.init_slide_counter = (self.init_slide_counter + 1).min(30);
                return;
            }
            let head_vx = self.body.chunks[HEAD].vel.x;
            if self.init_slide_counter > 10 && (head_vx > 0.0) == (sd > 0) && head_vx.abs() > 1.0 {
                self.slide_counter = 1;
            } else {
                self.slide_direction = x;
            }
            self.init_slide_counter = 0;
        } else if self.init_slide_counter > 0 {
            self.init_slide_counter -= 1;
        }
    }

    /// Somersault torque toward `slide_direction` until either chunk touches terrain.
    /// The whiplash flip uses the same stale `slide_direction`, as in the game.
    pub(super) fn flip(&mut self) {
        self.body_mode = BodyMode::Default;
        let boost = if self.flip_from_slide { 2.5 } else { 1.0 };
        let k = self.slide_direction as f32 * 0.38 * boost;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        let torque = (hips.pos - head.pos).perpendicular() * k;
        head.vel -= torque;
        hips.vel += torque;
        self.standing = false;
        if [&*head, &*hips]
            .iter()
            .any(|c| c.contact().x != 0 || c.contact().y != 0)
        {
            self.animation = Animation::None;
            self.standing = head.pos.y > hips.pos.y;
        }
    }

    /// Rolling: damping, spin torque, a push along the ground and a hop up one-tile
    /// steps; ends when blocked, upright after enough ticks, or reversed.
    pub(super) fn roll(&mut self, room: &Room) {
        self.body_mode = BodyMode::Default;
        let dir = self.roll_direction;
        let d = dir as f32;
        let gravity = self.body.params.gravity;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        let perp = (hips.pos - head.pos).perpendicular();
        head.vel *= 0.9;
        hips.vel *= 0.9;
        head.vel += perp * (2.0 * d);
        hips.vel -= perp * (2.0 * d);
        let mut blocked = i32::from(head.contact().x) == dir || i32::from(hips.contact().x) == dir;
        if i32::from(hips.on_slope()) == -dir || i32::from(head.on_slope()) == -dir {
            head.pos += perp * d;
            hips.pos -= perp * d;
        }
        let airborne = !self.is_tile_solid(room, HEAD, 0, -1)
            && !self.is_tile_solid(room, HIPS, 0, -1)
            && self.body.chunks.iter().all(|c| c.contact().y >= 0);
        if airborne {
            if self.is_tile_solid(room, HEAD, 0, -2) || self.is_tile_solid(room, HIPS, 0, -2) {
                for c in &mut self.body.chunks {
                    c.vel *= 0.7;
                    c.pos.y -= 2.5;
                }
            } else {
                blocked = true;
            }
        } else {
            for c in &mut self.body.chunks {
                c.vel.x += 1.1 * d;
            }
            self.can_jump = self.can_jump.max(5);
            let headroom =
                !self.is_tile_solid(room, HEAD, 0, 1) && !self.is_tile_solid(room, HIPS, 0, 1);
            let step_ahead = [HEAD, HIPS].into_iter().any(|k| {
                self.is_tile_solid(room, k, dir, 0) && !self.is_tile_solid(room, k, dir, 1)
            });
            if step_ahead && headroom {
                for c in &mut self.body.chunks {
                    c.vel *= 0.7;
                    c.pos.y += 5.0;
                    c.vel.y += gravity;
                }
                blocked = false;
            }
        }
        if blocked {
            self.stop_rolling_counter += 1;
        } else {
            self.stop_rolling_counter = 0;
        }

        let (x, y) = (self.input_x(), self.input_y());
        let diag = i32::from(self.input[0].down_diagonal());
        let n = self.roll_counter;
        let upright = self.body.chunks[HEAD].pos.y > self.body.chunks[HIPS].pos.y;
        let done_upright = (n > 15 && y > -1 && diag == 0) || n > 30 || x == -dir;
        if (done_upright && upright) || n > 60 || self.stop_rolling_counter > 6 {
            self.roll_direction = 0;
            self.animation = Animation::None;
            self.standing = y > -1;
        }
    }

    /// Thrust along the head's direction of travel until the hips touch terrain.
    pub(super) fn rocket_jump(&mut self) {
        self.body_mode = BodyMode::Default;
        self.standing = false;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        hips.vel *= 0.99;
        let thrust = head.vel.normalized();
        head.vel += thrust;
        hips.vel -= thrust;
        head.vel.y += 0.1;
        hips.vel.y += 0.1;
        if hips.contact().x != 0 || hips.contact().y != 0 {
            self.animation = Animation::None;
        }
    }

    /// Belly slide: a 6-tick hips kick back, a half-sine head push over 15 ticks,
    /// holding to the ground, the drop through a one-tile hole, and the exits.
    pub(super) fn belly_slide(&mut self, room: &Room) {
        self.body_mode = BodyMode::Default;
        let dir = self.roll_direction;
        let d = dir as f32;
        let n = self.roll_counter;
        if n < 6 {
            let hips = &mut self.body.chunks[HIPS];
            hips.vel.y += 2.7;
            hips.vel.x -= 9.1 * d;
        } else if self.is_tile_solid(room, HIPS, 0, -1) || self.is_tile_solid(room, HIPS, 0, -2) {
            self.body.chunks[HIPS].vel.y -= 0.5;
        }
        self.body.chunks[HEAD].vel.x += 18.1 * d * (n as f32 / 15.0 * PI).sin();
        if self.is_tile_solid(room, HEAD, 0, -1) || self.is_tile_solid(room, HEAD, 0, -2) {
            self.body.chunks[HEAD].vel.y -= 2.3;
        }
        let friction = self.body.params.surface_friction;
        for c in &mut self.body.chunks {
            if c.contact().y == 0 {
                c.vel.x *= friction;
            }
        }

        let (now, prev) = (self.input[0], self.input[1]);
        let (x, y) = (self.input_x(), self.input_y());
        let diag = i32::from(now.down_diagonal());
        if y < 0 && diag == 0 && x == 0 && n > 8 && self.tile_pos(HEAD).1 == self.tile_pos(HIPS).1 {
            let (tx, ty) = self.tile_pos(HEAD);
            if !room.is_solid_tile(tx, ty - 1)
                && room.is_solid_tile(tx - 1, ty - 1)
                && room.is_solid_tile(tx + 1, ty - 1)
            {
                self.drop_through_hole();
                return;
            }
        }
        if x != dir && diag != dir {
            self.exit_belly_slide_counter += 1;
        } else {
            self.exit_belly_slide_counter = 0;
        }
        if n > 5 && x == -dir {
            self.whiplash_jump = true;
        }
        let ungrounded =
            !self.is_tile_solid(room, HEAD, 0, -1) && !self.is_tile_solid(room, HIPS, 0, -1);
        let jump_pressed = now.jump && !prev.jump;
        if (n > 8 && self.exit_belly_slide_counter > 6)
            || n > 15
            || (n > 6 && ungrounded)
            || (jump_pressed && n > 0 && n < 12)
        {
            self.end_belly_slide(room);
        } else {
            self.standing = false;
        }
    }

    /// Both chunks fall into the one-tile hole under the head.
    fn drop_through_hole(&mut self) {
        let rod = self.body.connections[0].distance;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.pos = head.pos.tile_middle() + Vec2::new(0.0, -20.0);
        head.vel = Vec2::new(0.0, -11.0);
        hips.pos = hips.pos.lerp(head.pos + Vec2::new(0.0, rod), 0.5);
        hips.vel = Vec2::new(0.0, -11.0);
        self.animation = Animation::None;
        self.standing = false;
        self.body.set_go_through_floors(true);
        self.roll_direction = 0;
    }

    /// Stops the slide: vertical speed cut, halved if very fast, standing up only with
    /// up held under headroom, and slowed movement for 20 (standing) or 40 ticks.
    fn end_belly_slide(&mut self, room: &Room) {
        for c in &mut self.body.chunks {
            c.vel.y = 0.0;
        }
        self.roll_direction = 0;
        self.animation = Animation::None;
        self.standing = self.input_y() == 1 && !self.is_tile_solid(room, HEAD, 0, 1);
        for c in &mut self.body.chunks {
            if c.vel.x.abs() > 8.0 {
                c.vel *= 0.5;
            }
        }
        self.slow_movement_stun = if self.standing { 20 } else { 40 };
    }

    /// `Jump` while rolling: a rocket jump whose angle (60 to 35 degrees from up) and
    /// speed (9.5 to 13.1) grow over the first 25 roll ticks.
    pub(super) fn roll_jump(&mut self) {
        let dir = self.roll_direction as f32;
        let t = (self.roll_counter as f32).inverse_lerp(0.0, 25.0);
        let vel = Vec2::from_degrees(dir * 60.0_f32.lerp(35.0, t)) * 9.5_f32.lerp(13.1, t);
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        hips.vel = Vec2::ZERO;
        hips.pos += Vec2::new(5.0 * dir, 5.0);
        head.pos = hips.pos + Vec2::new(5.0 * dir, 5.0);
        head.vel = vel;
        hips.vel = vel;
        self.animation = Animation::RocketJump;
        self.roll_direction = 0;
    }

    /// `Jump` from a belly slide: the whiplash flip back when reversed (now or earlier
    /// in the slide), else a rocket jump forward.
    pub(super) fn belly_slide_jump(&mut self, room: &Room) {
        let dir = self.roll_direction;
        let d = dir as f32;
        if self.whiplash_jump || self.input_x() == -dir {
            self.animation = Animation::Flip;
            self.standing = true;
            let head_pos = self.body.chunks[HEAD].pos;
            let mut reach = 1;
            for j in 1..4 {
                let ahead = head_pos + Vec2::new((j * -dir) as f32 * 15.0, 0.0);
                if room.is_solid_at(ahead) || room.is_solid_at(ahead + Vec2::new(0.0, 20.0)) {
                    break;
                }
                reach = j;
            }
            let reach = reach as f32;
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.pos += Vec2::new(-d * (reach * 15.0 + 8.0), 14.0);
            hips.pos += Vec2::new(-d * (reach * 15.0 + 2.0), 0.0);
            head.vel = Vec2::new(-7.0 * d, 10.0);
            hips.vel = Vec2::new(-7.0 * d, 11.0);
            self.roll_direction = -dir;
            self.flip_from_slide = true;
            self.whiplash_jump = false;
            self.jump_boost = 0.0;
        } else {
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            hips.pos += Vec2::new(5.0 * d, 5.0);
            head.pos = hips.pos + Vec2::new(5.0 * d, 5.0);
            hips.vel = Vec2::new(9.0 * d, 8.5);
            head.vel = hips.vel;
            self.animation = Animation::RocketJump;
            self.rocket_jump_from_belly_slide = true;
            self.roll_direction = 0;
        }
    }

    /// A jump while still lowering to all fours, hips on the ground and down-diagonal
    /// toward the facing, starts a belly slide instead of jumping.
    pub(super) fn starts_belly_slide(&mut self) -> bool {
        let diag = i32::from(self.input[0].down_diagonal());
        if self.animation != Animation::DownOnFours
            || self.body.chunks[HIPS].contact().y >= 0
            || diag != self.flip_direction
        {
            return false;
        }
        self.animation = Animation::BellySlide;
        self.roll_direction = self.flip_direction;
        self.roll_counter = 0;
        self.standing = false;
        true
    }

    /// The skid flip jump (within ticks 1..9 of a skid), or `false` when not skidding.
    pub(super) fn flip_jump(&mut self) -> bool {
        if !(self.slide_counter > 0 && self.slide_counter < 10) {
            return false;
        }
        let sd = self.slide_direction as f32;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel.y = 9.0;
        hips.vel.y = 7.0;
        head.vel.x *= 0.5;
        hips.vel.x *= 0.5;
        head.vel.x -= sd * 4.0;
        self.jump_boost = 5.0;
        self.animation = Animation::Flip;
        self.slide_counter = 0;
        true
    }
}
