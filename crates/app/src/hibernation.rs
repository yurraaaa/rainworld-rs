//! `RainWorldGame.Win`: the save side (`rw_save`) run over the sim, and what the next
//! cycle takes from it.

use rw_assets::StreamingAssets;
use rw_assets::world::Regions;
use rw_random::UnityRandom;
use rw_save::session::{HibernationStats, HibernationWorld, PlayerSessionRecord};
use rw_save::{PlayerProgression, SaveError, WinOutcome};
use rw_sim::RainWorldGame;
use rw_sim::world::RegionState;

/// The running game as `SessionEnded` and `BringUpToDate` see it.
pub struct HibernatingGame<'a> {
    pub sim: &'a mut RainWorldGame,
    pub assets: &'a StreamingAssets,
}

impl HibernationWorld for HibernatingGame<'_> {
    fn deplete_one_swarm_room(&mut self, rng: &mut UnityRandom) {
        self.sim.world_mut().deplete_one_swarm_room(rng);
    }

    fn food_in_room_eat_and_destroy(&mut self) -> i32 {
        self.sim.food_in_room(true)
    }

    fn den_room(&self) -> String {
        let room = self.sim.player_pos().room;
        self.sim
            .world()
            .abstract_room(room)
            .map(|r| r.name.clone())
            .unwrap_or_default()
    }

    fn next_issued_id(&self) -> i32 {
        self.sim.next_issued_id()
    }

    fn region_name(&self) -> String {
        self.sim.world().name().to_owned()
    }

    fn adapt_region_state_to_world(&mut self, respawn: &mut Vec<i32>) -> String {
        let shelter = self.sim.player_pos().room;
        self.sim
            .world_mut()
            .adapt_region_state_to_world(shelter, respawn)
            .unwrap_or_default()
    }

    fn region_names(&self) -> Vec<String> {
        Regions::load(self.assets)
            .map(|r| r.0.into_iter().map(|r| r.name).collect())
            .unwrap_or_default()
    }

    fn default_alignments(&self) -> String {
        std::fs::read_to_string(self.assets.resolve_file_path("World/default alignments.txt"))
            .unwrap_or_default()
    }

    /// `TrySetVanillaDen`: `World/<prefix>-Rooms/<room>.txt` or
    /// `World/Gate Shelters/<room>.txt`, lower-cased.
    fn vanilla_room_exists(&self, room: &str) -> bool {
        let prefix = if room.contains('_') {
            room.split('_').next().unwrap_or("")
        } else {
            ""
        };
        [
            format!("World/{prefix}-Rooms/{room}.txt"),
            format!("World/Gate Shelters/{room}.txt"),
        ]
        .iter()
        .any(|p| self.assets.resolve_file_path(p).is_file())
    }
}

impl HibernatingGame<'_> {
    /// `Win(malnourished)`: the save's end of cycle with the sim's stream, written to
    /// disk. Returns where the game goes next and the region state the next cycle keeps
    /// in memory.
    ///
    /// # Errors
    /// When the save cannot be written.
    pub fn win(
        &mut self,
        saves: &mut PlayerProgression,
        malnourished: bool,
        backed_up: &mut bool,
    ) -> Result<(WinOutcome, Option<RegionState>), SaveError> {
        let story = self.sim.story().clone();
        let stats = self.sim.campaign().stats;
        let stats = if story.malnourished {
            stats.malnourished()
        } else {
            stats
        };
        let stats = HibernationStats {
            max_food: stats.max_food,
            food_to_hibernate: stats.food_to_hibernate,
        };
        let mut record = self.sim.session_record();
        if let Some(state) = saves.current_save_state.as_mut() {
            state.respawn_creatures.clone_from(&story.respawn_creatures);
            state.tot_food += self.sim.session_food();
        }
        let mut rng = self.sim.take_rng();
        let result = saves.win(self, &mut record, stats, malnourished, backed_up, &mut rng);
        self.sim.set_rng(rng);
        let outcome = result?;
        if let Some(state) = saves.current_save_state.as_mut() {
            // The next cycle reuses this state: no `LoadGame`, so no saved `NEXTID`.
            state.loaded_next_issued_id = None;
        }
        Ok((outcome, self.sim.world().region_state.clone()))
    }
}

/// `PlayerSessionRecord` of the sim's cycle.
pub trait SessionRecordSource {
    fn session_record(&self) -> PlayerSessionRecord;
}

impl SessionRecordSource for RainWorldGame {
    fn session_record(&self) -> PlayerSessionRecord {
        let mut record = PlayerSessionRecord {
            woke_up_in_region: Some(self.world().name().to_owned()),
            time: i32::try_from(self.ticks()).unwrap_or(i32::MAX),
            ..PlayerSessionRecord::default()
        };
        for &id in self.eaten() {
            record.add_eat((id.spawner, id.number), rw_save::session::EatenKind::Meat);
        }
        record
    }
}
