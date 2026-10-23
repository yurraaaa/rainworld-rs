//! The data of the pure-visual placed objects `CustomDecal` and `Rainbow`
//! (`RainbowNoFade` shares `Rainbow`'s fields), and the decal PNGs they name.
//!
//! Mirrors: PlacedObject.cs (CustomDecalData, QuadObjectData), Rainbow.cs (RainbowData), RainbowNoFade.cs (RainbowNoFadeData), CustomDecal.cs (LoadFile)

use crate::placed::DataFields;
use crate::{AssetError, PngImage, StreamingAssets};

/// `PlacedObject.CustomDecalData`.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomDecalData {
    /// `QuadObjectData.handles`: the three other corners relative to the object.
    pub handles: [[f32; 2]; 3],
    pub panel: [f32; 2],
    pub from_depth: f32,
    pub to_depth: f32,
    pub noise: f32,
    /// `vertices[corner][0]` and `[1]`; the first column defaults to 0.5, the second to 0.
    pub vertices: [[f32; 2]; 4],
    /// Lower-cased, as `FromString` stores it.
    pub image_name: String,
}

impl CustomDecalData {
    /// `QuadObjectData.FromString` then `CustomDecalData.FromString`:
    /// `h0.x~h0.y~h1.x~h1.y~h2.x~h2.y~panel.x~panel.y~from~to~noise~image[~8 vertices]`.
    /// The vertices are read only from 20 fields on; a 21st field or later makes the
    /// game index past `vertices` and throw, as does a missing or non-numeric field
    /// (`None`).
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        let mut vertices = [[0.5, 0.0]; 4];
        if f.0.len() >= 20 {
            if f.0.len() > 20 {
                return None;
            }
            for i in 12..20 {
                vertices[(i - 12) / 2][(i - 12) % 2] = f.float(i)?;
            }
        }
        Some(Self {
            handles: [
                [f.float(0)?, f.float(1)?],
                [f.float(2)?, f.float(3)?],
                [f.float(4)?, f.float(5)?],
            ],
            panel: [f.float(6)?, f.float(7)?],
            from_depth: f.float(8)?,
            to_depth: f.float(9)?,
            noise: f.float(10)?,
            image_name: f.get(11)?.to_lowercase(),
            vertices,
        })
    }

    /// `AssetManager.ResolveFilePath("Decals/<image>.png")` loaded as the game does
    /// (ARGB32, no mips; the renderer samples it clamped and point filtered).
    ///
    /// # Errors
    /// [`AssetError`] when the file is missing or not a PNG.
    pub fn load_image(assets: &StreamingAssets, image_name: &str) -> Result<PngImage, AssetError> {
        PngImage::load(&assets.resolve_file_path(&format!("Decals/{image_name}.png")))
    }
}

/// `Rainbow.RainbowData` (and `RainbowNoFade.RainbowNoFadeData`).
#[derive(Debug, Clone, PartialEq)]
pub struct RainbowData {
    /// `ResizableObjectData.handlePos`.
    pub handle_pos: [f32; 2],
    pub panel: [f32; 2],
    /// Four corner opacities (default 1), the inner ring's fraction (0.5), `Chance`
    /// (0.15).
    pub fades: [f32; 6],
}

impl RainbowData {
    /// `handle.x~handle.y~panel.x~panel.y~f0,f1,...`; fades missing from the list keep
    /// their defaults, extra ones are ignored. `None` where the game's `float.Parse` or
    /// array index would throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        let mut fades = [1.0, 1.0, 1.0, 1.0, 0.5, 0.15];
        for (fade, text) in fades.iter_mut().zip(f.get(4)?.split(',')) {
            *fade = text.trim().parse().ok()?;
        }
        Some(Self {
            handle_pos: [f.float(0)?, f.float(1)?],
            panel: [f.float(2)?, f.float(3)?],
            fades,
        })
    }

    /// `Chance`.
    #[must_use]
    pub fn chance(&self) -> f32 {
        self.fades[5]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "-65~1425~411~1451~440~-29~424~-377~0.054~1~0~Rust1~0~1~1~1~0~1~0~1";

    #[test]
    fn decal_fields() {
        let d = CustomDecalData::parse(SAMPLE).unwrap();
        assert_eq!(
            d.handles,
            [[-65.0, 1425.0], [411.0, 1451.0], [440.0, -29.0]]
        );
        assert_eq!(d.panel, [424.0, -377.0]);
        assert_eq!((d.from_depth, d.to_depth, d.noise), (0.054, 1.0, 0.0));
        assert_eq!(d.image_name, "rust1");
        // Even fields are column 0, odd ones column 1.
        assert_eq!(d.vertices, [[0.0, 1.0], [1.0, 1.0], [0.0, 1.0], [0.0, 1.0]]);
    }

    #[test]
    fn decal_without_vertices_has_the_defaults() {
        let d = CustomDecalData::parse("0~40~40~40~40~0~0~0~0~1~0~PH").unwrap();
        assert_eq!(d.vertices, [[0.5, 0.0]; 4]);
        assert_eq!(d.image_name, "ph");
    }

    #[test]
    fn decal_the_game_would_throw_on_is_rejected() {
        assert!(CustomDecalData::parse("0~40~40~40~40~0~0~0~0~1~0").is_none());
        assert!(CustomDecalData::parse(&format!("{SAMPLE}~0")).is_none());
        assert!(CustomDecalData::parse(&format!("{SAMPLE}~")).is_none());
        assert!(CustomDecalData::parse(&SAMPLE.replace("~0.054~", "~x~")).is_none());
    }

    #[test]
    fn rainbow_fields() {
        let d = RainbowData::parse("-402~555~-481~190~0,1,1,1").unwrap();
        assert_eq!(d.handle_pos, [-402.0, 555.0]);
        assert_eq!(d.panel, [-481.0, 190.0]);
        assert_eq!(d.fades, [0.0, 1.0, 1.0, 1.0, 0.5, 0.15]);
        assert_eq!(d.chance(), 0.15);
        assert!(RainbowData::parse("1~2~3~4").is_none());
    }
}
