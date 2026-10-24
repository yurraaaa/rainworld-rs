//! A room's `RainMask_<room>` texture (`RoomRain.GenerateShelterTex`), one texel per
//! tile: red where the column is open to the sky above that tile, blue on the water
//! surface's row and up to 20 rows above it while no solid tile intervenes (in a room
//! without water: the bottom row under solid tiles). `WaterLight` reads its blue
//! channel. The rain's own outputs (`splashTiles`, `rainReach`) are not made here: the
//! rain is not ported (M8).
//!
//! Mirrors: RoomRain.cs (GetOrGenerateShelterTex, GenerateShelterTex)

use crate::Room;

/// The texture, row 0 at the bottom (Unity's texel order), opaque RGBA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RainMask {
    width: i32,
    height: i32,
    texels: Vec<[u8; 4]>,
}

impl RainMask {
    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const SKY: [u8; 4] = [255, 0, 0, 255];
    const WATER: [u8; 4] = [0, 0, 255, 255];
    const SKY_AND_WATER: [u8; 4] = [255, 0, 255, 255];
    /// How many rows above the surface the water's blue may reach.
    const WATER_REACH: i32 = 20;

    /// `GenerateShelterTex(null, room)`. `HasAnySolid` is `Tile.Solid` here: its
    /// `terrain.ObstructsTile` (a `TerrainHandle` placed object's curve) is not ported,
    /// as no installed base room or level places one (D7).
    #[must_use]
    pub fn generate(room: &Room) -> Self {
        let (width, height) = (room.width(), room.height());
        let mut mask = Self {
            width,
            height,
            texels: vec![Self::BLACK; (width * height) as usize],
        };
        for x in 0..width {
            let mut open = true;
            for y in (0..height).rev() {
                if open && room.tile(x, y).is_solid() {
                    open = false;
                }
                mask.set(x, y, if open { Self::SKY } else { Self::BLACK });
            }
        }
        // The game keeps the room's first mask, made at `Room.Loaded` before the flood's
        // `AddWater` could give it row -10, so that water never reaches it (D47).
        match room.water_level().filter(|&l| l >= 0) {
            Some(level) => {
                for x in 0..width {
                    if room.tile(x, level).is_solid() {
                        continue;
                    }
                    mask.mark(x, level);
                    let mut y = level;
                    while y < height && y < level + Self::WATER_REACH && !room.tile(x, y).is_solid()
                    {
                        mask.mark(x, y + 1);
                        y += 1;
                    }
                }
            }
            None => {
                for x in 0..width {
                    if room.tile(x, 0).is_solid() {
                        mask.mark(x, 0);
                    }
                }
            }
        }
        mask
    }

    /// Width in texels (`TileWidth`).
    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Height in texels (`TileHeight`).
    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// `GetPixel(x, y)` as bytes, the coordinates wrapped (see [`Self::set`]).
    #[must_use]
    pub fn texel(&self, x: i32, y: i32) -> [u8; 4] {
        self.texels[self.index(x, y)]
    }

    /// The texels as RGBA bytes, bottom row first.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.texels.as_flattened()
    }

    /// Blue over what is there, keeping the red of an open column.
    fn mark(&mut self, x: i32, y: i32) {
        let c = if self.texel(x, y)[0] > 127 {
            Self::SKY_AND_WATER
        } else {
            Self::WATER
        };
        self.set(x, y, c);
    }

    /// `SetPixel`: the texture still has the default `Repeat` wrap mode while it is
    /// filled (`Clamp` is set afterwards), so a coordinate past an edge wraps around
    /// (the surface run reaches row `height` in a room whose water is within 20 rows of
    /// the top: it lands on row 0).
    fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        let i = self.index(x, y);
        self.texels[i] = c;
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y.rem_euclid(self.height) * self.width + x.rem_euclid(self.width)) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A room from columns written top row first (`#` solid, `.` air), water at
    /// `water` (-1 for none).
    fn room(columns: &[&str], water: i32) -> Room {
        let h = columns[0].len();
        let tiles: String = columns
            .iter()
            .map(|c| {
                c.chars()
                    .map(|t| if t == '#' { "1|" } else { "0|" })
                    .collect::<String>()
            })
            .collect();
        let text = format!(
            "Test\n{}*{h}|{water}|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{tiles}\n",
            columns.len()
        );
        Room::parse(&text).expect("test room parses")
    }

    /// Column of texels, top row first, as `.` black, `r` sky, `b` water, `m` both.
    fn column(mask: &RainMask, x: i32) -> String {
        (0..mask.height())
            .rev()
            .map(|y| match mask.texel(x, y) {
                RainMask::BLACK => '.',
                RainMask::SKY => 'r',
                RainMask::WATER => 'b',
                RainMask::SKY_AND_WATER => 'm',
                other => panic!("unexpected texel {other:?}"),
            })
            .collect()
    }

    /// Red down to the first solid tile from the top; the surface row (water row 2)
    /// and the run above it turn blue (keeping red), up to and including the first
    /// solid tile above the surface; a column solid at the surface row gets none.
    #[test]
    fn sky_columns_and_the_water_run() {
        let r = room(&["......", "..#...", "...#..", "###..."], 2);
        let mask = RainMask::generate(&r);
        assert_eq!((mask.width(), mask.height()), (4, 6));
        // Open column: all red; the run marks rows 2..=5, then row 6, which wraps to
        // row 0 (row 1 keeps plain red).
        assert_eq!(column(&mask, 0), "mmmmrm");
        // Solid at row 3: red above it; the run marks rows 2 and 3 and stops there.
        assert_eq!(column(&mask, 1), "rrbb..");
        // Solid at the surface row: no water.
        assert_eq!(column(&mask, 2), "rrr...");
        // Roofed from the top: black, the run as in column 1.
        assert_eq!(column(&mask, 3), "..bb..");
    }

    #[test]
    fn without_water_the_bottom_row_marks_solid_columns() {
        let r = room(&["...", "..#", ".##"], -1);
        let mask = RainMask::generate(&r);
        assert_eq!(column(&mask, 0), "rrr");
        assert_eq!(column(&mask, 1), "rrb");
        assert_eq!(column(&mask, 2), "r.b");
    }

    /// Water added by the flood (`AddWater`'s row -10) leaves the loaded room's mask.
    #[test]
    fn added_water_keeps_the_dry_mask() {
        let mut r = room(&["...", "..#", ".##"], -1);
        let dry = RainMask::generate(&r);
        r.add_water();
        assert_eq!(r.water_level(), Some(-10));
        assert_eq!(RainMask::generate(&r), dry);
    }
}
