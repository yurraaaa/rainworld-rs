//! The game's extensible identities (`ExtEnum`s) that data files name: slugcats
//! (`SlugcatStats.Name`) and campaign timelines (`SlugcatStats.Timeline`), and the active
//! mods (`ModManager`) that decide which are registered.
//!
//! An `ExtEnum` value is a string with a registration index; DLCs register more values
//! when enabled. Here each value is a name constant, and what is registered depends on
//! [`ActiveMods`]. Design: `rainworld-re/notes/campaign-model.md`.

use std::fmt;

/// `ModManager`'s flags. The port runs the base game with all of them off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActiveMods {
    /// More Slugcats (`ModManager.MSC`).
    pub msc: bool,
    /// The Watcher (`ModManager.Watcher`).
    pub watcher: bool,
    /// Remix (`ModManager.MMF`).
    pub mmf: bool,
    pub jolly_coop: bool,
    pub expedition: bool,
}

impl ActiveMods {
    /// The base game.
    pub const NONE: Self = Self {
        msc: false,
        watcher: false,
        mmf: false,
        jolly_coop: false,
        expedition: false,
    };
}

/// `SlugcatStats.Name`: a slugcat, by its `ExtEnum` value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlugcatName(&'static str);

impl SlugcatName {
    /// The Survivor.
    pub const WHITE: Self = Self("White");
    /// The Monk.
    pub const YELLOW: Self = Self("Yellow");
    /// The Hunter.
    pub const RED: Self = Self("Red");
    /// Arena player 4's colour; hidden, never a campaign.
    pub const NIGHT: Self = Self("Night");

    /// Values registered by the base game, in registration order (their `Index`).
    const BASE: [Self; 4] = [Self::WHITE, Self::YELLOW, Self::RED, Self::NIGHT];

    /// `ExtEnum<Name>.values` with `mods` active. More Slugcats and the Watcher append
    /// their names when they are supported.
    #[must_use]
    pub fn registered(_mods: ActiveMods) -> Vec<Self> {
        Self::BASE.to_vec()
    }

    /// The registered value whose string is `value`.
    #[must_use]
    pub fn from_value(value: &str, mods: ActiveMods) -> Option<Self> {
        Self::registered(mods).into_iter().find(|n| n.0 == value)
    }

    /// `BackwardsCompatibilityRemix.ParsePlayerNumber`: 0 to 3 are the base names, higher
    /// numbers the registered entry at that index.
    #[must_use]
    pub fn from_player_number(number: i32, mods: ActiveMods) -> Option<Self> {
        let index = usize::try_from(number).ok()?;
        Self::registered(mods).get(index).copied()
    }

    #[must_use]
    pub fn value(self) -> &'static str {
        self.0
    }

    /// `SlugcatStats.HiddenOrUnplayableSlugcat` (DLC names join with their mods).
    #[must_use]
    pub fn hidden_or_unplayable(self) -> bool {
        self == Self::NIGHT
    }

    /// `SlugcatStats.SlugcatToTimeline`: each base campaign is its own timeline; every
    /// other name (Night) falls back to the Survivor's.
    #[must_use]
    pub fn timeline(self) -> Timeline {
        match self {
            Self::RED => Timeline::RED,
            Self::YELLOW => Timeline::YELLOW,
            _ => Timeline::WHITE,
        }
    }
}

impl fmt::Debug for SlugcatName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Display for SlugcatName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// `SlugcatStats.Timeline`: the point in the world's history a campaign plays at. World
/// files filter lines by it (`(White)`, `(X-Red)`). Unlike the names, every value is
/// registered by the base game, DLC timelines included.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timeline(&'static str);

impl Timeline {
    pub const SPEAR: Self = Self("Spear");
    pub const ARTIFICER: Self = Self("Artificer");
    /// Inv's (`Sofanthiel`) timeline.
    pub const SOFANTHIEL: Self = Self("Inv");
    pub const RED: Self = Self("Red");
    pub const GOURMAND: Self = Self("Gourmand");
    pub const WHITE: Self = Self("White");
    pub const YELLOW: Self = Self("Yellow");
    pub const RIVULET: Self = Self("Rivulet");
    pub const SAINT: Self = Self("Saint");
    pub const WATCHER: Self = Self("Watcher");

    /// Registration order (`Index`).
    pub const ALL: [Self; 10] = [
        Self::SPEAR,
        Self::ARTIFICER,
        Self::SOFANTHIEL,
        Self::RED,
        Self::GOURMAND,
        Self::WHITE,
        Self::YELLOW,
        Self::RIVULET,
        Self::SAINT,
        Self::WATCHER,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        self.0
    }

    /// The registered timeline whose string is `value`.
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.0 == value)
    }
}

impl fmt::Debug for Timeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Display for Timeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_names_and_timelines() {
        assert_eq!(
            SlugcatName::registered(ActiveMods::NONE),
            [
                SlugcatName::WHITE,
                SlugcatName::YELLOW,
                SlugcatName::RED,
                SlugcatName::NIGHT
            ]
        );
        assert_eq!(SlugcatName::NIGHT.timeline(), Timeline::WHITE);
        assert_eq!(SlugcatName::RED.timeline(), Timeline::RED);
        assert_eq!(Timeline::from_value("Inv"), Some(Timeline::SOFANTHIEL));
        assert_eq!(
            SlugcatName::from_player_number(2, ActiveMods::NONE),
            Some(SlugcatName::RED)
        );
        assert_eq!(SlugcatName::from_player_number(4, ActiveMods::NONE), None);
    }
}
