//! A survived cycle: `RainWorldGame.Win`'s save side, `SaveState.SessionEnded(survived:
//! true)` with `RainCycleTick` and `BringUpToDate`, and the save that follows
//! (`SaveWorldStateAndProgression`). Every DLC off.

use rw_assets::SlugcatName;
use rw_random::UnityRandom;

use crate::communities::{CommunityId, CreatureCommunities};
use crate::dreams::{DreamContext, DreamId};
use crate::misc::MiscProgression;
use crate::progression::{PlayerProgression, SaveError};
use crate::save_state::SaveState;
use crate::session::{HibernationStats, HibernationWorld, PlayerSessionRecord};
use crate::win_state::CycleCompletion;

/// What `Win` leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinOutcome {
    /// `ProcessID.Dream` with this dream (only when not malnourished).
    Dream(DreamId),
    /// `ProcessID.SleepScreen`.
    SleepScreen,
}

impl SaveState {
    /// `RainCycleTick(game, depleteSwarmRoom)`'s save-state part after the swarm room:
    /// the communities' tick, flower depletion, the waiting respawns' rolls (one
    /// `Random.value` each from the last, a second for the Hunter) and the cycle.
    fn rain_cycle_tick(&mut self, communities: &mut CreatureCommunities, rng: &mut UnityRandom) {
        communities.cycle_tick(self.cycle_number, self.save_state_number);
        self.death_persistent.tick_flower_depletion(1);
        for i in (0..self.wait_respawn_creatures.len()).rev() {
            let mut respawn = rng.value() < 1.0 / 3.0;
            if self.save_state_number == SlugcatName::Red {
                respawn = rng.value() < 0.5;
            }
            if respawn {
                let id = self.wait_respawn_creatures.remove(i);
                self.respawn_creatures.push(id);
            }
        }
        self.cycle_number += 1;
    }

    /// `TrySetVanillaDen`.
    fn try_set_vanilla_den(&mut self, room: &str, world: &dyn HibernationWorld) {
        if world.vanilla_room_exists(room) {
            room.clone_into(&mut self.last_vanilla_den);
        }
    }

    /// `SessionEnded(game, survived: true, newMalnourished)` up to the save it ends
    /// with (`SaveWorldStateAndProgression`, done by the progression). Not ported:
    /// kills (`AppendKills`), friends in the den, the spinning top's branches, and the
    /// grasps and stomach `BringUpToDate` rebuilds (the saved ones are kept).
    pub fn session_ended_survived(
        &mut self,
        world: &mut dyn HibernationWorld,
        record: &mut PlayerSessionRecord,
        stats: HibernationStats,
        new_malnourished: bool,
        rng: &mut UnityRandom,
        misc: &mut MiscProgression,
    ) {
        let region = world.region_name();
        let regions = world.region_names();
        let mut communities = CreatureCommunities::for_session(
            self.communities.as_deref(),
            &regions,
            &world.default_alignments(),
            self.save_state_number,
        );
        let difficulty = self.session_difficulty();

        self.last_malnourished = self.malnourished;
        self.malnourished = new_malnourished;
        let travelled = record.woke_up_in_region.as_deref() != Some(region.as_str());
        self.death_persistent.add_session_record(true, travelled);
        self.death_persistent.age_death_positions();
        self.death_persistent.food_replenish_bonus = 0;
        world.deplete_one_swarm_room(rng);
        self.rain_cycle_tick(&mut communities, rng);
        self.cycles_in_current_world_version += 1;

        let food = world
            .food_in_room_eat_and_destroy()
            .clamp(0, stats.max_food);
        self.food = if self.malnourished || !self.last_malnourished {
            food - stats.food_to_hibernate
        } else {
            0
        };
        self.skip_next_cycle_food_drain = false;

        // `BringUpToDate`.
        let den = world.den_room();
        self.den_position.clone_from(&den);
        self.try_set_vanilla_den(&den, world);
        self.next_issued_id = world.next_issued_id();
        let state = world.adapt_region_state_to_world(&mut self.respawn_creatures);
        self.set_region_state(&region, state, &regions);
        self.communities = Some(communities.save_string());

        record.went_to_sleep_in_region = Some(region);
        self.tot_time += record.time / 40;
        record.player_grabbed_time = 0;
        record.time = 0;
        self.death_persistent.survives += 1;
        let cycle = CycleCompletion {
            slugcat: self.save_state_number,
            karma: self.death_persistent.karma,
            karma_cap: self.death_persistent.karma_cap,
            record,
            scavenger_like: communities
                .like_of_player_everywhere(CommunityId::Scavengers, difficulty),
        };
        if let Some(menu_region) = self.death_persistent.win_state.cycle_completed(&cycle) {
            misc.set_menu_region(&menu_region);
        }
        self.death_persistent.karma += 1;
        if self.malnourished {
            self.death_persistent.reinforced_karma = false;
        }
    }
}

impl PlayerProgression {
    /// `RainWorldGame.Win(malnourished, fromWarpPoint: false)`'s save side with every
    /// DLC off: the once-per-run backup (`backed_up` is `rainWorld.saveBackedUp`), the
    /// starvation tutorial counter, the dreams' end of cycle, `SessionEnded` and
    /// `SaveWorldStateAndProgression`. The Moon and Pebbles event dreams are not
    /// ported (no iterators yet).
    ///
    /// # Errors
    /// When the save file cannot be written.
    #[allow(clippy::too_many_arguments)]
    pub fn win(
        &mut self,
        world: &mut dyn HibernationWorld,
        record: &mut PlayerSessionRecord,
        stats: HibernationStats,
        malnourished: bool,
        backed_up: &mut bool,
        rng: &mut UnityRandom,
    ) -> Result<WinOutcome, SaveError> {
        if !malnourished && !*backed_up {
            *backed_up = true;
            self.back_up_save("_Backup")?;
        }
        if self.misc.starvation_tutorial_counter() > -1 {
            let n = self.misc.starvation_tutorial_counter() + 1;
            self.misc.set_starvation_tutorial_counter(n);
        }
        let Some(state) = self.current_save_state.as_mut() else {
            log::warn!("Win without a save state");
            return Ok(WinOutcome::SleepScreen);
        };
        let region = world.region_name();
        let den = world.den_room();
        let likes = state.misc_world.player_guide.likes_player;
        let cycle_number = state.cycle_number;
        if let Some(dreams) = &mut state.dreams {
            dreams.end_of_cycle_progress(&DreamContext {
                malnourished,
                cycle_number,
                guide_likes_player: likes,
                region: &region,
                den: &den,
            });
        }
        state.session_ended_survived(world, record, stats, malnourished, rng, &mut self.misc);
        let dream = state.dreams.as_ref().and_then(|d| d.upcoming_dream);
        self.save_world_state_and_progression(malnourished)?;
        Ok(match dream {
            Some(d) if !malnourished => WinOutcome::Dream(d),
            _ => WinOutcome::SleepScreen,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::ChecksumSalt;
    use crate::progression::CampaignStart;

    struct World {
        food: i32,
        depletes: u32,
    }

    impl HibernationWorld for World {
        fn deplete_one_swarm_room(&mut self, rng: &mut UnityRandom) {
            self.depletes += 1;
            rng.range_i32(0, 1);
        }
        fn food_in_room_eat_and_destroy(&mut self) -> i32 {
            self.food
        }
        fn den_room(&self) -> String {
            "SU_S01".to_owned()
        }
        fn next_issued_id(&self) -> i32 {
            1234
        }
        fn region_name(&self) -> String {
            "SU".to_owned()
        }
        fn adapt_region_state_to_world(&mut self, respawn: &mut Vec<i32>) -> String {
            respawn.push(5003);
            "REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>0<rgA>".to_owned()
        }
        fn region_names(&self) -> Vec<String> {
            ["CC", "SU"].map(str::to_owned).to_vec()
        }
        fn default_alignments(&self) -> String {
            "Lizards : SU=-3\n".to_owned()
        }
        fn vanilla_room_exists(&self, _room: &str) -> bool {
            true
        }
    }

    const SURVIVOR: HibernationStats = HibernationStats {
        max_food: 7,
        food_to_hibernate: 4,
    };

    fn new_state() -> SaveState {
        let mut s = SaveState::new(SlugcatName::White, 42);
        s.load_game("", "SU_C04", true).unwrap();
        s.world_version = 2;
        s
    }

    fn end(s: &mut SaveState, food: i32, malnourished: bool) -> PlayerSessionRecord {
        let mut world = World { food, depletes: 0 };
        let mut record = PlayerSessionRecord {
            woke_up_in_region: Some("SU".to_owned()),
            time: 4000,
            ..PlayerSessionRecord::default()
        };
        let mut misc = MiscProgression::default();
        let mut rng = UnityRandom::new(1);
        let stats = if s.malnourished {
            HibernationStats {
                food_to_hibernate: 7,
                ..SURVIVOR
            }
        } else {
            SURVIVOR
        };
        s.session_ended_survived(
            &mut world,
            &mut record,
            stats,
            malnourished,
            &mut rng,
            &mut misc,
        );
        assert_eq!(world.depletes, 1);
        record
    }

    #[test]
    fn first_hibernation_of_a_new_survivor() {
        let mut s = new_state();
        let record = end(&mut s, 5, false);
        assert_eq!(
            (s.food, s.cycle_number, s.death_persistent.karma),
            (1, 1, 1)
        );
        assert_eq!((s.death_persistent.survives, s.tot_time), (1, 100));
        assert_eq!(s.den_position, "SU_S01");
        assert_eq!(s.last_vanilla_den, "SU_S01");
        assert_eq!(record.went_to_sleep_in_region.as_deref(), Some("SU"));
        // SU's lizards start at -0.03 and the first tick brings them to 0.
        assert_eq!(
            s.save_to_string(),
            "SAV STATE NUMBER<svB>White<svA>TIMELINE<svB>White<svA>SEED<svB>42<svA>VERSION<svB>2<svA>\
             INITVERSION<svB>0<svA>WORLDVERSION<svB>2<svA>DENPOS<svB>SU_S01<svA>LASTVDENPOS<svB>SU_S01<svA>\
             CYCLENUM<svB>1<svA>FOOD<svB>1<svA>NEXTID<svB>1234<svA>RESPAWNS<svB>5003<svA>\
             COMMUNITIES<svB>SCAVSHY<coB>0<coA>All<coB>EVERY:0|CC:0|SU:0<coA>Scavengers<coB>EVERY:0|CC:0|SU:0\
             <coA>Lizards<coB>EVERY:0|CC:0|SU:0<coA>Cicadas<coB>EVERY:0|CC:0|SU:0<coA>GarbageWorms<coB>\
             EVERY:0|CC:0|SU:0<coA>Deer<coB>EVERY:0|CC:0|SU:0<coA>JetFish<coB>EVERY:0|CC:0|SU:0<svA>\
             REGIONSTATE<svB>REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>0<rgA><svA>\
             DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>1<dpA>KARMACAP<dpB>4<dpA>\
             GHOSTS<dpB><dpA>SESSIONRECORDS<dpB>10<dpA>TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB><dpA>\
             DEATHS<dpB>0<dpA>SURVIVES<dpB>1<dpA>QUITS<dpB>0<dpA><svA>\
             MISCWORLDSAVEDATA<svB>playerGuideState<mwB>integersArray<pgsB>0.0.0.0.0.0.0<pgsA>itemTypes<pgsB><pgsA>\
             creatureTypes<pgsB><pgsA>likesPlayer<pgsB>0.4<pgsA>directionHandHolding<pgsB>0.825<pgsA><mwA><svA>\
             DREAMSSTATE<svB>integersArray<dsB>0.0.0.0.0.0.0.0.0<dsA><svA>TOTFOOD<svB>0<svA>TOTTIME<svB>100<svA>\
             CURRVERCYCLES<svB>1<svA>"
        );
    }

    #[test]
    fn malnourished_food_and_karma() {
        // Starving: the meter's food is clamped to 7, less the malnourished cost.
        let mut s = new_state();
        s.death_persistent.reinforced_karma = true;
        end(&mut s, 9, true);
        assert_eq!(
            (s.food, s.malnourished, s.last_malnourished),
            (3, true, false)
        );
        assert!(!s.death_persistent.reinforced_karma);
        // The cycle after a starved one wakes with nothing, whatever was eaten.
        end(&mut s, 6, false);
        assert_eq!(
            (s.food, s.malnourished, s.last_malnourished),
            (0, false, true)
        );
        assert_eq!(s.death_persistent.karma, 2);
        assert_eq!(
            s.death_persistent.session_records.as_deref(),
            Some("10<dpC>10")
        );
    }

    #[test]
    fn waiting_respawns_draw_once_each_and_twice_for_the_hunter() {
        let mut s = new_state();
        s.wait_respawn_creatures = vec![1, 2, 3];
        let mut rng = UnityRandom::new(7);
        let mut c = CreatureCommunities::new(&[]);
        s.rain_cycle_tick(&mut c, &mut rng);
        let mut expect = UnityRandom::new(7);
        for _ in 0..3 {
            expect.value();
        }
        assert_eq!(rng.value(), expect.value());
        let mut red = SaveState::new(SlugcatName::Red, 0);
        red.wait_respawn_creatures = vec![1, 2, 3];
        let mut rng = UnityRandom::new(7);
        red.rain_cycle_tick(&mut c, &mut rng);
        let mut expect = UnityRandom::new(7);
        let mut back = Vec::new();
        for id in [3, 2, 1] {
            expect.value();
            if expect.value() < 0.5 {
                back.push(id);
            }
        }
        assert_eq!(rng.value(), expect.value());
        assert_eq!(red.respawn_creatures, back);
        assert_eq!(red.cycle_number, 1);
    }

    #[test]
    fn win_saves_the_whole_state_and_keeps_other_lines() {
        let dir = tempfile::tempdir().unwrap();
        let salt = ChecksumSalt::new("salt");
        let path = dir.path().join("sav");
        let mut p = PlayerProgression::load(&path, Some(salt.clone())).unwrap();
        let start = CampaignStart {
            slugcat: SlugcatName::White,
            den: "SU_C04",
            vanilla: true,
        };
        p.get_or_initiate_save_state(start, 42, false, true)
            .unwrap();
        p.temp_discover_shelter("SU_S01");
        let mut world = World {
            food: 4,
            depletes: 0,
        };
        let mut record = PlayerSessionRecord {
            woke_up_in_region: Some("SU".to_owned()),
            ..PlayerSessionRecord::default()
        };
        let mut backed_up = false;
        let mut rng = UnityRandom::new(3);
        let out = p
            .win(
                &mut world,
                &mut record,
                SURVIVOR,
                false,
                &mut backed_up,
                &mut rng,
            )
            .unwrap();
        assert_eq!(out, WinOutcome::SleepScreen);
        assert!(backed_up);
        let mut again = PlayerProgression::load(&path, Some(salt)).unwrap();
        assert!(!again.game_tinkered_with);
        let lines = again.prog_lines();
        assert!(lines[0].starts_with("SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA>"));
        assert!(lines[0].contains("DENPOS<svB>SU_S01<svA>"));
        assert!(lines[0].contains("FOOD<svB>0<svA>"));
        let misc = lines.iter().find(|l| l.starts_with("MISCPROG")).unwrap();
        assert!(misc.contains("SHELTERLIST<mpdB>SU<mpdB>SU_S01<mpdA>"));
        assert!(misc.contains("CONDITIONALSHELTERDATA<mpdB>SU_S01 : White : <mpdC><mpdA>"));
        assert!(misc.contains("INTEGERS<mpdB>0,0,0,0,0,1,0,0,0,0,0<mpdA>MENUREGION<mpdB>SU<mpdA>"));
        let state = again.peek_save_state(start).unwrap().unwrap();
        assert_eq!(
            (state.cycle_number, state.food, state.death_persistent.karma),
            (1, 0, 1)
        );
        assert_eq!(
            state.region_state("SU"),
            Some("REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>0<rgA>")
        );
    }
}
