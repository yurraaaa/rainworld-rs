//! `RainWorldGame.Win`: the save side (`rw_save`) run over the sim, and what the next
//! cycle takes from it.

use rw_assets::StreamingAssets;
use rw_assets::world::Regions;
use rw_random::UnityRandom;
use rw_save::session::{HibernationStats, HibernationWorld, KillRecord, PlayerSessionRecord};
use rw_save::{PlayerProgression, SaveError, WinOutcome};
use rw_sim::creature::ViolenceTemplate;
use rw_sim::world::RegionState;
use rw_sim::{RainWorldGame, WorldCoordinate};

use crate::saves::GuideStateCopy;

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
        std::fs::read_to_string(
            self.assets
                .resolve_file_path("World/default alignments.txt"),
        )
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

    fn player_grasps(&self) -> Vec<String> {
        self.sim.player_grasp_save_strings()
    }

    fn swallowed_item(&self) -> Option<String> {
        self.sim.swallowed_item_save_string()
    }

    fn community_influences(&self) -> Vec<rw_save::CommunityInfluence> {
        self.sim
            .session_deaths()
            .influences
            .iter()
            .filter_map(|i| {
                Some(rw_save::CommunityInfluence {
                    community: rw_save::CommunityId::from_value(i.community.value())?,
                    influence: i.influence,
                    inter_region_bleed: i.inter_region_bleed,
                    inter_community_bleed: i.inter_community_bleed,
                })
            })
            .collect()
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
        // `GetStorySession.characterStats`.
        let stats = self.sim.character_stats();
        let stats = HibernationStats {
            max_food: stats.max_food,
            food_to_hibernate: stats.food_to_hibernate,
        };
        let mut record = self.sim.session_record();
        if let Some(state) = saves.current_save_state.as_mut() {
            let deaths = self.sim.session_deaths();
            state.respawn_creatures.clone_from(&story.respawn_creatures);
            state
                .respawn_creatures
                .extend_from_slice(&deaths.respawn_creatures);
            state
                .wait_respawn_creatures
                .extend_from_slice(&deaths.wait_respawn_creatures);
            state
                .misc_world
                .player_guide
                .update_from_game(self.sim.guide_state());
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

    /// `GoToStarveScreen`'s and `GoToDeathScreen`'s save: `SessionEnded(survived:
    /// false)` with the sim's stream and the death save. `death` is the HUD prompt's
    /// death room index and position.
    ///
    /// # Errors
    /// When the save cannot be written.
    pub fn session_ended_died(
        &mut self,
        saves: &mut PlayerProgression,
        death: Option<(i32, [f32; 2])>,
    ) -> Result<(), SaveError> {
        let record = self.sim.session_record();
        let travelled = record.woke_up_in_region.as_deref() != Some(self.sim.world().name());
        let room = death.map(|(room, pos)| (self.room_index_name(room), pos));
        let death = room.as_ref().map(|(room, pos)| (room.as_str(), *pos));
        let mut rng = self.sim.take_rng();
        let result = saves.session_ended_died(travelled, death, &mut rng);
        self.sim.set_rng(rng);
        result
    }

    /// `PlaceKarmaFlowerOnDeathSpot`'s store: the save's `karmaFlowerPosition` becomes
    /// the player's `karmaFlowerGrowPos`, as `FLOWERPOS` writes it (`None` clears it).
    /// The Hunter's `redsFlower` (out of cycles) is not ported.
    pub fn place_karma_flower_on_death_spot(
        &self,
        saves: &mut PlayerProgression,
        grow_pos: Option<WorldCoordinate>,
    ) {
        let Some(state) = saves.current_save_state.as_mut() else {
            return;
        };
        state.death_persistent.karma_flower_position = grow_pos.map(|c| {
            format!(
                "{}.{}.{}.{}",
                self.room_index_name(c.room),
                c.tile.0,
                c.tile.1,
                c.node
            )
        });
    }

    /// `WorldCoordinate.SaveToString`'s room: its name in `RainWorld.roomIndexToName`
    /// (`World/indexmaps/roomindexmap<worldVersion>.txt`), else the index.
    fn room_index_name(&self, room: i32) -> String {
        let version =
            std::fs::read_to_string(self.assets.resolve_file_path("World/worldVersion.txt"))
                .ok()
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(0);
        let map = std::fs::read_to_string(
            self.assets
                .resolve_file_path(&format!("World/indexmaps/roomindexmap{version}.txt")),
        )
        .unwrap_or_default();
        map.lines()
            .filter_map(|l| l.split_once(' '))
            .find(|(i, _)| i.trim().parse::<i32>().ok() == Some(room))
            .map_or_else(|| room.to_string(), |(_, name)| name.trim().to_owned())
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
        let deaths = self.session_deaths();
        record.peaceful = deaths.peaceful;
        record.kills = deaths
            .kills
            .iter()
            .map(|k| {
                let counts = ViolenceTemplate::of(k.kind).counts_as_a_kill;
                KillRecord::new(k.kind, k.int_data, counts, k.lizard)
            })
            .collect();
        record
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rw_assets::world::{
        GameRooms, LineFilter, RoomSource, StoryConditions, WorldData, WorldRequest,
    };
    use rw_assets::{ActiveMods, tile_centre};
    use rw_save::{CampaignStart, ChecksumSalt};
    use rw_sim::campaign::{SlugcatCampaign, StoryStart};
    use rw_sim::shelter::{SessionEnd, ShelterDoor};
    use rw_sim::world::World;
    use rw_sim::{Input, Sounds, Start, Vec2};

    use super::*;
    use crate::saves::SaveStateStart;

    const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
    const START: CampaignStart<'static> = CampaignStart {
        slugcat: rw_assets::SlugcatName::White,
        den: "SU_C04",
        vanilla: true,
    };

    fn assets() -> Option<StreamingAssets> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        sa.join("world/regions.txt")
            .is_file()
            .then(|| StreamingAssets::new(sa))
    }

    /// A Survivor story from `story` with the player in `room` (on the shelter's spawn
    /// position in a shelter).
    fn game(assets: &StreamingAssets, room: &str, story: StoryStart) -> RainWorldGame {
        let campaign = &SlugcatCampaign::SURVIVOR;
        let filter = LineFilter {
            timeline: Some(campaign.timeline),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        };
        let regions = Regions::load(assets).unwrap();
        let request = WorldRequest::for_first_room(assets, room, filter);
        let data = WorldData::load(assets, &regions, &request).unwrap();
        let first = data.room_named(room).unwrap().index;
        let rooms = Arc::new(GameRooms {
            assets: assets.clone(),
            region: Some(request.name.to_lowercase()),
            timeline: campaign.timeline,
        });
        let tile = data.room_named(room).filter(|r| r.shelter()).map(|r| {
            let loaded = rooms.load_room(&r.name).unwrap();
            ShelterDoor::player_spawn_pos(&loaded.room, &loaded.settings, false, ActiveMods::NONE)
        });
        let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
        let start = Start { room: first, tile };
        RainWorldGame::new_story(
            world,
            campaign,
            story,
            start,
            UnityRandom::new(1),
            Sounds::default(),
            &mut (),
        )
    }

    /// Into SU_S01 from SU_A22, then still on the spawn position with 4 food until the
    /// door shuts (as `installed_shelter.rs`).
    fn hibernate(game: &mut RainWorldGame) -> SessionEnd {
        for _ in 0..450 {
            game.tick(Input::default(), &mut ());
        }
        let world = game.world();
        let src = world.room_index("SU_A22").unwrap();
        let shelter = world.room_index("SU_S01").unwrap();
        let node = world.abstract_room(src).unwrap().exit_index(shelter);
        let room = &world.realized(src).unwrap().room;
        let entrance = room
            .shortcut_map()
            .leading_to_node(usize::try_from(node).unwrap())
            .unwrap()
            .start;
        let hole = room.shortcut_hole_direction(entrance.0, entrance.1);
        let input = Input::new(-hole.0 as i8, -hole.1 as i8, false);
        game.set_player(
            Vec2::from(tile_centre(entrance.0, entrance.1)),
            Vec2::ZERO,
            &mut (),
        );
        while game.player_pos().room != shelter || game.player().unwrap().in_shortcut() {
            game.tick(input, &mut ());
        }
        let spawn = game.shelter_door(shelter).unwrap().player_spawn_pos;
        game.set_food_in_stomach(4);
        game.set_player(
            Vec2::from(tile_centre(spawn.0, spawn.1)),
            Vec2::ZERO,
            &mut (),
        );
        loop {
            game.tick(Input::default(), &mut ());
            if let Some(end) = game.take_session_end() {
                return end;
            }
        }
    }

    #[test]
    fn a_first_hibernation_saves_the_cycle_and_the_next_starts_from_it() {
        let Some(assets) = assets() else { return };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sav");
        let mut saves = PlayerProgression::load(&path, Some(ChecksumSalt::new("s"))).unwrap();
        let mut sim = game(
            &assets,
            "SU_A22",
            StoryStart::new_game(&SlugcatCampaign::SURVIVOR),
        );
        let seed = sim.save_state_seed();
        saves
            .get_or_initiate_save_state(START, seed, false, true)
            .unwrap();
        let end = hibernate(&mut sim);
        assert_eq!(
            end,
            SessionEnd::Win {
                malnourished: false
            }
        );
        let mut backed_up = false;
        let (outcome, region) = HibernatingGame {
            sim: &mut sim,
            assets: &assets,
        }
        .win(&mut saves, false, &mut backed_up)
        .unwrap();
        assert_eq!(outcome, WinOutcome::SleepScreen);
        assert!(backed_up);
        // Read back from the file: den, cycle 0 -> 1, food 4 - foodToHibernate 4, karma
        // 0 -> 1, a survive, the SU region state with its population.
        let mut again = PlayerProgression::load(&path, Some(ChecksumSalt::new("s"))).unwrap();
        let line = again
            .prog_lines()
            .into_iter()
            .find(|l| l.starts_with("SAVE STATE"))
            .unwrap();
        for part in [
            "<svA>DENPOS<svB>SU_S01<svA>",
            "<svA>LASTVDENPOS<svB>SU_S01<svA>",
            "<svA>CYCLENUM<svB>1<svA>FOOD<svB>0<svA>",
            "<dpA>KARMA<dpB>1<dpA>",
            "<dpA>SURVIVES<dpB>1<dpA>",
            "<svA>REGIONSTATE<svB>REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>0<rgA>",
            "<rgA>POPULATION<rgB>",
            "SU_S01<rgA>",
        ] {
            assert!(line.contains(part), "{part} not in {line}");
        }

        // The next cycle: the state in memory, its region state kept, the door shut.
        let state = saves
            .state_in_memory(rw_assets::SlugcatName::White)
            .unwrap();
        let mut story = state.story_start("SU_S01".to_owned(), true);
        assert_eq!((story.cycle_number, story.food, story.karma), (1, 0, 1));
        assert_eq!(story.next_issued_id, None);
        story.live_region_state = region;
        let saved = story.live_region_state.clone().unwrap();
        let next = game(&assets, "SU_S01", story);
        let shelter = next.world().room_index("SU_S01").unwrap();
        assert_eq!(next.shelter_door(shelter).unwrap().closed_fac, 1.0);
        // Every saved creature is back in its den.
        assert!(!saved.saved_population.is_empty());
        for entry in &saved.saved_population {
            let id = entry
                .split("<cA>")
                .nth(1)
                .unwrap()
                .split("<cB>")
                .next()
                .unwrap();
            let found = next
                .world()
                .creatures()
                .any(|(_, c)| format!("ID.{}.{}", c.id.spawner, c.id.number) == id && c.in_den);
            assert!(found, "{entry} restored");
        }
    }

    /// The HUD prompt's unset death room 0 saves as `GATE_CC_UW`, the first line of the
    /// installed `roomindexmap2.txt`.
    #[test]
    fn a_starve_saves_the_death_with_the_index_map_room_name() {
        let Some(assets) = assets() else { return };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sav");
        let mut saves = PlayerProgression::load(&path, Some(ChecksumSalt::new("s"))).unwrap();
        let mut sim = game(
            &assets,
            "SU_S01",
            StoryStart::new_game(&SlugcatCampaign::SURVIVOR),
        );
        saves
            .get_or_initiate_save_state(START, sim.save_state_seed(), false, true)
            .unwrap();
        saves.save_world_state_and_progression(false).unwrap();
        HibernatingGame {
            sim: &mut sim,
            assets: &assets,
        }
        .session_ended_died(&mut saves, Some((0, [0.0, 0.0])))
        .unwrap();
        let mut again = PlayerProgression::load(&path, Some(ChecksumSalt::new("s"))).unwrap();
        let lines = again.prog_lines();
        let line = lines.iter().find(|l| l.starts_with("SAVE STATE")).unwrap();
        assert!(
            line.contains("<dpA>DEATHPOSS<dpB>GATE_CC_UW.0.0.0<dpA>"),
            "{line}"
        );
        assert!(line.contains("<dpA>DEATHS<dpB>1<dpA>"), "{line}");
    }

    /// Reinforced karma places a flower: the death's `PlaceKarmaFlowerOnDeathSpot`
    /// stores the last `karmaFlowerGrowPos` as `FLOWERPOS` (room by the index map, the
    /// tile, node -1), and the death save writes it.
    #[test]
    fn a_reinforced_death_saves_the_flower_position() {
        let Some(assets) = assets() else { return };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sav");
        let mut saves = PlayerProgression::load(&path, Some(ChecksumSalt::new("s"))).unwrap();
        let story = StoryStart {
            karma_reinforced: true,
            ..StoryStart::new_game(&SlugcatCampaign::SURVIVOR)
        };
        let mut sim = game(&assets, "SU_C04", story);
        saves
            .get_or_initiate_save_state(START, sim.save_state_seed(), false, true)
            .unwrap();
        saves.save_world_state_and_progression(false).unwrap();
        // Onto the first open floor tile (two free tiles above, not a narrow space).
        let room = sim.player_pos().room;
        let realized = sim.world().realized(room).unwrap();
        let (r, map) = (&realized.room, realized.ai_map.as_ref().unwrap());
        let floor = (0..r.width())
            .flat_map(|x| (1..r.height() - 2).map(move |y| (x, y)))
            .find(|&(x, y)| {
                r.tile(x, y - 1).is_solid()
                    && !r.tile(x, y).is_solid()
                    && !r.tile(x, y + 1).is_solid()
                    && !r.tile(x, y + 2).is_solid()
                    && !map.narrow_space(x, y)
            })
            .unwrap();
        sim.set_player(
            Vec2::from(tile_centre(floor.0, floor.1)),
            Vec2::ZERO,
            &mut (),
        );
        for _ in 0..40 {
            sim.tick(Input::default(), &mut ());
        }
        let grow = sim.karma_flower_grow_pos().expect("stands on the floor");
        sim.kill_player();
        let over = sim.take_game_over().expect("game over");
        assert_eq!(over.karma_flower, Some(Some(grow)));
        let mut game = HibernatingGame {
            sim: &mut sim,
            assets: &assets,
        };
        game.place_karma_flower_on_death_spot(&mut saves, Some(grow));
        game.session_ended_died(
            &mut saves,
            Some((over.death_room, over.death_pos.to_array())),
        )
        .unwrap();
        let mut again = PlayerProgression::load(&path, Some(ChecksumSalt::new("s"))).unwrap();
        let lines = again.prog_lines();
        let line = lines.iter().find(|l| l.starts_with("SAVE STATE")).unwrap();
        let flower = format!(
            "<dpA>FLOWERPOS<dpB>SU_C04.{}.{}.-1<dpA>",
            grow.tile.0, grow.tile.1
        );
        assert!(line.contains(&flower), "{line}");
    }
}
