//! `get_state`'s `coral_systems`, `wall_mycelia` and `neuron_sparks` on `SS_D02` of the
//! installed game (skipped when it is absent). Expected values: its settings place seven
//! `WallMycelia` objects (`ss_d02_settings.txt`); a viewed room's system is neither frozen
//! nor away; an overseer put in the room joins the system's registry with its own strands
//! (two to six, `OverseerGraphics`' constructor).

use super::Session;
use super::params::{InputParam, SpawnCreatureParams, StepParams};

impl Session {
    fn step_n(&mut self, ticks: u32) {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    }
}

#[test]
fn a_superstructure_room_reports_its_coral() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_D02", 1);
    session.step_n(3);
    let state = session.state().expect("state");
    let system = state
        .coral_systems
        .iter()
        .find(|s| s.room == "SS_D02")
        .expect("system");
    assert!(system.built && !system.frozen);
    assert_eq!(system.not_viewed_counter, 0);
    assert_eq!(system.age, 3);
    let wall = state
        .wall_mycelia
        .iter()
        .find(|w| w.room == "SS_D02")
        .expect("wall mycelia");
    assert_eq!(wall.places.len(), 7);
    assert!(wall.strands > 100, "{}", wall.strands);
    assert_eq!(system.mycelia, wall.strands);
    assert_eq!(system.owners.len(), 1);
    assert_eq!(system.owners[0].kind, "wall_mycelia");
    assert_eq!((wall.pairs_connected, wall.linked_to_others), (0, 0));
    assert!(wall.culled <= wall.strands);

    session
        .spawn_creature(&SpawnCreatureParams {
            kind: "Overseer".to_owned(),
            room: None,
            tile: [91, 25],
            seed_id: None,
        })
        .expect("spawns");
    session.step_n(5);
    let state = session.state().expect("state");
    let system = state
        .coral_systems
        .iter()
        .find(|s| s.room == "SS_D02")
        .expect("system");
    let overseers: Vec<_> = system
        .owners
        .iter()
        .filter(|o| o.kind == "overseer")
        .collect();
    assert_eq!(overseers.len(), 1, "{:?}", system.owners);
    assert!(
        (2..=6).contains(&overseers[0].strands),
        "{}",
        overseers[0].strands
    );
    assert_eq!(system.mycelia, wall.strands + overseers[0].strands);
}
