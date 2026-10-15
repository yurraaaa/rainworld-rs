//! What the pause menu reads of `RainWorldGame` and `ExitGame`, whose save writes are
//! stubs until saves exist (docs/DIVERGENCES.md).

use super::RainWorldGame;

impl RainWorldGame {
    /// `IsStorySession`.
    #[must_use]
    pub fn is_story_session(&self) -> bool {
        self.world.story
    }

    /// `clock`: the ticks the game ran unpaused.
    #[must_use]
    pub fn clock(&self) -> u64 {
        self.tick
    }

    /// `cameras[0].roomSafeForPause`: the camera's room is fully loaded (or it has none).
    #[must_use]
    pub fn room_safe_for_pause(&self) -> bool {
        self.world
            .realized(self.camera_room)
            .is_none_or(crate::world::RealizedRoom::fully_loaded)
    }

    /// `ExitGame(asDeath, asQuit)`. Safari and Artificer dreams are More Slugcats.
    pub fn exit_game(&mut self, as_death: bool, as_quit: bool) {
        log::info!("Exit game");
        if self.is_story_session() {
            if as_quit {
                self.append_time_on_cycle_end(true);
            }
            let clock = self.clock();
            self.save_progression_and_death_persistent_data_of_current_state(
                as_death && clock > 1200,
                as_quit && clock > 200,
            );
        }
    }

    /// `StoryGameSession.AppendTimeOnCycleEnd`: not ported (the save's play time).
    fn append_time_on_cycle_end(&mut self, _death_or_ghost: bool) {}

    /// `PlayerProgression.SaveProgressionAndDeathPersistentDataOfCurrentState`: a no-op
    /// until saves are ported.
    pub fn save_progression_and_death_persistent_data_of_current_state(
        &mut self,
        save_as_if_player_died: bool,
        save_as_if_player_quit: bool,
    ) {
        log::info!(
            "save stub: died {save_as_if_player_died}, quit {save_as_if_player_quit} (not written)"
        );
    }
}
