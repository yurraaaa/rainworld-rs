//! Sound objects (`DisembodiedSound`, `StaticPositionSound`, `ObjectSound`) and the
//! chunk emitters they follow.

use std::collections::HashMap;

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
    /// Ticks ended since a loop's owner last set it (`requireActiveUpkeep`).
    pub unkept: u8,
}

/// The chunk emitters by handle.
#[derive(Debug, Clone, Default)]
pub struct Emitters(HashMap<EmitterId, Emitter>);

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
            _ => return,
        };
        self.0.entry(emitter).or_insert(Emitter {
            chunk,
            pos,
            last_pos: pos,
            volume: sound.emitter_volume,
            pitch: sound.emitter_pitch,
            looped: sound.looped,
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

    /// The emitters of bodies other than the player's die.
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
            SoundSource::Chunk { emitter, .. } | SoundSource::Object { emitter, .. } => {
                Some(emitter)
            }
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
            SoundSource::Position(pos) => (pos, pos, s.emitter_volume, s.emitter_pitch),
            SoundSource::Chunk { emitter, .. } | SoundSource::Object { emitter, .. } => {
                let e = emitters.get(emitter)?;
                (e.pos, e.last_pos, e.volume, e.pitch)
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
}
