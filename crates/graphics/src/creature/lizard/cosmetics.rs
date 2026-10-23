//! The `LizardCosmetics` a green lizard can get (`SpineSpikes`, `BumpHawk`,
//! `LongShoulderScales`, `ShortBodyScales`, `TailTuft`, `LongHeadScales`): their
//! constructors, which draw on the graphics' seeded stream, and the choice among them in
//! `LizardGraphics`' constructor. Their updates and sprites are in
//! `cosmetics_draw.rs`.
//!
//! Mirrors: LizardGraphics.cs (constructor), LizardCosmetics/SpineSpikes.cs,
//!   LizardCosmetics/BumpHawk.cs, LizardCosmetics/LongShoulderScales.cs,
//!   LizardCosmetics/ShortBodyScales.cs, LizardCosmetics/TailTuft.cs,
//!   LizardCosmetics/LongHeadScales.cs

use rw_assets::CreatureType;
use rw_random::UnityRandom;
use rw_sim::{FloatExt, Vec2};

use crate::body::body_part::BodyPart;
use crate::player::elements::LIZARD_SCALE_A_HEIGHT;

/// `Template.SpritesOverlap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpritesOverlap {
    Behind,
    BodySurface,
    BehindHead,
    InFront,
    Hud,
}

impl SpritesOverlap {
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Behind => "Behind",
            Self::BodySurface => "BodySurface",
            Self::BehindHead => "BehindHead",
            Self::InFront => "InFront",
            Self::Hud => "HUD",
        }
    }
}

/// `SpineSpikes` and `BumpHawk`: bumps along the spine.
#[derive(Debug, Clone, PartialEq)]
pub struct SpineBumps {
    pub bumps: i32,
    pub spine_length: f32,
    pub size_skew_exponent: f32,
    pub size_range_min: f32,
    pub size_range_max: f32,
    /// `SpineSpikes.graphic` (unused by `BumpHawk`).
    pub graphic: i32,
    pub scale_x: f32,
    /// `SpineSpikes.colored` (0 to 2), or `BumpHawk.coloredHawk` as 0/1.
    pub colored: i32,
}

/// A `LizardScale`: a body part a long scale's tip follows (made at the origin, never
/// reset).
#[derive(Debug, Clone, PartialEq)]
pub struct LizardScale {
    pub part: BodyPart,
    pub length: f32,
    pub width: f32,
}

impl LizardScale {
    #[must_use]
    pub fn new(length: f32, width: f32) -> Self {
        Self {
            part: BodyPart::new(0.0, 0.0, 0.0),
            length,
            width,
        }
    }
}

/// `BodyScales` and the `LongBodyScales` fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scales {
    pub graphic: i32,
    pub scale_x: f32,
    pub colored: bool,
    /// `scalesPositions`: x across the back (-1 to 1), y along the body and tail.
    pub positions: Vec<Vec2>,
    /// `LongBodyScales.scaleObjects` (empty for `ShortBodyScales`).
    pub scale_objects: Vec<LizardScale>,
    pub backwards_factors: Vec<f32>,
    pub rigor: f32,
    /// `LongBodyScales.graphicHeight`: the `LizardScaleA` element's height.
    pub graphic_height: f32,
}

/// One cosmetic's kind and state.
#[derive(Debug, Clone, PartialEq)]
pub enum CosmeticShape {
    SpineSpikes(SpineBumps),
    BumpHawk(SpineBumps),
    LongShoulderScales(Scales),
    ShortBodyScales(Scales),
    TailTuft(Scales),
    LongHeadScales(Scales),
}

impl CosmeticShape {
    /// The game's class name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SpineSpikes(_) => "SpineSpikes",
            Self::BumpHawk(_) => "BumpHawk",
            Self::LongShoulderScales(_) => "LongShoulderScales",
            Self::ShortBodyScales(_) => "ShortBodyScales",
            Self::TailTuft(_) => "TailTuft",
            Self::LongHeadScales(_) => "LongHeadScales",
        }
    }
}

/// A `LizardCosmetics.Template`.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardCosmetic {
    pub start_sprite: usize,
    pub number_of_sprites: usize,
    pub sprites_overlap: SpritesOverlap,
    pub shape: CosmeticShape,
}

/// What a cosmetic's constructor reads of the graphics.
#[derive(Debug, Clone, Copy)]
pub struct CosmeticCx {
    pub kind: CreatureType,
    pub body_length: f32,
    /// `BodyAndTailLength`.
    pub body_and_tail_length: f32,
    /// `lizardParams.headSize`.
    pub head_size: f32,
    /// `iVars.tailColor`.
    pub tail_color: f32,
}

impl LizardCosmetic {
    /// `LizardGraphics`' cosmetics choice for a green lizard (sprites from `start`;
    /// `(value < 0.8 && green)` draws before the breed check, as the game's does).
    /// Other breeds' choices and cosmetics are later lizard steps.
    pub fn choose_green(cx: CosmeticCx, start: usize, rng: &mut UnityRandom) -> Vec<Self> {
        let mut list: Vec<Self> = Vec::new();
        let mut next = start;
        let mut count = 0;
        let mut long_shoulders = false;
        let mut short_body = false;
        if rng.value() < 1.0 / 15.0
            || rng.value() < 0.8
            || (rng.value() < 0.7 && cx.kind == CreatureType::BlackLizard)
        {
            Self::add(&mut list, &mut next, Self::spine_spikes(cx, rng));
            count += 1;
        } else if rng.value() < 1.0 / 30.0 {
            Self::add(&mut list, &mut next, Self::bump_hawk(cx, rng));
            count += 1;
        } else if rng.value() < 1.0 / 21.0 {
            Self::add(&mut list, &mut next, Self::long_shoulder_scales(cx, rng));
            long_shoulders = true;
            count += 1;
        } else if rng.value() < 0.0625 || rng.value() < 0.5 {
            Self::add(&mut list, &mut next, Self::short_body_scales(cx, rng));
            short_body = true;
            count += 1;
        }
        if rng.value() < 1.0 / 9.0 || (count == 0 && rng.value() < 0.7) {
            let t = Self::tail_tuft(cx, &list, rng);
            Self::add(&mut list, &mut next, t);
        } else if count < 2 && rng.value() < 0.7 {
            let coin = rng.value() < 0.5;
            if coin | long_shoulders | short_body {
                let t = Self::tail_tuft(cx, &list, rng);
                Self::add(&mut list, &mut next, t);
            } else {
                Self::add(&mut list, &mut next, Self::long_shoulder_scales(cx, rng));
                long_shoulders = true;
                count += 1;
            }
        }
        let chance = if count == 0 { 0.7 } else { 0.1 };
        if rng.value() < chance
            && ((!long_shoulders && rng.value() < 0.9) || rng.value() < 1.0 / 30.0)
        {
            Self::add(&mut list, &mut next, Self::long_head_scales(cx, rng));
        }
        list
    }

    /// `AddCosmetic`: the next sprites go to it.
    fn add(list: &mut Vec<Self>, next: &mut usize, mut c: Self) {
        c.start_sprite = *next;
        *next += c.number_of_sprites;
        list.push(c);
    }

    fn new(overlap: SpritesOverlap, sprites: usize, shape: CosmeticShape) -> Self {
        Self {
            start_sprite: 0,
            number_of_sprites: sprites,
            sprites_overlap: overlap,
            shape,
        }
    }

    /// `new SpineSpikes`.
    fn spine_spikes(cx: CosmeticCx, rng: &mut UnityRandom) -> Self {
        let green = cx.kind == CreatureType::GreenLizard;
        let spacing = 5.0f32.lerp(8.0, rng.value().mathf_pow(0.7));
        let spine_length = 0.2f32.lerp(0.95, rng.value()) * cx.body_and_tail_length;
        let mut min = 0.1f32.lerp(0.5, rng.value().mathf_pow(2.0));
        let mut max = min.lerp(1.1, rng.value());
        if rng.value() < 0.5 {
            max = 1.0;
        }
        if cx.kind == CreatureType::BlueLizard {
            min = min.min(0.3);
            max = max.min(0.6);
        } else if !green && rng.value() < 0.7 {
            min *= 0.7;
            max *= 0.7;
        } else if green && rng.value() < 0.7 {
            min = min.lerp(1.1, 0.1);
            max = max.lerp(1.1, 0.4);
        }
        let size_skew_exponent = 0.1f32.lerp(0.9, rng.value());
        let bumps = (spine_length / spacing) as i32;
        let mut scale_x = 1.0;
        let mut graphic = rng.range_i32(0, 5);
        if graphic == 1 {
            graphic = 0;
        }
        if graphic == 4 {
            graphic = 3;
        } else if (graphic == 3 && rng.value() < 0.5) || rng.value() < 1.0 / 15.0 {
            scale_x = -1.0;
        }
        if cx.kind == CreatureType::PinkLizard && rng.value() < 0.7 {
            graphic = 0;
        } else if green && rng.value() < 0.5 {
            graphic = 3;
        }
        let mut colored = rng.range_i32(0, 3);
        if cx.kind == CreatureType::PinkLizard && rng.value() < 0.5 {
            colored = 0;
        } else if green && rng.value() < 0.5 {
            colored = 2;
        } else if green && rng.value() < 0.5 {
            colored = 1;
        }
        let n = usize::try_from(bumps).unwrap_or(0);
        let sprites = if colored > 0 { n * 2 } else { n };
        Self::new(
            SpritesOverlap::BehindHead,
            sprites,
            CosmeticShape::SpineSpikes(SpineBumps {
                bumps,
                spine_length,
                size_skew_exponent,
                size_range_min: min,
                size_range_max: max,
                graphic,
                scale_x,
                colored,
            }),
        )
    }

    /// `new BumpHawk`.
    fn bump_hawk(cx: CosmeticCx, rng: &mut UnityRandom) -> Self {
        let colored = rng.value() < 0.5;
        let (spacing, spine_length, min, max);
        if colored {
            spacing = 3.0f32.lerp(8.0, rng.value().mathf_pow(0.7));
            spine_length = 0.3f32.lerp(0.7, rng.value()) * cx.body_and_tail_length;
            min = 0.1f32.lerp(0.2, rng.value());
            max = min.lerp(0.35, rng.value().mathf_pow(0.5));
        } else {
            spacing = 6.0f32.lerp(12.0, rng.value().mathf_pow(0.5));
            spine_length = 0.3f32.lerp(0.9, rng.value()) * cx.body_and_tail_length;
            min = 0.2f32.lerp(0.3, rng.value().mathf_pow(0.5));
            max = min.lerp(0.5, rng.value());
        }
        let size_skew_exponent = 0.1f32.lerp(0.7, rng.value());
        let bumps = (spine_length / spacing) as i32;
        Self::new(
            SpritesOverlap::BehindHead,
            usize::try_from(bumps).unwrap_or(0),
            CosmeticShape::BumpHawk(SpineBumps {
                bumps,
                spine_length,
                size_skew_exponent,
                size_range_min: min,
                size_range_max: max,
                graphic: 0,
                scale_x: 0.0,
                colored: i32::from(colored),
            }),
        )
    }

    /// `new LongShoulderScales` (the red lizard's stretch is a later step).
    fn long_shoulder_scales(cx: CosmeticCx, rng: &mut UnityRandom) -> Self {
        let mut s = Scales::default();
        let pattern = if cx.kind != CreatureType::PinkLizard || rng.value() < 1.0 / 3.0 {
            rng.range_i32(0, 3)
        } else if cx.kind == CreatureType::GreenLizard || rng.value() < 0.5 {
            2
        } else {
            0
        };
        match pattern {
            0 => {
                let n = rng.range_i32(4, 15);
                s.generate_patch_pattern(cx, 0.05, n, 0.9, 2.0, rng);
            }
            1 => s.generate_two_lines(cx, 0.07, 1.0, 1.5, 3.0, rng),
            _ => s.generate_segments(cx, 0.1, 0.8, 5.0, rng),
        }
        s.move_scales_towards_head();
        let len = s.positions.len() as f32;
        let width_fac = 1.0f32.lerp(1.0 / 1.0f32.lerp(len, rng.value().mathf_pow(2.0)), 0.5);
        let a = 5.0f32.lerp(15.0, rng.value()) * width_fac;
        let b = a.lerp(35.0, rng.value().mathf_pow(0.5)) * width_fac;
        s.colored = matches!(cx.kind, CreatureType::GreenLizard | CreatureType::RedLizard)
            || rng.value() < 0.4;
        s.graphic = if rng.value() < 0.1 {
            rng.range_i32(0, 7)
        } else {
            rng.range_i32(3, 6)
        };
        if cx.kind == CreatureType::PinkLizard && rng.value() < 0.25 {
            s.graphic = 0;
        }
        s.set_graphic_height();
        let (lo, hi) = s.y_range();
        let p = 0.1f32.lerp(0.9, rng.value());
        for pos in &s.positions {
            let t = pos.y.inverse_lerp(lo, hi).mathf_pow(p);
            let bump = (t * std::f32::consts::PI)
                .sin()
                .lerp(1.0, if t < 0.5 { 0.5 } else { 0.0 });
            s.scale_objects.push(LizardScale::new(
                a.lerp(b, bump),
                0.8f32.lerp(1.2, bump) * width_fac,
            ));
            s.backwards_factors.push(pos.y * 0.7);
        }
        let n = s.positions.len();
        let sprites = if s.colored { n * 2 } else { n };
        Self::new(
            SpritesOverlap::BehindHead,
            sprites,
            CosmeticShape::LongShoulderScales(s),
        )
    }

    /// `new ShortBodyScales`.
    fn short_body_scales(cx: CosmeticCx, rng: &mut UnityRandom) -> Self {
        let mut s = Scales::default();
        let mut pattern = rng.range_i32(0, 3);
        if cx.kind == CreatureType::GreenLizard && rng.value() < 0.7 {
            pattern = 2;
        } else if cx.kind == CreatureType::BlueLizard && rng.value() < 0.93 {
            pattern = 1;
        }
        match pattern {
            0 => {
                let n = rng.range_i32(4, 15);
                s.generate_patch_pattern(cx, 0.1, n, 0.9, 1.2, rng);
            }
            1 => s.generate_two_lines(cx, 0.1, 1.0, 1.5, 1.0, rng),
            _ => {
                let exp = if cx.kind == CreatureType::PinkLizard {
                    1.5
                } else {
                    0.6
                };
                s.generate_segments(cx, 0.1, 0.9, exp, rng);
            }
        }
        let n = s.positions.len();
        Self::new(
            SpritesOverlap::BehindHead,
            n,
            CosmeticShape::ShortBodyScales(s),
        )
    }

    /// `new TailTuft`; `before` are the cosmetics added so far (it may take the
    /// graphic of the first long scales or spikes among them).
    fn tail_tuft(cx: CosmeticCx, before: &[Self], rng: &mut UnityRandom) -> Self {
        let mut s = Scales::default();
        let blue = cx.kind == CreatureType::BlueLizard;
        let red = cx.kind == CreatureType::RedLizard;
        if rng.value() < 1.0 / 7.0 || (rng.value() < 0.9 && blue) || red {
            if blue || red {
                s.generate_two_lines(cx, 0.0, if red { 0.3 } else { 0.7 }, 1.0, 3.0, rng);
            } else {
                s.generate_two_lines(cx, 0.0, 0.4, 1.2, 1.3, rng);
            }
        } else {
            let n = rng.range_i32(3, 7);
            s.generate_patch_pattern(cx, 0.0, n, 1.6, 1.5, rng);
        }
        s.move_scales_towards_tail();
        let len = s.positions.len() as f32;
        let mut width_fac = 1.0f32.lerp(1.0 / 1.0f32.lerp(len, rng.value().mathf_pow(2.0)), 0.5);
        if red {
            width_fac = width_fac.max(0.4) * 1.1;
        }
        let a = 5.0f32.lerp(10.0, rng.value()) * width_fac;
        let b = a.lerp(25.0, rng.value().mathf_pow(0.5)) * width_fac;
        s.colored = rng.value() < 0.8;
        s.graphic = rng.range_i32(3, 7);
        if s.graphic == 3 {
            s.graphic = 1;
        }
        if rng.value() < 1.0 / 30.0 {
            s.graphic = rng.range_i32(0, 7);
        }
        if rng.value() < 0.8 || red {
            for c in before {
                match &c.shape {
                    CosmeticShape::LongShoulderScales(o)
                    | CosmeticShape::TailTuft(o)
                    | CosmeticShape::LongHeadScales(o) => {
                        s.graphic = o.graphic;
                        break;
                    }
                    CosmeticShape::SpineSpikes(o) => {
                        s.graphic = o.graphic;
                        break;
                    }
                    CosmeticShape::BumpHawk(_) | CosmeticShape::ShortBodyScales(_) => {}
                }
            }
        }
        s.set_graphic_height();
        let (lo, hi) = s.y_range();
        let spread = 1.0f32.lerp(1.5, rng.value());
        for pos in &mut s.positions {
            let t = pos.y.inverse_lerp(lo, hi);
            s.scale_objects.push(LizardScale::new(
                a.lerp(b, t),
                0.8f32.lerp(1.2, t) * width_fac,
            ));
            s.backwards_factors
                .push(0.3 + 0.7 * pos.y.inverse_lerp(0.75, 1.0));
            pos.x *= pos.y.inverse_lerp(1.05, 0.85) * spread;
        }
        let n = s.positions.len();
        let sprites = if s.colored { n * 2 } else { n };
        Self::new(
            SpritesOverlap::BehindHead,
            sprites,
            CosmeticShape::TailTuft(s),
        )
    }

    /// `new LongHeadScales`: two horns.
    fn long_head_scales(cx: CosmeticCx, rng: &mut UnityRandom) -> Self {
        let mut s = Scales {
            rigor: rng.value(),
            ..Scales::default()
        };
        s.generate_two_horns(rng);
        let size = rng.value().mathf_pow(0.7) * cx.head_size;
        let white = cx.kind == CreatureType::WhiteLizard;
        s.colored = rng.value() < 0.5 && !white;
        s.graphic = rng.range_i32(4, 6);
        if size < 0.5 && rng.value() < 0.5 {
            s.graphic = 6;
        } else if size > 0.8 {
            s.graphic = 5;
        }
        if size < 0.2 && !white {
            s.colored = true;
        }
        if cx.kind == CreatureType::BlackLizard {
            s.colored = false;
        }
        s.set_graphic_height();
        let width = rng.value();
        let backwards = rng.value().mathf_pow(0.85);
        for _ in 0..s.positions.len() {
            s.scale_objects.push(LizardScale::new(
                5.0f32.lerp(35.0, size),
                0.65f32.lerp(1.2, width * size),
            ));
            s.backwards_factors.push(backwards);
        }
        let n = s.positions.len();
        let sprites = if s.colored { n * 2 } else { n };
        Self::new(
            SpritesOverlap::InFront,
            sprites,
            CosmeticShape::LongHeadScales(s),
        )
    }
}

impl Scales {
    /// `graphicHeight = sourcePixelSize.y` of `"LizardScaleA" + graphic`.
    fn set_graphic_height(&mut self) {
        let g = usize::try_from(self.graphic).unwrap_or(0).min(6);
        self.graphic_height = LIZARD_SCALE_A_HEIGHT[g];
    }

    /// `GeneratePatchPattern`.
    fn generate_patch_pattern(
        &mut self,
        cx: CosmeticCx,
        start: f32,
        count: i32,
        max_length: f32,
        exponent: f32,
        rng: &mut UnityRandom,
    ) {
        let end = (start + 0.1).lerp(
            (start + 0.2).max(max_length),
            rng.value().mathf_pow(exponent),
        );
        let body = cx.body_length / cx.body_and_tail_length;
        self.positions = (0..count)
            .map(|_| {
                let deg = rng.value() * 360.0;
                let v = Vec2::from_degrees(deg) * rng.value();
                Vec2::new(v.x, (start * body).lerp(end * body, (v.y + 1.0) / 2.0))
            })
            .collect();
    }

    /// `GenerateTwoLines`.
    fn generate_two_lines(
        &mut self,
        cx: CosmeticCx,
        start: f32,
        max_length: f32,
        exponent: f32,
        spacing_scale: f32,
        rng: &mut UnityRandom,
    ) {
        let end = (start + 0.1).lerp(
            (start + 0.2).max(max_length),
            rng.value().mathf_pow(exponent),
        );
        let length = end * cx.body_and_tail_length;
        let mut spacing = 2.0f32.lerp(9.0, rng.value());
        if cx.kind == CreatureType::BlueLizard {
            spacing = 2.0;
        }
        spacing *= spacing_scale;
        let n = ((length / spacing) as i32).max(3);
        self.positions.clear();
        for i in 0..n {
            let f = i as f32 / (n - 1) as f32;
            let y = 0.0f32.lerp(end, f);
            let x = 0.6 + 0.4 * (f * std::f32::consts::PI).sin();
            self.positions.push(Vec2::new(x, y));
            self.positions.push(Vec2::new(-x, y));
        }
    }

    /// `GenerateSegments`.
    fn generate_segments(
        &mut self,
        cx: CosmeticCx,
        start: f32,
        max_length: f32,
        exponent: f32,
        rng: &mut UnityRandom,
    ) {
        let end = (start + 0.1).lerp(
            (start + 0.2).max(max_length),
            rng.value().mathf_pow(exponent),
        );
        let length = end * cx.body_and_tail_length;
        let mut spacing = 7.0f32.lerp(14.0, rng.value());
        if cx.kind == CreatureType::RedLizard {
            spacing = spacing.min(11.0) * 0.75;
        }
        let rows = ((length / spacing) as i32).max(3);
        let across = rng.range_i32(1, 4) * 2;
        self.positions.clear();
        for i in 0..rows {
            let f = i as f32 / (rows - 1) as f32;
            let y = 0.0f32.lerp(end, f);
            for j in 0..across {
                let x = (0.6 + 0.6 * (f * std::f32::consts::PI).sin())
                    * (-1.0f32).lerp(1.0, j as f32 / (across - 1) as f32);
                self.positions.push(Vec2::new(x, y));
            }
        }
    }

    /// `LongHeadScales.GenerateTwoHorns`.
    fn generate_two_horns(&mut self, rng: &mut UnityRandom) {
        let y = 0.0f32.lerp(0.07, rng.value());
        let x = 0.5f32.lerp(1.5, rng.value());
        self.positions = vec![Vec2::new(-x, y), Vec2::new(x, y)];
    }

    /// `LongShoulderScales.MoveScalesTowardsHead`.
    fn move_scales_towards_head(&mut self) {
        let mut low = 1.0f32;
        for p in &self.positions {
            if p.y < low {
                low = p.y;
            }
        }
        if low > 0.07 {
            low -= 0.07;
            for p in &mut self.positions {
                p.y -= low;
            }
        }
    }

    /// `TailTuft.MoveScalesTowardsTail`.
    fn move_scales_towards_tail(&mut self) {
        let mut high = 0.0f32;
        for p in &self.positions {
            if p.y > high {
                high = p.y;
            }
        }
        for p in &mut self.positions {
            p.y += 0.9 - high;
        }
    }

    /// The lowest and highest `y` (from 1 and 0, as the constructors search).
    fn y_range(&self) -> (f32, f32) {
        let (mut lo, mut hi) = (1.0f32, 0.0f32);
        for p in &self.positions {
            if p.y > hi {
                hi = p.y;
            }
            if p.y < lo {
                lo = p.y;
            }
        }
        (lo, hi)
    }
}
