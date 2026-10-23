//! `Player.UpdateAnimation` for the ported animations.
//!
//! Behaviour: `slugcat-movement.md` sections 6 and 9, `horizontal-beams.md` sections 2-4.
//! Vertical poles are in `poles.rs`, `LedgeGrab` in `ledge.rs`, `Flip`, `Roll`,
//! `RocketJump` and `BellySlide` in `advanced.rs`, `CorridorTurn` in `corridor.rs`,
//! `DeepSwim` and `SurfaceSwim` in `swim.rs`.
//!
//! Mirrors: Player.cs (UpdateAnimation)

use rw_assets::{Room, Terrain};
use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{NoiseSink, SoundId, Sounds, Vec2, Water};

impl Player {
    pub(super) fn update_animation(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        noises: &mut dyn NoiseSink,
    ) {
        if self.long_belly_slide && self.animation != Animation::BellySlide {
            self.long_belly_slide = false;
        }
        if self.animation != Animation::Roll {
            self.stop_rolling_counter = 0;
        }
        if self.animation != Animation::ClimbOnBeam {
            self.slide_up_pole = 0;
        }
        match self.animation {
            Animation::None => {}
            Animation::CrawlTurn => self.crawl_turn(room),
            Animation::StandUp => self.stand_up(room),
            Animation::DownOnFours => self.down_on_fours(),
            Animation::LedgeCrawl => self.ledge_crawl(room),
            Animation::HangFromBeam => self.hang_from_beam(room),
            Animation::GetUpOnBeam => self.get_up_on_beam(room),
            Animation::StandOnBeam => self.stand_on_beam(room),
            Animation::LedgeGrab => self.ledge_grab(room),
            Animation::ClimbOnBeam => self.climb_on_beam(room),
            Animation::GetUpToBeamTip => self.get_up_to_beam_tip(room),
            Animation::BeamTip => self.beam_tip(room),
            Animation::HangUnderVerticalBeam => self.hang_under_vertical_beam(room),
            Animation::Roll => self.roll(room),
            Animation::RocketJump => self.rocket_jump(),
            Animation::Flip => self.flip(),
            Animation::BellySlide => self.belly_slide(room),
            Animation::CorridorTurn => self.corridor_turn(rng, sounds),
            Animation::DeepSwim => self.deep_swim(water, rng, sounds),
            Animation::SurfaceSwim => self.surface_swim(water),
            Animation::ZeroGSwim => self.zero_g_swim(room, rng, sounds),
            Animation::ZeroGPoleGrab => self.zero_g_pole_grab(room),
            Animation::VineGrab => self.vine_grab_animation(rng, sounds, noises),
            Animation::Dead => {}
        }
    }

    fn crawl_turn(&mut self, room: &Room) {
        self.body_mode = BodyMode::Default;
        let flip = self.flip_direction as f32;
        let x = self.input_x();
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel.x += flip;
        hips.vel.x -= 2.0 * flip;
        let mut done = false;
        if (x > 0) != (head.pos.x < hips.pos.x) {
            head.vel.y -= 3.0;
            if head.pos.y < hips.pos.y + 2.0 {
                done = true;
                head.vel.y -= 1.0;
            }
        } else {
            head.vel.y += 2.0;
        }
        if done || x == 0 || self.is_tile_solid(room, HIPS, 0, 1) {
            self.animation = Animation::None;
        }
    }

    fn stand_up(&mut self, room: &Room) {
        if !self.standing {
            self.animation = Animation::DownOnFours;
            return;
        }
        self.body.chunks[HEAD].vel.x *= 0.7;
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        if !self.is_tile_solid(room, HEAD, 0, 1) && (hips.on_slope() == 0 || self.input_x() == 0) {
            self.body_mode = BodyMode::Stand;
            if head.pos.y > hips.pos.y + 3.0 {
                self.animation = Animation::None;
                self.play_sound(SoundId::SlugcatRegainFooting, HIPS, 1.0, 1.0);
            }
        } else {
            self.animation = Animation::None;
        }
    }

    fn down_on_fours(&mut self) {
        if self.standing {
            self.animation = Animation::StandUp;
            return;
        }
        let flip = self.flip_direction as f32;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel.y -= 2.0;
        head.vel.x += flip;
        hips.vel.x -= flip;
        if head.pos.y < hips.pos.y || head.contact().y == -1 {
            self.animation = Animation::None;
        }
    }

    fn ledge_crawl(&mut self, room: &Room) {
        let flip = self.flip_direction;
        self.body.chunks[HEAD].vel.x += flip as f32 * 2.0;
        self.body_mode = BodyMode::Crawl;
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        // The game's "same row" test compares the head's tile row with itself, so it
        // always holds; kept as in the game.
        let on_flat =
            self.is_tile_solid(room, HEAD, 0, -1) && self.is_tile_solid(room, HIPS, 0, -1);
        let pushing_wall = i32::from(head.contact().x) == flip && self.input_x() != 0;
        let airborne = head.contact().y > -1 && hips.contact().y > -1;
        if !self.is_tile_solid(room, HEAD, flip, 0) && (on_flat || pushing_wall || airborne) {
            self.animation = Animation::None;
        }
    }

    fn hang_from_beam(&mut self, room: &Room) {
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.standing = true;
        let x = self.input_x();
        let flip = self.flip_direction as f32;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel.y = 0.0;
        head.vel.x *= 0.2;
        head.pos.y = head.pos.tile_middle().y;
        if x != 0 && i32::from(head.contact().x) != x {
            if i32::from(hips.contact().x) != x {
                head.vel.x += x as f32
                    * 1.2
                    * self.stats.pole_climb_speed_fac
                    * (self.slow_movement_stun as f32).lerp_map(0.0, 10.0, 1.0, 0.5);
            }
            self.animation_frame += 1;
            if self.animation_frame > 20 {
                self.animation_frame = 1;
                self.play_sound(SoundId::SlugcatClimbAlongHorizontalBeam, HEAD, 1.0, 1.0);
                self.aerobic_increase(0.05);
            }
            let hips = &mut self.body.chunks[HIPS];
            let phase = self.animation_frame as f32 / 20.0 * std::f32::consts::TAU;
            hips.vel.x += flip * (0.5 + 0.5 * phase.sin()) * -0.5;
        } else {
            self.animation_frame += (10 - self.animation_frame).signum();
        }

        let (now, prev) = (self.input[0], self.input[1]);
        let mut pull_up = false;
        if now.y < 0 && prev.y == 0 {
            self.animation = Animation::None;
        } else if now.y > 0 && prev.y == 0 {
            let head = self.body.chunks[HEAD].pos;
            if room.tile_at_pos(head).vertical_pole() {
                self.animation = Animation::ClimbOnBeam;
                self.flip_direction = if head.x < head.tile_middle().x { -1 } else { 1 };
            } else {
                pull_up = true;
            }
        } else if now.jump && !prev.jump {
            pull_up = true;
        }
        if pull_up {
            self.start_pull_up(room);
        }
        if !room
            .tile_at_pos(self.body.chunks[HEAD].pos)
            .horizontal_pole()
        {
            self.animation = Animation::None;
        }
    }

    /// Choose the side to swing the hips up on (or straight up) and start `GetUpOnBeam`.
    fn start_pull_up(&mut self, room: &Room) {
        self.play_sound(SoundId::SlugcatGetUpOnHorizontalBeam, HEAD, 1.0, 1.0);
        self.animation = Animation::GetUpOnBeam;
        self.pullup_softlock_safety = 0;
        self.straight_up_on_horizontal_beam = false;
        let head = self.body.chunks[HEAD].pos;
        if !self.beam_continues_to(room, head, self.flip_direction) {
            self.flip_direction = -self.flip_direction;
        }
        if !self.beam_continues_to(room, head, self.flip_direction) {
            self.flip_direction = -self.flip_direction;
            self.straight_up_on_horizontal_beam = true;
        }
        let above_side = head + Vec2::new(self.flip_direction as f32 * 20.0, 20.0);
        if !self.straight_up_on_horizontal_beam && room.is_solid_at(above_side) {
            self.straight_up_on_horizontal_beam = true;
        }
        self.up_on_horizontal_beam_pos = Vec2::new(head.x, head.tile_middle().y + 20.0);
    }

    /// The tile one step to `side` of `pos` is a non-Solid horizontal beam tile.
    fn beam_continues_to(&self, room: &Room, pos: Vec2, side: i32) -> bool {
        let tile = room.tile_at_pos(pos + Vec2::new(side as f32 * 20.0, 0.0));
        tile.terrain != Terrain::Solid && tile.horizontal_pole()
    }

    fn get_up_on_beam(&mut self, room: &Room) {
        self.pullup_softlock_safety += 1;
        if self.pullup_softlock_safety > 200 {
            self.play_sound(SoundId::SlugcatTurnInCorridor, HEAD, 1.0, 1.0);
            self.pullup_softlock_safety = 0;
            self.animation = Animation::None;
            return;
        }
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.body.chunks[HEAD].vel = Vec2::ZERO;
        self.force_feet_to_horizontal_beam_tile = 20;
        if self.straight_up_on_horizontal_beam {
            self.get_up_straight(room);
        } else {
            self.get_up_sideways(room);
        }
    }

    fn get_up_straight(&mut self, room: &Room) {
        let flip = self.flip_direction as f32;
        if self.input_y() < 0 || self.body.chunks[HEAD].contact().y > 0 {
            self.straight_up_on_horizontal_beam = false;
        }
        let mut target = self.up_on_horizontal_beam_pos;
        if room.is_solid_at(target) {
            for side in [1.0, -1.0] {
                let shifted = target + Vec2::new(flip * side * 20.0, 0.0);
                if !room.is_solid_at(shifted) {
                    target = shifted;
                    break;
                }
            }
            self.up_on_horizontal_beam_pos = target;
        }
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel += head.pos.direction_to(target) * 1.8;
        hips.vel += hips.pos.direction_to(target - Vec2::new(0.0, 20.0)) * 1.8;
        let hips_on_beam = room.tile_at_pos(hips.pos).horizontal_pole();
        if hips_on_beam && hips.pos.y > target.y - 25.0 {
            self.no_grab_counter = 15;
            self.animation = Animation::StandOnBeam;
            hips.pos.y = hips.pos.tile_middle().y + 5.0;
            hips.vel.y = 0.0;
        } else if (!room.tile_at_pos(head.pos).horizontal_pole() && !hips_on_beam)
            || head.pos.distance(target) >= 25.0
        {
            self.animation = Animation::None;
        }
    }

    fn get_up_sideways(&mut self, room: &Room) {
        let flip = self.flip_direction as f32;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.pos.y = head.pos.tile_middle().y;
        hips.vel.y += 2.0;
        hips.vel.x += flip * 0.5;
        // Keep the body over the beam's end.
        let side = (hips.pos.x - head.pos.x).signum();
        if hips.pos.y > head.pos.y - 15.0
            && !room
                .tile_at_pos(head.pos + Vec2::new(side * 35.0, 0.0))
                .horizontal_pole()
            && room
                .tile_at_pos(head.pos + Vec2::new(side * -15.0, 0.0))
                .horizontal_pole()
        {
            head.vel.x -= side * 1.5;
            hips.vel.x -= side * 0.5;
        }
        if hips.contact().y > 0 {
            if room.is_solid_at(head.pos + Vec2::new(0.0, 20.0)) {
                self.animation = Animation::HangFromBeam;
            } else {
                self.straight_up_on_horizontal_beam = true;
            }
        }
        if hips.pos.y > head.pos.y {
            self.no_grab_counter = 15;
            self.animation = Animation::StandOnBeam;
            // The head's tile, not the hips' (the hips may still be in the row below).
            hips.pos.y = head.pos.tile_middle().y + 5.0;
            hips.vel.y = 0.0;
        }
        if !room.tile_at_pos(head.pos).horizontal_pole() {
            self.pullup_softlock_safety = 0;
            self.animation = Animation::None;
        }
    }

    fn stand_on_beam(&mut self, room: &Room) {
        self.body_mode = BodyMode::ClimbingOnBeam;
        self.standing = true;
        self.can_jump = 5;
        self.body.chunks[HIPS].vel.x *= 0.5;
        let head_free = self.body.chunks[HEAD].contact().y < 1;
        if head_free || !self.is_tile_solid(room, HIPS, 0, 1) {
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            hips.vel.y = 0.0;
            hips.pos.y = hips.pos.tile_middle().y + 5.0;
            head.vel.y += 2.0;
            self.dynamic_run_speed = [2.1 * self.stats.runspeed_fac; 2];
            if self.input[0].y < 0 && self.input[1].y == 0 {
                self.animation = Animation::None;
            }
        } else {
            self.animation = Animation::None;
        }
        if self.input_x() != 0 {
            self.animation_frame += 1;
        } else {
            self.animation_frame = 0;
        }
        if self.animation_frame > 6 {
            self.animation_frame = 0;
            self.play_sound(SoundId::SlugcatWalkOnHorizontalBeam, HEAD, 1.0, 1.0);
        }
        let (hx, hy) = self.tile_pos(HEAD);
        if self.input[0].y == 1 && self.input[1].y == 0 && room.tile(hx, hy + 1).horizontal_pole() {
            for c in &mut self.body.chunks {
                c.pos.y += 8.0;
            }
            self.animation = Animation::HangFromBeam;
        }
    }
}
