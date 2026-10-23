//! `Spark.DrawSprites` (40745), `ExplosionSpikes.DrawSprites` (34935) and
//! `NeuronSpark.DrawSprites`: a streak in `Midground` (in the lizard's head colour while its
//! graphics flicker white), a ring of fading spikes in `Foreground`, and a neuron spark's
//! flickering disc and pixel in `Foreground`, and the dispatch of every cosmetic's draw (the
//! lizard's bubble and flash in `creature::lizard::bubble`, the air bubble and the splash in
//! `water`). The neuron spark draws on the cosmetic stream (D1).
//!
//! Mirrors: Spark.cs, ExplosionSpikes.cs, NeuronSpark.cs (InitiateSprites, DrawSprites)

use std::borrow::Cow;

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::creature::CreatureId;
use rw_sim::{Cosmetic, CosmeticKind, ExplosionSpikes, NeuronSpark, RoomExt, Spark, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::core::math::Mathf;
use crate::creature::lizard::bubble::{BubbleSprite, FlashSprite};
use crate::creature::lizard::draw::LizardReading;
use crate::creature::lizard::graphics::LizardGraphics;
use crate::effects::gameplay::electric_death::SparkFlashSprites;
use crate::water::bubble::AirBubbleSprite;
use crate::water::splash::SplashSprites;

const STREAK: [[u16; 3]; 1] = [[0, 1, 2]];

/// What a bubble, spark or flash reads of the `LizardGraphics` it was given when drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LizardModule {
    /// `HeadColor(t)`.
    pub head_color: Color,
    /// `palette.blackColor`.
    pub black: Color,
    /// `head` between `lastPos` and `pos` at `t`.
    pub head: Vec2,
    /// `HeadRotation(t)`.
    pub rotation: f32,
    pub white_flicker: i32,
}

impl LizardGraphics {
    /// What its cosmetics read at `t`, `black` standing in for a palette it never got.
    #[must_use]
    pub fn for_cosmetics(&self, lizard: &LizardReading, black: Color, t: f32) -> LizardModule {
        let black = self.palette_black.unwrap_or(black);
        LizardModule {
            head_color: self.head_color(lizard, black, t),
            black,
            head: self.head.last_pos.lerp(self.head.pos, t),
            rotation: self.head_rotation(lizard, t),
            white_flicker: self.white_flicker,
        }
    }
}

/// What the lizards' cosmetics read of their graphics when drawn.
pub trait LizardColors {
    /// Graphics module `module` (lizard `id`'s) at `t`, kept while a cosmetic given it
    /// lives.
    fn lizard_module(&self, id: CreatureId, module: u64, t: f32) -> Option<LizardModule>;
    /// The room palette's black.
    fn black(&self) -> Color;
}

/// No lizard graphics (a room nobody views).
pub struct NoLizards(pub Color);

impl LizardColors for NoLizards {
    fn lizard_module(&self, _id: CreatureId, _module: u64, _t: f32) -> Option<LizardModule> {
        None
    }

    fn black(&self) -> Color {
        self.0
    }
}

/// The sprites of a cosmetic sprite.
pub trait CosmeticSprites {
    /// `DrawSprites` at `t` between ticks in `room`; the flash draws on `rng`.
    fn draw(
        &self,
        room: &Room,
        t: f32,
        lizards: &dyn LizardColors,
        rng: &mut UnityRandom,
    ) -> DrawList;
}

impl CosmeticSprites for Cosmetic {
    fn draw(
        &self,
        room: &Room,
        t: f32,
        lizards: &dyn LizardColors,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let item = match &self.kind {
            CosmeticKind::Spark(s) => DrawItem::Mesh(s.lizard_mesh(room, t, lizards)),
            CosmeticKind::Spikes(s) => DrawItem::Mesh(s.mesh(room, t)),
            // The `OverseerZip` shader's glow is not drawn.
            CosmeticKind::OverseerEffect(_) => return DrawList { items: Vec::new() },
            CosmeticKind::LizardBubble(b) => b.sprite(t, lizards),
            CosmeticKind::Flash(f) => f.sprite(t, lizards, rng),
            CosmeticKind::Bubble(b) => b.sprite(t),
            CosmeticKind::Splash(s) => {
                return DrawList {
                    items: s.sprites(t),
                };
            }
            CosmeticKind::SparkFlash(f) => {
                return DrawList {
                    items: f.sprites(t, rng),
                };
            }
            CosmeticKind::NeuronSpark(s) => {
                return DrawList {
                    items: s.sprites(t, rng),
                };
            }
        };
        DrawList { items: vec![item] }
    }
}

/// `NeuronSpark.InitiateSprites` and `DrawSprites`.
trait NeuronSparkSprites {
    /// A blue `FlatLight` disc and a rotated pixel at the spark, both flickering with
    /// `life` and five cosmetic draws.
    fn sprites(&self, t: f32, rng: &mut UnityRandom) -> Vec<DrawItem>;
}

impl NeuronSparkSprites for NeuronSpark {
    fn sprites(&self, t: f32, rng: &mut UnityRandom) -> Vec<DrawItem> {
        let pos = self.last_pos.lerp(self.pos, t);
        let life = self.last_life.lerp_to(self.life, t);
        let glow = rw_sim::FloatExt::mathf_pow(life, 3.0) * rng.value();
        let pixel_scale = 4.0 * rng.value() * life;
        let disc_alpha = rng.value() * life;
        let wide = 15.0 * rng.value();
        let disc_scale =
            (wide + 15.0 * rw_sim::FloatExt::mathf_pow(rng.value(), 3.0)) * life / 10.0;
        vec![
            DrawItem::Sprite(Sprite {
                index: 0,
                element: "Futile_White",
                pos,
                rotation: 0.0,
                scale: Vec2::new(disc_scale, disc_scale),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::rgb(0.0, 0.0, 1.0).with_alpha(disc_alpha),
                shader: Shader::FlatLight,
                layer: Layer::Foreground,
            }),
            DrawItem::Sprite(Sprite {
                index: 1,
                element: "pixel",
                pos,
                rotation: 45.0,
                scale: Vec2::new(pixel_scale, pixel_scale),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::rgb(glow, glow, 1.0),
                shader: Shader::default(),
                layer: Layer::Foreground,
            }),
        ]
    }
}

/// A sprite's colour from the sim's RGBA.
trait Rgba {
    fn color(self) -> Color;
}

impl Rgba for [f32; 4] {
    fn color(self) -> Color {
        Color {
            r: self[0],
            g: self[1],
            b: self[2],
            a: self[3],
        }
    }
}

/// A spark's or the spikes' triangles.
trait CosmeticMesh {
    fn mesh(&self, room: &Room, t: f32) -> Mesh;
}

/// `Spark.DrawSprites`' colour.
trait SparkMesh {
    fn lizard_mesh(&self, room: &Room, t: f32, lizards: &dyn LizardColors) -> Mesh;
}

impl SparkMesh for Spark {
    /// The head colour of the module it was given while that flickers white, else its
    /// own colour.
    fn lizard_mesh(&self, room: &Room, t: f32, lizards: &dyn LizardColors) -> Mesh {
        let mut m = self.mesh(room, t);
        if let Some(module) = self
            .lizard
            .and_then(|(id, module)| lizards.lizard_module(id, module, t))
            && module.white_flicker > 0
        {
            m.color = module.head_color;
        }
        m
    }
}

impl CosmeticMesh for Spark {
    /// From the lerped position back toward the position three ticks ago (at least
    /// 9 px), shrinking to a point over the last tenth of its life; 2 px wide.
    fn mesh(&self, _room: &Room, t: f32) -> Mesh {
        let head = self.last_pos.lerp(self.pos, t);
        let mut tail = self.last_last_last_pos.lerp(self.last_last_pos, t);
        if head.dist_less(tail, 9.0) {
            tail = head + head.direction_to(tail) * 9.0;
        }
        tail = head.lerp(tail, self.life.inverse_lerp(0.0, 0.1));
        let side = (head - tail).normalized().perpendicular();
        Mesh {
            index: 0,
            vertices: vec![head + side, head - side, tail],
            uvs: Vec::new(),
            triangles: Cow::Borrowed(&STREAK),
            color: self.color.color(),
            vertex_colors: Vec::new(),
            shader: Shader::Basic,
            layer: Layer::Midground,
        }
    }
}

impl CosmeticMesh for ExplosionSpikes {
    /// Each spike grows from a tenth of its length and narrows as it fades over its
    /// lifetime, white at first; a tip inside a Solid tile fades twice as fast.
    fn mesh(&self, room: &Room, t: f32) -> Mesh {
        let value = self.time as f32 + t;
        let mut vertices = Vec::new();
        let mut colors = Vec::new();
        let mut triangles = Vec::new();
        let white = Color::WHITE;
        let base = self.color.color();
        for (i, (v, dir)) in self.spikes.iter().enumerate() {
            let num = value.inverse_lerp(0.0, v[2]);
            let mut num2 = if self.time == 0 {
                t
            } else {
                value.inverse_lerp(v[2], 0.0)
            };
            let num3 = (v[0] * 0.1).lerp_to(v[0], num.powf(0.45));
            let num4 =
                v[1] * (0.5 + 0.5 * (num * std::f32::consts::PI).mathf_sin()) * num2.powf(0.3);
            let col = base.lerp(white, num2.powf(0.2f32.lerp_to(1.5, num)));
            let tip = self.pos + *dir * (self.inner_rad + num3);
            if room.tile_at_pos(tip).is_solid() {
                num2 *= 0.5;
            }
            let root = self.pos + *dir * (self.inner_rad + num3 * 0.1);
            let side = (tip - root).perpendicular();
            vertices.extend([tip, root - side * (num4 * 0.5), root + side * (num4 * 0.5)]);
            colors.extend([
                col.with_alpha(num2.powf(0.75)),
                col.with_alpha(0.0),
                col.with_alpha(0.0),
            ]);
            let k = u16::try_from(i * 3).unwrap_or(0);
            triangles.push([k, k + 1, k + 2]);
        }
        Mesh {
            index: 0,
            vertices,
            uvs: Vec::new(),
            triangles: Cow::Owned(triangles),
            color: white,
            vertex_colors: colors,
            shader: Shader::Basic,
            layer: Layer::Foreground,
        }
    }
}

#[cfg(test)]
#[path = "tests/cosmetic.rs"]
mod tests;
