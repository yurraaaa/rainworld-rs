//! `get_state`'s `electric_death` and `border_push_back`, and the effect drawn, on rooms of
//! the installed game (skipped when it is absent). Expected values: `uw_a04`'s
//! `ElectricDeath-1` in `uw_a04_settings.txt`; the fog template's `BorderPushBack-1-5-79`
//! for `sl_c03`, whose cameras are at x -40, -40 and -37 (`sl_c03.txt`), so its edges are
//! -40 + 117 and -37 + 1400 - 117.

use super::Session;
use super::params::{InputParam, ScreenshotParams, SetPlayerParams, StepParams};

impl Session {
    fn step_ticks(&mut self, ticks: u32) {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    }

    /// Steps until the rain cycle's timer reaches `timer`.
    fn step_to_timer(&mut self, timer: i32) {
        loop {
            let now = self.state().expect("state").rain.timer;
            if now >= timer {
                return;
            }
            self.step_ticks(u32::try_from((timer - now).min(20_000)).expect("ticks"));
        }
    }

    /// The share of the screenshot's pixels that are clearly green.
    fn green_pixels(&mut self) -> Option<usize> {
        let shot = match self.screenshot(&ScreenshotParams::default()) {
            Ok(shot) => shot,
            Err(e) if e.starts_with("no GPU renderer") => {
                eprintln!("skipping: {e}");
                return None;
            }
            Err(e) => panic!("{e}"),
        };
        let decoder = png::Decoder::new(std::io::Cursor::new(&shot.png));
        let mut reader = decoder.read_info().expect("decodes");
        let mut pixels = vec![0; reader.output_buffer_size().expect("size")];
        reader.next_frame(&mut pixels).expect("frame");
        let rgb = pixels.as_chunks::<3>().0;
        Some(
            rgb.iter()
                .filter(|p| p[1] > 120 && i32::from(p[1]) > i32::from(p[0]) + 60)
                .count(),
        )
    }
}

/// An `ElectricDeath` room reports the effect before it has any intensity (the amount, the
/// wall tiles, no loops), then at 1200 ticks past the end of the cycle `Intensity` 1, the
/// loops at their volumes with their two emitters, and the strikes; the screenshot has
/// many more green pixels than before in every one of a few frames.
#[test]
fn electric_death_state_and_picture() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("UW_A04", 1);
    let state = session.state().expect("state");
    let [ed] = &state.electric_death[..] else {
        panic!("{:?}", state.electric_death);
    };
    assert_eq!(ed.amount, 1.0);
    assert_eq!(ed.intensity, 0.0);
    assert!(ed.wall_tiles > 100, "{}", ed.wall_tiles);
    assert!(ed.heavy_loop_volume.is_none() && ed.recent_strikes.is_empty());
    assert!(state.border_push_back.is_empty());
    let before = session.green_pixels();

    let end = state.rain.cycle_length;
    session.step_to_timer(end + 1200);
    let state = session.state().expect("state");
    let ed = &state.electric_death[0];
    assert_eq!(ed.intensity, 1.0);
    assert!(ed.sin > 10.0, "sin += Intensity * 0.1 a tick: {}", ed.sin);
    assert_eq!(ed.heavy_loop_volume, Some(1.0));
    assert!(ed.sizzle_loop_volume.is_some());
    assert_eq!(ed.loop_emitters, 2);
    assert_eq!(ed.flashes.len(), 8);
    assert!(ed.flashes.iter().all(|f| f.rad >= 40.0 && f.rad <= 800.0));
    // The green flash glows come and go with the random draws, but the web tint on the
    // wall tiles is there every frame: even the dimmest of several frames shows it.
    let mut dimmest = usize::MAX;
    for _ in 0..4 {
        let Some(green) = session.green_pixels() else {
            return;
        };
        dimmest = dimmest.min(green);
        session.step_ticks(10);
    }
    let before = before.expect("rendered");
    assert!(dimmest > before + 2000, "{before} -> {dimmest}");
}

/// `sl_c03` reports its edges and a `pushPower` of 0 per player; a slugcat put left of the
/// left edge is pushed: after one tick out `pushPower` is `0.01 + 1/600`.
#[test]
fn border_push_back_state_and_push() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SL_C03", 1);
    let state = session.state().expect("state");
    let [b] = &state.border_push_back[..] else {
        panic!("{:?}", state.border_push_back);
    };
    assert_eq!(b.margin, -117.0);
    assert_eq!(b.edges, [77.0, 1246.0]);
    assert_eq!(b.push_power, vec![0.0]);
    assert!(state.electric_death.is_empty());
    session
        .set_player(&SetPlayerParams {
            tile: Some([2, 118]),
            ..SetPlayerParams::default()
        })
        .expect("moves");
    session.step_ticks(1);
    let state = session.state().expect("state");
    let p = state.border_push_back[0].push_power[0];
    assert!((p - (0.01 + 1.0 / 600.0)).abs() < 1e-6, "{p}");
    session.step_ticks(60);
    let state = session.state().expect("state");
    let head = state.player.as_ref().expect("player").chunks[0].pos;
    assert!(head[0] > 77.0, "pushed back inside: {head:?}");
}
