//! `ProjectedScanLines`: draw counts of the constructor and `Update`, the refill, the
//! lines' life. Expected values follow the decompiled `ProjectedScanLines.cs`.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{ProjectedScanLine, ProjectedScanLines};
use crate::Vec2;

fn drawn(before: &UnityRandom, after: &UnityRandom) -> usize {
    let mut probe = before.clone();
    for n in 0..100_000 {
        if probe == *after {
            return n;
        }
        probe.next_u32();
    }
    panic!("not within 100000 draws");
}

#[test]
fn the_ideal_number_follows_the_room() {
    let mut seq = 0;
    let mut rng = UnityRandom::new(1);
    // 100 x 60 tiles at 0.71: Lerp(6000, 100, 0.5) / 2000 * 10 * 0.71 = 10.83.
    let p = ProjectedScanLines::new((100, 60), 0.71, &mut seq, &mut rng);
    assert_eq!(p.ideal_scan_line_number(), 10);
    assert_eq!(p.lines.len(), 10);
}

/// Per line: three draws for the side, the position and the direction, the line's own for
/// its speed in 0.5..3.5.
#[test]
fn constructor_draws_four_per_line() {
    let before = UnityRandom::new(3);
    let mut rng = before.clone();
    let mut seq = 7;
    let p = ProjectedScanLines::new((100, 60), 1.0, &mut seq, &mut rng);
    assert_eq!(p.lines.len(), 15);
    assert_eq!(drawn(&before, &rng), 15 * 4);
    // The lines are in the update list before the object.
    assert_eq!(p.update_order, 7 + 16);
    assert_eq!(p.lines[0].update_order, 8);
    assert!(p.lines.iter().all(|l| (0.5..=3.5).contains(&l.speed.abs())));
    assert!(p.visible);
}

#[test]
fn update_rolls_visibility_then_refills_one_line() {
    let mut seq = 0;
    let mut rng = UnityRandom::new(8);
    let mut p = ProjectedScanLines::new((100, 60), 1.0, &mut seq, &mut rng);
    p.lines[0].slated = true;
    p.lines[1].slated = true;
    let before = rng.clone();
    let mut probe = before.clone();
    let rolls = probe.value() < 0.1;
    p.update(0.25, true, &mut seq, &mut rng);
    // 15 lines, two gone: 13 < 15 adds one (side, direction, then the speed: 3 draws).
    assert_eq!(p.scan_lines.len(), 14);
    assert_eq!(p.lines.len(), 16);
    assert_eq!(drawn(&before, &rng), 1 + usize::from(rolls) + 3);
    assert!(!p.slated);
    if rolls {
        assert_eq!(p.visible, probe.value() < 0.25);
    }
}

#[test]
fn update_without_a_viewer_destroys_the_lines_it_counts() {
    let mut seq = 0;
    let mut rng = UnityRandom::new(9);
    let mut p = ProjectedScanLines::new((100, 60), 1.0, &mut seq, &mut rng);
    p.update(1.0, false, &mut seq, &mut rng);
    assert!(p.slated);
    assert!(p.lines.iter().all(|l| l.slated));
    p.remove_slated();
    assert!(p.lines.is_empty());
}

#[test]
fn a_line_dies_200_pixels_outside() {
    let room = Vec2::new(2000.0, 1200.0);
    let mut line = ProjectedScanLine {
        update_order: 1,
        slated: false,
        alignment: true,
        pos: 2100.0,
        last_pos: 2099.0,
        speed: 1.5,
    };
    line.update(room);
    assert!(!line.slated);
    assert_eq!((line.last_pos, line.pos), (2100.0, 2101.5));
    line.pos = 2300.0;
    line.last_pos = 2300.0;
    line.update(room);
    assert!(line.slated);
    // Along the height, below -200 on two ticks running.
    let mut line = ProjectedScanLine {
        alignment: false,
        pos: -199.0,
        last_pos: -199.0,
        speed: -1.0,
        slated: false,
        ..line
    };
    line.update(room);
    assert!(!line.slated);
    line.update(room);
    assert!(!line.slated);
    line.update(room);
    assert!(line.slated);
}
