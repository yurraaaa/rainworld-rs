//! `SlimeMold.CosmeticSlimeMold`'s sprites: one `Circle20` per dot in `Items`, scaled
//! `rad / 10`, coloured between the palette's dark wall tint and the slime-mold orange
//! by the dot's brightness (`ApplyPalette`).
//!
//! Mirrors: SlimeMold.cs (CosmeticSlimeMold.InitiateSprites, DrawSprites, ApplyPalette, AddToContainer; SlimeMoldColorFromPalette)

use rw_assets::RoomPalette;
use rw_sim::{CosmeticSlimeMold, FloatExt, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// The patch's sprites.
pub trait SlimeMoldSprites {
    /// `InitiateSprites`, `ApplyPalette` and `DrawSprites` (the dots never move).
    fn draw(&self, palette: &RoomPalette) -> DrawList;
}

/// The colours `ApplyPalette` reads.
pub(crate) trait SlimeMoldPalette {
    /// `SlimeMoldColorFromPalette`.
    fn slime_mold_color(&self) -> Color;
    /// `Lerp(blackColor, fogColor, 0.15 + 0.1 * fogAmount)`.
    fn slime_mold_dark(&self) -> Color;
}

impl SlimeMoldPalette for RoomPalette {
    fn slime_mold_color(&self) -> Color {
        let hue = 0.07_f32.lerp(0.05, self.darkness);
        let fog = 0.25_f32.lerp(0.35, self.fog_amount) * 0.1_f32.lerp(1.0, self.darkness);
        Color::hsl(hue, 1.0, 0.55).lerp(Color::from(self.fog_color), fog)
    }

    fn slime_mold_dark(&self) -> Color {
        Color::from(self.black_color)
            .lerp(Color::from(self.fog_color), 0.15 + 0.1 * self.fog_amount)
    }
}

impl SlimeMoldSprites for CosmeticSlimeMold {
    fn draw(&self, palette: &RoomPalette) -> DrawList {
        let (dark, orange) = (palette.slime_mold_dark(), palette.slime_mold_color());
        let items = self
            .positions
            .iter()
            .zip(&self.rads)
            .zip(&self.brightnesses)
            .enumerate()
            .map(|(index, ((&pos, &rad), &brightness))| {
                DrawItem::Sprite(Sprite {
                    index,
                    element: "Circle20",
                    pos,
                    rotation: 0.0,
                    scale: Vec2::new(rad / 10.0, rad / 10.0),
                    anchor: Vec2::new(0.5, 0.5),
                    color: dark.lerp(orange, brightness),
                    shader: Shader::default(),
                    layer: Layer::Items,
                })
            })
            .collect();
        DrawList { items }
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::RoomPalette;

    use super::SlimeMoldPalette;
    use crate::draw::Color;

    /// `SlimeMoldColorFromPalette` with no darkness and no fog amount: `HSL2RGB(0.07, 1,
    /// 0.55)` 2.5 % of the way to the fog colour; the dark end 15 % from black to fog.
    #[test]
    fn slime_mold_palette_colours() {
        let white = [1.0, 1.0, 1.0, 1.0];
        let p = RoomPalette {
            sky_color: white,
            fog_color: white,
            black_color: [0.0, 0.0, 0.0, 1.0],
            water_color1: white,
            water_color2: white,
            water_surface_color1: white,
            water_surface_color2: white,
            water_shine_color: white,
            fog_amount: 0.0,
            darkness: 0.0,
            shortcut_colors: [white; 3],
            shortcut_symbol: white,
        };
        let fog = Color::from(white);
        assert_eq!(
            p.slime_mold_color(),
            Color::hsl(0.07, 1.0, 0.55).lerp(fog, 0.025)
        );
        assert_eq!(p.slime_mold_dark(), Color::rgb(0.15, 0.15, 0.15));
    }
}
