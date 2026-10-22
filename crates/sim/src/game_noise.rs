//! Noises made in a room's update, heard at once by its creatures (`Room.InGameNoise`,
//! `world/hearing.rs`): an object's where the game makes it in its update, and the
//! player's from inside its update through a [`NoiseSink`].
//!
//! Mirrors: Room.cs (InGameNoise, ScreenMovement)

use rw_random::UnityRandom;

use super::RainWorldGame;
use crate::camera::CameraView;
use crate::creature::fly::PlayerBody;
use crate::item::Entity;
use crate::world::World;
use crate::{CameraShake, InGameNoise, NoiseSink, ScreenMovement, Vec2, Water};

/// The player's noises and screen movements while it updates in `room` (whose water
/// it holds).
pub(super) struct PlayerHearing<'a> {
    pub world: &'a mut World,
    pub room: i32,
    pub water: Option<&'a Water>,
    pub player: PlayerBody,
    pub camera: CameraView,
    pub shake: &'a mut CameraShake,
}

impl NoiseSink for PlayerHearing<'_> {
    fn in_game_noise(&mut self, pos: Vec2, strength: f32, interesting: f32, rng: &mut UnityRandom) {
        let noise = InGameNoise {
            pos,
            strength,
            source: Some(Entity::Creature(self.player.id)),
            interesting,
        };
        self.world
            .in_game_noise(self.room, noise, self.water, Some(&self.player), rng);
    }

    fn screen_movement(&mut self, m: ScreenMovement, rng: &mut UnityRandom) {
        self.world
            .screen_movement(self.room, m, &self.camera, self.shake, rng);
    }
}

impl RainWorldGame {
    /// `room.InGameNoise(noise)` in room `index`.
    pub(super) fn room_noise(&mut self, index: i32, noise: InGameNoise) {
        let player = self.player_body();
        self.world
            .in_game_noise(index, noise, None, player.as_ref(), &mut self.rng);
    }

    /// What hears the player's noises while it updates in room `index` (it is there).
    pub(super) fn player_hearing_body(&self, index: i32) -> Option<PlayerBody> {
        self.player_body().map(|b| PlayerBody {
            room: Some(index),
            ..b
        })
    }
}

#[cfg(test)]
#[path = "game_noise_tests.rs"]
mod tests;
