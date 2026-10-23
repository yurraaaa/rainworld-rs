//! Vertical poles: grabbing (`GrabVerticalPole`, the under-hang), the `BeamTip` trigger and
//! the `ClimbOnBeam`, `GetUpToBeamTip`, `BeamTip` and `HangUnderVerticalBeam` animations.
//!
//! Behaviour: `vertical-poles.md`.
//!
//! Mirrors: Player.cs (GrabVerticalPole, UpdateAnimation)

use rw_assets::Room;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{SoundId, Vec2};

impl Player {
    /// `GrabVerticalPole`: the head's column, plus the neighbours when standing with no x
    /// input. Every matching candidate grabs in turn, so the last one wins.
    pub(super) fn grab_vertical_pole(&mut self, room: &Room) {
        let (tx, ty) = self.tile_pos(HEAD);
        let grounded = self.body.chunks[HIPS].contact().y < 0 && self.input_x() == 0;
        let offsets: &[i32] = if grounded { &[0, -1, 1] } else { &[0] };
        for &dx in offsets {
            if !room.tile(tx + dx, ty).vertical_pole() {
                continue;
            }
            let mid_x = rw_assets::tile_centre(tx + dx, ty)[0];
            self.play_sound(SoundId::SlugcatGrabBeam, HEAD, 1.0, 1.0);
            self.animation = Animation::ClimbOnBeam;
            let head = &mut self.body.chunks[HEAD];
            self.flip_direction = if dx == 0 && head.vel.x.abs() > 5.0 {
                if head.vel.x < 0.0 { -1 } else { 1 }
            } else if head.pos.x < mid_x {
                -1
            } else {
                1
            };
            head.vel = Vec2::ZERO;
            head.pos.x = mid_x;
        }
    }

    /// Hang from a pole's bottom end when it is in the tile above the head.
    pub(super) fn grab_under_vertical_pole(&mut self, room: &Room) {
        let head = self.body.chunks[HEAD].pos;
        if matches!(
            self.animation,
            Animation::HangFromBeam | Animation::ClimbOnBeam
        ) || !room
            .tile_at_pos(head + Vec2::new(0.0, 20.0))
            .vertical_pole()
            || room.tile_at_pos(head).vertical_pole()
        {
            return;
        }
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.pos = head.pos.tile_middle() + Vec2::new(0.0, 5.0);
        head.vel = Vec2::ZERO;
        hips.vel = hips.vel.clamp_length(9.0);
        self.animation = Animation::HangUnderVerticalBeam;
    }

    /// An up press with the hips just above a pole's top stands on it, from any state.
    pub(super) fn trigger_beam_tip(&mut self, room: &Room) {
        let hips = self.body.chunks[HIPS].pos;
        if self.input[0].y > 0
            && self.input[1].y == 0
            && !room.tile_at_pos(hips).vertical_pole()
            && room
                .tile_at_pos(hips + Vec2::new(0.0, -20.0))
                .vertical_pole()
        {
            self.animation = Animation::BeamTip;
            self.body.chunks[HIPS].vel = Vec2::ZERO;
        }
    }

    pub(super) fn climb_on_beam(&mut self, room: &Room) {
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.standing = true;
        self.can_jump = 1;
        for i in [HEAD, HIPS] {
            let cx = i32::from(self.body.chunks[i].contact().x);
            if cx != 0 {
                self.flip_direction = -cx;
            }
        }
        self.body.chunks[HEAD].vel.x = 0.0;
        let (x, y) = (self.input_x(), self.input_y());
        // Climbing up into an opening above the side: centre on the pole.
        let centred = !self.is_tile_solid(room, HEAD, 0, 1)
            && y > 0
            && (self.body.chunks[HEAD].contact().y < 0
                || self.is_tile_solid(room, HEAD, self.flip_direction, 1));
        if !centred && self.is_tile_solid(room, HEAD, self.flip_direction, 0) {
            self.flip_direction = -self.flip_direction;
        }
        let side = if centred {
            0.0
        } else {
            self.flip_direction as f32 * 5.0
        };
        let gravity = self.body.params.gravity();
        let eg = self.effective_room_gravity();
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.pos.x = (head.pos.x + head.pos.tile_middle().x + side) / 2.0;
        hips.pos.x = (hips.pos.x * 7.0 + head.pos.tile_middle().x + side) / 8.0;
        head.vel.y *= 0.5;
        let mut climb_step = false;
        if y > 0 {
            self.animation_frame += 1;
            if self.animation_frame > 20 {
                self.animation_frame = 0;
                climb_step = true;
            }
            head.vel.y += self.stats.pole_climb_speed_fac
                * (self.slow_movement_stun as f32).lerp_map(0.0, 10.0, 1.0, 0.2);
        } else if y < 0 {
            head.vel.y -= 2.2 * (0.2 + 0.8 * eg);
        }
        head.vel.y += 1.0 + gravity;
        hips.vel.y -= 1.0 - gravity;
        if self.slide_up_pole > 0 {
            self.slide_up_pole -= 1;
            if self.slide_up_pole > 8 {
                self.animation_frame = 12;
            }
            if self.slide_up_pole == 0 {
                self.slow_movement_stun = self.slow_movement_stun.max(16);
            }
            if self.slide_up_pole > 14 {
                head.pos.y += 2.0;
                hips.pos.y += 2.0;
            }
            let n = self.slide_up_pole as f32;
            head.vel.y += n.lerp_map_pow(17.0, 0.0, 3.0, -1.2, 0.45);
            hips.vel.y += n.lerp_map_pow(17.0, 0.0, 1.5, -1.4, 0.45);
        }
        if climb_step {
            self.play_sound(SoundId::SlugcatClimbUpVerticalBeam, HEAD, 1.0, 1.0);
            self.aerobic_increase(0.1);
        }
        self.body
            .set_go_through_floors(x == 0 && self.input[0].down_diagonal() == 0);
        self.leave_pole_sideways(room);
        if self.body.chunks[HIPS].contact().y < 0 && y < 0 {
            self.play_sound(SoundId::SlugcatRegainFooting, HEAD, 1.0, 1.0);
            self.animation = Animation::StandUp;
        }
        let head = self.body.chunks[HEAD].pos;
        if !room.tile_at_pos(head).vertical_pole() {
            self.animation = Animation::None;
            let (hx, hy) = head.tile();
            if room.tile(hx, hy - 1).vertical_pole() {
                self.play_sound(SoundId::SlugcatGetUpOnTopOfVerticalBeamTip, HEAD, 1.0, 1.0);
                self.animation = Animation::GetUpToBeamTip;
            } else if room.tile(hx, hy + 1).vertical_pole() {
                self.animation = Animation::HangUnderVerticalBeam;
            }
        }
    }

    /// An x press toward the facing side moves onto a horizontal beam here or hops to a
    /// neighbouring pole.
    fn leave_pole_sideways(&mut self, room: &Room) {
        let (now, prev) = (self.input[0].x, self.input[1].x);
        let x = i32::from(now);
        let flip = self.flip_direction;
        let toward_facing = x == flip && flip == self.last_flip_direction;
        if now != 0 && prev != now && toward_facing {
            if room
                .tile_at_pos(self.body.chunks[HEAD].pos)
                .horizontal_pole()
                && !self.is_tile_solid(room, HEAD, 0, -1)
            {
                self.animation = Animation::HangFromBeam;
            } else if room
                .tile_at_pos(self.body.chunks[HIPS].pos)
                .horizontal_pole()
            {
                self.animation = Animation::StandOnBeam;
            }
        }
        let (hx, hy) = self.tile_pos(HEAD);
        if toward_facing && prev == 0 && room.tile(hx + flip, hy).vertical_pole() {
            self.body.chunks[HEAD].pos.x =
                rw_assets::tile_centre(hx + flip, hy)[0] - flip as f32 * 5.0;
            self.flip_direction = -flip;
            self.jump_stun = 11 * self.flip_direction;
        }
    }

    pub(super) fn get_up_to_beam_tip(&mut self, room: &Room) {
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.standing = true;
        self.can_jump = 5;
        let gravity = self.body.params.gravity();
        for c in &mut self.body.chunks {
            c.vel.y += gravity;
        }
        let tip = [HEAD, HIPS].into_iter().find_map(|i| {
            let pos = self.body.chunks[i].pos;
            let above_tip = !room.tile_at_pos(pos).vertical_pole()
                && room
                    .tile_at_pos(pos + Vec2::new(0.0, -20.0))
                    .vertical_pole();
            above_tip.then(|| pos.tile_middle())
        });
        // The game's "not found" is the zero vector; a tile middle never is.
        let Some(p) = tip else {
            self.animation = Animation::None;
            return;
        };
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.pos.x = (head.pos.x * 14.0 + p.x) / 15.0;
        hips.pos.x = (hips.pos.x * 4.0 + p.x) / 5.0;
        head.vel.y += 0.1;
        hips.pos.y = (hips.pos.y * 4.0 + p.y) / 5.0;
        if hips.pos.distance(p) < 6.0 {
            self.animation = Animation::BeamTip;
            self.play_sound(SoundId::SlugcatRegainFooting, HEAD, 0.3, 1.0);
        }
    }

    pub(super) fn beam_tip(&mut self, room: &Room) {
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.standing = true;
        self.can_jump = 5;
        let (now, prev) = (self.input[0], self.input[1]);
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        hips.vel *= 0.5;
        hips.pos = (hips.pos + hips.pos.tile_middle()) / 2.0;
        head.vel.y += 1.5;
        head.vel.y += f32::from(now.y) * 0.1;
        head.vel.x += f32::from(now.x) * 0.1;
        // Undone by `trigger_beam_tip` later in the same tick.
        if now.y > 0 && prev.y == 0 {
            hips.vel.y -= 1.0;
            self.can_jump = 0;
            self.animation = Animation::None;
        }
        if (now.y < 0 && prev.y == 0)
            || head.pos.y < hips.pos.y
            || !room
                .tile_at_pos(hips.pos + Vec2::new(0.0, -20.0))
                .vertical_pole()
        {
            self.animation = Animation::None;
        }
    }

    pub(super) fn hang_under_vertical_beam(&mut self, room: &Room) {
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.standing = false;
        let (now, prev) = (self.input[0], self.input[1]);
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if (now.y < 0 && prev.y == 0)
            || hips.vel.length() > 10.0
            || head.vel.length() > 10.0
            || !room
                .tile_at_pos(head.pos + Vec2::new(0.0, 20.0))
                .vertical_pole()
        {
            self.animation = Animation::None;
            self.standing = true;
        } else {
            let mid = head.pos.tile_middle();
            head.pos.x += (mid.x - head.pos.x) * 0.5;
            head.pos.y = head.pos.y.max(mid.y + 5.0 + head.vel.y);
            head.vel.x = 0.0;
            head.vel.y *= 0.5;
            hips.vel.x += f32::from(now.x);
            if now.y > 0 {
                head.vel.y += 2.5;
            }
            if room.tile_at_pos(head.pos).vertical_pole() {
                self.animation = Animation::ClimbOnBeam;
            }
        }
        if now.jump && !prev.jump {
            self.animation = Animation::None;
            if now.x == 0 {
                head.pos.y += 16.0;
                head.vel.y = 10.0;
                self.standing = true;
            } else {
                let x = f32::from(now.x);
                hips.vel.y += 4.0;
                hips.vel.x += 2.0 * x;
                head.vel.y += 6.0;
                head.vel.x += 3.0 * x;
            }
        }
    }
}
