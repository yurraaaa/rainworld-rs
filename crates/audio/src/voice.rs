//! Sound objects (`DisembodiedSound`, `StaticPositionSound`, `ObjectSound`,
//! `DisembodiedLoop`, `RectangularSound`) and the emitters they follow.
//!
//! Mirrors: VirtualMicrophone.cs (DisembodiedSound, StaticPositionSound, ObjectSound, DisembodiedLoop, RectangularSound), SoundLoader.cs

use rustc_hash::FxHashMap;
use rw_sim::{BodyChunk, EmitterId, FloatExt, PlayedSound, SoundSource, Vec2, Water};

use crate::mic::{Doppler, Microphone, SourceMix};

/// A `ChunkSoundEmitter`: copies its chunk's position every tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emitter {
    /// The player's chunk it follows; `None` for another body's, moved by the sim.
    pub chunk: Option<usize>,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
    /// A `DisembodiedLoopEmitter`'s `pan` once the sim set one.
    pub pan: Option<f32>,
    /// Ticks ended since a loop's owner last set it (`requireActiveUpkeep`).
    pub unkept: u8,
}

/// The chunk emitters by handle.
#[derive(Debug, Clone, Default)]
pub struct Emitters(FxHashMap<EmitterId, Emitter>);

impl Emitters {
    /// Makes the emitter of a chunk sound on its first play (`chunks`: the player's
    /// body, if in the room).
    pub fn insert_for(&mut self, sound: &PlayedSound, chunks: Option<&[BodyChunk]>) {
        let (emitter, chunk, pos) = match sound.source {
            SoundSource::Chunk { emitter, chunk } => {
                let Some(c) = chunks.and_then(|c| c.get(chunk)) else {
                    return;
                };
                (emitter, Some(chunk), c.pos)
            }
            SoundSource::Object { emitter, pos } => (emitter, None, pos),
            SoundSource::DisembodiedLoop { emitter, .. } | SoundSource::Rect { emitter, .. } => {
                (emitter, None, Vec2::ZERO)
            }
            _ => return,
        };
        self.0.entry(emitter).or_insert(Emitter {
            chunk,
            pos,
            last_pos: pos,
            volume: sound.emitter_volume,
            pitch: sound.emitter_pitch,
            looped: sound.looped,
            pan: None,
            unkept: 0,
        });
    }

    #[must_use]
    pub fn get(&self, id: EmitterId) -> Option<&Emitter> {
        self.0.get(&id)
    }

    pub fn set(&mut self, id: EmitterId, volume: f32, pitch: f32) {
        if let Some(e) = self.0.get_mut(&id) {
            e.volume = volume;
            e.pitch = pitch;
            e.unkept = 0;
        }
    }

    pub fn set_pan(&mut self, id: EmitterId, pan: f32) {
        if let Some(e) = self.0.get_mut(&id) {
            e.pan = Some(pan);
        }
    }

    pub fn remove(&mut self, id: EmitterId) {
        self.0.remove(&id);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// The player's body left: the emitters on its chunks die.
    pub fn clear_chunks(&mut self) {
        self.0.retain(|_, e| e.chunk.is_none());
    }

    /// The emitters of bodies other than the player's, and disembodied loops', die.
    pub fn clear_objects(&mut self) {
        self.0.retain(|_, e| e.chunk.is_some());
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `ChunkSoundEmitter.Update`: follow the chunks.
    pub fn follow(&mut self, chunks: &[BodyChunk]) {
        for e in self.0.values_mut() {
            let Some(chunk) = e.chunk else { continue };
            e.last_pos = e.pos;
            if let Some(c) = chunks.get(chunk) {
                e.pos = c.pos;
            }
        }
    }

    /// An object emitter's update moved it to `pos`.
    pub fn moved(&mut self, id: EmitterId, pos: Vec2) {
        if let Some(e) = self.0.get_mut(&id) {
            e.last_pos = e.pos;
            e.pos = pos;
        }
    }

    /// One-shot emitters live while a sound plays on them.
    pub fn prune(&mut self, used: &[EmitterId]) {
        self.0.retain(|id, e| e.looped || used.contains(id));
    }

    /// A loop the sim let go without a `Stop` dies at the end of its second unkept tick,
    /// where the game's emitter that updates before its owner dies (the sim's `Stop`
    /// ends every loop it tracks on its own tick).
    pub fn end_tick(&mut self) {
        for e in self.0.values_mut() {
            e.unkept = e.unkept.saturating_add(1);
        }
        self.0.retain(|_, e| !e.looped || e.unkept <= 2);
    }
}

/// One sound object.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub sound: PlayedSound,
    /// A disembodied sound's volume, set once when it is made.
    fixed_volume: f32,
    doppler: Doppler,
}

impl Voice {
    #[must_use]
    pub fn new(sound: PlayedSound, mic: &Microphone, now: f32) -> Self {
        let group = match sound.source {
            SoundSource::Disembodied { group, .. } => group,
            _ => 0,
        };
        let fixed_volume = mic.source_volume(
            sound.emitter_volume,
            sound.volume,
            mic.volume_groups()[group],
        );
        Self {
            sound,
            fixed_volume,
            doppler: Doppler::new(now),
        }
    }

    /// The chunk emitter this sound follows.
    #[must_use]
    pub fn emitter(&self) -> Option<EmitterId> {
        match self.sound.source {
            SoundSource::Chunk { emitter, .. }
            | SoundSource::Object { emitter, .. }
            | SoundSource::DisembodiedLoop { emitter, .. }
            | SoundSource::Rect { emitter, .. } => Some(emitter),
            _ => None,
        }
    }

    /// `SoundObject.Update` / `PositionedSound.Update`; `None` when the emitter it
    /// follows is gone (the sound ends).
    pub fn mix(
        &mut self,
        mic: &Microphone,
        emitters: &Emitters,
        time_stacker: f32,
        now: f32,
        water: Option<&Water>,
    ) -> Option<SourceMix> {
        let s = &self.sound;
        let (pos, last_pos, volume, pitch) = match s.source {
            SoundSource::Disembodied { pan, .. } => {
                return Some(SourceMix {
                    volume: self.fixed_volume,
                    pan,
                    pitch: s.emitter_pitch * s.pitch,
                    cutoff: Microphone::low_pass_cutoff(mic.under_water, s.ignore_effects),
                });
            }
            SoundSource::DisembodiedLoop { emitter, pan } => {
                // `DisembodiedLoop.Update` reads the emitter's volume and pitch each frame.
                let e = emitters.get(emitter)?;
                return Some(SourceMix {
                    volume: mic.source_volume(e.volume, s.volume, mic.volume_groups()[0]),
                    pan: e.pan.unwrap_or(pan),
                    pitch: e.pitch * s.pitch,
                    cutoff: Microphone::low_pass_cutoff(mic.under_water, s.ignore_effects),
                });
            }
            SoundSource::Position(pos) => (pos, pos, s.emitter_volume, s.emitter_pitch),
            SoundSource::Chunk { emitter, .. } | SoundSource::Object { emitter, .. } => {
                let e = emitters.get(emitter)?;
                (e.pos, e.last_pos, e.volume, e.pitch)
            }
            // `RectangularSound`: the rect's points nearest the listener's last two
            // ticks, with the emitter's volume and pitch.
            SoundSource::Rect { emitter, rect } => {
                let e = emitters.get(emitter)?;
                let pos = rect.restrict(mic.listener());
                let last_pos = rect.restrict(mic.last_listener());
                (pos, last_pos, e.volume, e.pitch)
            }
        };
        let ps = last_pos.lerp(pos, time_stacker);
        let factor = mic.vol_from_point(ps, time_stacker, s.range) * volume;
        let doppler = if s.doppler_fac > 0.0 {
            let dist = pos.distance(mic.listener());
            self.doppler
                .update(now, dist, s.doppler_fac, mic.doppler_block)
        } else {
            1.0
        };
        Some(SourceMix {
            volume: mic.source_volume(factor, s.volume, mic.volume_groups()[0]),
            pan: mic.pan_from_point(ps, time_stacker),
            pitch: pitch * doppler * mic.time_speed.lerp(1.0, s.ignore_effects) * s.pitch,
            cutoff: Microphone::low_pass_cutoff(mic.positioned_muffle(ps, water), s.ignore_effects),
        })
    }
}

#[cfg(test)]
mod tests {
    use rw_sim::{SoundId, SoundSource};

    use super::*;

    impl Emitters {
        /// Plays a sound on chunk 1 through emitter `emitter`.
        fn play_on_chunk(&mut self, emitter: u32, looped: bool, chunks: &[BodyChunk]) {
            let sound = PlayedSound {
                id: SoundId::SlugcatSkidOnGroundLoop,
                clip: 0,
                variation: 0,
                volume: 1.0,
                pitch: 1.0,
                range: 1.0,
                doppler_fac: 1.0,
                ignore_effects: 0.0,
                start_at: None,
                source: SoundSource::Chunk {
                    emitter: EmitterId(emitter),
                    chunk: 1,
                },
                looped,
                emitter_volume: 0.5,
                emitter_pitch: 1.0,
            };
            self.insert_for(&sound, Some(chunks));
        }
    }

    /// A loop lives through one unkept tick and dies at the end of the second.
    #[test]
    fn loops_die_after_two_unkept_ticks() {
        let chunk = BodyChunk::new(Vec2::new(10.0, 20.0), 5.0, 0.5);
        let mut chunks = [chunk.clone(), chunk];
        let mut emitters = Emitters::default();
        emitters.play_on_chunk(0, true, &chunks);
        emitters.play_on_chunk(1, false, &chunks);
        emitters.end_tick();
        chunks[1].pos = Vec2::new(15.0, 20.0);
        emitters.follow(&chunks);
        emitters.set(EmitterId(0), 0.25, 1.5);
        emitters.end_tick();
        let e = *emitters.get(EmitterId(0)).expect("refreshed loop lives");
        assert_eq!((e.volume, e.pitch), (0.25, 1.5));
        assert_eq!(
            (e.last_pos, e.pos),
            (Vec2::new(10.0, 20.0), Vec2::new(15.0, 20.0))
        );
        emitters.end_tick();
        assert!(emitters.get(EmitterId(0)).is_some(), "one unkept tick");
        emitters.end_tick();
        assert!(emitters.get(EmitterId(0)).is_none());
        assert!(emitters.get(EmitterId(1)).is_some());
        emitters.prune(&[]);
        assert!(emitters.get(EmitterId(1)).is_none());
    }

    /// A `DisembodiedLoop` mixes at its pan with the emitter's current volume and pitch,
    /// and ends with its emitter.
    #[test]
    fn a_disembodied_loop_follows_its_emitter() {
        let sound = PlayedSound {
            id: SoundId::CycleStartDrips,
            clip: 0,
            variation: 0,
            volume: 0.5,
            pitch: 1.0,
            range: 1.0,
            doppler_fac: 0.0,
            ignore_effects: 0.0,
            start_at: None,
            source: SoundSource::DisembodiedLoop {
                emitter: EmitterId(3),
                pan: 0.0,
            },
            looped: true,
            emitter_volume: 1.0,
            emitter_pitch: 1.0,
        };
        let mut emitters = Emitters::default();
        emitters.insert_for(&sound, None);
        let mic = Microphone::new(1.8);
        let mut voice = Voice::new(sound, &mic, 0.0);
        assert_eq!(voice.emitter(), Some(EmitterId(3)));
        emitters.set(EmitterId(3), 0.25, 1.2);
        let mix = voice.mix(&mic, &emitters, 1.0, 0.0, None).expect("plays");
        assert_eq!(mix.pan, 0.0);
        assert!((mix.pitch - 1.2).abs() < 1e-6);
        let want = mic.source_volume(0.25, 0.5, mic.volume_groups()[0]);
        assert!((mix.volume - want).abs() < 1e-6);
        // `DisembodiedLoop.Update` reads the emitter's pan too (`DisembodiedDynamicSoundLoop.Pan`).
        emitters.set_pan(EmitterId(3), -0.4);
        let mix = voice.mix(&mic, &emitters, 1.0, 0.0, None).expect("plays");
        assert_eq!(mix.pan, -0.4);
        emitters.remove(EmitterId(3));
        assert!(voice.mix(&mic, &emitters, 1.0, 0.0, None).is_none());
    }

    /// A `RectangularSound` is heard from the point of its rect nearest the listener,
    /// with its emitter's volume and pitch; it ends with the emitter.
    #[test]
    fn a_rect_sound_is_heard_from_its_nearest_point() {
        let rect = rw_sim::SoundRect {
            left: 0.0,
            bottom: 40.0,
            right: 600.0,
            top: 60.0,
        };
        let sound = PlayedSound {
            id: SoundId::WaterSurfaceCalmLoop,
            clip: 0,
            variation: 0,
            volume: 0.6,
            pitch: 1.0,
            range: 0.8,
            doppler_fac: 0.0,
            ignore_effects: 0.0,
            start_at: None,
            source: SoundSource::Rect {
                emitter: EmitterId(2),
                rect,
            },
            looped: true,
            emitter_volume: 1.0,
            emitter_pitch: 1.0,
        };
        let mut emitters = Emitters::default();
        emitters.insert_for(&sound, None);
        let listener = Vec2::new(900.0, 300.0);
        let mic = Microphone::following(listener);
        let mut voice = Voice::new(sound, &mic, 0.0);
        assert_eq!(voice.emitter(), Some(EmitterId(2)));
        emitters.set(EmitterId(2), 0.5, 1.1);
        let mix = voice.mix(&mic, &emitters, 1.0, 0.0, None).expect("plays");
        let ps = Vec2::new(600.0, 60.0);
        assert!((mix.pan - mic.pan_from_point(ps, 1.0)).abs() < 1e-6);
        let factor = mic.vol_from_point(ps, 1.0, 0.8) * 0.5;
        let want = mic.source_volume(factor, 0.6, mic.volume_groups()[0]);
        assert!((mix.volume - want).abs() < 1e-6);
        assert!((mix.pitch - 1.1).abs() < 1e-6);
        emitters.remove(EmitterId(2));
        assert!(voice.mix(&mic, &emitters, 1.0, 0.0, None).is_none());
    }
}
