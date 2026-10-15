//! The game's extensible identities (`ExtEnum`s) that data files name: slugcats
//! (`SlugcatStats.Name`) and campaign timelines (`SlugcatStats.Timeline`), and the active
//! mods (`ModManager`) that decide which are registered.
//!
//! An `ExtEnum` value is a string with a registration index; DLCs register more values
//! when enabled. Here each is a closed Rust enum with a variant for every base and DLC
//! value; what is registered (and so the index) depends on [`ActiveMods`]. Design:
//! `rainworld-re/notes/campaign-model.md`.

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

/// Which `RegisterAllEnumExtensions` registers an `ExtEnum` value. `RainWorld.OnModsInit`
/// runs them in this order, after the base game's static values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Registrar {
    Base,
    /// `DLCSharedEnums` (`ModManager.DLCShared`: More Slugcats or the Watcher).
    DlcShared,
    Msc,
    Jolly,
    Watcher,
}

impl Registrar {
    pub(crate) fn active(self, mods: ActiveMods) -> bool {
        match self {
            Self::Base => true,
            Self::DlcShared => mods.msc || mods.watcher,
            Self::Msc => mods.msc,
            Self::Jolly => mods.jolly_coop,
            Self::Watcher => mods.watcher,
        }
    }
}

/// An `ExtEnum` as a closed enum: every value the game and its DLCs declare, grouped by
/// the [`Registrar`] that registers it, in registration order. Generates `ALL`, `value`
/// (the game's spelling), `registered`, and `Debug`/`Display` as the spelling.
macro_rules! ext_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($registrar:ident {
                $($(#[$vmeta:meta])* $variant:ident = $value:literal,)*
            })*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($($(#[$vmeta])* $variant,)*)*
        }

        impl $name {
            /// Every declared value, in registration order.
            pub const ALL: &'static [Self] = &[$($(Self::$variant,)*)*];

            #[must_use]
            pub fn value(self) -> &'static str {
                match self {
                    $($(Self::$variant => $value,)*)*
                }
            }

            fn registrar(self) -> $crate::identity::Registrar {
                match self {
                    $($(Self::$variant => $crate::identity::Registrar::$registrar,)*)*
                }
            }

            /// `ExtEnum<T>.values` with `mods` active, in registration order (their
            /// `Index`).
            #[must_use]
            pub fn registered(mods: $crate::ActiveMods) -> Vec<Self> {
                Self::ALL
                    .iter()
                    .copied()
                    .filter(|v| v.registrar().active(mods))
                    .collect()
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.value())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.value())
            }
        }
    };
}
pub(crate) use ext_enum;

ext_enum! {
    /// `SlugcatStats.Name`: a slugcat, by its `ExtEnum` value.
    pub enum SlugcatName {
        Base {
            /// The Survivor.
            White = "White",
            /// The Monk.
            Yellow = "Yellow",
            /// The Hunter.
            Red = "Red",
            /// Arena player 4's colour; hidden, never a campaign.
            Night = "Night",
        }
        Msc {
            Rivulet = "Rivulet",
            Artificer = "Artificer",
            Saint = "Saint",
            /// The Spearmaster.
            Spear = "Spear",
            Gourmand = "Gourmand",
            Slugpup = "Slugpup",
            /// Inv.
            Sofanthiel = "Inv",
        }
        Jolly {
            JollyPlayer1 = "JollyPlayer1",
            JollyPlayer2 = "JollyPlayer2",
            JollyPlayer3 = "JollyPlayer3",
            JollyPlayer4 = "JollyPlayer4",
        }
        Watcher {
            Watcher = "Watcher",
        }
    }
}

impl SlugcatName {
    /// The registered value whose string is `value`.
    #[must_use]
    pub fn from_value(value: &str, mods: ActiveMods) -> Option<Self> {
        Self::registered(mods)
            .into_iter()
            .find(|n| n.value() == value)
    }

    /// `BackwardsCompatibilityRemix.ParsePlayerNumber`: 0 to 3 are the base names, higher
    /// numbers the registered entry at that index.
    #[must_use]
    pub fn from_player_number(number: i32, mods: ActiveMods) -> Option<Self> {
        let index = usize::try_from(number).ok()?;
        Self::registered(mods).get(index).copied()
    }

    /// `SlugcatStats.HiddenOrUnplayableSlugcat`. A DLC name is only registered while its
    /// mod is on, which stands in for the game's `ModManager` checks.
    #[must_use]
    pub fn hidden_or_unplayable(self) -> bool {
        matches!(
            self,
            Self::Night
                | Self::Sofanthiel
                | Self::Slugpup
                | Self::JollyPlayer1
                | Self::JollyPlayer2
                | Self::JollyPlayer3
                | Self::JollyPlayer4
        )
    }

    /// `SlugcatStats.SlugcatToTimeline`: each campaign is its own timeline; every other
    /// name (Night, Slugpup, the Jolly players) falls back to the Survivor's.
    #[must_use]
    pub fn timeline(self) -> Timeline {
        match self {
            Self::Red => Timeline::Red,
            Self::White => Timeline::White,
            Self::Yellow => Timeline::Yellow,
            Self::Spear => Timeline::Spear,
            Self::Artificer => Timeline::Artificer,
            Self::Sofanthiel => Timeline::Sofanthiel,
            Self::Gourmand => Timeline::Gourmand,
            Self::Rivulet => Timeline::Rivulet,
            Self::Saint => Timeline::Saint,
            Self::Watcher => Timeline::Watcher,
            _ => Timeline::White,
        }
    }
}

ext_enum! {
    /// `SlugcatStats.Timeline`: the point in the world's history a campaign plays at.
    /// World files filter lines by it (`(White)`, `(X-Red)`). Unlike the names, every
    /// value is registered by the base game, DLC timelines included.
    pub enum Timeline {
        Base {
            Spear = "Spear",
            Artificer = "Artificer",
            /// Inv's timeline.
            Sofanthiel = "Inv",
            Red = "Red",
            Gourmand = "Gourmand",
            White = "White",
            Yellow = "Yellow",
            Rivulet = "Rivulet",
            Saint = "Saint",
            Watcher = "Watcher",
        }
    }
}

impl Timeline {
    /// The registered timeline whose string is `value`.
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.value() == value)
    }

    /// Position in `SlugcatStats.SlugcatTimelineOrder` (the Watcher is not in it).
    fn order(self) -> Option<usize> {
        match self {
            Self::Watcher => None,
            _ => Self::ALL.iter().position(|&t| t == self),
        }
    }

    /// `SlugcatStats.AtOrBeforeTimeline(self, other)`.
    #[must_use]
    pub fn at_or_before(self, other: Self) -> bool {
        match (self.order(), other.order()) {
            (Some(a), Some(b)) => a <= b,
            _ => false,
        }
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
                SlugcatName::White,
                SlugcatName::Yellow,
                SlugcatName::Red,
                SlugcatName::Night
            ]
        );
        assert_eq!(SlugcatName::Night.timeline(), Timeline::White);
        assert_eq!(SlugcatName::Red.timeline(), Timeline::Red);
        assert_eq!(Timeline::from_value("Inv"), Some(Timeline::Sofanthiel));
        assert_eq!(
            SlugcatName::from_player_number(2, ActiveMods::NONE),
            Some(SlugcatName::Red)
        );
        assert_eq!(SlugcatName::from_player_number(4, ActiveMods::NONE), None);
    }

    #[test]
    fn dlc_names_register_with_their_mods() {
        assert_eq!(SlugcatName::from_value("Spear", ActiveMods::NONE), None);
        let all = ActiveMods {
            msc: true,
            watcher: true,
            jolly_coop: true,
            ..ActiveMods::NONE
        };
        let names = SlugcatName::registered(all);
        assert_eq!(names.len(), SlugcatName::ALL.len());
        assert_eq!(names[4], SlugcatName::Rivulet);
        assert_eq!(
            SlugcatName::from_value("Inv", all),
            Some(SlugcatName::Sofanthiel)
        );
    }
}
