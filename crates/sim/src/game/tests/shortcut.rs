use super::*;

/// A pipe from (1, 3) (hole right) down, along and up to (4, 3) (hole left).
fn pipe_room() -> Room {
    let rows = ["######", "#E..E#", "#x##x#", "#xxxx#", "######"];
    let (w, h) = (rows[0].len(), rows.len());
    let mut grid = String::new();
    for x in 0..w {
        for row in rows {
            grid.push_str(match row.as_bytes()[x] {
                b'#' => "1",
                b'E' => "4,3",
                b'x' => "1,3",
                _ => "0",
            });
            grid.push('|');
        }
    }
    let text = format!("T\n{w}*{h}|-1|0\n0*0|0|0\n0,0\n\n\n\n\n\n\n\n{grid}\n");
    Room::parse(&text).expect("test room parses")
}

/// `Room.BlinkShortCut` (61605-61620).
#[test]
fn blink_sends_a_pulse_and_flashes() {
    let mut b = ShortcutsBlinking::new(2);
    b.blink(Some(1), Some(0), 2.0);
    assert_eq!(b.rows(), [[1.0, 0.01, 0.0, -10.0], [0.0, 0.0, 0.0, -40.0]]);
    b.rows[0][1] = 0.5;
    b.blink(None, Some(0), 1.0);
    assert_eq!(
        b.rows()[0],
        [1.0, 0.5, 0.0, -5.0],
        "a running pulse keeps going"
    );
    b.blink(Some(5), None, 1.0);
}

/// The `shortcutsBlinking` block of `Room.Update` (63403-63434): a pulse fades and
/// advances by `1 / Lerp(20, length, 0.3)`, a flash counts up to 0 without drawing,
/// an idle entrance draws once (twice when it starts a flicker) and turns its symbol
/// toward 1 by 0.04, a flicker draws twice and counts down.
#[test]
fn update_steps_rows_with_the_games_draws() {
    let room = pipe_room();
    let length = room.shortcut_map().shortcuts()[0].length as f32;
    let span = 20.0 + (length - 20.0) * 0.3;
    let mut b = ShortcutsBlinking::new(2);
    b.blink(None, Some(0), 1.0);
    b.rows[0][3] = 0.0;
    b.rows[1] = [0.0, 0.0, 0.5, 3.0];
    let mut rng = UnityRandom::new(7);
    let mut expect = rng.clone();
    b.update(&room, &mut rng);
    assert!(
        expect.value() >= 0.01,
        "the idle row starts no flicker on this seed"
    );
    let to = expect.value();
    let t = expect.value() * 0.5;
    assert_eq!(rng.state(), expect.state(), "three draws");
    let r0 = b.rows()[0];
    assert!((r0[0] - (1.0 - 1.0 / span)).abs() < 1e-6);
    assert!((r0[1] - (0.01 + 1.0 / span)).abs() < 1e-6);
    assert!((r0[2] - 0.04).abs() < 1e-6);
    assert_eq!(r0[3], 0.0);
    let r1 = b.rows()[1];
    assert_eq!(r1[3], 2.0);
    assert!((r1[2] - (0.5 + (to - 0.5) * t)).abs() < 1e-6);
    // A flash counts up to 0 with no draw.
    b.rows[0][3] = -2.0;
    b.rows[1][3] = -1.0;
    let before = rng.state();
    b.update(&room, &mut rng);
    assert_eq!(rng.state(), before);
    assert_eq!((b.rows()[0][3], b.rows()[1][3]), (-1.0, 0.0));
}
