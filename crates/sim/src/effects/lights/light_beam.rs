//! `LightBeam`: a placed sunbeam, a quad of light in its room's update list. Its
//! `Update` only reads the camera's picture for a beam coloured by the environment;
//! the mesh and the per-frame alpha are `rw-graphics`'. Blinking and night beams (no
//! base beam has either) come with the DLCs that use them (D24). No draws.
//!
//! Mirrors: LightBeam.cs (LightBeam, Update, GetIdealGridDiv's quad), Room.cs (Loaded: LightBeam, SetLightBeamBlink)

use rw_assets::LightBeamData;

use crate::Vec2;

/// `LightBeam` (its placed data and what `Update` and `ApplyPalette` keep).
#[derive(Debug, Clone, PartialEq)]
pub struct LightBeam {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    pub data: LightBeamData,
    /// `quad`: the placed position and the three handles added to it.
    pub quad: [Vec2; 4],
    /// `environmentColor` (clear black until `Update` reads the camera).
    pub environment_color: [f32; 4],
    /// `paletteLitColor` (clear black until `ApplyPalette`).
    pub palette_lit_color: [f32; 4],
}

impl LightBeam {
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, data: LightBeamData) -> Self {
        Self {
            update_order,
            data,
            quad: Self::quad_of(pos, &data.quad.handles),
            environment_color: [0.0; 4],
            palette_lit_color: [0.0; 4],
        }
    }

    /// `quad[0]` is the position, `quad[i + 1]` the position plus handle `i`.
    #[must_use]
    pub fn quad_of(pos: Vec2, handles: &[[f32; 2]; 3]) -> [Vec2; 4] {
        [
            pos,
            pos + Vec2::from(handles[0]),
            pos + Vec2::from(handles[1]),
            pos + Vec2::from(handles[2]),
        ]
    }

    /// `Update`'s read of camera 0: with `colorB` above 0 and the camera showing the
    /// room, `environmentColor = PixelColorAtCoordinate(quad[1])`.
    #[must_use]
    pub fn wants_environment_color(&self) -> bool {
        self.data.color_b > 0.0
    }

    /// `Update` given the camera's colour at `quad[1]` (when it shows the room).
    pub fn update(&mut self, environment: Option<[f32; 4]>) {
        if self.wants_environment_color()
            && let Some(c) = environment
        {
            self.environment_color = c;
        }
    }

    /// `ApplyPalette`: `palette.texture.GetPixel(8, 5)`.
    pub fn apply_palette(&mut self, c: &crate::CameraImage) {
        self.palette_lit_color = c.palette_texel(8, 5);
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::LightBeamData;

    use super::LightBeam;
    use crate::Vec2;

    fn data(color_b: f32) -> LightBeamData {
        LightBeamData::parse(&format!("0~40~40~40~40~0~0~0~0.4~0~{color_b}~1")).expect("data")
    }

    /// The constructor's quad: the position, then the position plus each handle.
    #[test]
    fn quad_is_position_plus_handles() {
        let b = LightBeam::new(3, Vec2::new(100.0, 50.0), data(0.0));
        assert_eq!(
            b.quad,
            [
                Vec2::new(100.0, 50.0),
                Vec2::new(100.0, 90.0),
                Vec2::new(140.0, 90.0),
                Vec2::new(140.0, 50.0)
            ]
        );
    }

    /// `Update` reads the environment colour only for `colorB > 0`.
    #[test]
    fn environment_color_only_with_color_b() {
        let c = [0.2, 0.4, 0.6, 1.0];
        let mut b = LightBeam::new(0, Vec2::ZERO, data(0.0));
        b.update(Some(c));
        assert_eq!(b.environment_color, [0.0; 4]);
        let mut b = LightBeam::new(0, Vec2::ZERO, data(0.25));
        b.update(None);
        assert_eq!(b.environment_color, [0.0; 4]);
        b.update(Some(c));
        assert_eq!(b.environment_color, c);
    }
}
