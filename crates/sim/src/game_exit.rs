//! What the pause menu and `ExitGame` read of `RainWorldGame`; the exit's save is the
//! app's (`AppContext::exit_game`).

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
}
