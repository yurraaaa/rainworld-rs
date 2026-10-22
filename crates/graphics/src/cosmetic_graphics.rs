//! `Spark.DrawSprites` (40745), `ExplosionSpikes.DrawSprites` (34935),
//! `LizardBubble.DrawSprites`, `StationaryEffect.DrawSprites`, `Bubble.DrawSprites` and
//! `Splash.DrawSprites`: a streak in `Midground` (in the lizard's head colour while its
//! graphics flicker white), a ring of fading spikes in `Foreground`, a lizard's bubble
//! and its head shield's flash, an air bubble in the water, and a splash's two
//! `WaterSplash` quads. The flash draws on the cosmetic stream (D1).
//!
//! Mirrors: Spark.cs, ExplosionSpikes.cs, LizardBubble.cs, StationaryEffect.cs, Bubble.cs, SplashWater/Splash.cs (DrawSprites, InitiateSprites, AddToContainer)

use std::borrow::Cow;

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::creature::CreatureId;
use rw_sim::creature::lizard::LizardBubble;
use rw_sim::{
    Bubble, Cosmetic, CosmeticKind, ExplosionSpikes, RoomExt, Spark, Splash, StationaryEffect, Vec2,
};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::lizard_draw::LizardReading;
use crate::lizard_graphics::LizardGraphics;
use crate::math::{CustomVec, Mathf};

const STREAK: [[u16; 3]; 1] = [[0, 1, 2]];

/// `LizardBubble0` to `7`, by hollowness.
const BUBBLES: [&str; 8] = [
    "LizardBubble0",
    "LizardBubble1",
    "LizardBubble2",
    "LizardBubble3",
    "LizardBubble4",
    "LizardBubble5",
    "LizardBubble6",
    "LizardBubble7",
];

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
        };
        DrawList { items: vec![item] }
    }
}

/// A lizard bubble's or flash's sprite.
trait LizardCosmeticSprite {
    fn sprite_at(
        element: &'static str,
        pos: Vec2,
        scale: Vec2,
        rotation: f32,
        anchor_y: f32,
        color: Color,
    ) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index: 0,
            element,
            pos,
            rotation,
            scale,
            anchor: Vec2::new(0.5, anchor_y),
            color,
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }
}

impl LizardCosmeticSprite for LizardBubble {}
impl LizardCosmeticSprite for StationaryEffect {}

/// `LizardBubble.DrawSprites`.
trait BubbleSprite {
    fn sprite(&self, t: f32, lizards: &dyn LizardColors) -> DrawItem;
}

impl BubbleSprite for LizardBubble {
    /// Fading from the module's head colour to its black over the second half of its
    /// life, hollow rings once free, stretched toward its spot on the head while stuck
    /// to it. Without its module (a room nobody views) the camera's black and the head
    /// as the bubble last read it stand in.
    fn sprite(&self, t: f32, lizards: &dyn LizardColors) -> DrawItem {
        let last = (std::f32::consts::PI * self.last_life)
            .mathf_sin()
            .lerp_to(self.last_life, 0.5);
        let now = (std::f32::consts::PI * self.life)
            .mathf_sin()
            .lerp_to(self.life, 0.5);
        let size = 0.625 * last.lerp_to(now, t);
        let pos = self.last_pos.lerp(self.pos, t);
        let module = lizards.lizard_module(self.lizard, self.module, t);
        let black = module.map_or(lizards.black(), |m| m.black);
        let head = module.map_or(black, |m| m.head_color);
        let life = self.last_life.lerp_to(self.life, t);
        let color = head.lerp(black, 1.0 - (life * 2.0).clamp(0.0, 1.0));
        let mut frame = 0;
        if self.hollow {
            let h = self.life.inverse_lerp(self.life_time_when_free, 0.0);
            frame = ((rw_sim::FloatExt::mathf_pow(h, self.hollow_ness) * 7.0) as i32).clamp(1, 7);
        }
        let element = BUBBLES[usize::try_from(frame).unwrap_or(0)];
        if self.stuck_to_origin || !self.free_floating {
            let anchor = if self.stuck_to_origin {
                let (head, rotation) = module.map_or((self.head.pos, self.head.rotation), |m| {
                    (m.head, m.rotation)
                });
                head + self.origin_point.rotate_around_origo(rotation)
            } else {
                self.last_liberated_origin.lerp(self.liberated_origin, t)
            };
            let stretch = pos.distance(anchor) / 16.0;
            let scale = Vec2::new(
                size.min(size / stretch.lerp_to(1.0, 0.35)),
                size.max(stretch - 0.3125),
            );
            let anchor_y = stretch.inverse_lerp(1.25, 0.3125) * 0.5;
            Self::sprite_at(element, pos, scale, pos.aim_to(anchor), anchor_y, color)
        } else {
            Self::sprite_at(element, pos, Vec2::new(size, size), 0.0, 0.5, color)
        }
    }
}

/// `Bubble.DrawSprites`.
trait AirBubbleSprite {
    fn sprite(&self, t: f32) -> DrawItem;
}

impl AirBubbleSprite for Bubble {
    /// `LizardBubble5` in `Midground`, scaled `size * fullSize * 0.45`, in the
    /// `(0, 1/255, 0)` the water's shader shows as lit water (`backgroundBubble`, set
    /// only by More Slugcats' room scripts, is not ported: D11).
    fn sprite(&self, t: f32) -> DrawItem {
        let scale = self.last_size.lerp_to(self.size, t) * self.full_size * 0.45;
        DrawItem::Sprite(Sprite {
            index: 0,
            element: "LizardBubble5",
            pos: self.last_pos.lerp(self.pos, t),
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::rgb(0.0, 1.0 / 255.0, 0.0),
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }
}

/// `Splash.DrawSprites`.
trait SplashSprites {
    fn sprites(&self, t: f32) -> Vec<DrawItem>;
}

impl SplashSprites for Splash {
    /// Two `Futile_White` quads on the `WaterSplash` shader, anchored at their bottom
    /// edge on the splash and stretched to the linger point (`rad / 8` wide, `distance
    /// / 16` long), at half the life's alpha (clamped as `FNode.alpha`). Sprite 0, colour
    /// `(0, 0, 0)`, is in `Water`; sprite 1, `(0, 0, 1)` (the shader's foam), in
    /// `GrabShaders`.
    fn sprites(&self, t: f32) -> Vec<DrawItem> {
        let pos = self.last_pos.lerp(self.pos, t);
        let linger = self.last_linger_pos.lerp(self.linger_pos, t);
        let scale = Vec2::new(
            self.last_rad.lerp_to(self.rad, t) / 8.0,
            pos.distance(linger) / 16.0,
        );
        let alpha = (0.5 * self.last_life.lerp_to(self.life, t)).clamp(0.0, 1.0);
        [Layer::Water, Layer::GrabShaders]
            .into_iter()
            .enumerate()
            .map(|(i, layer)| {
                DrawItem::Sprite(Sprite {
                    index: i,
                    element: "Futile_White",
                    pos,
                    rotation: pos.aim_to(linger),
                    scale,
                    anchor: Vec2::new(0.5, 0.0),
                    color: Color::rgb(0.0, 0.0, i as f32).with_alpha(alpha),
                    shader: Shader::WaterSplash,
                    layer,
                })
            })
            .collect()
    }
}

/// `StationaryEffect.DrawSprites` of a `FlashingOrb`.
trait FlashSprite {
    fn sprite(&self, t: f32, lizards: &dyn LizardColors, rng: &mut UnityRandom) -> DrawItem;
}

impl FlashSprite for StationaryEffect {
    /// A circle jittering by up to 10 px, its size random, fading with its life; in
    /// the head colour of the module it was given (grey to white without one). Three
    /// or four draws. White stands in for a module this view lacks (a room nobody
    /// views).
    fn sprite(&self, t: f32, lizards: &dyn LizardColors, rng: &mut UnityRandom) -> DrawItem {
        let life = self.last_life.lerp_to(self.life, t);
        let deg = rng.value() * 360.0;
        let pos =
            self.last_pos.lerp(self.pos, t) + Vec2::from_degrees(deg) * (rng.value() * 10.0 * life);
        let scale = rw_sim::FloatExt::mathf_pow(rng.value() * life, 0.1);
        let color = match self.lizard {
            Some((id, module)) => lizards
                .lizard_module(id, module, t)
                .map_or(Color::WHITE, |m| m.head_color),
            None => Color::GRAY.lerp(Color::WHITE, rng.value()),
        };
        Self::sprite_at("Circle20", pos, Vec2::new(scale, scale), 0.0, 0.5, color)
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
mod tests {
    use rw_random::UnityRandom;
    use rw_sim::creature::CreatureId;
    use rw_sim::creature::lizard::{BubbleHead, LizardBubble};
    use rw_sim::{Spark, StationaryEffect, Vec2};

    use super::{
        AirBubbleSprite, BubbleSprite, CosmeticMesh, FlashSprite, LizardColors, LizardModule,
        SparkMesh, SplashSprites,
    };
    use crate::draw::{Color, DrawItem, Layer, Shader};

    /// `Splash.DrawSprites` halfway between ticks: both quads at the splash, anchored
    /// `(0.5, 0)`, `rad / 8` wide and `distance / 16` long, aimed at the linger point,
    /// alpha `0.5 * life` (clamped at 0), colour `(0, 0, i)`, sprite 0 in `Water` and 1 in
    /// `GrabShaders`, both on `WaterSplash`.
    #[test]
    fn splash_sprites() {
        let mut s = rw_sim::Splash::new(Vec2::ZERO, Vec2::new(0.0, 1.0), 8.0, 4.0, 0.0);
        s.last_pos = Vec2::new(10.0, 50.0);
        s.pos = Vec2::new(14.0, 54.0);
        s.last_linger_pos = Vec2::new(12.0, 20.0);
        s.linger_pos = Vec2::new(12.0, 20.0);
        s.last_rad = 4.0;
        s.rad = 6.0;
        s.last_life = 0.8;
        s.life = 0.6;
        let items = s.sprites(0.5);
        assert_eq!(items.len(), 2);
        for (i, item) in items.iter().enumerate() {
            let DrawItem::Sprite(sp) = item else {
                panic!("a sprite");
            };
            assert_eq!(sp.index, i);
            assert_eq!(sp.element, "Futile_White");
            assert_eq!(sp.shader, Shader::WaterSplash);
            assert_eq!(sp.pos, Vec2::new(12.0, 52.0));
            assert_eq!(sp.anchor, Vec2::new(0.5, 0.0));
            assert_eq!(sp.scale, Vec2::new(5.0 / 8.0, 32.0 / 16.0));
            assert_eq!(sp.rotation, 180.0);
            assert!((sp.color.a - 0.35).abs() < 1e-6);
            assert_eq!((sp.color.r, sp.color.g, sp.color.b), (0.0, 0.0, i as f32));
        }
        assert_eq!(items[0].layer(), Layer::Water);
        assert_eq!(items[1].layer(), Layer::GrabShaders);
        s.last_life = -0.1;
        s.life = -0.2;
        let DrawItem::Sprite(sp) = &s.sprites(0.5)[0] else {
            panic!("a sprite");
        };
        assert_eq!(sp.color.a, 0.0);
    }

    /// `Bubble.DrawSprites` halfway between ticks: size 0 to 1 at full size `f`, scale
    /// `0.5 * f * 0.45`, in `(0, 1/255, 0)` in `Midground`.
    #[test]
    fn air_bubble_sprite() {
        let mut b = rw_sim::Bubble::new(
            Vec2::ZERO,
            Vec2::ZERO,
            false,
            false,
            &mut UnityRandom::new(1),
        );
        b.last_pos = Vec2::new(10.0, 20.0);
        b.pos = Vec2::new(14.0, 30.0);
        b.size = 1.0;
        let DrawItem::Sprite(s) = b.sprite(0.5) else {
            panic!("a sprite");
        };
        assert_eq!(s.element, "LizardBubble5");
        assert_eq!(s.pos, Vec2::new(12.0, 25.0));
        let scale = 0.5 * b.full_size * 0.45;
        assert_eq!(s.scale, Vec2::new(scale, scale));
        assert_eq!(s.color, Color::rgb(0.0, 1.0 / 255.0, 0.0));
        assert_eq!(s.layer, Layer::Midground);
    }

    /// One kept module, `module` 7.
    struct OneModule(LizardModule);

    impl LizardColors for OneModule {
        fn lizard_module(&self, _id: CreatureId, module: u64, _t: f32) -> Option<LizardModule> {
            (module == 7).then_some(self.0)
        }

        fn black(&self) -> Color {
            Color::WHITE
        }
    }

    /// A bubble of module 7 stuck at `(10, 0)` in the head's frame, at the origin, its
    /// last read head far off.
    trait Stuck {
        fn stuck(life: f32) -> Self;
    }

    impl Stuck for LizardBubble {
        fn stuck(life: f32) -> Self {
            let mut b = Self::new(
                (CreatureId(0), 7),
                Vec2::ZERO,
                BubbleHead::default(),
                [1.0, 1.0, 0.0],
                &mut UnityRandom::new(1),
            );
            b.head = BubbleHead {
                pos: Vec2::new(100.0, 100.0),
                vel: Vec2::ZERO,
                rotation: 0.0,
            };
            b.pos = Vec2::ZERO;
            b.last_pos = Vec2::ZERO;
            b.origin_point = Vec2::new(10.0, 0.0);
            b.stuck_to_origin = true;
            b.free_floating = false;
            b.hollow = false;
            b.life = life;
            b.last_life = life;
            b
        }
    }

    const MODULE: LizardModule = LizardModule {
        head_color: Color::rgb(1.0, 0.0, 0.0),
        black: Color::rgb(0.0, 0.0, 1.0),
        head: Vec2::new(20.0, 10.0),
        rotation: 90.0,
        white_flicker: 0,
    };

    /// Stuck, it stretches toward `head + RotateAroundOrigo(originPoint, rotation)`
    /// of its module at `t`: (20, 10) + (0, -10) = (20, 0), 20 px off, so the stretch
    /// is 1.25: `scaleY = 1.25 - 0.3125`, `anchorY = InverseLerp(1.25, 0.3125, 1.25)
    /// * 0.5 = 0`, aimed at 90 degrees.
    #[test]
    fn a_stuck_bubble_stretches_toward_its_modules_head() {
        let b = LizardBubble::stuck(1.0);
        let DrawItem::Sprite(sp) = b.sprite(0.5, &OneModule(MODULE)) else {
            panic!("a sprite");
        };
        assert!((sp.rotation - 90.0).abs() < 1e-4);
        assert!((sp.scale.y - 0.9375).abs() < 1e-5);
        assert!(sp.anchor.y.abs() < 1e-6);
        assert_eq!(sp.color, MODULE.head_color);
    }

    /// Its colour runs from the module's head colour to the module's own black by
    /// `1 - Clamp(life * 2)`: halfway at a quarter of its life. Without a module the
    /// camera's black stands in for both.
    #[test]
    fn a_bubble_fades_to_its_modules_black() {
        let b = LizardBubble::stuck(0.25);
        let DrawItem::Sprite(sp) = b.sprite(0.0, &OneModule(MODULE)) else {
            panic!("a sprite");
        };
        assert_eq!(sp.color, Color::rgb(0.5, 0.0, 0.5));
        let mut orphan = LizardBubble::stuck(0.25);
        orphan.module = 8;
        let DrawItem::Sprite(sp) = orphan.sprite(0.0, &OneModule(MODULE)) else {
            panic!("a sprite");
        };
        assert_eq!(sp.color, Color::WHITE);
    }

    /// A head-shield flash takes `HeadColor(t)` of the module it was given (three
    /// draws, no colour draw), white when this view lacks it.
    #[test]
    fn a_flash_shows_its_modules_head_colour() {
        let lizards = OneModule(MODULE);
        let mut rng = UnityRandom::new(5);
        let flash = StationaryEffect::flashing_orb(Vec2::ZERO, Some((CreatureId(0), 7)));
        let DrawItem::Sprite(sp) = flash.sprite(0.5, &lizards, &mut rng) else {
            panic!("a sprite");
        };
        assert_eq!(sp.color, MODULE.head_color);
        let mut expect = UnityRandom::new(5);
        for _ in 0..3 {
            expect.value();
        }
        assert_eq!(rng.state(), expect.state());
        let orphan = StationaryEffect::flashing_orb(Vec2::ZERO, Some((CreatureId(0), 8)));
        let DrawItem::Sprite(sp) = orphan.sprite(0.5, &lizards, &mut rng) else {
            panic!("a sprite");
        };
        assert_eq!(sp.color, Color::WHITE);
    }

    /// A spark takes its module's head colour only while the module's `whiteFlicker`
    /// is above 0.
    #[test]
    fn a_spark_shows_its_modules_head_colour_while_it_flickers() {
        let room = crate::tests::room(&["...."]);
        let mut s = Spark::new(
            Vec2::new(50.0, 50.0),
            Vec2::new(5.0, 0.0),
            [0.0, 1.0, 0.0, 1.0],
            10,
            170,
            &mut UnityRandom::new(1),
        );
        s.lizard = Some((CreatureId(0), 7));
        let flickering = LizardModule {
            white_flicker: 1,
            ..MODULE
        };
        assert_eq!(
            s.lizard_mesh(&room, 0.0, &OneModule(flickering)).color,
            MODULE.head_color
        );
        assert_eq!(
            s.lizard_mesh(&room, 0.0, &OneModule(MODULE)).color,
            Color::rgb(0.0, 1.0, 0.0)
        );
    }

    #[test]
    fn a_new_spark_streaks_at_least_nine_pixels() {
        let mut rng = UnityRandom::new(1);
        let s = Spark::new(
            Vec2::new(50.0, 50.0),
            Vec2::new(5.0, 0.0),
            [1.0; 4],
            6,
            18,
            &mut rng,
        );
        let room = crate::tests::room(&["...."]);
        let m = s.mesh(&room, 0.0);
        assert_eq!(m.vertices.len(), 3);
        assert!((m.vertices[2] - Vec2::new(50.0, 50.0)).length() > 8.99);
    }
}
