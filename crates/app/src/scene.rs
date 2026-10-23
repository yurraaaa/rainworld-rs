//! What a frame shows: the game with its level graphic and water and the HUD, drawn the
//! same way in the window and in headless (MCP) screenshots.
//!
//! Mirrors: RoomCamera.cs

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rw_assets::{CustomDecalData, StreamingAssets, UnityResources};
use rw_graphics::{BackgroundSceneView, DrawList, Layer, SceneImageSizes};
use rw_hud::{Hud, IllustrationSource, PageText};
use rw_menu::{AtlasManager, Font, MenuShared};
use rw_random::UnityRandom;
use rw_render::{
    Camera, LevelView, Renderer, RoomCamera, RoomLayers, StageBatch, SurfaceLight, WaterFrame,
    WaterSprites, WaterSurface,
};
use rw_sim::{HudEvent, Input, LightningFrame, SoundEvent};

use crate::death_fall::DeathFallSprite;
use crate::game::Game;
use crate::hibernation::HibernatingGame;
use crate::sprite_order::Drawable;
use crate::sprites::SpriteAssets;
use crate::timestep::TICK_RATE;
use crate::visuals::Visuals;
use crate::water_light::WaterLight;

#[path = "game_music.rs"]
mod music;

/// Screen pixels kept free around the room in the debug view.
pub const MARGIN: f32 = 16.0;

/// The order of the region map's sprites and the room's in the room camera's `HUD`
/// container. `Map.Update` adds the map's sprites on its first run, a `RoomCamera.Update`
/// after the HUD was made; a `ChangeRoom` (every camera move) adds the room's sprites after
/// whatever is in the container then. So the map is under the room's sprites (a
/// `GravityDisruptor`'s disc, which ripples it) only if a camera move came after the map
/// loaded; a move before that, the session's first included, leaves it above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MapOrder {
    /// The map has had its first `Update`.
    loaded: bool,
    /// Its sprites were added after the room's.
    over_disc: bool,
}

impl MapOrder {
    /// A session: the first room's sprites come before the map's.
    const fn new() -> Self {
        Self {
            loaded: false,
            over_disc: true,
        }
    }

    /// `Map.Update` ran (the HUD is updated from the tick after the one that made it).
    fn map_updated(&mut self) {
        self.loaded = true;
    }

    /// `ChangeRoom`: the room's sprites are added after the map's, if it is loaded.
    fn camera_moved(&mut self) {
        if self.loaded {
            self.over_disc = false;
        }
    }
}

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
    /// The room camera's `WaterLight`, in a room with water.
    water_light: Option<WaterLight>,
    /// The camera moved to another room since the last [`Scene::take_entered_room`].
    entered_room: bool,
    /// `loadingRoom` and `loadingCameraPos` while `applyPosChangeWhenTextureIsLoaded`.
    pending_move: Option<rw_sim::CameraMove>,
    /// Camera 0's HUD, once fired up.
    hud: Option<Hud>,
    /// The quads of the HUD's first container, drawn by the next frame before the room's
    /// `HUD` container sprites ([`Scene::prepare_hud`]).
    hud_below: StageBatch,
    /// Where the map's sprites sit in the `HUD` container against the room's.
    map_order: MapOrder,
    /// The time stacker and cosmetic stream of the last frame drawn unpaused: while the
    /// game is paused nothing in the room redraws (`PausedDrawUpdate` only draws the
    /// camera's micro shake), so frames repeat that one.
    last_unpaused_frame: Option<(f32, UnityRandom)>,
    /// The room's `Lightning` as the last frame drawn read it (`DrawUpdate`).
    lightning_frame: Option<LightningFrame>,
    /// `GamePaused`: the room's frame repeats the last unpaused one.
    pub paused: bool,
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
    /// The visuals' palette generation uploaded with it.
    palette_of: u32,
    /// The decal images uploaded as atlas pages (`CustomDecal.LoadFile`'s loaded set).
    decals: HashSet<String>,
    /// The background scenes' images uploaded as atlas pages (`LoadGraphic`'s loaded set).
    scene_images: HashSet<&'static str>,
    /// Their sizes (`sourceSize` of each atlas element).
    scene_sizes: SceneImageSizes,
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
            water_light: None,
            entered_room: false,
            pending_move: None,
            hud: None,
            hud_below: StageBatch::default(),
            map_order: MapOrder::new(),
            last_unpaused_frame: None,
            lightning_frame: None,
            paused: false,
        };
        scene.load_visuals();
        scene.game.new_shortcut_room(scene.camera_room);
        scene.queue_camera_move();
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
        self.initiate_death_fall();
        self.water_light = self.new_water_light();
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
        self.apply_fade();
        let earth = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::earth_color);
        self.game.set_palette_earth(earth);
    }

    /// `DeathFallGraphic.InitiateSprites` for the camera room's, as the camera enters.
    fn initiate_death_fall(&mut self) {
        let index = self.camera_room;
        if let Some(r) = self.game.sim_mut().world_mut().realized_mut(index)
            && let Some(fall) = &mut r.death_fall
        {
            fall.initiate_sprites(&r.room);
        }
    }

    /// `RoomCamera.ChangeRoom`'s `WaterLight` for the camera's new room.
    fn new_water_light(&self) -> Option<WaterLight> {
        let sim = self.game.sim();
        let realized = sim.world().realized(self.camera_room)?;
        WaterLight::new(&realized.room, self.game.water(), &realized.settings)
    }

    /// The room's water line for `_waterLevel` (`DrawUpdate`): the water's level + 100,
    /// else the death fall's height + 180, else 0; `None` in gates and shelters.
    fn water_line(&self) -> Option<f32> {
        let sim = self.game.sim();
        let data = sim.world().abstract_room(self.camera_room)?;
        if data.gate() || data.shelter() {
            return None;
        }
        Some(match (self.game.water(), self.death_fall()) {
            (Some(water), _) => water.still_level() + 100.0,
            (None, Some(fall)) => fall.height + 180.0,
            (None, None) => 0.0,
        })
    }

    /// `ApplyPalette`'s `SetUpFullScreenEffect`: the effect sprite back on top of its
    /// container.
    fn apply_palette(&mut self) {
        self.game.raise_full_screen_effect();
    }

    /// The move `RoomCamera.Update` has yet to apply.
    #[must_use]
    pub fn pending_move(&self) -> Option<rw_sim::CameraMove> {
        self.pending_move.or_else(|| {
            Some(rw_sim::CameraMove {
                room: self.camera_room,
                camera: self.camera.pending()?,
            })
        })
    }

    /// `RoomCamera.MoveCamera`: a later move replaces an unapplied one (`loadingRoom`,
    /// `loadingCameraPos`).
    fn queue_camera_move(&mut self) {
        if let Some(mv) = self.game.take_camera_move() {
            // `MoveCamera` calls `virtualMicrophone.NewRoom` itself.
            self.entered_room |= mv.room != self.camera_room;
            self.pending_move = Some(mv);
        }
    }

    /// `applyPosChangeWhenTextureIsLoaded` in `RoomCamera.Update`: the move queued in
    /// the last tick takes effect (the texture was preloaded in `MoveCamera2`, so one
    /// update later). On another room its level graphic is loaded (at once: D14),
    /// `ChangeRoom` makes the shortcut graphics anew, and the camera cuts to the camera
    /// asked for.
    fn apply_camera_move(&mut self) {
        let Some(mv) = self.pending_move.take() else {
            return;
        };
        self.map_order.camera_moved();
        if mv.room != self.camera_room {
            self.camera_room = mv.room;
            self.game.set_shown_room(mv.room);
            self.game.new_shortcut_room(mv.room);
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
        // `ApplyPositionChange` ends with `ApplyPalette`.
        self.apply_palette();
    }

    /// `RoomCamera.Update`'s `ApplyFade` when `DarkPalette` changed (it runs before the
    /// rain cycle's update in `RainWorldGame.Update`); returns whether it ran.
    fn apply_fade(&mut self) -> bool {
        self.visuals
            .as_mut()
            .is_some_and(|v| v.fade(&self.game.sim().world().rain_cycle))
    }

    /// The level shader's inputs at the camera's position, for `get_state`.
    #[must_use]
    pub fn level_state(&self) -> Option<crate::visuals::LevelState> {
        let visuals = self.visuals.as_ref()?;
        let water_level = Visuals::water_level(
            self.water_line(),
            self.camera.view_origin(1.0)[1],
            self.camera.screen_size()[1],
        );
        // The flash is drawn from a copy of the cosmetic stream, so asking changes nothing.
        let lightning = self.lightning_frame_at(1.0, &mut self.game.cosmetic_rng().clone());
        Some(visuals.state(&self.game.sim().world().rain_cycle, water_level, lightning))
    }

    /// The camera room's `Lightning` as `DrawUpdate` reads it at `time_stacker`, with
    /// the flicker on `rng`.
    fn lightning_frame_at(
        &self,
        time_stacker: f32,
        rng: &mut UnityRandom,
    ) -> Option<LightningFrame> {
        let visuals = self.visuals.as_ref()?;
        let room = self.game.sim().world().realized(self.camera_room)?;
        Some(visuals.lightning_frame(room.lightning.as_ref()?, time_stacker, rng))
    }

    /// `DrawUpdate`'s reads of the room's `Lightning` for this frame, on the cosmetic
    /// stream (D1).
    fn draw_lightning(&mut self, alpha: f32) {
        let mut rng = self.game.cosmetic_rng().clone();
        self.lightning_frame = self.lightning_frame_at(alpha, &mut rng);
        if self.lightning_frame.is_some() {
            self.game.set_cosmetic_rng(rng);
        }
    }

    #[must_use]
    pub fn death_fall(&self) -> Option<&rw_sim::DeathFall> {
        let sim = self.game.sim();
        sim.world().realized(self.camera_room)?.death_fall.as_ref()
    }

    /// The camera's `WaterLight` and its alpha at the palette's darkness.
    #[must_use]
    pub fn water_light(&self) -> Option<(&WaterLight, f32)> {
        let darkness = self
            .visuals
            .as_ref()
            .map_or(0.0, |v| v.room_palette().darkness);
        self.water_light.as_ref().map(|l| (l, l.alpha(darkness)))
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

    /// R: a fresh player in a dev start, which also takes the HUD out of game-over
    /// mode (D4); nothing in a story session. Returns whether it respawned.
    pub fn respawn(&mut self) -> bool {
        if !self.game.respawn() {
            return false;
        }
        if let Some(hud) = &mut self.hud {
            hud.text_prompt.leave_game_over_mode();
        }
        true
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

    /// One 40 Hz tick: the HUD and the room camera (`RoomCamera.Update` comes before the
    /// rooms in `RainWorldGame.Update`), the game, the controls page (a room object),
    /// then the tick's camera shake and room move; returns the sound events since the
    /// last tick. Without `shared` (fonts, atlases, strings) there is no HUD.
    pub fn tick(&mut self, input: Input, shared: Option<&mut MenuShared>) -> Vec<SoundEvent> {
        match shared {
            Some(shared) => {
                self.update_cameras(input, shared);
                self.update_rooms(input, shared, false)
            }
            None => {
                self.update_room_camera();
                self.game.tick(input);
                self.game.sim_mut().take_game_over();
                self.after_game_tick()
            }
        }
    }

    fn set_camera_view(&mut self) {
        self.game
            .sim_mut()
            .set_camera_view(self.camera.current(), self.camera.most_likely_next());
    }

    /// The unpaused tick's `cameras[i].Update()`: the HUD, then the room camera up to
    /// its shake (which the sim's tick makes). `input` is this tick's controls, whose
    /// map button the game-over prompt reads.
    pub fn update_cameras(&mut self, input: Input, shared: &mut MenuShared) {
        self.set_camera_view();
        self.update_hud(input.map, shared, false);
        self.update_room_camera();
    }

    /// `RoomCamera.Update`'s camera choice and window lerp, handed to the sim with the
    /// window position its shake and the rooms' `ScreenMovement`s start from.
    fn update_room_camera(&mut self) {
        self.apply_camera_move();
        if self.camera.update(self.game.camera_follow().as_ref()) {
            log::info!("camera {}", self.camera.current() + 1);
            self.game.sim_mut().camera_moved();
            if let Some(visuals) = &mut self.visuals {
                visuals.show(self.camera.current());
            }
            self.apply_palette();
        }
        // `ApplyFade` when `fadeCoord` changed, still before the rooms update.
        if self.apply_fade() {
            self.apply_palette();
        }
        self.set_camera_view();
        let width = self.camera.screen_size()[0];
        self.game.sim_mut().set_camera_pos(self.camera.pos(), width);
    }

    /// A paused tick (`RainWorldGame.Update` with `GamePaused`): only the HUD updates;
    /// the rooms' `PausedUpdate` does nothing for anything ported.
    pub fn paused_tick(&mut self, input: Input, shared: &mut MenuShared) {
        self.update_hud(input.map, shared, true);
    }

    /// The rest of an unpaused tick after the cameras: the game, the controls page (a
    /// room object; `pause_menu` whether the pause menu opened this tick), then the room
    /// camera's move; returns the sound events since the last tick.
    pub fn update_rooms(
        &mut self,
        input: Input,
        shared: &mut MenuShared,
        pause_menu: bool,
    ) -> Vec<SoundEvent> {
        let script = self.game.start_up_script();
        let showed_controls = script.is_some_and(|s| s.showed_controls());
        let continue_page = script.is_some_and(|s| s.controls_page_continue());
        self.game.sim_mut().set_hud_present(self.hud.is_some());
        self.game.link_hud_map(self.hud.as_ref());
        let darkness = self
            .visuals
            .as_ref()
            .map_or(0.0, |v| v.room_palette().darkness);
        self.game.sim_mut().set_camera_darkness(darkness);
        let camera_room = self.camera_room;
        let image = self
            .visuals
            .as_ref()
            .map(|v| v.camera_image(camera_room, self.camera.position(self.camera.current())));
        self.game.sim_mut().set_camera_image(image);
        self.link_trigger_hud();
        self.game.tick(input);
        self.apply_game_over(shared);
        self.apply_music_requests(shared);
        self.apply_hud_events(shared);
        if let Some(hud) = &mut self.hud {
            hud.update_controls_page(continue_page, pause_menu);
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
    /// realized. `map_button`: the map button held (`GetButton(11)`, which the game-over
    /// prompt reads from the controls, not from the player); `paused`: the pause menu is
    /// open.
    fn update_hud(&mut self, map_button: bool, shared: &mut MenuShared, paused: bool) {
        match &mut self.hud {
            Some(hud) => {
                self.map_order.map_updated();
                hud.text_prompt.pause_menu = paused;
                let owner = self.game.hud_owner();
                let rain = self.game.rain_view();
                let (sounds, rng, discovery) = self.game.sim_mut().camera_io();
                hud.update(
                    &owner,
                    map_button,
                    &rain,
                    &shared.translator,
                    sounds,
                    rng,
                    discovery,
                );
            }
            None if self.game.hud_due() => {
                let mut setup = self.game.hud_setup(shared.screen_size);
                setup.map = self.game.map_data(&shared.assets, &shared.saves);
                if let Some(map) = &setup.map {
                    self.game.load_map_texture(&mut shared.saves, &map.region);
                }
                setup.cycle_label = self.game.map_cycle_label(&shared.translator);
                self.hud = Some(Hud::new(setup, &mut shared.atlases));
                log::info!("HUD fired up");
            }
            None => {}
        }
    }

    /// `RainWorldGame.GameOver` as the player's first death called it, in story sessions
    /// and dev starts (D4): `PlaceKarmaFlowerOnDeathSpot` (a dev start's on its
    /// in-memory save state, D16), camera 0's HUD into game-over mode, then the
    /// music's `DeathEvent`.
    fn apply_game_over(&mut self, shared: &mut MenuShared) {
        if self.game.sim_mut().take_reds_game_over() {
            Self::go_to_reds_game_over(shared);
        }
        let Some(over) = self.game.sim_mut().take_game_over() else {
            return;
        };
        log::info!("game over");
        if let Some(grow_pos) = over.karma_flower {
            let game = HibernatingGame {
                sim: self.game.sim_mut(),
                assets: &shared.assets,
            };
            game.place_karma_flower_on_death_spot(&mut shared.saves, grow_pos);
        }
        if let Some(hud) = &mut self.hud {
            hud.text_prompt.enter_game_over_mode(
                over.dependent_on_grasp,
                over.food_in_stomach,
                over.death_room,
                over.death_pos.to_array(),
            );
        }
        if let Some(music) = &mut shared.music {
            music.death_event();
        }
    }

    /// `GoToRedsGameOver`, with no process switch asked for yet: only the music's
    /// `FadeOutAllSongs(20)` is ported (the illness fade, `redsDeath`, the statistics
    /// and the save are not, D17).
    pub fn go_to_reds_game_over(shared: &mut MenuShared) {
        log::warn!("GoToRedsGameOver: only its music fade is ported");
        if let Some(music) = &mut shared.music {
            music.reds_game_over();
        }
    }

    /// The guide's `TutorialText`s (translated, darkening the screen), its rain meter
    /// resets and the food meter's refusals.
    fn apply_hud_events(&mut self, shared: &mut MenuShared) {
        let events = self.game.take_hud_events();
        let Some(hud) = &mut self.hud else {
            return;
        };
        for event in events {
            match event {
                HudEvent::DangleFruitInWaterMessage => {
                    if let Some(state) = shared.saves.current_save_state.as_mut() {
                        state
                            .death_persistent
                            .set_tutorial_message(rw_save::Tutorial::DangleFruitInWater, true);
                    }
                }
                HudEvent::Message(m) => {
                    let text = shared.translator.translate(m.text);
                    hud.add_message(text, m.wait, m.time, true, m.hide_hud);
                }
                HudEvent::HalfTimeBlinkReset => hud.rain_meter.half_time_blink = 0,
                HudEvent::RefuseFood => hud.food_meter.refuse_food(),
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

    /// Hands the HUD's changed textures to `atlases` (before they sync to a renderer).
    pub fn sync_hud_textures(&mut self, atlases: &mut AtlasManager) {
        if let Some(hud) = &mut self.hud {
            hud.sync_textures(atlases);
        }
    }

    /// The HUD map's shader globals at `alpha` between ticks.
    #[must_use]
    pub fn hud_map_globals(&self, alpha: f32) -> rw_render::MapGlobals {
        self.hud
            .as_ref()
            .map_or_else(rw_render::MapGlobals::default, |h| h.map_globals(alpha))
    }

    /// Sets the HUD's sprites for `alpha` between ticks and keeps the quads of its first
    /// container (`HUD`, the region map), which the next [`Self::draw`] queues before the
    /// room's `HUD` container sprites (a `GravityDisruptor`'s disc, which distorts them).
    /// Call it before `draw` whenever the HUD is drawn, then [`Self::draw_hud`].
    pub fn prepare_hud(&mut self, atlases: &AtlasManager, alpha: f32) {
        self.hud_below = StageBatch::default();
        if let Some(hud) = &mut self.hud {
            hud.draw(alpha);
            hud.push_quads_split(atlases, &mut self.hud_below, &mut StageBatch::default());
        }
    }

    /// Appends the rest of the HUD's quads (`HUD2`...), those [`Self::prepare_hud`] left.
    pub fn draw_hud(&mut self, atlases: &AtlasManager, batch: &mut StageBatch) {
        if let Some(hud) = &self.hud {
            hud.push_quads_split(atlases, &mut StageBatch::default(), batch);
        }
    }

    /// The room camera after the game's tick: the shake's jump and the rooms' bumps,
    /// then a move to another room; returns the sound events.
    fn after_game_tick(&mut self) -> Vec<SoundEvent> {
        let shake = self.game.sim_mut().take_camera_displacement();
        self.camera.displace(shake);
        self.queue_camera_move();
        self.game.take_sound_events()
    }

    /// `DrawUpdate`'s micro shake for this frame, on the cosmetic stream (D1): a random
    /// direction, then a random share of `8 * microShake` px.
    fn draw_micro_shake(&mut self) {
        let micro = self.game.sim().camera_shake().micro_shake;
        let offset = if micro > 0.0 {
            let rng = self.game.cosmetic_rng_mut();
            let dir = rw_sim::Vec2::random_unit(rng);
            dir * (8.0 * micro * rng.value())
        } else {
            rw_sim::Vec2::ZERO
        };
        self.camera.set_frame_shake(offset);
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
        let (alpha, live_rng) = self.frame_alpha(alpha);
        self.draw_micro_shake();
        let origin = rw_sim::Vec2::from(self.camera.view_origin(alpha));
        if let Some(globals) = self.game.background_scene_globals(origin) {
            target.renderer.set_scene_globals(globals);
        }
        self.draw_lightning(alpha);
        if let (Some(v), Some(frame)) = (&self.visuals, &self.lightning_frame) {
            target
                .renderer
                .set_level_palette(&v.lightning_palette(frame));
        }
        // The slugcat's sprites, eyes in the palette's black. Built every frame, as the
        // game runs `DrawSprites` whatever is shown; only the level view draws them.
        let face = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::black_color);
        let fog = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::fog_color);
        let mut sprites = if target.sprites.is_some() {
            self.game.draw_list(alpha, face, fog)
        } else {
            None
        };
        let earth = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::earth_color);
        let darkness = self
            .visuals
            .as_ref()
            .map_or(0.0, |v| v.room_palette().darkness);
        // `Water` draws before the light sources' owners hard-set them this frame.
        let surface_lights = self.game.surface_lights();
        // `ShortcutGraphics.Draw`, every frame there is a palette.
        let shortcuts = match &self.visuals {
            Some(visuals) if target.sprites.is_some() => {
                let camera = self.camera.position(self.camera.current());
                self.game
                    .shortcut_draw_list(visuals.shortcut_palette(), rw_sim::Vec2::from(camera))
            }
            _ => None,
        };
        // Every drawable's sprites, in camera 0's container order.
        let has_player_sprites = sprites.is_some();
        let objects = if target.sprites.is_some() {
            self.game.sync_sprite_order();
            let player = Drawable::Entity(rw_sim::item::Entity::Creature(
                self.game.sim().player_creature(),
            ));
            let mut lists: Vec<(Drawable, DrawList)> =
                sprites.take().map(|l| (player, l)).into_iter().collect();
            lists.extend(
                self.game
                    .shelter_draw_list(alpha)
                    .map(|l| (Drawable::ShelterDoor, l)),
            );
            let entities = [
                self.game.weapon_draw_lists(alpha, face),
                self.game.fly_draw_lists(alpha, face),
                self.game.lizard_draw_lists(alpha, face),
            ];
            lists.extend(
                entities
                    .into_iter()
                    .flatten()
                    .map(|(e, l)| (Drawable::Entity(e), l)),
            );
            lists.extend(self.game.overseer_draw_lists(alpha, earth));
            let drips = self.visuals.as_ref().map_or(
                rw_graphics::DripPalette {
                    black: face,
                    water_color1: face,
                    water_color2: face,
                    darkness,
                },
                Visuals::drip_palette,
            );
            lists.extend(self.game.drip_draw_lists(alpha, drips));
            lists.extend(self.game.cosmetic_draw_lists(alpha, face));
            let cam_pos = rw_sim::Vec2::from(self.camera.view_origin(alpha));
            lists.extend(self.game.ss_electric_draw_lists(
                alpha,
                rw_sim::Vec2::from(self.camera.position(self.camera.current())),
            ));
            lists.extend(self.game.ss_grab_draw_lists(
                alpha,
                rw_sim::Vec2::from(self.camera.position(self.camera.current())),
            ));
            lists.extend(self.game.holo_light_draw_lists(alpha, cam_pos, darkness));
            lists.extend(self.game.placed_light_draw_lists(self.camera.current()));
            if let Some(visuals) = &self.visuals {
                let screen = rw_sim::Vec2::from(self.camera.screen_size());
                lists.extend(self.game.insect_draw_lists(
                    alpha,
                    visuals.palette_texture(),
                    cam_pos,
                    screen,
                ));
                lists.extend(
                    self.game
                        .placed_cosmetic_draw_lists(alpha, &visuals.room_palette()),
                );
                let scene_view = BackgroundSceneView {
                    cam_pos,
                    camera_position: rw_sim::Vec2::from(
                        self.camera.position(self.camera.current()),
                    ),
                    h_displace: self.camera.h_displace(),
                    screen: rw_sim::Vec2::from(self.camera.screen_size()),
                    sky_color: rw_graphics::Color::from(visuals.room_palette().sky_color),
                    time_stacker: alpha,
                };
                lists.extend(
                    self.game
                        .background_scene_draw_lists(&scene_view, &target.scene_sizes),
                );
                lists.extend(
                    self.game
                        .dust_puff_draw_lists(alpha, visuals.palette_texture()),
                );
                lists.extend(
                    self.game
                        .particle_draw_lists(alpha, visuals.palette_texture()),
                );
                lists.extend(self.game.fixture_draw_lists(alpha, &visuals.room_palette()));
            }
            lists.extend(self.game.placed_visual_draw_lists());
            lists.extend(self.game.electric_death_draw_lists(alpha, cam_pos));
            lists.extend(self.game.coral_draw_lists(alpha, cam_pos));
            lists.extend(self.game.projection_draw_lists(
                alpha,
                cam_pos,
                rw_sim::Vec2::from(self.camera.position(self.camera.current())),
            ));
            let level_view = self.visuals.is_some() && view.level;
            if let Some(list) = shortcuts.filter(|_| level_view) {
                lists.push((Drawable::Shortcuts, list));
            }
            if let Some(list) = self.full_screen_effect(alpha).filter(|_| level_view) {
                lists.push((Drawable::FullScreenEffect, list));
            }
            if let (Some(light), Some(water)) = (&self.water_light, self.game.water())
                && level_view
            {
                lists.push((Drawable::WaterLight, light.draw_list(water, darkness)));
            }
            if let Some(fall) = self.death_fall().filter(|_| level_view) {
                let width = self.camera.screen_size()[0];
                lists.push((
                    Drawable::DeathFall,
                    DeathFallSprite::draw_list(fall, fall.height, width, cam_pos, alpha),
                ));
            }
            self.game.in_container_order(lists)
        } else {
            Vec::new()
        };

        let r = &mut target.renderer;
        let room = self.game.room();
        let time = (self.game.ticks() as f32 + alpha) / TICK_RATE as f32;
        let rain = &self.game.sim().world().rain_cycle;
        let level = match &self.visuals {
            Some(v) if view.level => Some(v.view(
                &self.camera,
                alpha,
                time,
                rain,
                self.lightning_frame.as_ref(),
                self.water_line(),
            )),
            _ => None,
        };
        // `SunBlocker`s, first in the `Shadows` container (spot lights follow).
        let shadows: Vec<DrawList> = match (&level, &self.visuals) {
            (Some(level), Some(v)) if target.sprites.is_some() => (0..v.sun_blockers())
                .map(|_| rw_graphics::SunBlocker::draw(rw_sim::Vec2::from(level.origin)))
                .collect(),
            _ => Vec::new(),
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
        let r = &mut target.renderer;
        let water = self.game.water_surface(alpha);
        let palette = self.visuals.as_ref().map(Visuals::room_palette);
        let water_depth = match (&level, &water, palette) {
            (Some(view), Some(surface), Some(palette)) => {
                self.draw_water_sprites(surface, view, palette, &surface_lights, alpha)
            }
            _ => None,
        };
        let water_sprites = self.water_sprites.as_ref().zip(water_depth);
        FrameDraw {
            game: &self.game,
            water: water.as_ref(),
            water_sprites,
            level,
            player_sprites: has_player_sprites,
            shadows: &shadows,
            objects: &objects,
            holograms: &holograms,
            hud_below: &self.hud_below,
            screen: self.camera.screen_size(),
            map_over_disc: self.map_order.over_disc,
            layers: view.layers,
            camera,
            alpha,
            time,
        }
        .queue(r);
        self.hud_below = StageBatch::default();
        if let Some(rng) = live_rng {
            self.game.set_cosmetic_rng(rng);
        }
        self.paused_draw_update();
    }
}

impl Scene {
    /// The camera's `fullScreenEffect` sprite this frame, if the room has one.
    fn full_screen_effect(&self, alpha: f32) -> Option<DrawList> {
        let visuals = self.visuals.as_ref()?;
        let effect = visuals.full_screen_effect()?;
        let strength = visuals.effect_alpha(
            &effect,
            &self.game.sim().world().rain_cycle,
            self.lightning_frame.as_ref(),
        );
        let cam_pos = rw_sim::Vec2::from(self.camera.view_origin(alpha));
        Some(effect.draw_list(self.camera.screen_size()[0], cam_pos, strength))
    }

    /// The time stacker to draw at: `alpha`, or while paused the last unpaused frame's,
    /// with the cosmetic stream as it was then so the frame repeats; the stream it
    /// replaced comes back for [`Self::paused_draw_update`].
    fn frame_alpha(&mut self, alpha: f32) -> (f32, Option<UnityRandom>) {
        match (&self.last_unpaused_frame, self.paused) {
            (Some((last, rng)), true) => {
                let last = *last;
                let live = self.game.cosmetic_rng().clone();
                self.game.set_cosmetic_rng(rng.clone());
                (last, Some(live))
            }
            _ => {
                self.last_unpaused_frame = Some((alpha, self.game.cosmetic_rng().clone()));
                (alpha, None)
            }
        }
    }

    /// `RoomCamera.PausedDrawUpdate`, last in every drawn frame (paused or not): while
    /// `microShake` is up it draws a random direction and a share of `8 * microShake` px
    /// for the camera position it hands the cosmetic sprites, which use none of it. The
    /// draws are on the cosmetic stream (D1).
    fn paused_draw_update(&mut self) {
        let micro = self.game.sim().camera_shake().micro_shake;
        if micro > 0.0 {
            let rng = self.game.cosmetic_rng_mut();
            let dir = rw_sim::Vec2::random_unit(rng);
            let _offset = dir * (8.0 * micro * rng.value());
        }
    }

    /// `Water.DrawSprites` for the level view `view`; returns `_waterDepth`.
    fn draw_water_sprites(
        &mut self,
        surface: &WaterSurface,
        view: &LevelView,
        palette: rw_assets::RoomPalette,
        lights: &[SurfaceLight],
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
            lights,
            darkness: palette.darkness,
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
    /// The slugcat has sprites this frame. They draw only with the level graphic;
    /// without it (the debug view, or no atlas) the physical body is drawn: chunks, rods
    /// and contacts.
    player_sprites: bool,
    /// The `Shadows` container's sprites (level view only).
    shadows: &'a [DrawList],
    /// Every drawable's sprites (level view only), the slugcat's and the shortcut
    /// graphics' among them, in the camera's container order.
    objects: &'a [DrawList],
    /// The holograms' sprites (level view only), in the `Bloom` container.
    holograms: &'a [DrawList],
    /// The HUD's first container's quads (`HUD`, the region map) and the screen they are
    /// in pixels of.
    hud_below: &'a StageBatch,
    screen: [f32; 2],
    /// [`Scene`]'s `map_over_disc`.
    map_over_disc: bool,
    layers: RoomLayers,
    camera: Camera,
    alpha: f32,
    time: f32,
}

impl FrameDraw<'_> {
    /// One container: every drawable's sprites in it, in order.
    fn draw_layer(&self, r: &mut impl Renderer, layer: Layer, level: LevelView) {
        for o in self.objects {
            if o.layer(layer).next().is_some() {
                r.draw_sprites(o, layer, level.origin);
            }
        }
    }

    /// The HUD's first container's quads (the region map).
    fn draw_map(&self, r: &mut impl Renderer) {
        if !self.hud_below.draws.is_empty() {
            r.draw_stage(self.hud_below, self.screen);
        }
    }

    fn queue(&self, r: &mut impl Renderer) {
        r.begin_frame(self.camera);
        if self.level.is_some()
            && let Some(offset) = self.game.projection_grid_offset(self.alpha)
        {
            r.set_grid_offset(offset);
        }
        r.draw_room(RoomLayers {
            tiles: self.layers.tiles && self.level.is_none(),
            ..self.layers
        });
        // Sprites sit at `room - camPos`, the level view's origin.
        if let Some(level) = self.level {
            for s in self.shadows {
                r.draw_sprites(s, Layer::Shadows, level.origin);
            }
            self.draw_layer(r, Layer::Shadows, level);
            self.draw_layer(r, Layer::BackgroundShortcuts, level);
            self.draw_layer(r, Layer::Background, level);
        }
        if self.level.is_none() || !self.player_sprites {
            for (body, status, kind) in self.game.debug_bodies() {
                let style = status.style(self.time).tinted(kind.color(), status.dead);
                r.draw_object_styled(body, self.alpha, style);
            }
            if let Some(body) = self.game.body() {
                r.draw_object(body, self.alpha, self.game.status(), self.time);
            }
        }
        if let Some(level) = self.level {
            self.draw_layer(r, Layer::Midground, level);
            self.draw_layer(r, Layer::Items, level);
            r.draw_level(level);
            self.draw_layer(r, Layer::Foreground, level);
            self.draw_layer(r, Layer::ForegroundLights, level);
            self.draw_layer(r, Layer::Shortcuts, level);
        } else {
            // The debug view's vessel marker.
            r.draw_shortcut_vessel(self.game.vessel_tile());
        }
        // The `Water` container, after the level, foreground sprites and shortcuts.
        match (self.level, self.water_sprites, self.water) {
            (Some(_), Some((sprites, depth)), _) => r.draw_water_sprites(sprites, depth),
            (None, _, Some(water)) => r.draw_water(water),
            _ => {}
        }
        if let Some(level) = self.level {
            self.draw_layer(r, Layer::Water, level);
            self.draw_layer(r, Layer::GrabShaders, level);
            // `Bloom`, after `GrabShaders`: the death fall, the full-screen effect, then
            // the holograms (D7).
            self.draw_layer(r, Layer::Bloom, level);
            for h in self.holograms {
                r.draw_sprites(h, Layer::Bloom, level.origin);
            }
            // `HUD`, the last container of the room camera: the HUD's own sprites in it (the
            // region map, added when the HUD was made) under whatever the room added, a
            // `GravityDisruptor`'s disc, which distorts them.
            if !self.map_over_disc {
                self.draw_map(r);
            }
            self.draw_layer(r, Layer::Hud, level);
            if self.map_over_disc {
                self.draw_map(r);
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
            palette_of: 0,
            decals: HashSet::new(),
            scene_images: HashSet::new(),
            scene_sizes: SceneImageSizes::default(),
        }
    }

    /// Uploads the images of the camera room's background scenes not yet uploaded, as
    /// `LoadGraphic` loads them (`Illustrations/<name>.png`, as an atlas of one element);
    /// one that cannot be read only warns (its element is not drawn).
    fn sync_scene_images(&mut self, scene: &Scene) {
        let Some(source) = &scene.source else { return };
        if self.sprites.is_none() {
            return;
        }
        for (name, sampling) in scene.game.camera_scene_images() {
            if !self.scene_images.insert(name) {
                continue;
            }
            let path = source
                .assets
                .resolve_file_path(&format!("Illustrations/{name}.png"));
            match rw_assets::PngImage::load(&path) {
                Ok(image) => {
                    let (w, h) = (image.width() as u32, image.height() as u32);
                    self.scene_sizes
                        .insert(name, rw_sim::Vec2::new(w as f32, h as f32));
                    self.renderer.add_atlas_page(
                        rw_graphics::Atlas::single(name, w, h),
                        &image,
                        sampling,
                    );
                }
                Err(e) => log::warn!("no scene image {name} ({e})"),
            }
        }
    }

    /// Uploads the images of the camera room's decals not yet uploaded, as
    /// `CustomDecal.LoadFile` loads them when the object is made; one that cannot be read
    /// only warns (its decal is not drawn).
    fn sync_decals(&mut self, scene: &Scene) {
        let Some(source) = &scene.source else { return };
        if self.sprites.is_none() {
            return;
        }
        for name in scene.game.camera_room_decal_images() {
            if !self.decals.insert(name.to_owned()) {
                continue;
            }
            match CustomDecalData::load_image(&source.assets, name) {
                Ok(image) => {
                    let (w, h) = (image.width() as u32, image.height() as u32);
                    self.renderer.add_atlas_page(
                        rw_graphics::Atlas::single(name, w, h),
                        &image,
                        rw_render::TextureSampling::POINT_CLAMP,
                    );
                }
                Err(e) => log::warn!("no decal image {name} ({e})"),
            }
        }
    }

    /// Uploads `scene`'s room and shown camera graphic where they changed.
    fn sync(&mut self, scene: &Scene) {
        self.sync_decals(scene);
        self.sync_scene_images(scene);
        if self.room_of != Some((scene.id, scene.camera_room)) {
            self.renderer.set_room(scene.game.room());
            self.room_of = Some((scene.id, scene.camera_room));
        }
        if let Some(visuals) = &scene.visuals {
            let key = (scene.id, scene.camera_room, visuals.generation());
            if self.level_of != Some(key) {
                visuals.upload(&mut self.renderer, &scene.camera);
                self.level_of = Some(key);
                self.palette_of = visuals.palette_generation();
            } else if self.palette_of != visuals.palette_generation() {
                visuals.upload_palette(&mut self.renderer);
                self.palette_of = visuals.palette_generation();
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
        fn set_level_palette(&mut self, _: &rw_assets::PaletteTexture) {}
        fn draw_level(&mut self, _: LevelView) {
            self.level = true;
            self.order.push("level");
        }
        fn set_atlas(
            &mut self,
            _: &SpriteAtlas,
            _: &[&PngImage],
            _: &[rw_render::TextureSampling],
        ) {
        }
        fn set_stage_texture(
            &mut self,
            _: rw_render::StageTextureId,
            _: &PngImage,
            _: rw_render::TextureSampling,
        ) {
        }
        fn remove_stage_texture(&mut self, _: rw_render::StageTextureId) {}
        fn set_stage_globals(&mut self, _: rw_render::StageGlobals) {}
        fn draw_stage(&mut self, _: &rw_render::StageBatch, _: [f32; 2]) {
            self.order.push("stage");
        }
        fn set_background(&mut self, _: Color) {}
        fn draw_sprites(&mut self, _: &DrawList, layer: Layer, _: [f32; 2]) {
            self.sprite_calls += 1;
            self.order.push(match layer {
                Layer::Shadows => "shadows",
                Layer::BackgroundShortcuts => "background shortcuts",
                Layer::Background => "background",
                Layer::Midground => "midground",
                Layer::Items => "items",
                Layer::Foreground => "foreground",
                Layer::ForegroundLights => "foreground lights",
                Layer::Shortcuts => "shortcuts",
                Layer::Water => "water",
                Layer::GrabShaders => "grab shaders",
                Layer::Bloom => "bloom",
                Layer::Hud => "hud",
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
        fn set_overlay_texture(&mut self, _: u32, _: u32, _: &[u8]) {}
        fn draw_overlay(&mut self, _: rw_render::Overlay) {}
    }

    impl Recorder {
        fn sprite(index: usize, layer: Layer) -> rw_graphics::DrawItem {
            rw_graphics::DrawItem::Sprite(rw_graphics::Sprite {
                index,
                element: "pixel",
                pos: rw_sim::Vec2::ZERO,
                rotation: 0.0,
                scale: rw_sim::Vec2::new(1.0, 1.0),
                anchor: rw_sim::Vec2::new(0.5, 0.5),
                color: SpriteColor::BLACK,
                shader: rw_graphics::Shader::Basic,
                layer,
            })
        }

        /// A slugcat-like list: one sprite in `Midground`, one in `Foreground`.
        fn slugcat_sprites() -> DrawList {
            DrawList {
                items: vec![
                    Self::sprite(0, Layer::Midground),
                    Self::sprite(1, Layer::Foreground),
                ],
            }
        }

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
            let objects: Vec<DrawList> = std::iter::once(sprites.clone())
                .chain(shortcuts.cloned())
                .collect();
            FrameDraw {
                game,
                water,
                water_sprites,
                level,
                player_sprites: true,
                objects: &objects,
                shadows: &[],
                holograms: &[],
                hud_below: &StageBatch::default(),
                screen: [1366.0, 768.0],
                map_over_disc: false,
                layers: RoomLayers::default(),
                camera: Camera::fit([1.0, 1.0], (1366, 768), 0.0),
                alpha: 1.0,
                time: 0.0,
            }
            .queue(&mut r);
            r
        }
    }

    /// The flag's lifecycle: above the disc at construction; a camera move before the map
    /// loaded (the HUD's first update) leaves it; a move after puts it under, for good.
    #[test]
    fn the_map_order_follows_the_maps_first_update() {
        let mut order = MapOrder::new();
        assert!(order.over_disc);
        order.camera_moved();
        assert!(order.over_disc, "a move before the map loads");
        order.map_updated();
        assert!(order.over_disc, "loading adds no room sprites");
        order.camera_moved();
        assert!(!order.over_disc, "a move after it");
        order.camera_moved();
        order.map_updated();
        assert!(!order.over_disc);
    }

    /// The region map's quads are queued before the room's `HUD` sprites (the disc ripples
    /// them) once the order says so, and after them otherwise.
    #[test]
    fn the_map_is_under_the_disc_except_in_the_first_room() {
        let game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let mut map = StageBatch::default();
        map.push_quad(
            rw_render::StageTextureId(1),
            rw_render::StageShader::Basic,
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].map(|(x, y)| {
                rw_render::SpriteVertex {
                    pos: [x, y],
                    uv: [0.0, 0.0],
                    color: [1.0; 4],
                }
            }),
        );
        let disc = DrawList {
            items: vec![Recorder::sprite(0, Layer::Hud)],
        };
        for (over, want) in [(false, ["stage", "hud"]), (true, ["hud", "stage"])] {
            let mut r = Recorder::default();
            let objects = [disc.clone(), Recorder::slugcat_sprites()];
            FrameDraw {
                game: &game,
                water: None,
                water_sprites: None,
                level: Some(LevelView {
                    origin: [0.0, 0.0],
                    size: [1366.0, 768.0],
                    rain: 0.0,
                    light: 1.0,
                    water_level: 1.0,
                }),
                player_sprites: true,
                objects: &objects,
                shadows: &[],
                holograms: &[],
                hud_below: &map,
                screen: [1366.0, 768.0],
                map_over_disc: over,
                layers: RoomLayers::default(),
                camera: Camera::fit([1.0, 1.0], (1366, 768), 0.0),
                alpha: 1.0,
                time: 0.0,
            }
            .queue(&mut r);
            let tail: Vec<&str> = r.order.iter().rev().take(2).rev().copied().collect();
            assert_eq!(tail, want, "map_over_disc {over}: {:?}", r.order);
        }
    }

    #[test]
    fn debug_view_draws_colliders_and_no_sprites() {
        let game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let sprites = Recorder::slugcat_sprites();
        let chunks = game.body().expect("body").chunks.len();
        let r = Recorder::record(&game, None, &sprites);
        assert_eq!(r.sprite_calls, 0);
        assert_eq!(r.circles, chunks, "one circle per body chunk");
        assert_eq!(r.rings, chunks, "one radius ring per body chunk");
        assert!(r.lines >= 1, "the connection rod");
        assert!(!r.level);
    }

    #[test]
    fn debug_view_draws_the_rooms_other_bodies() {
        // A floor along the bottom (cells are column by column, top first) and dense
        // random items, so rocks lie in the room.
        let (w, h) = (20, 12);
        let column = format!("{}1|", "0|".repeat(h - 1));
        let cells = column.repeat(w);
        let text = format!(
            "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{cells}\n"
        );
        let room = Room::parse(&text).expect("test room parses");
        let settings = rw_assets::RoomSettings {
            random_item_density: 1.0,
            ..rw_assets::RoomSettings::ANCESTOR
        };
        let game = Game::new(room, &settings, 1, Some((5, 5)));
        let sprites = Recorder::slugcat_sprites();
        let bodies = game.debug_bodies();
        assert!(!bodies.is_empty(), "items in the room");
        let others: usize = bodies.iter().map(|(b, _, _)| b.chunks.len()).sum();
        let chunks = game.body().expect("body").chunks.len() + others;
        let r = Recorder::record(&game, None, &sprites);
        assert_eq!(r.circles, chunks, "one circle per body chunk");
        assert_eq!(r.rings, chunks, "one radius ring per body chunk");
    }

    #[test]
    fn visual_view_draws_sprites_around_the_level() {
        let game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let sprites = Recorder::slugcat_sprites();
        let level = LevelView {
            origin: [-100.0, -50.0],
            size: [1366.0, 768.0],
            rain: 0.0,
            light: 1.0,
            water_level: 1.0,
        };
        let r = Recorder::record(&game, Some(level), &sprites);
        assert_eq!(r.sprite_calls, 2, "midground and foreground");
        assert_eq!(r.circles, 0);
        assert!(r.level);
    }

    #[test]
    fn water_sprites_after_the_level_and_foreground_flat_water_in_debug() {
        let game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let sprites = Recorder::slugcat_sprites();
        let surface = WaterSurface::flat(400.0, 50.0);
        let water = WaterSprites::new(surface.len(), 1366.0);
        let level = LevelView {
            origin: [0.0, 0.0],
            size: [1366.0, 768.0],
            rain: 0.0,
            light: 1.0,
            water_level: 1.0,
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
        let game = Game::new(open_room(), &rw_assets::RoomSettings::ANCESTOR, 1, None);
        let sprites = Recorder::slugcat_sprites();
        let surface = WaterSurface::flat(400.0, 50.0);
        let water = WaterSprites::new(surface.len(), 1366.0);
        let level = LevelView {
            origin: [0.0, 0.0],
            size: [1366.0, 768.0],
            rain: 0.0,
            light: 1.0,
            water_level: 1.0,
        };
        let layers = [
            Layer::BackgroundShortcuts,
            Layer::Background,
            Layer::Items,
            Layer::Shortcuts,
            Layer::Water,
        ];
        let template = sprites.items[0].clone();
        let shortcuts = DrawList {
            items: layers
                .into_iter()
                .map(|layer| {
                    let mut item = template.clone();
                    item.set_layer(layer);
                    item
                })
                .collect(),
        };
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
