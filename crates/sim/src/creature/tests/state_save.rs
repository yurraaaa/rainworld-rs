use rw_assets::{ActiveMods, CreatureType};

use super::*;
use crate::creature::{CreatureTemplate, EntityId};

fn creature(kind: CreatureType) -> AbstractCreature {
    let template = CreatureTemplate::of(kind, ActiveMods::NONE);
    AbstractCreature {
        template,
        id: EntityId::new(1, 5),
        pos: crate::creature::WorldCoordinate::new(0, -1, -1, 0),
        personality: crate::creature::Personality::new(
            EntityId::new(1, 5),
            &mut UnityRandom::new(0),
        ),
        state: AbstractCreature::state_for(&template),
        ai: None,
        remain_in_den_counter: -1,
        spawn_den: None,
        spawn_data: None,
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

fn round_trip(kind: CreatureType, s: &str) -> AbstractCreature {
    let mut c = creature(kind);
    c.load_state(s);
    assert_eq!(c.state_save_string(), s, "{kind}");
    c
}

#[test]
fn a_lizards_state_round_trips_in_the_games_order() {
    // `BaseSaveString` ends its entries with `<cB>`, `HealthState` starts its own
    // with it: an empty entry between them, read and written back.
    let s = "Social<cC>REL<rA>ID.-1.0<rA>L<rB>0.75<cB>SpawnData<cC>{Night}<cB>\
                 <cB>Health<cC>0.5<cB>ThroatHealth<cC>0.25<cB>LimbHealth<cC>1<cC>0.5<cC>1<cC>1\
                 <cB>MODKEY<cC>x";
    let c = round_trip(CreatureType::GreenLizard, s);
    assert_eq!(c.state.health, 0.5);
    assert_eq!(c.spawn_data.as_deref(), Some("{Night}"));
    let m = c.state.social_memory.as_ref().expect("social");
    assert_eq!(m.like(EntityId::new(-1, 0)), 0.75);
    assert_eq!(c.state.save.unrecognized, [("MODKEY".into(), "x".into())]);
}

#[test]
fn plain_states_end_unrecognized_entries_with_the_separator() {
    let c = round_trip(CreatureType::Fly, "Dead<cB>Health<cC>0.5<cB>");
    assert!(!c.state.alive);
    // `NoHealthState` keeps `Health` as unrecognized.
    assert_eq!(c.state.health, 1.0);
    round_trip(CreatureType::Vulture, "<cB>NOMASK");
    round_trip(CreatureType::BigNeedleWorm, "<cB>EGGSPAWN");
    round_trip(CreatureType::Centipede, "<cB>Shells<cC>101<cB>MEATINIT");
    round_trip(CreatureType::LanternMouse, "<cB>Battery<cC>120");
}

#[test]
fn a_dead_creature_saves_no_social_memory() {
    let mut c = creature(CreatureType::PinkLizard);
    c.load_state("Social<cC>REL<rA>ID.-1.0<rA>L<rB>1<cB>");
    c.state.alive = false;
    assert_eq!(c.state_save_string(), "Dead<cB>");
}

#[test]
fn cycle_tick_heals_and_regrows_shells_with_two_draws_per_regrowth() {
    let mut c = creature(CreatureType::GreenLizard);
    c.load_state("<cB>Health<cC>0.5<cB>ThroatHealth<cC>0.5<cB>LimbHealth<cC>0<cC>1<cC>1<cC>1");
    c.state.cycle_tick(&mut UnityRandom::new(1));
    assert_eq!(c.state.health, 0.75);
    assert_eq!(
        c.state.parts,
        StateParts::Lizard {
            throat_health: 0.75,
            limb_health: [0.5, 1.0, 1.0, 1.0],
            rot_type: 0
        }
    );

    let mut c = creature(CreatureType::Centipede);
    c.load_state("<cB>Shells<cC>000");
    let mut rng = UnityRandom::new(3);
    c.state.cycle_tick(&mut rng);
    let mut m = UnityRandom::new(3);
    let mut expected = Vec::new();
    for _ in 0..3 {
        expected.push(m.value() < 0.2 && m.value() < 0.985);
    }
    assert_eq!(rng.state(), m.state());
    let StateParts::Centipede { shells, .. } = &c.state.parts else {
        panic!("centipede state");
    };
    assert_eq!(shells.as_deref(), Some(expected.as_slice()));
}
