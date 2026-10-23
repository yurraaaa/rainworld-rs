//! `ZapCoil`: a placed rectangle of electric arcs. While it is on (`turnedOn` above a half)
//! a body resting against a tile face of the rectangle's long side (30 px beyond its
//! surface is inside the rectangle) is zapped: a `ZapFlash`, a kick away, and a creature
//! dies. It flickers with a `disruption` that rises by chance and with the room's gravity,
//! switches off for spells, and in a `BrokenZeroG` room loses its `powered` while the
//! world's switch is not settled on. Two rectangular sound loops (the hum, and the
//! sizzle fading in with the disruption). Also the `ZapCoilLight` that follows a coil and
//! the `ZapFlash` a zap makes. The coil's kill, kick and flash are the game's side
//! (`game_ss_electric.rs`); its sprites are `rw-graphics`'. Draws are on the shared stream
//! (D36); Watcher's creature exemptions, Moon's heart and the electric spear are DLC
//! branches (D36).
//!
//! Mirrors: ZapCoil.cs, ZapCoilLight.cs

use rw_random::UnityRandom;

use crate::float_ext::MathfSin;
use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::{FloatExt, LightSource, LightTie, SoundId, SoundRect, Sounds, Vec2};

/// The colour of a coil's, light's and flash's `LightSource`.
pub const ZAP_BLUE: [f32; 3] = [0.0, 0.0, 1.0];

/// What `ZapCoil.Update` reads of the room and the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoilEnv {
    /// `room.fullyLoaded`.
    pub fully_loaded: bool,
    /// `room.gravity`.
    pub gravity: f32,
    /// `roomSettings.GetEffectAmount(BrokenZeroG) > 0`.
    pub broken_zero_g: bool,
    /// `brokenAntiGrav.to == 1 && brokenAntiGrav.progress == 1`: the switch has settled on.
    pub switch_settled_on: bool,
}

/// One `ZapCoil`.
#[derive(Debug, Clone, PartialEq)]
pub struct ZapCoil {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `IntRect(left, bottom, right, top)` in tiles.
    pub rect: [i32; 4],
    pub zap_lit: f32,
    pub horizontal: bool,
    /// Two edges' `[current, last, phase, wobble]`.
    pub flicker: [[f32; 4]; 2],
    pub disruption: f32,
    pub smooth_disruption: f32,
    pub last_turned_on: f32,
    pub turned_on: f32,
    pub turned_off_counter: i32,
    pub powered: bool,
    pub sound_loop: ChunkDynamicSoundLoop,
    pub disrupted_loop: ChunkDynamicSoundLoop,
}

impl ZapCoil {
    /// The constructor (no draws).
    #[must_use]
    pub fn new(update_order: u64, rect: [i32; 4]) -> Self {
        let mut sound_loop = ChunkDynamicSoundLoop::default();
        sound_loop.sound = Some(SoundId::ZapperLoop);
        let mut disrupted_loop = ChunkDynamicSoundLoop::default();
        disrupted_loop.set_volume(0.0);
        disrupted_loop.sound = Some(SoundId::ZapperDisruptedLoop);
        let [left, bottom, right, top] = rect;
        Self {
            update_order,
            rect,
            zap_lit: 0.0,
            horizontal: (right - left).abs() > (top - bottom).abs(),
            flicker: [[0.0; 4]; 2],
            disruption: -1.0,
            smooth_disruption: 0.0,
            last_turned_on: 1.0,
            turned_on: 1.0,
            turned_off_counter: 0,
            powered: true,
            sound_loop,
            disrupted_loop,
        }
    }

    /// `GetFloatRect`: the tiles' pixels grown 2 px at the low edges and 2 past the high.
    #[must_use]
    pub fn float_rect(&self) -> SoundRect {
        let [left, bottom, right, top] = self.rect;
        SoundRect {
            left: left as f32 * 20.0 - 2.0,
            bottom: bottom as f32 * 20.0 - 2.0,
            right: right as f32 * 20.0 + 22.0,
            top: top as f32 * 20.0 + 22.0,
        }
    }

    /// `GetFloatRect.Vector2Inside(v)`.
    #[must_use]
    pub fn inside(&self, v: Vec2) -> bool {
        let r = self.float_rect();
        v.x > r.left && v.x < r.right && v.y > r.bottom && v.y < r.top
    }

    /// The head of `Update`: both sound loops.
    pub fn update_loops(&mut self, room: i32, cx: &mut LoopCx<'_>) {
        let rect = self.float_rect();
        self.sound_loop.update_rect(Some(room), rect, cx);
        self.disrupted_loop.update_rect(Some(room), rect, cx);
    }

    /// The tail of `Update` after the zaps: the disruption, the loops' volumes, the two
    /// edges' flicker, the spells off, and the power switch of a `BrokenZeroG` room.
    pub fn step(&mut self, env: &CoilEnv, rng: &mut UnityRandom) {
        self.last_turned_on = self.turned_on;
        if rng.value() < 0.005 {
            self.disruption = self.disruption.max(rng.value());
        }
        self.disruption = (self.disruption - 1.0 / 70.0f32.lerp(300.0, rng.value())).max(0.0);
        self.smooth_disruption = self.smooth_disruption.lerp(self.disruption, 0.2);
        let num = self.smooth_disruption.inverse_lerp(0.1, 1.0);
        self.sound_loop.set_volume((1.0 - num) * self.turned_on);
        self.disrupted_loop
            .set_volume(num * self.turned_on.mathf_pow(0.2));
        for f in &mut self.flicker {
            f[1] = f[0];
            f[3] = (f[3] + (-1.0f32).lerp(1.0, rng.value()) / 10.0).clamp(0.0, 1.0);
            f[2] += 1.0 / 70.0f32.lerp(20.0, f[3]);
            f[0] = (0.5
                + self.smooth_disruption
                    * (0.2f32.lerp(0.1, f[3]) * (std::f32::consts::PI * 2.0 * f[2]).mathf_sin()
                        + (-1.0f32).lerp(1.0, rng.value()) / 20.0))
                .clamp(0.0, 1.0);
        }
        if rng.value() < self.disruption && rng.value() < 0.0025 {
            self.turned_off_counter = rng.range_i32(10, 100);
        }
        if !self.powered {
            self.turned_on = (self.turned_on - 0.1).max(0.0);
        }
        if self.turned_off_counter > 0 {
            self.turned_off_counter -= 1;
            if rng.value() < 0.5 || rng.value() > self.disruption || !self.powered {
                self.turned_on = 0.0;
            } else {
                self.turned_on = rng.value();
            }
            if self.powered {
                self.turned_on = self.turned_on.lerp(1.0, self.zap_lit * rng.value());
            }
            self.smooth_disruption = 1.0;
        } else if self.powered {
            self.turned_on = (self.turned_on + rng.value() / 30.0).min(1.0);
        }
        self.zap_lit = (self.zap_lit - 0.1).max(0.0);
        if env.fully_loaded {
            self.disruption = self.disruption.max(env.gravity);
        }
        if !env.broken_zero_g {
            return;
        }
        if !env.switch_settled_on {
            self.disruption = 1.0;
            if self.powered && rng.value() < 0.2 {
                self.powered = false;
            }
        } else if !self.powered && rng.value() < 0.025 {
            self.powered = true;
        }
    }

    /// `TriggerZap`: the flash (one draw, joining the room's update list at
    /// `flash_order`), the disruption, maybe a spell off, `Zapper_Zap` and the lit flag.
    /// The caller adds the returned flash.
    pub fn trigger_zap(
        &mut self,
        contact: Vec2,
        mass_rad: f32,
        flash_order: u64,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> ZapFlash {
        let flash = ZapFlash::new(flash_order, contact, mass_rad.inverse_lerp(-0.05, 15.0), rng);
        self.disruption = self
            .disruption
            .max(mass_rad.inverse_lerp(-0.05, 9.0) + rng.value() * 0.5);
        self.smooth_disruption = self.disruption;
        if rng.value() < self.disruption && rng.value() < 0.5 {
            self.turned_off_counter = rng.range_i32(2, 15);
        }
        sounds.play_at(SoundId::ZapperZap, contact, 1.0, 1.0, rng);
        self.zap_lit = 1.0;
        flash
    }

    /// The emitters of both loops still in the room's update list.
    pub fn emitters(&self) -> impl Iterator<Item = &crate::LoopEmitter> {
        self.sound_loop
            .emitters()
            .chain(self.disrupted_loop.emitters())
    }

    /// `SoundEmitter.Update` of the emitter that joined the room at `order`, in either
    /// loop (its sound is cut when it dies).
    pub fn update_emitter(&mut self, order: u64, cx: &mut LoopCx<'_>) {
        for l in [&mut self.sound_loop, &mut self.disrupted_loop] {
            let Some(e) = l.emitter_mut(order).filter(|e| !e.slated) else {
                continue;
            };
            let room = e.room;
            e.update(Some(room), cx.camera_room, cx.tick);
            if e.slated {
                cx.sounds.stop_emitter(e.sound);
            }
            l.released.retain(|e| !e.slated);
        }
    }
}

/// `ZapCoil.ZapFlash`: a blue flash with a `LightSource` (made by its first update) that
/// lives a few ticks. Its sprites make seven cosmetic draws a frame (`rw-graphics`).
#[derive(Debug, Clone, PartialEq)]
pub struct ZapFlash {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub size: f32,
    pub life: f32,
    pub last_life: f32,
    pub life_time: f32,
    /// Its `LightSource`'s id, made by the first update.
    pub light: Option<u64>,
    pub slated: bool,
}

impl ZapFlash {
    /// The constructor: one draw for the life time.
    pub fn new(update_order: u64, pos: Vec2, size: f32, rng: &mut UnityRandom) -> Self {
        Self {
            update_order,
            pos,
            last_pos: pos,
            size,
            life: 1.0,
            last_life: 1.0,
            life_time: 1.0f32.lerp(4.0, rng.value()) + 2.0 * size,
            light: None,
            slated: false,
        }
    }

    /// `new LightSource(pos, false, (0, 0, 1), this)`.
    #[must_use]
    pub fn light_source(&self, id: u64, room: i32) -> LightSource {
        LightSource::new(
            id,
            room,
            self.pos,
            false,
            ZAP_BLUE,
            Some(LightTie::Fixture(self.update_order)),
        )
    }

    /// `Update` after the light exists: the life runs down, and past its end the flash
    /// (and its light, which the caller destroys) is gone. Returns that.
    pub fn update(&mut self) -> bool {
        self.last_pos = self.pos;
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        if self.last_life < 0.0 {
            self.slated = true;
        }
        self.slated
    }
}

/// `ZapCoilLight`: a blue light on a `ZapCoil` whose rectangle holds its position.
#[derive(Debug, Clone, PartialEq)]
pub struct ZapCoilLight {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`.
    pub pos: Vec2,
    /// `lightData.randomSeed`.
    pub random_seed: i32,
    /// Its `LightSource`'s id.
    pub light: u64,
    /// The update-list place of `myCoil`, found by `AIMapReady`.
    pub coil: Option<u64>,
}

impl ZapCoilLight {
    /// The constructor's light: blue, tied to the fixture, rad from the seed, alpha 1,
    /// half affected by the palette's darkness.
    #[must_use]
    pub fn light_source(id: u64, room: i32, order: u64, pos: Vec2, random_seed: i32) -> LightSource {
        let mut l = LightSource::new(
            id,
            room,
            pos,
            false,
            ZAP_BLUE,
            Some(LightTie::Fixture(order)),
        );
        l.set_rad = Some(Self::rad(random_seed));
        l.set_alpha = Some(1.0);
        l.affected_by_palette_darkness = 0.5;
        l
    }

    fn rad(random_seed: i32) -> f32 {
        100.0f32.lerp(2000.0, random_seed as f32 / 100.0)
    }

    /// `AIMapReady`: the first coil in the update list whose rectangle holds the light.
    pub fn find_coil(&mut self, coils: &[ZapCoil]) {
        if self.coil.is_none() {
            self.coil = coils
                .iter()
                .find(|c| c.inside(self.pos))
                .map(|c| c.update_order);
        }
    }

    /// `Update`: the light follows the fixture; with a coil, its alpha and rad follow
    /// the coil's `turnedOn` and `smoothDisruption` (one draw). `coil` is that coil's
    /// `(turnedOn, smoothDisruption)`.
    pub fn update(
        &self,
        lights: &mut [LightSource],
        coil: Option<(f32, f32)>,
        rng: &mut UnityRandom,
    ) {
        let Some(l) = LightSource::find_mut(lights, self.light) else {
            return;
        };
        l.set_pos = Some(self.pos);
        if let Some((turned_on, smooth_disruption)) = coil {
            let num = 1.0f32.lerp(0.85, smooth_disruption * rng.value()) * turned_on;
            l.set_alpha = Some(num.mathf_pow(0.8));
            l.set_rad = Some(Self::rad(self.random_seed) * 0.5f32.lerp(1.0, num));
        }
    }
}

#[cfg(test)]
#[path = "zap_coil_tests.rs"]
mod tests;
