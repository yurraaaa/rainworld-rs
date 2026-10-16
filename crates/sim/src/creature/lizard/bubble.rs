//! `LizardBubble`: a bubble of the lizard's colour that its graphics blow from the
//! head (on impacts, while `bubble` counts, when the head jumps, lunging and when it
//! spots a threat). A `CosmeticSprite` in the room's update list whose constructor and
//! update draw on the shared stream; its sprite is `rw-graphics`'. Decompile
//! `LizardBubble.cs`.

use rw_assets::{Room, Terrain};
use rw_random::UnityRandom;

use super::BubbleHead;
use crate::creature::CreatureId;
use crate::room_ext::RoomExt;
use crate::{FloatExt, Vec2};

/// A `LizardBubble`.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardBubble {
    /// The lizard whose graphics made it.
    pub lizard: CreatureId,
    /// The graphics' head as last read (the game keeps the module the bubble refers to,
    /// even after its room stops being viewed).
    pub head: BubbleHead,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub last_life: f32,
    pub life_time: i32,
    pub hollow_ness: f32,
    pub hollow: bool,
    /// Its offset from the head, in the head's frame.
    pub origin_point: Vec2,
    pub stuck_to_origin: bool,
    pub free_floating: bool,
    pub liberated_origin: Vec2,
    pub liberated_origin_vel: Vec2,
    pub last_liberated_origin: Vec2,
    pub life_time_when_free: f32,
    pub stickiness: f32,
}

impl LizardBubble {
    /// The constructor (eleven draws): somewhere between the main chunk at `main` and
    /// the head, a life of `10 + Range(0, Range(0, Range(0, Range(0, 200))))`, the
    /// head's velocity plus a random push, its hollowness; stuck to the head unless it
    /// has no stickiness.
    pub fn new(
        lizard: CreatureId,
        main: Vec2,
        head: BubbleHead,
        [intensity, stickiness, extra_speed]: [f32; 3],
        rng: &mut UnityRandom,
    ) -> Self {
        let t = rng.value();
        let dir = Vec2::from_degrees(rng.value() * 360.0);
        let pos = main.lerp(head.pos, t.clamp(0.0, 1.0)) + dir * (5.0 * rng.value());
        let origin_point = (pos - head.pos).rotate_around_origo(-head.rotation);
        let r = rng.range_i32(0, 200);
        let r = rng.range_i32(0, r);
        let r = rng.range_i32(0, r);
        let life_time = 10 + rng.range_i32(0, r);
        let dir = Vec2::from_degrees(rng.value() * 360.0);
        let a = rng.value();
        let b = rng.value();
        let vel = head.vel + dir * (a * b * 12.0 * intensity.mathf_pow(1.5) + extra_speed);
        let hollow_ness = 0.5f32.lerp(1.5, rng.value());
        let mut bubble = Self {
            lizard,
            head,
            pos,
            last_pos: pos,
            vel,
            life: 1.0,
            last_life: 1.0,
            life_time,
            hollow_ness,
            hollow: false,
            origin_point,
            stuck_to_origin: false,
            free_floating: false,
            liberated_origin: pos,
            liberated_origin_vel: Vec2::ZERO,
            last_liberated_origin: pos,
            life_time_when_free: 0.0,
            stickiness,
        };
        if bubble.stickiness() == 0.0 {
            bubble.free_floating = true;
        } else {
            bubble.stuck_to_origin = true;
        }
        bubble
    }

    /// `Stickiness`.
    #[must_use]
    pub fn stickiness(&self) -> f32 {
        self.stickiness
            .max(self.head.vel.length().inverse_lerp(4.0, 14.0))
    }

    /// `Update` then `CosmeticSprite.Update`, `head` the graphics' head now (`None`
    /// keeps the last one) and `water_level` `Room.FloatWaterLevel(pos)`; true once it
    /// is destroyed. The voice's pull (`voiceVisualizationIntensity`) is lizard step 5b.
    pub fn update(
        &mut self,
        room: &Room,
        head: Option<BubbleHead>,
        water_level: f32,
        rng: &mut UnityRandom,
    ) -> bool {
        if let Some(h) = head {
            self.head = h;
        }
        self.vel = self.vel.slerp(Vec2::from_degrees(rng.value() * 360.0), 0.2);
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time as f32;
        let slated = self.life <= 0.0;
        if self.free_floating && rng.value() < 0.5 {
            self.hollow = rng.value() < 0.5;
        }
        if room.tile_at_pos(self.pos).terrain == Terrain::Solid {
            self.life_time = 1.min(self.life_time - 5);
        }
        if self.pos.y < water_level {
            self.vel *= 0.9;
            self.vel.y += 4.0;
        }
        if self.stuck_to_origin {
            let v = self.head.pos + self.origin_point.rotate_around_origo(self.head.rotation);
            self.liberated_origin_vel = v - self.liberated_origin;
            self.liberated_origin = v;
            self.last_liberated_origin = v;
            let s = self.stickiness();
            if self.life < 0.5
                || rng.value() < 1.0 / (10.0 + s * 80.0)
                || !self.pos.dist_less(v, 10.0 + 90.0 * s)
            {
                self.stuck_to_origin = false;
            }
        } else if !self.free_floating {
            self.last_liberated_origin = self.liberated_origin;
            let d = self.liberated_origin.distance(self.pos);
            let want = self.liberated_origin.direction_to(self.pos) * d.lerp(10.0, 0.5);
            self.liberated_origin_vel = self.liberated_origin_vel.lerp(want, 0.7);
            self.liberated_origin += self.liberated_origin_vel;
            if self.liberated_origin.dist_less(self.pos, 5.0) {
                self.vel = self.vel.lerp(self.liberated_origin_vel, 0.3);
                self.life_time_when_free = self.life;
                self.free_floating = true;
            }
        }
        self.last_pos = self.pos;
        self.pos += self.vel;
        slated
    }
}

impl Vec2 {
    /// `Custom.RotateAroundOrigo`: turned `deg` degrees clockwise.
    #[must_use]
    pub fn rotate_around_origo(self, deg: f32) -> Self {
        let rad = deg * (-std::f32::consts::PI / 180.0);
        let (cos, sin) = (f64::from(rad).cos() as f32, f64::from(rad).sin() as f32);
        Self::new(cos * self.x - sin * self.y, sin * self.x + cos * self.y)
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;

    use super::{BubbleHead, LizardBubble};
    use crate::Vec2;
    use crate::creature::CreatureId;

    /// `LizardBubble`'s constructor: the lerp, angle and offset values, the four nested
    /// ranges (each drawing unless its bound is 0), then the angle, two speed factors and
    /// the hollowness.
    #[test]
    fn the_constructor_draws_in_the_games_order() {
        for seed in [1, 7, 42] {
            let mut rng = UnityRandom::new(seed);
            let mut expected = rng.clone();
            let head = BubbleHead {
                pos: Vec2::new(10.0, 0.0),
                vel: Vec2::new(1.0, 0.0),
                rotation: 90.0,
            };
            let b = LizardBubble::new(CreatureId(0), Vec2::ZERO, head, [1.0, 0.5, 2.0], &mut rng);
            for _ in 0..3 {
                expected.value();
            }
            let mut r = expected.range_i32(0, 200);
            r = expected.range_i32(0, r);
            r = expected.range_i32(0, r);
            let life = 10 + expected.range_i32(0, r);
            for _ in 0..4 {
                expected.value();
            }
            assert_eq!(rng, expected);
            assert_eq!(b.life_time, life);
            assert!(b.stuck_to_origin && !b.free_floating);
        }
    }

    /// No stickiness and a slow head: it floats free from the start.
    #[test]
    fn a_bubble_without_stickiness_floats_free() {
        let mut rng = UnityRandom::new(3);
        let b = LizardBubble::new(
            CreatureId(0),
            Vec2::ZERO,
            BubbleHead::default(),
            [1.0, 0.0, 0.0],
            &mut rng,
        );
        assert!(b.free_floating && !b.stuck_to_origin);
    }
}
