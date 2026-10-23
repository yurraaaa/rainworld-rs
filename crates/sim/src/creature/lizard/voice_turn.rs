//! The game's side of a lizard's `LizardVoice`: its update in `Lizard.Update`, the calls
//! that make a sound, and its emitter's place in the room's update list.
//!
//! Mirrors: LizardVoice.cs, Lizard.cs (Update)

use std::sync::Arc;

use super::{RainWorldGame, RoomObject};
use crate::creature::CreatureId;
use crate::creature::lizard::{LizardEmotion, LizardVoice, VoiceCx};

impl RainWorldGame {
    /// Lizard `id`'s voice, built from the game's sound table the first time.
    fn lizard_voice_mut(&mut self, id: CreatureId) -> &mut LizardVoice {
        let (kind, seed) = {
            let c = self.world.creature(id);
            (c.kind(), c.id.random_seed())
        };
        let table = Arc::clone(self.sounds.table());
        let l = self.world.lizard_at(id);
        let pending = std::mem::take(&mut l.pending_voice);
        let v = l
            .voice
            .get_or_insert_with(|| LizardVoice::new(kind, seed, &table));
        for s in pending {
            v.make_sound_with(s.emotion, s.intensity, s.conscious, s.holding);
        }
        v
    }

    /// `voice.MakeSound(emotion)`: the intensity's draw, then the sound.
    pub(super) fn lizard_voice(&mut self, id: CreatureId, emotion: LizardEmotion) {
        let holding = self.lizard_holding(id);
        let conscious = self.world.lizard_ref(id).conscious();
        self.lizard_voice_mut(id);
        if let Some(v) = &mut self.world.lizard_at(id).voice {
            v.make_sound(emotion, conscious, holding, &mut self.rng);
        }
    }

    /// `voice.MakeSound(emotion, intensity)`.
    pub(super) fn lizard_voice_with(
        &mut self,
        id: CreatureId,
        emotion: LizardEmotion,
        intensity: f32,
    ) {
        let holding = self.lizard_holding(id);
        let conscious = self.world.lizard_ref(id).conscious();
        self.lizard_voice_mut(id)
            .make_sound_with(emotion, intensity, conscious, holding);
    }

    /// `voice.Update()` of lizard `id` in room `index`.
    pub(super) fn lizard_voice_update(&mut self, index: i32, id: CreatureId) {
        let holding = self.lizard_holding(id);
        self.lizard_voice_mut(id);
        let table = Arc::clone(self.sounds.table());
        let mut seq = self.world.update_seq();
        let l = self.world.lizard_at(id);
        let pos = l.body.chunks[0].pos;
        let jaw_forced_shut = l.jaw_forced_shut;
        let Some(mut voice) = l.voice.take() else {
            return;
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = VoiceCx {
            room_index: index,
            camera_room: self.camera_room,
            tick: self.tick,
            sounds,
            rng: &mut self.rng,
            table: &table,
            update_seq: &mut seq,
            pos,
            holding,
            jaw_forced_shut,
        };
        voice.update(&mut cx);
        self.world.set_update_seq(seq);
        self.world.lizard_at(id).voice = Some(voice);
    }

    /// The lizards' voice emitters, held or released, in room `index`'s update list.
    pub(super) fn room_lizard_voice_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .creatures()
            .filter_map(|(id, c)| Some((id, c.lizard()?.voice.as_ref()?)))
            .flat_map(|(owner, v)| {
                v.emitters().filter(|e| e.room == index).map(move |e| {
                    let order = e.update_order;
                    (order, RoomObject::LizardVoice { owner, order })
                })
            })
            .collect()
    }

    /// The `ChunkSoundEmitter.Update` of lizard `owner`'s voice emitter that joined room
    /// `index` at `order`: it follows the head, and dies when its owner left the room or
    /// nothing kept it alive since its last update; its sound is cut then.
    pub(super) fn run_lizard_voice_emitter(&mut self, index: i32, owner: CreatureId, order: u64) {
        let Some(l) = self.world.creature_mut(owner).lizard_mut() else {
            return;
        };
        let (owner_room, pos) = (l.room, l.body.chunks[0].pos);
        let Some(voice) = l.voice.as_mut() else {
            return;
        };
        let Some(e) = voice
            .emitter_mut(order)
            .filter(|e| e.room == index && !e.slated)
        else {
            return;
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.move_emitter(e.sound, pos);
        e.update(owner_room, self.camera_room, self.tick);
        if e.slated {
            sounds.stop_emitter(e.sound);
            voice.released.retain(|e| !e.slated);
        }
    }
}
