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
