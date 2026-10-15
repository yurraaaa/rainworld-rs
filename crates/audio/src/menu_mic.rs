//! Playback for the menus' `MenuMicrophone`: unpositioned sources at a fixed pan, with
//! `MenuSoundObject`'s volume (`Clamp01(Pow(value * soundData.vol * sfxVolume,
//! volumeExponent))`) and pitch (`value * soundData.pitch`). The menus decide what
//! plays and make the random draws (`rw-menu`'s `MenuMic`); this only sounds it.

use crate::clips::{ClipKey, ClipSlot};
use crate::mic::SourceMix;
use crate::{Audio, Source};

/// A menu sound object to start, as the menus drew it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuSoundStart {
    /// Index into the sound table's clips, and the variation.
    pub clip: usize,
    pub variation: usize,
    /// `soundData.vol` and `soundData.pitch`.
    pub sound_volume: f32,
    pub sound_pitch: f32,
    /// `audioSource.panStereo`.
    pub pan: f32,
    /// The first `SetVolume` and `SetPitch` values.
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
}

struct MenuVoice {
    /// The loop's handle, for a loop.
    handle: Option<u32>,
    start: MenuSoundStart,
    clip: ClipKey,
    source: Option<Source>,
}

/// The menu microphone's playing sources.
#[derive(Default)]
pub struct MenuPlayback {
    voices: Vec<MenuVoice>,
}

impl MenuVoice {
    fn mix(&self, exponent: f32, sfx_volume: f32) -> SourceMix {
        let s = &self.start;
        SourceMix {
            volume: (s.volume * s.sound_volume * sfx_volume)
                .powf(exponent)
                .clamp(0.0, 1.0),
            pan: s.pan,
            pitch: s.pitch * s.sound_pitch,
            cutoff: None,
        }
    }
}

impl MenuPlayback {
    /// Starts a sound object; a loop is then driven by `handle`.
    pub fn play(&mut self, audio: &Audio, start: MenuSoundStart, handle: Option<u32>) {
        let Some(variation) = audio
            .table
            .clip_at(start.clip)
            .and_then(|c| c.variations.get(start.variation))
        else {
            return;
        };
        self.voices.push(MenuVoice {
            handle,
            start,
            clip: ClipKey::Effect(variation.asset.clone()),
            source: None,
        });
    }

    /// A loop's `SetVolume` and `SetPitch` values this frame.
    pub fn set_loop(&mut self, handle: u32, volume: f32, pitch: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.handle == Some(handle)) {
            v.start.volume = volume;
            v.start.pitch = pitch;
        }
    }

    /// `MenuSoundObject.Destroy` of a loop.
    pub fn stop(&mut self, handle: u32) {
        if let Some(i) = self.voices.iter().position(|v| v.handle == Some(handle))
            && let Some(source) = self.voices.swap_remove(i).source
        {
            source.stop();
        }
    }

    /// One drawn frame: starts sources whose clips are ready, applies the mixes and
    /// drops finished one-shots.
    pub fn frame(&mut self, audio: &mut Audio) {
        audio.clips.poll();
        let exponent = audio.table.volume_exponent;
        let sfx_volume = audio.game.mic.sfx_volume;
        let mut i = 0;
        while i < self.voices.len() {
            let v = &mut self.voices[i];
            let mix = v.mix(exponent, sfx_volume);
            let ended = v.source.as_ref().is_some_and(Source::finished)
                || matches!(audio.clips.get(&v.clip), ClipSlot::Missing);
            if ended {
                if let Some(source) = self.voices.swap_remove(i).source {
                    source.stop();
                }
                continue;
            }
            match &mut v.source {
                Some(source) => source.apply(mix),
                None => {
                    if let ClipSlot::Ready(data) = audio.clips.get(&v.clip) {
                        v.source = Source::start(
                            &mut audio.manager,
                            (**data).clone(),
                            v.start.looped,
                            None,
                            mix,
                        );
                    }
                }
            }
            i += 1;
        }
    }
}
