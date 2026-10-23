//! `get_state`'s `coral_circuits` and the player's circuit swimming fields on the installed
//! game's `SS_D02` (skipped when it is absent). Expected values: its settings place six
//! `CoralCircuit` objects (`ss_d02_settings.txt`) and `ZeroG` 1, so the player swims; the
//! picture shows the bits' red.
//!
//! Mirrors: - (port-only: MCP server)

use super::Session;
use super::params::{InputParam, ScreenshotParams, SetPlayerParams, StepParams};

impl Session {
    fn step_with(&mut self, ticks: u32, input: InputParam) {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(input),
        })
        .expect("steps");
    }
}

#[test]
fn a_circuit_room_reports_its_circuits() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_D02", 1);
    session.step_with(3, InputParam::default());
    let state = session.state().expect("state");
    let circuits: Vec<_> = state
        .coral_circuits
        .iter()
        .filter(|c| c.room == "SS_D02")
        .collect();
    assert!(!circuits.is_empty());
    let places: usize = circuits.iter().map(|c| c.places.len()).sum();
    assert_eq!(places, 6);
    for c in &circuits {
        assert!(c.bits >= c.tiles && c.bits <= 2 * c.tiles, "{}", c.bits);
        assert_eq!(c.displaced, 0);
        assert_eq!(c.links, c.active_links);
        assert!(!c.connectors.is_empty() && !c.crabs.is_empty());
        assert!(
            c.connectors
                .iter()
                .all(|x| x.strands >= 5 && x.strands < 10)
        );
        assert!(c.crabs.iter().all(|x| (4..8).contains(&x.legs)));
    }
    // The connectors' strands are in the system's registry with the other owners'.
    let system = state
        .coral_systems
        .iter()
        .find(|s| s.room == "SS_D02")
        .expect("system");
    let strands: usize = circuits
        .iter()
        .flat_map(|c| &c.connectors)
        .map(|x| x.strands)
        .sum();
    assert!(system.mycelia >= strands);
    assert_eq!(state.player.as_ref().unwrap().circuit_swim_resistance, 0.0);
    assert_eq!(state.player.as_ref().unwrap().circuit_jump_meter, 0.0);
    assert!(state.player.as_ref().unwrap().swim_bits.is_empty());
}

/// The player put in the middle of the first circuit's first place and held right is pushed
/// off the bits: its meter counts them, the bits it swam through are remembered, and bits
/// it knocked loose are reported displaced.
#[test]
fn swimming_through_a_circuit_counts_and_remembers_bits() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_D02", 1);
    session.step_with(3, InputParam::default());
    let state = session.state().expect("state");
    let first = state
        .coral_circuits
        .iter()
        .find(|c| c.room == "SS_D02")
        .expect("circuit");
    let at = first.places[0].0;
    session
        .set_player(&SetPlayerParams {
            pos: Some(at),
            vel: Some([3.0, 0.0]),
            ..Default::default()
        })
        .expect("set");
    let right = InputParam {
        x: 1,
        ..Default::default()
    };
    let mut best_meter = 0.0f32;
    let mut remembered = false;
    for _ in 0..40 {
        session.step_with(1, right);
        let p = session.state().expect("state").player.expect("player");
        best_meter = best_meter.max(p.circuit_jump_meter);
        remembered |= !p.swim_bits.is_empty();
    }
    assert!(best_meter > 0.0, "{best_meter}");
    assert!(remembered);
    let state = session.state().expect("state");
    let displaced: usize = state
        .coral_circuits
        .iter()
        .filter(|c| c.room == "SS_D02")
        .map(|c| c.displaced)
        .sum();
    assert!(displaced > 0);
}

/// The room's screenshot has the bits' dim red where the circuits are.
#[test]
fn the_circuits_are_drawn_red() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_D02", 1);
    session.step_with(3, InputParam::default());
    let state = session.state().expect("state");
    let at = state
        .coral_circuits
        .iter()
        .find(|c| c.room == "SS_D02")
        .expect("circuit")
        .places[0]
        .0;
    session
        .set_player(&SetPlayerParams {
            pos: Some(at),
            ..Default::default()
        })
        .expect("set");
    let swim = std::env::var("RW_CIRCUIT_SWIM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    session.step_with(
        swim,
        InputParam {
            x: i8::from(swim > 2),
            ..Default::default()
        },
    );
    let shot = match session.screenshot(&ScreenshotParams {
        save_path: std::env::var("RW_CIRCUIT_SHOT").ok(),
        crop_radius: std::env::var("RW_CIRCUIT_CROP")
            .ok()
            .and_then(|v| v.parse().ok()),
        ..Default::default()
    }) {
        Ok(shot) => shot,
        Err(e) if e.starts_with("no GPU renderer") => {
            eprintln!("skipping: {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let mut reader = png::Decoder::new(std::io::Cursor::new(&shot.png))
        .read_info()
        .expect("decodes");
    let mut pixels = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut pixels).expect("frame");
    let bytes = info.color_type.samples();
    let red = pixels
        .chunks(bytes)
        .filter(|p| {
            p[0] >= 150
                && i32::from(p[0]) > i32::from(p[1]) + 80
                && i32::from(p[0]) > i32::from(p[2]) + 80
        })
        .count();
    assert!(red > 2000, "{red} red pixels");
}
