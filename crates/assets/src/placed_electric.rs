//! The data of the superstructure's electric placed objects: a `ZapCoil`'s and
//! `SuperStructureFuses`' `GridRectObjectData`, and an `SSLightRod`'s `SSLightRodData`.
//!
//! Mirrors: PlacedObject.cs (GridRectObjectData, SSLightRodData)

use crate::placed::DataFields;

/// `PlacedObject.GridRectObjectData`: the second corner of a tile rectangle, relative to
/// the object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridRectData {
    pub handle: [f32; 2],
}

impl GridRectData {
    /// `handlePos.x~handlePos.y~...`; `None` where the game's `float.Parse` would throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        Some(Self {
            handle: [f.float(0)?, f.float(1)?],
        })
    }

    /// `Rect` for an object at `pos`: `IntRect(left, bottom, right, top)` in tiles, the
    /// floor of the corners over 20.
    #[must_use]
    pub fn rect(self, pos: [f32; 2]) -> [i32; 4] {
        let [hx, hy] = self.handle;
        [
            (pos[0].min(pos[0] + hx) / 20.0).floor() as i32,
            (pos[1].min(pos[1] + hy) / 20.0).floor() as i32,
            (pos[0].max(pos[0] + hx) / 20.0).floor() as i32,
            (pos[1].max(pos[1] + hy) / 20.0).floor() as i32,
        ]
    }
}

/// `PlacedObject.SSLightRodData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SsLightRodData {
    pub panel_pos: [f32; 2],
    pub depth: f32,
    /// Degrees clockwise from straight up.
    pub rotation: f32,
    pub length: f32,
    pub brightness: f32,
}

impl SsLightRodData {
    /// `panelPos.x~panelPos.y~depth~rotation~length~brightness~...`; `None` where the
    /// game's `float.Parse` or array access would throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        Some(Self {
            panel_pos: [f.float(0)?, f.float(1)?],
            depth: f.float(2)?,
            rotation: f.float(3)?,
            length: f.float(4)?,
            brightness: f.float(5)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{GridRectData, SsLightRodData};

    /// The rectangle runs between the object and its handle, in either direction, the
    /// floor of each corner over 20.
    #[test]
    fn grid_rect_is_the_floor_of_the_corners() {
        let d = GridRectData::parse("40~-65.5").expect("data");
        assert_eq!(d.rect([105.0, 130.0]), [5, 3, 7, 6]);
        let d = GridRectData::parse("-25~30~1").expect("data");
        assert_eq!(d.rect([105.0, 10.0]), [4, 0, 5, 2]);
        assert!(GridRectData::parse("40").is_none());
        assert!(GridRectData::parse("40~x").is_none());
    }

    /// Every field of a rod, in the game's order.
    #[test]
    fn light_rod_fields() {
        let d = SsLightRodData::parse("-462.3954~-43.96848~0.2347674~180~110.5898~0.5").expect("d");
        assert_eq!(d.panel_pos, [-462.3954, -43.96848]);
        assert_eq!(
            (d.depth, d.rotation, d.length, d.brightness),
            (0.2347674, 180.0, 110.5898, 0.5)
        );
        assert!(SsLightRodData::parse("1~2~3~4~5").is_none());
    }
}
