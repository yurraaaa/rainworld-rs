//! `InitializationScreen`, reduced: the port loads fonts, strings and settings before
//! the manager starts, so on its first tick it requests the start process (the game's
//! `WRAP_UP` step: `IntroRoll`, or `Game` with `setup.startScreen` off).

use crate::process::{MainLoopProcess, ProcessId, ProcessRequests};
use crate::stage::Container;

#[derive(Debug, Clone)]
pub struct Initialization {
    start: ProcessId,
    requested: bool,
    /// An empty page: only the fade is drawn over it.
    stage: Container,
}

impl Initialization {
    #[must_use]
    pub fn new(start: ProcessId) -> Self {
        Self {
            start,
            requested: false,
            stage: Container::default(),
        }
    }

    #[must_use]
    pub fn stage(&self) -> &Container {
        &self.stage
    }
}

impl<C> MainLoopProcess<C> for Initialization {
    fn id(&self) -> ProcessId {
        ProcessId::Initialization
    }

    fn update(&mut self, _ctx: &mut C, requests: &mut ProcessRequests) {
        if !self.requested {
            self.requested = true;
            requests.switch_to(self.start);
        }
    }

    fn graf_update(&mut self, _time_stacker: f32, _ctx: &mut C) {}
}
