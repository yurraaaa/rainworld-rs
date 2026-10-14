//! `Player.UpdateBodyMode`: per-mode forces and run-speed caps.
//!
//! Behaviour: `slugcat-movement.md` section 6, `slope-collision.md` section 5,
//! `horizontal-beams.md` section 3 (foot assist).

use rw_assets::{Room, Terrain};

use super::{Animation, BodyMode, HEAD, HIPS, Player, ROOM_GRAVITY, RUN_SPEED_FAC};
use crate::Vec2;
use crate::room_ext::RoomExt;

impl Player {
    pub(super) fn update_body_mode(&mut self, room: &Room) {
        self.dive_force = (self.dive_force - 0.05).max(0.0);
        self.count_ground_frames();
        match self.body_mode {
            BodyMode::Default => self.default_mode(room),
            BodyMode::Crawl => self.crawl_mode(room),
            BodyMode::Stand => self.stand_mode(room),
            BodyMode::WallClimb => self.wall_climb_mode(room),
            BodyMode::ClimbingOnBeam => self.climbing_on_beam_mode(room),
            // Set only while `movement_update` is skipped.
            BodyMode::Stunned | BodyMode::Dead => {}
        }
    }

    fn count_ground_frames(&mut self) {
        if self.body.chunks[HEAD].contact().y < 0 {
            self.upper_body_frames_on_ground += 1;
            self.upper_body_frames_off_ground = 0;
        } else {
            self.upper_body_frames_on_ground = 0;
            self.upper_body_frames_off_ground += 1;
        }
        if self.body.chunks[HIPS].contact().y < 0 {
            self.lower_body_frames_on_ground += 1;
            self.lower_body_frames_off_ground = 0;
        } else {
            self.lower_body_frames_on_ground = 0;
            self.lower_body_frames_off_ground += 1;
        }
    }

    /// Airborne: down drops through floors, `standing` keeps the body upright, air caps.
    fn default_mode(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        let no_contact = self
            .body
            .chunks
            .iter()
            .all(|c| c.contact().x == 0 && c.contact().y == 0);
        // A roll with x held keeps floors solid.
        if y < 0 && (self.animation != Animation::Roll || x == 0) {
            self.body.set_go_through_floors(true);
            if self.input[0].down_diagonal() != 0 && self.consistent_down_diagonal > 6 && no_contact
            {
                self.keep_floor_next_to_wall(room, x);
            }
        }
        if self.standing {
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y += 4.0 * ROOM_GRAVITY;
            hips.vel.y -= 4.0 * ROOM_GRAVITY;
            self.dynamic_run_speed = [4.2 * RUN_SPEED_FAC, 4.0 * RUN_SPEED_FAC];
            if y != 0 {
                self.dynamic_run_speed[1] = 2.0;
            }
            let head_tile = self.tile_pos(HEAD);
            if y > 0
                && !self.is_tile_solid(room, HEAD, 0, 1)
                && self.is_tile_solid(room, HEAD, -1, 1)
                && self.is_tile_solid(room, HEAD, 1, 1)
            {
                // Centre the head under a one-tile gap above.
                let mid_x = rw_assets::tile_centre(head_tile.0, head_tile.1)[0];
                let head = &mut self.body.chunks[HEAD];
                head.vel.x = head.vel.x * 0.8 - (head.pos.x - mid_x) * 0.4;
            }
        } else {
            let mut cap = if y != 0 { 2.5 } else { 4.0 };
            if self.animation == Animation::CrawlTurn {
                cap *= 0.75;
            }
            self.dynamic_run_speed = [cap, cap];
        }
        if y < 0 && no_contact && self.body.chunks[HEAD].vel.y < -6.0 {
            self.dive_force = (self.dive_force + 1.0 / 7.0).min(1.0);
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y -= 1.2 * self.dive_force;
            hips.vel.y += 1.2 * self.dive_force;
        }
        // `dropGrabTile` (corridor drop onto a pole) is never set: corridors are not ported.
    }

    /// Diagonal-down guard: falling toward a one-way floor that ends against a wall in
    /// the held direction keeps that floor solid, so the player lands next to the wall.
    fn keep_floor_next_to_wall(&mut self, room: &Room, x: i32) {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let lower = if head.pos.y < hips.pos.y {
            head.pos
        } else {
            hips.pos
        };
        let (tx, mut ty) = lower.tile();
        for _ in 0..5 {
            ty -= 1;
            match room.tile(tx, ty).terrain {
                Terrain::Solid => break,
                Terrain::Floor => {
                    if room.tile(tx + x, ty).terrain == Terrain::Solid
                        && !room.tile(tx + x, ty + 1).is_solid()
                    {
                        self.body.set_go_through_floors(false);
                    }
                    break;
                }
                _ => {}
            }
        }
    }

    /// On all fours on the ground.
    fn crawl_mode(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let backwards = (x > 0) == (head.pos.x < hips.pos.x);
        let on_slope = head.on_slope() != 0 || hips.on_slope() != 0;
        let mut cap = if y != 0 { 1.0 } else { 2.5 };
        if backwards && !on_slope {
            cap *= 0.75;
            if self.crawl_turn_delay > 5
                && !self.is_tile_solid(room, HEAD, 0, 1)
                && !self.is_tile_solid(room, HIPS, 0, 1)
                && x != 0
            {
                self.crawl_turn_delay = 0;
                if !self.is_tile_solid(room, HEAD, -1, 1) || !self.is_tile_solid(room, HEAD, 1, 1) {
                    self.animation = Animation::CrawlTurn;
                }
            }
        }
        // The hips' cap is copied before the slope factor, so only the head's is scaled.
        self.dynamic_run_speed = [cap, cap];
        if on_slope {
            self.dynamic_run_speed[0] *= if backwards { 0.5 } else { 0.75 };
            for c in &mut self.body.chunks {
                if c.on_slope() != 0 {
                    c.vel.y -= 1.5;
                }
            }
        }

        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        if head.contact().y > -1
            && x != 0
            && hips.pos.y < head.pos.y - 3.0
            && i32::from(hips.contact().x) == x
        {
            self.body.chunks[HIPS].pos.y += 1.0;
        }
        if y < 0 {
            self.body.set_go_through_floors(true);
            self.centre_over_hole(room);
        }
        if self.standing && self.ready_to_stand_up(room) {
            self.animation = Animation::StandUp;
            if x == 0 {
                self.pin_feet_for_stand_up(room);
            }
        }
        let hips = &self.body.chunks[HIPS];
        // Dead in practice: a chunk on a slope always has floor contact.
        if hips.on_slope() != 0 && hips.contact().y > -1 && x == 0 {
            self.body.chunks[HEAD].vel.x += f32::from(hips.on_slope());
        }
        let hips = &self.body.chunks[HIPS];
        if x != 0 && (hips.pos.x - hips.last_pos.x).abs() > 0.5 {
            self.animation_frame += 1;
        } else {
            self.animation_frame = 0;
        }
        if self.animation_frame > 10 {
            self.animation_frame = 0;
        }
    }

    /// Holding down while crawling over a one-tile hole pulls the first chunk over it
    /// toward the hole's middle.
    fn centre_over_hole(&mut self, room: &Room) {
        for i in [HEAD, HIPS] {
            if !self.is_tile_solid(room, i, 0, -1)
                && (self.is_tile_solid(room, i, -1, -1) || self.is_tile_solid(room, i, 1, -1))
            {
                let c = &mut self.body.chunks[i];
                let mid_x = c.pos.tile_middle().x;
                c.vel.x = c.vel.x * 0.8 - (c.pos.x - mid_x) * 0.4;
                c.vel.y -= 1.0;
                break;
            }
        }
    }

    /// Hips on the ground for 3+ ticks, or both chunks over non-Air ground.
    fn ready_to_stand_up(&self, room: &Room) -> bool {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let (hx, hy) = hips.pos.tile();
        let (gx, gy) = head.pos.tile();
        self.lower_body_frames_on_ground >= 3
            || (hips.contact().y < 0
                && room.tile(hx, hy - 1).terrain != Terrain::Air
                && room.tile(gx, gy - 1).terrain != Terrain::Air)
    }

    fn pin_feet_for_stand_up(&mut self, room: &Room) {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        if hips.contact().y == -1
            && self.is_tile_solid(room, HIPS, 0, -1)
            && !self.is_tile_solid(room, HIPS, 0, 1)
        {
            self.feet_stuck_pos = Some(hips.pos.tile_middle() + Vec2::new(0.0, -10.0 + hips.rad));
        } else if head.contact().y == -1
            && self.is_tile_solid(room, HEAD, 0, -1)
            && !self.is_tile_solid(room, HEAD, 0, 1)
        {
            self.feet_stuck_pos = Some(head.pos + Vec2::new(0.0, -1.0));
        }
    }

    /// Upright on the ground: lift forces, auto-lowering, slope handling, caps.
    fn stand_mode(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        {
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y += 1.5 * ROOM_GRAVITY;
            hips.vel.y -= 4.5 * ROOM_GRAVITY;
        }
        if !self.standing
            && self.lower_body_frames_on_ground >= 5
            && self.upper_body_frames_off_ground >= 5
            && self.should_go_down_on_fours(room)
        {
            self.animation = Animation::DownOnFours;
        }
        self.dynamic_run_speed = [4.2 * RUN_SPEED_FAC, 4.0 * RUN_SPEED_FAC];
        if y != 0 {
            self.dynamic_run_speed[1] = 2.0;
        }
        let downhill = i32::from(self.body.chunks[HIPS].on_slope());
        if downhill != 0 {
            self.dynamic_run_speed[0] *= 0.75;
            self.dynamic_run_speed[1] *= 0.75;
            if self.body.chunks[HEAD].contact().y == 1 {
                self.standing = false;
            }
            let gravity = self.body.params.gravity;
            let hips = &mut self.body.chunks[HIPS];
            if x == downhill {
                hips.vel.y -= gravity;
            }
            hips.pos.y -= 2.0;
        }
        if x != 0 {
            if i32::from(self.body.chunks[HIPS].contact().x) == x {
                let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
                head.vel.x += x as f32;
                head.vel.y += 2.0 * ROOM_GRAVITY;
                hips.vel.y += 2.0 * ROOM_GRAVITY;
            }
            self.animation_frame += 1;
        } else {
            self.animation_frame = 0;
        }
        if self.animation_frame > 6 {
            self.animation_frame = 0;
        }
        self.skid();
    }

    /// Whether to lower to all fours; at a ledge edge it instead nudges the hips back
    /// (or turns around) so the player does not walk off.
    fn should_go_down_on_fours(&mut self, room: &Room) -> bool {
        let flip = self.flip_direction;
        let (hx, hy) = self.tile_pos(HIPS);
        let at_wall_top = self.tile_pos(HEAD).1 == hy + 1
            && self.is_tile_solid(room, HIPS, flip, 0)
            && self.is_tile_solid(room, HEAD, flip, -1);
        if !at_wall_top {
            return true;
        }
        let mut blocked = [(0, 0), (0, -1), (flip, 0)]
            .iter()
            .all(|&(dx, dy)| room.tile(hx + dx, hy + dy).terrain != Terrain::Slope);
        if !blocked {
            blocked =
                self.is_tile_solid(room, HEAD, flip, 0) && self.is_tile_solid(room, HIPS, flip, 0);
        }
        if !blocked {
            return true;
        }
        if room.tile(hx - flip, hy).terrain == Terrain::Air {
            if room.tile(hx - flip, hy - 1).terrain != Terrain::Air {
                self.body.chunks[HIPS].vel.x += -1.1 * flip as f32;
            } else if self.input_x() != flip {
                if self.is_tile_solid(room, HEAD, flip, 1) {
                    self.flip_direction = -flip;
                } else if self.input[..4].iter().all(|i| i.y < 0) {
                    self.body.chunks[HIPS].vel.x += -0.4 * flip as f32;
                    self.body.chunks[HEAD].vel.y -= 1.0;
                }
            }
        }
        false
    }

    /// Head pressed against a wall: wall-jump window and slowing wall slide.
    fn wall_climb_mode(&mut self, room: &Room) {
        let x = self.input_x();
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        if head.pos.y > hips.pos.y {
            if x != 0 && x == i32::from(head.contact().x) {
                self.can_wall_jump = x * -15;
            }
            self.can_jump = 1;
        }
        if x != 0 && self.is_tile_solid(room, HEAD, x, 0) && self.is_tile_solid(room, HIPS, x, 0) {
            let t = (self.wall_slide_counter as f32 / 30.0).clamp(0.0, 1.0);
            let lift = self.body.params.gravity * (0.8 + (0.0 - 0.8) * t) * ROOM_GRAVITY;
            for c in &mut self.body.chunks {
                c.pos.y += lift;
            }
        }
        // Hips buoyancy when submerged: water is not ported.
    }

    /// On a beam or pole: keep the feet on horizontal beam tiles while getting up /
    /// standing; back to `Default` once the animation is no beam or pole one.
    fn climbing_on_beam_mode(&mut self, room: &Room) {
        let flip = self.flip_direction as f32;
        let on_beam_feet = matches!(
            self.animation,
            Animation::GetUpOnBeam | Animation::StandOnBeam
        );
        let hips_pos = self.body.chunks[HIPS].pos;
        if on_beam_feet && !room.tile_at_pos(hips_pos).horizontal_pole() {
            let ahead = hips_pos + Vec2::new(flip * 20.0, 0.0);
            let behind = hips_pos - Vec2::new(flip * 20.0, 0.0);
            let assist = self.force_feet_to_horizontal_beam_tile > 0;
            if assist && room.tile_at_pos(ahead).horizontal_pole() {
                let hips = &mut self.body.chunks[HIPS];
                hips.pos.x = ahead.tile_middle().x - flip * 8.0;
                hips.vel.x = 0.0;
                self.force_feet_to_horizontal_beam_tile = 20;
            } else if assist && room.tile_at_pos(behind).horizontal_pole() {
                let hips = &mut self.body.chunks[HIPS];
                hips.pos.x = behind.tile_middle().x + flip * 8.0;
                hips.vel.x = 0.0;
                self.force_feet_to_horizontal_beam_tile = 20;
            } else if !room
                .tile_at_pos(hips_pos + Vec2::new(0.0, 20.0))
                .horizontal_pole()
                || self.animation != Animation::GetUpOnBeam
            {
                self.animation = Animation::None;
            }
        }
        if !matches!(
            self.animation,
            Animation::BeamTip
                | Animation::ClimbOnBeam
                | Animation::GetUpOnBeam
                | Animation::HangFromBeam
                | Animation::StandOnBeam
                | Animation::GetUpToBeamTip
                | Animation::HangUnderVerticalBeam
        ) {
            self.body_mode = BodyMode::Default;
        }
    }
}
