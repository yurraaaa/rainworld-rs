//! Overseer hologram drawing. Expected values are worked out by hand from the
//! decompiled constructors and `DrawSprites` (`OverseerHolograms`,
//! `OverseerTutorialBehavior`; derivations next to each assertion), not from another run
//! of this code.

use rw_random::UnityRandom;
use rw_sim::creature::overseer::OverseerColor;
use rw_sim::creature::overseer::hologram::{
    ButtonState, HoloColor, HologramKind, HologramMessage, HologramPart, OverseerHologram,
    PartColor, PartKind, SymbolPart,
};
use rw_sim::{Vec2, Vec3};

use crate::{
    Atlas, Color, DrawItem, HologramFrame, HologramGraphics, HologramLine, Layer, Mathf,
    OverseerGraphics, PartShape, Shader, UnityVector3,
};

const EPS: f32 = 1e-3;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < EPS
}

fn v3(a: Vec3, b: [f32; 3]) -> bool {
    close(a.x, b[0]) && close(a.y, b[1]) && close(a.z, b[2])
}

fn shape(kind: PartKind) -> PartShape {
    let part = HologramPart::new(kind);
    PartShape::of(&part, std::slice::from_ref(&part))
}

fn key_shape(wider: f32) -> PartShape {
    shape(PartKind::Key {
        button: ButtonState::new("Z"),
        wider,
    })
}

fn hologram(parts: Vec<HologramPart>) -> OverseerHologram {
    OverseerHologram {
        id: 1,
        message: HologramMessage::Shelter,
        kind: HologramKind::Angry {
            counter: 0,
            show_time: 0,
        },
        room: 0,
        update_order: 0,
        pos: Vec2::new(683.0, 512.0),
        last_pos: Vec2::new(683.0, 512.0),
        look_at_communication_creature: false,
        look_at_comm_crit_counter: 0,
        parts,
        display_tile: (0, 0),
        temp_display_tile: (0, 0),
        fade: 1.0,
        last_fade: 1.0,
        still_relevant: true,
        importance: 1.0,
        push_around_pos: Vec2::ZERO,
        overseer_sit_still: false,
        slated: false,
    }
}

/// The game's camera at the room origin on a 1366 x 768 screen: (683, 512) is the
/// centre `ApplyDepthOnVector` scales from.
fn frame() -> HologramFrame {
    HologramFrame {
        t: 1.0,
        cam_pos: Vec2::ZERO,
        screen_size: Vec2::new(1366.0, 768.0),
        head: Vec2::new(600.0, 400.0),
    }
}

#[test]
fn arrow_is_a_3d_triangle() {
    // `AddClosed3DPolygon([(-3.5, 20), (0, 27), (3.5, 20)], 3)`: per edge the back
    // line, the front line and the back-to-front edge at its first corner.
    let s = shape(PartKind::Arrow { dir: Vec2::ZERO });
    assert_eq!(s.own_sprites, 0);
    assert_eq!(s.lines.len(), 9);
    let l = s.lines[0];
    assert!(v3(l.a, [-3.5, 20.0, -3.0]) && v3(l.b, [0.0, 27.0, -3.0]));
    assert!(v3(s.lines[1].a, [-3.5, 20.0, 3.0]) && v3(s.lines[1].b, [0.0, 27.0, 3.0]));
    assert!(v3(s.lines[2].a, [-3.5, 20.0, -3.0]) && v3(s.lines[2].b, [-3.5, 20.0, 3.0]));
    // The closing edge runs from the last corner to the first.
    assert!(v3(s.lines[6].a, [3.5, 20.0, -3.0]) && v3(s.lines[6].b, [-3.5, 20.0, -3.0]));
    assert!(s.lines.iter().all(|l| l.a == l.a2 && l.b == l.b2));
}

#[test]
fn part_line_counts() {
    // Pentagon 5 x 3; red cross 12 corners x 3; food arrow 4 + 4; frame 8 x 3; meter
    // box 4 x 3; socket 8 x 3; stick 8 x 5; Switch button 8 x 3 after its bubble.
    let counts = [
        (PartKind::Pentagon, 0, 15),
        (PartKind::RedCross { small: false }, 0, 36),
        (PartKind::FoodArrow, 0, 8),
        (PartKind::Frame, 0, 24),
        (
            PartKind::MeterBox {
                f: 0.0,
                lit: false,
                fill: [1.0; 4],
            },
            0,
            12,
        ),
        (PartKind::GamePadStickSocket { scale: 0.6 }, 0, 24),
        (PartKind::SwitchButton(ButtonState::new("")), 1, 24),
        (PartKind::Symbol(SymbolPart::new("batSymbol")), 1, 0),
        (
            PartKind::LightEffect {
                light_alpha: 0.6,
                dark_alpha: 0.2,
                light_rad: 40.0,
                dark_rad: 80.0,
            },
            2,
            0,
        ),
    ];
    for (kind, own, lines) in counts {
        let label = format!("{kind:?}");
        let s = shape(kind);
        assert_eq!((s.own_sprites, s.lines.len()), (own, lines), "{label}");
    }
}

#[test]
fn red_cross_corners() {
    // i = 0: v = DegToVec(45) = (0.7071, 0.7071), perpendicular (-0.7071, 0.7071):
    // v*5 + p*5 = (0, 7.071), v*60 + p*5 = (38.89, 45.96), v*60 - p*5 = (45.96, 38.89).
    let s = shape(PartKind::RedCross { small: false });
    assert!(v3(s.lines[0].a, [0.0, 7.071, -5.0]));
    assert!(v3(s.lines[0].b, [38.891, 45.962, -5.0]));
    assert!(v3(s.lines[3].a, [38.891, 45.962, -5.0]));
    assert!(v3(s.lines[3].b, [45.962, 38.891, -5.0]));
    // Small: every corner * 0.4.
    let small = shape(PartKind::RedCross { small: true });
    assert!(v3(small.lines[0].b, [15.556, 18.385, -5.0]));
}

#[test]
fn food_arrow_points_down() {
    // Square at y 10 (x, z in +-5), then from corners 3, 2, 1, 0 to (0, -10, 0).
    let s = shape(PartKind::FoodArrow);
    assert!(v3(s.lines[0].a, [-5.0, 10.0, -5.0]) && v3(s.lines[0].b, [5.0, 10.0, -5.0]));
    assert!(v3(s.lines[4].a, [-5.0, 10.0, 5.0]) && v3(s.lines[4].b, [0.0, -10.0, 0.0]));
    assert!(v3(s.lines[7].a, [-5.0, 10.0, -5.0]));
}

#[test]
fn key_press_and_widening() {
    // KeyBoardKey: the octagon starts at (-15 + 3, -15) = (-12, -15). Line 0 is the back
    // line (z -5): its ends shrink by 0.9 and their pressed ends go to z 5; line 1 is
    // the front line (z 5): its pressed ends grow by 1.1.
    let s = key_shape(0.0);
    assert_eq!((s.own_sprites, s.lines.len()), (1, 24));
    assert!(v3(s.lines[0].a, [-10.8, -13.5, -5.0]));
    assert!(v3(s.lines[0].a2, [-10.8, -13.5, 5.0]));
    assert!(v3(s.lines[1].a, [-12.0, -15.0, 5.0]));
    assert!(v3(s.lines[1].a2, [-13.2, -16.5, 5.0]));
    // MakeWider(10) (the pick-up key): every x 10 further out.
    let wide = key_shape(10.0);
    assert!(v3(wide.lines[0].a, [-20.8, -13.5, -5.0]));
    assert!(v3(wide.lines[1].a2, [-23.2, -16.5, 5.0]));
    // Line 0's b is (-15, -12): back end -13.5 - 10.
    assert!(close(wide.lines[0].b.x, -23.5));
}

#[test]
fn meter_box_swells_from_its_centre() {
    // f = 0: the corners are DegToVec(-2.5) and DegToVec(2.5) at radii 50 and 60; left
    // inner = (-2.1810, 49.9524). The centroid of the twelve lines' A: x 0, y
    // 55 * cos(2.5 deg) = 54.9477, z (-5 * 8 + 5 * 4) / 12 = -1.6667. A - c =
    // (-2.1810, -4.9953, -3.3333), length 6.3893, unit (-0.3414, -0.7818, -0.5217):
    // A moves in by it, A2 out by twice it.
    let s = shape(PartKind::MeterBox {
        f: 0.0,
        lit: false,
        fill: [1.0; 4],
    });
    let l = s.lines[0];
    assert!(v3(l.a, [-1.8396, 50.7342, -4.4783]), "{:?}", l.a);
    assert!(v3(l.a2, [-2.8638, 48.3888, -6.0434]), "{:?}", l.a2);
}

#[test]
fn rotate_3d_turns_counter_clockwise() {
    // Rotate3DPos about z by a quarter turn: x = x cos - y sin, y = x sin + y cos.
    let p = Vec3::new(1.0, 0.0, 2.0).rotate_3d(0.0, 0.0, 0.25);
    assert!(v3(p, [0.0, 1.0, 2.0]));
    // About y by a quarter turn: z = z cos - x sin = -1, x = z sin + x cos = 2.
    let q = Vec3::new(1.0, 0.0, 2.0).rotate_3d(0.0, 0.25, 0.0);
    assert!(v3(q, [2.0, 0.0, -1.0]));
}

#[test]
fn lerp_angle_goes_the_short_way() {
    // Mathf.LerpAngle(350, 10, 0.5): Repeat(-340, 360) = 20, so 350 + 10 = 360.
    assert!(close(350.0.lerp_angle(10.0, 0.5), 360.0));
    assert!(close(10.0.lerp_angle(350.0, 0.5), 0.0));
    assert!(close((-90.0).lerp_angle(90.0, 1.0), 90.0));
}

#[test]
fn depth_pushes_from_the_screen_point() {
    // ApplyDepthOnVector: (x - camX - 683) * (1 - z * 0.0015) + 683, and the same about
    // 768 * 2/3 = 512 for y. At z 100 the factor is 0.85.
    let f = HologramFrame {
        cam_pos: Vec2::new(100.0, 200.0),
        ..frame()
    };
    let a = f.apply_depth(Vec3::new(783.0, 712.0, 100.0));
    assert!(close(a.x, 683.0) && close(a.y, 512.0));
    let b = f.apply_depth(Vec3::new(100.0, 200.0, 100.0));
    assert!(close(b.x, 102.45) && close(b.y, 76.8));
}

#[test]
fn hologram_colour_of_the_guide() {
    // Yellow MainColor (1, 0.8, 0.3), earth black. Extended: ExtensionOfSegment(0, 0) =
    // 1^0.1 = 1, so ColorOfSegment = Lerp(Main, Neutral (0.5, 0.5, 0.5, 1), 0.5) =
    // (0.75, 0.65, 0.4, 1) and hologram.color = Lerp(Main, that, 0.5) =
    // (0.875, 0.725, 0.35, 1).
    let c = OverseerGraphics::hologram_color_of(OverseerColor::Yellow, 1.0, Color::BLACK);
    let expect = [0.875, 0.725, 0.35, 1.0];
    assert!(
        [c.r, c.g, c.b, c.a]
            .iter()
            .zip(expect)
            .all(|(a, b)| close(*a, b)),
        "{c:?}"
    );
    // Folded (lastExtended 0): ColorOfSegment is the deep colour
    // ((Main + blue + 8 earth) / 10, alpha 0.5) = (0.1, 0.08, 0.13, 0.5), so
    // (0.55, 0.44, 0.215, 0.75).
    let c = OverseerGraphics::hologram_color_of(OverseerColor::Yellow, 0.0, Color::BLACK);
    let expect = [0.55, 0.44, 0.215, 0.75];
    assert!(
        [c.r, c.g, c.b, c.a]
            .iter()
            .zip(expect)
            .all(|(a, b)| close(*a, b)),
        "{c:?}"
    );
}

#[test]
fn part_colours_go_half_way_each_update() {
    // `color` starts clear (Unity's default Color) and lerps half way to GetToColor:
    // 0.5, then 0.75 of it. A dimmed meter box is hologram.color * 0.75, opaque.
    let mut part = HologramPart::new(PartKind::Pentagon);
    part.to_color = PartColor::Holo(HoloColor::Red);
    let mut box_part = HologramPart::new(PartKind::MeterBox {
        f: 0.0,
        lit: false,
        fill: [1.0; 4],
    });
    box_part.to_color = PartColor::Dimmed(HoloColor::White);
    let h = hologram(vec![part, box_part]);
    let mut g = HologramGraphics::new(&h);
    g.update(&h, Color::WHITE);
    assert_eq!(
        g.part_color(0),
        Some(Color {
            r: 0.5,
            g: 0.0,
            b: 0.0,
            a: 0.5
        })
    );
    g.update(&h, Color::WHITE);
    assert_eq!(
        g.part_color(0),
        Some(Color {
            r: 0.75,
            g: 0.0,
            b: 0.0,
            a: 0.75
        })
    );
    assert_eq!(
        g.part_color(1),
        Some(Color {
            r: 0.5625,
            g: 0.5625,
            b: 0.5625,
            a: 0.75
        })
    );
}

#[test]
fn faded_in_symbol_and_lines_draw_in_bloom() {
    // Fully faded in (fade, partFade 1, fadeExponent 1): useFade 1, popOut 1, so every
    // line shows (value < 1) and none flickers (value > 1 never), and the symbol is its
    // element at the part's position with scaleX InverseLerp(0.5, 1, 1) = 1.
    let arrow = HologramPart::new(PartKind::Arrow { dir: Vec2::ZERO });
    let symbol = HologramPart::at(
        PartKind::Symbol(SymbolPart::new("shelterSymbol")),
        Vec2::new(10.0, -5.0),
    );
    let h = hologram(vec![arrow, symbol]);
    let mut g = HologramGraphics::new(&h);
    g.update(&h, Color::WHITE);
    let list = g.draw(&h, &frame(), &mut UnityRandom::new(3));
    assert_eq!(list.items.len(), 10);
    assert!(list.items.iter().all(|i| i.layer() == Layer::Bloom));
    let DrawItem::Sprite(s) = &list.items[9] else {
        panic!("symbol sprite");
    };
    assert_eq!(
        (s.index, s.element, s.shader),
        (9, "shelterSymbol", Shader::Hologram)
    );
    assert!(close(s.pos.x, 693.0) && close(s.pos.y, 507.0));
    assert!(close(s.scale.x, 1.0) && close(s.scale.y, 1.0));
    // Colour half way to white after one update, alpha 0.5 * sprite alpha 1.
    assert!(close(s.color.r, 0.5) && close(s.color.a, 0.5));
    // Line 0, rotation 0 (dir zero: VecToDeg(0, 0) = 0): from (-3.5, 20, -3) + (683,
    // 512): depth factor 1.0045 about (683, 512) gives (679.484, 532.09); to
    // (0, 27, -3): (683, 539.12). Rotation AimFromOneVectorToAnother: atan2(3.516,
    // 7.03) = 26.57 degrees; length 7.860. Alpha useColor.a 0.5 * useFade^0.5.
    let DrawItem::Sprite(l) = &list.items[0] else {
        panic!("line sprite");
    };
    assert_eq!((l.element, l.anchor.y), ("pixel", 0.0));
    assert!(
        close(l.pos.x, 679.484) && close(l.pos.y, 532.09),
        "{:?}",
        l.pos
    );
    assert!(close(l.rotation, 26.565), "{}", l.rotation);
    assert!(close(l.scale.y, 7.861), "{}", l.scale.y);
    assert!(close(l.color.a, 0.5));
}

#[test]
fn faded_out_hologram_draws_nothing() {
    // useFade 0: lines need value < 0, symbols value <= InverseLerp(0.5, 1, 0) = 0.
    let mut h = hologram(vec![
        HologramPart::new(PartKind::Arrow { dir: Vec2::ZERO }),
        HologramPart::new(PartKind::Symbol(SymbolPart::new("batSymbol"))),
    ]);
    h.fade = 0.0;
    h.last_fade = 0.0;
    let mut g = HologramGraphics::new(&h);
    let mut rng = UnityRandom::new(5);
    for _ in 0..20 {
        let list = g.draw(&h, &frame(), &mut rng);
        assert!(list.items.is_empty());
    }
}

#[test]
fn light_effect_sprites() {
    // HologramLightEffect(0.6, 0.2, 40, 80): sprite 0 dark (FlatLight, black, alpha
    // 0.2 * useFade^3, scale 80 / 8 * (0.5 + 0.5 useFade) = 10), sprite 1 the light
    // (LightSource, alpha 0.6, scale 5). At popOut 1 both sit at the part.
    let h = hologram(vec![HologramPart::new(PartKind::LightEffect {
        light_alpha: 0.6,
        dark_alpha: 0.2,
        light_rad: 40.0,
        dark_rad: 80.0,
    })]);
    let mut g = HologramGraphics::new(&h);
    g.update(&h, Color::WHITE);
    let list = g.draw(&h, &frame(), &mut UnityRandom::new(1));
    let sprites: Vec<_> = list
        .items
        .iter()
        .filter_map(|i| match i {
            DrawItem::Sprite(s) => Some(*s),
            DrawItem::Mesh(_) => None,
        })
        .collect();
    assert_eq!(sprites.len(), 2);
    assert_eq!(sprites[0].shader, Shader::FlatLight);
    assert!(close(sprites[0].color.r, 0.0) && close(sprites[0].color.a, 0.2));
    assert!(close(sprites[0].scale.x, 10.0));
    assert_eq!(sprites[1].shader, Shader::LightSource);
    assert!(close(sprites[1].color.a, 0.5 * 0.6) && close(sprites[1].scale.x, 5.0));
    assert!(close(sprites[1].pos.x, 683.0));
}

#[test]
fn key_symbol_element_follows_the_press() {
    // ButtonOrKey: "key" + symbol + (down || transform > 0.5 ? "B" : "A"), at depth
    // -5 + 10 * transform: transform 0 gives z -5, factor 1.0075 about (683, 512).
    let mut part = HologramPart::at(
        PartKind::Key {
            button: ButtonState::new("Shift"),
            wider: 10.0,
        },
        Vec2::new(-125.0, 0.0),
    );
    let h = hologram(vec![part.clone()]);
    let mut g = HologramGraphics::new(&h);
    let list = g.draw(&h, &frame(), &mut UnityRandom::new(2));
    let DrawItem::Sprite(s) = &list.items[0] else {
        panic!("symbol");
    };
    assert_eq!(s.element, "keyShiftA");
    assert!(close(s.pos.x, 683.0 - 125.0 * 1.0075), "{}", s.pos.x);
    part.transform = 0.6;
    part.last_transform = 0.6;
    let h = hologram(vec![part]);
    let list = g.draw(&h, &frame(), &mut UnityRandom::new(2));
    let DrawItem::Sprite(s) = &list.items[0] else {
        panic!("symbol");
    };
    assert_eq!(s.element, "keyShiftB");
}

#[test]
fn line_shape_of_hologram_line() {
    let l = HologramLine {
        a: Vec3::new(0.0, 0.0, 0.0),
        b: Vec3::new(1.0, 0.0, 0.0),
        a2: Vec3::new(0.0, 2.0, 0.0),
        b2: Vec3::new(1.0, 2.0, 0.0),
    };
    // Vector3.Lerp(A, A2, transform), clamped.
    assert!(v3(l.a.lerp_to(l.a2, 0.25), [0.0, 0.5, 0.0]));
    assert!(v3(l.b.lerp_to(l.b2, 2.0), [1.0, 2.0, 0.0]));
}

#[test]
fn installed_hologram_elements_and_projections() {
    // The elements the holograms ask for are in `rainWorld` or `uiSprites`; the
    // projections are 1000 x 1000 (a 5 x 5 grid of 200 px pictures).
    let game = std::env::var_os("RW_DIR").map_or_else(
        || std::path::PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        std::path::PathBuf::from,
    );
    let Ok(resources) = rw_assets::UnityResources::open(game.join("RainWorld_Data")) else {
        eprintln!("skipped: no game under {}", game.display());
        return;
    };
    let atlases = [
        Atlas::from_resources(&resources, "rainWorld").unwrap(),
        Atlas::from_resources(&resources, "uiSprites").unwrap(),
    ];
    let names = [
        "pixel",
        "Circle4",
        "LizardBubble0",
        "batSymbol",
        "foodSymbol",
        "shelterSymbol",
        "miscDangerSymbol",
        "GuidanceSlugcat",
        "GuidanceMoon",
        "GuidanceNeuron",
        "playerStandingSymbol",
        "playerCrouchingSymbol",
        "keyArrowA",
        "keyArrowB",
        "keyShiftA",
        "keyShiftB",
        "keyZA",
        "keyZB",
        "keyXA",
        "keyXB",
        "buttonXA",
        "buttonXB",
        "buttonAA",
        "buttonAB",
        "buttonBA",
        "buttonBB",
        "buttonSquareA",
        "buttonSquareB",
        "buttonCrossA",
        "buttonCrossB",
        "buttonCircleA",
        "buttonCircleB",
    ];
    for name in names {
        assert!(
            atlases.iter().any(|a| a.get(name).is_some()),
            "{name} missing"
        );
    }
    let assets = rw_assets::StreamingAssets::new(game.join("RainWorld_Data/StreamingAssets"));
    for name in ["RND_PROJ", "STR_PROJ"] {
        let path = assets.resolve_file_path(&format!("Projections/{name}.png"));
        let image = rw_assets::PngImage::load(&path).unwrap();
        assert_eq!((image.width(), image.height()), (1000, 1000), "{name}");
    }
}
