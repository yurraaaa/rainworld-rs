//! ASCII test rooms.
//!
//! Mirrors: - (port-only: test fixtures)

use rw_assets::Room;

pub trait FromAscii {
    /// Builds a room from ASCII rows (top row first): `.` Air, `#` Solid, `/` Slope,
    /// `-` Floor, `|` vertical pole, `=` horizontal pole, `E` entrance, `x` Solid
    /// with a shortcut path flag, `R` Solid room exit.
    fn from_ascii(rows: &[&str]) -> Self;
}

impl FromAscii for Room {
    fn from_ascii(rows: &[&str]) -> Self {
        let (w, h) = (rows[0].len(), rows.len());
        let mut grid = String::new();
        for x in 0..w {
            for row in rows {
                grid.push_str(match row.as_bytes()[x] {
                    b'#' => "1",
                    b'/' => "2",
                    b'-' => "3",
                    b'|' => "0,1",
                    b'=' => "0,2",
                    b'E' => "4,3",
                    b'x' => "1,3",
                    b'R' => "1,4",
                    _ => "0",
                });
                grid.push('|');
            }
        }
        let text = format!("T\n{w}*{h}|-1|0\n0*0|0|0\n0,0\n\n\n\n\n\n\n\n{grid}\n");
        let room = Room::parse(&text).expect("test room parses");
        room.map_shortcuts();
        room
    }
}
