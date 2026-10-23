//! `ElectricDeath` (the `ElectricDeath` room effect, 28 Underhang rooms): the lightning
//! web that grows over the last 400 ticks before the rain and the 1200 after it, its nine
//! `LightFlash`es, the two sound loops, the spontaneous sparks on the walls and the
//! per-chunk strikes that kick bodies off a wall and give creatures an electric
//! `Violence`. The game's side (the room's chunks, the kill, the sounds) is
//! `game_electric_death.rs`. `PrecycleModule`'s and Sofanthiel's `Intensity` branches and
//! `ElectricSpear` are DLC (D33).
//!
//! Mirrors: ElectricDeath.cs

use rw_random::UnityRandom;

use crate::float_ext::MathfSin;
use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::world::RainCycle;
use crate::{FloatExt, LoopEmitter, SoundId, Vec2, Vec3};

/// `ElectricDeath.LightFlash`: a light the shader sprites show at a random screen spot.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LightFlash {
    /// Screen pixels (not room pixels).
    pub pos: Vec2,
    pub life_time: f32,
    pub rad: f32,
    pub life: f32,
    pub last_life: f32,
}

impl LightFlash {
    /// `Reset`: four draws (x, y, lifetime, radius).
    pub fn reset(&mut self, intensity: f32, screen: Vec2, rng: &mut UnityRandom) {
        self.pos.x = screen.x * rng.value();
        self.pos.y = screen.y * rng.value();
        self.life_time = 3.0f32.lerp(34.0f32.lerp(12.0, intensity), rng.value());
        self.rad = 40.0f32.lerp(800.0, rng.value() * 0.5f32.lerp(1.0, intensity));
        self.life = 1.0;
        self.last_life = 1.0;
    }

    /// `Update`: a flash dead for two ticks resets, else its life runs down.
    pub fn update(&mut self, intensity: f32, screen: Vec2, rng: &mut UnityRandom) {
        if self.life <= 0.0 && self.last_life <= 0.0 {
            self.reset(intensity, screen, rng);
            return;
        }
        self.last_life = self.life;
        self.life = (self.life - 1.0 / self.life_time).max(0.0);
    }
}

/// `ElectricDeath.SparkFlash`: the green flash where a chunk is struck or a wall sparks;
/// it sheds a green `Spark` every tick while it lives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparkFlash {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub size: f32,
    pub life: f32,
    pub last_life: f32,
    pub life_time: f32,
}

impl SparkFlash {
    /// The constructor: one draw (the lifetime).
    pub fn new(pos: Vec2, size: f32, rng: &mut UnityRandom) -> Self {
        Self {
            pos,
            last_pos: pos,
            size,
            life: 1.0,
            last_life: 1.0,
            life_time: 2.0f32.lerp(16.0, size * rng.value()),
        }
    }

    /// `Update` after the spark it adds: true once destroyed (dead for two ticks).
    pub fn update(&mut self) -> bool {
        if self.life <= 0.0 && self.last_life <= 0.0 {
            return true;
        }
        self.last_life = self.life;
        self.life = (self.life - 1.0 / self.life_time).max(0.0);
        false
    }

    /// The spark `Update` adds: a green `Spark` at the flash flying off at up to 60 px a
    /// tick (`RNV` then a value, then the spark's own draws).
    pub fn spark(&self, rng: &mut UnityRandom) -> crate::cosmetic::Spark {
        let dir = Vec2::random_unit(rng);
        let vel = dir * (60.0 * rng.value());
        crate::cosmetic::Spark::new(self.pos, vel, [0.0, 1.0, 0.0, 1.0], 4, 50, rng)
    }
}

/// A spontaneous wall spark `Update` rolled: where it is and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallSpark {
    pub pos: Vec2,
    pub size: f32,
}

/// One body chunk struck: for the MCP's recent list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    pub tick: u64,
    pub creature: bool,
    pub pos: Vec2,
    pub power: f32,
    /// `Violence`'s damage and stun bonus for a creature.
    pub damage: f32,
    pub stun_bonus: f32,
}

/// An `ElectricDeath` in a realized room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct ElectricDeath {
    pub update_order: u64,
    /// The effect's `amount`.
    pub amount: f32,
    /// `flashes`; the first is never updated or drawn.
    pub flashes: [LightFlash; 9],
    pub color: Vec3,
    pub last_color: Vec3,
    pub get_to_color: Vec3,
    pub sin: f32,
    pub last_sin: f32,
    /// `soundLoop` (`Death_Lightning_Heavy_Lightning_LOOP`) and `soundLoop2` (the early
    /// sizzle), made on the first tick with any intensity.
    pub heavy_loop: Option<ChunkDynamicSoundLoop>,
    pub sizzle_loop: Option<ChunkDynamicSoundLoop>,
    /// `closeToWallTiles`, filled by `AIMapReady`.
    pub close_to_wall_tiles: Option<Vec<(i32, i32)>>,
    /// The last strikes, newest last (the MCP).
    pub strikes: Vec<Strike>,
}

/// How many strikes are kept for the MCP.
pub const STRIKES_KEPT: usize = 16;

impl ElectricDeath {
    #[must_use]
    pub fn new(update_order: u64, amount: f32) -> Self {
        Self {
            update_order,
            amount,
            flashes: [LightFlash::default(); 9],
            color: Vec3::default(),
            last_color: Vec3::default(),
            get_to_color: Vec3::default(),
            sin: 0.0,
            last_sin: 0.0,
            heavy_loop: None,
            sizzle_loop: None,
            close_to_wall_tiles: None,
            strikes: Vec::new(),
        }
    }

    /// `Intensity` (base game): `amount` times the rain cycle's position from 400 ticks
    /// before the rain to 1200 after it, to the power 2.2.
    #[must_use]
    pub fn intensity(&self, cycle: &RainCycle) -> f32 {
        let from = (cycle.cycle_length - 400) as f32;
        let to = (cycle.cycle_length + 1200) as f32;
        self.amount * (cycle.timer as f32).inverse_lerp(from, to).mathf_pow(2.2)
    }

    /// The two loops' block of `Update`: each made with volume 0 on the first tick, then
    /// updated and given its volume (the `Volume` setter writes the emitter's too).
    pub fn update_loops(&mut self, intensity: f32, cx: &mut LoopCx<'_>) {
        let sizzle = intensity.mathf_pow(0.1)
            * (0.5 + 0.5 * (self.sin * std::f32::consts::PI * 2.0).mathf_sin())
                .lerp(0.0, intensity.mathf_pow(8.0));
        Self::update_loop(
            &mut self.heavy_loop,
            SoundId::DeathLightningHeavyLightningLoop,
            intensity,
            cx,
        );
        Self::update_loop(
            &mut self.sizzle_loop,
            SoundId::DeathLightningEarlySizzleLoop,
            sizzle,
            cx,
        );
    }

    fn update_loop(
        slot: &mut Option<ChunkDynamicSoundLoop>,
        sound: SoundId,
        volume: f32,
        cx: &mut LoopCx<'_>,
    ) {
        let Some(sound_loop) = slot else {
            let mut sound_loop = ChunkDynamicSoundLoop::default();
            sound_loop.sound = Some(sound);
            sound_loop.set_volume(0.0);
            *slot = Some(sound_loop);
            return;
        };
        sound_loop.update_disembodied(Some(cx.room_index), 0.0, cx);
        sound_loop.set_volume(volume);
        if let Some(e) = sound_loop.emitter {
            cx.sounds
                .update_emitter(e.sound, volume, sound_loop.pitch());
        }
    }

    /// `SoundEmitter.Update` of the loop emitter that joined the room at `order`; its
    /// sound is cut when it dies.
    pub fn update_emitter(&mut self, order: u64, cx: &mut LoopCx<'_>) {
        for sound_loop in [&mut self.heavy_loop, &mut self.sizzle_loop]
            .into_iter()
            .flatten()
        {
            let Some(e) = sound_loop.emitter_mut(order).filter(|e| !e.slated) else {
                continue;
            };
            let room = e.room;
            e.update(Some(room), cx.camera_room, cx.tick);
            if e.slated {
                cx.sounds.stop_emitter(e.sound);
            }
            sound_loop.released.retain(|e| !e.slated);
            return;
        }
    }

    /// The loop emitters still in the room's update list.
    pub fn emitters(&self) -> impl Iterator<Item = &LoopEmitter> {
        [&self.heavy_loop, &self.sizzle_loop]
            .into_iter()
            .flatten()
            .flat_map(ChunkDynamicSoundLoop::emitters)
    }

    /// `lastSin = sin; sin += Intensity * 0.1`.
    pub fn advance_sin(&mut self, intensity: f32) {
        self.last_sin = self.sin;
        self.sin += intensity * 0.1;
    }

    /// The spontaneous spark's rolls: with wall tiles, the room viewed and `value <
    /// InverseLerp(1000, 9120, W * H) * Intensity` (the gate's one draw), a tile (a
    /// `Range` draw), the spot in it (two) and the size (one).
    pub fn roll_wall_spark(
        &self,
        intensity: f32,
        viewed: bool,
        area: i32,
        rng: &mut UnityRandom,
    ) -> Option<WallSpark> {
        let tiles = self
            .close_to_wall_tiles
            .as_ref()
            .filter(|t| !t.is_empty())?;
        if !viewed || rng.value() >= (area as f32).inverse_lerp(1000.0, 9120.0) * intensity {
            return None;
        }
        let n = i32::try_from(tiles.len()).unwrap_or(i32::MAX);
        let tile = tiles[usize::try_from(rng.range_i32(0, n)).unwrap_or(0)];
        let middle = Vec2::middle_of_tile(tile);
        let x = (-10.0f32).lerp(10.0, rng.value());
        let y = (-10.0f32).lerp(10.0, rng.value());
        let size = rng.value() * intensity;
        Some(WallSpark {
            pos: middle + Vec2::new(x, y),
            size,
        })
    }

    /// `flashes[1..]`' updates.
    pub fn update_flashes(&mut self, intensity: f32, screen: Vec2, rng: &mut UnityRandom) {
        for flash in &mut self.flashes[1..] {
            flash.update(intensity, screen, rng);
        }
    }

    /// The colour block: `color` lerped toward `getToColor` by up to 0.3 (one draw),
    /// then one component of `getToColor` redrawn by up to three chained one-in-three
    /// tests.
    pub fn update_color(&mut self, rng: &mut UnityRandom) {
        self.last_color = self.color;
        let t = (rng.value() * 0.3).clamp(0.0, 1.0);
        let (c, g) = (self.color, self.get_to_color);
        self.color = Vec3::new(
            c.x + (g.x - c.x) * t,
            c.y + (g.y - c.y) * t,
            c.z + (g.z - c.z) * t,
        );
        if rng.value() < 1.0 / 3.0 {
            self.get_to_color.x = rng.value();
        } else if rng.value() < 1.0 / 3.0 {
            self.get_to_color.y = rng.value();
        } else if rng.value() < 1.0 / 3.0 {
            self.get_to_color.z = rng.value();
        }
    }

    /// The strike gate: `Intensity > 0.5` and one draw below `LerpMap(Intensity, 0.5, 1,
    /// 0, 0.5)`. Returns that chance, which each chunk then rolls again.
    pub fn strike_chance(intensity: f32, rng: &mut UnityRandom) -> Option<f32> {
        let chance = intensity.lerp_map(0.5, 1.0, 0.0, 0.5);
        (intensity > 0.5 && rng.value() < chance).then_some(chance)
    }

    /// `num2`: `Pow(value, 0.9) * InverseLerp(0.5, 1, Intensity)` (one draw).
    pub fn strike_power(intensity: f32, rng: &mut UnityRandom) -> f32 {
        rng.value().mathf_pow(0.9) * intensity.inverse_lerp(0.5, 1.0)
    }

    /// Keeps the last [`STRIKES_KEPT`] strikes.
    pub fn record_strike(&mut self, strike: Strike) {
        self.strikes.push(strike);
        if self.strikes.len() > STRIKES_KEPT {
            self.strikes.remove(0);
        }
    }
}

#[cfg(test)]
#[path = "electric_death_tests.rs"]
mod tests;
