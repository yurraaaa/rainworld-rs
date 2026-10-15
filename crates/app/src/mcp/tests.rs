//! The tool handlers in-process, on rooms of the installed game (skipped when it is
//! absent). Screenshots skip when no Vulkan adapter is available.

use std::sync::{Arc, Mutex};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rw_assets::{StreamingAssets, UnityResources};

use rw_audio::music::{MusicContext, SongKind};
use rw_menu::ProcessId;

use super::params::{
    InputParam, ListRoomsParams, LoadProcessParams, LoadRoomParams, RespawnParams,
    ScreenshotParams, SetPlayerParams, Step, StepParams, StepProcessParams, StepSequenceParams,
    TilesParams, ViewParam,
};
use super::reply::State;
use super::{Server, Session};
use crate::GameDir;

const RIGHT: InputParam = InputParam {
    x: 1,
    y: 0,
    jump: false,
    map: false,
};

impl Session {
    /// A session on the installed game, or `None` (test skipped) without it.
    fn installed() -> Option<Self> {
        let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
        if !assets.file("levels").is_dir() {
            eprintln!("skipping: no game under {}", assets.path().display());
            return None;
        }
        let resources = UnityResources::open(assets.data_dir()).expect("Unity resources");
        Some(Self::new(assets, std::sync::Arc::new(resources)))
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
    // Summit's left block top (11, 17): a ledge with a wall to the right.
    session
        .load_room(&LoadRoomParams {
            name: "summit".to_owned(),
            seed: Some(1),
            spawn_tile: Some([11, 17]),
        })
        .expect("room loads");
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
        map: false,
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
                    map: false,
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
            map: false,
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

impl Session {
    /// Runs `frames` frames holding `keys`.
    fn frames(&mut self, frames: u32, keys: &[&str]) -> super::reply::ProcessState {
        self.step_process(&StepProcessParams {
            frames: Some(frames),
            keys: Some(keys.iter().map(|k| (*k).to_owned()).collect()),
        })
        .expect("steps")
    }

    /// Steps until `id` runs with the fade from black over.
    fn settle_on(&mut self, id: ProcessId) {
        for _ in 0..400 {
            let s = self.frames(1, &[]);
            if s.process.as_deref() == Some(&format!("{id:?}")) && s.fade_to_black == 0.0 {
                return;
            }
        }
        panic!("{id:?} never settled");
    }

    fn process_player(&mut self) -> super::reply::PlayerState {
        let frontend = self.frontend_mut().expect("process mode");
        let game = frontend.game().expect("game").scene.game();
        super::reply::PlayerState::of(game.live_player().expect("player"))
    }

    fn music(&mut self) -> &rw_audio::music::MusicPlayer {
        let frontend = self.frontend_mut().expect("process mode");
        frontend
            .ctx
            .menu
            .music
            .as_ref()
            .expect("music on by default")
    }
}

/// Boot to play through MCP: the intro roll (default), a key after its first second
/// goes to the main menu; Return on STORY (selected on `Init`) opens slugcat select;
/// START held fills in 36 ticks and starts a new Survivor game at SU_C04 (7, 28), where
/// held arrows now drive the player. The music player follows the processes
/// (`MusicPlayer.UpdateMusicContext`, 414132): `Menu` in the intro roll and main menu,
/// `StoryMode` in slugcat select (the intro song's context is `Menu`, so it plays on),
/// `StartGame`'s `FadeOut(20)` on the intro song, then the game's `FadeOut(2)` on
/// `Menu`-context songs (`FadeOut` keeps the shorter), and the song ends.
#[test]
fn boot_to_play_with_music_contexts() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    let state = s
        .load_process(&LoadProcessParams::default())
        .expect("process loads");
    assert_eq!(state.process.as_deref(), Some("Initialization"));
    s.settle_on(ProcessId::IntroRoll);
    assert_eq!(s.music().music_context, MusicContext::Menu);
    assert!(s.music().song.as_ref().is_some_and(|x| x.is_intro_roll()));
    s.frames(30, &[]);
    let state = s.frames(1, &["Z"]);
    assert_eq!(state.upcoming.as_deref(), Some("MainMenu"));
    s.settle_on(ProcessId::MainMenu);
    let song = s.music().song.as_ref().expect("intro song");
    assert!(
        matches!(song.kind, SongKind::IntroRoll(i) if i.music_track_started && i.fade_out_rain)
    );
    assert_eq!(song.context, MusicContext::Menu);
    s.frames(1, &["Return"]);
    s.frames(2, &[]);
    s.settle_on(ProcessId::SlugcatSelect);
    assert_eq!(s.music().music_context, MusicContext::StoryMode);
    assert!(!s.music().song.as_ref().expect("intro song").fading_out());
    let mut started = None;
    for frame in 0..80 {
        let state = s.frames(1, &["Z"]);
        if state.upcoming.is_some() {
            assert_eq!(state.upcoming.as_deref(), Some("SlideShow"));
            started = Some(frame);
            break;
        }
    }
    assert!(started.is_some(), "START never signalled");
    assert_eq!(
        s.music().song.as_ref().expect("intro song").fade_out_time,
        20.0
    );
    // The switch's fade, then the slideshow sets the fade itself (its first `Empty`
    // scene starts at 0, so the fade is briefly 0 over nothing drawn).
    let state = s.frames(60, &[]);
    assert_eq!(state.process.as_deref(), Some("SlideShow"));
    assert_eq!(state.slideshow.as_ref().and_then(|x| x.current), Some(1));
    let song = s.music().song.as_ref().expect("the slideshow's song");
    assert_eq!(song.name, "RW_Intro_Theme");
    let state = s.frames(1, &["Escape"]);
    assert_eq!(state.upcoming.as_deref(), Some("Game"), "Escape skips");
    assert_eq!(
        s.music().song.as_ref().expect("intro theme").fade_out_time,
        40.0
    );
    s.settle_on(ProcessId::Game);
    assert_eq!(s.music().music_context, MusicContext::StoryMode);
    // SU_C04StartUp holds the player until it drops out of the tunnel.
    let before = s.process_player();
    assert_eq!(before.controller.as_deref(), Some("Null"));
    let mut control = None;
    for frame in 0..400 {
        s.frames(1, &["RightArrow"]);
        if s.process_player().input.x == 1 {
            control = Some(frame);
            break;
        }
    }
    assert!(control.is_some(), "control never returned");
    let after = s.process_player();
    assert!(
        after.chunks[0].pos[0] > before.chunks[0].pos[0] + 5.0,
        "the player moved right ({:?} -> {:?}, {})",
        before.chunks[0].pos,
        after.chunks[0].pos,
        after.body_mode
    );
}

/// START with S (or the map button) held goes straight to the game.
#[test]
fn start_with_s_held_skips_the_slideshow() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.load_process(&LoadProcessParams {
        process: Some(super::params::ProcessParam::SlugcatSelect),
        ..LoadProcessParams::default()
    })
    .expect("process loads");
    s.settle_on(ProcessId::SlugcatSelect);
    for _ in 0..80 {
        let state = s.frames(1, &["Z", "S"]);
        if state.upcoming.is_some() {
            assert_eq!(state.upcoming.as_deref(), Some("Game"));
            return;
        }
    }
    panic!("START never signalled");
}

/// `SU_C04StartUp` (67669) from a new game, tick by tick, holding right throughout:
/// the controller is `Null` from the script's first update (it runs after the player,
/// so the first tick still reads the keys); nothing moves it during the 3 s black delay
/// (`FadeDelayInProgress`); then `HardSetPosition` puts the chunks at the ratcheted
/// middle +9 / -5 px each tick, which moves the middle 2 px right per tick; once the
/// head is below y 550 the push ends, the next update clears the controller and the
/// keys reach the player on the tick after; it stands up when the head drops below 350.
fn su_c04_start_up_ticks(slugcat: &str) {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.new_game(&super::params::NewGameParams {
        slugcat: slugcat.to_owned(),
        seed: Some(1),
    })
    .expect("new game");
    let first = s.run(1, RIGHT);
    assert_eq!(first.player.as_ref().expect("player").input.x, 1);
    let mut last_x = first.head()[0];
    let mut push_ended = None;
    let mut stood = None;
    for tick in 2..=260u32 {
        let st = s.run(1, RIGHT);
        let p = st.player.as_ref().expect("player");
        let su = st.start_up.as_ref().expect("SU_C04StartUp");
        assert_eq!(su.counter, tick as i32);
        // The field is what the next tick reads.
        assert_eq!(st.fade_delay_in_progress, tick < 120, "tick {tick}");
        let middle = (p.chunks[0].pos[0] + p.chunks[1].pos[0]) / 2.0;
        if (121..200).contains(&tick) {
            assert!(su.push_right, "tick {tick}");
            assert_eq!(p.chunks[0].pos[0] - p.chunks[1].pos[0], 14.0, "tick {tick}");
            if tick > 121 {
                assert!(
                    (p.chunks[0].pos[0] - last_x - 2.0).abs() < 1e-3,
                    "tick {tick}"
                );
            }
        }
        if tick <= 120 {
            assert!(middle < 160.0, "no push during the delay (tick {tick})");
        }
        last_x = p.chunks[0].pos[0];
        match push_ended {
            None if !su.push_right => {
                assert!(p.chunks[0].pos[1] < 550.0);
                assert_eq!(p.controller.as_deref(), Some("Null"));
                push_ended = Some(tick);
            }
            None => assert_eq!(p.controller.as_deref(), Some("Null"), "tick {tick}"),
            Some(end) if tick == end + 1 => {
                assert_eq!(p.controller, None);
                assert_eq!(
                    p.input.x, 0,
                    "the controller was cleared after this tick's input"
                );
            }
            Some(end) if tick == end + 2 => assert_eq!(p.input.x, 1),
            Some(_) => {}
        }
        if su.stood_up && stood.is_none() {
            assert!(p.chunks[0].pos[1] < 350.0);
            assert!(p.standing);
            assert_eq!(su.show_controls_counter, 1);
            stood = Some(tick);
        }
        if let Some(t) = stood {
            assert_eq!(su.show_controls_counter, 1 + (tick - t) as i32);
        }
    }
    let end = push_ended.expect("the push ended");
    assert!((200..=210).contains(&end), "push ended at tick {end}");
    assert!(
        stood.is_some_and(|t| t > end),
        "stood up after the push ({stood:?})"
    );
}

#[test]
fn su_c04_start_up_survivor() {
    su_c04_start_up_ticks("White");
}

#[test]
fn su_c04_start_up_monk() {
    su_c04_start_up_ticks("Yellow");
}

/// The Hunter starts elsewhere, behind 5.5 s of black, with no SU_C04 script.
#[test]
fn hunter_start_has_no_start_up_script() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.new_game(&super::params::NewGameParams {
        slugcat: "Red".to_owned(),
        seed: Some(1),
    })
    .expect("new game");
    let st = s.run(219, RIGHT);
    assert!(st.start_up.is_none());
    assert!(st.fade_delay_in_progress);
    let st = s.run(1, RIGHT);
    assert!(!st.fade_delay_in_progress, "5.5 s = 220 ticks");
}

impl Session {
    /// Saves the default screenshot as `<HUD_SHOTS>/<name>.png` when `HUD_SHOTS` is set.
    fn save_shot(&mut self, name: &str) {
        let Some(dir) = std::env::var_os("HUD_SHOTS") else {
            return;
        };
        let path = std::path::Path::new(&dir).join(format!("{name}.png"));
        let params = ScreenshotParams {
            save_path: Some(path.to_string_lossy().into_owned()),
            ..ScreenshotParams::default()
        };
        if let Err(e) = self.screenshot(&params) {
            eprintln!("no screenshot {name}: {e}");
        }
    }
}

/// The HUD from a new Survivor game: hidden at cycle 0 until the map button; the
/// tutorial controls page `SU_C04StartUp` opens once `showControlsCounter > 100` after
/// standing up (67766) and closes once the slugcat is past x 800 (67772), fading in at
/// 1/16 and out at 1/8 a tick (`TutorialControlsPage.Update`).
#[test]
fn new_game_hud_and_controls_page() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.new_game(&super::params::NewGameParams {
        slugcat: "White".to_owned(),
        seed: Some(1),
    })
    .expect("new game");
    let still = InputParam::default();
    s.run(1, still);
    let hud = s.hud_state().expect("the HUD fired up on the first tick");
    assert_eq!(hud.karma_meter.symbol, "smallKarma0");
    assert_eq!(hud.food_meter.max_food, 7);
    assert_eq!(hud.food_meter.survival_limit, 4);
    assert_eq!(hud.rain_meter.circles, 26);
    let mut stood = None;
    let mut opened = None;
    for tick in 2..=400u32 {
        let st = s.run(1, still);
        let su = st.start_up.as_ref().expect("SU_C04StartUp");
        let hud = s.hud_state().expect("hud");
        assert_eq!(hud.karma_meter.fade, 0.0, "cycle 0: hidden (tick {tick})");
        assert_eq!(hud.food_meter.fade, 0.0);
        if su.stood_up && stood.is_none() {
            stood = Some(tick);
        }
        if stood.is_some_and(|t| tick == t + 20) {
            s.save_shot("new_game_start");
        }
        if let Some(page) = &hud.controls_page {
            if opened.is_none() {
                opened = Some(tick);
                assert_eq!(su.show_controls_counter, 101, "tick {tick}");
                // `Room.Update` runs `updateList` from the end: the owner added during
                // it is first updated on the next tick.
                assert_eq!(page.counter, 0);
                assert_eq!(page.black_fade, 0.0);
            }
            if opened.is_some_and(|o| tick == o + 8) {
                s.save_shot("controls_page_opening");
            }
            let n = (tick - opened.unwrap()) as f32;
            assert_eq!(page.black_fade, (n * 0.0625).min(1.0), "tick {tick}");
        } else {
            assert!(
                opened.is_none(),
                "the page stays until the slugcat walks on"
            );
        }
    }
    let stood = stood.expect("stood up");
    let opened = opened.expect("the controls page opened");
    assert_eq!(opened - stood, 100, "stood {stood}, opened {opened}");
    s.save_shot("controls_page");
    let hud = s.hud_state().unwrap();
    assert_eq!(hud.controls_page.as_ref().unwrap().labels.len(), 7);

    // Walk right past x 800: the page fades at 1/8 a tick and is gone 8 ticks later.
    let mut walked = 0;
    loop {
        let st = s.run(1, RIGHT);
        walked += 1;
        assert!(walked < 600, "never reached x 800");
        if st.player.as_ref().unwrap().chunks[0].pos[0] > 800.0 {
            break;
        }
    }
    for i in 1..=8 {
        s.run(1, RIGHT);
        let hud = s.hud_state().unwrap();
        match &hud.controls_page {
            Some(page) => assert!(page.want_to_continue && i < 8, "{i} {page:?}"),
            None => assert_eq!(i, 8, "closed after {i} ticks"),
        }
    }
    assert!(s.hud_state().unwrap().controls_page.is_none());

    // The map button: the slugcat stands still and the meters fade in.
    let map = InputParam {
        map: true,
        ..InputParam::default()
    };
    let st = s.run(40, map);
    assert_eq!(st.player.as_ref().unwrap().input.x, 0);
    let hud = s.hud_state().unwrap();
    assert_eq!(hud.karma_meter.fade, 1.0);
    assert_eq!(hud.food_meter.fade, 1.0);
    assert!(hud.rain_meter.fade > 0.9);
    assert!(hud.map_fade > 0.0);
    s.save_shot("map_button_meters");

    // A tutorial-style message.
    s.hud_message(&super::params::HudMessageParams {
        text: "Hold the map button to see the cycle and food meters".to_owned(),
        wait: None,
        time: None,
        darken: None,
        hide_hud: None,
    })
    .unwrap();
    s.run(40, still);
    let hud = s.hud_state().unwrap();
    assert_eq!(hud.text_prompt.showing, "Message");
    assert_eq!(hud.karma_meter.fade, 0.0, "hideHud");
    s.save_shot("text_prompt");
}

impl Session {
    fn process_scene(&mut self) -> &mut crate::scene::Scene {
        &mut self
            .frontend_mut()
            .expect("process mode")
            .game_mut()
            .expect("game")
            .scene
    }

    /// Boots through the intro roll, main menu and slugcat select to a new Survivor
    /// game, skipping the slideshow with S held.
    fn boot_new_survivor(&mut self) {
        self.load_process(&LoadProcessParams::default())
            .expect("process loads");
        self.settle_on(ProcessId::IntroRoll);
        self.frames(30, &[]);
        self.frames(1, &["Z"]);
        self.settle_on(ProcessId::MainMenu);
        self.frames(1, &["Return"]);
        self.frames(2, &[]);
        self.settle_on(ProcessId::SlugcatSelect);
        for _ in 0..80 {
            if self.frames(1, &["Z", "S"]).upcoming.is_some() {
                break;
            }
        }
        self.settle_on(ProcessId::Game);
    }

    /// Puts the player on the camera room's entrance to `dest` and holds the way in
    /// until it stands in `dest`.
    fn take_exit_to(&mut self, dest: &str) {
        let exits = super::reply::ExitList::of(self.process_scene()).expect("exits");
        let exit = exits
            .exits
            .iter()
            .find(|e| e.destination.as_deref() == Some(dest))
            .unwrap_or_else(|| panic!("no exit to {dest} from {}", exits.room));
        let [x, y] = exit.entrance_tile.expect("entrance");
        let key = match exit.enter_input.expect("way in") {
            [1, _] => "RightArrow",
            [-1, _] => "LeftArrow",
            [_, 1] => "UpArrow",
            _ => "DownArrow",
        };
        let pos = rw_sim::Vec2::from(rw_assets::tile_centre(x, y));
        self.process_scene()
            .game_mut()
            .set_player(pos, rw_sim::Vec2::default());
        for _ in 0..400 {
            self.frames(1, &[key]);
            let game = self.process_scene().game();
            let here = game
                .sim()
                .world()
                .abstract_room(game.sim().player_pos().room);
            if here.is_some_and(|r| r.name == dest)
                && game.live_player().is_some_and(|p| !p.in_shortcut())
            {
                return;
            }
        }
        panic!("never reached {dest}");
    }
}

impl crate::scene::Scene {
    /// The HUD's subregion tracker.
    fn tracker(&self) -> &rw_hud::SubregionTracker {
        let hud = self.hud().expect("hud");
        hud.text_prompt.subregion_tracker.as_ref().expect("tracker")
    }
}

/// `SubregionTracker.Update` (482702) in a new Survivor game started through the menus
/// (`StoryGameInitCondition.New`): the start rooms have no subregion; the first
/// Outskirts room (SU_A38, via SU_A41, A43, A22, A23, A25) is counted from the tick
/// after the HUD first sees it (`lastRegion == num`), and on the tick the counter passes
/// 80 the prompt gets "Outskirts" (no cycle number: that is for `Load` starts only),
/// shown at once since nothing else was. `lastShownRegion` starts at 0, so the first
/// one is shown; SU_A23FirstCycleMessage (which would mark it shown) is only added at
/// cycle 1.
#[test]
fn subregion_name_in_a_new_game() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.boot_new_survivor();
    s.frames(400, &[]);
    for dest in ["SU_A41", "SU_A43", "SU_A22", "SU_A23", "SU_A25"] {
        s.take_exit_to(dest);
        let t = s.process_scene().tracker();
        assert_eq!((t.last_region, t.counter), (0, 0), "{dest}: no subregion");
    }
    s.take_exit_to("SU_A38");
    // The HUD updates before the game: this frame's saw the player still in the pipe.
    assert_eq!(s.process_scene().tracker().last_region, 0);
    for tick in 0..=81 {
        s.frames(1, &[]);
        let scene = s.process_scene();
        let t = scene.tracker().clone();
        let prompt = super::reply::HudState::of(scene.hud().expect("hud")).text_prompt;
        assert_eq!(t.last_region, 1, "tick {tick}");
        if tick < 81 {
            assert_eq!(t.counter, tick, "tick {tick}");
            assert_eq!(t.last_shown_region, 0);
            assert_eq!(prompt.queued, 0, "tick {tick}");
        } else {
            assert_eq!(t.last_shown_region, 1);
            assert!(!t.show_cycle_number);
            assert_eq!(prompt.queued, 1);
            assert_eq!(prompt.showing, "Message");
            assert_eq!(prompt.text, "Outskirts");
        }
    }
    // Shown once: the counter stays at 0 in the same subregion.
    s.frames(300, &[]);
    let t = s.process_scene().tracker();
    assert_eq!((t.counter, t.last_shown_region), (0, 1));
}

/// A Dev start (`load_room`) adopts the first room's subregion silently (482717): SU
/// has one subregion, so no name is ever shown.
#[test]
fn subregion_name_not_shown_on_a_dev_start() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.load("SU_A38", 1);
    s.run(200, InputParam::default());
    let t = s.scene_mut().expect("scene").tracker();
    assert_eq!((t.last_region, t.last_shown_region, t.counter), (1, 1, 0));
    assert_eq!(s.hud_state().expect("hud").text_prompt.queued, 0);
}
