//! Headless GPU checks of the room sprite shaders; skipped without a Vulkan adapter.

use rw_graphics::{Atlas, Color as SpriteColor, DrawItem, Mesh as SpriteMesh, Shader, Sprite};

use super::*;

const JSON: &str = r#"{"frames": {
  "RainWorld_White.png": {"frame": {"x": 0, "y": 0, "w": 16, "h": 16}, "rotated": false,
            "trimmed": false, "spriteSourceSize": {"x": 0, "y": 0, "w": 16, "h": 16},
            "sourceSize": {"w": 16, "h": 16}}},
  "meta": {"image": "t.png", "size": {"w": 16, "h": 16}}}"#;

const SIZE: (u32, u32) = (64, 64);

fn renderer() -> Option<WgpuRenderer> {
    let mut r = match WgpuRenderer::headless(SIZE) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let atlas = SpriteAtlas::new(Atlas::parse(JSON).unwrap()).unwrap();
    r.set_atlas(
        &atlas,
        &[&PngImage::from_pixels(16, 16, vec![[255; 4]; 256])],
    );
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], SIZE));
    r.set_background(Color::rgb(0.0, 0.0, 0.0));
    Some(r)
}

fn pixel(image: &FrameImage, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * image.width() + x) * 4) as usize;
    let p = &image.rgba()[i..i + 3];
    [p[0], p[1], p[2]]
}

fn white_sprite(shader: Shader, color: SpriteColor) -> DrawItem {
    DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(32.0, 32.0),
        rotation: 0.0,
        scale: Vec2::new(2.0, 2.0),
        anchor: Vec2::new(0.5, 0.5),
        color,
        shader,
        layer: Layer::Items,
    })
}

#[test]
fn flat_light_is_a_disc_over_futile_white() {
    let Some(mut r) = renderer() else { return };
    let list = DrawList {
        items: vec![white_sprite(
            Shader::FlatLight,
            SpriteColor::rgb(1.0, 0.0, 0.0),
        )],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    let image = r.render_image().unwrap();
    // The 32x32 quad spans 16..48. Pixel (32, 32) is at (32.5, 31.5), half a pixel
    // from the centre on each axis: d = 2 * sqrt(2) * 0.5 / 32 = 0.0442,
    // (1 - d^2)^3.5 = 0.9932 -> 253. Nothing in its corner (centre distance clamped
    // to 1) or outside it.
    assert_eq!(pixel(&image, 32, 32), [253, 0, 0]);
    assert_eq!(pixel(&image, 17, 17), [0, 0, 0]);
    assert_eq!(pixel(&image, 5, 5), [0, 0, 0]);
}

#[test]
fn shaders_split_and_overseer_zip_is_opaque_at_full_alpha() {
    let Some(mut r) = renderer() else { return };
    let mesh = SpriteMesh {
        index: 1,
        vertices: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(64.0, 0.0),
            Vec2::new(0.0, 64.0),
        ],
        uvs: vec![Vec2::new(0.5, 0.5); 3],
        triangles: std::borrow::Cow::Owned(vec![[0, 1, 2]]),
        color: SpriteColor::rgb(0.0, 1.0, 0.0),
        vertex_colors: Vec::new(),
        shader: Shader::OverseerZip,
        layer: Layer::Items,
    };
    let list = DrawList {
        items: vec![
            DrawItem::Mesh(mesh),
            white_sprite(Shader::Basic, SpriteColor::rgb(0.0, 0.0, 1.0)),
        ],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    let image = r.render_image().unwrap();
    // `dst <= lerp(0, 3, 1)` whatever the noise: the mesh colour, opaque; the Basic
    // quad over it in its own draw.
    assert_eq!(pixel(&image, 8, 60), [0, 255, 0]);
    assert_eq!(pixel(&image, 32, 32), [0, 0, 255]);
    assert_eq!(pixel(&image, 60, 4), [0, 0, 0]);
}
