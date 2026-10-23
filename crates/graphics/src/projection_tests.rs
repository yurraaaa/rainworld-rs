//! The sprites of the superstructure projections. Expected values follow
//! `SuperStructureProjector.cs`, `ProjectedScanLines.cs` and `StarMatrix.cs`
//! (`InitiateSprites`, `DrawSprites`).
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{
    Cursor, Glyph, GlyphMatrix, PartKind, ProjectedScanLines, ProjectorPart, ScanLine, SingleGlyph,
    StarMatrix, SuperStructureProjector, Vec2,
};

use crate::draw::{Color, DrawItem, Layer, Shader, Sprite};
use crate::{ProjectedScanLinesSprites, ProjectionView, ProjectorSprites, StarMatrixSprites};

const VIEW: ProjectionView = ProjectionView {
    cam_pos: Vec2::new(100.0, 40.0),
    camera_position: Vec2::new(60.0, 20.0),
};

/// A projector with nothing in it over a 40 x 30 tile room.
fn projector() -> SuperStructureProjector {
    let mut seq = 0;
    SuperStructureProjector::new((40, 30), 0.0, &mut seq, &mut UnityRandom::new(1))
}

fn with_part(p: &mut SuperStructureProjector, kind: PartKind) -> u64 {
    let update_order = 1000 + p.parts.len() as u64;
    p.parts.push(ProjectorPart {
        update_order,
        slated: false,
        kind,
    });
    update_order
}

fn sprites(items: &[DrawItem]) -> Vec<Sprite> {
    items
        .iter()
        .map(|i| match i {
            DrawItem::Sprite(s) => *s,
            other => panic!("{other:?}"),
        })
        .collect()
}

/// Two `pixel` bars 15 px apart; the vertical line's y is the stage's bottom (0 in the
/// game's stage pixels, so the camera's y in the room), anchored at its bottom.
#[test]
fn a_vertical_scan_line_is_two_bars_from_the_stage_bottom() {
    let mut p = projector();
    let order = with_part(
        &mut p,
        PartKind::ScanLine(ScanLine {
            alignment: true,
            pos: 520.0,
            last_pos: 510.0,
            speed: 10.0,
        }),
    );
    let list = p.draw_part(order, 0.5, &VIEW);
    let s = sprites(&list.items);
    assert_eq!(s.len(), 2);
    for (j, sprite) in s.iter().enumerate() {
        // x = Lerp(lastPos, pos, 0.5) - camPos.x + 15 j, in stage pixels.
        assert_eq!(
            sprite.pos,
            Vec2::new(515.0 - 100.0 + 15.0 * j as f32 + 100.0, 40.0)
        );
        assert_eq!(sprite.scale, Vec2::new(2.0, 768.0));
        assert_eq!(sprite.anchor, Vec2::new(0.5, 0.0));
        assert_eq!(
            (
                sprite.element,
                sprite.color,
                sprite.layer,
                sprite.shader,
                sprite.index
            ),
            ("pixel", Color::BLACK, Layer::Shadows, Shader::Basic, j)
        );
    }
}

#[test]
fn a_horizontal_scan_line_runs_from_the_stage_left() {
    let mut p = projector();
    let order = with_part(
        &mut p,
        PartKind::ScanLine(ScanLine {
            alignment: false,
            pos: 300.0,
            last_pos: 300.0,
            speed: 0.0,
        }),
    );
    let s = sprites(&p.draw_part(order, 0.0, &VIEW).items);
    assert_eq!(s[1].pos, Vec2::new(100.0, 300.0 - 40.0 + 15.0 + 40.0));
    assert_eq!(s[1].scale, Vec2::new(1366.0, 2.0));
    assert_eq!(s[1].anchor, Vec2::new(0.0, 0.5));
}

#[test]
fn nothing_is_drawn_while_the_projector_is_dark_or_the_part_is_gone() {
    let mut p = projector();
    let order = with_part(
        &mut p,
        PartKind::ScanLine(ScanLine {
            alignment: true,
            pos: 1.0,
            last_pos: 1.0,
            speed: 1.0,
        }),
    );
    p.visible = false;
    assert!(p.draw_part(order, 0.5, &VIEW).items.is_empty());
    p.visible = true;
    p.parts[0].slated = true;
    assert!(p.draw_part(order, 0.5, &VIEW).items.is_empty());
    assert!(p.draw_part(99, 0.5, &VIEW).items.is_empty());
}

/// Four bars boxing `15 * size` px and the white `glyphs` sprite scaled `15 size / 750` by
/// `size`.
#[test]
fn a_cursor_is_four_bars_and_a_glyph_sprite() {
    let mut p = projector();
    let order = with_part(
        &mut p,
        PartKind::Cursor(Cursor {
            g_pos: (3, 4),
            pos: Vec2::new(250.0, 200.0),
            last_pos: Vec2::new(240.0, 190.0),
            size: 3,
        }),
    );
    let s = sprites(&p.draw_part(order, 0.5, &VIEW).items);
    assert_eq!(s.len(), 5);
    // (245, 195) in the room, (145, 155) on the stage.
    assert_eq!(s[0].pos, Vec2::new(245.0, 40.0));
    assert_eq!(s[1].pos, Vec2::new(245.0 + 45.0, 40.0));
    assert_eq!(s[2].pos, Vec2::new(100.0, 195.0));
    assert_eq!(s[3].pos, Vec2::new(100.0, 195.0 + 45.0));
    assert_eq!(s[0].scale, Vec2::new(2.0, 768.0));
    assert_eq!(s[2].scale, Vec2::new(1366.0, 2.0));
    assert!(s[..4].iter().all(|b| b.anchor == Vec2::ZERO));
    let glyphs = s[4];
    assert_eq!(glyphs.element, "glyphs");
    assert_eq!(glyphs.shader, Shader::GlyphProjection);
    assert_eq!(glyphs.pos, Vec2::new(245.0, 195.0));
    assert_eq!(glyphs.scale, Vec2::new(45.0 / 750.0, 3.0));
    assert_eq!(glyphs.color, Color::WHITE);
    assert_eq!(glyphs.anchor, Vec2::ZERO);
}

fn single(selected: bool, inactive: bool) -> SingleGlyph {
    SingleGlyph {
        glyph: Glyph {
            pos: (4, 2),
            glyph_col: 0.3,
            life: 100,
            counter: 0,
        },
        procreate: -1,
        selected: !selected,
        sprite_selected: selected,
        inactive,
    }
}

/// The colour is the one the leaser was made with, not the current `selected`.
#[test]
fn a_single_glyph_is_one_cell_with_its_leaser_colour() {
    let mut p = projector();
    p.last_grid_pos = Vec2::new(1.0, 2.0);
    p.grid_pos = Vec2::new(3.0, 6.0);
    let order = with_part(&mut p, PartKind::Single(single(true, false)));
    let s = sprites(&p.draw_part(order, 0.5, &VIEW).items);
    assert_eq!(s.len(), 1);
    // GridPos(pos, 0.5) + cameraPositions[current].
    assert_eq!(
        s[0].pos,
        Vec2::new(4.0 * 15.0 + 2.0 + 60.0, 2.0 * 15.0 + 4.0 + 20.0)
    );
    assert_eq!(s[0].scale, Vec2::new(0.02, 1.0));
    assert_eq!(s[0].anchor, Vec2::ZERO);
    assert_eq!(s[0].color, Color::rgb(1.0, 0.0, 0.0));
    assert_eq!(s[0].shader, Shader::GlyphProjection);
    let order = with_part(&mut p, PartKind::Single(single(false, false)));
    let s = sprites(&p.draw_part(order, 0.5, &VIEW).items);
    assert_eq!(s[0].color, Color::rgb(0.0, 0.0, 0.0));
    let order = with_part(&mut p, PartKind::Single(single(true, true)));
    assert!(p.draw_part(order, 0.5, &VIEW).items.is_empty());
}

/// An odd size is shifted half a cell down and left; the alpha is half the life left.
#[test]
fn a_matrix_is_centred_on_its_cell_and_fades() {
    let mut p = projector();
    let order = with_part(
        &mut p,
        PartKind::Matrix(GlyphMatrix {
            glyph: Glyph {
                pos: (4, 2),
                glyph_col: 0.25,
                life: 200,
                counter: 50,
            },
            size: (3, 4),
            max_size: (5, 5),
        }),
    );
    let s = sprites(&p.draw_part(order, 0.0, &VIEW).items);
    assert_eq!(s[0].pos, Vec2::new(60.0 + 60.0 - 7.5, 30.0 + 20.0));
    assert_eq!(s[0].scale, Vec2::new(3.0 * 15.0 / 750.0, 4.0));
    assert_eq!(s[0].anchor, Vec2::new(0.5, 0.5));
    // InverseLerp(life, 0, counter) = 0.75, times 0.5.
    assert_eq!(s[0].color, Color::rgb(0.0, 0.0, 0.25).with_alpha(0.375));
}

#[test]
fn a_scan_line_of_the_projected_effect_is_one_bar() {
    let mut seq = 0;
    let mut rng = UnityRandom::new(3);
    let mut lines = ProjectedScanLines::new((100, 60), 1.0, &mut seq, &mut rng);
    let line = lines.lines[0];
    let list = lines.draw_line(line.update_order, 0.25, VIEW.cam_pos);
    let s = sprites(&list.items);
    assert_eq!(s.len(), 1);
    let along = line.last_pos + (line.pos - line.last_pos) * 0.25;
    if line.alignment {
        assert_eq!(s[0].scale, Vec2::new(2.0, 768.0));
        assert_eq!(s[0].anchor, Vec2::new(0.5, 0.0));
        assert!((s[0].pos.x - along).abs() < 1e-3 && s[0].pos.y == 40.0);
    } else {
        assert_eq!(s[0].scale, Vec2::new(1366.0, 2.0));
        assert_eq!(s[0].anchor, Vec2::new(0.0, 0.5));
        assert!((s[0].pos.y - along).abs() < 1e-3 && s[0].pos.x == 100.0);
    }
    assert_eq!((s[0].color, s[0].layer), (Color::BLACK, Layer::Shadows));
    lines.visible = false;
    assert!(
        lines
            .draw_line(line.update_order, 0.25, VIEW.cam_pos)
            .items
            .is_empty()
    );
}

/// Sprite indices run on star after star: the star, its lit connections, its rings.
#[test]
fn a_star_matrix_draws_squares_lines_and_rings() {
    let mut rng = UnityRandom::new(11);
    let mut m = StarMatrix::new(0, Vec2::new(500.0, 300.0), 150.0, &mut rng);
    let list = m.draw(0.0);
    let s = sprites(&list.items);
    let lit: usize = m
        .stars
        .iter()
        .map(|st| 1 + st.connections_on.iter().filter(|&&on| on).count() + st.circles)
        .sum();
    assert_eq!(s.len(), lit);
    let star = &m.stars[0];
    // Each star's square sits at its plane position times `rad`, about `pos`.
    let at = m.on_plane_pos(0, 0.0) * m.rad + m.pos;
    let square = s.iter().find(|x| x.index == star.first_sprite).unwrap();
    assert_eq!(square.pos, at);
    assert_eq!(
        (square.scale, square.element),
        (Vec2::new(5.0, 5.0), "pixel")
    );
    assert!(
        s.iter()
            .all(|x| x.layer == Layer::BackgroundShortcuts && x.color.r == 0.0)
    );
    // A star with a connection and a ring.
    let (i, st) = m
        .stars
        .iter()
        .enumerate()
        .find(|(_, st)| !st.connections.is_empty() && st.circles > 0)
        .expect("a connected star with rings");
    let (first_sprite, rings, connections) = (
        st.first_sprite,
        st.connections.len(),
        st.connections.clone(),
    );
    let here = m.on_plane_pos(i, 0.0) * m.rad + m.pos;
    let other = m.on_plane_pos(connections[0], 0.0) * m.rad + m.pos;
    // Switched off, it is not drawn; on, it points at the other star and spans the gap.
    m.stars[i].connections_on[0] = false;
    let off = sprites(&m.draw(0.0).items);
    assert!(off.iter().all(|x| x.index != first_sprite + 1));
    m.stars[i].connections_on[0] = true;
    let on = sprites(&m.draw(0.0).items);
    let line = on.iter().find(|x| x.index == first_sprite + 1).unwrap();
    assert_eq!(line.rotation, (other - here).to_degrees());
    assert_eq!(line.scale, Vec2::new(1.5, here.distance(other)));
    assert_eq!(line.anchor, Vec2::new(0.5, 0.0));
    // Rings: Futile_White scaled (8 + 5j) / 8 of alpha 2 / (8 + 5j) under VectorCircle.
    let ring = on
        .iter()
        .find(|x| x.index == first_sprite + 1 + rings)
        .unwrap();
    assert_eq!(ring.element, "Futile_White");
    assert_eq!(ring.shader, Shader::VectorCircle);
    assert_eq!(ring.scale, Vec2::new(1.0, 1.0));
    assert_eq!(ring.color.a, 0.25);
}
