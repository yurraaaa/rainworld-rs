//! A room's `DeathFallGraphic` sprite (the `DeathFall` shader in `Bloom`), drawn by the
//! camera viewing the room. The object, its counter and its height are `rw_sim::DeathFall`
//! (made by `Room.Loaded`, counted in the room's update list).
//!
//! Mirrors: DeathFallGraphic.cs (InitiateSprites, DrawSprites, AddToContainer, ApplyPalette)

use rw_graphics::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use rw_render::SpriteAtlas;
use rw_sim::{DeathFall, Vec2};

/// The sprite of a [`DeathFall`].
pub struct DeathFallSprite;

impl DeathFallSprite {
    /// The sprite (`InitiateSprites`, `DrawSprites`): `Futile_White` scaled to
    /// `sSize.x + 20` by 180 px, anchored at its bottom centre on the screen's middle
    /// column at `height`, white at [`DeathFall::alpha`], with the `DeathFall` shader in
    /// `Bloom`. The game takes the room's `RainMask_` element instead once the rain or
    /// the `WaterLight` has made one; the shader reads only the quad's uv, 0..1 either
    /// way.
    /// `DeathFallHeavy` is Remix's (`cfgClearerDeathGradients`). `ApplyPalette` is
    /// empty.
    #[must_use]
    pub fn draw_list(
        fall: &DeathFall,
        height: f32,
        screen_width: f32,
        cam_pos: Vec2,
        time_stacker: f32,
    ) -> DrawList {
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: SpriteAtlas::FUTILE_WHITE,
                pos: Vec2::new(cam_pos.x + screen_width / 2.0, height),
                rotation: 0.0,
                scale: Vec2::new((screen_width + 20.0) / 16.0, 180.0 / 16.0),
                anchor: Vec2::new(0.5, 0.0),
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: fall.alpha(time_stacker).clamp(0.0, 1.0),
                },
                shader: Shader::DeathFall,
                layer: Layer::Bloom,
            })],
        }
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::Room;
    use rw_sim::CameraView;

    use super::*;

    fn room() -> Room {
        let (w, h) = (20, 12);
        let column = format!("{}0|", "0|".repeat(h - 1));
        let text = format!(
            "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0|0,300\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{}\n",
            column.repeat(w)
        );
        Room::parse(&text).expect("test room parses")
    }

    /// `DrawSprites`: a 1386 x 180 quad (`(sSize.x + 20) / 16`, `180 / 16` of the 16 px
    /// `Futile_White`) anchored at its bottom centre, x the screen's middle (stage
    /// `sSize.x / 2`, room `camPos.x + 683`), y the height (stage `height - camPos.y`);
    /// alpha `0.85 + 0.15 sin((counter + timeStacker) / 25)`.
    #[test]
    fn the_sprite_spans_the_screen_bottom_up_from_the_height() {
        let r = room();
        let mut fall = DeathFall::new(&r, None, false, &[]).unwrap();
        let view = CameraView {
            room: 3,
            current: 0,
            next: 0,
        };
        for _ in 0..30 {
            fall.update(&r, 3, &view);
        }
        let list =
            DeathFallSprite::draw_list(&fall, fall.height, 1366.0, Vec2::new(-40.0, -900.0), 0.5);
        let [DrawItem::Sprite(s)] = list.items.as_slice() else {
            panic!("one sprite: {list:?}");
        };
        assert_eq!(s.element, SpriteAtlas::FUTILE_WHITE);
        assert_eq!(s.pos, Vec2::new(643.0, -860.0));
        assert_eq!(s.scale, Vec2::new(1386.0 / 16.0, 180.0 / 16.0));
        assert_eq!(s.anchor, Vec2::new(0.5, 0.0));
        assert_eq!((s.shader, s.layer), (Shader::DeathFall, Layer::Bloom));
        let a = 0.85 + 0.15 * (30.5f32 / 25.0).sin();
        assert!((s.color.a - a).abs() < 1e-6, "{} vs {a}", s.color.a);
        assert_eq!([s.color.r, s.color.g, s.color.b], [1.0; 3]);
    }
}
