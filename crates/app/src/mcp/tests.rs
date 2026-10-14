//! The tool handlers in-process, on rooms of the installed game (skipped when it is
//! absent). Screenshots skip when no Vulkan adapter is available.

use std::sync::{Arc, Mutex};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rw_assets::{AssetCache, StreamingAssets};

use super::params::{
    InputParam, ListRoomsParams, LoadRoomParams, RespawnParams, ScreenshotParams, SetPlayerParams,
    Step, StepParams, StepSequenceParams, TilesParams, ViewParam,
};
use super::reply::State;
use super::{Server, Session};
use crate::GameDir;

const RIGHT: InputParam = InputParam {
    x: 1,
    y: 0,
    jump: false,
};

impl Session {
    /// A session on the installed game, or `None` (test skipped) without it.
    fn installed() -> Option<Self> {
        let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
        if !assets.file("levels").is_dir() {
            eprintln!("skipping: no game under {}", assets.path().display());
            return None;
        }
        let cache = AssetCache::from_env().expect("asset cache directory");
        Some(Self::new(assets, cache))
    }

    fn load(&mut self, name: &str, seed: u64) {
        self.load_room(&LoadRoomParams {
            name: name.to_owned(),
            seed: Some(seed),
            spawn_tile: None,
        })
        .expect("room loads");
    }

    fn run(&mut self, ticks: u32, input: InputParam) -> State {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(input),
        })
        .expect("steps")
    }
}

impl State {
    fn head(&self) -> [f32; 2] {
        self.player.as_ref().expect("player").chunks[0].pos
    }
}

trait FirstText {
    fn text(&self) -> &str;
}

impl FirstText for CallToolResult {
    fn text(&self) -> &str {
        match &self.content[..] {
            [ContentBlock::Text(t), ..] => &t.text,
            other => panic!("expected text content, got {other:?}"),
        }
    }
}

#[test]
fn load_summit_reports_the_room() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    let info = session
        .load_room(&LoadRoomParams {
            name: "SUMMIT".to_owned(),
            seed: None,
            spawn_tile: None,
        })
        .expect("loads");
    assert!(info.name.eq_ignore_ascii_case("summit"), "{}", info.name);
    assert!(info.width > 0 && info.height > 0);
    assert_eq!(
        info.pixel_size,
        [info.width as f32 * 20.0, info.height as f32 * 20.0]
    );
    assert!(!info.cameras.is_empty());
    assert_eq!(info.state.tick, 0);
    let player = info.state.player.expect("spawned");
    assert_eq!(player.chunks.len(), 2);
    assert!(!player.dead && !player.in_shortcut);
}

#[test]
fn stepping_settles_then_walks_left() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    // Summit, seed 1, spawns on a ledge with a wall to the right.
    session.load("summit", 1);
    let settled = session.run(80, InputParam::default());
    assert_eq!(settled.tick, 80);
    let player = settled.player.as_ref().expect("player");
    assert!(
        player.chunks.iter().any(|c| c.contact[1] == -1),
        "on the ground after two seconds: {player:?}"
    );
    let left = InputParam {
        x: -1,
        y: 0,
        jump: false,
    };
    let walked = session.run(20, left);
    assert_eq!(walked.tick, 100);
    let player = walked.player.as_ref().expect("player");
    assert_eq!(player.flip_direction, -1);
    assert_eq!(player.input, left);
    assert!(
        walked.head()[0] < settled.head()[0] - 20.0,
        "moved left: {:?} -> {:?}",
        settled.head(),
        walked.head(),
    );
}

#[test]
fn runs_are_deterministic() {
    let Some(mut a) = Session::installed() else {
        return;
    };
    let mut b = Session::installed().expect("installed");
    let steps = StepSequenceParams {
        steps: vec![
            Step {
                ticks: 30,
                input: None,
            },
            Step {
                ticks: 25,
                input: Some(RIGHT),
            },
            Step {
                ticks: 10,
                input: Some(InputParam {
                    x: 1,
                    y: 0,
                    jump: true,
                }),
            },
        ],
        trace: Some(true),
    };
    a.load("su_a22", 3);
    b.load("su_a22", 3);
    let (state_a, trace_a) = a.step_sequence(&steps).expect("runs");
    let (state_b, trace_b) = b.step_sequence(&steps).expect("runs");
    assert_eq!(trace_a.len(), 3);
    assert_eq!(trace_a.last().map(|t| t.tick), Some(65));
    let json = serde_json::to_string(&(state_a, trace_a)).expect("json");
    assert_eq!(
        json,
        serde_json::to_string(&(state_b, trace_b)).expect("json")
    );
}

#[test]
fn set_player_and_respawn() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("summit", 1);
    let spawn = session.state().expect("state").head();
    let state = session
        .set_player(&SetPlayerParams {
            tile: None,
            pos: Some([spawn[0] + 10.0, spawn[1] + 40.0]),
            vel: Some([3.0, 0.0]),
        })
        .expect("moves");
    let player = state.player.expect("player");
    assert_eq!(player.chunks[0].pos, [spawn[0] + 10.0, spawn[1] + 41.0]);
    assert_eq!(player.chunks[1].vel, [3.0, 0.0]);

    let tile = player.chunks[1].tile;
    let state = session
        .set_player(&SetPlayerParams {
            tile: Some(tile),
            ..Default::default()
        })
        .expect("moves");
    let centre = rw_assets::tile_centre(tile[0], tile[1]);
    assert_eq!(state.head(), [centre[0], centre[1] + 1.0]);

    assert!(session.set_player(&SetPlayerParams::default()).is_err());
    let respawned = session
        .respawn(&RespawnParams { seed: Some(1) })
        .expect("respawns");
    assert_eq!(respawned.tick, 0);
    assert_eq!(respawned.head(), spawn);
}

#[test]
fn tiles_dump_a_rectangle_with_the_player() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("summit", 1);
    let head = session
        .state()
        .expect("state")
        .player
        .expect("player")
        .chunks[0]
        .tile;
    let rect = [head[0] - 5, head[1] - 3, head[0] + 5, head[1] + 3];
    let text = session
        .tiles(&TilesParams { rect: Some(rect) })
        .expect("tiles");
    let lines: Vec<&str> = text.lines().collect();
    // Header, legend, ruler, then 7 rows of 11 tiles under a 5-wide y label.
    assert_eq!(lines.len(), 3 + 7, "{text}");
    assert!(lines[1].starts_with("legend:"));
    assert!(
        lines[3..].iter().all(|l| l.chars().count() == 6 + 11),
        "{text}"
    );
    assert!(
        lines[3]
            .trim_start()
            .starts_with(&(head[1] + 3).to_string())
    );
    assert!(text.contains('@'), "{text}");
    let row = lines[3 + 3];
    assert_eq!(
        row.chars().nth(6 + 5),
        Some('@'),
        "head tile in the middle: {text}"
    );

    let whole = session.tiles(&TilesParams::default()).expect("whole room");
    assert!(whole.contains('#'));
    assert!(
        session
            .tiles(&TilesParams {
                rect: Some([5, 5, 4, 5])
            })
            .is_err()
    );
}

#[test]
fn list_rooms_filters() {
    let Some(session) = Session::installed() else {
        return;
    };
    let su = session
        .list_rooms(&ListRoomsParams {
            prefix: Some("SU_A".to_owned()),
            ..Default::default()
        })
        .expect("lists");
    assert!(su.count > 10, "{su:?}");
    assert!(su.rooms.iter().all(|r| r.starts_with("su_a")));
    let arena = session
        .list_rooms(&ListRoomsParams {
            region: Some("arena".to_owned()),
            limit: Some(3),
            ..Default::default()
        })
        .expect("lists");
    assert_eq!(arena.rooms.len(), 3);
    assert!(arena.truncated);
}

#[test]
fn tool_errors_are_results_not_panics() {
    let Some(session) = Session::installed() else {
        return;
    };
    let server = Server::new(Arc::new(Mutex::new(session)));
    let result = server.get_state().expect_err("no room yet");
    assert!(result.contains("load_room"), "{result}");
    let missing = server.load_room(Parameters(LoadRoomParams {
        name: "no_such_room".to_owned(),
        seed: None,
        spawn_tile: None,
    }));
    assert!(missing.is_err());
    server
        .load_room(Parameters(LoadRoomParams {
            name: "summit".to_owned(),
            seed: Some(1),
            spawn_tile: None,
        }))
        .expect("loads");
    let bad_input = server.step(Parameters(StepParams {
        ticks: Some(1),
        input: Some(InputParam {
            x: 2,
            y: 0,
            jump: false,
        }),
    }));
    assert!(bad_input.is_err());
    let ok = server
        .step(Parameters(StepParams::default()))
        .expect("steps");
    let state: serde_json::Value = serde_json::from_str(ok.text()).expect("json");
    assert_eq!(state["tick"], 1);
}

#[test]
fn screenshot_renders_offscreen() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("summit", 1);
    session.run(40, InputParam::default());
    let params = ScreenshotParams {
        view: Some(ViewParam::Debug),
        grid: Some(true),
        crop_radius: Some(100.0),
        scale: Some(2.0),
        save_path: None,
    };
    let shot = match session.screenshot(&params) {
        Ok(shot) => shot,
        Err(e) if e.starts_with("no GPU renderer") => {
            eprintln!("skipping: {e}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    assert_eq!((shot.width, shot.height), (400, 400));
    assert!(shot.png.starts_with(b"\x89PNG"));

    let full = session
        .screenshot(&ScreenshotParams::default())
        .expect("visuals view");
    let room = session.state().expect("state");
    assert!(full.width > 0 && full.height > 0, "{room:?}");
    let decoder = png::Decoder::new(std::io::Cursor::new(&full.png));
    let mut reader = decoder.read_info().expect("decodes");
    let mut pixels = vec![0; reader.output_buffer_size().expect("size")];
    reader.next_frame(&mut pixels).expect("frame");
    let rgb = pixels.as_chunks::<3>().0;
    assert!(
        rgb.iter().any(|px| *px != rgb[0]),
        "the frame is not one flat colour"
    );
}
