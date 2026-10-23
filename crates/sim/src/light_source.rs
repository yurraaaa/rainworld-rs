//! `LightSource`: a light in its room's update list and `lightSources`, made by
//! `Room.Loaded` for a placed light and by its owners (the Monk's guide light, a
//! lizard's or the glowing player's graphics, fireflies). Blinking and night lights
//! (no base room has one) come with the DLCs that use them (D24). Its sprites are
//! `rw-graphics`'. No draws.
//!
//! Mirrors: LightSource.cs

use crate::float_ext::SCurve;
use crate::{FloatExt, Vec2};

/// The object a light is tied to (`tiedToObject`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightTie {
    /// A `HologramLight` by id.
    HoloLight(u64),
    /// A lizard (its graphics' head light).
    Lizard(crate::creature::CreatureId),
    /// A cosmetic insect (a firefly) by its place in the update list.
    Insect(u64),
    /// The player (its graphics' light when `glowing`).
    Player,
    /// A `GreenSpark` by its place in the update list.
    GreenSpark(u64),
    /// A `LightFixture` (a `Redlight` or `HolyFire`) by its place in the update list.
    Fixture(u64),
}

/// `LightSource.ElementName`: the texture its sprite 0 is a `CircularSprite` of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightElement {
    #[default]
    FutileWhite,
    /// `RedlightLight`'s: the 128 px `LightMask0` atlas.
    LightMask0,
}

impl LightElement {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::FutileWhite => "Futile_White",
            Self::LightMask0 => "LightMask0",
        }
    }
}

/// What `LightSource.Update` reads of its room and camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSourceTick {
    /// The tied object is slated or in another room.
    pub tie_gone: bool,
    /// `room.world.rainCycle.ShaderLight`, for [`LightSource::alpha_now`].
    pub shader_light: f32,
    /// `cameras[0].PixelColorAtCoordinate(pos)` at the position after `setPos`, when
    /// camera 0 shows the room.
    pub environment_color: Option<[f32; 4]>,
    /// `room.FloatWaterLevel(pos)` at the position after `setPos`.
    pub water_level: f32,
}

/// `LightSource`.
#[derive(Debug, Clone, PartialEq)]
pub struct LightSource {
    /// Unique; also its place in the update list.
    pub id: u64,
    pub room: i32,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub set_pos: Option<Vec2>,
    pub rad: f32,
    pub last_rad: f32,
    pub set_rad: Option<f32>,
    pub alpha: f32,
    pub last_alpha: f32,
    pub set_alpha: Option<f32>,
    pub affected_by_palette_darkness: f32,
    /// `color`'s setter: the colour over its largest component (white when black).
    pub color: [f32; 3],
    /// The largest component the setter divided by.
    pub color_alpha: f32,
    /// `color.a`: Unity's `Color` division divides the alpha too, so it is the given
    /// alpha over `color_alpha` (1 for white). Futile multiplies it by the sprite's
    /// alpha, which cancels the draw's `colorAlpha` factor.
    pub color_a: f32,
    pub environmental_light: bool,
    /// `colorFromEnvironment`: the colour is the camera's level pixel under it.
    pub color_from_environment: bool,
    /// `effectColor`: the palette's effect colour 0 or 1, -1 for none.
    pub effect_color: i32,
    /// `fadeWithSun`: [`Self::alpha_now`] follows the rain cycle's light.
    pub fade_with_sun: bool,
    pub tied_to: Option<LightTie>,
    pub require_up_keep: bool,
    pub stay_alive: bool,
    /// Drawn in `Water` rather than `ForegroundLights`; set when it is first drawn in
    /// a room with the `WaterLights` effect.
    pub submersible: bool,
    /// `FlatLight` rather than `LightSource`.
    pub flat: bool,
    /// `ElementName`.
    pub element: LightElement,
    /// `Room.FloatWaterLevel(pos)` at the last update.
    pub water_surface_level: f32,
}

/// What `HologramLight.DrawSprites` hard-sets on its light each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightHardSet {
    pub pos: Vec2,
    pub rad: f32,
    pub alpha: f32,
}

impl LightSource {
    /// `new LightSource(initPos, environmentalLight, color, tiedToObject)`.
    #[must_use]
    pub fn new(
        id: u64,
        room: i32,
        pos: Vec2,
        environmental_light: bool,
        color: [f32; 3],
        tied_to: Option<LightTie>,
    ) -> Self {
        let mut light = Self {
            id,
            room,
            slated: false,
            pos,
            last_pos: pos,
            set_pos: None,
            rad: 0.0,
            last_rad: 0.0,
            set_rad: None,
            alpha: 0.0,
            last_alpha: 0.0,
            set_alpha: None,
            affected_by_palette_darkness: if environmental_light { 0.0 } else { 1.0 },
            color: [1.0; 3],
            color_alpha: 0.0,
            color_a: 1.0,
            environmental_light,
            color_from_environment: false,
            effect_color: -1,
            fade_with_sun: false,
            tied_to,
            require_up_keep: false,
            stay_alive: true,
            submersible: false,
            flat: false,
            element: LightElement::FutileWhite,
            water_surface_level: 0.0,
        };
        light.set_color(color);
        light
    }

    /// The light `id` among a room's `lights`.
    pub fn find_mut(lights: &mut [Self], id: u64) -> Option<&mut Self> {
        lights.iter_mut().find(|l| l.id == id)
    }

    /// The `color` setter of an opaque `new Color(r, g, b)`.
    pub fn set_color(&mut self, c: [f32; 3]) {
        let [r, g, b] = c;
        self.set_color_rgba([r, g, b, 1.0]);
    }

    /// The `color` setter: the largest of r, g and b is `colorAlpha`, and all four
    /// channels are divided by it (`Color / float`); black becomes `Color.white`.
    pub fn set_color_rgba(&mut self, c: [f32; 4]) {
        let [r, g, b, a] = c;
        self.color_alpha = [r, g, b]
            .iter()
            .fold(0.0f32, |m, &v| if v > m { v } else { m });
        if self.color_alpha == 0.0 {
            self.color = [1.0; 3];
            self.color_a = 1.0;
        } else {
            self.color = [r, g, b].map(|v| v / self.color_alpha);
            self.color_a = a / self.color_alpha;
        }
    }

    /// `ApplyPalette` for a light drawn by camera `c`: an effect-coloured light takes
    /// its palette texel.
    pub fn apply_palette(&mut self, c: &crate::CameraImage) {
        if self.effect_color >= 0 {
            self.set_color_rgba(c.effect_color(self.effect_color));
        }
    }

    /// The `Alpha` property (Watcher's void weaver and night lights not ported).
    #[must_use]
    pub fn alpha_now(&self, shader_light: f32) -> f32 {
        if self.fade_with_sun {
            self.alpha * shader_light.inverse_lerp(-0.5, 0.0).mathf_pow(1.2)
        } else {
            self.alpha
        }
    }

    /// `Lightness`.
    #[must_use]
    pub fn lightness(&self) -> f32 {
        if self.color_alpha == 0.0 || self.alpha == 0.0 {
            0.0
        } else {
            self.color.iter().sum::<f32>() / 3.0 * self.color_alpha * self.alpha
        }
    }

    /// Where the light reaches `pos`, its `SCurve(InverseLerp(Rad, 0, dist), 0.5) *
    /// Lightness` (`Room.LightSourceExposure`'s and `LightSourceColor`'s term).
    #[must_use]
    pub fn exposure_at(&self, pos: Vec2) -> Option<f32> {
        pos.dist_less(self.pos, self.rad).then(|| {
            pos.distance(self.pos)
                .inverse_lerp(self.rad, 0.0)
                .s_curve(0.5)
                * self.lightness()
        })
    }

    /// `Room.LightSourceColor(pos)` over `lights`: per channel the largest of colour
    /// times exposure.
    #[must_use]
    pub fn color_over(lights: &[Self], pos: Vec2) -> [f32; 3] {
        let mut c = [0.0f32; 3];
        for l in lights.iter().filter(|l| !l.slated) {
            if let Some(e) = l.exposure_at(pos) {
                for (out, channel) in c.iter_mut().zip(l.color) {
                    *out = out.max(channel * e);
                }
            }
        }
        c
    }

    pub fn hard_set(&mut self, s: LightHardSet) {
        self.last_pos = s.pos;
        self.pos = s.pos;
        self.set_pos = None;
        self.last_rad = s.rad;
        self.rad = s.rad;
        self.set_rad = None;
        self.last_alpha = s.alpha;
        self.alpha = s.alpha;
        self.set_alpha = None;
    }

    /// `HardSetRad`.
    pub fn hard_set_rad(&mut self, r: f32) {
        self.last_rad = r;
        self.rad = r;
        self.set_rad = None;
    }

    /// `HardSetAlpha`.
    pub fn hard_set_alpha(&mut self, a: f32) {
        self.last_alpha = a;
        self.alpha = a;
        self.set_alpha = None;
    }

    /// `Update`: the pending sets, then fading out (rad and alpha to 0) once the tied
    /// object is gone (slated or in another room), destroyed when dark; `lastAlpha`
    /// takes the `Alpha` property; the colour from the environment.
    pub fn update(&mut self, tick: &LightSourceTick) {
        self.last_pos = self.pos;
        if let Some(p) = self.set_pos.take() {
            self.pos = p;
        }
        self.last_rad = self.rad;
        if let Some(r) = self.set_rad.take() {
            self.rad = r;
        }
        if self.tied_to.is_some() && tick.tie_gone {
            if self.alpha == 0.0 {
                self.slated = true;
            } else {
                self.set_alpha = Some(0.0);
                self.set_rad = Some(0.0);
            }
        }
        self.last_alpha = self.alpha_now(tick.shader_light);
        if let Some(a) = self.set_alpha.take() {
            self.alpha = a;
        }
        if self.color_from_environment
            && let Some(c) = tick.environment_color
        {
            self.set_color_rgba(c);
        }
        self.water_surface_level = tick.water_level;
        if self.require_up_keep {
            if self.stay_alive {
                self.stay_alive = false;
            } else {
                self.slated = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LightHardSet, LightSource, LightSourceTick, LightTie};
    use crate::Vec2;

    fn tick(tie_gone: bool) -> LightSourceTick {
        LightSourceTick {
            tie_gone,
            shader_light: 1.0,
            environment_color: None,
            water_level: f32::MIN,
        }
    }

    /// A placed light as `Room.Loaded` makes it: white, environmental, rad and alpha
    /// set on the first update; `lastAlpha` is the `Alpha` before the set, scaled by
    /// `InverseLerp(-0.5, 0, ShaderLight)^1.2` while fading with the sun; the colour
    /// follows the camera's level pixel.
    #[test]
    fn placed_light_fades_with_the_sun() {
        let mut l = LightSource::new(4, 0, Vec2::new(10.0, 20.0), true, [1.0; 3], None);
        l.set_rad = Some(50.0);
        l.set_alpha = Some(0.75);
        l.fade_with_sun = true;
        l.color_from_environment = true;
        let mut t = tick(false);
        t.shader_light = -0.25;
        t.environment_color = Some([0.25, 0.5, 0.125, 1.0]);
        l.update(&t);
        assert_eq!((l.rad, l.alpha, l.last_alpha), (50.0, 0.75, 0.0));
        assert_eq!(
            (l.color, l.color_alpha, l.color_a),
            ([0.5, 1.0, 0.25], 0.5, 2.0)
        );
        let want = 0.75 * 0.5f32.powf(1.2);
        assert!((l.alpha_now(-0.25) - want).abs() < 1e-6);
        l.update(&t);
        assert!((l.last_alpha - want).abs() < 1e-6);
        assert_eq!((l.alpha_now(0.5), l.alpha_now(-0.6)), (0.75, 0.0));
        l.fade_with_sun = false;
        assert_eq!(l.alpha_now(-1.0), 0.75);
    }

    /// `Lightness`: the mean of the normalised colour times colour alpha and alpha;
    /// the exposure term is 0 outside `Rad` and `Lightness` at the centre.
    #[test]
    fn lightness_and_exposure() {
        let mut l = LightSource::new(1, 0, Vec2::ZERO, true, [0.0, 0.25, 0.5], None);
        l.alpha = 0.8;
        l.rad = 100.0;
        assert!((l.lightness() - 0.5 * 0.5 * 0.8).abs() < 1e-6);
        assert_eq!(l.exposure_at(Vec2::new(100.0, 0.0)), None);
        assert!((l.exposure_at(Vec2::ZERO).expect("in") - l.lightness()).abs() < 1e-6);
        let half = l.exposure_at(Vec2::new(50.0, 0.0)).expect("in");
        assert!((half - 0.5 * l.lightness()).abs() < 1e-6);
    }

    /// The guide light's colour (1, 0.8, 0.3) is already at most 1: kept, colour alpha 1.
    #[test]
    fn color_over_its_largest_component() {
        let l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
        assert_eq!((l.color, l.color_alpha), ([1.0, 0.8, 0.3], 1.0));
        assert_eq!(l.affected_by_palette_darkness, 1.0);
        let l = LightSource::new(1, 0, Vec2::ZERO, true, [0.0, 0.25, 0.5], None);
        assert_eq!(
            (l.color, l.color_alpha, l.color_a),
            ([0.0, 0.5, 1.0], 0.5, 2.0)
        );
        assert_eq!(l.affected_by_palette_darkness, 0.0);
        let l = LightSource::new(1, 0, Vec2::ZERO, false, [0.0; 3], None);
        assert_eq!((l.color, l.color_alpha, l.color_a), ([1.0; 3], 0.0, 1.0));
    }

    /// Lit when its tie goes: rad and alpha are set to 0 (alpha this update, rad the
    /// next), then it is destroyed on the update that finds it dark.
    #[test]
    fn fades_then_dies_without_its_tie() {
        let mut l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
        l.tied_to = Some(LightTie::HoloLight(3));
        l.hard_set(LightHardSet {
            pos: Vec2::new(5.0, 6.0),
            rad: 200.0,
            alpha: 0.1,
        });
        l.update(&tick(false));
        assert_eq!((l.rad, l.alpha, l.last_alpha), (200.0, 0.1, 0.1));
        l.update(&tick(true));
        assert!(!l.slated);
        assert_eq!((l.rad, l.alpha, l.last_alpha), (200.0, 0.0, 0.1));
        l.update(&tick(true));
        assert!(l.slated);
        assert_eq!((l.rad, l.last_rad), (0.0, 200.0));
        assert_eq!(l.pos, Vec2::new(5.0, 6.0));
    }
}
