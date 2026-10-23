//! `Player.MovementUpdate`: ground classification, standing toggles, run force, beam
//! grab and jump dispatch. Runs after physics every tick.
//!
//! Behaviour: `slugcat-movement.md` sections 3-4, `horizontal-beams.md` section 1,
//! `vertical-poles.md` sections 0-1, `ledge-grab.md` section 1, `corridors.md` section 1.
//!
//! Mirrors: Player.cs (MovementUpdate)

use rw_assets::{Room, ShortcutType, Terrain};
use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{SoundId, Sounds, Vec2, Water};

/// The four neighbour directions as `Custom.fourDirections`.
const FOUR_DIRECTIONS: [(f32, f32); 4] = [(-1.0, 0.0), (0.0, -1.0), (1.0, 0.0), (0.0, 1.0)];

impl Player {
    pub(super) fn movement_update(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        self.direct_into_holes(room);
        if self.animation != Animation::RocketJump {
            self.rocket_jump_from_belly_slide = false;
        }
        if self.animation != Animation::Flip {
            self.flip_from_slide = false;
        }
        if self.animation != Animation::BellySlide {
            self.whiplash_jump = false;
        }

        let mut run_dir = self.input_x();
        if self.jump_stun != 0 {
            run_dir = self.jump_stun.signum();
        }
        self.last_flip_direction = self.flip_direction;
        if run_dir != 0 && run_dir != self.flip_direction {
            self.flip_direction = run_dir;
        }

        // The pairs are counted before the feet pin moves the hips, the rest after.
        let pairs = self.corridor_pairs(room);
        self.update_feet_stuck_pos(room);
        if self.in_corridor(room, pairs) {
            self.go_into_corridor_climb += 1;
        } else {
            self.go_into_corridor_climb = 0;
            self.classify_ground(room);
        }
        self.squeeze_up_through_gap(room);
        self.toggle_standing(room);
        self.leave_zero_g_pole(room);
        self.enter_corridor_mode(room);
        self.classify_swimming(room, water);
        self.enter_zero_g(room);

        self.dynamic_run_speed = [3.6, 3.6];
        self.update_animation(room, water, rng, sounds);
        self.trigger_ledge_grab(room);
        if self.body_mode == BodyMode::Crawl {
            self.crawl_turn_delay += 1;
        } else {
            self.crawl_turn_delay = 0;
        }
        if self.standing && self.is_tile_solid(room, HIPS, 0, 1) {
            self.standing = false;
        }
        self.trigger_beam_tip(room);
        self.update_body_mode(room, water);
        // A roll or belly slide runs toward its direction whatever x is held.
        if self.roll_direction != 0 {
            run_dir = self.roll_direction;
        }
        self.count_roll();
        self.grab_beam(room, water);
        self.heavy_carry_movement(room);
        self.apply_run_force(run_dir);
        self.dispatch_jump(room);
        self.enter_shortcut_by_input(room);
        // GrabUpdate: not ported.
    }

    /// `DirectIntoHoles`: pushing straight into a one-tile gap steers the head into it.
    fn direct_into_holes(&mut self, room: &Room) {
        let head = self.body.chunks[HEAD].pos;
        if FOUR_DIRECTIONS
            .iter()
            .any(|&(dx, dy)| room.is_solid_at(head + Vec2::new(dx, dy) * 20.0))
        {
            return;
        }
        let (x, y) = (self.input_x(), self.input_y());
        if (x == 0) == (y == 0) {
            return;
        }
        let step = Vec2::new(x as f32, y as f32) * 20.0;
        let target = head + step;
        if room.is_solid_at(target) {
            return;
        }
        let walls_x = [-1.0, 1.0]
            .iter()
            .all(|&s| room.is_solid_at(target + Vec2::new(20.0 * s, 0.0)));
        let walls_y = [-1.0, 1.0]
            .iter()
            .all(|&s| room.is_solid_at(target + Vec2::new(0.0, 20.0 * s)));
        if !walls_x && !walls_y {
            return;
        }
        // Only Normal and RoomExit shortcuts draw the player in.
        let beyond = head + step * 2.0;
        if room.tile_at_pos(beyond).terrain == Terrain::ShortcutEntrance
            && !matches!(
                room.shortcut_map().type_at(beyond.tile()),
                ShortcutType::Normal | ShortcutType::RoomExit
            )
        {
            return;
        }
        self.body.chunks[HEAD].vel += (target.tile_middle() - head) * 0.1;
    }

    /// The anti-slide pin of the hips while standing still (`feetStuckPos`).
    fn update_feet_stuck_pos(&mut self, room: &Room) {
        let friction = self.body.params.surface_friction;
        let hips = &self.body.chunks[HIPS];
        let stuck =
            hips.on_slope() == 0 && self.input_x() == 0 && self.standing && hips.contact().y == -1;
        if self.feet_stuck_pos.is_some() && !stuck {
            self.feet_stuck_pos = None;
        } else if self.feet_stuck_pos.is_none() && stuck {
            let floor_y = hips.pos.tile_middle().y - 10.0 + hips.rad;
            self.feet_stuck_pos = Some(Vec2::new(hips.pos.x, floor_y));
        }
        let Some(mut pin) = self.feet_stuck_pos else {
            return;
        };
        let hips = &mut self.body.chunks[HIPS];
        pin.x += (hips.pos.x - pin.x) * (1.0 - friction);
        hips.pos = pin;
        self.feet_stuck_pos = Some(pin);
        if self.is_tile_solid(room, HIPS, 0, -1) {
            return;
        }
        // Over a hole: slide toward the side that has ground, or let go.
        let ledge_right =
            self.is_tile_solid(room, HIPS, 1, -1) && !self.is_tile_solid(room, HIPS, 1, 0);
        let ledge_left =
            self.is_tile_solid(room, HIPS, -1, -1) && !self.is_tile_solid(room, HIPS, -1, 0);
        self.feet_stuck_pos = match (ledge_left, ledge_right) {
            (true, false) => Some(pin - Vec2::new(1.6 * friction, 0.0)),
            (false, true) => Some(pin + Vec2::new(1.6 * friction, 0.0)),
            _ => None,
        };
    }

    /// Body mode from contacts (Default / Stand / Crawl), the airborne jump boost,
    /// wall-climb and ledge-crawl detection and the squeeze-under-lip pop.
    fn classify_ground(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let grounded = head.contact().y == -1 || hips.contact().y == -1;
        self.body_mode = BodyMode::Default;
        if grounded {
            self.can_jump = 5;
            let upright = head.pos.y > hips.pos.y + 3.0
                && !self.is_tile_solid(room, HIPS, 0, 1)
                && self.animation != Animation::CrawlTurn
                && head.contact().y > -1;
            self.body_mode = if upright {
                BodyMode::Stand
            } else {
                BodyMode::Crawl
            };
        } else if self.jump_boost > 0.0
            && (self.input[0].jump || self.simulate_hold_jump_button > 0)
        {
            self.jump_boost -= 1.5;
            let add = (self.jump_boost + 1.0) * 0.3;
            for c in &mut self.body.chunks {
                c.vel.y += add;
            }
        } else {
            self.jump_boost = 0.0;
        }

        let head = &self.body.chunks[HEAD];
        let head_contact_x = i32::from(head.contact().x);
        if head_contact_x != 0 && head_contact_x == x {
            if i32::from(head.last_contact().x) != x {
                self.play_sound(SoundId::SlugcatEnterWallSlide, HEAD, 1.0, 1.0);
            }
            self.body_mode = BodyMode::WallClimb;
        }
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        if x != 0
            && head.pos.y > hips.pos.y
            && self.animation != Animation::CrawlTurn
            && !self.is_tile_solid(room, HEAD, x, 0)
            && self.is_tile_solid(room, HIPS, x, 0)
            && i32::from(hips.contact().x) == x
        {
            self.body_mode = BodyMode::Crawl;
            self.animation = Animation::LedgeCrawl;
        }
        if y == 1
            && self.is_tile_solid(room, HEAD, 0, 1)
            && !self.is_tile_solid(room, HIPS, 0, 1)
            && (self.is_tile_solid(room, HIPS, -1, 1) || self.is_tile_solid(room, HIPS, 1, 1))
        {
            self.animation = Animation::None;
            let eg = self.effective_room_gravity();
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            hips.vel.y += 2.0 * eg;
            head.vel.x -= (head.pos.x - hips.pos.x) * 0.25 * eg;
            head.vel.y -= eg;
        }
    }

    /// Up held under a one-tile gap above the head: centre the head under it and pop up.
    fn squeeze_up_through_gap(&mut self, room: &Room) {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let (hx, hy) = self.tile_pos(HIPS);
        let under_gap = self.input_y() > 0
            && self.input_x() == 0
            && self.body_mode == BodyMode::Default
            && head.pos.y - head.last_pos.y < 2.0
            && hips.contact().y == 0
            && !self.is_tile_solid(room, HEAD, 0, 1)
            && self.is_tile_solid(room, HEAD, -1, 1)
            && self.is_tile_solid(room, HEAD, 1, 1)
            && !self.is_tile_solid(room, HIPS, -1, 0)
            && !self.is_tile_solid(room, HIPS, 1, 0)
            && self.tile_pos(HEAD) == (hx, hy + 1)
            && (head.pos.x - head.pos.tile_middle().x).abs() < 5.0
            && self.effective_room_gravity() > 0.0;
        if !under_gap {
            return;
        }
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.pos.x = head.pos.tile_middle().x;
        head.pos.y += 1.0;
        head.vel.y += 1.0;
        hips.vel.y += 1.0;
        hips.pos.y += 1.0;
    }

    /// Up press stands up (not on a slope under a ceiling); down press goes to all fours.
    fn toggle_standing(&mut self, room: &Room) {
        let (now, prev) = (self.input[0].y, self.input[1].y);
        if now == 1 && prev != 1 {
            if self.body.chunks[HIPS].on_slope() == 0 || !self.is_tile_solid(room, HEAD, 0, 1) {
                self.standing = true;
            }
        } else if now == -1 && prev != -1 {
            if self.standing && self.body_mode == BodyMode::Stand {
                self.play_sound(SoundId::SlugcatDownOnFours, HEAD, 1.0, 1.0);
            }
            self.standing = false;
        }
    }

    /// Holding up (not mostly under water) grabs a horizontal beam whose tile the head
    /// passed through this tick, then a vertical pole (which overrides it), then a pole
    /// end above the head.
    pub(super) fn grab_beam(&mut self, room: &Room, water: Option<&Water>) {
        let want_to_grab = self.input_y() > 0 && self.body.submersion(water) <= 0.9;
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let can_grab = want_to_grab
            && self.no_grab_counter == 0
            && matches!(
                self.body_mode,
                BodyMode::Default
                    | BodyMode::WallClimb
                    | BodyMode::Stand
                    | BodyMode::ClimbingOnBeam
                    | BodyMode::Swimming
            )
            && (self.time_since_in_corridor_mode >= 20
                || hips.pos.y <= head.pos.y
                || head.pos.tile().0 != hips.pos.tile().0)
            && !matches!(
                self.animation,
                Animation::ClimbOnBeam
                    | Animation::HangFromBeam
                    | Animation::GetUpOnBeam
                    | Animation::DeepSwim
                    | Animation::HangUnderVerticalBeam
                    | Animation::GetUpToBeamTip
            );
        if !can_grab {
            return;
        }
        let x = head.pos.tile().0;
        let (y0, y1) = (head.last_pos.tile().1, head.pos.tile().1);
        let beam_row = (y0.min(y1)..=y0.max(y1))
            .rev()
            .find(|&r| room.tile(x, r).horizontal_pole());
        if let Some(row) = beam_row {
            self.animation = Animation::HangFromBeam;
            self.play_sound(SoundId::SlugcatGrabBeam, HEAD, 1.0, 1.0);
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y = 0.0;
            hips.vel.y *= 0.25;
            head.pos.y = rw_assets::tile_centre(x, row)[1];
        }
        self.grab_vertical_pole(room);
        self.grab_under_vertical_pole(room);
    }

    /// The horizontal run force toward `dynamic_run_speed` and ground friction.
    fn apply_run_force(&mut self, run_dir: i32) {
        if self.body_mode == BodyMode::CorridorClimb
            || matches!(
                self.animation,
                Animation::ClimbOnBeam
                    | Animation::HangFromBeam
                    | Animation::GetUpOnBeam
                    | Animation::LedgeGrab
            )
            || (self.dynamic_run_speed[0] <= 0.0 && self.dynamic_run_speed[1] <= 0.0)
        {
            return;
        }
        let friction = self.body.params.surface_friction;
        let mut accel = 2.4;
        if self.slow_movement_stun > 0 {
            let t = (self.slow_movement_stun as f32).inverse_lerp(10.0, 0.0);
            for cap in &mut self.dynamic_run_speed {
                *cap *= 0.5 + 0.5 * t;
            }
            accel *= 0.4 + 0.6 * t;
        }
        let no_contact = self
            .body
            .chunks
            .iter()
            .all(|c| c.contact().x == 0 && c.contact().y == 0);
        if self.body_mode == BodyMode::Default && no_contact {
            accel *= self.effective_room_gravity();
        }
        let vertical_contact = self.body.chunks.iter().any(|c| c.contact().y != 0);
        let steering = self.input_x() != 0;
        let ground_blend = friction.powf(1.5);
        for (c, &cap) in self.body.chunks.iter_mut().zip(&self.dynamic_run_speed) {
            if run_dir != 0 {
                let dir = run_dir as f32;
                let room_left = cap - c.vel.x * dir;
                let push = (accel * friction).min(room_left);
                if push > 0.0 {
                    c.vel.x += push * dir;
                }
            }
            if vertical_contact {
                let target = if steering {
                    c.vel.x.clamp(-cap, cap)
                } else {
                    0.0
                };
                c.vel.x += (target - c.vel.x) * ground_blend;
            }
        }
    }

    /// Crouch super-jump charge, then `Jump` / `WallJump` when the buffers line up.
    fn dispatch_jump(&mut self, room: &Room) {
        let mut spend_charge =
            self.super_launch_jump > 0 && self.kill_super_launch_jump_counter < 1;
        let crouched_on_ground =
            self.body_mode == BodyMode::Crawl && self.body.chunks.iter().all(|c| c.contact().y < 0);
        if crouched_on_ground {
            if self.input_x() == 0 && self.input_y() == 0 {
                spend_charge = false;
                self.want_to_jump = 0;
                if self.input[0].jump {
                    if self.super_launch_jump < 20 {
                        self.super_launch_jump += 1;
                    } else {
                        self.kill_super_launch_jump_counter = 15;
                    }
                }
            }
            if !self.input[0].jump && self.input[1].jump {
                self.want_to_jump = 1;
            }
        }
        if self.kill_super_launch_jump_counter > 0 {
            self.kill_super_launch_jump_counter -= 1;
        }
        if self.simulate_hold_jump_button > 0 {
            self.simulate_hold_jump_button -= 1;
        }

        if self.can_jump > 0 && self.want_to_jump > 0 {
            self.can_jump = 0;
            self.want_to_jump = 0;
            self.jump(room);
        } else if self.can_wall_jump != 0
            && self.want_to_jump > 0
            && self.input_x() != -self.can_wall_jump.signum()
        {
            self.wall_jump(room, self.can_wall_jump.signum());
            self.want_to_jump = 0;
        } else if self.jump_chunk_counter > 0 && self.want_to_jump > 0 {
            self.jump_chunk_counter = -5;
            self.want_to_jump = 0;
            self.jump_on_chunk();
        }
        if spend_charge {
            self.super_launch_jump -= 1;
        }
    }
}
