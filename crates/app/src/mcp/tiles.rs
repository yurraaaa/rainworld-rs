//! The `get_tiles` text map.

use std::fmt;

use rw_assets::{Room, Shortcut, Slope, Terrain, Tile};

/// A rectangle of a room's tiles as one character per tile, top row first.
pub struct TileDump<'a> {
    pub room: &'a Room,
    /// Inclusive corners, y up.
    pub min: (i32, i32),
    pub max: (i32, i32),
    /// Tiles holding a player chunk, drawn as `@`.
    pub player: Vec<(i32, i32)>,
}

impl TileDump<'_> {
    pub const LEGEND: &'static str = "legend: # solid, / \\ slopes, - one-way floor, \
        E shortcut entrance, s shortcut path, X room exit, D den, n transport, \
        | vertical pole, = horizontal pole, + both, ~ water (at or below the surface row), \
        . air, ? broken slope, @ player chunk, out-of-room tiles blank";
}

impl fmt::Display for TileDump<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (x0, y0) = self.min;
        let (x1, y1) = self.max;
        writeln!(
            f,
            "room {} ({}x{} tiles, y up, tile = 20 px); x {x0}..={x1}, y {y1} down to {y0}",
            self.room.name(),
            self.room.width(),
            self.room.height()
        )?;
        writeln!(f, "{}", Self::LEGEND)?;
        // Column ruler: the x of every tenth column, and a tick every fifth.
        let mut ruler = String::new();
        let mut x = x0;
        while x <= x1 {
            if x.rem_euclid(10) == 0 {
                let label = x.to_string();
                ruler.push_str(&label);
                x += label.len() as i32;
            } else {
                ruler.push(if x.rem_euclid(5) == 0 { ':' } else { ' ' });
                x += 1;
            }
        }
        writeln!(f, "{:>5} {ruler}", "y\\x")?;
        for y in (y0..=y1).rev() {
            let row: String = (x0..=x1)
                .map(|x| match self.room.get(x, y) {
                    _ if self.player.contains(&(x, y)) => '@',
                    Some(tile) => tile.glyph(self.room, x, y),
                    None => ' ',
                })
                .collect();
            writeln!(f, "{y:>5} {row}")?;
        }
        Ok(())
    }
}

/// One character per tile in [`TileDump`]; see [`TileDump::LEGEND`].
trait TileGlyph {
    fn glyph(self, room: &Room, x: i32, y: i32) -> char;
}

impl TileGlyph for Tile {
    fn glyph(self, room: &Room, x: i32, y: i32) -> char {
        match self.terrain {
            Terrain::Solid => match self.shortcut {
                Shortcut::None => '#',
                Shortcut::Path => 's',
                Shortcut::RoomExit => 'X',
                Shortcut::CreatureDen => 'D',
                Shortcut::NpcTransportation | Shortcut::RegionTransportation => 'n',
            },
            Terrain::Slope => match room.identify_slope(x, y) {
                Some(Slope::UpLeft | Slope::DownRight) => '/',
                Some(Slope::UpRight | Slope::DownLeft) => '\\',
                None => '?',
            },
            Terrain::Floor => '-',
            Terrain::ShortcutEntrance => 'E',
            Terrain::Air => match (self.vertical_pole(), self.horizontal_pole()) {
                (true, true) => '+',
                (true, false) => '|',
                (false, true) => '=',
                (false, false) if self.shortcut != Shortcut::None => 's',
                (false, false) if room.water_level().is_some_and(|w| y <= w) => '~',
                (false, false) => '.',
            },
        }
    }
}
