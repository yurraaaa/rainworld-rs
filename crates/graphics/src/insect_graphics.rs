//! The cosmetic insects' sprites (`InitiateSprites`, `DrawSprites`, `ApplyPalette` of
//! each kind): pixels and `Circle20` ovals in `Midground` (the ant in `Foreground`), in
//! palette colours. The dragonfly's, red swarmer's, beetle's and wasp's wings jitter
//! with draws on the cosmetic stream (D1). The ant's `SpecificDepth` shader is not
//! ported: it draws with `Basic` (D23).
//!
//! Mirrors: MiniFly.cs, FireFly.cs, TinyDragonfly.cs, RockFlea.cs, GrassHopper.cs, RedSwarmer.cs, Ant.cs, Beetle.cs, WaterGlowworm.cs, Wasp.cs, Moth.cs (InitiateSprites, DrawSprites, ApplyPalette)

use rw_assets::PaletteTexture;
use rw_random::UnityRandom;
use rw_sim::insect::{
    Ant, Beetle, CosmeticInsect, FireFly, GrassHopper, InsectBase, InsectBody, MiniFly, Moth,
    RedSwarmer, RockFlea, TinyDragonfly, Wasp, WaterGlowworm,
};
use rw_sim::{FloatExt, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::math::{Custom, CustomVec};

/// What the insects' sprites read of the room camera.
pub struct InsectDraw<'a> {
    /// The camera's palette texture (`palette.texture`).
    pub palette: &'a PaletteTexture,
    /// `palette.darkness`.
    pub darkness: f32,
    /// `palette.fogAmount`.
    pub fog_amount: f32,
    /// The camera position at this frame (`camPos`) and `sSize` (the ant's depth).
    pub cam_pos: Vec2,
    pub screen: Vec2,
    /// The cosmetic stream (`DrawSprites`' draws).
    pub rng: &'a mut UnityRandom,
}

impl InsectDraw<'_> {
    /// `palette.texture.GetPixel(x, y)`.
    fn texel(&self, x: usize, y: usize) -> Color {
        let [r, g, b, a] = self.palette.texel(x, y).map(|c| f32::from(c) / 255.0);
        Color { r, g, b, a }
    }

    fn black(&self) -> Color {
        self.texel(2, 7)
    }

    fn fog(&self) -> Color {
        self.texel(1, 7)
    }

    /// `(i == 0) ? -1 : 1`: the left and right of a pair of legs or wings.
    fn side(i: usize) -> f32 {
        if i == 0 { -1.0 } else { 1.0 }
    }

    fn value(&mut self) -> f32 {
        self.rng.value()
    }

    #[allow(clippy::too_many_arguments)]
    fn sprite(
        index: usize,
        element: &'static str,
        pos: Vec2,
        rotation: f32,
        scale: Vec2,
        anchor_y: f32,
        color: Color,
    ) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element,
            pos,
            rotation,
            scale,
            anchor: Vec2::new(0.5, anchor_y),
            color,
            shader: Shader::Basic,
            layer: Layer::Midground,
        })
    }
}

/// The sprites of a cosmetic insect.
pub trait InsectSprites {
    /// `DrawSprites` at `t` between ticks, in the colours `ApplyPalette` gives.
    fn draw(&self, t: f32, cx: &mut InsectDraw) -> DrawList;
}

impl InsectSprites for CosmeticInsect {
    fn draw(&self, t: f32, cx: &mut InsectDraw) -> DrawList {
        let b = &self.base;
        let items = match &self.body {
            InsectBody::MiniFly(k) => k.sprites(b, t, cx),
            InsectBody::FireFly(k) => k.sprites(b, t, cx),
            InsectBody::TinyDragonfly(k) => k.sprites(b, t, cx),
            InsectBody::RockFlea(k) => k.sprites(b, t, cx),
            InsectBody::GrassHopper(k) => k.sprites(b, t, cx),
            InsectBody::RedSwarmer(k) => k.sprites(b, t, cx),
            InsectBody::Ant(k) => k.sprites(b, t, cx),
            InsectBody::Beetle(k) => k.sprites(b, t, cx),
            InsectBody::WaterGlowworm(k) => k.sprites(b, t, cx),
            InsectBody::Wasp(k) => k.sprites(b, t, cx),
            InsectBody::Moth(k) => k.sprites(b, t, cx),
        };
        DrawList { items }
    }
}

/// One kind's sprites, in sprite-index order.
trait KindSprites {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem>;
}

/// What every kind's `DrawSprites` reads first.
trait BaseDraw {
    /// `Vector2.Lerp(lastPos, pos, t)`.
    fn at(&self, t: f32) -> Vec2;
    /// `Mathf.Lerp(lastInGround, inGround, t)`.
    fn in_ground(&self, t: f32) -> f32;
    /// The dash of a mini fly or firefly: from where it was two ticks ago to now.
    fn streak(&self, last_last: Vec2, t: f32, color: Color) -> DrawItem;
}

impl BaseDraw for InsectBase {
    fn at(&self, t: f32) -> Vec2 {
        self.last_pos.lerp(self.pos, t)
    }

    fn in_ground(&self, t: f32) -> f32 {
        self.last_in_ground.lerp(self.in_ground, t)
    }

    fn streak(&self, last_last: Vec2, t: f32, color: Color) -> DrawItem {
        let from = last_last.lerp(self.last_pos, t);
        let to = self.at(t);
        let len = (2.0 + 1.1 * from.distance(to)).max(2.0);
        InsectDraw::sprite(
            0,
            "pixel",
            to,
            from.aim_to(to),
            Vec2::new(2.0, len),
            0.0,
            color,
        )
    }
}

impl KindSprites for MiniFly {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        vec![b.streak(self.last_last_pos, t, cx.black())]
    }
}

impl KindSprites for FireFly {
    fn sprites(&self, b: &InsectBase, t: f32, _cx: &mut InsectDraw) -> Vec<DrawItem> {
        let [r, g, bl] = self.color;
        vec![b.streak(self.last_last_pos, t, Color::rgb(r, g, bl))]
    }
}

impl KindSprites for TinyDragonfly {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let dir = self.last_dir.slerp(self.dir, t);
        pos.y -= 5.0 * ground;
        let glint = dir
            .dot(Vec2::from_degrees(47.0))
            .inverse_lerp(0.6, 1.0)
            .mathf_pow(2.0)
            * cx.value().mathf_pow(0.5);
        let black = cx.black();
        let body = if self.si_colors {
            Color::hsl(0.6.lerp(0.7, self.hue), 1.0, 0.3 + 0.4 * glint)
                .lerp(black, 0.3 * (1.0 - glint) + 0.5 * cx.darkness)
        } else {
            Color::hsl(
                0.35.lerp(0.45, self.hue),
                0.8 + 0.2 * glint.mathf_pow(0.3),
                0.5 + 0.4 * glint,
            )
            .lerp(black, 0.2 * (1.0 - glint) + 0.5 * cx.darkness)
        };
        let wing = Color::WHITE
            .lerp(cx.fog(), 0.5)
            .lerp(black, 0.5 * cx.darkness);
        let mut items = vec![InsectDraw::sprite(
            0,
            "pixel",
            pos,
            dir.to_degrees(),
            Vec2::new(2.0 * (1.0 - ground), 7.0 * (1.0 - ground)),
            0.2,
            body,
        )];
        for i in 0..2 {
            let rotation = (-30.0).lerp(30.0, cx.value()) + if i == 0 { -90.0 } else { 90.0 };
            items.push(InsectDraw::sprite(
                1 + i,
                "pixel",
                pos,
                rotation,
                Vec2::new(1.0 - ground, 5.0 * (1.0 - ground)),
                -0.5,
                wing,
            ));
        }
        items
    }
}

impl KindSprites for RockFlea {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let rot = self.last_rot.slerp(self.rot, t);
        pos.y -= 5.0 * ground * self.size;
        let color = cx.black().lerp(cx.texel(6, 2), self.color_fac);
        let body_move = self.last_body_move.lerp(self.body_move, t);
        let up = 1.0 - ground;
        let mut items = vec![InsectDraw::sprite(
            0,
            "Circle20",
            pos,
            rot.to_degrees(),
            Vec2::new(
                4.5 * self.size * up / 20.0,
                (6.0 + body_move * 3.0) * self.size * up / 20.0,
            ),
            0.2,
            color,
        )];
        for i in 0..2 {
            let leg = self.legs[i][1].slerp(self.legs[i][0], t);
            let at = pos + leg.perpendicular() * (InsectDraw::side(i) * 2.0 * self.size);
            let len = if b.submerged { 7.0 } else { 5.0 } * self.size * up;
            items.push(InsectDraw::sprite(
                1 + i,
                "pixel",
                at,
                leg.to_degrees(),
                Vec2::new(1.0, len),
                0.0,
                color,
            ));
        }
        items
    }
}

impl KindSprites for GrassHopper {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let rot = self.last_rot.slerp(self.rot, t);
        pos.y -= 5.0 * ground;
        let color = cx.black().lerp(
            cx.texel(30, 3).lerp(cx.texel(30, 2), self.color_fac2),
            self.color_fac,
        );
        let up = 1.0 - ground;
        let mut items = vec![InsectDraw::sprite(
            0,
            "pixel",
            pos,
            rot.to_degrees(),
            Vec2::new(1.2 * up, 6.0 * up),
            0.2,
            color,
        )];
        for i in 0..2 {
            let leg = self.legs[i][1].slerp(self.legs[i][0], t);
            items.push(InsectDraw::sprite(
                1 + i,
                "pixel",
                pos + leg.perpendicular() * InsectDraw::side(i),
                leg.to_degrees(),
                Vec2::new(1.0, 3.0 * up),
                0.0,
                color,
            ));
        }
        items
    }
}

impl KindSprites for RedSwarmer {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let dir = self.last_dir.slerp(self.dir, t);
        pos.y -= 5.0 * ground;
        let up = 1.0 - ground;
        let (black, dark) = (cx.black(), cx.darkness);
        let red = Color::hsl(0.005.lerp(0.03, self.hue), 1.0, 0.5).lerp(black, 0.1 + 0.6 * dark);
        let star = Color::hsl(0.005.lerp(0.03, self.hue), 1.0, 0.6).lerp(black, 0.4 * dark);
        let wing = Color::hsl(0.01.lerp(0.03, self.hue) + 0.02, 1.0, 0.6)
            .lerp(cx.fog(), 0.4)
            .lerp(black, 0.5 * dark);
        let deg = dir.to_degrees();
        let mut items = vec![
            InsectDraw::sprite(
                0,
                "Circle20",
                pos,
                deg,
                Vec2::new(3.0 * up / 20.0, 6.0 * up / 20.0),
                0.3,
                red,
            ),
            InsectDraw::sprite(1, "pixel", pos, deg, Vec2::new(1.0, 11.0 * up), 0.0, red),
            // The game offsets y by the direction's x too.
            InsectDraw::sprite(
                2,
                "tinyStar",
                Vec2::new(pos.x + dir.x * 2.0 - 0.5, pos.y + dir.x * 2.0 + 0.5),
                0.0,
                Vec2::new(1.0, 1.0),
                0.5,
                star,
            ),
        ];
        for i in 0..2 {
            let spread = cx.value().mathf_pow(0.5) * 40.0;
            let sign = if cx.value() < 0.5 { -1.0 } else { 1.0 };
            let rotation = spread * sign + if i == 0 { -90.0 } else { 90.0 };
            items.push(InsectDraw::sprite(
                3 + i,
                "pixel",
                pos,
                rotation,
                Vec2::new(up, 4.0 * up),
                -1.2,
                wing,
            ));
        }
        items
    }
}

impl KindSprites for Ant {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let ground = b.in_ground(t);
        let depth_point = Vec2::new(cx.screen.x / 2.0, cx.screen.y * (2.0 / 3.0));
        let mut screen = (b.at(t) - cx.cam_pos).apply_depth(depth_point, self.depth * 30.0);
        screen.y -= 5.0 * ground;
        let color = cx
            .texel((30.0 * self.depth) as usize, 2)
            .lerp(cx.black(), 0.5)
            .lerp(cx.fog(), self.depth * (0.3 + 0.2 * cx.fog_amount));
        let rotation = self.last_dir.slerp(self.dir, t).to_degrees();
        let mut item = InsectDraw::sprite(
            0,
            "pixel",
            screen + cx.cam_pos,
            rotation,
            Vec2::new(2.0, 4.0),
            0.5,
            color.with_alpha(1.0 - self.depth),
        );
        item.set_layer(Layer::Foreground);
        vec![item]
    }
}

impl KindSprites for Beetle {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let rot = self.last_rot.slerp(self.rot, t);
        let wings = self
            .last_wings_out
            .lerp(self.wings_out, t)
            .max(0.0)
            .mathf_pow(0.6);
        let deg = rot.to_degrees();
        pos.y -= 5.0 * ground;
        let up = 1.0 - ground;
        let black = cx.black();
        let wing = Color::WHITE.lerp(cx.fog(), 0.5 + 0.5 * cx.darkness);
        let mut items = vec![InsectDraw::sprite(
            0,
            "Circle20",
            pos,
            deg,
            Vec2::new((3.5 - wings * 0.5) * up / 20.0, 6.0 * up / 20.0),
            0.2,
            black,
        )];
        // `WingSprite(wing, part)` = 1 + part * 2 + wing: the two wings' first parts
        // (sprites 1, 2), then their second parts (3, 4).
        let mut parts = [None, None, None, None];
        for i in 0..2 {
            let flap =
                (45.0 + cx.value() * 90.0 * wings.mathf_pow(4.0)) * wings * InsectDraw::side(i)
                    + deg;
            parts[i] = Some(InsectDraw::sprite(
                1 + i,
                "pixel",
                pos,
                flap,
                Vec2::new(wings, 3.0 * up),
                -wings,
                wing,
            ));
            parts[2 + i] = Some(InsectDraw::sprite(
                3 + i,
                "pixel",
                pos,
                0.0.lerp(135.0, wings) * InsectDraw::side(i) + deg,
                Vec2::new(1.0, 5.0 * up),
                0.0,
                black,
            ));
        }
        items.extend(parts.into_iter().flatten());
        items
    }
}

impl KindSprites for WaterGlowworm {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let n = self.segments.len();
        let ground = b.in_ground(t);
        let breath = self.last_breath.lerp(self.breath, t);
        let pi = std::f32::consts::PI;
        let mut items: Vec<Option<DrawItem>> = vec![None; (n + 1) * 4];
        let black = cx.black();
        let glow = Color::rgb(0.0, 1.0 / 255.0, 0.0);
        for i in 0..=n {
            let pulse = f64::from((breath + 0.04 * i as f32) * 2.0 * pi).sin() as f32;
            let legs = 0.5 + 0.5 * f64::from((breath + 0.1 * i as f32) * 7.0 * pi).sin() as f32;
            let head = b.at(t);
            let mut p = if i == 0 {
                head
            } else {
                self.segments[i - 1][1].lerp(self.segments[i - 1][0], t)
            };
            let v = match i {
                0 => -self.last_rot.slerp(self.rot, t),
                1 => p.direction_to(head),
                _ => p.direction_to(self.segments[i - 2][1].lerp(self.segments[i - 2][0], t)),
            };
            p.y -= 5.0 * ground;
            let size = (i as f32).lerp_map_pow(0.0, n as f32, 1.0, 0.5, 1.2) * (1.0 - ground);
            let deg = v.to_degrees();
            let perp = v.perpendicular();
            let leg_len = (3.5 + (i * 2) as f32).lerp(
                3.0,
                (f64::from((i as f32).inverse_lerp(0.0, n as f32) * pi)).sin() as f32,
            ) * size;
            for j in 0..2 {
                let rotation = deg
                    + ((-20.0).lerp(70.0, legs) + (i as f32).lerp_map(0.0, n as f32, 70.0, 140.0))
                        * InsectDraw::side(j);
                items[i * 2 + j] = Some(InsectDraw::sprite(
                    i * 2 + j,
                    "pixel",
                    p - perp * (2.0 * size * InsectDraw::side(j)),
                    rotation,
                    Vec2::new(1.0, leg_len),
                    0.0,
                    black,
                ));
            }
            let s0 = (n + 1) * 2 + i * 2;
            items[s0] = Some(InsectDraw::sprite(
                s0,
                "Circle20",
                p,
                deg,
                Vec2::new(
                    4.0 * size * (1.0 - ground) / 20.0,
                    6.5 * size * (1.0 - ground) / 20.0,
                ),
                0.3,
                black,
            ));
            items[s0 + 1] = Some(InsectDraw::sprite(
                s0 + 1,
                "Circle20",
                p,
                deg,
                Vec2::new(
                    3.0 * size * (1.0 - ground) * pulse / 20.0,
                    5.5 * size * (1.0 - ground) * pulse / 20.0,
                ),
                0.4,
                glow,
            ));
        }
        items.into_iter().flatten().collect()
    }
}

impl KindSprites for Wasp {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let rot = self.last_rot.slerp(self.rot, t);
        let wings = self
            .last_wings_out
            .lerp(self.wings_out, t)
            .max(0.0)
            .mathf_pow(0.6);
        pos.y -= 5.0 * ground;
        let up = 1.0 - ground;
        let body = Color::hsl(0.35.lerp(0.4, self.color_fac), 1.0, 0.07)
            .lerp(cx.black(), 0.2 + 0.8 * cx.darkness);
        let wing = Color::WHITE.lerp(cx.fog(), 0.5 + 0.5 * cx.darkness);
        let down = rot.slerp(Vec2::new(0.0, -1.0), 0.4);
        let mut items = vec![
            InsectDraw::sprite(
                0,
                "Circle20",
                pos,
                down.to_degrees(),
                Vec2::new((3.0 - wings * 0.5) * up / 20.0, 5.0 * up / 20.0),
                -0.2,
                body,
            ),
            InsectDraw::sprite(
                1,
                "Circle20",
                pos,
                rot.to_degrees(),
                Vec2::new(2.5 * up / 20.0, 4.0 * up / 20.0),
                0.7,
                body,
            ),
            InsectDraw::sprite(
                2,
                "pixel",
                pos + down * 4.0,
                rot.slerp(Vec2::new(0.0, -1.0), 0.6).to_degrees(),
                Vec2::new(1.0, 5.0 * up),
                0.0,
                body,
            ),
        ];
        let base = rot.slerp(Vec2::new(0.0, 1.0), 0.8 * wings).to_degrees();
        for i in 0..2 {
            let rotation =
                (45.0 + cx.value() * 90.0 * wings.mathf_pow(4.0)) * wings * InsectDraw::side(i)
                    + base;
            items.push(InsectDraw::sprite(
                3 + i,
                "pixel",
                pos,
                rotation,
                Vec2::new(1.0, 5.0 * up),
                -0.5 * wings,
                wing,
            ));
        }
        items
    }
}

impl KindSprites for Moth {
    fn sprites(&self, b: &InsectBase, t: f32, cx: &mut InsectDraw) -> Vec<DrawItem> {
        let mut pos = b.at(t);
        let ground = b.in_ground(t);
        let rot = self.last_rot.slerp(self.rot, t);
        let wings = self
            .last_wings_out
            .lerp(self.wings_out, t)
            .max(0.0)
            .mathf_pow(0.6);
        let deg = rot.to_degrees();
        let flap = self.last_flap.lerp(self.flap, t).s_curve(0.6);
        pos.y -= 5.0 * ground;
        let up = 1.0 - ground;
        // `colorFac` is never set: palette column `(int)Lerp(14, 20, 0)` = 14.
        let color = cx
            .texel(14, 2)
            .lerp(Color::WHITE, 0.85)
            .lerp(cx.fog(), 0.1 * cx.fog_amount + 0.75 * cx.darkness);
        let mut items = vec![InsectDraw::sprite(
            0,
            "Circle20",
            pos,
            deg,
            Vec2::new((2.5 - wings * 0.5) * up / 20.0, 5.0 * up / 20.0),
            0.2,
            color,
        )];
        let mut antennae = Vec::new();
        let mut wing_sprites = Vec::new();
        for i in 0..2 {
            let at = pos - rot * 3.0 + rot.perpendicular() * (InsectDraw::side(i) * 1.6);
            antennae.push(InsectDraw::sprite(
                1 + i,
                "pixel",
                at,
                0.0,
                Vec2::new(1.0, 1.0),
                0.5,
                color,
            ));
            let rotation =
                (30.0 + flap * 120.0 * wings.mathf_pow(4.0)) * wings * InsectDraw::side(i) + deg;
            wing_sprites.push(InsectDraw::sprite(
                3 + i,
                "pixel",
                pos,
                rotation,
                Vec2::new(wings * 1.2, 6.0 * up),
                -0.1,
                color,
            ));
        }
        items.extend(antennae);
        items.extend(wing_sprites);
        items
    }
}
