//! The centipede's size from its ID, which its icon reads while it has no body
//! (`CreatureSymbol.SymbolDataFromCreature`).
//!
//! Mirrors: Centipede.cs (GenerateSize)

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
mod tests {
    use rw_assets::ActiveMods;

    use super::*;
    use crate::creature::{CreatureTemplate, EntityId, Personality, WorldCoordinate};

    fn centipede(kind: CreatureType, number: i32, spawn_data: Option<&str>) -> AbstractCreature {
        let template = CreatureTemplate::of(kind, ActiveMods::NONE);
        let id = EntityId::new(-1, number);
        AbstractCreature {
            template,
            id,
            pos: WorldCoordinate::new(0, 1, 1, -1),
            personality: Personality::new(id, &mut UnityRandom::new(0)),
            state: AbstractCreature::state_for(&template),
            ai: None,
            remain_in_den_counter: -1,
            spawn_den: None,
            spawn_data: spawn_data.map(str::to_owned),
            night_creature: false,
            ignore_cycle: false,
            pre_cycle: false,
            in_den: false,
            time_spent_here: 0,
            even_update: false,
            distance_to_my_node: 0,
            slated_for_deletion: false,
            realized: None,
            carried: Vec::new(),
            grasps: Vec::new(),
            grabbed_by: Vec::new(),
            sticks: Vec::new(),
            kill_tag: None,
            kill_tag_counter: 0,
        }
    }

    #[test]
    fn size_is_the_seeded_draw_to_the_power_one_and_a_half() {
        // `Mathf.Pow(Random.value, 1.5f)` after `InitState(ID.RandomSeed)`.
        let mut rng = UnityRandom::new(4321);
        let expected = rng.value().powf(1.5);
        let c = centipede(CreatureType::Centipede, 4321, None);
        assert!((c.centipede_generate_size() - expected).abs() < 1e-6);
        assert_eq!(
            centipede(CreatureType::RedCentipede, 4321, None).centipede_generate_size(),
            1.0
        );
        assert_eq!(
            centipede(CreatureType::SmallCentipede, 4321, None).centipede_generate_size(),
            0.0
        );
    }

    #[test]
    fn a_number_flag_sets_a_centipedes_size() {
        let c = centipede(CreatureType::Centipede, 7, Some("{Night,0.9}"));
        assert_eq!(c.centipede_generate_size(), 0.9);
    }
}
