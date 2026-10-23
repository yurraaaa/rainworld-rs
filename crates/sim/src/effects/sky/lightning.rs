//! `Lightning` (the `Lightning` and `BkgOnlyLightning` room effects): two sources, a
//! front one and a background one, each flashing after a random wait and queueing
//! thunder that shakes the screen, plays `Thunder`/`Thunder_Close` and makes
//! background noise the creatures' hearing is dulled by. The camera reads the flash
//! (light level, sky and fog colours) each frame ([`Lightning::frame`]). Base game:
//! `rippleFade` is 1, no Watcher `WSK` region, no MSC `RM` exception.
//!
//! Mirrors: Lightning.cs

use rw_random::UnityRandom;

use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::math::float_ext::SCurve;
use crate::{FloatExt, ScreenMovement, SoundId, Vec2};

/// A `Color`, RGBA 0..1.
type Rgba = [f32; 4];

/// `bkgGradient`: blue, then green.
const BKG_GRADIENT: [Rgba; 2] = [
    [50.0 / 255.0, 60.0 / 255.0, 200.0 / 255.0, 1.0],
    [54.0 / 255.0, 1.0, 57.0 / 255.0, 1.0],
];

/// What `Lightning.Update` does to its room and the game besides its own state.
pub trait LightningRoom {
    /// The shared random stream.
    fn rng(&mut self) -> &mut UnityRandom;
    /// `Room.MakeBackgroundNoise`.
    fn make_background_noise(&mut self, noise: f32);
    /// `Room.ScreenMovement`.
    fn screen_movement(&mut self, m: ScreenMovement);
    /// What the source's sound loop and one-shots read of the game.
    fn loop_cx(&mut self) -> LoopCx<'_>;
}

/// What `Lightning.Update` reads of the room and the rain cycle this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightningEnv {
    /// `room.roomSettings.GetEffectAmount(ExtraLoudThunder)`.
    pub extra_loud_thunder: f32,
    /// `rainCycle.TimeUntilRain`.
    pub time_until_rain: i32,
    /// `CycleEndIntensityBoost`.
    pub cycle_end_boost: f32,
}

impl LightningEnv {
    /// `CycleEndIntensityBoost`: 0 until 400 ticks before the cycle's end, 1 at 800
    /// ticks past it.
    #[must_use]
    pub fn boost(cycle_length: i32, timer: i32) -> f32 {
        (timer as f32).inverse_lerp((cycle_length - 400) as f32, (cycle_length + 800) as f32)
    }
}

/// A queued `ThunderSound`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThunderSound {
    pub count_down: i32,
    pub distance: f32,
    pub power: f32,
    pub pan: f32,
}

/// `LightningSource`.
#[derive(Debug, Clone, PartialEq)]
pub struct LightningSource {
    pub wait: i32,
    pub thunder: i32,
    pub thunder_length: i32,
    pub random_level: f32,
    pub random_level_change: i32,
    pub front_lightning_show_in_bkg: bool,
    pub last_intensity: f32,
    pub intensity: f32,
    pub distance: f32,
    pub pan: f32,
    pub power: f32,
    is_bkg: bool,
    pub loop_vol: f32,
    pub loop_pitch: f32,
    pub loop_pan: f32,
    pub noise_counter: f32,
    pub noise_level: f32,
    pub sound_loop: Option<ChunkDynamicSoundLoop>,
    pub thunder_sounds: Vec<ThunderSound>,
}

impl LightningSource {
    #[must_use]
    pub fn new(is_bkg: bool) -> Self {
        Self {
            wait: 0,
            thunder: 0,
            thunder_length: 0,
            random_level: 0.0,
            random_level_change: 0,
            front_lightning_show_in_bkg: false,
            last_intensity: 0.0,
            intensity: 0.0,
            distance: 0.0,
            pan: 0.0,
            power: 0.0,
            is_bkg,
            loop_vol: 0.0,
            loop_pitch: 0.0,
            loop_pan: 0.0,
            noise_counter: 0.0,
            noise_level: 0.0,
            sound_loop: None,
            thunder_sounds: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_bkg(&self) -> bool {
        self.is_bkg
    }

    /// `tF`.
    fn t_f(&self) -> f32 {
        1.0 - self.thunder as f32 / self.thunder_length as f32
    }

    /// `Reset`: the next flash. Seven draws (the pan's second and the length's last).
    pub fn reset(&mut self, intensity: f32, cycle_end_boost: f32, rng: &mut UnityRandom) {
        let top = if self.is_bkg { 120.0 } else { 440.0 };
        self.wait = (10.0f32.lerp(top, rng.value())
            * 1.5f32.lerp(1.0, intensity)
            * (1.0 - cycle_end_boost))
            .to_int_cs();
        self.front_lightning_show_in_bkg = rng.value() < 1.0 / 3.0;
        if self.is_bkg {
            self.distance = 1.0f32
                .lerp(0.5, intensity)
                .lerp(1.0, rng.value().mathf_pow(0.5f32.lerp(2.0, intensity)));
            self.power = rng.value();
        } else {
            self.distance = 0.25f32
                .lerp(0.0, intensity)
                .lerp(0.25, rng.value().mathf_pow(0.5f32.lerp(2.0, intensity)));
            self.power = 0.7f32.lerp(1.0, rng.value());
        }
        self.pan = (-1.0f32).lerp(1.0, rng.value());
        self.pan = self
            .pan
            .lerp(0.0, (1.0 - self.distance) * rng.value().mathf_pow(0.5));
        self.thunder_length = rng.range_i32(1, 10.0f32.lerp(32.0, self.power).to_int_cs());
    }

    /// `Update`, in the order the game makes its calls and draws.
    pub fn update(
        &mut self,
        intensity: f32,
        bkg_only: bool,
        env: LightningEnv,
        room: &mut dyn LightningRoom,
    ) {
        room.make_background_noise(
            self.noise_counter.mathf_pow(1.4)
                * self.noise_level.inverse_lerp(0.1, 0.7).mathf_pow(0.5),
        );
        if self.noise_counter > 0.0 {
            self.noise_counter = (self.noise_counter - 1.0 / 120.0).max(0.0);
        } else {
            self.noise_level = 0.0;
        }
        if !bkg_only {
            self.update_sound_loop(room);
        }
        self.random_level_change -= 1;
        if self.random_level_change < 1 {
            let rng = room.rng();
            self.random_level_change = rng.range_i32(1, 6);
            self.random_level = rng.value();
        }
        if self.wait > 0 {
            self.wait -= 1;
            if self.wait < 1 {
                self.thunder = self.thunder_length;
                if !self.is_bkg
                    || (room.rng().value() * 0.5 < self.power * (1.0 - self.distance) && !bkg_only)
                {
                    self.thunder_sounds.push(ThunderSound {
                        count_down: (self.distance * 100.0).to_int_cs(),
                        distance: self.distance,
                        power: self.power,
                        pan: self.pan,
                    });
                }
            }
        } else {
            self.thunder -= 1;
            if self.thunder < 1 {
                self.reset(intensity, env.cycle_end_boost, room.rng());
            }
        }
        let nearest = self.run_thunder_sounds(intensity, bkg_only, env, room);
        self.update_loop_levels(intensity, nearest, env);
        self.last_intensity = self.intensity;
        self.intensity = if self.thunder < 1 {
            0.0
        } else {
            self.random_level
                .mathf_pow(3.0f32.lerp(0.1, (self.t_f() * std::f32::consts::PI).sin()))
        };
        if env.cycle_end_boost > 0.0 {
            self.intensity = self.intensity.lerp(room.rng().value(), env.cycle_end_boost);
        }
    }

    /// The `soundLoop` block: made on the first tick, then its volume, pitch and pan set
    /// and updated.
    fn update_sound_loop(&mut self, room: &mut dyn LightningRoom) {
        let Some(sound_loop) = &mut self.sound_loop else {
            let mut sound_loop = ChunkDynamicSoundLoop::default();
            sound_loop.sound = Some(SoundId::ElectricityLoop);
            self.sound_loop = Some(sound_loop);
            return;
        };
        sound_loop.set_volume(self.loop_vol);
        sound_loop.set_pitch(self.loop_pitch);
        let mut cx = room.loop_cx();
        let index = cx.room_index;
        sound_loop.update_disembodied(Some(index), self.loop_pan, &mut cx);
        // The `Pan` setter writes the emitter's pan.
        if let Some(e) = sound_loop.emitter {
            cx.sounds.set_emitter_pan(e.sound, self.loop_pan);
        }
    }

    /// The thunder queue: every sound counts down; the nearest sets the loop; those under
    /// 10 shake the screen and those at 0 play. Returns the nearest after counting down.
    fn run_thunder_sounds(
        &mut self,
        intensity: f32,
        bkg_only: bool,
        env: LightningEnv,
        room: &mut dyn LightningRoom,
    ) -> Option<(ThunderSound, i32)> {
        let mut nearest = None;
        let mut best = i32::MAX;
        for i in (0..self.thunder_sounds.len()).rev() {
            self.thunder_sounds[i].count_down -= 1;
            let sound = self.thunder_sounds[i];
            if sound.count_down < best {
                best = sound.count_down;
                nearest = Some((sound, best));
            }
            if sound.count_down < 10 && !bkg_only {
                room.screen_movement(ScreenMovement {
                    source: None,
                    bump: Vec2::ZERO,
                    shake: ((1.0 - sound.distance) * sound.power * intensity).mathf_pow(3.0)
                        * (sound.count_down as f32).lerp_map(
                            10.0,
                            0.0,
                            0.0,
                            0.9 * intensity.inverse_lerp(0.5, 1.0),
                        ),
                });
            }
            if sound.count_down < 1 && !bkg_only {
                let volume = (1.0 - sound.distance)
                    * sound.power
                    * intensity.inverse_lerp(0.2, 1.0).mathf_pow(1.3);
                let cx = &mut room.loop_cx();
                cx.sounds
                    .play_disembodied(SoundId::Thunder, self.pan, volume, 1.0, cx.rng);
                if env.extra_loud_thunder > 0.0 {
                    cx.sounds.play_disembodied(
                        SoundId::ThunderClose,
                        self.pan,
                        volume * env.extra_loud_thunder,
                        1.0,
                        cx.rng,
                    );
                }
                self.noise_counter = 1.0;
                self.noise_level = (1.0 - sound.distance)
                    * sound.power
                    * intensity.inverse_lerp(0.2, 1.0).mathf_pow(1.3);
                self.thunder_sounds.remove(i);
            }
        }
        nearest
    }

    /// `loopVol`, `loopPitch` and `loopPan` follow the nearest queued thunder, else the
    /// next flash; the rain's approach silences them.
    fn update_loop_levels(
        &mut self,
        intensity: f32,
        nearest: Option<(ThunderSound, i32)>,
        env: LightningEnv,
    ) {
        if let Some((sound, count_down)) = nearest {
            self.loop_vol = self
                .loop_vol
                .lerp((1.0 - sound.distance) * sound.power * intensity, 0.2);
            self.loop_pitch = self.loop_pitch.lerp(
                0.5f32.lerp(2.0, (count_down as f32).inverse_lerp(40.0, 5.0)),
                0.2,
            );
            self.loop_pan = self.loop_vol.lerp(sound.pan, 0.2);
        } else {
            self.loop_vol = self.loop_vol.lerp(
                (1.0 - self.distance)
                    * self.power
                    * intensity
                    * (self.wait as f32).inverse_lerp(40.0, 5.0),
                0.04,
            );
            self.loop_pitch = self.loop_pitch.lerp(1.0, 0.5);
            self.loop_pan = self.loop_vol.lerp(self.pan, 0.1);
        }
        self.loop_vol *= (env.time_until_rain as f32).inverse_lerp(0.0, 1200.0);
    }

    /// `LightIntensity(timeStacker)`: the flash as drawn, with its flicker (up to three
    /// draws). The front source of a background-only room is dark without drawing.
    pub fn light_intensity(&self, bkg_only: bool, time_stacker: f32, rng: &mut UnityRandom) -> f32 {
        if !self.is_bkg && bkg_only {
            return 0.0;
        }
        let mut num = self.last_intensity.lerp(self.intensity, time_stacker);
        if rng.value() < 1.0 / 3.0 {
            let target = if rng.value() < 0.5 { 1.0 } else { 0.0 };
            num = num.lerp(target, rng.value() * num);
        }
        num.s_curve(0.5)
    }
}

/// What the camera reads of a room's `Lightning` in one `DrawUpdate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightningFrame {
    /// `CurrentLightLevel` as `num4`; `None` for a background-only room (the rain
    /// cycle's light stays).
    pub light_level: Option<f32>,
    /// `CurrentBackgroundColor`: palette texel (0, 7).
    pub background: Rgba,
    /// `CurrentFogColor`: palette texel (1, 7).
    pub fog: Rgba,
}

/// `Lightning`, an object in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct Lightning {
    pub update_order: u64,
    pub intensity: f32,
    pub bkg_only: bool,
    pub sources: [LightningSource; 2],
}

impl Lightning {
    #[must_use]
    pub fn new(update_order: u64, intensity: f32, bkg_only: bool) -> Self {
        Self {
            update_order,
            intensity,
            bkg_only,
            sources: [LightningSource::new(false), LightningSource::new(true)],
        }
    }

    /// `Update`: the front source, then the background one.
    pub fn update(&mut self, env: LightningEnv, room: &mut dyn LightningRoom) {
        for source in &mut self.sources {
            source.update(self.intensity, self.bkg_only, env, room);
        }
    }

    /// `SoundEmitter.Update` of the loop emitter that joined the room at `order`; its
    /// sound is cut when it dies.
    pub fn update_emitter(&mut self, order: u64, cx: &mut LoopCx<'_>) {
        for source in &mut self.sources {
            let Some(sound_loop) = &mut source.sound_loop else {
                continue;
            };
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
    pub fn emitters(&self) -> impl Iterator<Item = &crate::LoopEmitter> {
        self.sources
            .iter()
            .filter_map(|s| s.sound_loop.as_ref())
            .flat_map(ChunkDynamicSoundLoop::emitters)
    }

    /// `InverseLerp(1.1, 0, distance) * power` of the background source.
    fn bkg_reach(&self) -> f32 {
        let s = &self.sources[1];
        s.distance.inverse_lerp(1.1, 0.0) * s.power
    }

    /// `CurrentLightLevel`.
    pub fn current_light_level(&self, time_stacker: f32, rng: &mut UnityRandom) -> f32 {
        let flash = self.sources[0].light_intensity(self.bkg_only, time_stacker, rng);
        (-1.0f32).lerp(
            1.0,
            flash * self.intensity * self.bkg_reach().mathf_pow(0.4),
        )
    }

    /// `CurrentBackgroundColor` over the palette's `sky`.
    pub fn current_background_color(
        &self,
        time_stacker: f32,
        sky: Rgba,
        rng: &mut UnityRandom,
    ) -> Rgba {
        let mut num = self.sources[1].light_intensity(self.bkg_only, time_stacker, rng);
        if self.sources[0].front_lightning_show_in_bkg {
            num = num.max(self.sources[0].light_intensity(self.bkg_only, time_stacker, rng));
        }
        let gradient = BKG_GRADIENT[Self::gradient_index(num)];
        Self::color_lerp(
            sky,
            gradient,
            (num.mathf_pow(3.0) * self.bkg_reach() * self.intensity).mathf_pow(1.2),
        )
    }

    /// `CurrentFogColor` over the palette's `fog`.
    pub fn current_fog_color(&self, time_stacker: f32, fog: Rgba, rng: &mut UnityRandom) -> Rgba {
        let (front, back) = (&self.sources[0], &self.sources[1]);
        let f = (front.light_intensity(self.bkg_only, time_stacker, rng) * 0.5)
            .max(back.light_intensity(self.bkg_only, time_stacker, rng));
        let gradient = BKG_GRADIENT[Self::gradient_index(f)];
        let front_part = (front.light_intensity(self.bkg_only, time_stacker, rng)
            * front.distance.inverse_lerp(1.1, 0.0)
            * front.power)
            .mathf_pow(1.5)
            * 0.5;
        let back_part = (back.light_intensity(self.bkg_only, time_stacker, rng)
            * back.distance.inverse_lerp(1.1, 0.0)
            * back.power)
            .mathf_pow(1.2)
            * 0.5;
        Self::color_lerp(fog, gradient, (front_part + back_part) * self.intensity)
    }

    /// `Custom.IntClamp(FloorToInt(Pow(f, 10) * bkgGradient.Length), 0, Length - 1)`.
    fn gradient_index(f: f32) -> usize {
        ((f.mathf_pow(10.0) * 2.0).floor() as i32).clamp(0, 1) as usize
    }

    /// `Color.Lerp`.
    fn color_lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
    }

    /// What `RoomCamera.DrawUpdate` reads at `time_stacker` over the palette's sky and
    /// fog colours: the light level (unless background-only), then the two colours.
    pub fn frame(
        &self,
        time_stacker: f32,
        sky: Rgba,
        fog: Rgba,
        rng: &mut UnityRandom,
    ) -> LightningFrame {
        let light_level = (!self.bkg_only).then(|| self.current_light_level(time_stacker, rng));
        LightningFrame {
            light_level,
            background: self.current_background_color(time_stacker, sky, rng),
            fog: self.current_fog_color(time_stacker, fog, rng),
        }
    }
}

#[cfg(test)]
#[path = "tests/lightning.rs"]
mod tests;
