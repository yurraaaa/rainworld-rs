//! The player's `room.PlaySound` calls and its slide loop (`Player.Update`'s
//! `slideLoop`; `rainworld-re/notes/sound.md` section 2.7).
//!
//! Mirrors: Player.cs (Update)

use rw_random::UnityRandom;

use super::drips::DripBurst;
use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::math::float_ext::FloatExt;
use crate::{EmitterPlace, LoopEmitter, NoiseSink, SoundId, Sounds, Vec2};

/// A `room.PlaySound(id, bodyChunks[chunk], false, volume, pitch)`, or a site's
/// `WaterDrip`s, waiting for its draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum QueuedSound {
    Sound {
        id: SoundId,
        chunk: usize,
        volume: f32,
        pitch: f32,
    },
    Drips(DripBurst),
}

/// A `room.InGameNoise(new InGameNoise(pos, strength, this, interesting))` waiting for
/// the update's last flush, after the first `after_sounds` queued sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct QueuedNoise {
    after_sounds: usize,
    pos: Vec2,
    strength: f32,
    interesting: f32,
}

impl Player {
    /// A noise made where nobody can hear it yet; it is heard at
    /// [`Player::flush_sounds_and_noises`], after the sounds queued before it.
    pub(super) fn make_noise(&mut self, pos: Vec2, strength: f32, interesting: f32) {
        self.noise_queue.push(QueuedNoise {
            after_sounds: self.sound_queue.len(),
            pos,
            strength,
            interesting,
        });
    }

    /// Plays the queued sounds and makes the queued noises in call order.
    pub(super) fn flush_sounds_and_noises(
        &mut self,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        noises: &mut dyn NoiseSink,
    ) {
        let queued = std::mem::take(&mut self.noise_queue);
        let mut pending = queued.iter().peekable();
        for (k, q) in std::mem::take(&mut self.sound_queue)
            .into_iter()
            .enumerate()
        {
            while let Some(n) = pending.next_if(|n| n.after_sounds <= k) {
                noises.in_game_noise(n.pos, n.strength, n.interesting, rng);
            }
            self.play_queued(q, rng, sounds);
        }
        for n in pending {
            noises.in_game_noise(n.pos, n.strength, n.interesting, rng);
        }
    }

    /// A one-shot on a chunk. Its draws are made at the next [`Player::flush_sounds`],
    /// which runs before any other draw of the tick.
    pub(super) fn play_sound(&mut self, id: SoundId, chunk: usize, volume: f32, pitch: f32) {
        self.sound_queue.push(QueuedSound::Sound {
            id,
            chunk,
            volume,
            pitch,
        });
    }

    /// Queues a drip site's drips; their draws are made in turn with the sounds'.
    pub(super) fn shed_drips(&mut self, burst: DripBurst) {
        self.sound_queue.push(QueuedSound::Drips(burst));
    }

    fn play_queued(&mut self, q: QueuedSound, rng: &mut UnityRandom, sounds: &mut Sounds) {
        match q {
            QueuedSound::Sound {
                id,
                chunk,
                volume,
                pitch,
            } => {
                sounds.play_on_chunk(id, chunk, false, volume, pitch, rng);
            }
            QueuedSound::Drips(burst) => burst.make(self.ceiling_drips, rng, &mut self.drips),
        }
    }

    /// Plays the queued sounds and makes the queued drips in call order.
    pub(crate) fn flush_sounds(&mut self, rng: &mut UnityRandom, sounds: &mut Sounds) {
        for q in std::mem::take(&mut self.sound_queue) {
            self.play_queued(q, rng, sounds);
        }
        for n in &mut self.noise_queue {
            n.after_sounds = 0;
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

    /// Lets the loop go and starts the new one when the wanted sound changes, then keeps
    /// it alive with this tick's volume and pitch. A loop its own update destroyed stays
    /// held (and silent) until the wanted sound changes, as in the game.
    pub(super) fn update_slide_loop(&mut self, rng: &mut UnityRandom, sounds: &mut Sounds) {
        self.flush_sounds(rng, sounds);
        let wanted = self.slide_loop_wanted();
        if wanted != self.slide_loop_sound {
            if let Some(mut e) = self.slide_loop.take() {
                e.alive = false;
                if !e.slated {
                    self.released_slide_loops.push(e);
                }
            }
            self.slide_loop_sound = wanted;
            if let Some(id) = wanted {
                let before = sounds.events().len();
                let sound = sounds.play_on_chunk(id, HEAD, true, 1.0, 1.0, rng);
                let played = sounds.events().len() > before;
                self.slide_loop = Some(LoopEmitter::join(sound, played, &mut self.emitter_place));
            }
        }
        let (Some(e), Some(id)) = (&mut self.slide_loop, self.slide_loop_sound) else {
            return;
        };
        e.alive = true;
        if e.slated {
            return;
        }
        let sound = e.sound;
        let (volume, pitch) = self.slide_loop_volume_pitch(id);
        sounds.update_emitter(sound, volume, pitch);
    }

    /// Set before the update: where a slide loop started in it joins.
    pub(crate) fn set_emitter_place(&mut self, place: EmitterPlace) {
        self.emitter_place = place;
    }

    #[must_use]
    pub(crate) fn emitter_place(&self) -> EmitterPlace {
        self.emitter_place
    }

    #[cfg(test)]
    pub(crate) fn set_horizontal_corridor_slide(&mut self, ticks: i32) {
        self.horizontal_corridor_slide_counter = ticks;
    }

    /// The slide loop's emitters, held or let go, still in a room's update list.
    pub fn slide_loop_emitters(&self) -> impl Iterator<Item = &LoopEmitter> {
        self.slide_loop
            .iter()
            .chain(&self.released_slide_loops)
            .filter(|e| !e.slated)
    }

    /// The slide loop emitters let go and not yet destroyed.
    #[must_use]
    pub fn released_slide_loop_count(&self) -> usize {
        self.released_slide_loops
            .iter()
            .filter(|e| !e.slated)
            .count()
    }

    /// The slide loop emitter that joined the update list at `order`.
    pub(crate) fn slide_loop_emitter_mut(&mut self, order: u64) -> Option<&mut LoopEmitter> {
        self.slide_loop
            .iter_mut()
            .chain(self.released_slide_loops.iter_mut())
            .find(|e| e.update_order == order)
    }

    /// Forgets the let-go emitters that were destroyed or whose room `in_room` no longer
    /// holds.
    pub(crate) fn retain_released_slide_loops(&mut self, in_room: impl Fn(i32) -> bool) {
        self.released_slide_loops
            .retain(|e| !e.slated && in_room(e.room));
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
