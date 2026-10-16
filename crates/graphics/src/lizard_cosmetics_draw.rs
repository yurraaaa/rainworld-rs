//! The green lizard's cosmetics' `Update` (`LongBodyScales`' scales trailing their
//! roots; the others have none), `DrawSprites` and `ApplyPalette` colours, and where
//! `AddToContainer` puts them among the body's sprites (decompile
//! `LizardCosmetics/*.cs`). Their only draws are the scales' shake while
//! `showDominance` is above 0, which no base-game lizard shows.

use rw_random::UnityRandom;
use rw_sim::creature::lizard::{LizardGraphicsTick, LizardView};
use rw_sim::{FloatExt, Vec2};

use crate::body_part::PointConnection;
use crate::draw::{Color, DrawItem, Layer, Shader, Sprite};
use crate::elements::{CIRCLE_20, LIZARD_SCALE_A, LIZARD_SCALE_B, PIXEL};
use crate::lizard_cosmetics::{CosmeticShape, LizardCosmetic, LizardScale, Scales, SpineBumps};
use crate::lizard_draw::{LizardReading, LizardSpineData};
use crate::lizard_graphics::LizardGraphics;
use crate::math::CustomVec;

/// What a cosmetic reads when drawn: the module, the body, the palette's black and the
/// frame's time between ticks.
pub struct CosmeticDraw<'a, 'v> {
    pub g: &'a LizardGraphics,
    pub view: &'a LizardView<'v>,
    pub black: Color,
    pub t: f32,
}

impl CosmeticDraw<'_, '_> {
    /// `BodyColor(f)`.
    fn body_color(&self, f: f32) -> Color {
        self.g.body_color(self.view, self.black, f)
    }

    /// `HeadColor(t)`.
    fn head_color(&self) -> Color {
        self.g
            .head_color(&LizardReading::of(self.view), self.black, self.t)
    }

    fn effect_color(&self) -> Color {
        Color::from_rgb(self.g.effect_color)
    }

    /// `BodyAndTailLength`.
    fn body_and_tail(&self) -> f32 {
        self.g.body_length + self.g.tail_length
    }
}

impl LizardGraphics {
    /// The cosmetics' `Update` (while not culled).
    pub(crate) fn update_cosmetics(
        &mut self,
        view: &LizardView<'_>,
        cx: &mut LizardGraphicsTick<'_>,
    ) {
        let mut cosmetics = std::mem::take(&mut self.cosmetics);
        for c in &mut cosmetics {
            if let CosmeticShape::LongShoulderScales(s)
            | CosmeticShape::TailTuft(s)
            | CosmeticShape::LongHeadScales(s) = &mut c.shape
            {
                s.update_long(self, view, cx);
            }
        }
        self.cosmetics = cosmetics;
    }

    /// `BodyScales.GetBackPos(i, t, changeDepthRotation)`: the spine at scale `i`'s
    /// place, its outer point moved across the back by the scale's `x`.
    #[must_use]
    pub fn back_pos(
        &self,
        view: &LizardView<'_>,
        scale: Vec2,
        t: f32,
        change_depth_rotation: bool,
    ) -> LizardSpineData {
        let mut d = self.spine_position(view, scale.y, t);
        let across = (scale.x + d.depth_rotation).clamp(-1.0, 1.0);
        d.outer_pos = d.pos + d.perp * (across * d.rad);
        if change_depth_rotation {
            d.depth_rotation = across;
        }
        d
    }
}

impl Scales {
    /// `LongBodyScales.Update`: each scale's tip pulled toward where it points from its
    /// root (back along the spine, out to the side as the body turns, the front ones
    /// away from the head), kept within its length.
    fn update_long(
        &mut self,
        g: &LizardGraphics,
        view: &LizardView<'_>,
        cx: &mut LizardGraphicsTick<'_>,
    ) {
        let head_dir = Vec2::from_degrees(g.head_rotation(&g.lizard, 1.0));
        let dominance = g.show_dominance;
        for i in 0..self.scale_objects.len() {
            let at = self.positions[i];
            let back = g.back_pos(view, at, 1.0, true);
            let mut a = back.dir.lerp(
                back.pos.direction_to(back.outer_pos),
                back.depth_rotation.abs(),
            );
            if at.y < 0.2 {
                a -= head_dir * (at.y.inverse_lerp(0.2, 0.0).mathf_pow(2.0) * 2.0);
            }
            let bend = self.backwards_factors[i].mathf_pow(1.0f32.lerp(15.0, dominance));
            a = a.lerp(back.dir, bend).normalized();
            let s = &mut self.scale_objects[i];
            let target = back.outer_pos + a * s.length;
            s.pull_toward(target, dominance, self.rigor, cx.rng);
            s.part.connect_to_point(PointConnection {
                point: back.outer_pos,
                rad: s.length,
                push: true,
                elastic: 0.0,
                host_vel: Vec2::ZERO,
                adapt_vel: 0.0,
                exaggerate_vel: 0.0,
            });
            let submerged = cx.point_submerged(s.part.pos);
            s.update(submerged);
        }
    }
}

impl LizardScale {
    /// The tip kept within half its length of `target`, then sped toward it (shaken
    /// while showing dominance: a draw).
    fn pull_toward(&mut self, target: Vec2, dominance: f32, rigor: f32, rng: &mut UnityRandom) {
        let p = &mut self.part;
        let half = self.length / 2.0;
        if !p.pos.dist_less(target, half) {
            let dir = p.pos.direction_to(target);
            let over = p.pos.distance(target) - half;
            p.pos += dir * over;
            p.vel += dir * over;
        }
        p.vel += (target - p.pos).clamp_magnitude(10.0f32.lerp(20.0, dominance))
            / 5.0f32.lerp(1.5, rigor);
        p.vel *= 1.0f32.lerp(0.8, rigor);
        if dominance > 0.0 {
            let deg = rng.value() * 360.0;
            p.vel += Vec2::from_degrees(deg) * 0.0f32.lerp(6.0, dominance);
        }
    }

    /// `LizardScale.Update`: water halves its speed (else a tenth off), then it moves.
    fn update(&mut self, submerged: bool) {
        let p = &mut self.part;
        p.vel *= if submerged { 0.5 } else { 0.9 };
        p.last_pos = p.pos;
        p.pos += p.vel;
    }
}

impl LizardCosmetic {
    /// `DrawSprites` with the colours `ApplyPalette` gives, in sprite order.
    #[must_use]
    pub fn draw(&self, cx: &CosmeticDraw<'_, '_>) -> Vec<DrawItem> {
        let mut items = match &self.shape {
            CosmeticShape::SpineSpikes(b) => self.draw_spine_spikes(b, cx),
            CosmeticShape::BumpHawk(b) => self.draw_bump_hawk(b, cx),
            CosmeticShape::ShortBodyScales(s) => self.draw_short_scales(s, cx),
            CosmeticShape::LongShoulderScales(s) | CosmeticShape::TailTuft(s) => {
                self.draw_long_scales(s, cx, false)
            }
            CosmeticShape::LongHeadScales(s) => self.draw_long_scales(s, cx, true),
        };
        items.sort_by_key(DrawItem::index);
        items
    }

    /// `SpineSpikes`: a spike per bump along the spine, sized by a sine over its
    /// length, flattened as the body faces the camera; the coloured overlay
    /// (`colored` 1: effect colour; 2: fading into the body colour toward the tail).
    fn draw_spine_spikes(&self, b: &SpineBumps, cx: &CosmeticDraw<'_, '_>) -> Vec<DrawItem> {
        let n = usize::try_from(b.bumps).unwrap_or(0);
        let start = self.start_sprite;
        let graphic = usize::try_from(b.graphic).unwrap_or(0).min(6);
        let depth = cx.g.depth_rotation;
        let sign = crate::math::Mathf::unity_sign(depth);
        let mut items = Vec::with_capacity(n * 2);
        for k in 0..n {
            let along = Self::along(k, n);
            let f = 0.05f32.lerp(b.spine_length / cx.body_and_tail(), along);
            let d = cx.g.spine_position(cx.view, f, cx.t);
            let side = d.perp * d.depth_rotation;
            let rotation = (-side).aim_to(side);
            let size = b.size_range_min.lerp(
                b.size_range_max,
                (along.mathf_pow(b.size_skew_exponent) * std::f32::consts::PI).sin(),
            );
            let scale = Vec2::new(
                sign * b.scale_x * size,
                size * 0.2f32.max(depth.abs().inverse_lerp(0.0, 0.5)),
            );
            let anchor = Vec2::new(0.5, 0.15);
            items.push(Self::sprite(
                start + k,
                LIZARD_SCALE_A[graphic],
                d.outer_pos,
                rotation,
                scale,
                anchor,
                cx.body_color(f),
            ));
            if b.colored > 0 {
                let color = if b.colored == 1 {
                    cx.effect_color()
                } else {
                    cx.effect_color()
                        .lerp(cx.body_color(f), along.mathf_pow(0.5))
                };
                items.push(Self::sprite(
                    start + n + k,
                    LIZARD_SCALE_B[graphic],
                    d.outer_pos,
                    rotation,
                    scale,
                    anchor,
                    color,
                ));
            }
        }
        items
    }

    /// `BumpHawk`: round bumps along the spine; the coloured hawk fades from the head
    /// colour into the body colour.
    fn draw_bump_hawk(&self, b: &SpineBumps, cx: &CosmeticDraw<'_, '_>) -> Vec<DrawItem> {
        let n = self.number_of_sprites;
        (0..n)
            .map(|k| {
                let along = Self::along(k, n);
                let f = 0.05f32.lerp(b.spine_length / cx.body_and_tail(), along);
                let d = cx.g.spine_position(cx.view, f, cx.t);
                let bump = (along.mathf_pow(b.size_skew_exponent) * std::f32::consts::PI)
                    .sin()
                    .lerp(1.0, if along < 0.5 { 0.5 } else { 0.0 });
                let size = b.size_range_min.lerp(b.size_range_max, bump);
                let color = if b.colored > 0 {
                    cx.head_color().lerp(cx.body_color(f), along)
                } else {
                    cx.body_color(f)
                };
                Self::sprite(
                    self.start_sprite + k,
                    CIRCLE_20,
                    d.outer_pos,
                    0.0,
                    Vec2::new(size, size),
                    Vec2::new(0.5, 0.5),
                    color,
                )
            })
            .collect()
    }

    /// `ShortBodyScales`: 2 x 3 pixel flecks on the back in the head colour.
    fn draw_short_scales(&self, s: &Scales, cx: &CosmeticDraw<'_, '_>) -> Vec<DrawItem> {
        let color = cx.head_color();
        s.positions
            .iter()
            .enumerate()
            .map(|(k, &at)| {
                let d = cx.g.back_pos(cx.view, at, cx.t, true);
                Self::sprite(
                    self.start_sprite + k,
                    PIXEL,
                    d.outer_pos,
                    d.dir.aim_to(-d.dir),
                    Vec2::new(2.0, 3.0),
                    Vec2::new(0.5, 0.5),
                    color,
                )
            })
            .collect()
    }

    /// `LongBodyScales` (`LongShoulderScales`, `TailTuft`): a scale from each root on
    /// the back toward its tip, flipped by the side it is on, in the body colour with an
    /// effect-coloured overlay; `LongHeadScales` takes the head colour.
    fn draw_long_scales(&self, s: &Scales, cx: &CosmeticDraw<'_, '_>, head: bool) -> Vec<DrawItem> {
        let n = s.positions.len();
        let graphic = usize::try_from(s.graphic).unwrap_or(0).min(6);
        let mut items = Vec::with_capacity(n * 2);
        for (k, scale) in s.scale_objects.iter().enumerate() {
            let at = s.positions[k];
            let d = cx.g.back_pos(cx.view, at, cx.t, true);
            let tip = scale.part.last_pos.lerp(scale.part.pos, cx.t);
            let rotation = d.outer_pos.aim_to(tip);
            let size = Vec2::new(
                scale.width * crate::math::Mathf::unity_sign(d.depth_rotation),
                scale.length / s.graphic_height,
            );
            let anchor = Vec2::new(0.5, 0.1);
            let color = if head {
                cx.head_color()
            } else {
                cx.body_color(at.y)
            };
            items.push(Self::sprite(
                self.start_sprite + k,
                LIZARD_SCALE_A[graphic],
                d.outer_pos,
                rotation,
                size,
                anchor,
                color,
            ));
            if s.colored {
                items.push(Self::sprite(
                    self.start_sprite + n + k,
                    LIZARD_SCALE_B[graphic],
                    d.outer_pos,
                    rotation,
                    size,
                    anchor,
                    cx.effect_color(),
                ));
            }
        }
        items
    }

    /// `Mathf.InverseLerp(startSprite, startSprite + n - 1, sprite)` for the `k`th.
    fn along(k: usize, n: usize) -> f32 {
        if n < 2 {
            0.0
        } else {
            k as f32 / (n - 1) as f32
        }
    }

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

#[cfg(test)]
mod tests {
    use rw_assets::CreatureType;
    use rw_random::UnityRandom;
    use rw_sim::Vec2;

    use crate::lizard_cosmetics::{
        CosmeticCx, CosmeticShape, LizardCosmetic, LizardScale, SpritesOverlap,
    };

    /// Each cosmetic's sprites as its constructor counts them (`SpineSpikes`: a spike
    /// per bump, doubled when coloured; `BumpHawk`: a circle per bump; the long scales:
    /// one per scale, doubled when coloured; `ShortBodyScales`: one per scale), chained
    /// from the body's 20 (`AddCosmetic`); all behind the head but the head scales.
    #[test]
    fn the_green_cosmetics_chain_their_sprites() {
        let cx = CosmeticCx {
            kind: CreatureType::GreenLizard,
            body_length: 60.0,
            body_and_tail_length: 140.0,
            head_size: 1.0,
            tail_color: 0.0,
        };
        let mut kinds = Vec::new();
        for seed in 0..300 {
            let mut rng = UnityRandom::new(seed);
            let list = LizardCosmetic::choose_green(cx, 20, &mut rng);
            let mut next = 20;
            for c in &list {
                assert_eq!(c.start_sprite, next);
                next += c.number_of_sprites;
                let (count, doubled, overlap) = match &c.shape {
                    CosmeticShape::SpineSpikes(b) => {
                        (b.bumps as usize, b.colored > 0, SpritesOverlap::BehindHead)
                    }
                    CosmeticShape::BumpHawk(b) => {
                        (b.bumps as usize, false, SpritesOverlap::BehindHead)
                    }
                    CosmeticShape::ShortBodyScales(s) => {
                        (s.positions.len(), false, SpritesOverlap::BehindHead)
                    }
                    CosmeticShape::LongShoulderScales(s) | CosmeticShape::TailTuft(s) => {
                        (s.positions.len(), s.colored, SpritesOverlap::BehindHead)
                    }
                    CosmeticShape::LongHeadScales(s) => {
                        (s.positions.len(), s.colored, SpritesOverlap::InFront)
                    }
                };
                assert_eq!(c.number_of_sprites, if doubled { count * 2 } else { count });
                assert_eq!(c.sprites_overlap, overlap);
                kinds.push(c.shape.name());
            }
        }
        for k in [
            "SpineSpikes",
            "ShortBodyScales",
            "TailTuft",
            "LongHeadScales",
        ] {
            assert!(kinds.contains(&k), "never chosen: {k}");
        }
    }

    /// A scale's pull draws only while the lizard shows dominance (one angle).
    #[test]
    fn a_scale_draws_only_showing_dominance() {
        let mut rng = UnityRandom::new(3);
        let expected = rng.clone();
        let mut s = LizardScale::new(20.0, 1.0);
        s.pull_toward(Vec2::new(100.0, 0.0), 0.0, 0.5, &mut rng);
        assert_eq!(rng, expected);
        let mut e = rng.clone();
        s.pull_toward(Vec2::new(100.0, 0.0), 0.5, 0.5, &mut rng);
        e.value();
        assert_eq!(rng, e);
    }

    /// From the origin, a tip 100 px off is first moved to half its length (10 px) of
    /// the target, then sped toward it by `ClampMagnitude(10 px, 10) / Lerp(5, 1.5, 0)`
    /// and slowed by `Lerp(1, 0.8, 0)`: position (90, 0), velocity (92, 0).
    #[test]
    fn a_scale_is_pulled_within_half_its_length() {
        let mut rng = UnityRandom::new(3);
        let mut s = LizardScale::new(20.0, 1.0);
        s.pull_toward(Vec2::new(100.0, 0.0), 0.0, 0.0, &mut rng);
        assert!((s.part.pos - Vec2::new(90.0, 0.0)).length() < 1e-4);
        assert!((s.part.vel - Vec2::new(92.0, 0.0)).length() < 1e-4);
    }
}
