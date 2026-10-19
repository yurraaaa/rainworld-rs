//! `AbstractPhysicalObject.AbstractObjectType`: the kinds of physical object, as saves and
//! the game's code name them. What the simulation makes of them is `rw_sim`'s.
//!
//! Mirrors: AbstractPhysicalObject.cs

use crate::ActiveMods;
use crate::identity::ext_enum;

ext_enum! {
    /// `AbstractPhysicalObject.AbstractObjectType`, every base and DLC value.
    pub enum AbstractObjectType {
        Base {
            Creature = "Creature",
            Rock = "Rock",
            Spear = "Spear",
            FlareBomb = "FlareBomb",
            VultureMask = "VultureMask",
            PuffBall = "PuffBall",
            GraffitiBomb = "GraffitiBomb",
            DangleFruit = "DangleFruit",
            Oracle = "Oracle",
            PebblesPearl = "PebblesPearl",
            SlOracleSwarmer = "SLOracleSwarmer",
            SsOracleSwarmer = "SSOracleSwarmer",
            DataPearl = "DataPearl",
            SeedCob = "SeedCob",
            WaterNut = "WaterNut",
            JellyFish = "JellyFish",
            Lantern = "Lantern",
            KarmaFlower = "KarmaFlower",
            Mushroom = "Mushroom",
            VoidSpawn = "VoidSpawn",
            FirecrackerPlant = "FirecrackerPlant",
            SlimeMold = "SlimeMold",
            FlyLure = "FlyLure",
            ScavengerBomb = "ScavengerBomb",
            SporePlant = "SporePlant",
            AttachedBee = "AttachedBee",
            EggBugEgg = "EggBugEgg",
            NeedleEgg = "NeedleEgg",
            DartMaggot = "DartMaggot",
            BubbleGrass = "BubbleGrass",
            NshSwarmer = "NSHSwarmer",
            OverseerCarcass = "OverseerCarcass",
            CollisionField = "CollisionField",
            BlinkingFlower = "BlinkingFlower",
            Pomegranate = "Pomegranate",
            LobeTree = "LobeTree",
        }
        DlcShared {
            SingularityBomb = "SingularityBomb",
            Seed = "Seed",
            GooieDuck = "GooieDuck",
            LillyPuck = "LillyPuck",
            GlowWeed = "GlowWeed",
            DandelionPeach = "DandelionPeach",
        }
        Msc {
            JokeRifle = "JokeRifle",
            Bullet = "Bullet",
            Spearmasterpearl = "Spearmasterpearl",
            FireEgg = "FireEgg",
            EnergyCell = "EnergyCell",
            Germinator = "Germinator",
            MoonCloak = "MoonCloak",
            HalcyonPearl = "HalcyonPearl",
            HrGuard = "HRGuard",
        }
        Watcher {
            RippleSpawn = "RippleSpawn",
            FireSpriteLarva = "FireSpriteLarva",
            Boomerang = "Boomerang",
            PrinceBulb = "PrinceBulb",
            Prince = "Prince",
            SpinToy = "SpinToy",
            BallToy = "BallToy",
            SoftToy = "SoftToy",
            WeirdToy = "WeirdToy",
            KnotSpawn = "KnotSpawn",
            RippleJelly = "RippleJelly",
        }
    }
}

impl AbstractObjectType {
    /// The registered value spelled `value` (`type.Index != -1`).
    #[must_use]
    pub fn from_value(value: &str, mods: ActiveMods) -> Option<Self> {
        Self::registered(mods)
            .into_iter()
            .find(|t| t.value() == value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_values_only_without_dlc() {
        assert_eq!(
            AbstractObjectType::from_value("Spear", ActiveMods::NONE),
            Some(AbstractObjectType::Spear)
        );
        assert_eq!(
            AbstractObjectType::from_value("SLOracleSwarmer", ActiveMods::NONE),
            Some(AbstractObjectType::SlOracleSwarmer)
        );
        assert_eq!(
            AbstractObjectType::from_value("LillyPuck", ActiveMods::NONE),
            None
        );
        let msc = ActiveMods {
            msc: true,
            ..ActiveMods::NONE
        };
        assert_eq!(
            AbstractObjectType::from_value("LillyPuck", msc),
            Some(AbstractObjectType::LillyPuck)
        );
        assert_eq!(AbstractObjectType::registered(ActiveMods::NONE).len(), 36);
    }
}
