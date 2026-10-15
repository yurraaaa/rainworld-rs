//! `AmbientSoundPlayer`: one endless loop per `AmbientSounds:` entry of the room
//! (`rainworld-re/notes/sound.md` section 2.5).

use rw_assets::{AmbientKind, AmbientSound};
use rw_sim::{FloatExt, Vec2};

use crate::mic::{Doppler, Microphone, SourceMix};

/// The mix of one ambient loop.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientPlayer {
    pub sound: AmbientSound,
    doppler: Doppler,
}

impl AmbientPlayer {
    #[must_use]
    pub fn new(sound: AmbientSound, now: f32) -> Self {
        Self {
            sound,
            doppler: Doppler::new(now),
        }
    }

    /// The clip's name in the ambient bundle (`LoadAsset` without ".ogg").
    #[must_use]
    pub fn clip_name(&self) -> &str {
        let s = &self.sound.sample;
        s.len()
            .checked_sub(4)
            .filter(|&i| s.is_char_boundary(i) && s[i..].eq_ignore_ascii_case(".ogg"))
            .map_or(s, |i| &s[..i])
    }

    /// `ambientSoundCloseEnoughCheck`: an `OMNI` loop keeps playing into a room with
    /// an `OMNI` entry of the same sample.
    #[must_use]
    pub fn carries_over(a: &AmbientSound, b: &AmbientSound) -> bool {
        a.kind == AmbientKind::Omni && b.kind == AmbientKind::Omni && a.sample == b.sample
    }

    /// `AmbientSoundPlayer.DrawUpdate` with the listener at `listener` and `now` seconds
    /// of real time.
    pub fn mix(&mut self, mic: &Microphone, listener: Vec2, now: f32) -> SourceMix {
        let groups = mic.volume_groups();
        let factor = self.factor(listener);
        let volume = (factor * groups[0] * groups[2] * self.sound.volume * mic.sfx_volume)
            .powf(mic.exponent())
            .clamp(0.0, 1.0);
        SourceMix {
            volume,
            pitch: self.doppler_pitch(mic, listener, now) * mic.time_speed * self.sound.pitch,
            pan: self.pan(listener),
            cutoff: Microphone::low_pass_cutoff(mic.under_water, 0.0),
        }
    }

    /// `Vol`: 1, or a spot's fall-off from `rad` to its taper radius.
    fn factor(&self, listener: Vec2) -> f32 {
        match (self.sound.kind, self.sound.taper_rad()) {
            (AmbientKind::Spot { pos, rad, .. }, Some(taper)) => listener
                .distance(Vec2::new(pos[0], pos[1]))
                .inverse_lerp(taper, rad),
            _ => 1.0,
        }
    }

    fn pan(&self, listener: Vec2) -> f32 {
        match (self.sound.kind, self.sound.taper_rad()) {
            (AmbientKind::Directional { direction }, _) => direction[0],
            (AmbientKind::Spot { pos, .. }, Some(taper)) => {
                listener
                    .x
                    .lerp_map(pos[0] - taper, pos[0] + taper, 1.0, -1.0)
            }
            _ => 0.0,
        }
    }

    /// `Pitch`: the doppler factor from the listener's motion along a directional
    /// sound's axis or toward a spot.
    fn doppler_pitch(&mut self, mic: &Microphone, listener: Vec2, now: f32) -> f32 {
        if self.sound.doppler_fac == 0.0 {
            return 1.0;
        }
        let dist = match self.sound.kind {
            AmbientKind::Omni => return 1.0,
            AmbientKind::Directional { direction } => {
                // `RotateAroundOrigo(listener, VecToDeg(dir.x, -dir.y)).y`.
                let deg = direction[0].atan2(-direction[1]).to_degrees();
                let rad = -deg.to_radians();
                rad.sin() * listener.x + rad.cos() * listener.y
            }
            AmbientKind::Spot { pos, .. } => listener.distance(Vec2::new(pos[0], pos[1])),
        };
        self.doppler
            .update(now, dist, self.sound.doppler_fac, mic.doppler_block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mic::{Follow, ListenerInput};

    impl Microphone {
        /// A microphone at full in-world volume listening at `listener`.
        fn settled_at(listener: Vec2) -> Self {
            let mut mic = Self::new(1.8);
            let input = ListenerInput {
                camera_pos: Vec2::ZERO,
                screen_width: 1366.0,
                follow: Some(Follow::Body(listener)),
            };
            for _ in 0..100 {
                mic.tick(&input, None);
            }
            mic
        }
    }

    impl AmbientPlayer {
        fn parsed(line: &str) -> Self {
            let sound = AmbientSound::parse_list(line).expect("parses").remove(0);
            Self::new(sound, 0.0)
        }
    }

    #[test]
    fn omni_is_centred_at_its_volume() {
        let mic = Microphone::settled_at(Vec2::new(300.0, 300.0));
        let mut p = AmbientPlayer::parsed("OMNI><AM_ENV-Forest.ogg><0><0><0.5><1.2");
        assert_eq!(p.clip_name(), "AM_ENV-Forest");
        let mix = p.mix(&mic, mic.listener(), 1.0);
        assert!((mix.volume - 0.4f32.powf(1.8)).abs() < 1e-6);
        assert_eq!((mix.pitch, mix.pan, mix.cutoff), (1.2, 0.0, None));
    }

    #[test]
    fn directional_pans_by_its_direction() {
        let mic = Microphone::settled_at(Vec2::new(300.0, 300.0));
        let mut p = AmbientPlayer::parsed("DIR><wind.ogg><0><0><1><1><90><0");
        let mix = p.mix(&mic, mic.listener(), 1.0);
        assert!((mix.pan - 1.0).abs() < 1e-6);
        assert_eq!(mix.pitch, 1.0);
    }

    #[test]
    fn spot_fades_out_to_its_taper_radius_and_pans_away() {
        // rad 100, taper 0.5: taperRad = 100 + 1200 * 0.5 = 700.
        let line = "SPOT><drip.ogg><0><0><1><1><1000><500><60><80><0><0.5";
        let mut p = AmbientPlayer::parsed(line);
        assert_eq!(p.sound.taper_rad(), Some(700.0));
        let mic = Microphone::settled_at(Vec2::ZERO);
        let inside = p.mix(&mic, Vec2::new(1050.0, 500.0), 1.0);
        assert!((inside.volume - 0.8f32.powf(1.8)).abs() < 1e-6);
        assert!((inside.pan - (1.0 - 2.0 * 750.0 / 1400.0)).abs() < 1e-6);
        let half = p.mix(&mic, Vec2::new(1400.0, 500.0), 1.0);
        assert!((half.volume - (0.5f32 * 0.8).powf(1.8)).abs() < 1e-6);
        let edge = p.mix(&mic, Vec2::new(1700.0, 500.0), 1.0);
        assert_eq!((edge.volume, edge.pan), (0.0, -1.0));
    }

    #[test]
    fn only_omni_entries_of_the_same_sample_carry_over() {
        let omni = AmbientPlayer::parsed("OMNI><a.ogg><0><0><1><1").sound;
        let other_vol = AmbientPlayer::parsed("OMNI><a.ogg><0><0><0.2><1").sound;
        let dir = AmbientPlayer::parsed("DIR><a.ogg><0><0><1><1><0><0").sound;
        assert!(AmbientPlayer::carries_over(&omni, &other_vol));
        assert!(!AmbientPlayer::carries_over(&omni, &dir));
        assert!(!AmbientPlayer::carries_over(&dir, &dir));
    }
}
