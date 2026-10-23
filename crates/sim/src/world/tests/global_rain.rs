//! `GlobalRain`: the direction draws, the `DeathRain` modes with the draws of each entry on
//! both paths, the values each mode writes and the flood's start. Expected values from
//! `GlobalRain.cs` (`Update`, `DeathRain.DeathRainUpdate`, `NextDeathRainMode`).
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::*;

fn story(time_until_rain: i32) -> RainTick {
    RainTick {
        time_until_rain,
        rain_approaching: (time_until_rain as f32).inverse_lerp(0.0, 2400.0),
        story: true,
        passive: None,
    }
}

/// The first seed whose first draw is below (or at least) 0.7: the path `NextDeathRainMode`
/// takes from `None`.
fn seed_with_first_draw(below_070: bool) -> i32 {
    (0..)
        .find(|&s| (UnityRandom::new(s).value() < 0.7) == below_070)
        .unwrap()
}

fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} vs {b}");
}

/// Every tick draws `value < 0.025`, and a second value when it passes; the direction
/// follows `Lerp(d, target, 0.01)` then a step of at most 0.0125 toward the target.
#[test]
fn the_direction_draws_one_or_two_a_tick() {
    let mut rng = UnityRandom::new(7);
    let mut mirror = rng.clone();
    let mut g = GlobalRain::default();
    let (mut direction, mut target) = (0.0_f32, 0.0_f32);
    let (mut ones, mut twos) = (0, 0);
    for _ in 0..3000 {
        if mirror.value() < 0.025 {
            target = (-1.0_f32).lerp(1.0, mirror.value());
            twos += 1;
        } else {
            ones += 1;
        }
        let last = direction;
        direction = direction.lerp(target, 0.01);
        if direction < target {
            direction = (direction + 0.0125).min(target);
        } else if direction > target {
            direction = (direction - 0.0125).max(target);
        }
        g.update(&story(4000), &mut rng);
        assert_eq!(rng, mirror, "draws");
        assert_eq!((g.last_rain_direction, g.rain_direction), (last, direction));
    }
    assert!(
        ones > 2800 && twos > 20,
        "{ones} one-draw ticks, {twos} two-draw"
    );
    assert_eq!(g.water_flux_ticker, 3000);
    assert_eq!(g.flood_lerp_speed, 0.2);
}

/// The 70 % path: the first draw below 0.7 makes `AlternateBuildUp`, whose length is one
/// more draw: `Lerp(400, 1200, value)`.
#[test]
fn the_hit_picks_the_alternate_build_up_below_070() {
    let mut rng = UnityRandom::new(seed_with_first_draw(true));
    let mut mirror = rng.clone();
    let mut g = GlobalRain::default();
    g.init_death_rain(&mut rng);
    mirror.value();
    let length = 400.0_f32.lerp(1200.0, mirror.value());
    assert_eq!(rng, mirror, "two draws");
    let d = g.death_rain.unwrap();
    assert_eq!(d.mode, DeathRainMode::AlternateBuildUp);
    assert_eq!((d.time_in_this_mode, d.progression), (length, 0.0));
}

/// The 30 % path: `CalmBeforeStorm` lasts `Lerp(400, 800, value)` and its sunlight is 0
/// when a value is below 0.5, else one more value.
#[test]
fn the_hit_picks_the_calm_before_the_storm_otherwise() {
    let mut rng = UnityRandom::new(seed_with_first_draw(false));
    let mut mirror = rng.clone();
    let mut g = GlobalRain::default();
    g.init_death_rain(&mut rng);
    mirror.value();
    let length = 400.0_f32.lerp(800.0, mirror.value());
    let sunlight = if mirror.value() < 0.5 {
        0.0
    } else {
        mirror.value()
    };
    assert_eq!(rng, mirror, "three or four draws");
    let d = g.death_rain.unwrap();
    assert_eq!(d.mode, DeathRainMode::CalmBeforeStorm);
    assert_eq!(d.time_in_this_mode, length);
    assert_eq!(d.calm_before_storm_sunlight, sunlight);
}

impl GlobalRain {
    /// Runs `DeathRainUpdate` until the mode changes; returns the ticks it took.
    fn run_mode(&mut self, rng: &mut UnityRandom) -> usize {
        let mode = self.death_rain.unwrap().mode;
        let mut ticks = 0;
        while self.death_rain.unwrap().mode == mode {
            let mut d = self.death_rain.take().unwrap();
            d.update(self, 0.0, rng);
            self.death_rain = Some(d);
            ticks += 1;
        }
        ticks
    }
}

/// The draws at each mode entry after the hit, in order, and each mode's length in ticks
/// (`progression` passes 1 after `ceil(length)` ticks): `GradeAPlateu` one value,
/// `GradeBBuildUp` and `GradeBPlateu` one or two, `FinalBuildUp` two, `Mayhem` none.
#[test]
fn the_modes_follow_each_other_with_the_games_draws() {
    for below in [true, false] {
        let mut rng = UnityRandom::new(seed_with_first_draw(below));
        let mut g = GlobalRain::default();
        g.init_death_rain(&mut rng);
        let mut mirror = rng.clone();
        let mut modes = vec![g.death_rain.unwrap().mode];
        loop {
            let d = g.death_rain.unwrap();
            if d.mode == DeathRainMode::Mayhem {
                break;
            }
            let ticks = g.run_mode(&mut rng);
            let steps = (0..)
                .scan(0.0_f32, |p, _| {
                    *p += 1.0 / d.time_in_this_mode;
                    Some(*p)
                })
                .position(|p| p > 1.0)
                .unwrap()
                + 1;
            assert_eq!(
                ticks, steps,
                "{:?} lasts {} ticks",
                d.mode, d.time_in_this_mode
            );
            let next = g.death_rain.unwrap();
            let expect_time = match next.mode {
                DeathRainMode::GradeABuildUp => 6.0,
                DeathRainMode::GradeAPlateu => 400.0_f32.lerp(600.0, mirror.value()),
                DeathRainMode::GradeBBuildUp | DeathRainMode::GradeBPlateu => {
                    if mirror.value() < 0.5 {
                        100.0
                    } else {
                        50.0_f32.lerp(300.0, mirror.value())
                    }
                }
                DeathRainMode::FinalBuildUp => {
                    if mirror.value() < 0.5 {
                        300.0_f32.lerp(500.0, mirror.value())
                    } else {
                        100.0_f32.lerp(800.0, mirror.value())
                    }
                }
                DeathRainMode::Mayhem => next.time_in_this_mode,
                other => panic!("{other:?} follows {:?}", d.mode),
            };
            assert_eq!(rng, mirror, "draws entering {:?}", next.mode);
            assert_eq!(next.time_in_this_mode, expect_time);
            modes.push(next.mode);
        }
        use DeathRainMode::{
            AlternateBuildUp as Alt, CalmBeforeStorm as Calm, FinalBuildUp as Fin,
            GradeABuildUp as A, GradeAPlateu as AP, GradeBBuildUp as B, GradeBPlateu as BP,
            Mayhem as M,
        };
        let expect = if below {
            vec![Alt, AP, B, BP, Fin, M]
        } else {
            vec![Calm, A, AP, B, BP, Fin, M]
        };
        assert_eq!(modes, expect);
        // Mayhem ends nothing and draws nothing.
        let before = rng.clone();
        let mut d = g.death_rain.take().unwrap();
        d.update(&mut g, 0.0, &mut rng);
        assert_eq!(rng, before);
        assert_eq!(d.mode, DeathRainMode::Mayhem);
    }
}

fn in_mode(mode: DeathRainMode, progression: f32) -> GlobalRain {
    GlobalRain {
        death_rain: Some(DeathRain {
            mode,
            time_in_this_mode: 1000.0,
            progression: progression - 0.001,
            calm_before_storm_sunlight: 1.0,
        }),
        ..GlobalRain::default()
    }
}

fn step(g: &mut GlobalRain) {
    g.update(&story(-10), &mut UnityRandom::new(5));
}

/// The values each mode writes at a given progression (0.001 per tick on a 1000 tick mode).
#[test]
fn the_modes_write_their_values() {
    let mut g = in_mode(DeathRainMode::AlternateBuildUp, 0.5);
    step(&mut g);
    assert_close(g.intensity, 0.24 + (0.6 - 0.24) * 0.5);
    assert_close(g.micro_screen_shake, 1.25);
    assert_eq!((g.screen_shake, g.shader_light), (0.0, -1.0));

    let mut g = in_mode(DeathRainMode::GradeABuildUp, 0.5);
    step(&mut g);
    assert_close(g.intensity, 0.3);
    assert_close(g.micro_screen_shake, 0.75);
    assert_close(g.bullet_rain_density, 0.5);

    let mut g = in_mode(DeathRainMode::GradeBBuildUp, 0.5);
    step(&mut g);
    assert_close(g.intensity, 0.655);
    assert_close(g.micro_screen_shake, 1.8);
    assert_close(g.screen_shake, 0.6);

    let mut g = in_mode(DeathRainMode::FinalBuildUp, 0.5);
    step(&mut g);
    assert_close(g.intensity, 0.855);
    assert_close(g.micro_screen_shake, 2.1 + (4.0 - 2.1) * 0.5_f32.powf(1.2));
    assert_close(g.screen_shake, 2.1);

    // The plateaus hold what the build-up left.
    let mut g = in_mode(DeathRainMode::GradeAPlateu, 0.5);
    g.intensity = 0.6;
    g.micro_screen_shake = 1.5;
    step(&mut g);
    assert_eq!((g.intensity, g.micro_screen_shake), (0.6, 1.5));

    // Calm: a drizzle fading over the first 15 %, the sunlight blink, the bullet burst.
    let mut g = in_mode(DeathRainMode::CalmBeforeStorm, 0.075);
    step(&mut g);
    assert_close(g.intensity, 0.5_f32.powf(1.5) * 0.24);
    let blink = ((0.075_f32 - 0.03) / (0.8 - 0.03) * std::f32::consts::PI).sin();
    assert_close(g.shader_light, -1.0 + 0.3 * blink);
    assert_eq!(g.bullet_rain_density, 0.0);
    let mut g = in_mode(DeathRainMode::CalmBeforeStorm, 0.65);
    step(&mut g);
    assert_close(g.bullet_rain_density, 0.5_f32.powi(8));
}

/// `RumbleSound` falls by 0.025 in the calm and otherwise approaches
/// `1 - InverseLerp(0, 0.6, RainApproaching)` by a fifth.
#[test]
fn the_rumble_follows_the_mode() {
    let mut g = in_mode(DeathRainMode::CalmBeforeStorm, 0.5);
    g.rumble_sound = 0.01;
    step(&mut g);
    assert_eq!(g.rumble_sound, 0.0);
    let mut g = in_mode(DeathRainMode::GradeAPlateu, 0.5);
    g.rumble_sound = 0.5;
    step(&mut g);
    assert_close(g.rumble_sound, 0.5 + (1.0 - 0.5) * 0.2);
}

/// The flood rises once the mode's index is past `GradeABuildUp`'s (2), and the game
/// compares indices: `AlternateBuildUp` (8) counts. It speeds up by 0.0025 a tick to 0.8
/// and moves by that speed.
#[test]
fn the_flood_starts_by_mode_index() {
    for (mode, rises) in [
        (DeathRainMode::CalmBeforeStorm, false),
        (DeathRainMode::GradeABuildUp, false),
        (DeathRainMode::GradeAPlateu, true),
        (DeathRainMode::FinalBuildUp, true),
        (DeathRainMode::Mayhem, true),
        (DeathRainMode::AlternateBuildUp, true),
    ] {
        let mut g = in_mode(mode, 0.5);
        step(&mut g);
        assert_eq!(g.flood_speed > 0.0, rises, "{mode:?}");
        assert_eq!(g.flood, g.flood_speed, "{mode:?}");
    }
    let mut g = in_mode(DeathRainMode::Mayhem, 1.0);
    let mut expect = (0.0_f32, 0.0_f32);
    for _ in 0..400 {
        step(&mut g);
        expect.0 = (expect.0 + 0.0025).min(0.8);
        expect.1 += expect.0;
    }
    assert_eq!((g.flood_speed, g.flood), expect);
    assert_eq!(g.flood_speed, 0.8);
}

/// The index test fires on the tick a mode changes: `GradeABuildUp` is 6 ticks, its
/// transition tick already has `GradeAPlateu` (index 3) and raises the flood.
#[test]
fn the_flood_counts_the_mode_after_the_transition() {
    let mut rng = UnityRandom::new(3);
    let mut g = GlobalRain {
        death_rain: Some(DeathRain {
            mode: DeathRainMode::GradeABuildUp,
            time_in_this_mode: 6.0,
            progression: 0.0,
            calm_before_storm_sunlight: 0.0,
        }),
        ..GlobalRain::default()
    };
    let mut progression = 0.0_f32;
    let mut change = 0;
    while progression <= 1.0 {
        progression += 1.0 / 6.0;
        change += 1;
    }
    assert!(change >= 6);
    for tick in 1..=change + 1 {
        g.update(&story(-1), &mut rng);
        let rising = g.flood_speed > 0.0;
        assert_eq!(rising, tick >= change, "tick {tick}");
    }
    assert_eq!(g.death_rain.unwrap().mode, DeathRainMode::GradeAPlateu);
}

/// Before the hit `Intensity` is `InverseLerp(600, 200, TimeUntilRain) * 0.24`; the camera
/// room's rain effects override it: `HeavyRain` to `(1 + 4 x) * 0.24` with a rumble and
/// shake, `LightRain` to `x * 0.24`; `BulletRain` sets the bullet density.
#[test]
fn before_the_hit_the_intensity_ramps_and_the_room_effects_override() {
    let mut rng = UnityRandom::new(1);
    let mut g = GlobalRain::default();
    g.update(&story(400), &mut rng);
    assert_close(g.intensity, 0.12);
    g.update(&story(700), &mut rng);
    assert_eq!(g.intensity, 0.0);
    g.update(&story(100), &mut rng);
    assert_close(g.intensity, 0.24);
    let tick = |passive| RainTick {
        passive: Some(passive),
        ..story(700)
    };
    g.update(
        &tick(PassiveRain {
            heavy_rain: 0.5,
            ..PassiveRain::default()
        }),
        &mut rng,
    );
    assert_close(g.intensity, 0.72);
    assert_close(g.rumble_sound, 0.1);
    assert_close(g.screen_shake, 0.5);
    g.update(
        &tick(PassiveRain {
            light_rain: 0.5,
            ..PassiveRain::default()
        }),
        &mut rng,
    );
    assert_close(g.intensity, 0.12);
    g.update(
        &tick(PassiveRain {
            bullet_rain: 0.3,
            ..PassiveRain::default()
        }),
        &mut rng,
    );
    assert_close(g.bullet_rain_density, 0.3);
    g.update(&tick(PassiveRain::default()), &mut rng);
    assert_eq!(g.bullet_rain_density, 0.0);
}

/// `HeavyRainFlux` pulses the heavy rain: up over 60 ticks, held for `1200 x flux`,
/// down over 60, off for the hold and one fade, the timer wrapping at `120 + 2400 x flux`.
#[test]
fn the_heavy_rain_flux_pulses() {
    let mut rng = UnityRandom::new(1);
    let mut g = GlobalRain::default();
    let passive = PassiveRain {
        heavy_rain: 1.0,
        heavy_rain_flux: 0.05,
        ..PassiveRain::default()
    };
    let tick = RainTick {
        passive: Some(passive),
        ..story(700)
    };
    let hold = 60.0;
    let mut seen = Vec::new();
    for _ in 0..(120 + 120) {
        g.update(&tick, &mut rng);
        seen.push((g.heavy_timer, g.screen_shake));
    }
    // timer 1: 1/60 of the heavy rain; 60..120 held (hold 60); then down; then off.
    assert_close(seen[0].1, 1.0 / 60.0);
    assert_eq!(seen[59], (60, 1.0));
    assert_eq!(seen[119].0, 120);
    assert_close(seen[119].1, 1.0 - (120.0 - (hold + 60.0)) / 60.0);
    assert_close(seen[150].1, 1.0 - (151.0 - 120.0) / 60.0);
    // Off: the shake keeps the last value it had.
    assert_eq!(seen[200], (201, seen[178].1));
    assert_eq!(seen[239].0, 0);
}

/// `OutsidePushAround` starts at `Intensity` 0.35 and `InsidePushAround` at 0.63.
#[test]
fn the_push_starts_at_the_thresholds() {
    let g = |intensity| GlobalRain {
        intensity,
        ..GlobalRain::default()
    };
    assert!(!g(0.35).any_push_around());
    assert!(g(0.36).any_push_around());
    assert_eq!(g(0.35).inside_push_around(), 0.0);
    assert!(g(0.64).inside_push_around() > 0.0);
    assert_close(g(0.7).outside_push_around(), 1.0);
    assert_close(g(0.98).inside_push_around(), 1.0);
}

#[test]
fn the_modes_have_the_games_indices() {
    assert_eq!(DeathRainMode::GradeABuildUp.index(), 2);
    assert_eq!(DeathRainMode::AlternateBuildUp.index(), 8);
    assert_eq!(
        DeathRainMode::from_value("Mayhem"),
        Some(DeathRainMode::Mayhem)
    );
    assert_eq!(
        DeathRainMode::FinalBuildUp.next_registered(),
        Some(DeathRainMode::Mayhem)
    );
}
