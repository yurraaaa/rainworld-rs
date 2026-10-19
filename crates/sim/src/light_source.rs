//! `LightSource` (128563): a light in its room's update list and `lightSources`. Only
//! the Monk's guide light and a lizard's graphics make one so far, so the placed-object
//! paths (blinking, colour from the environment, night lights, fading with the sun) stay at their
//! defaults. Its sprites are `rw-graphics`'. No draws.
//!
//! Mirrors: LightSource.cs

use crate::Vec2;

/// The object a light is tied to (`tiedToObject`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightTie {
    /// A `HologramLight` by id.
    HoloLight(u64),
    /// A lizard (its graphics' head light).
    Lizard(crate::creature::CreatureId),
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
    pub environmental_light: bool,
    pub tied_to: Option<LightTie>,
    pub require_up_keep: bool,
    pub stay_alive: bool,
    /// Drawn in `Water` rather than `ForegroundLights`; set when it is first drawn in
    /// a room with the `WaterLights` effect.
    pub submersible: bool,
    /// `FlatLight` rather than `LightSource`.
    pub flat: bool,
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
            environmental_light,
            tied_to,
            require_up_keep: false,
            stay_alive: true,
            submersible: false,
            flat: false,
            water_surface_level: 0.0,
        };
        light.set_color(color);
        light
    }

    /// The `color` setter.
    pub fn set_color(&mut self, c: [f32; 3]) {
        self.color_alpha = c.iter().fold(0.0f32, |m, &v| if v > m { v } else { m });
        self.color = if self.color_alpha == 0.0 {
            [1.0; 3]
        } else {
            c.map(|v| v / self.color_alpha)
        };
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

    /// `Update`: the pending sets, then fading out (rad and alpha to 0) once the tied
    /// object is gone (slated or in another room), destroyed when dark; `water_level`
    /// is `Room.FloatWaterLevel(pos)`.
    pub fn update(&mut self, tie_gone: bool, water_level: f32) {
        self.last_pos = self.pos;
        if let Some(p) = self.set_pos.take() {
            self.pos = p;
        }
        self.last_rad = self.rad;
        if let Some(r) = self.set_rad.take() {
            self.rad = r;
        }
        if self.tied_to.is_some() && tie_gone {
            if self.alpha == 0.0 {
                self.slated = true;
            } else {
                self.set_alpha = Some(0.0);
                self.set_rad = Some(0.0);
            }
        }
        self.last_alpha = self.alpha;
        if let Some(a) = self.set_alpha.take() {
            self.alpha = a;
        }
        self.water_surface_level = water_level;
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
    use super::{LightHardSet, LightSource, LightTie};
    use crate::Vec2;

    /// The guide light's colour (1, 0.8, 0.3) is already at most 1: kept, colour alpha 1.
    #[test]
    fn color_over_its_largest_component() {
        let l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
        assert_eq!((l.color, l.color_alpha), ([1.0, 0.8, 0.3], 1.0));
        assert_eq!(l.affected_by_palette_darkness, 1.0);
        let l = LightSource::new(1, 0, Vec2::ZERO, true, [0.0, 0.25, 0.5], None);
        assert_eq!((l.color, l.color_alpha), ([0.0, 0.5, 1.0], 0.5));
        assert_eq!(l.affected_by_palette_darkness, 0.0);
        let l = LightSource::new(1, 0, Vec2::ZERO, false, [0.0; 3], None);
        assert_eq!((l.color, l.color_alpha), ([1.0; 3], 0.0));
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
        l.update(false, f32::MIN);
        assert_eq!((l.rad, l.alpha, l.last_alpha), (200.0, 0.1, 0.1));
        l.update(true, f32::MIN);
        assert!(!l.slated);
        assert_eq!((l.rad, l.alpha, l.last_alpha), (200.0, 0.0, 0.1));
        l.update(true, f32::MIN);
        assert!(l.slated);
        assert_eq!((l.rad, l.last_rad), (0.0, 200.0));
        assert_eq!(l.pos, Vec2::new(5.0, 6.0));
    }
}
