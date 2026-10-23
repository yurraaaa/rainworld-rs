//! Shared fixtures of the crate's unit tests: test rooms, float asserts and a lizard
//! module stand-in. Expected values in the tests are derived by hand from the formulas in
//! `rainworld-re/notes/slugcat-graphics.md` and `sprite-atlases.md` (derivations next
//! to each assertion), not from another run of this code.
//!
//! Mirrors: - (port-only: test fixtures)

use rw_sim::creature::CreatureId;

use rw_assets::Room;
use rw_sim::Vec2;

use crate::Color;

use crate::effects::cosmetic::{LizardColors, LizardModule};

pub(crate) const EPS: f32 = 1e-4;

/// Builds a room from ASCII rows, top row first: `.` Air, `#` Solid, `/` Slope,
/// `=` Air with a horizontal beam, `|` Air with a vertical beam.
pub(crate) fn room(rows: &[&str]) -> Room {
    let h = rows.len();
    let w = rows[0].len();
    let mut grid = String::new();
    for x in 0..w {
        for row in rows {
            grid.push_str(match row.as_bytes()[x] {
                b'.' => "0",
                b'#' => "1",
                b'/' => "2",
                b'=' => "0,2",
                b'|' => "0,1",
                other => panic!("unknown cell {}", other as char),
            });
            grid.push('|');
        }
    }
    let text =
        format!("Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{grid}\n");
    Room::parse(&text).expect("test room parses")
}

/// 30 x 20 tiles, Solid rows y = 0 and 1: ground top at y = 40.
pub(crate) fn flat_room() -> Room {
    let mut rows = vec![".".repeat(30); 18];
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

pub(crate) fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: expected {expected}, got {actual}"
    );
}

pub(crate) fn assert_vec(actual: Vec2, x: f32, y: f32, what: &str) {
    assert_close(actual.x, x, what);
    assert_close(actual.y, y, what);
}

/// One kept module, `module` 7.
pub(crate) struct OneModule(pub(crate) LizardModule);

impl LizardColors for OneModule {
    fn lizard_module(&self, _id: CreatureId, module: u64, _t: f32) -> Option<LizardModule> {
        (module == 7).then_some(self.0)
    }

    fn black(&self) -> Color {
        Color::WHITE
    }
}

pub(crate) const MODULE: LizardModule = LizardModule {
    head_color: Color::rgb(1.0, 0.0, 0.0),
    black: Color::rgb(0.0, 0.0, 1.0),
    head: Vec2::new(20.0, 10.0),
    rotation: 90.0,
    white_flicker: 0,
};
