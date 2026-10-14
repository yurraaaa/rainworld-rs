//! What a frame shows: the game with its level graphic and water, drawn the same way in
//! the window and in headless (MCP) screenshots.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rw_graphics::Layer;
use rw_render::{Camera, Renderer, RoomCamera, RoomLayers, WaterSurface};
use rw_sim::Input;

use crate::game::Game;
use crate::sprites::SpriteAssets;
use crate::timestep::TICK_RATE;
use crate::visuals::Visuals;

/// Screen pixels kept free around the room in the debug view.
pub const MARGIN: f32 = 16.0;

/// Gives every [`Scene`] its own id, so renderers notice a new room.
static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(0);

/// A room being played: the game, the level graphic and the water.
pub struct Scene {
    id: u64,
    game: Game,
    /// The level graphic, when the room's images and palettes loaded.
    visuals: Option<Visuals>,
    /// Flat at the room's water row until the sim provides the wavy surface.
    water: Option<WaterSurface>,
}

/// How a frame is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneView {
    /// The level graphic rather than the debug tiles (when the room has one).
    pub level: bool,
    pub layers: RoomLayers,
    pub framing: Framing,
}

/// Which part of the room a frame shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Framing {
    /// The game's camera window, or the whole room in the debug view.
    Auto,
    /// A rectangle in room pixels.
    Rect { origin: [f32; 2], size: [f32; 2] },
}

/// A renderer and what of a [`Scene`] it already holds.
pub struct SceneRenderer<R> {
    pub renderer: R,
    /// The slugcat's atlas; without it the body is drawn as circles.
    sprites: Option<Arc<SpriteAssets>>,
    /// Scene whose room geometry is uploaded.
    room_of: Option<u64>,
    /// Scene and visuals generation whose level graphic is uploaded.
    level_of: Option<(u64, u32)>,
}

impl Scene {
    #[must_use]
    pub fn new(game: Game, visuals: Option<Visuals>) -> Self {
        Self {
            id: NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed),
            water: WaterSurface::for_room(game.room()),
            game,
            visuals,
        }
    }

    #[must_use]
    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn game_mut(&mut self) -> &mut Game {
        &mut self.game
    }

    #[must_use]
    pub fn visuals(&self) -> Option<&Visuals> {
        self.visuals.as_ref()
    }

    /// One 40 Hz tick of the game, then of the room camera.
    pub fn tick(&mut self, input: Input) {
        self.game.tick(input);
        if let Some(visuals) = &mut self.visuals {
            visuals.tick(self.game.camera_follow().as_ref());
        }
    }

    /// Size in room pixels that [`Framing::Auto`] shows: the game's screen, or the room
    /// plus [`MARGIN`] in the debug view.
    #[must_use]
    pub fn auto_size(&self, level: bool) -> [f32; 2] {
        if level && self.visuals.is_some() {
            RoomCamera::SCREEN_SIZE
        } else {
            let room = self.game.room();
            [
                room.pixel_width() + 2.0 * MARGIN,
                room.pixel_height() + 2.0 * MARGIN,
            ]
        }
    }

    /// Queues one frame on `target` at `alpha` between ticks (call `present` after).
    pub fn draw<R: Renderer>(
        &mut self,
        target: &mut SceneRenderer<R>,
        alpha: f32,
        view: SceneView,
    ) {
        target.sync(self);
        // A frozen body is drawn where it stopped, not between its last two ticks.
        let alpha = if self.game.player_dead() { 1.0 } else { alpha };
        // The slugcat's sprites (also in the debug view), eyes in the palette's black.
        let face = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::black_color);
        let sprites = if target.sprites.is_some() {
            self.game.draw_list(alpha, face)
        } else {
            None
        };

        let r = &mut target.renderer;
        let room = self.game.room();
        let time = (self.game.ticks() as f32 + alpha) / TICK_RATE as f32;
        let level = match &self.visuals {
            Some(visuals) if view.level => Some(visuals.view(alpha, time)),
            _ => None,
        };
        let camera = match (view.framing, &level) {
            (Framing::Rect { origin, size }, _) => Camera::window(origin, size, r.size()),
            (Framing::Auto, Some(level)) => Camera::window(level.origin, level.size, r.size()),
            (Framing::Auto, None) => {
                Camera::fit([room.pixel_width(), room.pixel_height()], r.size(), MARGIN)
            }
        };
        r.begin_frame(camera);
        r.draw_room(RoomLayers {
            tiles: view.layers.tiles && level.is_none(),
            ..view.layers
        });
        // Sprites sit at `room - camPos`; the debug view's stage is the room itself.
        let cam_pos = level.as_ref().map_or([0.0, 0.0], |l| l.origin);
        match &sprites {
            Some(list) => r.draw_sprites(list, Layer::Midground, cam_pos),
            None => {
                if let Some(body) = self.game.body() {
                    r.draw_object(body, alpha, self.game.status(), time);
                }
            }
        }
        if let Some(level) = level {
            r.draw_level(level);
        }
        if let Some(list) = &sprites {
            r.draw_sprites(list, Layer::Foreground, cam_pos);
        }
        r.draw_shortcut_vessel(self.game.vessel_tile());
        if let Some(water) = &self.water {
            r.draw_water(water);
        }
    }
}

impl<R: Renderer> SceneRenderer<R> {
    /// Wraps `renderer`, uploading the sprite atlas if there is one.
    pub fn new(mut renderer: R, sprites: Option<Arc<SpriteAssets>>) -> Self {
        if let Some(sprites) = &sprites {
            sprites.upload(&mut renderer);
        }
        Self {
            renderer,
            sprites,
            room_of: None,
            level_of: None,
        }
    }

    /// Uploads `scene`'s room and shown camera graphic where they changed.
    fn sync(&mut self, scene: &Scene) {
        if self.room_of != Some(scene.id) {
            self.renderer.set_room(scene.game.room());
            self.room_of = Some(scene.id);
        }
        if let Some(visuals) = &scene.visuals {
            let key = (scene.id, visuals.generation());
            if self.level_of != Some(key) {
                visuals.upload(&mut self.renderer);
                self.level_of = Some(key);
            }
        }
    }
}
