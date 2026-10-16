//! AI modules that read the path finder: `StuckTracker` with its `MoveBacklog`, and
//! `DiscomfortTracker`. Decompile `StuckTracker.cs`, `DiscomfortTracker.cs`.

use super::WorldCoordinate;
use crate::FloatExt;
use crate::world::MovementConnection;

/// `StuckTracker.MoveBacklog`: the last 10 different moves, newest first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MoveBacklog {
    pub log: Vec<MovementConnection>,
}

impl MoveBacklog {
    const TRACKED_MOVES: usize = 10;

    /// `ReportNewMove`: kept unless it repeats the newest.
    pub fn report_new_move(&mut self, connection: Option<MovementConnection>) {
        let Some(c) = connection else { return };
        if self.log.first() == Some(&c) {
            return;
        }
        self.log.insert(0, c);
        if self.log.len() > Self::TRACKED_MOVES {
            self.log.pop();
        }
    }

    /// `IsMoveInLog` among the newest `steps_back`.
    #[must_use]
    pub fn is_move_in_log(&self, connection: Option<MovementConnection>, steps_back: i32) -> bool {
        let Some(c) = connection else { return false };
        let n = usize::try_from(steps_back).unwrap_or(0).min(self.log.len());
        self.log[..n].contains(&c)
    }
}

/// `StuckTracker` with the lizard's settings (`LizardAI`'s constructor): it tracks past
/// positions (40, compared from the 30th, 5 tiles close, more than 10 close counts as
/// stuck) and not following the current goal (200 at most), and has a `MoveBacklog`.
#[derive(Debug, Clone, PartialEq)]
pub struct StuckTracker {
    pub past_positions: Vec<(i32, i32)>,
    pub stuck_counter: i32,
    pub not_following_path_to_current_goal_counter: i32,
    pub not_following_current_goal_max: i32,
    pub goal_satisfaction_distance: i32,
    pub total_tracked_last_positions: usize,
    pub check_past_positions_from: usize,
    pub past_pos_stuck_distance: f32,
    pub past_stuck_positions_close_to_increment_stuck_counter: i32,
    pub min_stuck_counter: i32,
    pub max_stuck_counter: i32,
    pub satisfied_with_this_position: bool,
    pub move_backlog: MoveBacklog,
}

impl StuckTracker {
    #[must_use]
    pub fn lizard() -> Self {
        Self {
            past_positions: Vec::new(),
            stuck_counter: 0,
            not_following_path_to_current_goal_counter: 0,
            not_following_current_goal_max: 200,
            goal_satisfaction_distance: 3,
            total_tracked_last_positions: 40,
            check_past_positions_from: 30,
            past_pos_stuck_distance: 5.0,
            past_stuck_positions_close_to_increment_stuck_counter: 10,
            min_stuck_counter: 100,
            max_stuck_counter: 200,
            satisfied_with_this_position: false,
            move_backlog: MoveBacklog::default(),
        }
    }

    /// `Update`: the backlog adds nothing to the count; not following the goal counts up
    /// to its max while the effectual destination is not the destination; away from the
    /// goal (or long off it) the position is logged, and more than the threshold of old
    /// positions close by makes it stuck (+1, else -2, 0-200).
    pub fn update(
        &mut self,
        pos: WorldCoordinate,
        destination: WorldCoordinate,
        effectual_destination: WorldCoordinate,
    ) {
        if self.not_following_path_to_current_goal_counter < self.not_following_current_goal_max
            && effectual_destination != destination
        {
            self.not_following_path_to_current_goal_counter += 1;
        } else if self.not_following_path_to_current_goal_counter > 0 {
            self.not_following_path_to_current_goal_counter -= 1;
        }
        let manhattan = if pos.room == effectual_destination.room {
            (pos.tile.0 - effectual_destination.tile.0).abs()
                + (pos.tile.1 - effectual_destination.tile.1).abs()
        } else {
            i32::MAX
        };
        let mut close = 0;
        if !self.satisfied_with_this_position
            && (manhattan > self.goal_satisfaction_distance
                || self.not_following_path_to_current_goal_counter
                    > self.not_following_current_goal_max / 2)
        {
            self.past_positions.insert(0, pos.tile);
            if self.past_positions.len() > self.total_tracked_last_positions {
                self.past_positions.pop();
            }
            for p in self
                .past_positions
                .iter()
                .skip(self.check_past_positions_from)
            {
                let (dx, dy) = ((pos.tile.0 - p.0) as f32, (pos.tile.1 - p.1) as f32);
                if dx * dx + dy * dy < self.past_pos_stuck_distance * self.past_pos_stuck_distance {
                    close += 1;
                }
            }
        }
        if close > self.past_stuck_positions_close_to_increment_stuck_counter {
            self.stuck_counter += 1;
        } else {
            self.stuck_counter -= 2;
        }
        self.stuck_counter = self.stuck_counter.clamp(0, self.max_stuck_counter);
    }

    /// `NewRoom`: the backlog is cleared.
    pub fn new_room(&mut self) {
        self.move_backlog.log.clear();
    }

    /// `Utility`.
    #[must_use]
    pub fn utility(&self) -> f32 {
        (self.stuck_counter as f32)
            .inverse_lerp(self.min_stuck_counter as f32, self.max_stuck_counter as f32)
    }
}

/// `DiscomfortTracker`: discomfort comes from tracked uncomfortable creatures, unknown
/// noises, an uncomfortable item and threats, all of them lizard step 6's trackers;
/// until then every tile is comfortable, as the game's is with nothing tracked.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DiscomfortTracker {
    pub uncomfortable_with_unknown_noises: f32,
}

impl DiscomfortTracker {
    /// `DiscomfortOfTile`.
    #[must_use]
    pub fn discomfort_of_tile(&self, _test: WorldCoordinate) -> f32 {
        0.0
    }
}
