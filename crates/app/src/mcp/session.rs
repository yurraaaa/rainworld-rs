//! The state the MCP tools drive: one loaded room, stepped only on request, and a
//! headless renderer for screenshots. Every tool is a method returning a typed reply or
//! an error message.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rw_assets::{AssetCache, Room, StreamingAssets};
use rw_render::{RoomLayers, WgpuRenderer};
use rw_sim::{Input, Vec2};

use super::params::{
    InputParam, ListRoomsParams, LoadRoomParams, RespawnParams, ScreenshotParams, SetPlayerParams,
    StepParams, StepSequenceParams, TilesParams, ViewParam,
};
use super::reply::{
    ChunkState, ImpactState, PlayerState, RoomInfo, RoomList, Screenshot, State, StepTrace,
};
use super::tiles::TileDump;
use crate::game::Game;
use crate::scene::{Framing, Scene, SceneRenderer, SceneView};
use crate::sprites::SpriteAssets;
use crate::visuals::Visuals;

/// Most ticks one `step` or `step_sequence` call may run (10 minutes of game time).
const MAX_TICKS: u64 = 40 * 60 * 10;
/// Largest screenshot side in pixels.
const MAX_IMAGE_SIDE: f32 = 4096.0;
/// Most tiles one `get_tiles` call returns.
const MAX_TILES: i64 = 40_000;

pub struct Session {
    assets: StreamingAssets,
    cache: AssetCache,
    sprites: Option<Arc<SpriteAssets>>,
    scene: Option<Scene>,
    /// Created on the first screenshot.
    renderer: Option<SceneRenderer<WgpuRenderer>>,
}

impl Session {
    /// A session with no room loaded; loads the slugcat atlas from `cache` if present.
    #[must_use]
    pub fn new(assets: StreamingAssets, cache: AssetCache) -> Self {
        let sprites = SpriteAssets::load(&cache)
            .inspect_err(|e| log::warn!("no slugcat sprites ({e}); drawing the circle body"))
            .ok()
            .map(Arc::new);
        Self {
            assets,
            cache,
            sprites,
            scene: None,
            renderer: None,
        }
    }

    /// The loaded scene, for the mirror window.
    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        self.scene.as_mut()
    }

    pub fn sprites(&self) -> Option<Arc<SpriteAssets>> {
        self.sprites.clone()
    }

    fn scene(&self) -> Result<&Scene, String> {
        self.scene
            .as_ref()
            .ok_or_else(|| "no room loaded; call load_room first".to_owned())
    }

    fn scene_or_err(&mut self) -> Result<&mut Scene, String> {
        self.scene
            .as_mut()
            .ok_or_else(|| "no room loaded; call load_room first".to_owned())
    }

    /// Room names (lower case, as on disk) under `world/<region>-rooms`, `world/gates`
    /// and `levels` that have a first camera image.
    ///
    /// # Errors
    /// When no room directory can be read.
    pub fn list_rooms(&self, params: &ListRoomsParams) -> Result<RoomList, String> {
        let world = self.assets.file("world");
        let mut dirs: Vec<(String, PathBuf)> = std::fs::read_dir(&world)
            .map_err(|e| format!("cannot read {}: {e}", world.display()))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let region = name.strip_suffix("-rooms")?.to_owned();
                Some((region, entry.path()))
            })
            .collect();
        dirs.push(("gates".to_owned(), self.assets.file("world/gates")));
        dirs.push(("arena".to_owned(), self.assets.file("levels")));
        let region = params.region.as_deref().map(str::to_lowercase);
        let prefix = params.prefix.as_deref().unwrap_or("").to_lowercase();
        let mut rooms: Vec<String> = dirs
            .iter()
            .filter(|(r, _)| region.as_ref().is_none_or(|want| want == r))
            .filter_map(|(_, dir)| std::fs::read_dir(dir).ok())
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let file = entry.file_name().into_string().ok()?;
                let name = file.strip_suffix(".txt")?;
                let image = entry.path().with_file_name(format!("{name}_1.png"));
                (name.starts_with(&prefix) && image.is_file()).then(|| name.to_owned())
            })
            .collect();
        rooms.sort();
        rooms.dedup();
        let count = rooms.len();
        let limit = params.limit.unwrap_or(500) as usize;
        rooms.truncate(limit);
        Ok(RoomList {
            count,
            truncated: count > rooms.len(),
            rooms,
        })
    }

    /// Loads a room (as the game looks it up) and spawns the player with `seed`.
    ///
    /// # Errors
    /// When the room is not found or does not parse.
    pub fn load_room(&mut self, params: &LoadRoomParams) -> Result<RoomInfo, String> {
        let name = params.name.to_lowercase();
        let path = self
            .assets
            .find_room_file(&name, ".txt")
            .ok_or_else(|| format!("room {:?} not found (see list_rooms)", params.name))?;
        let room =
            Room::load(&path).map_err(|e| format!("loading room {}: {e}", path.display()))?;
        let visuals = Visuals::load(
            &self.assets,
            &self.cache,
            &room,
            rw_render::Resolution::DEFAULT.size_f32(),
        )
        .inspect_err(|e| log::warn!("no level graphic for {name} ({e})"))
        .ok();
        let spawn = params.spawn_tile.map(|[x, y]| (x, y));
        let game = Game::new(room, params.seed.unwrap_or(1), spawn);
        log::info!("loaded room {name}");
        self.scene = Some(Scene::new(game, visuals));
        self.room_info()
    }

    fn room_info(&self) -> Result<RoomInfo, String> {
        let scene = self.scene()?;
        let room = scene.game().room();
        Ok(RoomInfo {
            name: room.name().to_owned(),
            width: room.width(),
            height: room.height(),
            pixel_size: [room.pixel_width(), room.pixel_height()],
            cameras: room.cameras().to_vec(),
            water_level: room.water_level(),
            water_in_front: room.water_in_front(),
            shortcuts: room.shortcut_map().shortcuts().len(),
            level_graphic: scene.visuals().is_some(),
            state: self.state()?,
        })
    }

    /// Runs `ticks` ticks holding `input`.
    ///
    /// # Errors
    /// Without a room, or on a bad input or tick count.
    pub fn step(&mut self, params: &StepParams) -> Result<State, String> {
        let input = params.input.unwrap_or_default().to_input()?;
        let ticks = u64::from(params.ticks.unwrap_or(1));
        if ticks > MAX_TICKS {
            return Err(format!("at most {MAX_TICKS} ticks per call"));
        }
        let scene = self.scene_or_err()?;
        for _ in 0..ticks {
            scene.tick(input);
        }
        self.state()
    }

    /// Runs the steps in order; with `trace`, also reports where each step ended.
    ///
    /// # Errors
    /// Without a room, or on a bad input or too many ticks in total.
    pub fn step_sequence(
        &mut self,
        params: &StepSequenceParams,
    ) -> Result<(State, Vec<StepTrace>), String> {
        let steps = params
            .steps
            .iter()
            .map(|s| Ok((s.input.unwrap_or_default().to_input()?, s.ticks)))
            .collect::<Result<Vec<_>, String>>()?;
        let total: u64 = steps.iter().map(|(_, t)| u64::from(*t)).sum();
        if total > MAX_TICKS {
            return Err(format!("at most {MAX_TICKS} ticks per call (got {total})"));
        }
        let mut trace = Vec::new();
        for (input, ticks) in steps {
            let scene = self.scene_or_err()?;
            for _ in 0..ticks {
                scene.tick(input);
            }
            if params.trace.unwrap_or(false) {
                trace.push(StepTrace::of(scene));
            }
        }
        Ok((self.state()?, trace))
    }

    /// A fresh player: with `seed`, as if the room was loaded with it; without,
    /// continuing the seeded spawn sequence (the window's R).
    ///
    /// # Errors
    /// Without a room.
    pub fn respawn(&mut self, params: &RespawnParams) -> Result<State, String> {
        let scene = self.scene_or_err()?;
        match params.seed {
            Some(seed) => {
                let room = scene.game().room().clone();
                *scene.game_mut() = Game::new(room, seed, None);
            }
            None => scene.game_mut().respawn(),
        }
        self.state()
    }

    /// Moves the player to a tile centre or a room-pixel position.
    ///
    /// # Errors
    /// Without a room, or unless exactly one of `tile` and `pos` is given.
    pub fn set_player(&mut self, params: &SetPlayerParams) -> Result<State, String> {
        let pos = match (params.tile, params.pos) {
            (Some([x, y]), None) => Vec2::from(rw_assets::tile_centre(x, y)),
            (None, Some(pos)) => Vec2::from(pos),
            _ => return Err("give exactly one of tile or pos".to_owned()),
        };
        let vel = Vec2::from(params.vel.unwrap_or([0.0, 0.0]));
        self.scene_or_err()?.game_mut().set_player(pos, vel);
        self.state()
    }

    /// The current tick and player state.
    ///
    /// # Errors
    /// Without a room.
    pub fn state(&self) -> Result<State, String> {
        let scene = self.scene()?;
        let game = scene.game();
        Ok(State {
            tick: game.ticks(),
            room: game.room().name().to_owned(),
            camera: scene.visuals().map(Visuals::current_camera),
            player: game.live_player().map(PlayerState::of),
        })
    }

    /// Renders the scene offscreen as the window would and encodes it as PNG.
    ///
    /// # Errors
    /// Without a room or a GPU, or when saving fails.
    pub fn screenshot(&mut self, params: &ScreenshotParams) -> Result<Screenshot, String> {
        let level = params.view.unwrap_or_default() == ViewParam::Visuals;
        let scale = params.scale.unwrap_or(1.0);
        if !(scale > 0.0 && scale <= 4.0) {
            return Err("scale must be in (0, 4]".to_owned());
        }
        let scene = self.scene()?;
        let (framing, size) = match params.crop_radius {
            Some(r) if r > 0.0 => {
                let centre = scene.focus();
                let origin = [centre[0] - r, centre[1] - r];
                let size = [2.0 * r, 2.0 * r];
                (Framing::Rect { origin, size }, size)
            }
            Some(_) => return Err("crop_radius must be positive".to_owned()),
            None => (Framing::Auto, scene.auto_size(level)),
        };
        let pixels = size.map(|s| (s * scale).round().clamp(1.0, MAX_IMAGE_SIDE) as u32);
        let view = SceneView {
            level,
            layers: RoomLayers {
                tiles: true,
                grid: params.grid.unwrap_or(false),
            },
            framing,
        };
        if self.renderer.is_none() {
            let renderer = WgpuRenderer::headless((pixels[0], pixels[1]))
                .map_err(|e| format!("no GPU renderer: {e}"))?;
            self.renderer = Some(SceneRenderer::new(renderer, self.sprites.clone()));
        }
        let (Some(scene), Some(target)) = (self.scene.as_mut(), self.renderer.as_mut()) else {
            return Err("no room loaded; call load_room first".to_owned());
        };
        target.renderer.set_frame_size((pixels[0], pixels[1]));
        scene.draw(target, 1.0, view);
        let image = target
            .renderer
            .render_image()
            .map_err(|e| format!("rendering failed: {e}"))?;
        let png = image
            .encode_png()
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        let saved = match &params.save_path {
            Some(path) => {
                let path = Path::new(path);
                std::fs::write(path, &png)
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                Some(path.to_owned())
            }
            None => None,
        };
        Ok(Screenshot {
            png,
            width: image.width(),
            height: image.height(),
            saved,
        })
    }

    /// Text map of the tiles in a rectangle (default: the whole room).
    ///
    /// # Errors
    /// Without a room, or when the rectangle is empty or too large.
    pub fn tiles(&self, params: &TilesParams) -> Result<String, String> {
        let scene = self.scene()?;
        let room = scene.game().room();
        let [x0, y0, x1, y1] = params
            .rect
            .unwrap_or([0, 0, room.width() - 1, room.height() - 1]);
        if x1 < x0 || y1 < y0 {
            return Err("rect is [x0, y0, x1, y1] with x0 <= x1 and y0 <= y1".to_owned());
        }
        let count = (i64::from(x1 - x0) + 1) * (i64::from(y1 - y0) + 1);
        if count > MAX_TILES {
            return Err(format!("{count} tiles requested; at most {MAX_TILES}"));
        }
        let player = scene
            .game()
            .live_player()
            .filter(|p| !p.in_shortcut())
            .map(|p| p.chunks().iter().map(|c| c.pos.tile()).collect())
            .unwrap_or_default();
        Ok(TileDump {
            room,
            min: (x0, y0),
            max: (x1, y1),
            player,
        }
        .to_string())
    }
}

impl InputParam {
    fn to_input(self) -> Result<Input, String> {
        if !(-1..=1).contains(&self.x) || !(-1..=1).contains(&self.y) {
            return Err(format!("input x and y must be -1, 0 or 1 (got {self:?})"));
        }
        Ok(Input::new(self.x, self.y, self.jump))
    }
}

impl Scene {
    /// Where a cropped screenshot centres: the head, the shortcut tile being travelled,
    /// or the room's middle.
    fn focus(&self) -> [f32; 2] {
        let game = self.game();
        match (game.body(), game.vessel_tile()) {
            (Some(body), _) => body.chunks[0].pos.to_array(),
            (None, Some((x, y))) => rw_assets::tile_centre(x, y),
            (None, None) => {
                let room = game.room();
                [room.pixel_width() / 2.0, room.pixel_height() / 2.0]
            }
        }
    }
}

impl PlayerState {
    fn of(player: &rw_sim::Player) -> Self {
        let input = player.input_history()[0];
        Self {
            chunks: player
                .chunks()
                .iter()
                .map(|c| {
                    let contact = c.contact();
                    ChunkState {
                        pos: c.pos.to_array(),
                        vel: c.vel.to_array(),
                        tile: c.pos.tile().into(),
                        contact: [contact.x, contact.y],
                        on_slope: c.on_slope(),
                    }
                })
                .collect(),
            body_mode: format!("{:?}", player.body_mode()),
            animation: format!("{:?}", player.animation()),
            flip_direction: player.flip_direction(),
            standing: player.is_standing(),
            stunned: player.is_stunned(),
            stun_ticks: player.stun_ticks(),
            dead: player.is_dead(),
            in_shortcut: player.in_shortcut(),
            vessel_tile: player.shortcut_vessel_tile().map(Into::into),
            impacts: player
                .body
                .impacts()
                .iter()
                .map(|i| ImpactState {
                    chunk: i.chunk,
                    direction: [i.direction.x, i.direction.y],
                    speed: i.speed,
                    first_contact: i.first_contact,
                })
                .collect(),
            input: InputParam {
                x: input.x,
                y: input.y,
                jump: input.jump,
            },
        }
    }
}

impl StepTrace {
    fn of(scene: &Scene) -> Self {
        let game = scene.game();
        let player = game.live_player();
        Self {
            tick: game.ticks(),
            head: player.map(|p| p.head().pos.to_array()),
            body_mode: player.map(|p| format!("{:?}", p.body_mode())),
            animation: player.map(|p| format!("{:?}", p.animation())),
        }
    }
}
