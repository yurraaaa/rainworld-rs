//! The light fixtures `Room.Loaded` makes for placed `LightFixture` objects that need no
//! electricity of their own: the `Redlight` (with its `RedlightLight` and flat light), the
//! `HolyFire` (three lights, a flat light and the `HolyFireSprite`s it sheds) and the
//! `SlimeMoldLight` (five soft discs, no `LightSource`). Their lights are
//! [`LightSource`]s in the room's `light_sources`, found by id; every draw is on the
//! shared stream except the `SlimeMoldLight`'s, made under `InitState` with the global
//! state restored. A red light in a room with `BrokenZeroG` reads the room's electric
//! power (`Room.ElectricPower`, which the world's `BrokenAntiGravity` moves: D34).
//! Sprites: `rw-graphics` `effects/lights/light_fixture.rs`.
//!
//! Mirrors: LightFixture.cs, Redlight.cs, HolyFire.cs, SlimeMoldLight.cs

use rw_random::UnityRandom;

use crate::color::Hsl;
use crate::{FloatExt, LightSource, SoundId, Sounds, Vec2};

/// One `Redlight` fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct Redlight {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`.
    pub pos: Vec2,
    /// `lightData.randomSeed`.
    pub random_seed: i32,
    pub submersible: bool,
    /// `flatLightSource`'s id.
    pub flat_light: u64,
    /// `lightSource`'s id, made by the first `Update`.
    pub light: Option<u64>,
    pub flicker_wait: i32,
    pub flicker: i32,
    pub sin: f32,
    pub switch_on: f32,
    pub gravity_dependent: bool,
    pub powered: bool,
}

impl Redlight {
    /// `RedlightLight`'s rad is this fraction of its flicker.
    const LIGHT_RAD_SCALE: f32 = 0.16;

    /// The constructor: `sin` and `flickerWait` from two draws. `broken_zero_g` is the
    /// room's `BrokenZeroG` amount. The fixture has no room yet, so `NoElectricity` is 0
    /// and it starts powered unless `gravityDependent`.
    pub fn new(
        update_order: u64,
        flat_light: u64,
        pos: Vec2,
        random_seed: i32,
        submersible: bool,
        broken_zero_g: f32,
        rng: &mut UnityRandom,
    ) -> Self {
        let sin = rng.value();
        let flicker_wait = rng.range_i32(0, 700);
        let gravity_dependent = broken_zero_g > 0.0 && random_seed > 0;
        Self {
            update_order,
            pos,
            random_seed,
            submersible,
            flat_light,
            light: None,
            flicker_wait,
            flicker: 0,
            sin,
            switch_on: random_seed as f32 / 100.0,
            gravity_dependent,
            powered: !gravity_dependent,
        }
    }

    /// The flat light as the constructor makes it: `(1, 0.05, 0.05)`, tied to the fixture.
    #[must_use]
    pub fn flat_light_source(id: u64, room: i32, update_order: u64, pos: Vec2) -> LightSource {
        let mut l = LightSource::new(
            id,
            room,
            pos,
            false,
            [1.0, 0.05, 0.05],
            Some(crate::LightTie::Fixture(update_order)),
        );
        l.flat = true;
        l
    }

    /// `new RedlightLight(pos, (1, 0, 0), this, submersible)`: a light of the `LightMask0`
    /// element.
    #[must_use]
    pub fn mask_light_source(
        id: u64,
        room: i32,
        update_order: u64,
        pos: Vec2,
        submersible: bool,
    ) -> LightSource {
        let mut l = LightSource::new(
            id,
            room,
            pos,
            false,
            [1.0, 0.0, 0.0],
            Some(crate::LightTie::Fixture(update_order)),
        );
        l.submersible = submersible;
        l.element = crate::LightElement::LightMask0;
        l
    }

    /// `Update` after the `RedlightLight` exists: the power switching of a gravity-dependent
    /// light (`electric_power` is `Room.ElectricPower`), the flicker and the sets on its
    /// two lights. Draws as the game's, in its order.
    pub fn update(
        &mut self,
        lights: &mut [LightSource],
        electric_power: f32,
        sounds: &mut Sounds,
        rng: &mut UnityRandom,
    ) {
        let (Some(main), flat) = (self.light, self.flat_light) else {
            return;
        };
        let no_electricity = 1.0 - electric_power;
        if self.gravity_dependent {
            if self.powered {
                if no_electricity < 0.6 && rng.value() < 0.05 {
                    self.powered = false;
                }
            } else {
                Self::set_alphas(lights, main, flat, 0.0, 0.0);
                if no_electricity <= 0.65_f32.lerp(0.95, self.switch_on)
                    || rng.value() >= 1.0 / 20.0_f32.lerp(80.0, self.switch_on)
                {
                    return;
                }
                self.powered = true;
                self.flicker = rng.range_i32(1, 15);
                sounds.play_at(SoundId::RedLightOn, self.pos, 1.0, 1.0, rng);
            }
        }
        let num = if self.gravity_dependent {
            no_electricity
        } else {
            1.0
        };
        self.flicker_wait -= 1;
        self.sin += 1.0 / 60.0_f32.lerp(80.0, rng.value());
        let wave = f64::from(self.sin * std::f32::consts::PI * 2.0).sin() as f32 * 0.5;
        if let Some(l) = LightSource::find_mut(lights, main) {
            l.set_rad = Some(290.0_f32.lerp(310.0, 0.5 + wave) * Self::LIGHT_RAD_SCALE);
        }
        if self.flicker_wait < 1 {
            self.flicker_wait = rng.range_i32(0, 700);
            self.flicker = rng.range_i32(1, 15);
        }
        if self.flicker > 0 {
            self.flicker -= 1;
            if rng.value() < 1.0 / 3.0 {
                let n = rng.value().mathf_pow(0.5);
                Self::set_alphas(lights, main, flat, n * num, n * 0.25 * num);
                if let Some(l) = LightSource::find_mut(lights, flat) {
                    l.set_rad = Some(n * 30.0);
                }
            }
        } else {
            let alpha = 0.9_f32.lerp(1.0, 0.5 + wave * rng.value()) * num;
            Self::set_alphas(lights, main, flat, alpha, 0.25 * num);
            let rad = 28.0_f32.lerp(32.0, rng.value());
            if let Some(l) = LightSource::find_mut(lights, flat) {
                l.set_rad = Some(rad);
            }
        }
        for id in [main, flat] {
            if let Some(l) = LightSource::find_mut(lights, id) {
                l.set_pos = Some(self.pos);
            }
        }
    }

    fn set_alphas(
        lights: &mut [LightSource],
        main: u64,
        flat: u64,
        main_alpha: f32,
        flat_alpha: f32,
    ) {
        if let Some(l) = LightSource::find_mut(lights, main) {
            l.set_alpha = Some(main_alpha);
        }
        if let Some(l) = LightSource::find_mut(lights, flat) {
            l.set_alpha = Some(flat_alpha);
        }
    }
}

/// One `HolyFire` fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct HolyFire {
    pub update_order: u64,
    pub pos: Vec2,
    /// The three coloured lights' ids.
    pub lights: [u64; 3],
    pub flat_light: u64,
    pub get_to_positions: [Vec2; 3],
    pub get_to_rads: [f32; 3],
}

impl HolyFire {
    /// The light `i` as the constructor makes it: `HSL2RGB(Lerp(0.01, 0.07, i / 2), 1,
    /// 0.5)`, alpha 1 on its first update.
    #[must_use]
    pub fn light_source(i: usize, id: u64, room: i32, update_order: u64, pos: Vec2) -> LightSource {
        let hue = 0.01_f32.lerp(0.07, i as f32 / 2.0);
        let mut l = LightSource::new(
            id,
            room,
            pos,
            false,
            Hsl::new(hue, 1.0, 0.5).rgb(),
            Some(crate::LightTie::Fixture(update_order)),
        );
        l.set_alpha = Some(1.0);
        l
    }

    /// The flat white light.
    #[must_use]
    pub fn flat_light_source(id: u64, room: i32, update_order: u64, pos: Vec2) -> LightSource {
        let mut l = LightSource::new(
            id,
            room,
            pos,
            false,
            [1.0; 3],
            Some(crate::LightTie::Fixture(update_order)),
        );
        l.flat = true;
        l
    }

    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, lights: [u64; 3], flat_light: u64) -> Self {
        Self {
            update_order,
            pos,
            lights,
            flat_light,
            get_to_positions: [Vec2::ZERO; 3],
            get_to_rads: [0.0; 3],
        }
    }

    /// `Update`: each light's target position and rad now and then (the draws), the
    /// lights easing toward them, the flat light flickering, and a new `HolyFireSprite`
    /// (returned, for the room to add).
    pub fn update(&mut self, lights: &mut [LightSource], rng: &mut UnityRandom) -> HolyFireSprite {
        for i in 0..3 {
            if rng.value() < 0.2 {
                let dir = Vec2::random_unit(rng);
                self.get_to_positions[i] = dir * (50.0 * rng.value());
            }
            if rng.value() < 0.2 {
                let far = 400.0_f32.lerp(200.0, i as f32 / 2.0);
                self.get_to_rads[i] = 50.0_f32.lerp(far, rng.value().mathf_pow(0.5));
            }
            if let Some(l) = LightSource::find_mut(lights, self.lights[i]) {
                l.set_pos = Some(l.pos.lerp(self.pos + self.get_to_positions[i], 0.2));
                l.set_rad = Some(l.rad.lerp(self.get_to_rads[i], 0.2));
            }
        }
        let mut sprite = HolyFireSprite::new(self.pos, rng);
        sprite.fire = self.update_order;
        let alpha = 0.2_f32.lerp(0.4, rng.value());
        let rad = 24.0_f32.lerp(33.0, rng.value());
        if let Some(l) = LightSource::find_mut(lights, self.flat_light) {
            l.set_alpha = Some(alpha);
            l.set_rad = Some(rad);
            l.set_pos = Some(self.pos);
        }
        sprite
    }
}

/// `HolyFire.HolyFireSprite`: a rising ember, a `CosmeticSprite` in the room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct HolyFireSprite {
    pub update_order: u64,
    /// The place in the update list of the `HolyFire` that shed it (port-only, for replies).
    pub fire: u64,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub life_time: f32,
    pub life: f32,
    pub last_life: f32,
}

impl HolyFireSprite {
    /// The constructor: `RNV() * (1.5 * value)` (two draws) and a life time (one).
    pub fn new(pos: Vec2, rng: &mut UnityRandom) -> Self {
        let dir = Vec2::random_unit(rng);
        let vel = dir * (1.5 * rng.value());
        Self {
            update_order: 0,
            fire: 0,
            slated: false,
            pos,
            last_pos: pos,
            vel,
            life_time: 10.0_f32.lerp(40.0, rng.value()),
            life: 1.0,
            last_life: 0.0,
        }
    }

    /// `CosmeticSprite.Update` then `Update`: slows and rises with a random nudge (two
    /// draws), and burns down; destroyed once `life` is below 0.
    pub fn update(&mut self, rng: &mut UnityRandom) {
        self.last_pos = self.pos;
        self.pos += self.vel;
        self.vel *= 0.8;
        self.vel.y += 0.4;
        let dir = Vec2::random_unit(rng);
        self.vel += dir * (rng.value() * 0.5);
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        if self.life < 0.0 {
            self.slated = true;
        }
    }
}

/// One `SlimeMoldLight` fixture: five soft discs round its position.
#[derive(Debug, Clone, PartialEq)]
pub struct SlimeMoldLight {
    pub update_order: u64,
    /// `placedObject.pos`.
    pub pos: Vec2,
    /// `lightData.randomSeed`.
    pub random_seed: i32,
    /// `lstPos`: the position the discs were last laid out for.
    pub last_laid_out: Vec2,
    pub positions: [Vec2; 5],
    /// Per disc: its size in pixels and its alpha.
    pub sizes_and_alphas: [[f32; 2]; 5],
}

impl SlimeMoldLight {
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, random_seed: i32) -> Self {
        Self {
            update_order,
            pos,
            random_seed,
            last_laid_out: Vec2::ZERO,
            positions: [Vec2::ZERO; 5],
            sizes_and_alphas: [[0.0; 2]; 5],
        }
    }

    /// `Update`: lays the discs out when the position is not the last one's.
    pub fn update(&mut self, rng: &mut UnityRandom) {
        if self.pos != self.last_laid_out {
            self.last_laid_out = self.pos;
            self.reset_positions(rng);
        }
    }

    /// `ResetPositions`: four draws a disc from `InitState((int)(pos.x + pos.y))`, the
    /// global state restored afterwards.
    fn reset_positions(&mut self, rng: &mut UnityRandom) {
        let saved = rng.state();
        rng.init_state((self.pos.x + self.pos.y).to_int_cs());
        for i in 0..5 {
            let dir = Vec2::random_unit(rng);
            self.positions[i] = dir * rng.value();
            self.sizes_and_alphas[i][0] = 70.0_f32.lerp(200.0, rng.value());
            self.sizes_and_alphas[i][1] = 0.2_f32.lerp(0.35, rng.value());
        }
        rng.set_state(saved);
    }
}

#[cfg(test)]
#[path = "tests/light_fixture.rs"]
mod tests;
