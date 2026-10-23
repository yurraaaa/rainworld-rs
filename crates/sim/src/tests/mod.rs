//! Test rooms shared by the crate's unit tests: ASCII rows parsed as a room file.
//!
//! Mirrors: - (port-only: test fixtures)

use rw_assets::Room;

/// Builds a room from ASCII rows, top row first.
/// `.` Air, `#` Solid, `/` Slope, `-` Floor, `=` Air with a horizontal beam, `|` Air
/// with a vertical pole, `+` Air with both,
/// `E` shortcut entrance, `x` Solid shortcut path, `X` Solid room exit, `D` Solid den,
/// `T` Solid region transport.
pub(crate) fn room(rows: &[&str]) -> Room {
    room_with_water(rows, -1)
}

/// [`room`] with the file's water level (tile row; -1 for none).
pub(crate) fn room_with_water(rows: &[&str], water_level: i32) -> Room {
    room_file(rows, water_level, "")
}

/// [`room`] with line 5's items (`type,x,y|...`).
pub(crate) fn room_with_items(rows: &[&str], items: &str) -> Room {
    room_file(rows, -1, items)
}

fn room_file(rows: &[&str], water_level: i32, items: &str) -> Room {
    room_file_named("Test", rows, water_level, items)
}

/// [`room_with_water`] under another room name.
pub(crate) fn named_room(name: &str, rows: &[&str]) -> Room {
    room_file_named(name, rows, -1, "")
}

fn room_file_named(name: &str, rows: &[&str], water_level: i32, items: &str) -> Room {
    let h = rows.len();
    let w = rows[0].len();
    let mut grid = String::new();
    for x in 0..w {
        for row in rows {
            let cell = match row.as_bytes()[x] {
                b'.' => "0",
                b'w' => "0,8",
                b'#' => "1",
                b'/' => "2",
                b'-' => "3",
                b'=' => "0,2",
                b'|' => "0,1",
                b'+' => "0,1,2",
                b'E' => "4,3",
                b'x' => "1,3",
                b'X' => "1,3,4",
                b'D' => "1,3,5",
                b'T' => "1,3,12",
                other => panic!("unknown cell {}", other as char),
            };
            grid.push_str(cell);
            grid.push('|');
        }
    }
    let text = format!(
        "{name}\n{w}*{h}|{water_level}|0\n1*1|0|0\n0,0\nBorder: Solid\n{items}\n\n\n\n17|0|0|\nblob\n{grid}\n"
    );
    Room::parse(&text).expect("test room parses")
}
