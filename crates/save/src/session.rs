//! A cycle's end as the save sees it: `PlayerSessionRecord`, the world queries
//! `SaveState.SessionEnded` makes ([`HibernationWorld`]) and the slugcat's food stats.
//!
//! Mirrors: PlayerSessionRecord.cs, SaveState.cs (SessionEnded)

use rw_assets::CreatureType;
use rw_random::UnityRandom;

/// `PlayerSessionRecord.KillRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillRecord {
    /// `CreatureTemplate.Type` value.
    pub creature: String,
    /// The kill symbol's `intData`.
    pub int_data: i32,
    /// The template's `countsAsAKill`.
    pub counts_as_a_kill: i32,
    /// `Template.IsLizard`; with `lizard_index` its place in `WinState.lizardsOrder`.
    pub lizard: bool,
    pub lizard_index: Option<usize>,
}

impl KillRecord {
    /// `WinState.lizardsOrder`.
    const LIZARDS_ORDER: [CreatureType; 6] = [
        CreatureType::GreenLizard,
        CreatureType::PinkLizard,
        CreatureType::BlueLizard,
        CreatureType::WhiteLizard,
        CreatureType::YellowLizard,
        CreatureType::BlackLizard,
    ];

    /// A kill of `creature` with its symbol's `intData`, its template's
    /// `countsAsAKill` and `IsLizard`.
    #[must_use]
    pub fn new(creature: CreatureType, int_data: i32, counts_as_a_kill: i32, lizard: bool) -> Self {
        Self {
            creature: creature.value().to_owned(),
            int_data,
            counts_as_a_kill,
            lizard,
            lizard_index: Self::LIZARDS_ORDER.iter().position(|&l| l == creature),
        }
    }

    /// The kill's `IconSymbolData.ToString()`: type, item type and `intData`.
    #[must_use]
    pub fn symbol(&self) -> String {
        format!("{}-Creature-{}", self.creature, self.int_data)
    }
}

/// What `PlayerSessionRecord.AddEat` was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EatenKind {
    /// A creature, a jellyfish or an egg bug egg: not vegetarian.
    Meat,
    /// Anything else edible: not carnivorous.
    Plant,
    /// A karma flower or a mushroom: neither.
    Neither,
}

/// `PlayerSessionRecord` of player 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSessionRecord {
    pub kills: Vec<KillRecord>,
    /// `eats`' entity IDs (spawner, number), each counted once.
    pub eaten: Vec<(i32, i32)>,
    pub ate_anything: bool,
    pub vegetarian: bool,
    pub peaceful: bool,
    pub carnivorous: bool,
    pub woke_up_in_region: Option<String>,
    pub went_to_sleep_in_region: Option<String>,
    /// `DataPearlType` indices.
    pub pearls_found: Vec<i32>,
    /// Ticks.
    pub time: i32,
    pub player_grabbed_time: i32,
}

impl Default for PlayerSessionRecord {
    fn default() -> Self {
        Self {
            kills: Vec::new(),
            eaten: Vec::new(),
            ate_anything: false,
            vegetarian: true,
            peaceful: true,
            carnivorous: true,
            woke_up_in_region: None,
            went_to_sleep_in_region: None,
            pearls_found: Vec::new(),
            time: 0,
            player_grabbed_time: 0,
        }
    }
}

impl PlayerSessionRecord {
    /// `AddEat`: an object of entity `id` was eaten (once per ID).
    pub fn add_eat(&mut self, id: (i32, i32), kind: EatenKind) {
        if self.eaten.contains(&id) {
            return;
        }
        self.eaten.push(id);
        match kind {
            EatenKind::Meat => {
                self.ate_anything = true;
                self.vegetarian = false;
            }
            EatenKind::Plant => {
                self.ate_anything = true;
                self.carnivorous = false;
            }
            EatenKind::Neither => {}
        }
    }
}

/// `SlugcatStats` of the session (`characterStats`, malnourished if the save state
/// was at the start).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HibernationStats {
    pub max_food: i32,
    pub food_to_hibernate: i32,
}

/// What `SessionEnded` and `BringUpToDate` ask of the running game.
pub trait HibernationWorld {
    /// `DepleteOneSwarmRoom`'s draws and change on the world's region state (the only
    /// region state the port loads): `Random.Range(0, 1)` for the region, then, with
    /// candidates, one for the candidate.
    fn deplete_one_swarm_room(&mut self, rng: &mut UnityRandom);
    /// The first player's `FoodInRoom(eatAndDestroy: true)`.
    fn food_in_room_eat_and_destroy(&mut self) -> i32;
    /// The first player's room (`denPosition`).
    fn den_room(&self) -> String;
    /// `game.nextIssuedId`.
    fn next_issued_id(&self) -> i32;
    /// `world.region.name`.
    fn region_name(&self) -> String;
    /// `regionState.AdaptRegionStateToWorld(playerShelter, -1)`; returns
    /// `RegionState.SaveToString()`. Dead loaded creatures' spawners on neither list go
    /// on `respawn`.
    fn adapt_region_state_to_world(&mut self, respawn: &mut Vec<i32>, wait: &[i32]) -> String;
    /// `progression.regionNames` (`World/regions.txt` order).
    fn region_names(&self) -> Vec<String>;
    /// `World/default alignments.txt`, for a save without `COMMUNITIES`.
    fn default_alignments(&self) -> String;
    /// `TrySetVanillaDen`'s check: the room's file is in its region's rooms or the gate
    /// shelters.
    fn vanilla_room_exists(&self, room: &str) -> bool;
    /// The save strings of what the first player holds, hand 0 first (`playerGrasps`).
    fn player_grasps(&self) -> Vec<String>;
    /// The save string of the first player's `objectInStomach`, if any.
    fn swallowed_item(&self) -> Option<String>;
    /// The session's `InfluenceLikeOfPlayer` calls, in order (all in its region).
    fn community_influences(&self) -> Vec<crate::communities::CommunityInfluence> {
        Vec::new()
    }
}
