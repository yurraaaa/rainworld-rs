//! `SlugcatPage`'s mark of communication and glow (`HasMark`, `HasGlow`; 553799,
//! 553994, 554040, 554085): only continue pages in depth mode have them.
//!
//! Mirrors: Menu/SlugcatSelectMenu.cs (SlugcatPage)

use rw_graphics::{Color, PlayerGraphics};
use rw_menu::{AtlasManager, NodeId, Slugcat, Sprite, StageColor, StageTree};
use rw_random::UnityRandom;
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

/// The mark and glow state of a page, with `AddImage`'s placement.
#[derive(Debug, Clone)]
pub struct PageLights {
    pub slugcat: Slugcat,
    /// `effectColor`.
    pub effect_color: StageColor,
    /// `slugcatDepth`: the parallax depth the mark and glow move at.
    pub slugcat_depth: f32,
    pub mark_offset: Vec2,
    pub glow_offset: Vec2,
    pub has_mark: bool,
    pub has_glow: bool,
    /// `markSquare` and `markGlow`.
    pub mark: Option<[NodeId; 2]>,
    /// `glowSpriteB` and `glowSpriteA`.
    pub glow: Option<[NodeId; 2]>,
    pub mark_alpha: f32,
    pub last_mark_alpha: f32,
    pub mark_flicker: f32,
    pub glow_alpha: f32,
    pub last_glow_alpha: f32,
    pub glow_sin_counter: f32,
}

/// What a page's `GrafUpdate` hands the mark and glow for one frame.
#[derive(Debug, Clone, Copy)]
pub struct LightsFrame {
    pub t: f32,
    /// `MidXpos + Scroll(t) * ScrollMagnitude`.
    pub x: f32,
    pub image_y: f32,
    /// `UseAlpha(t)`.
    pub alpha: f32,
    /// `slugcatImage.CamPos(t)`.
    pub cam_pos: Vec2,
    /// `Lerp(lastInposition, inPosition, t)`.
    pub in_position: f32,
    /// The Hunter's mark fades in over cycles 4 to 14 (`InverseLerp(4, 14, cycle)^3.5`);
    /// 1 for the others.
    pub mark_fade: f32,
}

impl PageLights {
    /// `AddImage`'s colour, depth and offsets for `slugcat` (the constructor's
    /// `effectColor`, a fifth redder for the Hunter).
    #[must_use]
    pub fn new(slugcat: Slugcat, has_mark: bool, has_glow: bool) -> Self {
        let mut color = PlayerGraphics::default_slugcat_color(slugcat);
        let (slugcat_depth, mark_offset, glow_offset) = match slugcat {
            Slugcat::White => (3.1, Vec2::new(-15.0, -2.0), Vec2::new(-30.0, -50.0)),
            Slugcat::Yellow => (3.0, Vec2::new(24.0, -19.0), Vec2::new(0.0, -50.0)),
            Slugcat::Red => {
                color = color.lerp(Color::rgb(1.0, 0.0, 0.0), 0.2);
                (2.7, Vec2::new(-3.0, -73.0), Vec2::new(-20.0, -90.0))
            }
            _ => (1.0, Vec2::ZERO, Vec2::ZERO),
        };
        Self {
            slugcat,
            effect_color: StageColor::rgb(color.r, color.g, color.b),
            slugcat_depth,
            mark_offset,
            glow_offset,
            has_mark,
            has_glow,
            mark: None,
            glow: None,
            mark_alpha: 0.0,
            last_mark_alpha: 0.0,
            mark_flicker: 0.0,
            glow_alpha: 0.0,
            last_glow_alpha: 0.0,
            glow_sin_counter: 0.0,
        }
    }

    /// `AddImage`'s mark sprites (with `HasMark`), after the scene: a 14 px square in a
    /// light `FlatLight` glow.
    pub fn add_mark(&mut self, stage: &mut StageTree, container: NodeId) {
        if !self.has_mark {
            return;
        }
        let mut square = Sprite::new("pixel");
        square.transform.scale_x = 14.0;
        square.transform.scale_y = 14.0;
        square.color = self.effect_color.lerp(StageColor::WHITE, 0.7);
        let square = stage.add_sprite(container, square);
        let mut glow = Sprite::new(AtlasManager::WHITE);
        glow.shader = StageShader::FlatLight;
        glow.color = self.effect_color;
        let glow = stage.add_sprite(container, glow);
        self.mark = Some([square, glow]);
    }

    /// `SlugcatPage.Update` after `inPosition`: with `HasMark` three draws, five when
    /// the flicker takes; with `HasGlow` two, three on a dip. `alpha` is `UseAlpha(1)`.
    pub fn update(&mut self, alpha: f32, in_position: f32, rng: &mut UnityRandom) {
        self.last_mark_alpha = self.mark_alpha;
        if self.has_mark {
            self.mark_alpha = self
                .mark_alpha
                .lerp_and_tick(alpha * in_position, 0.07, 1.0 / 300.0);
            if rng.value() > alpha * 0.95 {
                self.mark_flicker = self
                    .mark_flicker
                    .max(rng.value() * 1.0_f32.lerp(0.4, alpha));
            } else {
                self.mark_flicker =
                    (self.mark_flicker - rng.value() * self.mark_alpha / 10.0).max(0.0);
            }
            if rng.value().mathf_pow(2.0) < self.mark_flicker {
                let to = 1.0 - rng.value() * self.mark_flicker;
                self.mark_alpha = self.mark_alpha.lerp(to, rng.value() * in_position);
            }
        }
        self.last_glow_alpha = self.glow_alpha;
        if self.has_glow {
            self.glow_sin_counter += rng.value() * in_position;
            self.glow_alpha = (0.5 + 0.5 * (self.glow_sin_counter / 25.0).sin()) * in_position;
            if rng.value() < 0.0125 {
                self.glow_alpha *= 1.0 - 0.2 * rng.value().mathf_pow(5.0);
            }
        }
    }

    /// `SlugcatPage.GrafUpdate`'s mark and glow: one draw each per frame for the mark's
    /// glow size and glow B's alpha, from `cosmetic` (D1).
    pub fn draw(&self, stage: &mut StageTree, f: &LightsFrame, cosmetic: &mut UnityRandom) {
        let parallax = f.cam_pos * 80.0 / self.slugcat_depth;
        if let (true, Some([square, glow])) = (self.has_mark, self.mark) {
            let a = self.last_mark_alpha.lerp(self.mark_alpha, f.t) * f.alpha * f.mark_fade;
            let pos = Vec2::new(f.x, f.image_y + 150.0) + self.mark_offset - parallax;
            let tr = stage.transform_mut(square);
            tr.x = pos.x;
            tr.y = pos.y;
            tr.alpha = a.mathf_pow(0.75);
            let shrink = if self.has_glow { -0.5 } else { 0.0 };
            let scale = 3.0_f32.lerp(3.3, a.mathf_pow(0.2))
                + shrink
                + (-0.1_f32).lerp(0.1, cosmetic.value()) * self.mark_flicker;
            let peak = if self.slugcat == Slugcat::White {
                0.4
            } else {
                0.6
            };
            let tr = stage.transform_mut(glow);
            tr.x = pos.x;
            tr.y = pos.y;
            tr.scale_x = scale;
            tr.scale_y = scale;
            tr.alpha = peak * a.mathf_pow(0.75);
        }
        if let (true, Some([b, a])) = (self.has_glow, self.glow) {
            let glow_alpha = self.last_glow_alpha.lerp(self.glow_alpha, f.t);
            let g = 0.8_f32.lerp(1.0, glow_alpha) * f.alpha * f.in_position;
            let pos = Vec2::new(f.x, f.image_y) + self.glow_offset - parallax;
            let flicker = 0.394_f32.lerp(0.406, cosmetic.value() * (1.0 - glow_alpha));
            let sprite = stage.sprite_mut(b);
            sprite.color = self.effect_color.lerp(StageColor::WHITE, 0.3 * g);
            let scale = 20.0_f32.lerp(38.0, g.mathf_pow(0.75));
            sprite.transform.x = pos.x;
            sprite.transform.y = pos.y;
            sprite.transform.scale_x = scale;
            sprite.transform.scale_y = scale;
            sprite.transform.alpha = g.mathf_pow(0.25) * flicker;
            let sprite = stage.sprite_mut(a);
            sprite.color = self.effect_color.lerp(StageColor::WHITE, 0.9 * g);
            let scale = 10.0_f32.lerp(17.0, g.mathf_pow(1.2));
            sprite.transform.x = pos.x;
            sprite.transform.y = pos.y;
            sprite.transform.scale_x = scale;
            sprite.transform.scale_y = scale;
            sprite.transform.alpha = g * 0.6;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The draws `f` made on a stream seeded `seed`.
    fn draws(seed: i32, f: impl FnOnce(&mut UnityRandom)) -> usize {
        let mut rng = UnityRandom::new(seed);
        f(&mut rng);
        let mut probe = UnityRandom::new(seed);
        (0..=8)
            .find(|_| {
                let same = probe == rng;
                if !same {
                    probe.value();
                }
                same
            })
            .expect("at most eight draws")
    }

    #[test]
    fn update_draws_follow_has_mark_and_has_glow() {
        let mut none = PageLights::new(Slugcat::White, false, false);
        assert_eq!(draws(1, |r| none.update(1.0, 1.0, r)), 0);
        let mut counts = [0; 9];
        for seed in 0..400 {
            let mut mark = PageLights::new(Slugcat::White, true, false);
            // A flicker of 0.5 takes the `Lerp` about half the time.
            mark.mark_flicker = 0.5;
            counts[draws(seed, |r| mark.update(0.5, 1.0, r))] += 1;
        }
        assert!(counts[3] > 0 && counts[5] > 0);
        assert_eq!(counts[3] + counts[5], 400);
        let mut counts = [0; 9];
        for seed in 0..2000 {
            let mut glow = PageLights::new(Slugcat::Yellow, false, true);
            counts[draws(seed, |r| glow.update(1.0, 1.0, r))] += 1;
        }
        // The dip (`value < 0.0125`) takes one more.
        assert!(counts[2] > 0 && counts[3] > 0);
        assert_eq!(counts[2] + counts[3], 2000);
    }

    #[test]
    fn mark_update_follows_the_game() {
        // `markAlpha` ticks towards `num * inPosition`, then the flicker rolls.
        let mut l = PageLights::new(Slugcat::White, true, false);
        let mut r = UnityRandom::new(7);
        l.update(1.0, 1.0, &mut r);
        let mut p = UnityRandom::new(7);
        let mut alpha = 0.0_f32.lerp_and_tick(1.0, 0.07, 1.0 / 300.0);
        let mut flicker = 0.0_f32;
        if p.value() > 0.95 {
            flicker = flicker.max(p.value() * 0.4);
        } else {
            flicker = (flicker - p.value() * alpha / 10.0).max(0.0);
        }
        if p.value().mathf_pow(2.0) < flicker {
            let to = 1.0 - p.value() * flicker;
            alpha = alpha.lerp(to, p.value());
        }
        assert_eq!((l.mark_alpha, l.mark_flicker), (alpha, flicker));
        assert_eq!(r, p);
    }

    #[test]
    fn hunter_effect_color_and_placement() {
        let l = PageLights::new(Slugcat::Red, false, false);
        // `Lerp((1, 115/255, 115/255), red, 0.2)`.
        let g = 115.0 / 255.0 * 0.8;
        assert_eq!(l.effect_color, StageColor::rgb(1.0, g, g));
        assert_eq!(
            (l.slugcat_depth, l.mark_offset, l.glow_offset),
            (2.7, Vec2::new(-3.0, -73.0), Vec2::new(-20.0, -90.0))
        );
    }

    #[test]
    fn a_frame_draws_once_for_the_mark_and_once_for_the_glow() {
        let mut stage = StageTree::new();
        let container = stage.new_container();
        let mut l = PageLights::new(Slugcat::White, true, true);
        l.add_mark(&mut stage, container);
        l.glow = Some([0, 1].map(|_| stage.add_sprite(container, Sprite::new("x"))));
        l.mark_alpha = 1.0;
        l.last_mark_alpha = 1.0;
        let f = LightsFrame {
            t: 1.0,
            x: 683.0,
            image_y: 484.0,
            alpha: 1.0,
            cam_pos: Vec2::new(0.25, 0.0),
            in_position: 1.0,
            mark_fade: 1.0,
        };
        assert_eq!(draws(3, |r| l.draw(&mut stage, &f, r)), 2);
        let [square, glow] = l.mark.unwrap();
        // `(MidXpos, imagePos.y + 150) + markOffset - CamPos * 80 / slugcatDepth`.
        let tr = stage.sprite(square).transform;
        assert_eq!((tr.x, tr.y), (683.0 - 15.0 - 20.0 / 3.1, 632.0));
        assert_eq!(tr.alpha, 1.0);
        // The Survivor's glow is dimmer, and smaller with `HasGlow`.
        let tr = stage.sprite(glow).transform;
        assert_eq!(tr.alpha, 0.4);
        assert!((tr.scale_x - 2.8).abs() < 1e-6);
    }
}
