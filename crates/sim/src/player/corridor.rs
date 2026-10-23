//! Corridors: detection (`goIntoCorridorClimb`), the `CorridorClimb` body mode with its
//! shoots, slides, U-turns and drops, the `CorridorTurn` animation and the
//! `dropGrabTile` catch.
//!
//! Behaviour: `corridors.md`, base game with Remix off (docs/divergences/ D10).
//!
//! Mirrors: Player.cs

use rw_assets::Room;
use rw_random::UnityRandom;

use super::drips::DripBurst;
use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{SoundId, Sounds, Vec2, Water};

/// `Custom.eightDirections`: W, SW, S, SE, E, NE, N, NW; entry `i + 4` is opposite `i`.
const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// Ticks of a corridor U-turn before the body is pushed out the new way.
const CORRIDOR_TURN_TICKS: i32 = 40;
/// Turn ticks with the random head jiggle; the rest push the ends apart.
const CORRIDOR_TURN_JIGGLE_TICKS: i32 = 30;

impl Player {
    /// Opposite-neighbour pairs of solid tiles around the head and hips (0..=8).
    pub(super) fn corridor_pairs(&self, room: &Room) -> i32 {
        let mut pairs = 0;
        for (dx, dy) in EIGHT_DIRECTIONS.into_iter().take(4) {
            for i in [HEAD, HIPS] {
                if self.is_tile_solid(room, i, dx, dy) && self.is_tile_solid(room, i, -dx, -dy) {
                    pairs += 1;
                }
            }
        }
        pairs
    }

    /// Wedged in a corridor: two or more opposite pairs off slopes with a chunk in a
    /// non-solid tile, or walls left and right of either chunk.
    pub(super) fn in_corridor(&self, room: &Room, pairs: i32) -> bool {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let squeezed = pairs > 1
            && head.on_slope() == 0
            && hips.on_slope() == 0
            && (!self.is_tile_solid(room, HEAD, 0, 0) || !self.is_tile_solid(room, HIPS, 0, 0));
        squeezed
            || [HEAD, HIPS]
                .into_iter()
                .any(|i| self.is_tile_solid(room, i, -1, 0) && self.is_tile_solid(room, i, 1, 0))
    }

    /// After three corridor ticks the mode switches to `CorridorClimb`, unless dropping
    /// out of a shaft, which keeps `Default` until up is released or ground is near.
    pub(super) fn enter_corridor_mode(&mut self, room: &Room) {
        if self.go_into_corridor_climb > 2 && !self.corridor_drop {
            self.body_mode = BodyMode::CorridorClimb;
            self.animation = if self.corridor_turn_dir.is_some() {
                Animation::CorridorTurn
            } else {
                Animation::None
            };
        }
        if !self.corridor_drop {
            return;
        }
        self.body_mode = BodyMode::Default;
        self.animation = Animation::None;
        if self.input_y() >= 0 || self.go_into_corridor_climb < 2 {
            self.corridor_drop = false;
        }
        let head = &self.body.chunks[HEAD];
        if head.pos.y < self.body.chunks[HIPS].pos.y {
            // The head's own tile (k = 0) counts too.
            let reach = ((head.vel.y * -0.3) as i32).clamp(1, 10);
            if (0..reach).any(|k| self.is_tile_solid(room, HEAD, 0, -k)) {
                self.corridor_drop = false;
            }
        }
    }

    /// `shootUpCounter`: while it runs no beam is grabbed; it stops when jump or up is
    /// released or the head moved down. Sees last tick's input.
    pub(super) fn count_shoot_up(&mut self) {
        if self.shoot_up_counter < 1 {
            return;
        }
        self.no_grab_counter = self.no_grab_counter.max(2);
        self.shoot_up_counter -= 1;
        let head = &self.body.chunks[HEAD];
        if !self.input[0].jump || self.input[0].y < 1 || head.pos.y < head.last_pos.y {
            self.shoot_up_counter = 0;
        }
    }

    /// The `CorridorClimb` branch of `UpdateBodyMode`.
    pub(super) fn corridor_climb_mode(&mut self, room: &Room, water: Option<&Water>) {
        self.body.set_go_through_floors(true);
        self.roll_direction = 0;
        if self.corridor_turn_dir.is_some() {
            let gravity = self.body.params.gravity();
            for c in &mut self.body.chunks {
                c.vel.y += gravity;
            }
            return;
        }
        self.update_drop_grab_tile(room);
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let shaft = (head.pos.x - hips.pos.x).abs() < 5.0
            && self.is_tile_solid(room, HEAD, -1, 0)
            && self.is_tile_solid(room, HEAD, 1, 0);
        let tunnel = (head.pos.y - hips.pos.y).abs() < 7.5
            && self.is_tile_solid(room, HEAD, 0, -1)
            && self.is_tile_solid(room, HEAD, 0, 1);
        if self.corridor_drop_or_turn(room, shaft, tunnel) {
            self.can_corridor_jump = 5;
        } else if self.can_corridor_jump > 0 {
            self.can_corridor_jump -= 1;
        }
        if self.jump_pressed() && self.slow_movement_stun < 1 {
            self.corridor_jump(shaft, tunnel);
        }
        if self.vertical_corridor_slide_counter == 1 || self.horizontal_corridor_slide_counter == 1
        {
            self.slow_movement_stun = 15;
        }
        self.corridor_damping(water);
        self.corridor_climb_force(room);
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        self.standing = head.pos.y > hips.pos.y + 5.0;
        if (self.input_x() != 0 || self.input_y() != 0) && self.vertical_corridor_slide_counter < 16
        {
            self.animation_frame += 1;
        } else {
            self.animation_frame = 0;
        }
        if self.animation_frame > 12 {
            self.animation_frame = 1;
            self.play_sound(SoundId::SlugcatInCorridorStep, HEAD, 1.0, 1.0);
        }
        self.leave_corridor_onto_beam(room);
    }

    /// Jump press this tick (direct edge, no buffer).
    fn jump_pressed(&self) -> bool {
        self.input[0].jump && !self.input[1].jump
    }

    /// Down held (no jump) with the lower end at a shaft's bottom mouth: remember the
    /// first beam tile at or below that end. Otherwise forget it.
    fn update_drop_grab_tile(&mut self, room: &Room) {
        let (head, hips) = (self.body.chunks[HEAD].pos, self.body.chunks[HIPS].pos);
        let dropping = self.input_y() < 0
            && !self.input[0].jump
            && (1..=3).all(|k| !self.is_tile_solid(room, HEAD, 0, -k))
            && ((head.y < hips.y && self.at_shaft_mouth(room, HEAD))
                || (head.y > hips.y && self.at_shaft_mouth(room, HIPS)));
        if !dropping {
            self.drop_grab_tile = None;
            return;
        }
        let lower = if head.y < hips.y { head } else { hips };
        let below = lower + Vec2::new(0.0, -20.0);
        if room.tile_at_pos(lower).any_pole() {
            self.drop_grab_tile = Some(lower.tile());
        } else if room.tile_at_pos(below).any_pole() {
            self.drop_grab_tile = Some(below.tile());
        }
    }

    /// Chunk `i` has just come out of a shaft's bottom: walls on both sides of the tile
    /// above, not on both sides of its own.
    fn at_shaft_mouth(&self, room: &Room, i: usize) -> bool {
        self.is_tile_solid(room, i, -1, 1)
            && self.is_tile_solid(room, i, 1, 1)
            && (!self.is_tile_solid(room, i, -1, 0) || !self.is_tile_solid(room, i, 1, 0))
    }

    /// Jump with down over an open bottom starts a drop; a jump press facing the wrong
    /// way starts a U-turn. Returns whether a shoot or slide would now be a "success"
    /// (hips braced against the back end).
    fn corridor_drop_or_turn(&mut self, room: &Room, shaft: bool, tunnel: bool) -> bool {
        let (x, y) = (self.input_x(), self.input_y());
        let pressed = self.jump_pressed();
        let (head, hips) = (self.body.chunks[HEAD].pos, self.body.chunks[HIPS].pos);
        if pressed
            && y < 0
            && !self.is_tile_solid(room, HEAD, 0, -1)
            && !self.is_tile_solid(room, HIPS, 0, -1)
        {
            self.corridor_drop = true;
            self.can_corridor_jump = 0;
            return false;
        }
        if shaft {
            if head.y > hips.y {
                return self.is_tile_solid(room, HIPS, 0, -1);
            }
            if pressed && head.y < hips.y && y > 0 {
                self.start_corridor_turn((0, 1));
            }
            return false;
        }
        if tunnel {
            if pressed && x != 0 && (head.x < hips.x) == (x > 0) {
                self.start_corridor_turn((x, 0));
                return false;
            }
            let flip = self.flip_direction;
            return self.is_tile_solid(room, HIPS, -flip, 0)
                && !self.is_tile_solid(room, HEAD, flip, 0);
        }
        false
    }

    fn start_corridor_turn(&mut self, dir: (i32, i32)) {
        self.corridor_turn_dir = Some(dir);
        self.corridor_turn_counter = 0;
        self.can_corridor_jump = 0;
    }

    /// The shaft shoot (head up) and the tunnel slide (head first); a "success" needs
    /// `can_corridor_jump`. A running vertical slide blocks both.
    fn corridor_jump(&mut self, shaft: bool, tunnel: bool) {
        if self.vertical_corridor_slide_counter >= 1 {
            return;
        }
        let x = self.input_x();
        let (head, hips) = (self.body.chunks[HEAD].pos, self.body.chunks[HIPS].pos);
        if shaft && self.input_y() > -1 && head.y > hips.y {
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y += 15.0;
            hips.vel.y += 10.0;
            if self.can_corridor_jump > 0 {
                self.shoot_up_counter = 30;
                let pos = self.body.chunks[HIPS].pos.tile_middle() + Vec2::new(0.0, -9.0);
                self.shed_drips(DripBurst::ShootUp { pos });
                self.play_sound(SoundId::SlugcatVerticalChuteJumpSuccess, HEAD, 1.0, 1.0);
                self.vertical_corridor_slide_counter = 22;
                self.slow_movement_stun = 2;
            } else {
                self.play_sound(SoundId::SlugcatVerticalChuteJumpFail, HEAD, 1.0, 1.0);
                self.vertical_corridor_slide_counter = 34;
                self.slow_movement_stun = 18;
            }
            self.can_corridor_jump = 0;
            return;
        }
        if !tunnel {
            return;
        }
        self.flip_direction = if head.x > hips.x { 1 } else { -1 };
        let flip = self.flip_direction;
        if x != flip && x != 0 {
            return;
        }
        let dir = flip as f32;
        if self.can_corridor_jump > 0 && x != 0 {
            let pos = self.body.chunks[HIPS].pos.tile_middle() + Vec2::new(-9.0 * dir, 0.0);
            self.shed_drips(DripBurst::SideShoot { pos, flip });
            self.play_sound(
                SoundId::SlugcatCorridorHorizontalSlideSuccess,
                HEAD,
                1.0,
                1.0,
            );
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.pos.x += 12.0 * dir;
            hips.pos.x += 12.0 * dir;
            head.pos.y = head.pos.tile_middle().y;
            hips.pos.y = head.pos.y;
            head.vel.x += 7.0 * dir;
            hips.vel.x += 7.0 * dir;
            self.horizontal_corridor_slide_counter = 25;
            self.slow_movement_stun = 5;
        } else {
            self.play_sound(SoundId::SlugcatCorridorHorizontalSlideFail, HEAD, 1.0, 1.0);
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.x += 6.0 * dir;
            hips.vel.x += 4.0 * dir;
            self.horizontal_corridor_slide_counter = 15;
            self.slow_movement_stun = 15;
        }
    }

    /// Velocity damping (weaker during a slide), gravity compensation (weaker early
    /// in a vertical slide) and cancelled buoyancy.
    fn corridor_damping(&mut self, water: Option<&Water>) {
        let friction = self.body.params.surface_friction;
        let gravity = self.body.params.gravity();
        let slides = self
            .vertical_corridor_slide_counter
            .max(self.horizontal_corridor_slide_counter);
        let slide = (slides as f32).inverse_lerp(0.0, 10.0);
        let keep = 0.9 - 0.3 * friction * (1.0 - slide);
        let lift = gravity
            * (friction * 8.0).clamp(0.2, 1.0)
            * 1.0_f32.lerp(
                0.2,
                (self.vertical_corridor_slide_counter as f32).inverse_lerp(0.0, 10.0),
            );
        let buoyancy = self.body.params.buoyancy;
        for c in &mut self.body.chunks {
            c.vel *= keep;
            c.vel.y += lift;
        }
        for c in &mut self.body.chunks {
            // Base game: not scaled by room gravity (Remix scales it).
            let sink = buoyancy * c.submersion(water);
            c.vel.y -= sink;
        }
        self.dynamic_run_speed = [0.0, 0.0];
    }

    /// The climb push for the first chunk (head first) with an open tile in the held
    /// direction, centring across the corridor, plus a small push on the head.
    fn corridor_climb_force(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        let friction = self.body.params.surface_friction;
        let mut force = 2.4
            * (friction + 0.2).clamp(0.2, 0.5)
            * self.stats.corridor_climb_speed_fac
            * 0.1_f32.lerp(
                1.0,
                (self.slow_movement_stun as f32).inverse_lerp(10.0, 0.0),
            );
        if x != 0 && y != 0 {
            force *= 0.4;
        }
        let (head, hips) = (self.body.chunks[HEAD].pos, self.body.chunks[HIPS].pos);
        if (x != 0 && (x > 0) == (head.x < hips.x)) || (y > 0 && head.y < hips.y) {
            self.backwards_counter += 2;
        }
        self.backwards_counter = self.backwards_counter.min(20);
        if self.backwards_counter > 10 {
            force *= 0.6;
        }
        for i in [HEAD, HIPS] {
            let other = 1 - i;
            if x != 0 && !self.is_tile_solid(room, i, x, 0) {
                let off = self.body.chunks[i].pos.y - self.body.chunks[i].pos.tile_middle().y;
                let c = &mut self.body.chunks[i];
                c.vel.x += force * x as f32;
                c.vel.y = c.vel.y * 0.8 - off * 0.2;
                if y == 0 {
                    let o = &mut self.body.chunks[other];
                    o.vel.y = o.vel.y * 0.8 - off * 0.2;
                }
                break;
            }
            if y != 0 && !self.is_tile_solid(room, i, 0, y) {
                let off = self.body.chunks[i].pos.x - self.body.chunks[i].pos.tile_middle().x;
                let c = &mut self.body.chunks[i];
                c.vel.y += force * y as f32;
                c.vel.x = c.vel.x * 0.8 - off * 0.2;
                if x == 0 {
                    let o = &mut self.body.chunks[other];
                    o.vel.x = o.vel.x * 0.8 - off * 0.2;
                }
                break;
            }
        }
        self.body.chunks[HEAD].vel += Vec2::new(x as f32, y as f32) * (force * 0.1);
    }

    /// Up onto a vertical pole once the head is out of the shaft walls (not during a
    /// shoot), or x onto a horizontal beam from a tunnel.
    fn leave_corridor_onto_beam(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        let head_tile = room.tile_at_pos(self.body.chunks[HEAD].pos);
        if y > 0
            && self.shoot_up_counter < 1
            && (!self.is_tile_solid(room, HEAD, -1, 0) || !self.is_tile_solid(room, HEAD, 1, 0))
            && head_tile.vertical_pole()
        {
            self.play_sound(SoundId::SlugcatGrabBeam, HEAD, 0.2, 1.0);
            self.body_mode = BodyMode::ClimbingOnBeam;
            self.animation = Animation::ClimbOnBeam;
        } else if x != 0
            && head_tile.horizontal_pole()
            && self.is_tile_solid(room, HIPS, 0, -1)
            && self.is_tile_solid(room, HIPS, 0, 1)
            && !self.is_tile_solid(room, HEAD, x, -1)
        {
            self.body_mode = BodyMode::ClimbingOnBeam;
            self.animation = Animation::HangFromBeam;
        }
    }

    /// `CorridorTurn`: 40 ticks of slowed wriggling (a random unit vector added to the
    /// head on the first 30), the last 10 pushing head and hips apart along the new
    /// direction, then a push out that way.
    pub(super) fn corridor_turn(&mut self, rng: &mut UnityRandom, sounds: &mut Sounds) {
        self.flush_sounds(rng, sounds);
        let Some((dx, dy)) = self.corridor_turn_dir else {
            self.animation = Animation::None;
            return;
        };
        let dir = Vec2::new(dx as f32, dy as f32);
        let turning = self.body_mode == BodyMode::CorridorClimb
            && self.corridor_turn_counter < CORRIDOR_TURN_TICKS;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if turning {
            self.slow_movement_stun = self.slow_movement_stun.max(10);
            head.vel *= 0.5;
            hips.vel *= 0.5;
            if self.corridor_turn_counter < CORRIDOR_TURN_JIGGLE_TICKS {
                head.vel += Vec2::random_unit(rng);
            } else {
                head.vel += dir * 0.5;
                hips.vel -= dir * 0.5;
            }
            self.corridor_turn_counter += 1;
            return;
        }
        head.vel += dir * 6.0;
        hips.vel += dir * 5.0;
        self.corridor_turn_dir = None;
        self.animation = Animation::None;
        self.play_sound(SoundId::SlugcatTurnInCorridor, HEAD, 1.0, 1.0);
    }

    /// End of the `Default` branch: on reaching the remembered beam tile (jump not held),
    /// snap the head to its middle and hold on; give up once out of reach.
    pub(super) fn catch_drop_grab_tile(&mut self, room: &Room) {
        let Some(tile) = self.drop_grab_tile else {
            return;
        };
        let (head, hips) = (self.tile_pos(HEAD), self.tile_pos(HIPS));
        if (head == tile || hips == tile) && !self.input[0].jump {
            let t = room.tile(tile.0, tile.1);
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            let mid = rw_assets::tile_centre(tile.0, tile.1);
            head.pos = Vec2::new(mid[0], mid[1]);
            if t.vertical_pole() {
                self.animation = Animation::ClimbOnBeam;
            } else if t.horizontal_pole() {
                self.animation = Animation::HangFromBeam;
            }
            if hips.pos.y > head.pos.y {
                let away = if hips.pos.x < head.pos.x { -2.0 } else { 2.0 };
                hips.vel.x += away;
                hips.pos.x += away;
            }
            self.drop_grab_tile = None;
        } else if head.1 > tile.1 + 2
            || head.1 < tile.1
            || hips.1 < tile.1
            || (head.0 != tile.0 && hips.0 != tile.0)
        {
            self.drop_grab_tile = None;
        }
    }
}
