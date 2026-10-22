//! `RainWorldGame.GameOver` and the karma flower's spot: `Player.karmaFlowerGrowPos`
//! and `StoryGameSession.PlaceKarmaFlowerOnDeathSpot`.
//!
//! Mirrors: RainWorldGame.cs (GameOver), Player.cs (karmaFlowerGrowPos), StoryGameSession.cs (PlaceKarmaFlowerOnDeathSpot)

use rw_assets::{Room, SlugcatName};

use super::RainWorldGame;
use crate::player::HIPS;
use crate::{Vec2, WorldCoordinate};

/// What `GameOver` hands camera 0's HUD (`InitGameOverMode`), the save state and the
/// music (`DeathEvent`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameOver {
    /// `FoodInStomach`.
    pub food_in_stomach: i32,
    /// `Players[0].pos.room`.
    pub death_room: i32,
    /// The main chunk at the death, restricted to the room grown by 50 px.
    pub death_pos: Vec2,
    /// `dependentOnGrasp`: the serial of the danger grasp a held player's game over
    /// waits on (`None` for a death).
    pub dependent_on_grasp: Option<u64>,
    /// `PlaceKarmaFlowerOnDeathSpot`: `Some` when the player places a flower
    /// (`PlaceKarmaFlower`), holding the `karmaFlowerGrowPos` the save's
    /// `karmaFlowerPosition` becomes (`None` if the player never stood on a spot).
    pub karma_flower: Option<Option<WorldCoordinate>>,
}

impl RainWorldGame {
    /// `RedIsOutOfCycles`: the Hunter at or past `RedsIllness.RedsCycles` (19, 24 with
    /// the extra cycles).
    #[must_use]
    pub fn red_is_out_of_cycles(&self) -> bool {
        let cycles = if self.story.red_extra_cycles { 24 } else { 19 };
        self.campaign.name == SlugcatName::Red && self.story.cycle_number >= cycles
    }

    /// `Player.PlaceKarmaFlower`: reinforced karma, the Monk, or the Hunter out of cycles.
    #[must_use]
    pub fn place_karma_flower(&self) -> bool {
        self.story.karma_reinforced
            || self.campaign.name == SlugcatName::Yellow
            || self.red_is_out_of_cycles()
    }

    /// The player's `karmaFlowerGrowPos`.
    #[must_use]
    pub fn karma_flower_grow_pos(&self) -> Option<WorldCoordinate> {
        self.karma_flower_grow_pos
    }

    /// The `GameOver` call the player's death made, once.
    pub fn take_game_over(&mut self) -> Option<GameOver> {
        self.game_over.take()
    }

    /// Whether `GameOver` went to `GoToRedsGameOver` since the last call.
    pub fn take_reds_game_over(&mut self) -> bool {
        std::mem::take(&mut self.reds_game_over)
    }

    /// `Player.Update`'s `karmaFlowerGrowPos` (86975): a living, ungrabbed player that
    /// places flowers marks the hips' tile when they stand on solid ground with two free
    /// tiles above, outside deep water, worm grass (none in the port), narrow spaces and
    /// shelters. The body has not moved yet this tick.
    pub(super) fn update_karma_flower_grow_pos(&mut self, index: i32, room: &Room) {
        if !self.place_karma_flower() {
            return;
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let hips = p.body.chunks[HIPS].pos;
        let (x, y) = hips.tile();
        let narrow = self
            .world
            .realized(index)
            .and_then(|r| r.ai_map.as_ref())
            .is_some_and(|m| m.narrow_space(x, y));
        let shelter = self.world.abstract_room(index).is_some_and(|r| r.shelter());
        let held = !self.player_grabbed_by().is_empty();
        if !p.is_dead()
            && !held
            && p.is_tile_solid(room, HIPS, 0, -1)
            && room.water_int(x, y) != 1
            && !p.is_tile_solid(room, HIPS, 0, 0)
            && !p.is_tile_solid(room, HIPS, 0, 1)
            && !narrow
            && !shelter
        {
            self.karma_flower_grow_pos = Some(WorldCoordinate::new(index, x, y, -1));
        }
    }

    /// `Player.Die`'s `GameOver` call after the player's first death, with the main
    /// chunk where it died. Later `Die` calls place the flower again, from the same
    /// `karmaFlowerGrowPos` (it no longer moves), which changes nothing.
    pub(super) fn game_over_on_death(&mut self, room: &Room) {
        let Some(main) = self.player.as_mut().and_then(crate::Player::take_died_at) else {
            return;
        };
        self.game_over(main, room, None);
    }

    /// `RainWorldGame.GameOver(dependentOnGrasp)` for the player in `room`: story
    /// sessions only; the Hunter out of cycles goes to `GoToRedsGameOver` (only its
    /// music is ported, D17).
    pub(super) fn game_over(&mut self, main: Vec2, room: &Room, dependent_on_grasp: Option<u64>) {
        log::info!("Trying to game over...");
        if !self.is_story_session() {
            return;
        }
        if self.red_is_out_of_cycles() {
            self.reds_game_over = true;
            return;
        }
        let death_pos = Vec2::new(
            main.x.clamp(-50.0, room.pixel_width() + 50.0),
            main.y.clamp(-50.0, room.pixel_height() + 50.0),
        );
        self.game_over = Some(GameOver {
            food_in_stomach: self.food_in_stomach().cast_signed(),
            death_room: self.pos().room,
            death_pos,
            dependent_on_grasp,
            karma_flower: self
                .place_karma_flower()
                .then_some(self.karma_flower_grow_pos),
        });
    }
}

#[cfg(test)]
#[path = "game_over_tests.rs"]
mod tests;
