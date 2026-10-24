//! The shortcut graphics on the installed game: headless screenshots of the entrance
//! symbols and of the slugcat travelling a pipe, the symbols the tutorial rooms' exits
//! get, and the blink of a pipe the player walks up to.
//!
//! Mirrors: - (port-only: MCP screenshots)

use std::path::Path;

use rw_assets::{Room, ShortcutType, Terrain};

use super::Session;
use super::params::{InputParam, NewGameParams, ScreenshotParams, SetPlayerParams, StepParams};

impl Session {
    fn shot(&mut self, dir: &Path, name: &str, crop: Option<f32>) {
        let params = ScreenshotParams {
            crop_radius: crop,
            scale: crop.map(|_| 2.0),
            save_path: Some(
                dir.join(format!("{name}.png"))
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..ScreenshotParams::default()
        };
        self.screenshot(&params).expect("screenshot");
    }

    fn hold(&mut self, ticks: u32, x: i8, y: i8) {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(InputParam {
                x,
                y,
                ..InputParam::default()
            }),
        })
        .expect("step");
    }

    /// The camera room's shortcut symbols, by shortcut.
    fn entrance_elements(&mut self) -> Vec<Option<&'static str>> {
        // `Draw` makes the sprites once the room is `shortCutsReady`.
        self.screenshot(&ScreenshotParams::default())
            .expect("screenshot");
        let scene = self.scene_mut().expect("scene");
        let count = scene.game().room().shortcut_map().shortcuts().len();
        let g = scene
            .game_mut()
            .shortcut_graphics()
            .expect("shortcut graphics");
        (0..count).map(|k| g.entrance_element(k)).collect()
    }

    /// The camera room's `shortcutsBlinking`.
    fn blinking(&mut self) -> Vec<[f32; 4]> {
        let sim = self.scene_mut().expect("scene").game().sim();
        sim.world()
            .realized(sim.camera_room())
            .and_then(|r| r.shortcuts_blinking.as_ref())
            .expect("blinking")
            .rows()
            .to_vec()
    }

    /// Loads the first of SU's A rooms with a pipe (a `Normal` shortcut) whose entrance
    /// has air two tiles out; returns the room, the shortcut and that tile.
    fn load_pipe_room(&mut self) -> (Room, usize, (i32, i32)) {
        for name in ["SU_A01", "SU_A02", "SU_A03", "SU_A05", "SU_A10", "SU_A14"] {
            self.load(name, 1);
            let room = self.scene_mut().expect("scene").game().room().clone();
            let found = room
                .shortcut_map()
                .shortcuts()
                .iter()
                .enumerate()
                .filter(|(_, sc)| sc.kind == ShortcutType::Normal)
                .find_map(|(k, sc)| {
                    let (hx, hy) = room.shortcut_hole_direction(sc.start.0, sc.start.1);
                    let spot = (sc.start.0 + 2 * hx, sc.start.1 + 2 * hy);
                    (room.tile(spot.0, spot.1).terrain == Terrain::Air).then_some((k, spot))
                });
            if let Some((k, spot)) = found {
                return (room, k, spot);
            }
        }
        panic!("no pipe with air two tiles out");
    }

    fn put_player(&mut self, (x, y): (i32, i32)) {
        self.set_player(&SetPlayerParams {
            tile: Some([x, y]),
            ..SetPlayerParams::default()
        })
        .expect("set player");
    }
}

/// Screenshots written to `SHORTCUT_SHOTS=<dir>`: SU_C04 at the start, then the slugcat
/// going into its room exit at (151, 11) and out in SU_A41 frame by frame (cropped on
/// it), then SU_A41 and SU_A22 at full frame and cropped.
#[test]
#[ignore = "writes screenshots; run with SHORTCUT_SHOTS=<dir>"]
fn shortcut_screenshots() {
    let Some(dir) = std::env::var_os("SHORTCUT_SHOTS") else {
        return;
    };
    let dir = Path::new(&dir);
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.new_game(&NewGameParams {
        slugcat: "White".to_owned(),
        seed: Some(1),
        malnourished: None,
    })
    .expect("new game");
    s.hold(200, 0, 0);
    s.shot(dir, "su_c04", None);
    s.shot(dir, "su_c04_crop", Some(250.0));
    s.set_player(&SetPlayerParams {
        tile: Some([150, 11]),
        ..SetPlayerParams::default()
    })
    .expect("set player");
    s.hold(20, 0, 0);
    s.shot(dir, "su_c04_exit", None);
    s.shot(dir, "su_c04_exit_crop", Some(150.0));
    for frame in 0..40 {
        s.hold(1, 1, 0);
        s.shot(dir, &format!("travel_{frame:02}"), Some(150.0));
    }
    for room in ["SU_A41", "SU_A22"] {
        s.load(room, 1);
        s.hold(40, 0, 0);
        let name = room.to_lowercase();
        s.shot(dir, &name, None);
        s.shot(dir, &format!("{name}_crop"), Some(250.0));
    }
    // A pipe's pulse as the player walks up to it, then the player riding it.
    let (room, k, spot) = s.load_pipe_room();
    let name = room.name().to_lowercase();
    s.put_player(spot);
    for frame in 0..12 {
        s.hold(2, 0, 0);
        s.shot(dir, &format!("pulse_{name}_{frame:02}"), Some(200.0));
    }
    let start = room.shortcut_map().shortcuts()[k].start;
    s.put_player(start);
    let (hx, hy) = room.shortcut_hole_direction(start.0, start.1);
    for frame in 0..30 {
        s.hold(1, -hx as i8, -hy as i8);
        s.shot(dir, &format!("ride_{name}_{frame:02}"), Some(200.0));
    }
}

/// The room exits' symbols on the tutorial route (`GenerateSprites`, the
/// world file's connections and `SHELTER` tag, no `ExitSymbol` object in these rooms'
/// settings): SU_A41's exits show dots; SU_A22's exit node 2, to the shelter SU_S01
/// (`SU_A22 : SU_A43, SU_A23, SU_S01`), the shelter symbol, its others dots; pipes the
/// arrow; anything else nothing.
#[test]
fn tutorial_rooms_entrance_symbols() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    for (room, shelter_node) in [("SU_A41", None), ("SU_A22", Some(2))] {
        s.load(room, 1);
        let got = s.entrance_elements();
        let shortcuts = s
            .scene_mut()
            .expect("scene")
            .game()
            .room()
            .shortcut_map()
            .shortcuts()
            .to_vec();
        assert_eq!(got.len(), shortcuts.len());
        for (sc, element) in shortcuts.iter().zip(&got) {
            let want = match sc.kind {
                ShortcutType::RoomExit if sc.dest_node == shelter_node => Some("ShortcutShelter"),
                ShortcutType::RoomExit => Some("ShortcutDots"),
                ShortcutType::Normal => Some("ShortcutArrow"),
                _ => None,
            };
            assert_eq!(*element, want, "{room} {:?} at {:?}", sc.kind, sc.start);
        }
        eprintln!("{room}: {got:?}");
    }
}

/// `Creature.NewTile` then `Room.BlinkShortCut`: the player two
/// tiles out from a pipe's entrance (the first such pipe in SU's A rooms) blinks it: a
/// pulse from that entrance (`[0] = 1`, `[1] = 0.01`, `[3] = -5`) and a flash at the
/// far end (`[3] = -20`). The room's update runs before the player's, so the tick that
/// blinks ends with these values.
#[test]
fn approaching_a_pipe_blinks_it() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    let (room, k, spot) = s.load_pipe_room();
    let map = room.shortcut_map();
    let far = map.index_of(map.shortcuts()[k].dest).expect("far entrance");
    s.put_player(spot);
    for _ in 0..5 {
        s.hold(1, 0, 0);
        let rows = s.blinking();
        if rows[k][0] == 1.0 {
            assert_eq!(rows[k][1], 0.01);
            assert_eq!(rows[k][3], -5.0);
            assert_eq!(rows[far][3], -20.0);
            return;
        }
    }
    panic!("pipe {k} never blinked");
}
