//! What a frame shows: the game with its level graphic and water, drawn the same way in
//! the window and in headless (MCP) screenshots.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rw_graphics::{DrawList, Layer};
use rw_render::{Camera, LevelView, Renderer, RoomLayers, WaterSurface};
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
        if let Some(visuals) = self.visuals.as_ref().filter(|_| level) {
            visuals.screen_size()
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
        // The slugcat's sprites, eyes in the palette's black. Built every frame, as the
        // game runs `DrawSprites` whatever is shown; only the level view draws them.
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
        FrameDraw {
            game: &self.game,
            water: self.water.as_ref(),
            level,
            sprites: sprites.as_ref(),
            layers: view.layers,
            camera,
            alpha,
            time,
        }
        .queue(r);
    }
}

/// One frame's draw calls, in the game's order.
struct FrameDraw<'a> {
    game: &'a Game,
    water: Option<&'a WaterSurface>,
    /// The game's window into the level graphic; `None` in the debug view.
    level: Option<LevelView>,
    /// The slugcat's sprites, drawn only with the level graphic. Without it (the debug
    /// view, or no atlas) the physical body is drawn: chunks, rods and contacts.
    sprites: Option<&'a DrawList>,
    layers: RoomLayers,
    camera: Camera,
    alpha: f32,
    time: f32,
}

impl FrameDraw<'_> {
    fn queue(&self, r: &mut impl Renderer) {
        r.begin_frame(self.camera);
        r.draw_room(RoomLayers {
            tiles: self.layers.tiles && self.level.is_none(),
            ..self.layers
        });
        // Sprites sit at `room - camPos`, the level view's origin.
        let sprites = self.level.as_ref().zip(self.sprites);
        match sprites {
            Some((level, list)) => r.draw_sprites(list, Layer::Midground, level.origin),
            None => {
                if let Some(body) = self.game.body() {
                    r.draw_object(body, self.alpha, self.game.status(), self.time);
                }
            }
        }
        if let Some(level) = self.level {
            r.draw_level(level);
        }
        if let Some((level, list)) = sprites {
            r.draw_sprites(list, Layer::Foreground, level.origin);
        }
        r.draw_shortcut_vessel(self.game.vessel_tile());
        if let Some(water) = self.water {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rw_assets::{PngImage, Room};
    use rw_graphics::Color as SpriteColor;
    use rw_render::{Color, LevelGraphics, Mesh, RenderError, SpriteAtlas};

    use super::*;
    use crate::game::tests::open_room;

    /// Counts what a frame queues.
    #[derive(Default)]
    struct Recorder {
        circles: usize,
        rings: usize,
        lines: usize,
        sprite_calls: usize,
        level: bool,
    }

    impl Renderer for Recorder {
        fn size(&self) -> (u32, u32) {
            (1366, 768)
        }
        fn resize(&mut self, _: u32, _: u32) {}
        fn set_room(&mut self, _: &Room) {}
        fn begin_frame(&mut self, _: Camera) {}
        fn draw_room(&mut self, _: RoomLayers) {}
        fn set_level(&mut self, _: &LevelGraphics<'_>) {}
        fn draw_level(&mut self, _: LevelView) {
            self.level = true;
        }
        fn set_atlas(&mut self, _: &SpriteAtlas, _: &PngImage) {}
        fn draw_sprites(&mut self, _: &DrawList, _: Layer, _: [f32; 2]) {
            self.sprite_calls += 1;
        }
        fn draw_circle(&mut self, _: [f32; 2], _: f32, _: Color) {
            self.circles += 1;
        }
        fn draw_ring(&mut self, _: [f32; 2], _: f32, _: f32, _: Color) {
            self.rings += 1;
        }
        fn draw_line(&mut self, _: [f32; 2], _: [f32; 2], _: f32, _: Color) {
            self.lines += 1;
        }
        fn draw_rect(&mut self, _: [f32; 2], _: [f32; 2], _: Color) {}
        fn draw_mesh(&mut self, _: &Mesh) {}
        fn present(&mut self) -> Result<(), RenderError> {
            Ok(())
        }
        fn capture_next_frame(&mut self, _: PathBuf) {}
    }

    impl Recorder {
        fn record(game: &Game, level: Option<LevelView>, sprites: &DrawList) -> Self {
            let mut r = Self::default();
            FrameDraw {
                game,
                water: None,
                level,
                sprites: Some(sprites),
                layers: RoomLayers::default(),
                camera: Camera::fit([1.0, 1.0], (1366, 768), 0.0),
                alpha: 1.0,
                time: 0.0,
            }
            .queue(&mut r);
            r
        }
    }

    #[test]
    fn debug_view_draws_colliders_and_no_sprites() {
        let mut game = Game::new(open_room(), 1, None);
        let sprites = game.draw_list(1.0, SpriteColor::BLACK).expect("player");
        let chunks = game.body().expect("body").chunks.len();
        let r = Recorder::record(&game, None, &sprites);
        assert_eq!(r.sprite_calls, 0);
        assert_eq!(r.circles, chunks, "one circle per body chunk");
        assert_eq!(r.rings, chunks, "one radius ring per body chunk");
        assert!(r.lines >= 1, "the connection rod");
        assert!(!r.level);
    }

    #[test]
    fn visual_view_draws_sprites_around_the_level() {
        let mut game = Game::new(open_room(), 1, None);
        let sprites = game.draw_list(1.0, SpriteColor::BLACK).expect("player");
        let level = LevelView {
            origin: [-100.0, -50.0],
            size: [1366.0, 768.0],
            rain: 0.0,
        };
        let r = Recorder::record(&game, Some(level), &sprites);
        assert_eq!(r.sprite_calls, 2, "midground and foreground");
        assert_eq!(r.circles, 0);
        assert!(r.level);
    }
}
