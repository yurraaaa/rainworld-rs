//! `LedgeGrab`: catching a wall top at head height and hauling up over it.
//!
//! Behaviour: `ledge-grab.md`.
//!
//! Mirrors: Player.cs (UpdateAnimation)

use rw_assets::Room;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::Vec2;

impl Player {
    /// Head pressed into a wall whose top is at head height, nothing under the head for
    /// two tiles. Overrides any animation.
    pub(super) fn trigger_ledge_grab(&mut self, room: &Room) {
        let x = self.input_x();
        let head = &self.body.chunks[HEAD];
        if x != 0
            && i32::from(head.contact().x) == x
            && head.pos.y > head.pos.tile_middle().y
            && matches!(self.body_mode, BodyMode::Default | BodyMode::WallClimb)
            && !self.is_tile_solid(room, HEAD, -x, 0)
            && !self.is_tile_solid(room, HEAD, 0, -2)
            && !self.is_tile_solid(room, HEAD, x, 1)
        {
            self.animation = Animation::LedgeGrab;
            self.body_mode = BodyMode::Default;
        }
    }

    pub(super) fn ledge_grab(&mut self, room: &Room) {
        self.body_mode = BodyMode::Default;
        let flip = self.flip_direction;
        if !self.is_tile_solid(room, HEAD, flip, 0) || self.is_tile_solid(room, HEAD, flip, 1) {
            self.animation = Animation::None;
            self.ledge_grab_counter = 0;
            return;
        }
        let f = flip as f32;
        let n = self.ledge_grab_counter as f32;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel *= 0.5;
        head.pos = (head.pos + (head.pos.tile_middle() + Vec2::new(f * n, 8.0 + n))) / 2.0;
        head.last_pos = head.pos;
        hips.vel.x += f;
        self.can_jump = 1;
        let (now, prev) = (self.input[0], self.input[1]);
        if i32::from(now.x) == flip || now.y == 1 {
            self.ledge_grab_counter += 1;
            hips.vel += Vec2::new(-0.5 * f, -0.5);
        } else if self.ledge_grab_counter > 0 {
            self.ledge_grab_counter -= 1;
        }
        if now.y == -1 && prev.y != -1 {
            head.pos.y -= 10.0;
            // Consumes the down edge for the rest of the tick.
            self.input[1].y = -1;
            self.animation = Animation::None;
            self.ledge_grab_counter = 0;
        } else if i32::from(now.x) == -flip && prev.x == 0 {
            head.vel.y += 10.0;
            self.animation = Animation::None;
            self.ledge_grab_counter = 0;
        }
        self.standing = true;
    }
}
