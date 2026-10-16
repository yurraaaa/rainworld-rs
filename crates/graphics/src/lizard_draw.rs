//! `LizardGraphics.DrawSprites`, `ApplyPalette` and `AddToContainer` for the body:
//! the body circles and mesh, the tail mesh, the legs and their colour layer, the five
//! head sprites; in the midground, in the game's container order. Its draws (jaw twitch,
//! head jitter, flicker colour, the every-second-draw toggle) come from the cosmetic
//! stream (D1). The cosmetics' sprites are lizard step 5b.

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::creature::LizardAnimation;
use rw_sim::creature::lizard::LizardView;
use rw_sim::{FloatExt, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::elements::{
    LIZARD_ARM, LIZARD_ARM_COLOR, LIZARD_EYES, LIZARD_HEAD, LIZARD_JAW, LIZARD_LOWER_TEETH,
    LIZARD_UPPER_TEETH,
};
use crate::lizard_graphics::LizardGraphics;
use crate::math::CustomVec;

/// `SpriteBodyMesh`'s triangles: two per quad of the four body segments.
const BODY_TRIANGLES: [[u16; 3]; 8] = [
    [0, 1, 2],
    [1, 2, 3],
    [4, 5, 6],
    [5, 6, 7],
    [8, 9, 10],
    [9, 10, 11],
    [12, 13, 14],
    [13, 14, 15],
];

/// Sprite indices for four limbs.
const BODY_MESH: usize = 5;
const TAIL: usize = 6;
const LIMBS: usize = 7;
const HEAD: usize = 11;
const LIMB_COLORS: usize = 16;

/// `AddToContainer`'s order of the base sprites (body circle 4 is never placed).
const CONTAINER_ORDER: [usize; 20] = [
    7, 16, 9, 18, 5, 0, 1, 2, 3, 4, 6, 8, 17, 10, 19, 11, 12, 13, 14, 15,
];

impl Color {
    /// A sim colour, opaque.
    #[must_use]
    pub fn from_rgb(c: [f32; 3]) -> Self {
        Self::rgb(c[0], c[1], c[2])
    }
}

impl LizardGraphics {
    /// `HeadColor(t)`: white during the strong white flicker (or every second draw of a
    /// weak one), else between the palette's black and the effect colour by the blink,
    /// the head colour setter and the stun flicker.
    #[must_use]
    pub fn head_color(&self, view: &LizardView<'_>, black: Color, t: f32) -> Color {
        if self.white_flicker > 0 && (self.white_flicker > 15 || self.every_second_draw) {
            return Color::WHITE;
        }
        let blink = self.last_blink.lerp(self.blink, t);
        let wave = 0.5 + 0.5 * (blink * 2.0 * std::f32::consts::PI).sin();
        let mut a = 1.0 - wave.mathf_pow(1.5 + view.lizard.ai.excitement * 1.5);
        if self.head_color_setter != 0.0 {
            let to = if self.head_color_setter > 0.0 {
                1.0
            } else {
                0.0
            };
            a = a.lerp(to, self.head_color_setter.abs());
        }
        if self.flicker > 10 {
            a = self.flicker_color;
        }
        black.lerp(Color::from_rgb(self.effect_color), a)
    }

    /// `BodyColor(f)` at `f` along body and tail: black, the tail toward the effect
    /// colour by `tailColor`.
    #[must_use]
    pub fn body_color(&self, view: &LizardView<'_>, black: Color, f: f32) -> Color {
        let body = self.body_length / (self.body_length + self.tail_length);
        if f < body || self.ivars.tail_color == 0.0 {
            return black;
        }
        let params = view.lizard.params;
        let v = f.inverse_lerp(body, 1.0);
        let c = v
            .inverse_lerp(params.tail_coloration_start, 0.95)
            .clamp(0.0, 1.0)
            .mathf_pow(params.tail_coloration_exponent)
            * self.ivars.tail_color;
        black.lerp(Color::from_rgb(self.effect_color), c)
    }

    /// `BodyPosition(p, t)`.
    fn body_position(&self, p: usize, t: f32) -> Vec2 {
        if p > 3 {
            return Vec2::new(100.0, 100.0);
        }
        let n = if p < 2 { 1 } else { 2 };
        let at = self.dp(n, t);
        if p % 2 == 1 {
            return at;
        }
        let next = if n == 1 {
            self.dp(2, t)
        } else {
            self.tail[0].part.last_pos.lerp(self.tail[0].part.pos, t)
        };
        let prev = self.dp(n - 1, t);
        let dir = (at - prev).slerp(next - at, 0.35).normalized() * (-0.5 * at.distance(prev));
        at + dir
    }

    /// `drawPositions[i]` at `t`.
    fn dp(&self, i: usize, t: f32) -> Vec2 {
        self.draw_positions[i][1].lerp(self.draw_positions[i][0], t)
    }

    /// `DrawSprites` at `t` between ticks, in the palette's `black`; draws on `rng`.
    /// Nothing while culled.
    pub fn draw(
        &mut self,
        view: &LizardView<'_>,
        black: Color,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList {
        if self.culled {
            return DrawList::default();
        }
        let mut slots: Vec<Option<DrawItem>> = vec![None; 20];
        let lizard = view.lizard;
        let params = lizard.params;
        let depth = self
            .last_head_depth_rotation
            .lerp(self.head_depth_rotation, t);
        let mut jaw = lizard.last_jaw_open.lerp(view.jaw_open(), t);
        let ready = view.jaw_open() > 0.9 && lizard.bite_delay == 0;
        if ready && lizard.conscious() {
            jaw += rng.value() * 0.2;
        }
        let jaw = jaw.clamp(0.0, 1.0);
        self.draw_body(view, black, t, rng, &mut slots);
        self.draw_limbs(view, black, t, &mut slots);
        self.draw_tail(view, black, t, &mut slots);
        let head = self.head.last_pos.lerp(self.head.pos, t);
        let mut at = head.lerp(self.dp(0, t), 0.2);
        if lizard.conscious() {
            if lizard.bubble > 0 {
                let deg = rng.value() * 360.0;
                at += Vec2::from_degrees(deg) * (2.0 * lizard.bubble_intensity * 0.5);
            }
            if self.white_flicker > 0 {
                let deg = rng.value() * 360.0;
                at += Vec2::from_degrees(deg) * (rng.value() * 2.0);
            }
            if matches!(
                lizard.animation,
                LizardAnimation::PreySpotted
                    | LizardAnimation::PreyReSpotted
                    | LizardAnimation::ThreatSpotted
                    | LizardAnimation::ThreatReSpotted
            ) {
                let deg = rng.value() * 360.0;
                at += Vec2::from_degrees(deg) * rng.value();
            }
        }
        let aim = self.dp(0, t).aim_to(head);
        let side = (self.dp(0, t) - head).perpendicular();
        let turn = (3 - (depth.abs() * 3.9) as i32).clamp(0, 3) as usize;
        let graphics = params
            .head_graphics
            .map(|g| usize::try_from(g).unwrap_or(0).min(9));
        let elements = [
            LIZARD_JAW[turn][graphics[0]],
            LIZARD_LOWER_TEETH[turn][graphics[1]],
            LIZARD_UPPER_TEETH[turn][graphics[2]],
            LIZARD_HEAD[turn][graphics[3]],
            LIZARD_EYES[turn][graphics[4]],
        ];
        let head_color = self.head_color(view, black, t);
        let size = params.head_size * self.ivars.head_size;
        let sign = if depth >= 0.0 { 1.0 } else { -1.0 };
        let lower = params.jaw_open_lower_jaw_fac;
        for (i, element) in elements.into_iter().enumerate() {
            let (pos, rotation) = if i > 1 {
                let off = jaw * depth * params.jaw_open_move_jaws_apart * (1.0 - lower);
                (
                    at + side * off,
                    aim + params.jaw_open_angle * (1.0 - lower) * jaw * depth,
                )
            } else {
                let off = jaw * depth * params.jaw_open_move_jaws_apart * lower;
                (
                    at - side * off,
                    aim - params.jaw_open_angle * lower * jaw * depth,
                )
            };
            let color = if i == 0 || i == 3 { head_color } else { black };
            slots[HEAD + i] = Some(Self::sprite(
                HEAD + i,
                element,
                pos,
                rotation,
                Vec2::new(sign * size, size),
                Vec2::new(0.5, Self::head_anchor_y(view, i)),
                color,
            ));
        }
        if self.flicker > 10 {
            self.flicker_color = rng.value();
        }
        if rng.value() > 0.025 {
            self.every_second_draw = !self.every_second_draw;
        }
        DrawList {
            items: CONTAINER_ORDER
                .iter()
                .filter_map(|&i| slots[i].take())
                .collect(),
        }
    }

    /// The head sprites' `anchorY`: 0.55 for the green lizard, else 0.7; graphics 8,
    /// 9 and eyes 3 have their own.
    fn head_anchor_y(view: &LizardView<'_>, i: usize) -> f32 {
        let g = view.lizard.params.head_graphics;
        let mut y = if view.kind == rw_assets::CreatureType::GreenLizard {
            0.55
        } else {
            0.7
        };
        if i == 4 && g[4] == 3 {
            y = 0.75;
        }
        match g[i] {
            9 => 0.61,
            8 => 0.65,
            _ => y,
        }
    }

    /// The body circles and mesh: four quads from the head to the hips, thicker as the
    /// lizard breathes at rest.
    fn draw_body(
        &self,
        view: &LizardView<'_>,
        black: Color,
        t: f32,
        rng: &mut UnityRandom,
        slots: &mut [Option<DrawItem>],
    ) {
        let lizard = view.lizard;
        let mut width = 5.0f32;
        let head = self.head.last_pos.lerp(self.head.pos, t);
        let mut a = self.dp(0, t).lerp(head, 0.2);
        let breath = self.last_breath.lerp(self.breath, t);
        let swell = ((breath * std::f32::consts::PI * 2.0).sin() + 1.0)
            * 0.5
            * (1.0 - lizard.ai.run_speed).mathf_pow(2.0);
        let threat = matches!(
            lizard.animation,
            LizardAnimation::ThreatSpotted | LizardAnimation::ThreatReSpotted
        );
        let mut vertices = vec![Vec2::ZERO; 16];
        for j in 0..4 {
            let chunk = if j < 2 { 1 } else { 2 };
            let mut v = self.body_position(j, t);
            if threat {
                let deg = rng.value() * 360.0;
                v += Vec2::from_degrees(deg) * (rng.value() * 2.0);
            }
            let mut rad = Self::body_chunk_display_rad(view, chunk);
            if j % 2 == 0 {
                rad = (rad + Self::body_chunk_display_rad(view, chunk - 1)) / 2.0;
            }
            rad *= 1.0 + swell * (3 - j) as f32 * 0.1 * if j == 0 { 0.5 } else { 1.0 };
            rad *= self.ivars.fatness;
            let dir = (a - v).normalized();
            let perp = dir.perpendicular();
            let dist = v.distance(a);
            vertices[j * 4] = v + dir * dist - perp * width;
            vertices[j * 4 + 1] = v + dir * dist + perp * width;
            vertices[j * 4 + 2] = v - perp * rad;
            vertices[j * 4 + 3] = v + perp * rad;
            width = rad;
            a = v;
            slots[j] = Some(Self::sprite(
                j,
                "Circle20",
                v,
                0.0,
                Vec2::new(rad / 10.0, rad / 10.0),
                Vec2::new(0.5, 0.5),
                black,
            ));
        }
        slots[BODY_MESH] = Some(DrawItem::Mesh(Mesh {
            index: BODY_MESH,
            vertices,
            uvs: Vec::new(),
            triangles: Cow::Borrowed(&BODY_TRIANGLES),
            color: black,
            vertex_colors: Vec::new(),
            shader: Shader::Basic,
            layer: Layer::Midground,
        }));
    }

    /// The legs: an arm element by reach and turn (front 1-27, hind 28-54), and its
    /// colour layer, the front pair's fading as the body turns.
    fn draw_limbs(
        &self,
        view: &LizardView<'_>,
        black: Color,
        t: f32,
        slots: &mut [Option<DrawItem>],
    ) {
        let params = view.lizard.params;
        let depth = self.last_depth_rotation.lerp(self.depth_rotation, t).abs();
        let effect = Color::from_rgb(self.effect_color);
        for (k, l) in self.limbs.iter().enumerate() {
            let pos = l.limb.part.last_pos.lerp(l.limb.part.pos, t);
            let mut base = self.dp(if k >= 2 { 2 } else { 0 }, t);
            if k < 2 {
                base = base.lerp(self.dp(1, t), 0.2);
            }
            let reach = ((pos.distance(base) / (4.0 * params.limb_size)) as i32 + 1).clamp(1, 9);
            let turned = (l.flip.abs() * 3.0).clamp(0.0, 2.0) as i32;
            let mut frame = reach + 9 * (2 - turned);
            if k >= 2 {
                frame += 27;
            }
            let frame = usize::try_from(frame - 1).unwrap_or(0).min(53);
            let rotation = base.aim_to(pos) - 90.0;
            let sign = if l.flip >= 0.0 { 1.0 } else { -1.0 };
            let scale = Vec2::new(params.limb_size, sign * params.limb_thickness);
            let alpha = if k % 2 == 0 {
                1.0f32.lerp(0.3, depth)
            } else {
                1.0
            };
            slots[LIMBS + k] = Some(Self::sprite(
                LIMBS + k,
                LIZARD_ARM[frame],
                pos,
                rotation,
                scale,
                Vec2::new(0.5, 0.5),
                black,
            ));
            slots[LIMB_COLORS + k] = Some(Self::sprite(
                LIMB_COLORS + k,
                LIZARD_ARM_COLOR[frame],
                pos,
                rotation,
                scale,
                Vec2::new(0.5, 0.5),
                effect.with_alpha(alpha),
            ));
        }
    }

    /// The tail mesh, narrowing to a point; coloured toward the tip by `tailColor`.
    fn draw_tail(
        &self,
        view: &LizardView<'_>,
        black: Color,
        t: f32,
        slots: &mut [Option<DrawItem>],
    ) {
        let n = self.tail.len();
        let fat = self.ivars.fatness;
        let tail_fat = self.ivars.tail_fatness;
        let mut a = self.dp(2, t);
        let mut width = Self::body_chunk_display_rad(view, 2) * fat;
        let mut vertices = Vec::with_capacity(4 * n - 1);
        for (i, seg) in self.tail.iter().enumerate() {
            let p = seg.part.last_pos.lerp(seg.part.pos, t);
            let dir = (p - a).normalized();
            let perp = dir.perpendicular();
            let step = p.distance(a) / 5.0;
            let w = perp * ((width + seg.stretched_rad()) * 0.5 * fat * tail_fat);
            vertices.push(a - w + dir * step);
            vertices.push(a + w + dir * step);
            if i < n - 1 {
                let w = perp * (seg.stretched_rad() * fat * tail_fat);
                vertices.push(p - w - dir * step);
                vertices.push(p + w - dir * step);
            } else {
                vertices.push(p);
            }
            width = seg.stretched_rad() * fat * tail_fat;
            a = p;
        }
        let mut triangles = Vec::with_capacity((n - 1) * 4 + 1);
        for m in 0..n - 1 {
            let b = u16::try_from(m * 4).unwrap_or(0);
            for k in 0..4 {
                triangles.push([b + k, b + k + 1, b + k + 2]);
            }
        }
        let b = u16::try_from((n - 1) * 4).unwrap_or(0);
        triangles.push([b, b + 1, b + 2]);
        let count = vertices.len();
        let body = self.body_length / (self.body_length + self.tail_length);
        let vertex_colors = if self.ivars.tail_color > 0.0 {
            (0..count)
                .map(|j| {
                    let f = (j / 2) as f32 * 2.0 / (count - 1) as f32;
                    self.body_color(view, black, body.lerp(1.0, f))
                })
                .collect()
        } else {
            Vec::new()
        };
        slots[TAIL] = Some(DrawItem::Mesh(Mesh {
            index: TAIL,
            vertices,
            uvs: Vec::new(),
            triangles: Cow::Owned(triangles),
            color: black,
            vertex_colors,
            shader: Shader::Basic,
            layer: Layer::Midground,
        }));
    }

    /// One midground sprite.
    fn sprite(
        index: usize,
        element: &'static str,
        pos: Vec2,
        rotation: f32,
        scale: Vec2,
        anchor: Vec2,
        color: Color,
    ) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element,
            pos,
            rotation,
            scale,
            anchor,
            color,
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }
}
