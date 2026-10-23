use rw_assets::{LightBeamData, QuadData};
use rw_sim::{LightBeam, SpotLight, Vec2};

use super::{LightBeamSprites, LightBeamView, SpotLightSprites};
use crate::core::draw::{Color, DrawItem, Layer, Shader};

fn view() -> LightBeamView {
    LightBeamView {
        camera: 0,
        shader_light: 1.0,
        darken_lights: 0.0,
        water_lights: false,
    }
}

fn beam(alpha: f32, color_a: f32, color_b: f32, sun: bool) -> LightBeam {
    let data = LightBeamData::parse(&format!(
        "0~40~40~40~40~0~0~0~{alpha}~{color_a}~{color_b}~{}",
        u8::from(sun)
    ))
    .expect("data");
    let mut b = LightBeam::new(0, Vec2::new(10.0, 20.0), data);
    b.palette_lit_color = [0.5, 0.25, 0.0, 1.0];
    b
}

/// With the sun at full strength the alpha is the data's; the third it lies in
/// is unchanged by the sun's fade of the position inside it.
#[test]
fn alpha_in_thirds() {
    let b = beam(0.5, 0.0, 0.0, true);
    assert!((LightBeamSprites::frame_alpha(&b, &view()) - 0.5).abs() < 1e-6);
    // ShaderLight -0.1: InverseLerp(-0.2, 0, -0.1) = 0.5, ^1.2; base = 1 (0.333..),
    // x = InverseLerp(1/3, 2/3, 0.5) = 0.5.
    let mut v = view();
    v.shader_light = -0.1;
    let x = 0.5f32 * 0.5f32.powf(1.2);
    let want = (1.0 / 3.0) + x * (1.0 / 3.0);
    assert!((LightBeamSprites::frame_alpha(&b, &v) - want).abs() < 1e-6);
    // Full dark: down to the third's floor. The room's darkening scales it.
    v.shader_light = -1.0;
    assert!((LightBeamSprites::frame_alpha(&b, &v) - 1.0 / 3.0).abs() < 1e-6);
    v.darken_lights = 0.25;
    assert!((LightBeamSprites::frame_alpha(&b, &v) - 0.25).abs() < 1e-6);
    // Without the sun flag the shader light is ignored.
    let b = beam(0.5, 0.0, 0.0, false);
    assert!((LightBeamSprites::frame_alpha(&b, &v) - 0.375).abs() < 1e-6);
}

/// The colour over its largest component: the palette's (0.5, 0.25, 0) becomes
/// (1, 0.5, 0); with `colorA` 1 it is white; the mesh is in `ForegroundLights`.
#[test]
fn colour_over_its_largest_component() {
    let mut s = LightBeamSprites::default();
    let b = beam(0.5, 0.0, 0.0, true);
    let list = s.draw(&b, &view());
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("a mesh");
    };
    assert_eq!(
        (m.shader, m.layer),
        (Shader::LightBeam, Layer::ForegroundLights)
    );
    assert_eq!((m.color.r, m.color.g, m.color.b), (1.0, 0.5, 0.0));
    assert!((m.color.a - 0.5).abs() < 1e-6);
    let mut s = LightBeamSprites::default();
    let list = s.draw(&beam(0.5, 1.0, 0.0, true), &view());
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("a mesh");
    };
    assert_eq!((m.color.r, m.color.g, m.color.b), (1.0, 1.0, 1.0));
}

/// `UpdateColor` runs only on a first draw, an alpha change or a new camera
/// position: an environment colour that changes in between is not drawn until then.
#[test]
fn colour_is_rebuilt_only_on_alpha_or_camera_change() {
    let mut s = LightBeamSprites::default();
    let mut b = beam(0.5, 0.0, 1.0, true);
    b.environment_color = [0.0, 1.0, 0.0, 1.0];
    let first = s.draw(&b, &view());
    b.environment_color = [0.0, 0.0, 1.0, 1.0];
    let same = s.draw(&b, &view());
    assert_eq!(first, same);
    let mut v = view();
    v.camera = 1;
    let DrawItem::Mesh(m) = &s.draw(&b, &v).items[0] else {
        panic!("a mesh");
    };
    assert_eq!((m.color.g, m.color.b), (0.0, 1.0));
}

/// A mesh in a room with `WaterLights` is in `Water`; a 400 px quad is a 2 x 2 grid.
#[test]
fn water_lights_layer_and_grid() {
    let mut s = LightBeamSprites::default();
    let mut v = view();
    v.water_lights = true;
    let data = LightBeamData::parse("0~400~400~400~400~0~0~0~0.4~0~0~1").expect("data");
    let b = LightBeam::new(0, Vec2::ZERO, data);
    let DrawItem::Mesh(m) = &s.draw(&b, &v).items[0] else {
        panic!("a mesh");
    };
    assert_eq!(
        (m.layer, m.vertices.len(), m.triangles.len()),
        (Layer::Water, 9, 8)
    );
}

/// A 300 px quad is a 2 x 2 grid (150 px per division), black, in `Shadows`.
#[test]
fn black_grid_in_shadows() {
    let data = QuadData::parse("0~300~300~300~300~0").expect("data");
    let s = SpotLight::new(0, Vec2::new(5.0, 6.0), &data);
    let list = s.draw();
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("a mesh");
    };
    assert_eq!((m.shader, m.layer), (Shader::VectorCircle, Layer::Shadows));
    assert_eq!((m.vertices.len(), m.triangles.len()), (9, 8));
    assert_eq!(m.color, Color::BLACK);
    assert_eq!(m.vertices[0], Vec2::new(5.0, 6.0));
    assert_eq!(m.vertices[8], Vec2::new(305.0, 306.0));
}
