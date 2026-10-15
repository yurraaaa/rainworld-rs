//! The game's parts for the bat flies: `FliesWorldAI.Update` among the world processes,
//! `ReadyForAI`'s placement of quantified creatures, and the per-call records a reference
//! model is compared against.

use rw_assets::CreatureType;

use super::RainWorldGame;
use crate::world::{FliesTick, PlacedQuantified};

/// Which flies' call a record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FliesCall {
    /// `FliesWorldAI.Update`.
    Update,
    /// `Room.ReadyForAI`'s quantified placement in the room.
    Ready(i32),
    /// `AbstractRoom.Abstractize` of the room (no draws).
    Abstractize(i32),
}

/// One flies' call with the random stream around it and the world it saw.
#[derive(Debug, Clone)]
pub struct FliesCallRecord {
    pub tick: u64,
    pub call: FliesCall,
    pub rng_before: [u32; 4],
    pub rng_after: [u32; 4],
    pub rain_timer: i32,
    pub cycle_length: i32,
    /// The realized rooms and whether each is fully loaded, before the call.
    pub realized: Vec<(i32, bool)>,
    pub placed: Vec<PlacedQuantified>,
    /// After the call: every room's stored fly counts that are not 0 (room, node, count).
    pub counts: Vec<(i32, i32, i32)>,
    /// After the call: the rooms holding fly `AbstractCreature`s, and how many.
    pub creatures: Vec<(i32, usize)>,
    /// `nextIssuedId` after the call.
    pub next_id: i32,
}

impl RainWorldGame {
    /// The world processes in `worldProcesses` order: `OverseersWorldAI` (made by the
    /// population's first overseer) before `FliesWorldAI` (made with the world after
    /// it). The overseers' update draws nothing, so their order does not matter.
    pub(super) fn update_world_processes(&mut self) {
        let region = self.world.region_name().unwrap_or_default().to_owned();
        let story = crate::world::StoryProgress {
            woke_up_in_region: &region,
            ever_met_moon: false,
            karma: self.campaign.start_karma,
        };
        self.world.update_world_processes(Some(story));
        let before = self.flies_record_start();
        let session = self.story_session();
        let mut cx = FliesTick {
            rng: &mut self.rng,
            ids: &mut self.ids,
            session: &session,
        };
        self.world.update_flies_world_ai(&mut cx);
        self.flies_record_end(before, FliesCall::Update, Vec::new());
    }

    /// `ReadyForAI`'s `PlaceQuantifiedCreaturesInRoom` for room `index`.
    pub(super) fn place_quantified(&mut self, index: i32) {
        let before = self.flies_record_start();
        let session = self.story_session();
        let mut cx = FliesTick {
            rng: &mut self.rng,
            ids: &mut self.ids,
            session: &session,
        };
        let placed = self.world.place_quantified_creatures(index, &mut cx);
        if !placed.is_empty() {
            log::info!("placed {} flies in room {index}", placed.len());
        }
        self.flies_record_end(before, FliesCall::Ready(index), placed);
    }

    /// Records the abstraction of the rooms that left the active list.
    pub(super) fn record_abstractized(&mut self, active_before: &[i32]) {
        if self.flies_calls.is_none() {
            return;
        }
        let now = self.world.active_rooms();
        for &room in active_before.iter().filter(|r| !now.contains(r)) {
            let before = self.flies_record_start();
            self.flies_record_end(before, FliesCall::Abstractize(room), Vec::new());
        }
    }

    fn flies_record_start(&self) -> Option<FliesCallRecord> {
        self.flies_calls.as_ref()?;
        let rain = &self.world.rain_cycle;
        Some(FliesCallRecord {
            tick: self.tick,
            call: FliesCall::Update,
            rng_before: self.rng.state(),
            rng_after: [0; 4],
            rain_timer: rain.timer,
            cycle_length: rain.cycle_length,
            realized: self
                .world
                .active_rooms()
                .into_iter()
                .map(|r| (r, self.world.realized(r).is_some_and(|x| x.fully_loaded())))
                .collect(),
            placed: Vec::new(),
            counts: Vec::new(),
            creatures: Vec::new(),
            next_id: 0,
        })
    }

    fn flies_record_end(
        &mut self,
        record: Option<FliesCallRecord>,
        call: FliesCall,
        placed: Vec<PlacedQuantified>,
    ) {
        let Some(mut record) = record else { return };
        record.call = call;
        record.rng_after = self.rng.state();
        record.placed = placed;
        let world = &self.world;
        for i in 0..world.number_of_rooms() {
            let room = world.first_room_index() + i32::try_from(i).unwrap_or(0);
            for q in world.room_quantified(room) {
                if q.kind == CreatureType::Fly && q.count != 0 {
                    record.counts.push((room, q.node, q.count));
                }
            }
            let flies = world
                .room_creatures(room)
                .iter()
                .filter(|&&c| world.creature(c).kind() == CreatureType::Fly)
                .count();
            if flies > 0 {
                record.creatures.push((room, flies));
            }
        }
        record.counts.sort_unstable();
        record.next_id = self.ids.next_issued_id;
        if let Some(calls) = &mut self.flies_calls {
            calls.push(record);
        }
    }

    /// Starts or stops recording the flies' calls.
    pub fn record_flies_calls(&mut self, on: bool) {
        self.flies_calls = on.then(Vec::new);
    }

    /// The flies' calls recorded since the last take.
    pub fn take_flies_calls(&mut self) -> Vec<FliesCallRecord> {
        self.flies_calls
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// The random state right before and after `FliesWorldAI`'s constructor in the world
    /// load.
    #[must_use]
    pub fn flies_ctor_rng(&self) -> [[u32; 4]; 2] {
        self.flies_ctor_rng
    }
}
