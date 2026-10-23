//! The centipede's size from its ID, which its icon reads while it has no body
//! (`CreatureSymbol.SymbolDataFromCreature`).
//!
//! Mirrors: Centipede.cs (GenerateSize), AbstractCreature.cs (setCustomFlags)

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use super::AbstractCreature;
use crate::FloatExt;

impl AbstractCreature {
    /// `Centipede.GenerateSize`: drawn from the ID's seed on a stream of its own (the
    /// game saves and restores `Random.state`, so the shared stream is untouched), with
    /// More Slugcats' aqua centipede left out; a centipede takes the first number among
    /// its spawn data's unrecognized flags (`{0.5}`). The region's
    /// `globalCreatureFlags` are not read (no vanilla region sets them).
    #[must_use]
    pub fn centipede_generate_size(&self) -> f32 {
        let mut rng = UnityRandom::new(self.id.random_seed());
        let mut size = 0.0_f32.lerp(1.0, rng.value().mathf_pow(1.5));
        match self.kind() {
            CreatureType::SmallCentipede => size = 0.0,
            CreatureType::Centiwing => size = 0.5_f32.lerp(0.65, rng.value()),
            CreatureType::RedCentipede => size = 1.0,
            CreatureType::Centipede => {
                if let Some(flag) = self.unrecognized_spawn_flags().find_map(|f| f.parse().ok()) {
                    size = flag;
                }
            }
            _ => {}
        }
        size
    }

    /// `unrecognizedFlags` of `setCustomFlags`: the spawn data's `{...}` entries that
    /// name no flag the game knows.
    fn unrecognized_spawn_flags(&self) -> impl Iterator<Item = &str> {
        const KNOWN: [&str; 13] = [
            "Night",
            "Lavasafe",
            "Ignorecycle",
            "AlternateForm",
            "TentacleImmune",
            "Winter",
            "PreCycle",
            "Voidsea",
            "Ripple",
            "Seed",
            "RotType",
            "Slayer",
            "NamedAttr",
        ];
        self.spawn_data
            .as_deref()
            .filter(|d| d.starts_with('{'))
            .and_then(|d| d.get(1..d.len().saturating_sub(1)))
            .into_iter()
            .flat_map(|list| list.split([',', '|']))
            .filter(|f| !f.is_empty() && !KNOWN.contains(&f.split(':').next().unwrap_or("")))
            .map(str::trim)
    }
}

#[cfg(test)]
#[path = "tests/centipede.rs"]
mod tests;
