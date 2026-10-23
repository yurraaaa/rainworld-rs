//! The player pressing into a wrong shortcut hole: `ShortcutHelper.Update`'s loop over
//! the room's pushers and `ShortcutPusher.PushPlayerTowardsValidNeighbor`.
//!
//! Omitted until the Five Pebbles oracle is ported (OPEN-ITEMS): `Room.lockedShortcuts`
//! (`SSOracleBehavior.LockShortcuts`); also the Challenge arena's closed exits (DLC).
//!
//! Mirrors: ShortcutHelper.cs (Update, ShortcutPusher.PushPlayerTowardsValidNeighbor)

use super::{Animation, BodyMode, Player};
use crate::game::shortcut_helper::ShortcutPusher;
use crate::{FloatExt, Vec2};

impl Player {
    /// `enteringShortCut = tile`, set by an `ObjectPusher` giving up a vertical spit-out.
    pub(crate) fn set_entering_shortcut(&mut self, tile: (i32, i32)) {
        self.entering_shortcut = Some(tile);
    }

    /// The part of `ShortcutHelper.Update` that is about this player (it is conscious and
    /// not carried): every wrong hole pushes it away. `room_gravity` is `room.gravity`.
    pub(crate) fn pushed_by_shortcuts(
        &mut self,
        pushers: &mut [ShortcutPusher],
        room_gravity: f32,
    ) {
        let input = (self.input_x(), self.input_y());
        for j in 0..pushers.len() {
            let dir = pushers[j].shortcut_dir;
            let dir_vec = Vec2::new(dir.0 as f32, dir.1 as f32);
            let push_pos = pushers[j].push_pos;
            let goes_through_floors = self.body.chunks.iter().all(|c| c.go_through_floors);
            if !pushers[j].wrong_hole
                || (pushers[j].floor && !goes_through_floors)
                || (dir.1 > 0
                    && matches!(
                        self.animation,
                        Animation::BellySlide | Animation::DownOnFours
                    ))
            {
                continue;
            }
            let into_hole =
                (input.0 != 0 && input.0 == -dir.0) || (input.1 != 0 && input.1 == -dir.1);
            let swells = self.input[0].jump
                || self.jump_boost > 0.0
                || !pushers[j].valid_neighbors.is_empty();
            if self.entering_shortcut == Some(pushers[j].shortcut_pos) {
                self.entering_shortcut = None;
            }
            for k in 0..self.body.chunks.len() {
                let (pos, rad) = (self.body.chunks[k].pos, self.body.chunks[k].rad);
                if into_hole
                    && self.input[0].jump
                    && !self.input[1].jump
                    && push_pos.dist_less(pos, 30.0 + rad)
                {
                    let aim = dir_vec * 6.0 + Vec2::new(0.0, if dir.0 != 0 { 6.0 } else { 0.0 });
                    let c = &mut self.body.chunks[k];
                    c.vel = c.vel.lerp(aim, 0.5);
                } else if swells {
                    self.swell_into_hole(pushers, j, k, into_hole, input, room_gravity);
                }
                let c = &self.body.chunks[k];
                if self.roll_direction != 0
                    && dir.0 == -self.roll_direction
                    && push_pos.dist_less(c.pos, 30.0 + c.rad)
                {
                    self.roll_direction = 0;
                    self.animation = Animation::None;
                    self.roll_counter = 0;
                }
                let reach = 10.0 + self.body.chunks[k].rad;
                let c = &mut self.body.chunks[k];
                if c.pos.y > push_pos.y - reach
                    && c.pos.y < push_pos.y + reach
                    && c.pos.x > push_pos.x - reach
                    && c.pos.x < push_pos.x + reach
                {
                    if dir.0 != 0 {
                        let d = push_pos.x + reach * dir.0 as f32 - c.pos.x;
                        c.vel.x += d;
                        c.pos.x += d;
                    } else {
                        let d = push_pos.y + reach * dir.1 as f32 - c.pos.y;
                        c.vel.y += d;
                        c.pos.y += d;
                    }
                }
            }
        }
    }

    /// The `else if` branch of one chunk: the pusher's swelling radius around its push
    /// position slows and shoves the chunk, or hands it to a valid neighbour.
    fn swell_into_hole(
        &mut self,
        pushers: &mut [ShortcutPusher],
        j: usize,
        k: usize,
        into_hole: bool,
        input: (i32, i32),
        room_gravity: f32,
    ) {
        let dir = pushers[j].shortcut_dir;
        let push_pos = pushers[j].push_pos;
        let (pos, rad) = (self.body.chunks[k].pos, self.body.chunks[k].rad);
        let down_into = input.1 != 0 && input.1 == -dir.1;
        let num = 20.0 + rad + pushers[j].swell.lerp_map_pow(0.5, 1.0, -5.0, 10.0, 3.0)
            - if down_into { 5.0 } else { 0.0 };
        pushers[j].swell_up = push_pos.dist_less(pos, (20.0 + rad).max(num - 1.0)) & into_hole;
        if !push_pos.dist_less(pos, num) {
            return;
        }
        let near = push_pos.distance(pos);
        let num2 = near.inverse_lerp(num - if into_hole { 2.5 } else { 5.0 }, num - 20.0);
        if !pushers[j].valid_neighbors.is_empty() && into_hole {
            self.push_towards_valid_neighbor(pushers, j, num2, room_gravity);
            return;
        }
        let gravity = self.body.params.g * room_gravity * room_gravity;
        let dir_vec = Vec2::new(dir.0 as f32, dir.1 as f32);
        let c = &mut self.body.chunks[k];
        c.vel *= 1.0_f32.lerp(0.5, num2);
        c.vel.y += gravity * num2;
        let push = push_pos.direction_to(c.pos).slerp(dir_vec, 0.9)
            * ((if into_hole { 3.0 } else { 0.9 }) * num2);
        c.vel += push;
        c.pos += push;
        if into_hole && dir.0 != 0 {
            c.vel.y = c.vel.y.lerp(c.vel.y.clamp(-2.0, 20.0), 0.75);
        }
    }

    /// `ShortcutPusher.PushPlayerTowardsValidNeighbor` of pusher `j`.
    pub(crate) fn push_towards_valid_neighbor(
        &mut self,
        pushers: &mut [ShortcutPusher],
        j: usize,
        push_fac: f32,
        room_gravity: f32,
    ) {
        pushers[j].swell = 2.0;
        let mut nearest: Option<usize> = None;
        let mut best = f32::MAX;
        for chunk in &self.body.chunks {
            for &n in &pushers[j].valid_neighbors {
                if chunk.pos.dist_less(pushers[n].push_pos, best) {
                    nearest = Some(n);
                    best = chunk.pos.distance(pushers[n].push_pos);
                }
            }
        }
        let Some(n) = nearest.filter(|_| best < 40.0) else {
            return;
        };
        self.animation = Animation::None;
        self.body_mode = BodyMode::Default;
        let (target, target_dir) = (pushers[n].push_pos, pushers[n].shortcut_dir);
        let horizontal = pushers[j].shortcut_dir.0 != 0;
        let gravity = self.body.params.g * room_gravity * room_gravity;
        for c in &mut self.body.chunks {
            if horizontal {
                c.vel.x += target_dir.0 as f32 * push_fac;
                c.vel.y *= 0.5 - 0.3 * push_fac;
                c.vel.y += gravity;
                c.vel.y += (target.y - c.pos.y).clamp(-1.2, 1.2);
                c.pos.y += (target.y - c.pos.y).clamp(-1.2, 1.2);
            } else {
                c.vel.y += target_dir.1 as f32 * push_fac;
                c.vel.x *= 0.5 - 0.3 * push_fac;
                c.vel.x += (target.x - c.pos.x).clamp(-1.2, 1.2);
                c.pos.x += (target.x - c.pos.x).clamp(-1.2, 1.2);
            }
        }
    }
}
