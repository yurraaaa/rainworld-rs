//! `SuperStructureProjector` and its parts: draw counts and orders per function, the ideal
//! counts, the grid, the lifetime of glyphs. Expected values follow the decompiled
//! `SuperStructureProjector.cs`.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{
    Cursor, GRID_SIZE, Glyph, GlyphMatrix, PartKind, ProjectorFrame, ProjectorPart, ScanLine,
    SingleGlyph, SuperStructureProjector,
};
use crate::Vec2;

/// How many words the generator moved between `before` and `after`.
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

const VIEWED: ProjectorFrame = ProjectorFrame {
    electric_power: 1.0,
    being_viewed: true,
    camera_position: None,
};

/// A projector over a room of `tiles` with nothing wanted (`amount` 0): no scan lines,
/// glyphs or cursors, so the lists are filled by hand.
fn empty(tiles: (i32, i32)) -> SuperStructureProjector {
    let mut seq = 0;
    SuperStructureProjector::new(tiles, 0.0, &mut seq, &mut UnityRandom::new(1))
}

fn single_at(p: &mut SuperStructureProjector, pos: (i32, i32), life: i32) -> u64 {
    let order = 100 + p.parts.len() as u64;
    p.parts.push(ProjectorPart {
        update_order: order,
        slated: false,
        kind: PartKind::Single(SingleGlyph {
            glyph: Glyph {
                pos,
                glyph_col: 0.5,
                life,
                counter: 0,
            },
            procreate: -1,
            selected: false,
            sprite_selected: false,
            inactive: false,
        }),
    });
    p.glyphs_list.push(order);
    p.set_grid(pos, Some(order));
    order
}

fn single(p: &SuperStructureProjector, order: u64) -> SingleGlyph {
    match p.parts[p.part_index(order).unwrap()].kind {
        PartKind::Single(s) => s,
        k => panic!("{k:?}"),
    }
}

#[test]
fn ideal_numbers_follow_the_room_and_the_amount() {
    let mut p = empty((100, 60));
    p.amount = 1.0;
    // (int)(520 * 1); Lerp(6000, 100, 0.5) = 3050 -> 3050 / 2000 * 10 = 15.25;
    // Max(1, (int)(6000 / 2000 * 1.2)) = 3.
    assert_eq!(p.ideal_glyph_number(), 520);
    assert_eq!(p.ideal_scan_line_number(), 15);
    assert_eq!(p.ideal_cursor_number(), 3);
    // A faint effect: no cursor below amount 0.1 unless the room is big enough for one.
    p.amount = 0.05;
    assert_eq!(p.ideal_glyph_number(), 26);
    p.tile_size = (10, 10);
    assert_eq!(p.ideal_cursor_number(), 0);
    p.amount = 0.2;
    assert_eq!(p.ideal_cursor_number(), 1);
}

#[test]
fn scan_line_constructor_draws_once_for_its_speed() {
    let before = UnityRandom::new(4);
    let mut rng = before.clone();
    let line = ScanLine::new(true, 300.0, -1.0, &mut rng);
    assert_eq!(drawn(&before, &rng), 1);
    let mut probe = before.clone();
    let speed = 1.0 + 6.0 * probe.value();
    assert_eq!(line.speed, -speed);
    // pos = pos + Sign(speed), lastPos = pos - Sign(speed).
    assert_eq!((line.pos, line.last_pos), (299.0, 301.0));
}

#[test]
fn scan_line_follows_the_grid_and_dies_beyond_200_pixels() {
    let room = Vec2::new(2000.0, 1000.0);
    let mut line = ScanLine {
        alignment: true,
        pos: 10.0,
        last_pos: 9.0,
        speed: 2.0,
    };
    assert!(!line.update(Vec2::new(3.0, 50.0), room));
    // pos += speed, then the grid's movement along x.
    assert_eq!((line.last_pos, line.pos), (10.0, 15.0));
    // Past the far edge by 200 on both ticks, moving away.
    line.pos = 2300.0;
    line.last_pos = 2300.0;
    assert!(line.update(Vec2::ZERO, room));
    // The other axis tests the height.
    let mut line = ScanLine {
        alignment: false,
        pos: 1250.0,
        last_pos: 1250.0,
        speed: 1.0,
    };
    assert!(line.update(Vec2::ZERO, room));
    line.pos = -300.0;
    line.last_pos = -300.0;
    line.speed = -1.0;
    assert!(line.update(Vec2::ZERO, room));
    // Heading back in, it lives.
    line.speed = 1.0;
    assert!(!line.update(Vec2::ZERO, room));
}

/// The constructor: per scan line 3 draws and the line's 1, per random glyph the cell (2),
/// the matrix roll (1) and the glyph's own, then `intOffset` (2).
#[test]
fn constructor_draws() {
    let before = UnityRandom::new(7);
    let mut rng = before.clone();
    let mut seq = 0;
    let p = SuperStructureProjector::new((100, 60), 1.0, &mut seq, &mut rng);
    assert_eq!(p.scan_lines.len(), 15);
    let mut want = 15 * 4 + 2;
    let (mut singles, mut matrices) = (0, 0);
    for part in &p.parts {
        match part.kind {
            PartKind::Single(s) => {
                singles += 1;
                // Glyph: color, life; Reset: life, spreads?, [when], selected.
                want += 2 + 1 + 2 + 3 + usize::from(s.procreate != -1);
            }
            PartKind::Matrix(m) => {
                matrices += 1;
                // Glyph: color, life; sizes: two maxima, a size per maximum above 1, life.
                want += 2
                    + 1
                    + 2
                    + 2
                    + usize::from(m.max_size.0 > 1)
                    + usize::from(m.max_size.1 > 1)
                    + 1;
            }
            _ => {}
        }
    }
    assert_eq!(singles + matrices, 260);
    assert!(matrices > 0, "seed 7 should roll a matrix");
    assert_eq!(drawn(&before, &rng), want);
    assert!(p.int_offset.0 >= 100 && p.int_offset.0 < 1000);
    // The parts took their places first, the projector last.
    assert_eq!(p.update_order, seq);
    assert!(p.parts.iter().all(|q| q.update_order < p.update_order));
    assert!(
        p.parts
            .windows(2)
            .all(|w| w[0].update_order < w[1].update_order)
    );
    assert_eq!(p.glyphs_list.len(), 260);
    assert!(p.visible);
}

#[test]
fn a_new_single_glyph_draws_five_or_six() {
    for seed in 0..40 {
        let before = UnityRandom::new(seed);
        let mut rng = before.clone();
        let mut p = empty((20, 10));
        let mut seq = 50;
        p.add_single_glyph_at((3, 4), &mut seq, &mut rng);
        let s = single(&p, 51);
        assert_eq!(drawn(&before, &rng), 5 + usize::from(s.procreate != -1));
        assert_eq!(p.grid_at((3, 4)), Some(51));
        assert_eq!(s.sprite_selected, s.selected);
        assert!((10..600).contains(&s.glyph.life) && s.glyph.counter == 0);
        if s.procreate != -1 {
            assert!((1..s.glyph.life).contains(&s.procreate));
        }
    }
}

/// `AddSingleGlyphAt` takes the first glyph of the waiting room and `Reset`s it, which
/// does not move its `pos` (kept from the game): the grid holds it at the new cell.
#[test]
fn a_waiting_glyph_is_reused_and_keeps_its_old_pos() {
    let mut p = empty((20, 10));
    let order = single_at(&mut p, (5, 5), 10);
    p.parts[0].kind = match p.parts[0].kind {
        PartKind::Single(mut s) => {
            s.inactive = true;
            s.sprite_selected = true;
            PartKind::Single(s)
        }
        k => k,
    };
    p.glyphs_list.clear();
    p.set_grid((5, 5), None);
    p.inactive_glyphs_waiting_room.push(order);
    let before = UnityRandom::new(3);
    let mut rng = before.clone();
    let mut seq = 500;
    p.add_single_glyph_at((8, 2), &mut seq, &mut rng);
    let s = single(&p, order);
    // Reset: life, spreads?, [when], selected: no color or Range(20, 200).
    assert_eq!(drawn(&before, &rng), 3 + usize::from(s.procreate != -1));
    assert!(!s.inactive);
    assert_eq!(s.glyph.pos, (5, 5));
    assert_eq!(p.grid_at((8, 2)), Some(order));
    assert_eq!(p.glyphs_list, [order]);
    assert!(p.inactive_glyphs_waiting_room.is_empty());
    // The leaser's colour was set when it was made.
    assert!(s.sprite_selected);
    assert_eq!(seq, 500);
}

#[test]
fn a_single_glyph_goes_inactive_at_the_end_of_its_life() {
    let mut p = empty((20, 10));
    let order = single_at(&mut p, (5, 5), 3);
    let mut seq = 0;
    let mut rng = UnityRandom::new(2);
    for tick in 1..=3 {
        p.update_part(order, &mut seq, &mut rng);
        assert_eq!(single(&p, order).inactive, tick == 3);
    }
    assert_eq!(p.inactive_glyphs_waiting_room, [order]);
    assert_eq!(p.grid_at((5, 5)), None);
    // Still in glyphsList until the projector's next update takes it out.
    assert_eq!(p.glyphs_list, [order]);
    // An inactive glyph does nothing, draws nothing.
    let before = rng.clone();
    p.update_part(order, &mut seq, &mut rng);
    assert_eq!(rng, before);
    p.update(&VIEWED, &mut seq, &mut rng);
    assert!(p.glyphs_list.is_empty());
    assert!(p.part_index(order).is_some());
}

/// `counter == 10 && value > 1/3 && glyphsList.Count < ideal`: the draw is made at counter
/// 10 only; a spreading glyph at its `procreate` tick draws only `Range(0, free)`.
#[test]
fn procreation_draws_follow_the_conditions() {
    let mut p = empty((20, 10));
    p.amount = 1.0;
    let order = single_at(&mut p, (5, 5), 500);
    let mut seq = 1000;
    let mut rng = UnityRandom::new(9);
    for _ in 0..9 {
        let before = rng.clone();
        p.update_part(order, &mut seq, &mut rng);
        assert_eq!(rng, before);
    }
    // Counter 10: one draw for the chance; a new glyph only when it rolls above 1/3.
    let before = rng.clone();
    let mut roll = before.clone();
    let chance = roll.value();
    p.update_part(order, &mut seq, &mut rng);
    if chance > 1.0 / 3.0 {
        // Range(0, 4): four free neighbours, then the new glyph's 5 or 6.
        let made = single(&p, 1001);
        assert_eq!(
            drawn(&before, &rng),
            1 + 1 + 5 + usize::from(made.procreate != -1)
        );
        assert_eq!(p.glyphs_list.len(), 2);
    } else {
        assert_eq!(drawn(&before, &rng), 1);
        assert_eq!(p.glyphs_list.len(), 1);
    }
}

/// A glyph at `procreate` spreads into a free neighbour of `Custom.fourDirections`'s
/// order, drawing `Range(0, count)`; occupied cells and the grid's edge are skipped.
#[test]
fn procreation_picks_among_free_neighbours() {
    let mut p = empty((20, 10));
    let order = single_at(&mut p, (0, 0), 500);
    // (-1, 0) is off the grid, (0, -1) too; (1, 0) is taken; only (0, 1) is free.
    let taken = single_at(&mut p, (1, 0), 500);
    assert_ne!(order, taken);
    let before = UnityRandom::new(5);
    let mut rng = before.clone();
    let mut seq = 2000;
    p.procreate((0, 0), &mut seq, &mut rng);
    assert_eq!(p.grid_at((0, 1)), Some(2001));
    let made = single(&p, 2001);
    assert_eq!(
        drawn(&before, &rng),
        1 + 5 + usize::from(made.procreate != -1)
    );
    // With nothing free there is no draw at all.
    let mut p = empty((20, 10));
    single_at(&mut p, (0, 0), 500);
    single_at(&mut p, (1, 0), 500);
    single_at(&mut p, (0, 1), 500);
    let before = rng.clone();
    p.procreate((0, 0), &mut seq, &mut rng);
    assert_eq!(rng, before);
}

#[test]
fn a_matrix_is_destroyed_at_the_end_of_its_life_and_its_size_stays_in_range() {
    for seed in 0..30 {
        let mut p = empty((20, 10));
        p.amount = 1.0;
        let before = UnityRandom::new(seed);
        let mut rng = before.clone();
        let glyph = Glyph {
            pos: (4, 4),
            glyph_col: 0.2,
            life: 200,
            counter: 0,
        };
        let m = GlyphMatrix::new(glyph, 1.0, &mut rng);
        assert_eq!(
            drawn(&before, &rng),
            2 + usize::from(m.max_size.0 > 1) + usize::from(m.max_size.1 > 1) + 1
        );
        assert!((10..600).contains(&m.glyph.life));
        let order = 100;
        p.parts.push(ProjectorPart {
            update_order: order,
            slated: false,
            kind: PartKind::Matrix(m),
        });
        let mut seq = 0;
        for _ in 0..m.glyph.life {
            p.update_part(order, &mut seq, &mut rng);
            if p.parts[0].slated {
                break;
            }
            let PartKind::Matrix(now) = p.parts[0].kind else {
                panic!();
            };
            assert!((1..=now.max_size.0.max(1)).contains(&now.size.0));
            assert!((1..=now.max_size.1.max(1)).contains(&now.size.1));
        }
        assert!(p.parts[0].slated);
        assert_eq!(p.parts[0].kind_counter(), m.glyph.life);
    }
}

impl ProjectorPart {
    fn kind_counter(&self) -> i32 {
        match self.kind {
            PartKind::Matrix(m) => m.glyph.counter,
            PartKind::Single(s) => s.glyph.counter,
            _ => 0,
        }
    }
}

#[test]
fn a_cursor_walks_its_grid_and_dies_outside_the_room() {
    let mut p = empty((100, 60));
    p.amount = 0.0;
    let before = UnityRandom::new(8);
    let mut rng = before.clone();
    let mut seq = 0;
    p.add_cursor(&mut seq, &mut rng);
    // Two coordinates and the size.
    assert_eq!(drawn(&before, &rng), 3);
    let order = p.cursors[0];
    let PartKind::Cursor(c) = p.parts[p.part_index(order).unwrap()].kind else {
        panic!();
    };
    assert!((1..4).contains(&c.size));
    assert_eq!(c.pos, c.last_pos);
    assert_eq!(
        c.pos,
        Vec2::new(c.g_pos.0 as f32 * 15.0, c.g_pos.1 as f32 * 15.0)
    );
    // Pulled 12 pixels a tick toward the cell, a random step on each axis 1 time in 10.
    let mut cursor = Cursor {
        g_pos: (10, 10),
        pos: Vec2::new(0.0, 150.0),
        last_pos: Vec2::ZERO,
        size: 2,
    };
    p.parts.clear();
    p.parts.push(ProjectorPart {
        update_order: 7,
        slated: false,
        kind: PartKind::Cursor(cursor),
    });
    let before = rng.clone();
    p.update_cursor(0, &mut rng);
    let PartKind::Cursor(now) = p.parts[0].kind else {
        panic!();
    };
    cursor = now;
    assert_eq!(cursor.last_pos, Vec2::new(0.0, 150.0));
    assert_eq!(cursor.pos, Vec2::new(12.0, 150.0));
    let mut probe = before.clone();
    let mut want = 2;
    for _ in 0..2 {
        if probe.value() < 0.1 {
            want += 1;
            probe.range_i32(-1, 2);
        }
    }
    assert_eq!(drawn(&before, &rng), want);
    // Outside the room's cells it is destroyed.
    p.parts[0].kind = PartKind::Cursor(Cursor {
        g_pos: (-5, 3),
        ..cursor
    });
    p.update_cursor(0, &mut rng);
    assert!(p.parts[0].slated);
}

/// The projector's own turn: `gridSpeed` and `RNV()` (1), the visibility roll (1, +1),
/// nothing wanted refilled.
#[test]
fn update_draws_without_refills() {
    let mut p = empty((20, 10));
    let mut seq = 10;
    for seed in 0..20 {
        let before = UnityRandom::new(seed);
        let mut rng = before.clone();
        let mut probe = before.clone();
        probe.value();
        let rolls = probe.value() < 0.1;
        let power = 0.5;
        p.update(
            &ProjectorFrame {
                electric_power: power,
                ..VIEWED
            },
            &mut seq,
            &mut rng,
        );
        assert_eq!(drawn(&before, &rng), 2 + usize::from(rolls), "seed {seed}");
        assert!(!p.slated);
    }
    assert_eq!(seq, 10);
    // Not viewed: it destroys itself (and its parts).
    let mut rng = UnityRandom::new(1);
    let order = single_at(&mut p, (2, 2), 50);
    p.update(
        &ProjectorFrame {
            being_viewed: false,
            ..VIEWED
        },
        &mut seq,
        &mut rng,
    );
    assert!(p.slated);
    assert!(p.parts.iter().all(|q| q.slated));
    assert_eq!(p.grid_at((2, 2)), None);
    // A slated part does not update.
    let before = rng.clone();
    p.update_part(order, &mut seq, &mut rng);
    assert_eq!(rng, before);
}

/// With every list empty and the ideal counts above zero, one tick adds one scan line (3
/// draws), one glyph and one cursor (3 draws).
#[test]
fn update_refills_one_of_each() {
    let before = UnityRandom::new(12);
    let mut rng = before.clone();
    let mut seq = 0;
    let mut p = SuperStructureProjector::new((100, 60), 1.0, &mut seq, &mut rng);
    for part in &mut p.parts {
        part.slated = true;
    }
    p.prune();
    p.scan_lines.clear();
    p.glyphs_list.clear();
    let before = rng.clone();
    let mut probe = before.clone();
    probe.value();
    let base = 2 + usize::from(probe.value() < 0.1);
    p.update(&VIEWED, &mut seq, &mut rng);
    assert_eq!(p.scan_lines.len(), 1);
    assert_eq!(p.glyphs_list.len(), 1);
    assert_eq!(p.cursors.len(), 1);
    let glyph = match p.parts[p.part_index(p.glyphs_list[0]).unwrap()].kind {
        PartKind::Single(s) => 2 + 1 + 5 + usize::from(s.procreate != -1),
        PartKind::Matrix(m) => {
            2 + 1 + 2 + 2 + usize::from(m.max_size.0 > 1) + usize::from(m.max_size.1 > 1) + 1
        }
        k => panic!("{k:?}"),
    };
    assert_eq!(drawn(&before, &rng), base + 3 + glyph + 3);
}

/// The grid moves when `gridPos` passes 15 in a direction: glyphs by 2 cells (wrapping),
/// `gridPos` back by 30, `intOffset` by 2.
#[test]
fn the_grid_shifts_by_two_cells() {
    let mut p = empty((20, 10));
    let a = single_at(&mut p, (94, 3), 500);
    let b = single_at(&mut p, (10, 0), 500);
    p.grid_pos = Vec2::new(16.0, -16.0);
    p.last_grid_pos = Vec2::new(16.0, -16.0);
    p.grid_speed = Vec2::ZERO;
    p.int_offset = (500, 500);
    let mut seq = 0;
    let mut rng = UnityRandom::new(3);
    p.update(&VIEWED, &mut seq, &mut rng);
    // RNV added to the (zero) speed after the move, so the grid just moved by 0 and then
    // was wrapped: x > 15 -> -14, y < -15 -> +14.
    assert_eq!(single(&p, a).glyph.pos, (96 - GRID_SIZE.0, 3 - 2));
    assert_eq!(single(&p, b).glyph.pos, (12, 53));
    assert_eq!(p.int_offset, (502, 498));
    assert_eq!(p.grid_pos, Vec2::new(-14.0, 14.0));
    assert_eq!(p.grid_at((1, 1)), Some(a));
    assert_eq!(p.grid_at((12, 53)), Some(b));
    assert_eq!(p.grid_at((94, 3)), None);
}

/// `ScrambleAllGlyphs`: two draws per glyph, the cell cleared only while the old `pos` is
/// strictly inside (`> 0`).
#[test]
fn scrambling_draws_two_per_glyph_and_tests_pos_above_zero() {
    let mut p = empty((20, 10));
    let a = single_at(&mut p, (5, 5), 500);
    let edge = single_at(&mut p, (0, 7), 500);
    let before = UnityRandom::new(6);
    let mut rng = before.clone();
    p.scramble_all_glyphs(&mut rng);
    assert_eq!(drawn(&before, &rng), 4);
    let mut probe = before.clone();
    let a_new = (probe.range_i32(0, 95), probe.range_i32(0, 55));
    let edge_new = (probe.range_i32(0, 95), probe.range_i32(0, 55));
    assert_eq!(single(&p, a).glyph.pos, a_new);
    assert_eq!(single(&p, edge).glyph.pos, edge_new);
    assert_eq!(p.grid_at(a_new), Some(a));
    assert_eq!(p.grid_at(edge_new), Some(edge));
    // The glyph at x = 0 kept its old cell in the grid.
    assert_eq!(p.grid_at((0, 7)), Some(edge));
    assert_eq!(p.grid_at((5, 5)), None);
}

#[test]
fn a_camera_position_change_scrambles_once() {
    let mut p = empty((20, 10));
    single_at(&mut p, (5, 5), 500);
    let mut seq = 0;
    let mut rng = UnityRandom::new(10);
    let frame = ProjectorFrame {
        camera_position: Some(2),
        ..VIEWED
    };
    let before = rng.clone();
    let mut probe = before.clone();
    probe.value();
    let rolls = probe.value() < 0.1;
    p.update(&frame, &mut seq, &mut rng);
    assert_eq!(p.last_cam_pos, 2);
    assert_eq!(drawn(&before, &rng), 2 + usize::from(rolls) + 2);
    let before = rng.clone();
    let mut probe = before.clone();
    probe.value();
    let rolls = probe.value() < 0.1;
    p.update(&frame, &mut seq, &mut rng);
    assert_eq!(drawn(&before, &rng), 2 + usize::from(rolls));
}

#[test]
fn grid_positions_and_the_shader_offset() {
    let mut p = empty((20, 10));
    p.last_grid_pos = Vec2::new(2.0, 4.0);
    p.grid_pos = Vec2::new(4.0, 8.0);
    p.int_offset = (100, 200);
    assert_eq!(p.grid_pos_at((3, 2), 0.5), Vec2::new(48.0, 36.0));
    // Lerp(last, grid, t) + (0, -1) + 15 * intOffset.
    assert_eq!(
        p.grid_offset(0.5),
        Vec2::new(3.0 + 1500.0, 6.0 - 1.0 + 3000.0)
    );
    assert_eq!(p.entire_room_size, (400 / 15 + 1, 200 / 15 + 1));
}
