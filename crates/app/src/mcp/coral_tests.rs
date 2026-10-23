//! `get_state`'s `coral_systems`, `wall_mycelia` and `neuron_sparks` on `SS_D02` of the
//! installed game (skipped when it is absent). Expected values: its settings place seven
//! `WallMycelia` objects (`ss_d02_settings.txt`); a viewed room's system is neither frozen
//! nor away; an overseer put in the room joins the system's registry with its own strands
//! (two to six, `OverseerGraphics`' constructor).

use super::Session;
use super::params::{InputParam, SetPlayerParams, SpawnCreatureParams, StepParams};

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
    // The room's placed neurons hang their strands in the same registry.
    let neurons: Vec<_> = state
        .coral_neurons
        .iter()
        .filter(|n| n.room == "SS_D02")
        .collect();
    let neuron_strands: usize = neurons.iter().map(|n| n.mycelia.len()).sum();
    assert_eq!(system.mycelia, wall.strands + neuron_strands);
    assert_eq!(system.owners.len(), 1 + neurons.len());
    // Registered in the order `AIMapReady` made them: the neurons, then the wall.
    assert_eq!(system.owners.last().unwrap().kind, "wall_mycelia");
    // A wall strand pairs with another owner's only (the neurons' strands).
    assert_eq!(wall.pairs_connected, 0);
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
    assert_eq!(
        system.mycelia,
        wall.strands + neuron_strands + overseers[0].strands
    );
}

/// `SS_D02`'s placed neurons are reported with their ends and strands, registered as
/// climbable vines of kind `CoralNeuron`, and the player put on one with an up press takes
/// hold of it (`being_climbed`).
#[test]
fn neurons_are_reported_and_climbed() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_D02", 1);
    session.step_n(3);
    let state = session.state().expect("state");
    let neurons: Vec<_> = state
        .coral_neurons
        .iter()
        .filter(|n| n.room == "SS_D02")
        .collect();
    assert!(!neurons.is_empty());
    let vines = state
        .climbable_vines
        .iter()
        .find(|v| v.room == "SS_D02")
        .expect("vines");
    assert_eq!(
        vines
            .vines
            .iter()
            .filter(|v| v.kind == "CoralNeuron")
            .count(),
        neurons.len()
    );
    let system = state
        .coral_systems
        .iter()
        .find(|s| s.room == "SS_D02")
        .expect("system");
    for n in &neurons {
        assert!(n.segments >= 2 && n.bumps == (n.segments as f32 * 1.2) as usize);
        assert_eq!(n.mycelia.len(), (n.segments - 2).min(20) * 2);
        let owner = system
            .owners
            .iter()
            .find(|o| o.owner == n.owner)
            .expect("registry owner");
        assert_eq!(
            (owner.kind, owner.strands),
            ("coral_neuron", n.mycelia.len())
        );
        assert!(!n.being_climbed && !n.frozen);
    }
    let n = neurons[0];
    let middle = [
        (n.first[0] + n.last[0]) / 2.0,
        (n.first[1] + n.last[1]) / 2.0,
    ];
    session
        .set_player(&SetPlayerParams {
            pos: Some(middle),
            ..Default::default()
        })
        .expect("moves");
    let up = StepParams {
        ticks: Some(2),
        input: Some(InputParam {
            y: 1,
            ..Default::default()
        }),
    };
    let state = session.step(&up).expect("steps");
    let player = state.player.expect("player");
    assert_eq!(player.animation, "VineGrab");
    assert!(
        player
            .vine_pos
            .expect("vine_pos")
            .vine
            .starts_with("CoralNeuron")
    );
    assert_eq!(
        state
            .coral_neurons
            .iter()
            .filter(|n| n.being_climbed)
            .count(),
        1
    );
}
