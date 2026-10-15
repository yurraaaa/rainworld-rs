//! The app's save slot: where the port's saves and the game's live, loading the slot
//! for the menus, what a story game takes from its save state, and the importer.

use std::path::PathBuf;

use rw_assets::StreamingAssets;
use rw_save::{ChecksumSalt, ImportReport, Platform, PlayerProgression, SaveSlot, SaveState};
use rw_sim::campaign::StoryStart;

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
            den,
        }
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
}
