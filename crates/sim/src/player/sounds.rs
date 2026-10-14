//! The player's `room.PlaySound` calls and its slide loop (`Player.Update`'s
//! `slideLoop`; `rainworld-re/notes/sound.md` section 2.7).

use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::{SoundId, Sounds};

/// A `room.PlaySound(id, bodyChunks[chunk], false, volume, pitch)` waiting for its draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct QueuedSound {
    id: SoundId,
    chunk: usize,
    volume: f32,
    pitch: f32,
}

impl Player {
    /// A one-shot on a chunk. Its draws are made at the next [`Player::flush_sounds`],
    /// which runs before any other draw of the tick.
    pub(super) fn play_sound(&mut self, id: SoundId, chunk: usize, volume: f32, pitch: f32) {
        self.sound_queue.push(QueuedSound {
            id,
            chunk,
            volume,
            pitch,
        });
    }

    /// Plays the queued sounds in call order.
    pub(super) fn flush_sounds(&mut self, rng: &mut UnityRandom, sounds: &mut Sounds) {
        for q in std::mem::take(&mut self.sound_queue) {
            sounds.play_on_chunk(q.id, q.chunk, false, q.volume, q.pitch, rng);
        }
    }

    /// The loop the current state calls for, first match wins (no adrenaline, so never
    /// `Mushroom_Trip_LOOP`).
    fn slide_loop_wanted(&self) -> Option<SoundId> {
        Some(if self.is_stunned() {
            SoundId::UiSlugcatStunnedLoop
        } else if self.corridor_drop
            || self.vertical_corridor_slide_counter > 0
            || self.horizontal_corridor_slide_counter > 0
        {
            SoundId::SlugcatSlideInNarrowCorridorLoop
        } else if self.slide_counter > 0 && self.body_mode == BodyMode::Stand {
            SoundId::SlugcatSkidOnGroundLoop
        } else if self.animation == Animation::Roll {
            SoundId::SlugcatRollLoop
        } else if self.animation == Animation::ClimbOnBeam && self.input[0].y < 0 {
            SoundId::SlugcatSlideDownVerticalBeamLoop
        } else if self.animation == Animation::BellySlide {
            SoundId::SlugcatBellySlideLoop
        } else if self.body_mode == BodyMode::WallClimb {
            SoundId::SlugcatWallSlideLoop
        } else {
            return None;
        })
    }

    /// Restarts the loop when the wanted sound changes, then keeps it alive with this
    /// tick's volume and pitch.
    pub(super) fn update_slide_loop(&mut self, rng: &mut UnityRandom, sounds: &mut Sounds) {
        self.flush_sounds(rng, sounds);
        let wanted = self.slide_loop_wanted();
        if wanted != self.slide_loop_sound {
            if let Some(emitter) = self.slide_loop.take() {
                sounds.stop_emitter(emitter);
            }
            self.slide_loop_sound = wanted;
            if let Some(id) = wanted {
                self.slide_loop = Some(sounds.play_on_chunk(id, HEAD, true, 1.0, 1.0, rng));
            }
        }
        let (Some(emitter), Some(id)) = (self.slide_loop, self.slide_loop_sound) else {
            return;
        };
        let (volume, pitch) = self.slide_loop_volume_pitch(id);
        sounds.update_emitter(emitter, volume, pitch);
    }

    fn slide_loop_volume_pitch(&self, id: SoundId) -> (f32, f32) {
        let head = &self.body.chunks[HEAD];
        let (vel, vx, vy) = (head.vel.length(), head.vel.x.abs(), head.vel.y.abs());
        match id {
            SoundId::UiSlugcatStunnedLoop => (
                (self.stun as f32).inverse_lerp(8.0, 27.0).powf(0.7),
                0.5 + (self.stun as f32).inverse_lerp(11.0, self.last_stun as f32),
            ),
            SoundId::SlugcatSlideInNarrowCorridorLoop => {
                let volume = if self.vertical_corridor_slide_counter > 0 {
                    1.0
                } else {
                    (vel / 4.0).min(1.0)
                };
                let full = if self.corridor_drop { 25.0 } else { 12.5 };
                (volume, 0.7.lerp(1.3, vel / full))
            }
            SoundId::SlugcatBellySlideLoop => ((vx / 10.0).min(1.0), 0.5.lerp(1.5, vx / 25.5)),
            SoundId::SlugcatRollLoop => {
                let dir = head.pos.direction_to(self.body.chunks[HIPS].pos);
                (0.5 + dir.x.abs() * 0.5, 0.85.lerp(1.15, 0.5 + dir.y * 0.5))
            }
            SoundId::SlugcatSkidOnGroundLoop => ((vx / 6.0).min(1.0), 0.7.lerp(1.3, vx / 9.5)),
            SoundId::SlugcatSlideDownVerticalBeamLoop => {
                ((vy / 2.5).min(1.0), 0.7.lerp(1.3, vy / 4.9))
            }
            SoundId::SlugcatWallSlideLoop => {
                let dy = (head.pos.y - head.last_pos.y).abs();
                ((vy * 1.5).min(1.0), 0.7.lerp(1.3, dy / 1.75))
            }
            _ => (1.0, 1.0),
        }
    }
}
