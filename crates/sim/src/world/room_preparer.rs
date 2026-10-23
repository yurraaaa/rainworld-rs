//! A loading room's `RoomPreparer`: the hand-offs of its background thread to the main
//! thread, one `Update` call per tick in the loading step. The thread's work (the
//! shortcut and AI mapping, which draw nothing) is a fixed number of calls per stage
//! (D14); the stages themselves run in [`crate::game::loading_turn`]. A preparer stays in
//! `loadingRooms` when its room is abstracted, as in the game.
//!
//! Mirrors: RoomPreparer.cs (Update, UpdateThread's status and requests)

/// How many loading-step calls the thread takes per stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingSchedule {
    /// Both stages and `done` on the first call (world-only tests).
    Immediate,
    /// The thread hands stage 1 over `shortcuts_calls` calls after the one that started
    /// it, and stage 2 `ai_calls` calls after stage 1.
    Deferred { shortcuts_calls: u32, ai_calls: u32 },
}

impl LoadingSchedule {
    /// The game's minimum: `ShortCutsReady` on the second call, `ReadyForAI` on the third.
    pub const GAME_MINIMUM: Self = Self::Deferred {
        shortcuts_calls: 1,
        ai_calls: 1,
    };
}

impl Default for LoadingSchedule {
    fn default() -> Self {
        Self::GAME_MINIMUM
    }
}

/// A stage `Update` hands to the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparerStage {
    ShortcutsReady,
    ReadyForAi,
}

/// The thread's `status`: what it works on or waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreparerStatus {
    /// 0: `ShortcutMapper`, then waiting for stage 1 to be taken.
    #[default]
    MappingShortcuts,
    /// 1: `AImapper` (2, the heat maps, follows in the same pass).
    MappingAi,
    /// 4: waiting for stage 2 to be taken.
    AiReady,
    /// 5: the thread ends on its next pass.
    Finishing,
}

impl PreparerStatus {
    /// The game's `status` number.
    #[must_use]
    pub fn value(self) -> u8 {
        match self {
            Self::MappingShortcuts => 0,
            Self::MappingAi => 1,
            Self::AiReady => 4,
            Self::Finishing => 5,
        }
    }
}

/// `RoomPreparer` of global room `room`'s realization `serial`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomPreparer {
    pub room: i32,
    /// The realization it loads ([`crate::world::RealizedRoom::serial`]): it outlives an
    /// `Abstractize` of that room, as the game's does.
    pub serial: u64,
    status: PreparerStatus,
    /// `Update` calls so far.
    calls: u32,
    /// Calls the thread has worked on its current stage.
    work: u32,
    thread_started: bool,
    request_shortcuts_ready: bool,
    request_ready_for_ai: bool,
    thread_finished: bool,
    done: bool,
}

impl RoomPreparer {
    #[must_use]
    pub fn new(room: i32, serial: u64) -> Self {
        Self {
            room,
            serial,
            status: PreparerStatus::default(),
            calls: 0,
            work: 0,
            thread_started: false,
            request_shortcuts_ready: false,
            request_ready_for_ai: false,
            thread_finished: false,
            done: false,
        }
    }

    #[must_use]
    pub fn status(&self) -> PreparerStatus {
        self.status
    }

    #[must_use]
    pub fn calls(&self) -> u32 {
        self.calls
    }

    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }

    /// Whether stage 2 has been handed over: the room it loads is `fullyLoaded`, even
    /// one abstracted since (the game's `Room` object keeps loading).
    #[must_use]
    pub fn handed_over_ready_for_ai(&self) -> bool {
        self.status == PreparerStatus::Finishing
    }

    /// `Update`: the thread's passes since the last call, then the main thread's part,
    /// which takes at most one stage (the thread waits at each hand-off).
    pub fn update(&mut self, schedule: LoadingSchedule) -> Vec<PreparerStage> {
        self.calls += 1;
        let LoadingSchedule::Deferred {
            shortcuts_calls,
            ai_calls,
        } = schedule
        else {
            self.status = PreparerStatus::Finishing;
            self.thread_finished = true;
            self.done = true;
            return vec![PreparerStage::ShortcutsReady, PreparerStage::ReadyForAi];
        };
        if self.thread_started {
            self.run_thread(shortcuts_calls, ai_calls);
        } else if !self.thread_finished {
            self.thread_started = true;
        }
        let mut stages = Vec::new();
        if self.request_shortcuts_ready {
            self.request_shortcuts_ready = false;
            self.status = PreparerStatus::MappingAi;
            stages.push(PreparerStage::ShortcutsReady);
        }
        if self.request_ready_for_ai {
            self.request_ready_for_ai = false;
            self.status = PreparerStatus::Finishing;
            stages.push(PreparerStage::ReadyForAi);
        }
        if self.thread_finished {
            self.done = true;
            self.thread_started = false;
        }
        stages
    }

    /// `UpdateThread` between two calls: it runs flat out, so a stage whose work is done
    /// is requested, and a taken stage 2 ends the thread.
    fn run_thread(&mut self, shortcuts_calls: u32, ai_calls: u32) {
        match self.status {
            PreparerStatus::MappingShortcuts if !self.request_shortcuts_ready => {
                self.work += 1;
                if self.work >= shortcuts_calls {
                    self.work = 0;
                    self.request_shortcuts_ready = true;
                }
            }
            PreparerStatus::MappingAi => {
                self.work += 1;
                if self.work >= ai_calls {
                    self.work = 0;
                    self.status = PreparerStatus::AiReady;
                    self.request_ready_for_ai = true;
                }
            }
            PreparerStatus::Finishing => self.thread_finished = true,
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "tests/room_preparer.rs"]
mod tests;
