//! rainworld-rs: a room from the installed game, drawn as the game draws its level
//! graphic (or as debug tiles), with a slugcat body.
//!
//! Usage: `rainworld-rs [ROOM] [--seed N] [--screenshot FILE.ppm] [--debug-view]`
//! (default room `summit`, seed 1; `--screenshot` saves the frame after one second and
//! quits; `--debug-view` starts in the debug view). The game
//! directory comes from `RW_DIR`; it is only read.
//!
//! `ROOM` is a room name looked up as the game does (`world/<prefix>-rooms`, gates,
//! `levels`), e.g. `summit` or `SU_B04`.
//!
//! Controls: arrows/WASD move, Space/Z jump, R respawn, V level graphic / debug view,
//! G tile grid, F12 screenshot
//! (`screenshot-<tick>.ppm` in the working directory), Esc quit.

mod app;
mod game;
mod input;
mod timestep;
mod visuals;

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use rw_assets::{AssetCache, Room, StreamingAssets};

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
}

impl Args {
    /// Parses `[ROOM] [--seed N] [--screenshot FILE] [--debug-view]` (program name
    /// already removed).
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
                "-h" | "--help" => {
                    return Err(
                        "usage: rainworld-rs [ROOM] [--seed N] [--screenshot FILE.ppm] \
                                [--debug-view]"
                            .to_owned(),
                    );
                }
                other if room.is_none() && !other.starts_with('-') => {
                    room = Some(other.to_owned());
                }
                other => return Err(format!("unexpected argument {other:?}")),
            }
        }
        Ok(Self {
            room: room.unwrap_or_else(|| DEFAULT_ROOM.to_owned()),
            seed,
            screenshot,
            debug_view,
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
    let cache =
        AssetCache::from_env().ok_or("no asset cache directory: set RW_ASSET_CACHE (or HOME)")?;
    let visuals = visuals::Visuals::load(&assets, &cache, &room)
        .inspect_err(|e| log::warn!("no level graphic ({e}); showing the debug view"))
        .ok();
    let options = app::Options {
        screenshot: args.screenshot,
        debug_view: args.debug_view,
    };
    app::run(game::Game::new(room, args.seed), visuals, options)
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
        });
        assert_eq!(Args::parse(["SU_A22", "--seed", "7"]), want);
        assert_eq!(Args::parse(["--seed", "7", "SU_A22"]), want);
    }

    #[test]
    fn screenshot_path() {
        let args = Args::parse(["--screenshot", "/tmp/x.ppm"]).expect("parses");
        assert_eq!(args.screenshot, Some(PathBuf::from("/tmp/x.ppm")));
        assert!(Args::parse(["--screenshot"]).is_err());
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(Args::parse(["--seed"]).is_err());
        assert!(Args::parse(["--seed", "x"]).is_err());
        assert!(Args::parse(["a", "b"]).is_err());
        assert!(Args::parse(["--frobnicate"]).is_err());
    }
}
