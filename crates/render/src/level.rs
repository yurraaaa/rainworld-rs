//! The level graphic: the current camera's image coloured through the room palette by
//! `level.wgsl` (the game's `LevelColor` shader without its heat, melt, ripple and
//! combined-level variants), drawn in front of the sprites queued before it.
//!
//! Sprites are hidden behind the level as the game hides them: the level shader reads
//! what was drawn before it ("behind", the game's `_GrabTexture` and
//! `_PreLevelColorGrab`); sky pixels and pixels at depth >= 5 show that content wherever
//! it is not black, depth 0-4 always covers it. The same target casts object shadows: a
//! sunlit pixel at depth >= 5 turns shadowed where anything non-black was drawn behind
//! the level a little along the room's light angle (a `SunBlocker` fills the screen with
//! `(1/255, 0, 0)`, which shadows everything and masks nothing).
//!
//! Wet terrain moves the colour lookup up by under a row in thin moving bands above the
//! water line (`_WetTerrain`, `_waterLevel`). Not drawn yet: the heat/melt/ripple
//! variants.
//!
//! Mirrors: RoomCamera.cs (ApplyPalette, ChangeRoom, DrawUpdate shader globals), Room.cs (NowViewed)

use rw_assets::{NoiseTexture, PaletteTexture, PngImage, RoomEffectType, RoomSettings};

/// Uniform inputs of the level shader that stay fixed while a room is shown (per-frame
/// ones are in [`LevelView`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelParams {
    pub grime: f32,
    pub fog_amount: f32,
    pub clouds_speed: f32,
    pub darkness: f32,
    pub contrast: f32,
    pub saturation: f32,
    /// Hue rotation in degrees.
    pub hue: f32,
    pub brightness: f32,
    pub swarm_room: f32,
    /// `_rimFix`: 1 in rooms with a sky scene (sky pixels take `atmosphere`, colour
    /// lookups move half a texel down-left), else 0.
    pub rim_fix: f32,
    /// `_AboveCloudsAtmosphereColor`, RGB.
    pub atmosphere: [f32; 3],
    /// `room.lightAngle`, the object-shadow offset per depth step (`_lightDirAndPixelSize.xy`).
    pub light_dir: [f32; 2],
    /// `_WetTerrain`: 1 unless the room's own settings say `WetTerrain` (off).
    pub wet_terrain: f32,
}

impl LevelParams {
    /// The shader globals with no room effect: identity grading, no rimFix, no light angle.
    pub const DEFAULT: Self = Self {
        grime: 0.0,
        fog_amount: 0.0,
        clouds_speed: 1.0,
        darkness: 1.0,
        contrast: 1.0,
        saturation: 1.0,
        hue: 0.0,
        brightness: 0.0,
        swarm_room: 0.0,
        rim_fix: 0.0,
        atmosphere: [0.0; 3],
        light_dir: [0.0; 2],
        wet_terrain: 0.0,
    };

    /// `atmosphereColor` of `AboveCloudsView` and `RoofTopView`, which their constructors
    /// set as `_AboveCloudsAtmosphereColor` (changed later only by the `DayNight` effect).
    pub const SKY_SCENE_ATMOSPHERE: [f32; 3] = [41.0 / 255.0, 59.0 / 255.0, 81.0 / 255.0];

    /// The game's values for a room with `settings` and geometry light angle `light_dir`
    /// shown with `palette`, without a ghost and without the colour-grading effects (no
    /// base-game room uses them; they come with the DLCs, so their uniforms stay at the
    /// game's defaults).
    #[must_use]
    pub fn new(settings: &RoomSettings, palette: &PaletteTexture, light_dir: [f32; 2]) -> Self {
        let rim_fix = Self::rim_fix(settings);
        Self {
            grime: settings.grime,
            fog_amount: palette.fog_amount(),
            rim_fix: if rim_fix { 1.0 } else { 0.0 },
            atmosphere: if rim_fix {
                Self::SKY_SCENE_ATMOSPHERE
            } else {
                [0.0; 3]
            },
            light_dir,
            wet_terrain: if settings.wet_terrain { 1.0 } else { 0.0 },
            ..Self::DEFAULT
        }
    }

    /// `Room.NowViewed`: `_rimFix` is 1 when the room has an `AboveCloudsView`,
    /// `RoofTopView` or `PinkSky` effect, whatever its amount.
    #[must_use]
    pub fn rim_fix(settings: &RoomSettings) -> bool {
        [
            RoomEffectType::AboveCloudsView,
            RoomEffectType::RoofTopView,
            RoomEffectType::PinkSky,
        ]
        .into_iter()
        .any(|kind| settings.has_effect(kind.value()))
    }

    /// `_light`: 1 when `Clouds` is 0, else `lerp(lerp(shader_light, -1, clouds), -0.4,
    /// ghost_mode)`; `shader_light` is `num4` of `DrawUpdate` (the rain cycle's
    /// `ShaderLight` in rooms with rain, else 1).
    #[must_use]
    pub fn light(clouds: f32, shader_light: f32, ghost_mode: f32) -> f32 {
        if clouds == 0.0 {
            1.0
        } else {
            let cloudy = shader_light + (-1.0 - shader_light) * clouds.clamp(0.0, 1.0);
            cloudy + (-0.4 - cloudy) * ghost_mode.clamp(0.0, 1.0)
        }
    }
}

/// Everything [`Renderer::set_level`](crate::Renderer::set_level) uploads for one camera.
#[derive(Debug, Clone, Copy)]
pub struct LevelGraphics<'a> {
    /// Room position of the image's bottom-left corner.
    pub camera_pos: [f32; 2],
    pub image: &'a PngImage,
    pub palette: &'a PaletteTexture,
    pub noise: &'a NoiseTexture,
    pub params: LevelParams,
}

/// Per-frame inputs of the level: the game's visible window, shader time and light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelView {
    /// Bottom-left of the visible window in room pixels.
    pub origin: [f32; 2],
    pub size: [f32; 2],
    /// `_RAIN`: shader time in seconds / 5.
    pub rain: f32,
    /// `_light`: 1 = full sun, -1 = no sun ([`LevelParams::light`]).
    pub light: f32,
    /// `_waterLevel`: the water line (water + 100 px, or the death fall + 180 px) as a
    /// fraction of the screen from its top, 0 in gates and shelters, 1 (clamped) with
    /// neither. Wet terrain is drawn only above it; the `Fog` effect fades out below.
    pub water_level: f32,
}

#[cfg(test)]
mod reference {
    //! CPU version of `level.wgsl`'s colour function, checked against values derived by
    //! hand from the game's shader.

    use super::*;

    /// The shader's `3.14 * 2`, not 2π.
    #[allow(clippy::approx_constant, reason = "the game's shader uses 3.14")]
    pub const GAME_TAU: f32 = 6.28;

    /// Inputs of one level pixel.
    pub struct Shading<'a> {
        pub palette: &'a PaletteTexture,
        pub noise: &'a NoiseTexture,
        pub params: LevelParams,
        pub rain: f32,
        /// `_light`.
        pub light: f32,
        /// The image's top row (decal colour table), 0..1 RGB.
        pub top_row: &'a [[f32; 3]],
        /// What was drawn behind the level at the object-shadow lookup.
        pub cast: [f32; 3],
    }

    /// The object-shadow grab position (x right, y up, 0..1 over the screen) of a pixel
    /// at D3D screen position `sp` (x right, y down) and `depth` >= 5.
    pub fn grab_pos(light_dir: [f32; 2], sp: [f32; 2], depth: i32) -> [f32; 2] {
        let fix: [f32; 4] = [
            (sp[0] / 0.3).clamp(0.0, 1.0),
            (sp[1] / 0.3).clamp(0.0, 1.0),
            (sp[0] / 0.1).clamp(0.0, 1.0),
            (sp[1] / 0.1).clamp(0.0, 1.0),
        ];
        let d = (depth - 5) as f32;
        let g = [
            sp[0] - light_dir[0] / 1400.0 * d * fix[0],
            1.0 - sp[1] + light_dir[1] / 800.0 * d * fix[1],
        ];
        let centre = [0.5, 0.3];
        std::array::from_fn(|i| {
            let zoomed = (g[i] - centre[i]) * (1.0 + d / 460.0) + centre[i];
            g[i] + (zoomed - g[i]) * fix[2 + i]
        })
    }

    pub trait Mix {
        fn mix(self, to: Self, t: f32) -> Self;
    }

    impl Mix for [f32; 3] {
        fn mix(self, to: Self, t: f32) -> Self {
            std::array::from_fn(|i| self[i] + (to[i] - self[i]) * t)
        }
    }

    pub trait Fmod {
        fn fmod_trunc(self, y: f32) -> f32;
    }

    impl Fmod for f32 {
        fn fmod_trunc(self, y: f32) -> f32 {
            self - y * (self / y).trunc()
        }
    }

    impl Shading<'_> {
        fn pal(&self, x: i32, row: i32) -> [f32; 3] {
            let t = self
                .palette
                .texel(x.clamp(0, 31) as usize, row.clamp(0, 7) as usize);
            [t[0], t[1], t[2]].map(|c| f32::from(c) / 255.0)
        }

        /// Bilinear, repeat-wrapped red channel of the noise texture.
        fn noise(&self, uv: [f32; 2]) -> f32 {
            let n = NoiseTexture::SIZE as f32;
            let (x, y) = (uv[0] * n - 0.5, uv[1] * n - 0.5);
            let (x0, y0) = (x.floor(), y.floor());
            let (fx, fy) = (x - x0, y - y0);
            let (a, b) = (self.noise_texel(x0, y0), self.noise_texel(x0 + 1.0, y0));
            let (c, d) = (
                self.noise_texel(x0, y0 + 1.0),
                self.noise_texel(x0 + 1.0, y0 + 1.0),
            );
            let low = a + (b - a) * fx;
            let high = c + (d - c) * fx;
            low + (high - low) * fy
        }

        /// Noise texel at integer coordinates, wrapped.
        fn noise_texel(&self, x: f32, y: f32) -> f32 {
            let n = NoiseTexture::SIZE as f32;
            let red = self
                .noise
                .red(x.rem_euclid(n) as usize, y.rem_euclid(n) as usize);
            f32::from(red) / 255.0
        }

        /// Colour of a pixel with code `code` at image `uv` (v up); `behind` is what was
        /// drawn before the level there. Grading is left out (identity by default).
        pub fn colour(&self, code: [u8; 3], uv: [f32; 2], behind: [f32; 3]) -> [f32; 3] {
            let p = self.params;
            let fog = self.pal(1, 7);
            let (colour, shows_behind) = if code == [255, 255, 255] {
                let sky = if p.rim_fix > 0.5 {
                    p.atmosphere
                } else {
                    self.pal(0, 7)
                };
                (sky, true)
            } else {
                let raw_red = i32::from(code[0]);
                let mut green = i32::from(code[1]);
                let blue = f32::from(code[2]) / 255.0;
                let mut not_floor_dark = 1.0;
                if green >= 16 {
                    not_floor_dark = 0.0;
                    green -= 16;
                }
                let decal = green >= 8;
                if decal {
                    green -= 8;
                }
                let drift = self.rain * 0.1 * p.clouds_speed;
                let layer = 0.003 * (raw_red % 30) as f32;
                let n = self.noise([
                    uv[0] * 0.5 + drift - layer,
                    1.0 - uv[1] * 0.5 + self.rain * 0.2 * p.clouds_speed - layer,
                ]);
                let wave = 0.5 + ((n + drift - uv[1]).fmod_trunc(1.0) * GAME_TAU).sin() * 0.5;
                let mut shadow = ((wave - 0.5) * 6.0 + 0.5 - self.light * 4.0).clamp(0.0, 1.0);
                let mut red = raw_red;
                if red > 90 {
                    red -= 90;
                } else {
                    shadow = 1.0;
                }
                let group = ((red - 1) as f32 / 30.0).floor().clamp(0.0, 2.0) as i32;
                let depth = ((red - 1) as f32).fmod_trunc(30.0) as i32;
                let depth_f = depth as f32;
                let column = (depth_f * not_floor_dark) as i32;
                if shadow != 1.0 && depth >= 5 && self.cast != [0.0; 3] {
                    shadow = 1.0;
                }
                let mut c = self
                    .pal(column, group + 3)
                    .mix(self.pal(column, group), shadow);
                if green >= 4 {
                    let rb = ((self.rain
                        + self.noise([uv[0] * 2.0, uv[1] * 2.0]) * 4.0
                        + depth_f / 12.0)
                        * GAME_TAU)
                        .sin()
                        * 0.5
                        + 0.5;
                    c = c.mix(self.pal((5.5 + rb * 25.0).floor() as i32, 6), 0.2 * p.grime);
                }
                if decal {
                    let x = (255 - i32::from(code[2])).clamp(0, self.top_row.len() as i32 - 1);
                    let mut d = self.top_row[x as usize];
                    if group == 2 {
                        d = d.mix([1.0; 3], 0.2 - shadow * 0.1);
                    }
                    d = d.mix(fog, depth_f / 60.0);
                    let product = std::array::from_fn(|i| c[i] * d[i] * 1.5);
                    let t =
                        0.9 + (0.3 + 0.4 * shadow - 0.9) * ((depth_f - 3.5) * 0.3).clamp(0.0, 1.0);
                    c = c.mix(d, 0.7).mix(product, t);
                } else if green > 0 && green < 3 {
                    let row = 5 - (green - 1) * 2;
                    let near = self.pal(30, row).mix(self.pal(31, row), shadow);
                    let far = self.pal(30, row - 1).mix(self.pal(31, row - 1), shadow);
                    c = c.mix(near.mix(far, depth_f / 30.0), blue);
                } else if green == 3 {
                    c = c.mix([1.0; 3], blue * p.swarm_room);
                }
                let scale = if depth < 10 {
                    not_floor_dark + (1.0 - not_floor_dark) * 0.5
                } else {
                    1.0
                };
                c = c.mix(fog, (depth_f * scale * p.fog_amount / 30.0).clamp(0.0, 1.0));
                (c, depth >= 5)
            };
            let behind_drawn = behind[0] > 1.0 / 255.0 || behind[1] != 0.0 || behind[2] != 0.0;
            if shows_behind && behind_drawn {
                behind
            } else {
                colour
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::reference::{Fmod, GAME_TAU, Mix, Shading, grab_pos};
    use super::*;
    use rw_assets::PaletteImage;

    /// Test palette: top-half texel (x, runtime row r) is (8x, 30r, 7), and the fog
    /// amount texel gives `fog_amount` 0.6.
    trait TestPalette {
        fn test() -> Self;
        fn rgb(&self, x: usize, row: usize) -> [f32; 3];
    }

    impl TestPalette for PaletteTexture {
        fn test() -> Self {
            let pixels = (0..16)
                .flat_map(|png_row: usize| (0..32).map(move |x: usize| (x, png_row)))
                .map(|(x, png_row)| {
                    let r = 7usize.saturating_sub(png_row);
                    if png_row == 0 && x == 9 {
                        [102, 0, 0, 255]
                    } else {
                        [(8 * x) as u8, (30 * r) as u8, 7, 255]
                    }
                })
                .collect();
            let image = PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap();
            Self::blend(&image, None, 0.0)
        }

        fn rgb(&self, x: usize, row: usize) -> [f32; 3] {
            let t = self.texel(x, row);
            [t[0], t[1], t[2]].map(|c| f32::from(c) / 255.0)
        }
    }

    trait ApproxEq {
        /// Equal within 1e-5 per channel.
        fn approx(self, other: Self) -> bool;
    }

    impl ApproxEq for [f32; 3] {
        fn approx(self, other: Self) -> bool {
            self.iter().zip(other).all(|(x, y)| (x - y).abs() < 1e-5)
        }
    }

    impl LevelParams {
        fn test(clouds: f32, grime: f32, palette: &PaletteTexture) -> Self {
            let settings = RoomSettings {
                clouds,
                grime,
                ..RoomSettings::ANCESTOR
            };
            Self::new(&settings, palette, [0.0, 0.0])
        }
    }

    /// A noise texture that is 0 everywhere.
    trait ZeroNoise {
        fn zero() -> Self;
    }

    impl ZeroNoise for NoiseTexture {
        fn zero() -> Self {
            let black = PngImage::from_pixels(64, 64, vec![[0, 0, 0, 255]; 64 * 64]);
            Self::from_image(&black).unwrap()
        }
    }

    #[test]
    fn light_from_clouds() {
        assert_eq!(LevelParams::light(0.0, 0.3, 0.0), 1.0);
        assert_eq!(LevelParams::light(1.0, 1.0, 0.0), -1.0);
        assert!((LevelParams::light(0.25, 1.0, 0.0) - 0.5).abs() < 1e-6);
        // Rain light 0.6 at clouds 0.5: lerp(0.6, -1, 0.5) = -0.2; a full ghost -0.4.
        assert!((LevelParams::light(0.5, 0.6, 0.0) + 0.2).abs() < 1e-6);
        assert!((LevelParams::light(0.5, 0.6, 1.0) + 0.4).abs() < 1e-6);
    }

    #[test]
    fn hand_derived_pixels() {
        let pal = PaletteTexture::test();
        let noise = NoiseTexture::zero();
        let top_row = vec![[0.5, 0.25, 1.0]; 1400];
        assert!((pal.fog_amount() - 0.6).abs() < 1e-6);
        let s = Shading {
            palette: &pal,
            noise: &noise,
            params: LevelParams::test(0.0, 0.5, &pal),
            rain: 0.0,
            light: LevelParams::light(0.0, 1.0, 0.0),
            cast: [0.0; 3],
            top_row: &top_row,
        };
        let uv = [0.3, 0.6];
        let black = [0.0; 3];
        let fog = pal.rgb(1, 7);

        // Sky: palette (0, 7).
        assert!(s.colour([255, 255, 255], uv, black).approx(pal.rgb(0, 7)));
        // R = 1: depth 0, group 0, no sun -> shadowed row 0, no fog.
        assert!(s.colour([1, 0, 0], uv, black).approx(pal.rgb(0, 0)));
        // R = 95: sunlit (Clouds 0 -> light 1 -> shadow 0), depth 4 -> lit row 3,
        // fog 4 * 1 * 0.6 / 30.
        assert!(
            s.colour([95, 0, 0], uv, black)
                .approx(pal.rgb(4, 3).mix(fog, 0.08))
        );
        // R = 73, G = 16: depth 12, group 2, floor-dark -> column 0 of row 2; fog
        // 12 * 1 * 0.6 / 30.
        assert!(
            s.colour([73, 16, 0], uv, black)
                .approx(pal.rgb(0, 2).mix(fog, 0.24))
        );
        // Floor-dark below depth 10 scales fog by lerp(0, 1, 0.5): 5 * 0.5 * 0.6 / 30.
        assert!(
            s.colour([6, 16, 0], uv, black)
                .approx(pal.rgb(0, 0).mix(fog, 0.05))
        );
        // Effect colour A at depth 3, full amount, shadowed: x31 of rows 5 -> 4 by 3/30.
        let want = pal.rgb(31, 5).mix(pal.rgb(31, 4), 0.1).mix(fog, 0.06);
        assert!(s.colour([4, 1, 255], uv, black).approx(want));
        // Effect colour B, amount 128, depth 0 sunlit: x30 of row 3.
        let want = pal.rgb(0, 3).mix(pal.rgb(30, 3), 128.0 / 255.0);
        assert!(s.colour([91, 2, 128], uv, black).approx(want));
        // Grime (G = 4), zero noise, rain 0, depth 0: rb = 0.5 -> x = 18 of row 6,
        // weight 0.2 * Grime.
        let want = pal.rgb(0, 0).mix(pal.rgb(18, 6), 0.1);
        assert!(s.colour([1, 4, 0], uv, black).approx(want));
        // Decal (G = 8), B = 250 -> table x 5; group 0, shadow 1, depth 0 -> 0.9 toward
        // the 1.5x product after 70% toward the decal colour.
        let (base, d) = (pal.rgb(0, 0), [0.5, 0.25, 1.0]);
        let product = std::array::from_fn(|i| base[i] * d[i] * 1.5);
        assert!(
            s.colour([1, 8, 250], uv, black)
                .approx(base.mix(d, 0.7).mix(product, 0.9))
        );
    }

    #[test]
    fn sprites_show_through_sky_and_depth_5_and_beyond() {
        let pal = PaletteTexture::test();
        let noise = NoiseTexture::zero();
        let s = Shading {
            palette: &pal,
            noise: &noise,
            params: LevelParams::test(1.0, 0.0, &pal),
            rain: 0.0,
            light: LevelParams::light(1.0, 1.0, 0.0),
            cast: [0.0; 3],
            top_row: &[],
        };
        let sprite = [0.9, 0.1, 0.2];
        let uv = [0.5, 0.5];
        assert_eq!(s.colour([255, 255, 255], uv, sprite), sprite);
        assert_eq!(s.colour([6, 0, 0], uv, sprite), sprite, "depth 5");
        assert_ne!(s.colour([5, 0, 0], uv, sprite), sprite, "depth 4 covers");
        // Red of at most 1/255 behind counts as nothing drawn.
        let faint = [1.0 / 255.0, 0.0, 0.0];
        assert_ne!(s.colour([6, 0, 0], uv, faint), faint);
    }

    #[test]
    fn cloud_shadow_from_noise() {
        // Clouds 0.5 -> light 0: with zero noise at rain 0,
        // shadow = saturate((wave - 0.5) * 6 + 0.5), wave = 0.5 + 0.5 sin(fmod(-v, 1) * 6.28).
        let pal = PaletteTexture::test();
        let noise = NoiseTexture::zero();
        let s = Shading {
            palette: &pal,
            noise: &noise,
            params: LevelParams::test(0.5, 0.0, &pal),
            rain: 0.0,
            light: LevelParams::light(0.5, 1.0, 0.0),
            cast: [0.0; 3],
            top_row: &[],
        };
        assert_eq!(s.light, 0.0);
        for v in [0.1f32, 0.3, 0.55, 0.8] {
            let wave = 0.5 + ((-v).fmod_trunc(1.0) * GAME_TAU).sin() * 0.5;
            let shadow = ((wave - 0.5) * 6.0 + 0.5).clamp(0.0, 1.0);
            let want = pal.rgb(0, 3).mix(pal.rgb(0, 0), shadow);
            assert!(
                s.colour([91, 0, 0], [0.2, v], [0.0; 3]).approx(want),
                "v = {v}"
            );
        }
    }

    /// The object-shadow step: a sunlit pixel at depth >= 5 is shadowed where something
    /// was drawn behind the level at the lookup; depth < 5 and unlit pixels are not.
    /// The SunBlocker's `(1/255, 0, 0)` shadows without masking.
    #[test]
    fn object_shadow_and_sun_block() {
        let pal = PaletteTexture::test();
        let noise = NoiseTexture::zero();
        let sun_block = [1.0 / 255.0, 0.0, 0.0];
        let mut s = Shading {
            palette: &pal,
            noise: &noise,
            params: LevelParams::test(0.0, 0.0, &pal),
            rain: 0.0,
            light: 1.0,
            top_row: &[],
            cast: sun_block,
        };
        let uv = [0.5, 0.5];
        let fog = pal.rgb(1, 7);
        // R = 101: sunlit, depth 10, group 0 -> shadowed row 0 instead of lit row 3.
        let want = pal.rgb(10, 0).mix(fog, 0.2);
        assert!(s.colour([101, 0, 0], uv, sun_block).approx(want));
        // Depth 4 keeps its sun.
        assert!(
            s.colour([95, 0, 0], uv, sun_block)
                .approx(pal.rgb(4, 3).mix(fog, 0.08))
        );
        s.cast = [0.0; 3];
        assert!(
            s.colour([101, 0, 0], uv, sun_block)
                .approx(pal.rgb(10, 3).mix(fog, 0.2))
        );
    }

    /// `_lightDirAndPixelSize` with SS_AI's light angle (0.454, 0.891), and
    /// `FixEdgeShadowStretch` near the top-left corner; values worked from the shader.
    #[test]
    fn shadow_grab_position() {
        let light = [0.454, 0.891];
        let [x, y] = grab_pos(light, [0.5, 0.5], 25);
        assert!((x - 0.493_232_3).abs() < 1e-6 && (y - 0.531_939_1).abs() < 1e-6);
        let [x, y] = grab_pos(light, [0.05, 0.03], 15);
        assert!((x - 0.044_562_34).abs() < 1e-6 && (y - 0.975_490_6).abs() < 1e-6);
    }

    /// `_rimFix`: AboveCloudsView, RoofTopView or PinkSky at any amount; sky pixels then
    /// take the scene's atmosphere colour.
    #[test]
    fn rim_fix_sky() {
        let pal = PaletteTexture::test();
        let noise = NoiseTexture::zero();
        let settings = RoomSettings {
            effects: vec![rw_assets::RoomEffect {
                kind: RoomEffectType::AboveCloudsView.value().to_owned(),
                amount: 0.0,
            }],
            ..RoomSettings::ANCESTOR
        };
        let params = LevelParams::new(&settings, &pal, [0.0; 2]);
        assert_eq!(params.rim_fix, 1.0);
        assert_eq!(params.atmosphere, LevelParams::SKY_SCENE_ATMOSPHERE);
        let s = Shading {
            palette: &pal,
            noise: &noise,
            params,
            rain: 0.0,
            light: 1.0,
            top_row: &[],
            cast: [0.0; 3],
        };
        let sky = s.colour([255, 255, 255], [0.5, 0.5], [0.0; 3]);
        assert!(sky.approx([41.0 / 255.0, 59.0 / 255.0, 81.0 / 255.0]));
        assert_eq!(LevelParams::test(0.0, 0.0, &pal).rim_fix, 0.0);
    }
}
