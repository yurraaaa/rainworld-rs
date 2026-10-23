//! `SuperStructureFuses`: the grid's size, the culling, a tick's draws, the cell modes, the
//! neighbour smoothing, the malfunction spell and the gravity fade.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{FuseSetup, SuperStructureFuses};
use crate::{FloatExt, Vec2};

fn setup(rect: [i32; 4], size: f32, broken: f32, gravity_dependent: bool) -> FuseSetup {
    FuseSetup {
        pos: Vec2::new(100.0, 40.0),
        rect,
        size,
        depth: 1,
        broken,
        gravity_dependent,
    }
}

/// One cell: a 20 px grid over a rectangle one tile wide and high (its `Width` is 1).
fn one_cell(broken: f32) -> SuperStructureFuses {
    SuperStructureFuses::new(3, setup([0, 0, 1, 1], 20.0, broken, false))
}

fn advance(rng: &mut UnityRandom, n: usize) {
    for _ in 0..n {
        rng.value();
    }
}

/// `rect.Width * (int)(20 / size)` cells across: `Width` is `|right - left|`, so a
/// rectangle of one tile across has none; the cells start at zero.
#[test]
fn the_grid_has_width_times_the_cells_per_tile() {
    let f = SuperStructureFuses::new(3, setup([5, 2, 8, 3], 10.0, 0.0, false));
    assert_eq!((f.width, f.height), (6, 2));
    assert_eq!(f.cell(5, 1), [0.0; 5]);
    let f = SuperStructureFuses::new(3, setup([5, 2, 8, 3], 5.0, 0.0, false));
    assert_eq!((f.width, f.height), (12, 4));
    let f = SuperStructureFuses::new(3, setup([4, 4, 4, 4], 10.0, 0.0, false));
    assert_eq!((f.width, f.height), (0, 0));
    let f = SuperStructureFuses::new(3, setup([3, 7, 1, 8], 20.0, 0.0, false));
    assert_eq!((f.width, f.height), (2, 1));
    assert_eq!((f.power, f.power_flicker), (1.0, 1.0));
}

/// `GetFloatRect.GetCorner(i)`: top left, top right, bottom right, bottom left of the
/// tiles' pixels (the rectangle reaches the next tile's edge).
#[test]
fn corners_run_around_the_rectangle() {
    let f = SuperStructureFuses::new(3, setup([1, 2, 3, 4], 10.0, 0.0, false));
    assert_eq!(f.corner(0), Vec2::new(20.0, 100.0));
    assert_eq!(f.corner(1), Vec2::new(80.0, 100.0));
    assert_eq!(f.corner(2), Vec2::new(80.0, 40.0));
    assert_eq!(f.corner(3), Vec2::new(20.0, 40.0));
}

/// Out of every camera's sight the grid does nothing and draws nothing.
#[test]
fn a_culled_grid_does_not_update() {
    let mut f = one_cell(1.0);
    let before = f.clone();
    let mut rng = UnityRandom::new(4);
    f.update(false, 0.0, &mut rng);
    assert!(f.culled);
    assert_eq!(rng.state(), UnityRandom::new(4).state());
    assert_eq!(SuperStructureFuses { culled: false, ..f }, before);
}

/// A quiet one-cell grid: the draws are the lightness step, the cell's gate, its
/// `Pow(v, 0.25)` roll, the wobble, the 0.0025 roll, then the malfunction roll. The cell
/// moves by `Lerp(-1, 1, v) / 10`, then halfway to its neighbours' (none: its own, 0).
#[test]
fn a_quiet_cell_draws_four_and_wobbles() {
    let mut f = one_cell(0.0);
    let mut rng = UnityRandom::new(6);
    let mut e = UnityRandom::new(6);
    let draws: Vec<f32> = (0..6).map(|_| e.value()).collect();
    assert!(draws[4] >= 0.0025, "this seed has no jump");
    f.update(true, 0.0, &mut rng);
    assert_eq!(rng.state(), e.state());
    assert!(!f.culled);
    let wobble = ((-1.0f32).lerp(1.0, draws[3]) / 10.0).clamp(0.0, 1.0);
    assert_eq!(f.cell(0, 0)[0], wobble.lerp(0.0, 0.5));
    assert_eq!(f.last_lightness, 0.0);
    assert_eq!(
        f.lightness, 0.0,
        "no more than the smoothed malfunction (0)"
    );
    assert_eq!(f.last_pos, Vec2::new(100.0, 40.0));
    assert_eq!(f.debug_mode, 0);
}

/// The fixed modes: 1 and 5 hold the brightness at 1 (two draws a cell), 2 at 0, 3 draws
/// a random brightness, 4 flips while its timer's remainder over 12 is above 5; the timer
/// counts down first and a spent mode falls to the wobble.
#[test]
fn the_modes_hold_the_cell() {
    for (mode, timer, want, extra) in [
        (1.0, 3.0, 1.0, 0),
        (5.0, 3.0, 1.0, 0),
        (2.0, 3.0, 0.0, 0),
        (4.0, 8.0, 1.0, 0),
        (4.0, 4.0, 0.0, 0),
    ] {
        let mut f = one_cell(0.0);
        let c = f.cell_mut(0, 0);
        c[3] = mode;
        c[4] = timer;
        let mut rng = UnityRandom::new(6);
        f.update(true, 0.0, &mut rng);
        let c = f.cell(0, 0);
        assert_eq!(c[0], want, "mode {mode}");
        assert_eq!(c[4], timer - 1.0);
        // lightness + gate + `Pow(v, 0.25)` roll + malfunction roll.
        let mut e = UnityRandom::new(6);
        advance(&mut e, 4 + extra);
        assert_eq!(rng.state(), e.state(), "mode {mode}");
    }
    let mut f = one_cell(0.0);
    let c = f.cell_mut(0, 0);
    c[3] = 3.0;
    c[4] = 3.0;
    let mut rng = UnityRandom::new(6);
    let mut e = UnityRandom::new(6);
    let draws: Vec<f32> = (0..5).map(|_| e.value()).collect();
    f.update(true, 0.0, &mut rng);
    assert_eq!(f.cell(0, 0)[0], draws[3]);
    assert_eq!(rng.state(), e.state());
    // A mode whose timer runs out is 0 again and wobbles.
    let mut f = one_cell(0.0);
    let c = f.cell_mut(0, 0);
    c[3] = 1.0;
    c[4] = 1.0;
    f.update(true, 0.0, &mut UnityRandom::new(6));
    assert_eq!(f.cell(0, 0)[3], 0.0);
    assert_ne!(f.cell(0, 0)[0], 1.0);
}

/// A wobbling cell moves halfway toward the mean of its own and its neighbours' brightness
/// two ticks ago (`lights[.., 2]`), read before the neighbour's own turn.
#[test]
fn a_cell_moves_toward_its_neighbours() {
    let mut f = SuperStructureFuses::new(3, setup([0, 0, 2, 1], 20.0, 0.0, false));
    assert_eq!((f.width, f.height), (2, 1));
    f.cell_mut(1, 0)[2] = 0.6;
    f.cell_mut(0, 0)[1] = 0.3;
    let mut rng = UnityRandom::new(6);
    let mut e = UnityRandom::new(6);
    let draws: Vec<f32> = (0..4).map(|_| e.value()).collect();
    f.update(true, 0.0, &mut rng);
    // The cell's brightness 0 plus the wobble, then halfway to (own [2] + right [2]) / 2
    // with its own [2] the old [1].
    let wobble = ((-1.0f32).lerp(1.0, draws[3]) / 10.0).clamp(0.0, 1.0);
    let mean = (0.3 + 0.6) / 2.0;
    assert_eq!(f.cell(0, 0)[0], wobble.lerp(mean, 0.5));
}

/// `broken` is the `CorruptionSpores` amount, except outside an arena session in a region
/// that is neither `SS` nor `UW`, where it is 1.
#[test]
fn broken_depends_on_the_session_and_region() {
    let b = SuperStructureFuses::broken_for;
    assert_eq!(b(0.3, false, Some("SS")), 0.3);
    assert_eq!(b(0.3, false, Some("UW")), 0.3);
    assert_eq!(b(0.3, false, Some("SL")), 1.0);
    assert_eq!(b(0.3, true, Some("SL")), 0.3);
    assert_eq!(b(0.3, false, None), 0.3);
}

/// A malfunction starts with `Pow(v, broken) < 1 / Lerp(600, 120, broken)`: it lasts
/// `Range(20, 80)` ticks over a rectangle of cell coordinates drawn `-10` to the size
/// `+10` each, its corners put in order; the smoothed value climbs by 0.02.
#[test]
fn a_malfunction_draws_its_rectangle() {
    let mut found = 0;
    for seed in 0..4000 {
        let mut f = one_cell(1.0);
        let mut rng = UnityRandom::new(seed);
        f.update(true, 0.0, &mut rng);
        if f.mal_function == 0 {
            continue;
        }
        let mut e = UnityRandom::new(seed);
        advance(&mut e, 5);
        let roll = e.value();
        let mut again = e.clone();
        if roll >= 1.0 / 120.0 {
            continue;
        }
        let want = again.range_i32(20, 80);
        let mut r = [
            again.range_i32(-10, 11),
            again.range_i32(-10, 11),
            again.range_i32(-10, 11),
            again.range_i32(-10, 11),
        ];
        if r[0] > r[2] {
            r.swap(0, 2);
        }
        if r[1] > r[3] {
            r.swap(1, 3);
        }
        assert_eq!(f.mal_function, want);
        assert_eq!(f.malfunction_rect, r);
        assert!(r[0] <= r[2] && r[1] <= r[3]);
        assert_eq!(f.smoothed_malfunction, 0.0f32.lerp(1.0, 0.02));
        assert_eq!(rng.state(), again.state());
        // The next tick counts it down without a roll.
        let mut g = f.clone();
        let mut rng = UnityRandom::new(1);
        g.update(true, 0.0, &mut rng);
        assert_eq!(g.mal_function, want - 1);
        found += 1;
        if found == 3 {
            break;
        }
    }
    assert!(found >= 1, "no seed starts a spell");
}

/// A gravity-dependent grid fades its power toward `gravity > 0.5 ? 0 : 1` by
/// `0.2 * v`; a roll under 0.1 re-draws `powerFlicker`; lightness is held under `1 - power`.
#[test]
fn gravity_fades_the_power() {
    let seed = (0..100)
        .find(|&s| {
            let mut e = UnityRandom::new(s);
            e.value();
            e.value() >= 0.1
        })
        .expect("a seed that does not re-draw the flicker");
    let mut f = SuperStructureFuses::new(3, setup([0, 0, 1, 1], 20.0, 0.0, true));
    let mut rng = UnityRandom::new(seed);
    let mut e = UnityRandom::new(seed);
    let fade = e.value();
    e.value();
    f.update(true, 1.0, &mut rng);
    assert_eq!(f.power, 1.0f32.lerp(0.0, 0.2 * fade));
    assert_eq!(f.power_flicker, 1.0);
    assert_eq!(f.lightness, 0.0f32.min(1.0 - f.power));
    // Seven draws for the quiet cell tick and the extras of the power.
    advance(&mut e, 6);
    assert_eq!(rng.state(), e.state());
    // A flicker re-draw: `Lerp(v, power, |power - 0.5| * 2)`.
    for seed in 0..500 {
        let mut f = SuperStructureFuses::new(3, setup([0, 0, 1, 1], 20.0, 0.0, true));
        let mut e = UnityRandom::new(seed);
        e.value();
        if e.value() >= 0.1 {
            continue;
        }
        let v = e.value();
        f.update(true, 0.0, &mut UnityRandom::new(seed));
        assert_eq!(
            f.power,
            1.0f32.lerp(1.0, 0.2 * UnityRandom::new(seed).value())
        );
        assert_eq!(
            f.power_flicker,
            v.lerp(f.power, (f.power - 0.5).abs() * 2.0)
        );
        return;
    }
    panic!("no seed re-draws the flicker");
}
