//! rainworld-rs: a region of the installed game (its creatures generated, only the
//! overseers behaving), drawn as the game draws its level graphics (or as debug tiles),
//! with a slugcat body.
//!
//! Usage: `rainworld-rs [ROOM] [--seed N] [--screenshot FILE.ppm] [--debug-view]
//! [--spawn X,Y] [--script STEPS] [--resolution WxH] [--windowed] [--mute]
//! [--process NAME]` (default room
//! `summit`, seed 1; `--screenshot` saves the frame after one second, or once the
//! script has run, and quits; `--debug-view` starts in the debug view; `--spawn` puts
//! the player on tile X,Y; `--script` plays `X,Y,J:TICKS;...` input steps instead of
//! the keyboard; `--resolution` is one of the game's screen sizes, default 1366x768;
//! `--process` picks the process `Initialization` starts: `intro-roll` (the default, as
//! the game boots), `main-menu`, `slugcat-select`, `game` (the default with `--spawn`,
//! `--script` or `--screenshot`)
//! or `font-test`, the M0 screen of labels and a flat illustration, whose UI submit goes
//! on to the game).
//! As in the game the frame is shown full screen, scaled; `--windowed` (implied by
//! `--screenshot`) shows it 1:1 in a window of the resolution. Sound plays on the default
//! output device unless `--mute` (or `--screenshot`), music included; either way the sim
//! makes the sounds' random draws.
//!
//! `rainworld-rs --mcp [--window]` serves the MCP tools of [`mcp`] on stdin/stdout
//! instead (headless; `--window` also shows the driven scene). The game directory comes
//! from `RW_DIR`; it is only read, including its Unity files (sprite atlas, noise
//! texture), with `rw_assets::UnityResources`.
//!
//! `ROOM` is a room name looked up as the game does (`world/<prefix>-rooms`, gates,
//! `levels`), e.g. `summit` or `SU_B04`: a game started directly begins there, in the
//! room's region world (arena levels and rooms outside their world: a single-room
//! world). From the slugcat select menu a new game begins at the campaign's start.
//!
//! Controls: Rewired's default keyboard map: arrows move, Z jumps; R respawn, V level
//! graphic / debug view, G tile grid, F12 screenshot (`screenshot-<frame>.ppm` in the
//! working directory), Esc quit (in the game, until there is a pause menu). Menus take
//! Rewired's default UI keys: arrows, Enter/Space/Z submit, Esc cancel, and the mouse.

mod app;
mod frontend;
mod game;
mod input;
mod mcp;
mod scene;
mod sprites;
mod timestep;
mod visuals;

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use rw_assets::{Room, StreamingAssets, UnityResources};
use rw_menu::ProcessId;
use rw_render::Resolution;

use crate::frontend::{AppContext, Frontend, GameSetup};
use crate::input::Script;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const DEFAULT_ROOM: &str = "summit";
const DEFAULT_SEED: u64 = 1;

/// Parsed command line.
#[derive(Debug, PartialEq, Eq)]
struct Args {
    room: String,
    seed: u64,
    /// Save a frame here after one second, then quit.
    screenshot: Option<PathBuf>,
    /// Start in the debug view instead of the level graphic.
    debug_view: bool,
    /// Player tile instead of a random spawn tile.
    spawn: Option<(i32, i32)>,
    /// Input steps played instead of the keyboard.
    script: Option<Script>,
    /// Serve MCP on stdio instead of playing.
    mcp: bool,
    /// With `mcp`: also show the scene in a window.
    window: bool,
    /// The game's screen size.
    resolution: Resolution,
    /// A window instead of full screen.
    windowed: bool,
    /// No sound output.
    mute: bool,
    /// The process `Initialization` starts.
    start: ProcessId,
}

impl Args {
    /// Parses `[ROOM] [--seed N] [--screenshot FILE] [--debug-view] [--spawn X,Y]
    /// [--script STEPS] [--resolution WxH] [--windowed] [--mute]` or `--mcp [--window]` (program
    /// name already removed).
    fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let mut room = None;
        let mut seed = DEFAULT_SEED;
        let mut screenshot = None;
        let mut debug_view = false;
        let mut spawn = None;
        let mut script = None;
        let mut mcp = false;
        let mut window = false;
        let mut resolution = Resolution::DEFAULT;
        let mut windowed = false;
        let mut mute = false;
        let mut start = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--seed" => {
                    let value = args.next().ok_or("--seed needs a value")?;
                    seed = value
                        .parse()
                        .map_err(|_| format!("invalid seed {value:?}"))?;
                }
                "--screenshot" => {
                    let value = args.next().ok_or("--screenshot needs a path")?;
                    screenshot = Some(PathBuf::from(value));
                }
                "--debug-view" => debug_view = true,
                "--mcp" => mcp = true,
                "--window" => window = true,
                "--windowed" => windowed = true,
                "--mute" => mute = true,
                "--process" => {
                    let value = args.next().ok_or("--process needs a name")?;
                    start = Some(match value.as_str() {
                        "intro-roll" => ProcessId::IntroRoll,
                        "main-menu" => ProcessId::MainMenu,
                        "slugcat-select" => ProcessId::SlugcatSelect,
                        "game" => ProcessId::Game,
                        "font-test" => ProcessId::FontTest,
                        _ => {
                            return Err(format!(
                                "unknown process {value:?} (intro-roll, main-menu, slugcat-select, \
                                 game, font-test)"
                            ));
                        }
                    });
                }
                "--resolution" => {
                    let value = args.next().ok_or("--resolution needs WxH")?;
                    resolution = value.parse()?;
                }
                "--spawn" => {
                    let value = args.next().ok_or("--spawn needs X,Y")?;
                    let (x, y) = value
                        .split_once(',')
                        .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))
                        .ok_or_else(|| format!("invalid spawn tile {value:?}"))?;
                    spawn = Some((x, y));
                }
                "--script" => {
                    let value = args.next().ok_or("--script needs steps")?;
                    script = Some(Script::parse(&value)?);
                }
                "-h" | "--help" => {
                    return Err(
                        "usage: rainworld-rs [ROOM] [--seed N] [--screenshot FILE.ppm] \
                                [--debug-view] [--spawn X,Y] [--script X,Y,J:TICKS;...] \
                                [--resolution WxH] [--windowed] [--mute] \
                                [--process intro-roll|main-menu|slugcat-select|game|font-test]\n       \
                                rainworld-rs --mcp [--window]"
                            .to_owned(),
                    );
                }
                other if room.is_none() && !other.starts_with('-') => {
                    room = Some(other.to_owned());
                }
                other => return Err(format!("unexpected argument {other:?}")),
            }
        }
        // A run that places, drives or photographs the player goes straight to the game.
        let start = start.unwrap_or(
            if spawn.is_some() || script.is_some() || screenshot.is_some() {
                ProcessId::Game
            } else {
                ProcessId::IntroRoll
            },
        );
        Ok(Self {
            room: room.unwrap_or_else(|| DEFAULT_ROOM.to_owned()),
            seed,
            screenshot,
            debug_view,
            spawn,
            script,
            mcp,
            window,
            resolution,
            windowed,
            mute,
            start,
        })
    }
}

/// The installed game's directory (read only).
struct GameDir(PathBuf);

impl GameDir {
    /// `RW_DIR`, or the default Steam location.
    fn from_env() -> Self {
        Self(
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from),
        )
    }

    fn streaming_assets(&self) -> PathBuf {
        self.0.join("RainWorld_Data").join("StreamingAssets")
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(std::env::args().skip(1))?;
    let assets = GameDir::from_env().streaming_assets();
    let assets = StreamingAssets::new(assets);
    let resources = UnityResources::open(assets.data_dir())
        .map_err(|e| format!("reading the game's Unity resources: {e}"))?;
    let resources = Arc::new(resources);
    if args.mcp {
        return mcp::run(assets, resources, args.window);
    }
    let path = assets
        .find_room_file(&args.room.to_lowercase(), ".txt")
        .ok_or_else(|| {
            format!(
                "room {:?} not found under {}",
                args.room,
                assets.path().display()
            )
        })?;
    let room = Room::load(&path).map_err(|e| format!("loading room {}: {e}", path.display()))?;
    log::info!(
        "room {} ({}x{} tiles), seed {}",
        room.name(),
        room.width(),
        room.height(),
        args.seed
    );
    let sprites = sprites::SpriteAssets::load(&resources)
        .inspect_err(|e| log::warn!("no slugcat sprites ({e}); drawing the circle body"))
        .ok()
        .map(Arc::new);
    let table = game::Game::load_sound_table(&assets);
    // A screenshot run stays silent, like `--mute`.
    let audio = if args.mute || args.screenshot.is_some() {
        None
    } else {
        rw_audio::Audio::new(&assets, Arc::clone(&table))
            .inspect_err(|e| log::warn!("no audio output ({e}); playing silently"))
            .ok()
    };
    let title = room.name().to_owned();
    let setup = GameSetup {
        start: game::GameStart::Room {
            name: room.name().to_owned(),
            tile: args.spawn,
        },
        seed: args.seed,
        sound_table: table,
    };
    let mut ctx = AppContext::new(assets, resources, args.resolution, setup, args.start)
        .map_err(|e| format!("loading the fonts: {e}"))?;
    if let Some(audio) = audio {
        ctx.set_audio(audio);
    }
    ctx.script = args.script;
    let options = app::Options {
        // A screenshot run never covers the screen.
        windowed: args.windowed || args.screenshot.is_some(),
        screenshot: args.screenshot,
        debug_view: args.debug_view,
        resolution: args.resolution,
        title,
    };
    app::run(Frontend::new(ctx), sprites, options)
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .init();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            let mut source = e.source();
            while let Some(s) = source {
                log::error!("  caused by: {s}");
                source = s.source();
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(
            Args::parse(std::iter::empty::<&str>()),
            Ok(Args {
                room: "summit".into(),
                seed: 1,
                screenshot: None,
                debug_view: false,
                spawn: None,
                script: None,
                mcp: false,
                window: false,
                resolution: Resolution::DEFAULT,
                windowed: false,
                mute: false,
                start: ProcessId::IntroRoll,
            })
        );
    }

    #[test]
    fn room_and_seed_in_any_order() {
        let want = Ok(Args {
            room: "SU_A22".into(),
            seed: 7,
            screenshot: None,
            debug_view: false,
            spawn: None,
            script: None,
            mcp: false,
            window: false,
            resolution: Resolution::DEFAULT,
            windowed: false,
            mute: false,
            start: ProcessId::IntroRoll,
        });
        assert_eq!(Args::parse(["SU_A22", "--seed", "7"]), want);
        assert_eq!(Args::parse(["--seed", "7", "SU_A22"]), want);
    }

    #[test]
    fn resolution_and_windowed() {
        let args = Args::parse(["--resolution", "1024x768", "--windowed"]).expect("parses");
        assert_eq!(args.resolution, Resolution::new(1024, 768));
        assert!(args.windowed);
        assert!(Args::parse(["--resolution", "1920x1080"]).is_err());
        assert!(Args::parse(["--resolution"]).is_err());
    }

    #[test]
    fn mute() {
        assert!(Args::parse(["--mute"]).expect("parses").mute);
        assert!(!Args::parse(["--windowed"]).expect("parses").mute);
    }

    #[test]
    fn process() {
        let args = Args::parse(["--process", "font-test"]).expect("parses");
        assert_eq!(args.start, ProcessId::FontTest);
        let args = Args::parse(["--process", "intro-roll"]).expect("parses");
        assert_eq!(args.start, ProcessId::IntroRoll);
        let args = Args::parse(["--process", "slugcat-select"]).expect("parses");
        assert_eq!(args.start, ProcessId::SlugcatSelect);
        assert!(Args::parse(["--process", "nope"]).is_err());
        // Placing, scripting or photographing the player starts the game.
        for args in [
            &["--spawn", "1,2"][..],
            &["--script", "0,0,0:1"],
            &["--screenshot", "x"],
        ] {
            assert_eq!(
                Args::parse(args.iter().copied()).unwrap().start,
                ProcessId::Game
            );
        }
        let args = Args::parse(["--spawn", "1,2", "--process", "main-menu"]).unwrap();
        assert_eq!(args.start, ProcessId::MainMenu);
    }

    #[test]
    fn screenshot_path() {
        let args = Args::parse(["--screenshot", "/tmp/x.ppm"]).expect("parses");
        assert_eq!(args.screenshot, Some(PathBuf::from("/tmp/x.ppm")));
        assert!(Args::parse(["--screenshot"]).is_err());
    }

    #[test]
    fn spawn_and_script() {
        let args = Args::parse(["--spawn", "12,-3", "--script", "0,1,0:2;1,0,1:3"]).unwrap();
        assert_eq!(args.spawn, Some((12, -3)));
        let mut script = args.script.expect("script");
        assert_eq!(script.len(), 5);
        let up = rw_sim::Input::new(0, 1, false);
        let jump_right = rw_sim::Input::new(1, 0, true);
        let inputs: Vec<_> = std::iter::from_fn(|| script.next_input()).collect();
        assert_eq!(inputs, [up, up, jump_right, jump_right, jump_right]);
        assert!(Args::parse(["--spawn", "1"]).is_err());
        assert!(Args::parse(["--script", "2,0,0:1"]).is_err());
        assert!(Args::parse(["--script", "0,0:1"]).is_err());
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(Args::parse(["--seed"]).is_err());
        assert!(Args::parse(["--seed", "x"]).is_err());
        assert!(Args::parse(["a", "b"]).is_err());
        assert!(Args::parse(["--frobnicate"]).is_err());
    }
}
