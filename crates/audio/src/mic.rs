//! `VirtualMicrophone`: the listener and the formulas that turn a sound's position into
//! pan, volume and low-pass (`rainworld-re/notes/sound.md` sections 2.3, 2.4).

use rw_sim::{FloatExt, Vec2, Water};

/// Where the camera's followed creature is, as `VirtualMicrophone.Update` reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Follow {
    /// Its main body chunk.
    Body(Vec2),
    /// In a shortcut: the middle of its vessel's tile while that is in this room.
    Shortcut(Option<Vec2>),
    /// The followed creature exists only abstractly (destroyed): listener at the
    /// screen centre, no underwater effect from it.
    Abstract,
}

/// The camera's state for one microphone tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListenerInput {
    /// `RoomCamera.pos`: bottom left of the viewed screen.
    pub camera_pos: Vec2,
    /// `options.ScreenSize.x`.
    pub screen_width: f32,
    /// `None` when the camera follows nothing.
    pub follow: Option<Follow>,
}

/// What a source is set to on one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceMix {
    pub volume: f32,
    pub pan: f32,
    pub pitch: f32,
    /// `None`: the low-pass filter is off.
    pub cutoff: Option<f32>,
}

/// One camera's microphone: listener points, underwater amount, doppler block and
/// volume groups.
#[derive(Debug, Clone, PartialEq)]
pub struct Microphone {
    listener: Vec2,
    last_listener: Vec2,
    camera_pos: Vec2,
    screen_width: f32,
    /// 0 with the listener above the water, 1 well below it.
    pub under_water: f32,
    pub doppler_block: f32,
    /// Fades in from 0 when the microphone is made.
    in_world_volume: f32,
    /// `soundLoader.volumeExponent`.
    exponent: f32,
}

impl Microphone {
    /// `listenerRad`.
    pub const LISTENER_RAD: f32 = 1000.0;
    /// The viewed screen's height in `DistanceFromViewedScreen`.
    pub const SCREEN_HEIGHT: f32 = 768.0;
    /// `options.soundEffectsVolume` default.
    pub const SFX_VOLUME: f32 = 0.8;
    /// Volume groups `VirtualMicrophone.volumeGroups` has.
    pub const GROUPS: usize = 5;

    #[must_use]
    pub fn new(exponent: f32) -> Self {
        Self {
            listener: Vec2::ZERO,
            last_listener: Vec2::ZERO,
            camera_pos: Vec2::ZERO,
            screen_width: 1366.0,
            under_water: 0.0,
            doppler_block: 0.0,
            in_world_volume: 0.0,
            exponent,
        }
    }

    /// `VirtualMicrophone.Update`, once per game tick. Rain, deafness, fade to black
    /// and the global muffle are not ported and count as 0.
    pub fn tick(&mut self, input: &ListenerInput, water: Option<&Water>) {
        self.in_world_volume = self.in_world_volume.lerp_and_tick(1.0, 0.03, 1.0 / 60.0);
        self.camera_pos = input.camera_pos;
        self.screen_width = input.screen_width;
        self.last_listener = self.listener;
        self.listener = input.camera_pos + Vec2::new(512.0, 384.0);
        match input.follow {
            Some(Follow::Body(pos) | Follow::Shortcut(Some(pos))) => self.listener = pos,
            Some(Follow::Shortcut(None) | Follow::Abstract) | None => {}
        }
        if input.follow.is_some() {
            self.under_water = water.map_or(0.0, |w| {
                let level = w.level_at(self.listener.x);
                self.listener.y.inverse_lerp(level + 30.0, level - 30.0)
            });
            self.doppler_block = if matches!(input.follow, Some(Follow::Shortcut(_))) {
                (self.doppler_block + 0.05).min(1.0)
            } else {
                (self.doppler_block - 0.05).max(0.0)
            };
        } else {
            self.under_water = 0.0;
            self.doppler_block = 0.5;
        }
    }

    /// `soundLoader.volumeExponent`.
    #[must_use]
    pub fn exponent(&self) -> f32 {
        self.exponent
    }

    /// `listenerPoint` (not interpolated).
    #[must_use]
    pub fn listener(&self) -> Vec2 {
        self.listener
    }

    /// The listener between the last two ticks.
    #[must_use]
    pub fn listener_at(&self, time_stacker: f32) -> Vec2 {
        self.last_listener.lerp(self.listener, time_stacker)
    }

    /// `volumeGroups`: [0] everything, [2] ambience (also [0]), [3] the rain loops.
    #[must_use]
    pub fn volume_groups(&self) -> [f32; Self::GROUPS] {
        let v = self.in_world_volume;
        [v, 1.0, v, v, 1.0]
    }

    /// `PanFromPoint`: -1 at 1000 px left of the listener, 1 at 1000 px right.
    #[must_use]
    pub fn pan_from_point(&self, ps: Vec2, time_stacker: f32) -> f32 {
        let x = self.listener_at(time_stacker).x;
        -1.0 + 2.0
            * ps.x
                .inverse_lerp(x - Self::LISTENER_RAD, x + Self::LISTENER_RAD)
    }

    /// `VolFromPoint`: full within 500 px (listener distance plus distance off the
    /// viewed screen, divided by `range`), silent at 2000.
    #[must_use]
    pub fn vol_from_point(&self, ps: Vec2, time_stacker: f32, range: f32) -> f32 {
        let dist =
            ps.distance(self.listener_at(time_stacker)) + self.distance_from_viewed_screen(ps);
        (dist / range).inverse_lerp(Self::LISTENER_RAD * 2.0, Self::LISTENER_RAD / 2.0)
    }

    /// `RoomCamera.DistanceFromViewedScreen`.
    #[must_use]
    pub fn distance_from_viewed_screen(&self, v: Vec2) -> f32 {
        let c = self.camera_pos;
        let inside = Vec2::new(
            v.x.clamp(c.x, c.x + self.screen_width),
            v.y.clamp(c.y, c.y + Self::SCREEN_HEIGHT),
        );
        v.distance(inside)
    }

    /// `SoundObject.SetVolume`: the source volume for `value` (distance factor times
    /// emitter volume), the sound's own volume and its volume group.
    #[must_use]
    pub fn source_volume(&self, value: f32, sound_volume: f32, group: f32) -> f32 {
        (value * sound_volume * group * Self::SFX_VOLUME)
            .powf(self.exponent)
            .clamp(0.0, 1.0)
    }

    /// `SetLowPassCutOff`: the cutoff in Hz, `None` with the filter disabled.
    #[must_use]
    pub fn low_pass_cutoff(effect: f32, ignore_effects: f32) -> Option<f32> {
        let effect = effect * (1.0 - ignore_effects);
        (effect > 0.0).then(|| 22000.0.lerp(1500.0, effect.sqrt()))
    }

    /// The low-pass effect for a positioned sound at `ps`: the listener's, or a
    /// submerged source's own.
    #[must_use]
    pub fn positioned_muffle(&self, ps: Vec2, water: Option<&Water>) -> f32 {
        let source = water.map_or(0.0, |w| {
            let level = w.level_at(ps.x);
            0.75 * ps.y.inverse_lerp(level + 20.0, level - 20.0)
        });
        self.under_water.max(source)
    }
}

/// The doppler pitch of a moving source or listener, sampled every 0.05 s of real time
/// (`Time.time`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Doppler {
    last_time: f32,
    last_dist: f32,
    pitch: f32,
}

impl Doppler {
    const INTERVAL: f32 = 0.05;

    #[must_use]
    pub fn new(now: f32) -> Self {
        Self {
            last_time: now,
            last_dist: 0.0,
            pitch: 1.0,
        }
    }

    /// The pitch factor at `now` (seconds) for a listener `dist` away.
    pub fn update(&mut self, now: f32, dist: f32, doppler_fac: f32, doppler_block: f32) -> f32 {
        let dt = now - self.last_time;
        if dt > Self::INTERVAL {
            self.last_time = now;
            let closing = (self.last_dist - dist) / dt;
            self.last_dist = dist;
            self.pitch =
                (1.0 + closing / 4000.0 * doppler_fac * (1.0 - doppler_block)).clamp(0.1, 2.0);
        }
        self.pitch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Microphone {
        /// A microphone that has followed a body at `listener` for two ticks.
        fn following(listener: Vec2) -> Self {
            let mut mic = Self::new(1.8);
            let input = ListenerInput {
                camera_pos: Vec2::ZERO,
                screen_width: 1366.0,
                follow: Some(Follow::Body(listener)),
            };
            mic.tick(&input, None);
            mic.tick(&input, None);
            mic
        }
    }

    #[test]
    fn pan_is_linear_over_a_thousand_pixels() {
        let mic = Microphone::following(Vec2::new(500.0, 300.0));
        assert_eq!(mic.pan_from_point(Vec2::new(500.0, 900.0), 1.0), 0.0);
        assert_eq!(mic.pan_from_point(Vec2::new(1000.0, 0.0), 1.0), 0.5);
        assert_eq!(mic.pan_from_point(Vec2::new(-500.0, 0.0), 1.0), -1.0);
        assert_eq!(mic.pan_from_point(Vec2::new(5000.0, 0.0), 1.0), 1.0);
    }

    #[test]
    fn volume_falls_from_500_to_2000_px() {
        let mic = Microphone::following(Vec2::new(100.0, 400.0));
        assert_eq!(mic.vol_from_point(Vec2::new(500.0, 400.0), 1.0, 1.0), 1.0);
        let half = mic.vol_from_point(Vec2::new(1350.0, 400.0), 1.0, 1.0);
        assert!((half - 0.5).abs() < 1e-6);
        // 100 px past the screen's right edge adds to the 1366 px.
        let off = mic.vol_from_point(Vec2::new(1466.0, 400.0), 1.0, 1.0);
        assert!((off - 1466.0f32.inverse_lerp(2000.0, 500.0)).abs() < 1e-6);
        // rangeFac 0.5 doubles the distance.
        let near = mic.vol_from_point(Vec2::new(600.0, 400.0), 1.0, 0.5);
        assert!((near - 1000.0f32.inverse_lerp(2000.0, 500.0)).abs() < 1e-6);
    }

    #[test]
    fn source_volume_is_the_clamped_power() {
        let mut mic = Microphone::new(1.8);
        assert_eq!(mic.volume_groups()[0], 0.0);
        let input = ListenerInput {
            camera_pos: Vec2::ZERO,
            screen_width: 1366.0,
            follow: None,
        };
        for _ in 0..100 {
            mic.tick(&input, None);
        }
        let g = mic.volume_groups()[0];
        assert_eq!(g, 1.0);
        let v = mic.source_volume(1.0, 0.5, g);
        assert!((v - 0.4f32.powf(1.8)).abs() < 1e-6);
        assert_eq!(mic.source_volume(3.0, 1.0, g), 1.0);
        assert_eq!(mic.listener(), Vec2::new(512.0, 384.0));
        assert_eq!(mic.doppler_block, 0.5);
    }

    #[test]
    fn low_pass_cutoff_scales_with_the_root_of_the_effect() {
        assert_eq!(Microphone::low_pass_cutoff(0.0, 0.0), None);
        assert_eq!(Microphone::low_pass_cutoff(1.0, 1.0), None);
        assert_eq!(Microphone::low_pass_cutoff(1.0, 0.0), Some(1500.0));
        assert_eq!(Microphone::low_pass_cutoff(0.25, 0.0), Some(11750.0));
        assert_eq!(Microphone::low_pass_cutoff(0.5, 0.5), Some(11750.0));
    }

    #[test]
    fn doppler_samples_every_twentieth_second() {
        let mut d = Doppler::new(0.0);
        assert_eq!(d.update(0.01, 100.0, 1.0, 0.0), 1.0);
        // First sample: the distance went from 0 to 100 in 0.1 s (moving away).
        assert!((d.update(0.1, 100.0, 1.0, 0.0) - 0.75).abs() < 1e-6);
        assert!((d.update(0.12, 0.0, 1.0, 0.0) - 0.75).abs() < 1e-6);
        // Closing 40 px in 0.1 s, half blocked.
        assert!((d.update(0.2, 60.0, 1.0, 0.5) - 1.05).abs() < 1e-5);
    }
}
