//! `Spark` (40664) and `ExplosionSpikes` (34870): `CosmeticSprite`s a weapon's terrain
//! hits and the belly-slide throw add to their room's update list, and the
//! `OverseerEffect` (232098) a dying overseer sheds, and a lizard's graphics'
//! `LizardBubble`s and its head shield's sparks and `StationaryEffect` flash. Their
//! constructors draw on the shared stream, and the overseer effect's and the bubble's
//! updates too. The sprites are `rw-graphics`' (the overseer effect's, on the
//! `OverseerZip` shader, is not drawn).
//!
//! Mirrors: Spark.cs, ExplosionSpikes.cs, OverseerEffect.cs, LizardBubble.cs, StationaryEffect.cs

use rw_assets::{Room, Terrain};
use rw_random::UnityRandom;

use crate::creature::CreatureId;
use crate::creature::lizard::{BubbleHead, LizardBubble, LizardVoiceView};
use crate::room_ext::RoomExt;
use crate::{FloatExt, Vec2};

/// A cosmetic sprite in a realized room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct Cosmetic {
    /// The port's id for it (graphics).
    pub id: u64,
    pub room: i32,
    /// Its place in the room's update list.
    pub update_order: u64,
    pub slated: bool,
    pub kind: CosmeticKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CosmeticKind {
    Spark(Spark),
    Spikes(ExplosionSpikes),
    OverseerEffect(OverseerEffectSprite),
    LizardBubble(Box<LizardBubble>),
    Flash(StationaryEffect),
}

/// What a cosmetic tied to a lizard's graphics reads of them in its update.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CosmeticTie {
    /// The `whiteFlicker` of the module the cosmetic was given, while it is the lizard's.
    pub white_flicker: Option<i32>,
    /// That module's head, while it is the lizard's.
    pub head: Option<BubbleHead>,
    /// That module's voice reading, while it is the lizard's.
    pub voice: Option<LizardVoiceView>,
    /// `Room.FloatWaterLevel(pos)` at the object.
    pub water_level: f32,
}

/// `StationaryEffect` of type `FlashingOrb`: a four-tick flash a lizard's head shield
/// shows where it is hit (no draws in its update).
#[derive(Debug, Clone, PartialEq)]
pub struct StationaryEffect {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub life: f32,
    pub last_life: f32,
    pub life_time: i32,
    /// The lizard and the serial of the graphics module whose head colour it shows
    /// (`lizard`, kept even after the module is dropped).
    pub lizard: Option<(CreatureId, u64)>,
}

impl StationaryEffect {
    #[must_use]
    pub fn flashing_orb(pos: Vec2, lizard: Option<(CreatureId, u64)>) -> Self {
        Self {
            pos,
            last_pos: pos,
            life: 1.0,
            last_life: 1.0,
            life_time: 4,
            lizard,
        }
    }

    /// `Update` (the lizard's `whiteFlicker` is never below 0) then
    /// `CosmeticSprite.Update`; true once destroyed.
    pub fn update(&mut self) -> bool {
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time as f32;
        self.last_pos = self.pos;
        self.life <= 0.0
    }
}

/// `OverseerEffect`: a drifting, spinning glow of a dying overseer's colour.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerEffectSprite {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    /// RGB.
    pub color: [f32; 3],
    pub rad: f32,
    pub rotation: f32,
    pub last_rotation: f32,
    pub rot_vel: f32,
    pub life: f32,
    pub last_life: f32,
    pub life_time: f32,
}

impl OverseerEffectSprite {
    /// The constructor: its rotation, spin and life from three draws.
    pub fn new(
        pos: Vec2,
        vel: Vec2,
        color: [f32; 3],
        rad: f32,
        life_time_fac: f32,
        rng: &mut UnityRandom,
    ) -> Self {
        let rotation = rng.value() * 360.0;
        let rot_vel = (-26.0f32).lerp(26.0, rng.value());
        let life_time = 5.0f32.lerp(11.0, rng.value()) * life_time_fac;
        Self {
            pos,
            last_pos: pos,
            vel,
            color,
            rad,
            rotation,
            last_rotation: rotation,
            rot_vel,
            life: 1.0,
            last_life: 1.0,
            life_time,
        }
    }

    /// `Update`: slowing with a random nudge (two draws), spinning with its speed; gone
    /// the tick after its life runs out. True once destroyed.
    pub fn update(&mut self, rng: &mut UnityRandom) -> bool {
        self.vel *= 0.9;
        let dir = Vec2::random_unit(rng);
        self.vel += dir * (rng.value() * 0.04);
        self.last_rotation = self.rotation;
        self.rotation += self.rot_vel * self.vel.length();
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        self.last_pos = self.pos;
        self.pos += self.vel;
        self.last_life <= 0.0
    }
}

/// `Spark`: a short falling streak that bounces once off a floor.
#[derive(Debug, Clone, PartialEq)]
pub struct Spark {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub last_last_pos: Vec2,
    pub last_last_last_pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub life_time: i32,
    pub gravity: f32,
    /// RGBA.
    pub color: [f32; 4],
    /// The lizard and the serial of the graphics module it was given (`lizard`, kept
    /// even after the module is dropped).
    pub lizard: Option<(CreatureId, u64)>,
    /// That module's `whiteFlicker` as last read: it fades fast below 5 and takes the
    /// head's colour above 0.
    pub white_flicker: i32,
}

/// `ExplosionSpikes`: a ring of thin spikes that grow and fade over `2 * lifeTime` ticks.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplosionSpikes {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub inner_rad: f32,
    pub life_time: f32,
    pub time: i32,
    /// RGBA.
    pub color: [f32; 4],
    /// Per spike: `values[i, 0..3]` (length, width, its own lifetime) and its direction.
    pub spikes: Vec<([f32; 3], Vec2)>,
}

impl Spark {
    /// The constructor: drawn ahead along `vel` by up to 30 px (one draw), its gravity
    /// (one), its life `Range(0, standard)` and, one time in ten, `Range(standard,
    /// exceptional)` instead.
    pub fn new(
        pos: Vec2,
        vel: Vec2,
        color: [f32; 4],
        standard: i32,
        exceptional: i32,
        rng: &mut UnityRandom,
    ) -> Self {
        let start = pos + vel.normalized() * (30.0 * rng.value());
        let gravity = 0.4f32.lerp(0.9, rng.value());
        let mut life_time = rng.range_i32(0, standard);
        if rng.value() < 0.1 {
            life_time = rng.range_i32(standard, exceptional);
        }
        Self {
            pos: start,
            last_pos: pos,
            last_last_pos: pos,
            last_last_last_pos: pos,
            vel,
            life: 1.0,
            life_time,
            gravity,
            color,
            lizard: None,
            white_flicker: 0,
        }
    }

    /// `Update` then `CosmeticSprite.Update`; true once it is destroyed.
    pub fn update(&mut self, room: &Room, white_flicker: Option<i32>) -> bool {
        self.last_last_last_pos = self.last_last_pos;
        self.last_last_pos = self.last_pos;
        self.vel.y -= self.gravity;
        self.life -= 1.0 / self.life_time as f32;
        if self.lizard.is_some() {
            if let Some(f) = white_flicker {
                self.white_flicker = f;
            }
            if self.white_flicker < 5 {
                self.life -= 0.2;
            }
        }
        let mut slated = false;
        if room.tile_at_pos(self.pos).terrain == Terrain::Solid {
            let above = room.tile_at_pos(self.pos + Vec2::new(0.0, 20.0)).terrain;
            if self.vel.y < 0.0 && above == Terrain::Air {
                self.pos.y = self.pos.tile_middle().y + 10.0;
                self.vel.y *= -0.5;
                if self.vel.y.abs() < 3.0 {
                    self.life -= 1.0 / 3.0;
                }
            } else {
                slated = true;
            }
        }
        if self.life <= 0.0 {
            slated = true;
        }
        self.last_pos = self.pos;
        self.pos += self.vel;
        slated
    }
}

impl ExplosionSpikes {
    /// The constructor: `spikes` scaled by 0.8 to 1.2 (truncated), a random start angle,
    /// then per spike its jittered direction, lifetime and length (both halved when the
    /// spike would reach into a Solid tile) and width: 2 + 4 draws a spike.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        room: &Room,
        pos: Vec2,
        spikes: i32,
        inner_rad: f32,
        life_time: f32,
        width: f32,
        length: f32,
        color: [f32; 4],
        rng: &mut UnityRandom,
    ) -> Self {
        let n = (spikes as f32 * 0.8f32.lerp(1.2, rng.value())) as i32;
        let start = rng.value() * 360.0;
        let mut list = Vec::new();
        for i in 0..n {
            let angle = i as f32 / n as f32 * 360.0 + start;
            let dir =
                Vec2::from_degrees(angle + (-0.5f32).lerp(0.5, rng.value()) * 360.0 / n as f32);
            let mut v = [0.0; 3];
            let half = if room
                .tile_at_pos(pos + dir * (inner_rad + length * 0.4))
                .is_solid()
            {
                0.5
            } else {
                1.0
            };
            v[2] = life_time * 0.5f32.lerp(1.5, rng.value()) * half;
            v[0] = length * 0.6f32.lerp(1.4, rng.value()) * half;
            v[1] = width * 0.6f32.lerp(1.4, rng.value());
            list.push((v, dir));
        }
        Self {
            pos,
            last_pos: pos,
            inner_rad,
            life_time,
            time: 0,
            color,
            spikes: list,
        }
    }

    /// `CosmeticSprite.Update` then the count; true once it is destroyed.
    pub fn update(&mut self) -> bool {
        self.last_pos = self.pos;
        self.time += 1;
        self.time as f32 > self.life_time * 2.0
    }
}

impl Cosmetic {
    /// Its `Update`; true once destroyed.
    pub fn update(&mut self, room: &Room, tie: CosmeticTie, rng: &mut UnityRandom) -> bool {
        self.slated = match &mut self.kind {
            CosmeticKind::Spark(s) => s.update(room, tie.white_flicker),
            CosmeticKind::Spikes(s) => s.update(),
            CosmeticKind::OverseerEffect(e) => e.update(rng),
            CosmeticKind::LizardBubble(b) => {
                b.update(room, tie.head, tie.voice, tie.water_level, rng)
            }
            CosmeticKind::Flash(f) => f.update(),
        };
        self.slated
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;

    use super::{ExplosionSpikes, Spark};
    use crate::tests::room;
    use crate::{FloatExt, Vec2};

    #[test]
    fn a_spark_draws_three_or_four() {
        // `pos + vel.normalized * (30 * value)`, `Lerp(0.4, 0.9, value)`,
        // `Range(0, standard)`, `value < 0.1` and then `Range(standard, exceptional)`.
        for seed in 0..40 {
            let mut rng = UnityRandom::new(seed);
            let mut expect = UnityRandom::new(seed);
            let s = Spark::new(
                Vec2::new(10.0, 10.0),
                Vec2::new(3.0, 0.0),
                [1.0; 4],
                6,
                18,
                &mut rng,
            );
            let ahead = expect.value() * 30.0;
            let gravity = 0.4f32.lerp(0.9, expect.value());
            let mut life = expect.range_i32(0, 6);
            if expect.value() < 0.1 {
                life = expect.range_i32(6, 18);
            }
            assert_eq!(rng.state(), expect.state());
            assert_eq!(s.pos, Vec2::new(10.0 + ahead, 10.0));
            assert_eq!((s.gravity, s.life_time), (gravity, life));
        }
    }

    #[test]
    fn spikes_draw_two_and_four_a_spike() {
        // `(int)(6 * Lerp(0.8, 1.2, value))` spikes, the start angle, then per spike
        // the direction jitter, lifetime, length and width.
        let r = room(&["....", "....", "...."]);
        let mut rng = UnityRandom::new(3);
        let mut expect = UnityRandom::new(3);
        let s = ExplosionSpikes::new(
            &r,
            Vec2::new(40.0, 30.0),
            6,
            5.5,
            4.0,
            4.5,
            21.0,
            [1.0, 1.0, 1.0, 0.25],
            &mut rng,
        );
        let n = (6.0 * 0.8f32.lerp(1.2, expect.value())) as i32;
        assert_eq!(s.spikes.len(), usize::try_from(n).unwrap());
        for _ in 0..1 + 4 * n {
            expect.value();
        }
        assert_eq!(rng.state(), expect.state());
    }
}
