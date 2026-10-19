//! The app's save slot: where the port's saves and the game's live, loading the slot
//! for the menus, what a story game takes from its save state, and the importer.
//!
//! Mirrors: PlayerProgression.cs, SaveState.cs

use std::path::PathBuf;

use rw_assets::StreamingAssets;
use rw_save::{
    ChecksumSalt, ImportReport, Options, Platform, PlayerProgression, SaveSlot, SaveState, Tutorial,
};
use rw_sim::campaign::StoryStart;
use rw_sim::creature::PlayerGuideState;

/// Where saves are read and written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveConfig {
    /// The port's save directory; `None` keeps saves in memory.
    pub dir: Option<PathBuf>,
    /// The game's save directory the importer reads; `None` finds it from the game's.
    pub game_saves: Option<PathBuf>,
}

impl SaveConfig {
    /// The platform's directory (`--save-dir` overrides it).
    #[must_use]
    pub fn platform(dir: Option<PathBuf>, game_saves: Option<PathBuf>) -> Self {
        Self {
            dir: dir.or_else(Platform::port_save_dir),
            game_saves,
        }
    }

    /// `Custom.CheckSumSalt`, from the installed game's assembly.
    #[must_use]
    pub fn salt(assets: &StreamingAssets) -> Option<ChecksumSalt> {
        let path = assets
            .data_dir()
            .join("Managed")
            .join("Assembly-CSharp.dll");
        ChecksumSalt::from_assembly(&path)
            .inspect_err(|e| log::warn!("no save checksum salt ({e}); saves are not written"))
            .ok()
    }

    /// `slot` of the port's directory, as `LoadProgression` reads it; in memory without
    /// a directory or when it cannot be read.
    #[must_use]
    pub fn load(&self, assets: &StreamingAssets, slot: SaveSlot) -> PlayerProgression {
        let salt = Self::salt(assets);
        let Some(dir) = &self.dir else {
            return PlayerProgression::in_memory(salt);
        };
        let path = dir.join(slot.file_name());
        PlayerProgression::load(&path, salt).unwrap_or_else(|e| {
            log::error!("cannot read the save {}: {e}; not saving", path.display());
            PlayerProgression::in_memory(None)
        })
    }

    /// The port's options, the defaults without a directory or when they cannot be read.
    #[must_use]
    pub fn load_options(&self) -> Options {
        let Some(dir) = &self.dir else {
            return Options::default();
        };
        Options::load(dir).unwrap_or_else(|e| {
            log::error!("cannot read the options in {}: {e}", dir.display());
            Options::default()
        })
    }

    /// `Options.Save` into the port's directory (nothing without one).
    pub fn save_options(&self, options: &Options) {
        if let Some(dir) = &self.dir
            && let Err(e) = options.save(dir)
        {
            log::error!("saving the options in {}: {e}", dir.display());
        }
    }

    /// The game's save directory: the override, else the platform's next to the game.
    #[must_use]
    pub fn game_save_dir(&self, assets: &StreamingAssets) -> Option<PathBuf> {
        self.game_saves.clone().or_else(|| {
            let game_dir = assets.data_dir().parent()?;
            Platform::current().game_save_dir_here(game_dir)
        })
    }

    /// Imports the game's `slot` into the port's directory (backing up the port's).
    ///
    /// # Errors
    /// Without a port directory or the game's, or when the import fails.
    pub fn import(&self, assets: &StreamingAssets, slot: SaveSlot) -> Result<ImportReport, String> {
        let dir = self
            .dir
            .as_deref()
            .ok_or("the port has no save directory")?;
        let game = self
            .game_save_dir(assets)
            .ok_or("no game save directory on this platform; give one")?;
        rw_save::import_slot(&game, dir, slot).map_err(|e| {
            format!(
                "importing {} from {}: {e}",
                slot.file_name(),
                game.display()
            )
        })
    }
}

/// What a story game takes from its loaded save state.
pub trait SaveStateStart {
    /// The start in `den` (`GetSaveStateDenToUse`); `in_memory` for a state the last
    /// cycle left (not read again: the game draws a new `nextIssuedId`).
    fn story_start(&self, den: String, in_memory: bool) -> StoryStart;
}

impl SaveStateStart for SaveState {
    fn story_start(&self, den: String, in_memory: bool) -> StoryStart {
        let d = &self.death_persistent;
        StoryStart {
            cycle_number: self.cycle_number,
            food: self.food,
            karma: d.karma,
            karma_cap: d.karma_cap,
            karma_reinforced: d.reinforced_karma,
            the_mark: d.the_mark,
            the_glow: self.the_glow,
            red_extra_cycles: self.red_extra_cycles(),
            guide_overseer_dead: self.guide_overseer_dead,
            next_issued_id: self.loaded_next_issued_id.filter(|_| !in_memory),
            seed: Some(self.seed),
            malnourished: self.malnourished,
            food_replenish_bonus: d.food_replenish_bonus,
            respawn_creatures: self.respawn_creatures.clone(),
            // `OverWorld.LoadFirstWorld`: the den's region.
            region_state: self
                .region_state(den.split('_').next().unwrap_or(&den))
                .map(str::to_owned),
            in_memory,
            live_region_state: None,
            swallowed_item: self.swallowed_item(0).map(str::to_owned),
            guide: self.misc_world.player_guide.to_game(),
            ever_met_moon: self.misc_world.ever_met_moon(),
            ss_ai_conversations_had: self.misc_world.ss_ai_conversations_had(),
            dangle_fruit_in_water_message: d.tutorial_message(Tutorial::DangleFruitInWater),
            communities: self.communities.clone(),
            difficulty: self.session_difficulty(),
            den,
        }
    }
}

/// The save state's `PlayerGuideState` and the game's copy of it: the game changes it
/// during a cycle, the save state keeps it (with its unknown entries) in memory and on
/// disk.
pub trait GuideStateCopy {
    fn to_game(&self) -> PlayerGuideState;
    fn update_from_game(&mut self, guide: &PlayerGuideState);
}

impl GuideStateCopy for rw_save::PlayerGuideState {
    fn to_game(&self) -> PlayerGuideState {
        PlayerGuideState {
            integers: self.integers,
            item_types: self.item_types.clone(),
            creature_types: self.creature_types.clone(),
            likes_player: self.likes_player,
            hand_holding: self.hand_holding,
            images_shown: self.images_shown.clone(),
            forced_directions_given: self.forced_directions_given.clone(),
            increase_like_on_save: self.increase_like_on_save,
            has_been_to_a_swarm_room_this_cycle: self.has_been_to_a_swarm_room_this_cycle,
            want_food_hand_holding_this_cycle: self.want_food_hand_holding_this_cycle,
            want_shelter_hand_holding_this_cycle: self.want_shelter_hand_holding_this_cycle,
            want_direction_hand_holding_this_cycle: self.want_direction_hand_holding_this_cycle,
        }
    }

    fn update_from_game(&mut self, guide: &PlayerGuideState) {
        self.integers = guide.integers;
        self.item_types.clone_from(&guide.item_types);
        self.creature_types.clone_from(&guide.creature_types);
        self.likes_player = guide.likes_player;
        self.hand_holding = guide.hand_holding;
        self.images_shown.clone_from(&guide.images_shown);
        self.forced_directions_given
            .clone_from(&guide.forced_directions_given);
        self.increase_like_on_save = guide.increase_like_on_save;
        self.has_been_to_a_swarm_room_this_cycle = guide.has_been_to_a_swarm_room_this_cycle;
        self.want_food_hand_holding_this_cycle = guide.want_food_hand_holding_this_cycle;
        self.want_shelter_hand_holding_this_cycle = guide.want_shelter_hand_holding_this_cycle;
        self.want_direction_hand_holding_this_cycle = guide.want_direction_hand_holding_this_cycle;
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::SlugcatName;

    use super::*;

    /// Continue takes the den region's `REGIONSTATE` (its `ROOMSVISITED` in order, which
    /// the HUD map's `CreateDiscoveryTextureFromVisitedRooms` reads) and the respawns.
    #[test]
    fn continue_takes_the_den_regions_state_and_respawns() {
        let line = "SAV STATE NUMBER<svB>White<svA>DENPOS<svB>CC_S04<svA>CYCLENUM<svB>5<svA>\
            NEXTID<svB>777<svA>RESPAWNS<svB>3011.3047<svA>\
            REGIONSTATE<svB>REGIONNAME<rgB>SU<rgA>ROOMSVISITED<rgB>SU_C04<rgA><svA>\
            REGIONSTATE<svB>REGIONNAME<rgB>CC<rgA>LASTCYCLEUPDATED<rgB>4<rgA>ROOMSVISITED\
            <rgB>CC_H01,CC_A06,CC_C09,GATE_SI_CC,CC_S04,CC_C07,CC_C12<rgA><svA>";
        let mut state = SaveState::new(SlugcatName::White, 0);
        state.load_game(line, "SU_C04", true).unwrap();
        let start = state.story_start("CC_S04".to_owned(), false);
        assert_eq!(start.respawn_creatures, [3011, 3047]);
        assert_eq!(start.next_issued_id, Some(777));
        assert_eq!(
            start.region_state.as_deref(),
            Some(
                "REGIONNAME<rgB>CC<rgA>LASTCYCLEUPDATED<rgB>4<rgA>ROOMSVISITED<rgB>\
                 CC_H01,CC_A06,CC_C09,GATE_SI_CC,CC_S04,CC_C07,CC_C12<rgA>"
            )
        );
        // The next cycle's state in memory: `nextIssuedId` is drawn again.
        assert_eq!(
            state.story_start("CC_S04".to_owned(), true).next_issued_id,
            None
        );
    }

    /// The whole guide state goes to the game and back: the save keeps its unknown
    /// entries, the per-cycle fields stay with the state in memory, and Moon's
    /// encounters and Pebbles' conversations come from the rest of the misc data.
    #[test]
    fn guide_state_goes_to_the_game_and_back() {
        let line = "SAV STATE NUMBER<svB>White<svA>MISCWORLDSAVEDATA<svB>SSaiConversationsHad<mwB>2\
            <mwA>SLaiState<mwB>integersArray<slosB>1.0<slosA><mwA>playerGuideState<mwB>\
            integersArray<pgsB>0.2.1.0.0.0.1<pgsA>itemTypes<pgsB>DangleFruit<pgsA>creatureTypes\
            <pgsB>PinkLizard<pgsA>likesPlayer<pgsB>0.5<pgsA>directionHandHolding<pgsB>0.7<pgsA>\
            imagesShown<pgsB>SU_A22<pgsA>modKey<pgsB>1<pgsA><mwA><svA>";
        let mut state = SaveState::new(SlugcatName::White, 0);
        state.load_game(line, "SU_C04", true).unwrap();
        let start = state.story_start("SU_C04".to_owned(), false);
        assert!(start.ever_met_moon);
        assert_eq!(start.ss_ai_conversations_had, 2);
        let mut guide = start.guide;
        assert_eq!(guide.super_jumps_shown(), 2);
        assert_eq!(guide.guide_symbol(), 1);
        assert!(guide.knows_item_type(rw_assets::AbstractObjectType::DangleFruit));
        assert!(guide.has_image_been_shown_in_room("SU_A22"));
        guide.set_super_jumps_shown(3);
        guide.image_shown_in_room("SU_A37");
        guide.increment_times_forced_direction_has_been_given(4);
        guide.want_food_hand_holding_this_cycle = 0.25;
        let saved = &mut state.misc_world.player_guide;
        saved.update_from_game(&guide);
        assert_eq!(saved.want_food_hand_holding_this_cycle, 0.25);
        let text = saved.save_string();
        // A forced direction makes `ToString` start over from it; each save also raises
        // `likesPlayer` by 0.1 and `handHolding` by 0.025 (in floats).
        assert_eq!(text, "forcedDirsGiven<pgsB>4,1<pgsA>modKey<pgsB>1<pgsA>");
        saved.forced_directions_given.clear();
        assert_eq!(
            saved.save_string(),
            "integersArray<pgsB>0.3.1.0.0.0.1<pgsA>itemTypes<pgsB>DangleFruit<pgsA>\
             creatureTypes<pgsB>PinkLizard<pgsA>likesPlayer<pgsB>0.7<pgsA>\
             directionHandHolding<pgsB>0.7499999<pgsA>imagesShown<pgsB>SU_A22.SU_A37<pgsA>\
             modKey<pgsB>1<pgsA>"
        );
    }
}
