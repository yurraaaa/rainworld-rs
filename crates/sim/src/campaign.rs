//! What a campaign is, per slugcat: the data the game spreads over `SlugcatStats`
//! (food meter, stats, timeline), `SaveState` (new-game karma and mark, start den via
//! `GetStoryDenPosition`, the final fallback shelter) and the story start positions of
//! `RainWorldGame.TryGetPlayerStartPos`.
//!
//! Adding a slugcat is adding a [`SlugcatCampaign`] to [`SlugcatCampaign::all`] (gated by
//! its mod) and its timeline/name to `rw_assets::{SlugcatName, Timeline}`. Design:
//! `rainworld-re/notes/campaign-model.md`.

use rw_assets::{ActiveMods, SlugcatName, Timeline};

/// `SlugcatStats` (not malnourished unless built so).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlugcatStats {
    pub max_food: i32,
    pub food_to_hibernate: i32,
    pub runspeed_fac: f32,
    pub body_weight_fac: f32,
    pub general_visibility_bonus: f32,
    pub visual_stealth_in_sneak_mode: f32,
    pub loudness_fac: f32,
    pub lungs_fac: f32,
    pub throwing_skill: i32,
    pub pole_climb_speed_fac: f32,
    pub corridor_climb_speed_fac: f32,
    pub malnourished: bool,
}

impl SlugcatStats {
    /// The constructor's defaults (the Survivor's but for its throwing skill).
    const BASE: Self = Self {
        max_food: 7,
        food_to_hibernate: 4,
        runspeed_fac: 1.0,
        body_weight_fac: 1.0,
        general_visibility_bonus: 0.0,
        visual_stealth_in_sneak_mode: 0.5,
        loudness_fac: 1.0,
        lungs_fac: 1.0,
        throwing_skill: 0,
        pole_climb_speed_fac: 1.0,
        corridor_climb_speed_fac: 1.0,
        malnourished: false,
    };

    /// These stats when malnourished: hibernating needs a full meter.
    #[must_use]
    pub fn malnourished(self) -> Self {
        Self {
            food_to_hibernate: self.max_food,
            malnourished: true,
            ..self
        }
    }
}

/// One playable campaign, looked up by slugcat name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlugcatCampaign {
    pub name: SlugcatName,
    /// `SlugcatToTimeline`: filters world lines, settings and placed objects.
    pub timeline: Timeline,
    pub stats: SlugcatStats,
    /// `GetStoryDenPosition`: where a new game starts.
    pub start_den: &'static str,
    /// Whether `start_den` is also `lastVanillaDen` (`isVanilla`).
    pub start_den_is_vanilla: bool,
    /// `GetFinalFallbackShelter`.
    pub fallback_shelter: &'static str,
    /// New-game `deathPersistentSaveData.karma`, `karmaCap` and `theMark`.
    pub start_karma: i32,
    pub start_karma_cap: i32,
    pub start_with_mark: bool,
    /// Whether the campaign has a `DreamsState`.
    pub dreams: bool,
}

impl SlugcatCampaign {
    pub const SURVIVOR: Self = Self {
        name: SlugcatName::WHITE,
        timeline: Timeline::WHITE,
        stats: SlugcatStats {
            throwing_skill: 1,
            ..SlugcatStats::BASE
        },
        start_den: "SU_C04",
        start_den_is_vanilla: true,
        fallback_shelter: "SU_S01",
        start_karma: 0,
        start_karma_cap: 4,
        start_with_mark: false,
        dreams: true,
    };

    pub const MONK: Self = Self {
        name: SlugcatName::YELLOW,
        timeline: Timeline::YELLOW,
        stats: SlugcatStats {
            max_food: 5,
            food_to_hibernate: 3,
            body_weight_fac: 0.95,
            general_visibility_bonus: -0.1,
            visual_stealth_in_sneak_mode: 0.6,
            loudness_fac: 0.75,
            // Remix's `cfgMonkBreathTime` makes it 0.8.
            lungs_fac: 1.2,
            throwing_skill: 0,
            ..SlugcatStats::BASE
        },
        start_den: "SU_C04",
        start_den_is_vanilla: true,
        fallback_shelter: "SU_S01",
        start_karma: 0,
        start_karma_cap: 4,
        start_with_mark: false,
        dreams: true,
    };

    pub const HUNTER: Self = Self {
        name: SlugcatName::RED,
        timeline: Timeline::RED,
        stats: SlugcatStats {
            max_food: 9,
            food_to_hibernate: 6,
            runspeed_fac: 1.2,
            body_weight_fac: 1.12,
            general_visibility_bonus: 0.1,
            visual_stealth_in_sneak_mode: 0.3,
            loudness_fac: 1.35,
            throwing_skill: 2,
            pole_climb_speed_fac: 1.25,
            corridor_climb_speed_fac: 1.2,
            ..SlugcatStats::BASE
        },
        start_den: "LF_H01",
        start_den_is_vanilla: true,
        fallback_shelter: "LF_S02",
        start_karma: 2,
        start_karma_cap: 4,
        start_with_mark: true,
        dreams: false,
    };

    /// The campaigns available with `mods` (More Slugcats and the Watcher add theirs).
    #[must_use]
    pub fn all(_mods: ActiveMods) -> Vec<&'static Self> {
        vec![&Self::SURVIVOR, &Self::MONK, &Self::HUNTER]
    }

    /// The campaign of `name`, if it is playable with `mods`.
    #[must_use]
    pub fn of(name: SlugcatName, mods: ActiveMods) -> Option<&'static Self> {
        Self::all(mods).into_iter().find(|c| c.name == name)
    }

    /// The region acronym of the start den (`OverWorld.LoadFirstWorld`: text before `_`).
    #[must_use]
    pub fn start_region(&self) -> &'static str {
        self.start_den.split('_').next().unwrap_or(self.start_den)
    }
}

/// `RainWorldGame.TryGetPlayerStartPos`: the tile a story player starts on in these
/// rooms (story starts, intros and the dev rooms), instead of the default (15, 25).
/// Remix's `cfgVanillaExploits` removes the `LF_H01` entry.
#[must_use]
pub fn player_start_pos(room: &str, _mods: ActiveMods) -> Option<(i32, i32)> {
    Some(match room {
        "DS_B02" => (12, 25),
        "SH_D01" => (15, 10),
        "GW_B01" => (10, 19),
        "SB_H03" => (13, 70),
        "SS_A08" => (8, 13),
        "SU_A22" => (22, 12),
        "SI_B13" => (36, 49),
        "SL_A08" => (8, 13),
        "SB_F03" => (43, 208),
        "SU_C04" => (7, 28),
        "LF_H01" => (350, 13),
        _ => return None,
    })
}

/// `SpawnPlayers`' default tile when a room has no start position.
pub const DEFAULT_PLAYER_TILE: (i32, i32) = (15, 25);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_campaigns() {
        let mods = ActiveMods::NONE;
        let names: Vec<_> = SlugcatCampaign::all(mods).iter().map(|c| c.name).collect();
        assert_eq!(
            names,
            [SlugcatName::WHITE, SlugcatName::YELLOW, SlugcatName::RED]
        );
        let monk = SlugcatCampaign::of(SlugcatName::YELLOW, mods).unwrap();
        assert_eq!((monk.stats.max_food, monk.stats.food_to_hibernate), (5, 3));
        assert_eq!(monk.start_den, "SU_C04");
        let hunter = SlugcatCampaign::of(SlugcatName::RED, mods).unwrap();
        assert_eq!(hunter.start_region(), "LF");
        assert_eq!(hunter.stats.malnourished().food_to_hibernate, 9);
        assert!(SlugcatCampaign::of(SlugcatName::NIGHT, mods).is_none());
        for c in SlugcatCampaign::all(mods) {
            assert_eq!(c.name.timeline(), c.timeline);
        }
        assert_eq!(player_start_pos("SU_C04", mods), Some((7, 28)));
        assert_eq!(player_start_pos("LF_H01", mods), Some((350, 13)));
        assert_eq!(player_start_pos("SU_A41", mods), None);
    }
}
