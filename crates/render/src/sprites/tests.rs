//! Expected corners and coordinates are computed by hand from Futile's formulas
//! (`rainworld-re/notes/sprite-atlases.md` section 3), not by this code.

use rw_graphics::{Color as SpriteColor, TAIL_TRIANGLES};

use super::*;

/// A 64x32 atlas: `A` trimmed (frame 4x6 at (10, 20), offset (1, 2) in a 8x10 box) and
/// the white element at (40, 0).
const JSON: &str = r#"{"frames": {
  "A.png": {"frame": {"x": 10, "y": 20, "w": 4, "h": 6}, "rotated": false, "trimmed": true,
            "spriteSourceSize": {"x": 1, "y": 2, "w": 4, "h": 6}, "sourceSize": {"w": 8, "h": 10}},
  "RainWorld_White.png": {"frame": {"x": 40, "y": 0, "w": 16, "h": 16}, "rotated": false,
            "trimmed": false, "spriteSourceSize": {"x": 0, "y": 0, "w": 16, "h": 16},
            "sourceSize": {"w": 16, "h": 16}}},
  "meta": {"image": "t.png", "size": {"w": 64, "h": 32}}}"#;

fn atlas() -> SpriteAtlas {
    SpriteAtlas::new(Atlas::parse(JSON).unwrap()).unwrap()
}

fn sprite(element: &'static str, scale: [f32; 2], rotation: f32, layer: Layer) -> Sprite {
    Sprite {
        index: 0,
        element,
        pos: Vec2::new(100.0, 50.0),
        rotation,
        scale: Vec2::from(scale),
        anchor: Vec2::new(0.5, 0.5),
        color: SpriteColor::rgb(1.0, 0.5, 0.25),
        shader: rw_graphics::Shader::Basic,
        layer,
    }
}

const CAM: Vec2 = Vec2::new(20.0, 10.0);

/// Asserts each vertex position equals `expected` (stage pixels) within 1e-4.
fn assert_corners(quad: &[SpriteVertex; 4], expected: [[f32; 2]; 4]) {
    for (v, e) in quad.iter().zip(expected) {
        assert!(
            (v.pos[0] - e[0]).abs() < 1e-4 && (v.pos[1] - e[1]).abs() < 1e-4,
            "{:?} != {e:?} in {quad:?}",
            v.pos
        );
    }
}

#[test]
fn local_corners_use_the_untrimmed_box_and_trim_offset() {
    let a = atlas();
    let frame = a.atlas().get("A").unwrap();
    // Centre anchor: box origin (-4, -5); x0 = -4 + 1 = -3, y0 = -5 + (10 - 2 - 6) = -3.
    let c = SpriteAtlas::local_corners(frame, Vec2::new(0.5, 0.5));
    assert_eq!(
        c.map(Vec2::to_array),
        [[-3.0, 3.0], [1.0, 3.0], [1.0, -3.0], [-3.0, -3.0]]
    );
    // Bottom-left anchor: x0 = 1, y0 = 2.
    let c = SpriteAtlas::local_corners(frame, Vec2::new(0.0, 0.0));
    assert_eq!(
        c.map(Vec2::to_array),
        [[1.0, 8.0], [5.0, 8.0], [5.0, 2.0], [1.0, 2.0]]
    );
}

#[test]
fn quad_places_scales_mirrors_and_rotates_clockwise() {
    let a = atlas();
    let mid = Layer::Midground;
    // Stage centre (100, 50) - (20, 10) = (80, 40).
    let q = a.quad(&sprite("A", [1.0, 1.0], 0.0, mid), CAM).unwrap();
    assert_corners(&q, [[77.0, 43.0], [81.0, 43.0], [81.0, 37.0], [77.0, 37.0]]);
    // UVs: u 10/64..14/64, v (32 - 20 - 6)/32..(32 - 20)/32, TL at (u0, v1).
    let (u0, u1, v0, v1) = (0.156_25, 0.218_75, 0.1875, 0.375);
    let uvs = q.map(|v| v.uv);
    assert_eq!(uvs, [[u0, v1], [u1, v1], [u1, v0], [u0, v0]]);
    assert_eq!(q[0].color, [1.0, 0.5, 0.25, 1.0]);
    // Mirrored x: local x negated about the anchor, UVs kept per vertex.
    let q = a.quad(&sprite("A", [-1.0, 1.0], 0.0, mid), CAM).unwrap();
    assert_corners(&q, [[83.0, 43.0], [79.0, 43.0], [79.0, 37.0], [83.0, 37.0]]);
    assert_eq!(q.map(|v| v.uv), uvs);
    // 90 degrees clockwise: (x, y) -> (y, -x).
    let q = a.quad(&sprite("A", [1.0, 1.0], 90.0, mid), CAM).unwrap();
    assert_corners(&q, [[83.0, 43.0], [83.0, 39.0], [77.0, 39.0], [77.0, 43.0]]);
    // Scale (2, -1) first, then 90 clockwise: TL (-3, 3) -> (-6, -3) -> (-3, 6).
    let q = a.quad(&sprite("A", [2.0, -1.0], 90.0, mid), CAM).unwrap();
    assert_corners(&q, [[77.0, 46.0], [77.0, 38.0], [83.0, 38.0], [83.0, 46.0]]);
    // Unknown element: no quad.
    assert!(a.quad(&sprite("Nope", [1.0, 1.0], 0.0, mid), CAM).is_none());
}

#[test]
fn quad_rotation_45_degrees() {
    // Anchor (0, 0) puts the box corner on pos; the BL vertex (1, 2) rotated 45
    // degrees clockwise: (x cos + y sin, -x sin + y cos) = (3, 1) / sqrt 2.
    let a = atlas();
    let mut s = sprite("A", [1.0, 1.0], 45.0, Layer::Midground);
    s.anchor = Vec2::new(0.0, 0.0);
    let q = a.quad(&s, Vec2::ZERO).unwrap();
    let r = std::f32::consts::FRAC_1_SQRT_2;
    assert_corners(
        &q,
        [
            [100.0 + 9.0 * r, 50.0 + 7.0 * r],
            [100.0 + 13.0 * r, 50.0 + 3.0 * r],
            [100.0 + 7.0 * r, 50.0 - 3.0 * r],
            [100.0 + 3.0 * r, 50.0 + r],
        ],
    );
}

fn tail_mesh() -> SpriteMesh {
    SpriteMesh {
        index: 2,
        vertices: (0..15)
            .map(|i| Vec2::new(i as f32, 2.0 * i as f32))
            .collect(),
        uvs: Vec::new(),
        triangles: std::borrow::Cow::Borrowed(&TAIL_TRIANGLES),
        color: SpriteColor::rgb(0.0, 1.0, 0.0),
        vertex_colors: Vec::new(),
        shader: rw_graphics::Shader::Basic,
        layer: Layer::Midground,
    }
}

#[test]
fn tail_mesh_vertices_and_indices() {
    let a = atlas();
    let mut batch = SpriteBatch::default();
    // One quad first, so the mesh indices are offset by 4.
    assert!(batch.push_sprite(&a, &sprite("A", [1.0, 1.0], 0.0, Layer::Midground), CAM));
    batch.push_mesh(&a, &tail_mesh(), CAM);
    assert_eq!(batch.vertices.len(), 4 + 15);
    assert_eq!(batch.indices.len(), 6 + 13 * 3);
    assert_eq!(&batch.indices[..6], &[0, 1, 2, 0, 2, 3]);
    // TAIL_TRIANGLES starts [0, 1, 2], [1, 2, 3], [4, 5, 6] and ends [11, 12, 13].
    assert_eq!(&batch.indices[6..12], &[4, 5, 6, 5, 6, 7]);
    assert_eq!(&batch.indices[batch.indices.len() - 3..], &[15, 16, 17]);
    // Vertex 7 at (7, 14) - (20, 10); every UV at the white element's centre:
    // u (40 + 8) / 64, v (32 - 0 - 16 + 8) / 32.
    let v = batch.vertices[4 + 7];
    assert_eq!(v.pos, [-13.0, 4.0]);
    assert_eq!(v.uv, [0.75, 0.75]);
    assert_eq!(v.color, [0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn midground_before_the_level_and_foreground_after() {
    let a = atlas();
    let list = DrawList {
        items: vec![
            DrawItem::Sprite(sprite("A", [1.0, 1.0], 0.0, Layer::Midground)),
            DrawItem::Mesh(tail_mesh()),
            DrawItem::Sprite(sprite("Nope", [1.0, 1.0], 0.0, Layer::Midground)),
            DrawItem::Sprite(sprite("A", [1.0, 1.0], 0.0, Layer::Foreground)),
        ],
    };
    let mut queue = DrawQueue::default();
    queue
        .shapes
        .rect([0.0, 0.0], [1.0, 1.0], crate::Color::WHITE);
    let missing = queue.push_sprites(&a, &list, Layer::Midground, CAM);
    assert_eq!(missing, ["Nope"]);
    queue.mark_level();
    queue.push_sprites(&a, &list, Layer::Foreground, CAM);
    queue
        .shapes
        .rect([0.0, 0.0], [1.0, 1.0], crate::Color::WHITE);
    // Midground: quad (6) + tail (39) indices behind the level; Foreground quad after.
    assert_eq!(
        queue.batches(),
        [
            Batch::Shapes(0..6),
            Batch::Sprites(0..45),
            Batch::Level,
            Batch::Sprites(45..51),
            Batch::Shapes(6..12),
        ]
    );
    // Without a level every batch is drawn in call order.
    queue.clear();
    queue.push_sprites(&a, &list, Layer::Foreground, CAM);
    assert_eq!(queue.batches(), [Batch::Sprites(0..6)]);
}

#[test]
fn water_after_the_level_and_foreground_band_then_body() {
    let a = atlas();
    let list = DrawList {
        items: vec![DrawItem::Sprite(sprite(
            "A",
            [1.0, 1.0],
            0.0,
            Layer::Foreground,
        ))],
    };
    let mut queue = DrawQueue::default();
    queue.mark_level();
    queue.push_sprites(&a, &list, Layer::Foreground, CAM);
    let water = crate::WaterSprites::new(10, 40.0);
    queue.push_water(&water);
    // 7 points to render: 14 triangles per sprite.
    assert_eq!(
        queue.batches(),
        [
            Batch::Level,
            Batch::Sprites(0..6),
            Batch::Water(WaterShader::Surface, 0..42),
            Batch::Water(WaterShader::Deep, 42..84),
        ]
    );
    assert_eq!(queue.water.len(), 84);
    queue.clear();
    assert!(queue.water.is_empty());
    assert!(queue.batches().is_empty());
}

#[test]
fn installed_atlas_uvs() {
    // The installed game's rainWorld atlas (464x512), when `RW_DIR` has the game.
    let game = std::env::var_os("RW_DIR").map_or_else(
        || std::path::PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        std::path::PathBuf::from,
    );
    let Ok(resources) = rw_assets::UnityResources::open(game.join("RainWorld_Data")) else {
        eprintln!("skipped: no game under {}", game.display());
        return;
    };
    let atlas = Atlas::from_resources(&resources, "rainWorld").unwrap();
    let a = SpriteAtlas::new(atlas).expect("RainWorld_White present");
    // BodyA: frame (358, 50, 14, 19); FaceA0: (379, 301, 12, 7); HeadA15: (312, 176, 24, 15).
    for (name, [x, y, w, h]) in [
        ("BodyA", [358.0, 50.0, 14.0, 19.0]),
        ("FaceA0", [379.0, 301.0, 12.0, 7.0]),
        ("HeadA15", [312.0, 176.0, 24.0, 15.0]),
    ] {
        let frame = a.atlas().get(name).unwrap();
        let want = [
            [x / 464.0, (512.0 - y - h) / 512.0],
            [(x + w) / 464.0, (512.0 - y) / 512.0],
        ];
        assert_eq!(SpriteAtlas::uv_rect(a.atlas(), frame), want, "{name}");
    }
    // RainWorld_White (315, 376, 16x16): centre (323 / 464, (512 - 376 - 8) / 512).
    assert!((a.white_uv[0] - 323.0 / 464.0).abs() < 1e-6);
    assert!((a.white_uv[1] - 128.0 / 512.0).abs() < 1e-6);
}
