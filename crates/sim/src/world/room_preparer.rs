//! A loading room's `RoomPreparer`: a worker thread maps the room's shortcuts and builds
//! its AI map (`UpdateThread`), handing each stage to the main thread through flags under
//! a lock, which the main thread's `Update` (once a tick, in the loading step) takes on
//! whichever tick they are set. The worker reads only the realized room's geometry and draws
//! nothing; it waits on a condition variable where the game's thread spins, and the
//! main thread puts its maps on the room as it takes each stage (D14). The stages
//! themselves run on the main thread in
//! [`crate::game::loading_turn`]. A preparer stays in `loadingRooms` when its room is
//! abstracted, as in the game.
//!
//! Mirrors: RoomPreparer.cs (Update, UpdateThread)

use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::{Condvar, Mutex, MutexGuard};
use rw_assets::{Room, ShortcutMap};

use super::ai_map::AiMap;

/// A stage `Update` hands to the room, with what the worker made for it.
#[derive(Debug)]
pub enum PreparerStage {
    /// `ShortcutMapper`'s `shortcuts`, `shortcutsIndex` and `exitAndDenIndex`.
    ShortcutsReady(ShortcutMap),
    /// `AImapper`'s map with the decoded heat maps (`room.aimap`).
    ReadyForAi(Box<AiMap>),
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

/// What the worker builds from. `room` is the room as it stands when the thread starts
/// (as realized, for a room abstracted before that), and the AI map reads it as it stands
/// when stage 1 is taken: the game's thread reads the live room.
#[derive(Debug, Clone)]
pub struct PreparerJob {
    pub room: Arc<Room>,
    /// The room's global index.
    pub index: i32,
    /// Shelters and gates have no border exits.
    pub shelter_or_gate: bool,
    /// A room with the LethalWater effect.
    pub lethal_water: bool,
}

impl PreparerJob {
    /// `UpdateThread`'s status 0: the `ShortcutMapper` over every tile.
    #[must_use]
    pub fn map_shortcuts(&self) -> ShortcutMap {
        ShortcutMap::new(&self.room)
    }

    /// Statuses 1 and 2: `AImapper`, then `DecompressStringToAImaps` (`loadAiHeatMaps`
    /// is true for every `RealizeRoom`).
    #[must_use]
    pub fn map_ai(&self, shortcuts: ShortcutMap) -> AiMap {
        AiMap::with_shortcuts(
            &self.room,
            shortcuts,
            self.index,
            self.shelter_or_gate,
            self.lethal_water,
        )
    }
}

/// The fields `RoomPreparer` shares with its thread under `lock (this)`.
#[derive(Debug, Default)]
struct HandOff {
    status: PreparerStatus,
    request_shortcuts_ready: bool,
    request_ready_for_ai: bool,
    thread_finished: bool,
    /// Port-only: the preparer was dropped, so the thread stops where it waits.
    cancelled: bool,
    shortcuts: Option<ShortcutMap>,
    /// The room as the main thread took stage 1, for the AI map.
    ai_room: Option<Arc<Room>>,
    ai_map: Option<AiMap>,
}

/// The lock and the wake-up the worker waits on where the game's thread spins.
#[derive(Debug, Default)]
struct Shared {
    state: Mutex<HandOff>,
    changed: Condvar,
}

impl Shared {
    /// Blocks while the thread waits at a hand-off in `status` that the main thread has
    /// not taken; `false` when the preparer was dropped meanwhile.
    fn wait_while(&self, guard: &mut MutexGuard<'_, HandOff>, status: PreparerStatus) -> bool {
        while guard.status == status && !guard.cancelled {
            self.changed.wait(guard);
        }
        !guard.cancelled
    }
}

/// The worker thread's side (`UpdateThread`).
struct PreparerThread {
    job: PreparerJob,
    shared: Arc<Shared>,
}

impl PreparerThread {
    fn run(mut self) {
        let shortcuts = self.job.map_shortcuts();
        {
            let mut s = self.shared.state.lock();
            s.shortcuts = Some(shortcuts.clone());
            s.request_shortcuts_ready = true;
            self.shared.changed.notify_all();
            if !self
                .shared
                .wait_while(&mut s, PreparerStatus::MappingShortcuts)
            {
                return;
            }
            if let Some(room) = s.ai_room.take() {
                self.job.room = room;
            }
        }
        let ai_map = self.job.map_ai(shortcuts);
        let mut s = self.shared.state.lock();
        s.status = PreparerStatus::AiReady;
        s.ai_map = Some(ai_map);
        s.request_ready_for_ai = true;
        self.shared.changed.notify_all();
        if !self.shared.wait_while(&mut s, PreparerStatus::AiReady) {
            return;
        }
        s.thread_finished = true;
        self.shared.changed.notify_all();
    }
}

/// `RoomPreparer` of global room `room`'s realization `serial`.
#[derive(Debug)]
pub struct RoomPreparer {
    pub room: i32,
    /// The realization it loads ([`crate::world::RealizedRoom::serial`]): it outlives an
    /// `Abstractize` of that room, as the game's does.
    pub serial: u64,
    /// What the thread takes when the first `Update` starts it.
    job: Option<PreparerJob>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    /// `Update` calls so far.
    calls: u32,
    done: bool,
}

impl RoomPreparer {
    #[must_use]
    pub fn new(room: i32, serial: u64, job: PreparerJob) -> Self {
        Self {
            room,
            serial,
            job: Some(job),
            shared: Arc::default(),
            thread: None,
            calls: 0,
            done: false,
        }
    }

    #[must_use]
    pub fn status(&self) -> PreparerStatus {
        self.shared.state.lock().status
    }

    #[must_use]
    pub fn calls(&self) -> u32 {
        self.calls
    }

    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }

    /// Whether the first `Update` has started the thread.
    #[must_use]
    pub fn thread_started(&self) -> bool {
        self.job.is_none()
    }

    /// `threadFinished`.
    #[must_use]
    pub fn thread_finished(&self) -> bool {
        self.shared.state.lock().thread_finished
    }

    /// The stage the thread has requested and the next `Update` will take.
    #[must_use]
    pub fn requested_stage(&self) -> Option<u8> {
        let s = self.shared.state.lock();
        if s.request_shortcuts_ready {
            Some(1)
        } else if s.request_ready_for_ai {
            Some(2)
        } else {
            None
        }
    }

    /// Whether stage 2 has been handed over: the room it loads is `fullyLoaded`, even
    /// one abstracted since (the game's `Room` object keeps loading).
    #[must_use]
    pub fn handed_over_ready_for_ai(&self) -> bool {
        self.status() == PreparerStatus::Finishing
    }

    /// `Update`: starts the thread on the first call, then takes the stage it requested
    /// (at most one: the thread waits at each hand-off) and sees it finished. `live` is
    /// the room's current geometry while it is realized.
    ///
    /// # Panics
    /// Resumes a panic of the thread (it would otherwise leave the room loading).
    pub fn update(&mut self, live: Option<&Arc<Room>>) -> Vec<PreparerStage> {
        self.calls += 1;
        let mut stages = Vec::new();
        let mut finished = None;
        {
            let mut s = self.shared.state.lock();
            // Started under the lock, as the game's, so its first request waits for the
            // next call.
            if !s.thread_finished
                && let Some(mut job) = self.job.take()
            {
                if let Some(room) = live {
                    job.room = Arc::clone(room);
                }
                let thread = PreparerThread {
                    job,
                    shared: Arc::clone(&self.shared),
                };
                self.thread = Some(
                    std::thread::Builder::new()
                        .name("RoomPreparer".to_owned())
                        .spawn(move || thread.run())
                        .expect("the room preparer's thread starts"),
                );
            }
            if s.request_shortcuts_ready {
                s.request_shortcuts_ready = false;
                s.status = PreparerStatus::MappingAi;
                s.ai_room = live.cloned();
                if let Some(map) = s.shortcuts.take() {
                    stages.push(PreparerStage::ShortcutsReady(map));
                }
            }
            if s.request_ready_for_ai {
                s.request_ready_for_ai = false;
                s.status = PreparerStatus::Finishing;
                if let Some(map) = s.ai_map.take() {
                    stages.push(PreparerStage::ReadyForAi(Box::new(map)));
                }
            }
            if s.thread_finished || self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
                finished = self.thread.take();
            }
            if s.thread_finished {
                self.done = true;
            }
        }
        self.shared.changed.notify_all();
        if let Some(thread) = finished
            && let Err(panic) = thread.join()
        {
            std::panic::resume_unwind(panic);
        }
        stages
    }

    /// Blocks until the next `Update` has something to see: a requested stage or the
    /// thread finished (tests that step stage by stage).
    ///
    /// # Panics
    /// When the thread hands nothing over within 60 s.
    pub fn await_thread(&self) {
        if !self.thread_started() {
            return;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut s = self.shared.state.lock();
        while !(s.request_shortcuts_ready || s.request_ready_for_ai || s.thread_finished) {
            if self.thread.as_ref().is_some_and(JoinHandle::is_finished) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the room preparer's thread does not hand over"
            );
            self.shared
                .changed
                .wait_for(&mut s, std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for RoomPreparer {
    /// Port-only: a preparer dropped mid-load (its world gone) releases its thread.
    fn drop(&mut self) {
        self.shared.state.lock().cancelled = true;
        self.shared.changed.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
#[path = "tests/room_preparer.rs"]
mod tests;
