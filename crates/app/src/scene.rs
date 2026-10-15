//! What a frame shows: the game with its level graphic and water and the HUD, drawn the
//! same way in the window and in headless (MCP) screenshots.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rw_assets::{StreamingAssets, UnityResources};
use rw_graphics::{DrawList, Layer};
use rw_hud::{Hud, IllustrationSource, PageText};
use rw_menu::{AtlasManager, Font, MenuShared};
use rw_render::{
    Camera, LevelView, Renderer, RoomCamera, RoomLayers, StageBatch, WaterFrame, WaterSprites,
    WaterSurface,
};
use rw_sim::{HudEvent, Input, SoundEvent};

use crate::game::Game;
use crate::sprites::SpriteAssets;
use crate::timestep::TICK_RATE;
use crate::visuals::Visuals;

/// Screen pixels kept free around the room in the debug view.
pub const MARGIN: f32 = 16.0;

/// Gives every [`Scene`] its own id, so renderers notice a new room.
static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(0);

/// The game being played as the room camera shows it: the camera's room, its level
/// graphic and water.
pub struct Scene {
    id: u64,
    game: Game,
    /// Where level graphics are loaded from; `None` draws the debug view only.
    source: Option<VisualsSource>,
    /// The room the camera shows (global index).
    camera_room: i32,
    /// Camera 0's position in its room (`RoomCamera`), run headless too: the
    /// overseers read it.
    camera: RoomCamera,
    /// The camera's room's level graphic, when its images and palettes loaded.
    visuals: Option<Visuals>,
    /// The water's sprites (`Water.InitiateSprites`), made on the first level frame.
    water_sprites: Option<WaterSprites>,
    /// The camera moved to another room since the last [`Scene::take_entered_room`].
    entered_room: bool,
    /// Camera 0's HUD, once fired up.
    hud: Option<Hud>,
}

/// What a room's level graphic is loaded from.
#[derive(Clone)]
pub struct VisualsSource {
    pub assets: StreamingAssets,
    pub resources: Arc<UnityResources>,
    /// The room camera's window (`sSize`).
    pub screen: [f32; 2],
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
    /// Scene and camera room whose room geometry is uploaded.
    room_of: Option<(u64, i32)>,
    /// Scene, camera room and visuals generation whose level graphic is uploaded.
    level_of: Option<(u64, i32, u32)>,
}

impl Scene {
    /// Shows `game`'s camera room, with level graphics from `source` when given.
    #[must_use]
    pub fn new(game: Game, source: Option<VisualsSource>) -> Self {
        let mut scene = Self {
            id: NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed),
            camera_room: game.sim().camera_room(),
            camera: RoomCamera::new(game.room().cameras(), Self::screen(source.as_ref())),
            game,
            source,
            visuals: None,
            water_sprites: None,
            entered_room: false,
            hud: None,
        };
        scene.load_visuals();
        scene.apply_camera_move();
        scene.entered_room = false;
        scene
    }

    /// The room camera's window (`sSize`).
    fn screen(source: Option<&VisualsSource>) -> [f32; 2] {
        source.map_or(RoomCamera::SCREEN_SIZE, |s| s.screen)
    }

    /// Loads the camera room's level graphic.
    fn load_visuals(&mut self) {
        self.water_sprites = None;
        self.visuals = self.source.as_ref().and_then(|source| {
            let room = self.game.room();
            let timeline = self.game.sim().campaign().timeline.value();
            Visuals::load(&source.assets, &source.resources, room, timeline)
                .inspect_err(|e| {
                    log::warn!(
                        "no level graphic for {} ({e}); showing the debug view",
                        room.name()
                    );
                })
                .ok()
        });
        let earth = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::earth_color);
        self.game.set_palette_earth(earth);
    }

    /// `RoomCamera.MoveCamera`: on another room, its level graphic is loaded (at once:
    /// D14); the camera cuts to the camera asked for.
    fn apply_camera_move(&mut self) {
        let Some(mv) = self.game.take_camera_move() else {
            return;
        };
        if mv.room != self.camera_room {
            self.camera_room = mv.room;
            self.entered_room = true;
            log::info!("camera in {}", self.game.room().name());
            self.camera = RoomCamera::new(
                self.game.room().cameras(),
                Self::screen(self.source.as_ref()),
            );
            self.load_visuals();
        }
        let i = mv
            .camera
            .min(self.game.room().cameras().len().saturating_sub(1));
        self.camera.cut_to(i);
        if let Some(visuals) = &mut self.visuals {
            visuals.show(i);
        }
    }

    /// Whether the camera moved to another room since the last call.
    pub fn take_entered_room(&mut self) -> bool {
        std::mem::take(&mut self.entered_room)
    }

    #[must_use]
    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn game_mut(&mut self) -> &mut Game {
        &mut self.game
    }

    /// Camera 0.
    #[must_use]
    pub fn camera(&self) -> &RoomCamera {
        &self.camera
    }

    #[must_use]
    pub fn visuals(&self) -> Option<&Visuals> {
        self.visuals.as_ref()
    }

    #[must_use]
    pub fn hud(&self) -> Option<&Hud> {
        self.hud.as_ref()
    }

    pub fn hud_mut(&mut self) -> Option<&mut Hud> {
        self.hud.as_mut()
    }

    /// One 40 Hz tick: the HUD (`RoomCamera.Update` comes before the rooms in
    /// `RainWorldGame.Update`), the game, the controls page (a room object), then the
    /// room camera's move; returns the sound events since the last tick. Without
    /// `shared` (fonts, atlases, strings) there is no HUD.
    pub fn tick(&mut self, input: Input, shared: Option<&mut MenuShared>) -> Vec<SoundEvent> {
        self.game
            .sim_mut()
            .set_camera_view(self.camera.current(), self.camera.most_likely_next());
        let Some(shared) = shared else {
            self.game.tick(input);
            return self.after_game_tick();
        };
        self.update_hud(shared);
        let script = self.game.start_up_script();
        let showed_controls = script.is_some_and(|s| s.showed_controls());
        let continue_page = script.is_some_and(|s| s.controls_page_continue());
        self.game.sim_mut().set_hud_present(self.hud.is_some());
        self.game.tick(input);
        self.apply_hud_events(shared);
        if let Some(hud) = &mut self.hud {
            hud.update_controls_page(continue_page);
            if !showed_controls
                && self
                    .game
                    .start_up_script()
                    .is_some_and(|s| s.showed_controls())
            {
                Self::open_controls_page(hud, shared);
            }
        }
        self.after_game_tick()
    }

    /// `RoomCamera.Update`'s HUD step: update it, or fire it up once the player is
    /// realized.
    fn update_hud(&mut self, shared: &MenuShared) {
        match &mut self.hud {
            Some(hud) => {
                let owner = self.game.hud_owner();
                let map_button = self.game.map_button();
                let rain = self.game.rain_view();
                let (sounds, rng) = self.game.sim_mut().camera_io();
                hud.update(&owner, map_button, &rain, &shared.translator, sounds, rng);
            }
            None if self.game.hud_due() => {
                let setup = self.game.hud_setup(shared.screen_size);
                self.hud = Some(Hud::new(&setup, &shared.atlases));
                log::info!("HUD fired up");
            }
            None => {}
        }
    }

    /// The guide's `TutorialText`s (translated, darkening the screen) and its rain
    /// meter resets.
    fn apply_hud_events(&mut self, shared: &MenuShared) {
        let events = self.game.take_hud_events();
        let Some(hud) = &mut self.hud else {
            return;
        };
        for event in events {
            match event {
                HudEvent::Message(m) => {
                    let text = shared.translator.translate(m.text);
                    hud.add_message(text, m.wait, m.time, true, m.hide_hud);
                }
                HudEvent::HalfTimeBlinkReset => hud.rain_meter.half_time_blink = 0,
            }
        }
    }

    /// `new TutorialControlsPageOwner(game)`.
    fn open_controls_page(hud: &mut Hud, shared: &mut MenuShared) {
        let font = shared.atlases.font(Font::FONT).cloned();
        let text = PageText {
            font: font.as_ref(),
            translator: &shared.translator,
        };
        let mut source = IllustrationSource {
            assets: &shared.assets,
            atlases: &mut shared.atlases,
        };
        hud.open_controls_page(&mut source, &text);
        log::info!("controls page shown");
    }

    /// Sets the HUD's sprites for `alpha` between ticks and appends its quads.
    pub fn draw_hud(&mut self, atlases: &AtlasManager, batch: &mut StageBatch, alpha: f32) {
        if let Some(hud) = &mut self.hud {
            hud.draw(alpha);
            hud.push_quads(atlases, batch);
        }
    }

    /// The room camera after the game's tick; returns the sound events.
    fn after_game_tick(&mut self) -> Vec<SoundEvent> {
        self.apply_camera_move();
        if self.camera.update(self.game.camera_follow().as_ref()) {
            log::info!("camera {}", self.camera.current() + 1);
            if let Some(visuals) = &mut self.visuals {
                visuals.show(self.camera.current());
            }
        }
        self.game.take_sound_events()
    }

    /// Size in room pixels that [`Framing::Auto`] shows: the game's screen, or the room
    /// plus [`MARGIN`] in the debug view.
    #[must_use]
    pub fn auto_size(&self, level: bool) -> [f32; 2] {
        if self.visuals.is_some() && level {
            self.camera.screen_size()
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
        let earth = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::earth_color);
        let overseers = if target.sprites.is_some() {
            self.game.overseer_draw_lists(alpha, earth)
        } else {
            Vec::new()
        };

        let r = &mut target.renderer;
        let room = self.game.room();
        let time = (self.game.ticks() as f32 + alpha) / TICK_RATE as f32;
        let level = match &self.visuals {
            Some(_) if view.level => Some(Visuals::view(&self.camera, alpha, time)),
            _ => None,
        };
        let camera = match (view.framing, &level) {
            (Framing::Rect { origin, size }, _) => Camera::window(origin, size, r.size()),
            (Framing::Auto, Some(level)) => Camera::window(level.origin, level.size, r.size()),
            (Framing::Auto, None) => {
                Camera::fit([room.pixel_width(), room.pixel_height()], r.size(), MARGIN)
            }
        };
        let holograms = match &level {
            Some(view) if target.sprites.is_some() => self.game.hologram_draw_lists(
                alpha,
                rw_sim::Vec2::from(view.origin),
                rw_sim::Vec2::from(self.camera.screen_size()),
            ),
            _ => Vec::new(),
        };
        // `ShortcutGraphics.Draw`, every frame there is a palette.
        let shortcuts = match &self.visuals {
            Some(visuals) if target.sprites.is_some() => {
                let camera = self.camera.position(self.camera.current());
                self.game
                    .shortcut_draw_list(visuals.shortcut_palette(), rw_sim::Vec2::from(camera))
            }
            _ => None,
        };
        let r = &mut target.renderer;
        let water = self.game.water_surface(alpha);
        let palette = self.visuals.as_ref().map(Visuals::room_palette);
        let water_depth = match (&level, &water, palette) {
            (Some(view), Some(surface), Some(palette)) => {
                self.draw_water_sprites(surface, view, palette, alpha)
            }
            _ => None,
        };
        let water_sprites = self.water_sprites.as_ref().zip(water_depth);
        FrameDraw {
            game: &self.game,
            water: water.as_ref(),
            water_sprites,
            level,
            sprites: sprites.as_ref(),
            overseers: &overseers,
            holograms: &holograms,
            shortcuts: shortcuts.as_ref(),
            layers: view.layers,
            camera,
            alpha,
            time,
        }
        .queue(r);
    }
}

impl Scene {
    /// `Water.DrawSprites` for the level view `view`; returns `_waterDepth`.
    fn draw_water_sprites(
        &mut self,
        surface: &WaterSurface,
        view: &LevelView,
        palette: rw_assets::RoomPalette,
        alpha: f32,
    ) -> Option<f32> {
        let game = &self.game;
        let first_point = game.water_previous_point(view.origin[0] - 30.0)?;
        let total = surface.len();
        if !self
            .water_sprites
            .as_ref()
            .is_some_and(|s| s.fits(total, view.size[0]))
        {
            self.water_sprites = Some(WaterSprites::new(total, view.size[0]));
        }
        let sprites = self.water_sprites.as_mut()?;
        sprites.draw(&WaterFrame {
            surface,
            first_point,
            cam_pos: view.origin,
            screen_size: view.size,
            palette: &palette,
            glitter: game.water_glitter(alpha),
            light: game.player_light(alpha),
            time_stacker: alpha,
        });
        Some(game.water_depth())
    }
}

/// One frame's draw calls, in the game's order.
struct FrameDraw<'a> {
    game: &'a Game,
    /// The surface for the debug view's flat water.
    water: Option<&'a WaterSurface>,
    /// The game's water sprites and `_waterDepth`, in the level view.
    water_sprites: Option<(&'a WaterSprites, f32)>,
    /// The game's window into the level graphic; `None` in the debug view.
    level: Option<LevelView>,
    /// The slugcat's sprites, drawn only with the level graphic. Without it (the debug
    /// view, or no atlas) the physical body is drawn: chunks, rods and contacts.
    sprites: Option<&'a DrawList>,
    /// The overseers' sprites (level view only), after the slugcat's in each container.
    overseers: &'a [DrawList],
    /// The holograms' sprites (level view only), in the `Bloom` container.
    holograms: &'a [DrawList],
    /// The shortcut graphics (level view only), in their containers.
    shortcuts: Option<&'a DrawList>,
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
        let shortcuts = self.level.as_ref().zip(self.shortcuts);
        if let Some((level, list)) = shortcuts {
            r.draw_sprites(list, Layer::BackgroundShortcuts, level.origin);
            r.draw_sprites(list, Layer::Background, level.origin);
        }
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
            for layer in [Layer::Midground, Layer::Items] {
                for o in self.overseers {
                    r.draw_sprites(o, layer, level.origin);
                }
            }
            if let Some(list) = self.shortcuts {
                r.draw_sprites(list, Layer::Items, level.origin);
            }
            r.draw_level(level);
        }
        if let Some((level, list)) = sprites {
            r.draw_sprites(list, Layer::Foreground, level.origin);
        }
        if let Some(level) = self.level {
            for o in self.overseers {
                r.draw_sprites(o, Layer::Foreground, level.origin);
            }
        }
        match (shortcuts, self.level) {
            (Some((level, list)), _) => r.draw_sprites(list, Layer::Shortcuts, level.origin),
            // The debug view's vessel marker.
            (None, None) => r.draw_shortcut_vessel(self.game.vessel_tile()),
            (None, Some(_)) => {}
        }
        // The `Water` container, after the level, foreground sprites and shortcuts.
        match (self.level, self.water_sprites, self.water) {
            (Some(_), Some((sprites, depth)), _) => r.draw_water_sprites(sprites, depth),
            (None, _, Some(water)) => r.draw_water(water),
            _ => {}
        }
        if let Some((level, list)) = shortcuts {
            r.draw_sprites(list, Layer::Water, level.origin);
        }
        // `Bloom`, after `Water` (and the empty `GrabShaders`).
        if let Some(level) = self.level {
            for h in self.holograms {
                r.draw_sprites(h, Layer::Bloom, level.origin);
            }
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
        if self.room_of != Some((scene.id, scene.camera_room)) {
            self.renderer.set_room(scene.game.room());
            self.room_of = Some((scene.id, scene.camera_room));
        }
        if let Some(visuals) = &scene.visuals {
            let key = (scene.id, scene.camera_room, visuals.generation());
            if self.level_of != Some(key) {
                visuals.upload(&mut self.renderer, &scene.camera);
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
    use rw_render::{Color, LevelGraphics, Mesh, RenderError, SpriteAtlas, WaterSprites};

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
        /// Level, sprite, mesh and water calls in order.
        order: Vec<&'static str>,
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
            self.order.push("level");
        }
        fn set_atlas(&mut self, _: &SpriteAtlas, _: &[&PngImage]) {}
        fn set_stage_texture(
            &mut self,
            _: rw_render::StageTextureId,
            _: &PngImage,
            _: rw_render::TextureSampling,
        ) {
        }
        fn remove_stage_texture(&mut self, _: rw_render::StageTextureId) {}
        fn set_stage_globals(&mut self, _: rw_render::StageGlobals) {}
        fn draw_stage(&mut self, _: &rw_render::StageBatch, _: [f32; 2]) {}
        fn set_background(&mut self, _: Color) {}
        fn draw_sprites(&mut self, _: &DrawList, layer: Layer, _: [f32; 2]) {
            self.sprite_calls += 1;
            self.order.push(match layer {
                Layer::BackgroundShortcuts => "background shortcuts",
                Layer::Background => "background",
                Layer::Midground => "midground",
                Layer::Items => "items",
                Layer::Foreground => "foreground",
                Layer::Shortcuts => "shortcuts",
                Layer::Water => "water",
                Layer::Bloom => "bloom",
            });
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
        fn draw_mesh(&mut self, _: &Mesh) {
            self.order.push("mesh");
        }
        fn draw_water_sprites(&mut self, _: &WaterSprites, _: f32) {
            self.order.push("water sprites");
        }
        fn present(&mut self) -> Result<(), RenderError> {
            Ok(())
        }
        fn capture_next_frame(&mut self, _: PathBuf) {}
    }

    impl Recorder {
        fn record(game: &Game, level: Option<LevelView>, sprites: &DrawList) -> Self {
            Self::record_with_water(game, level, sprites, None, None)
        }

        fn record_with_water(
            game: &Game,
            level: Option<LevelView>,
            sprites: &DrawList,
            water: Option<&WaterSurface>,
            water_sprites: Option<(&WaterSprites, f32)>,
        ) -> Self {
            Self::record_all(game, level, sprites, water, water_sprites, None)
        }

        fn record_all(
            game: &Game,
            level: Option<LevelView>,
            sprites: &DrawList,
            water: Option<&WaterSurface>,
            water_sprites: Option<(&WaterSprites, f32)>,
            shortcuts: Option<&DrawList>,
        ) -> Self {
            let mut r = Self::default();
            FrameDraw {
                game,
                water,
                water_sprites,
                level,
                sprites: Some(sprites),
                overseers: &[],
                holograms: &[],
                shortcuts,
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
        let mut game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
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
        let mut game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
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

    #[test]
    fn water_sprites_after_the_level_and_foreground_flat_water_in_debug() {
        let mut game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let sprites = game.draw_list(1.0, SpriteColor::BLACK).expect("player");
        let surface = WaterSurface::flat(400.0, 50.0);
        let water = WaterSprites::new(surface.len(), 1366.0);
        let level = LevelView {
            origin: [0.0, 0.0],
            size: [1366.0, 768.0],
            rain: 0.0,
        };
        let r = Recorder::record_with_water(
            &game,
            Some(level),
            &sprites,
            Some(&surface),
            Some((&water, 0.0)),
        );
        assert_eq!(
            r.order,
            ["midground", "level", "foreground", "water sprites"],
            "the Water container comes after the level and the foreground sprites"
        );
        // The debug view keeps the flat translucent water, drawn last.
        let r = Recorder::record_with_water(&game, None, &sprites, Some(&surface), None);
        assert_eq!(r.order.last(), Some(&"mesh"));
        assert!(!r.order.contains(&"water sprites"));
    }

    /// `RoomCamera`'s containers: BackgroundShortcuts and Background before Midground,
    /// Items before the level, Shortcuts after Foreground, Water after the water.
    #[test]
    fn shortcut_sprites_in_their_containers() {
        let mut game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let sprites = game.draw_list(1.0, SpriteColor::BLACK).expect("player");
        let surface = WaterSurface::flat(400.0, 50.0);
        let water = WaterSprites::new(surface.len(), 1366.0);
        let level = LevelView {
            origin: [0.0, 0.0],
            size: [1366.0, 768.0],
            rain: 0.0,
        };
        let shortcuts = DrawList::default();
        let r = Recorder::record_all(
            &game,
            Some(level),
            &sprites,
            Some(&surface),
            Some((&water, 0.0)),
            Some(&shortcuts),
        );
        assert_eq!(
            r.order,
            [
                "background shortcuts",
                "background",
                "midground",
                "items",
                "level",
                "foreground",
                "shortcuts",
                "water sprites",
                "water"
            ]
        );
        assert_eq!(r.rings, 0, "no debug vessel marker over the level");
    }
}
