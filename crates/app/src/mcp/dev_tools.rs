//! Dev tools: `spawn_creature`, `spawn_object`, and `screenshot` / `get_tiles` of a
//! realized room other than the camera's.
//!
//! A room shot reads the world and draws with its own renderer and its own room camera,
//! so the camera's renderer uploads, the room camera, the cosmetic stream and the
//! simulation are untouched.
//!
//! Mirrors: - (port-only: MCP server)

use std::sync::Arc;

use rw_assets::{AbstractObjectType, CreatureType, Room};
use rw_graphics::{CosmeticSprites, DrawList, Layer, ShelterDoorSprites, WeaponSprites};
use rw_render::{
    BodyStatus, Camera, LevelView, Renderer, RoomCamera, RoomLayers, WaterSurface, WgpuRenderer,
};
use rw_sim::Vec2;

use super::Session;
use crate::death_fall::DeathFallSprite;
use crate::dev_spawn::{RainTimerEdit, SpawnReply, TeleportReply};
use crate::frontend::FrontendRenderer;
use crate::game::{DebugBody, Game};
use crate::mcp::params::{
    ScreenshotParams, SetRainTimerParams, SpawnCreatureParams, SpawnObjectParams,
    TeleportPlayerParams,
};
use crate::mcp::reply::{RainState, Screenshot};
use crate::scene::{Framing, MARGIN, SceneRenderer, SceneView};
use crate::timestep::TICK_RATE;
use crate::visuals::Visuals;
use crate::water_light::WaterLight;

/// A room's tiles and the tiles the player's chunks are on there.
type RoomTiles = (Arc<Room>, Vec<(i32, i32)>);

impl Session {
    /// The global index of room `name`, or the camera's room.
    pub(in crate::mcp) fn room_named(&self, name: Option<&str>) -> Result<i32, String> {
        let sim = self.scene()?.game().sim();
        match name {
            Some(name) => sim
                .world()
                .room_index(name)
                .ok_or_else(|| format!("no room {name} in this world")),
            None => Ok(sim.camera_room()),
        }
    }

    /// Adds a new creature at a tile (`dev_spawn_creature`), realized there when its
    /// room is loaded and the port has its body.
    ///
    /// # Errors
    /// Without a game, for an unknown type or room, or a tile outside a realized room.
    pub fn spawn_creature(&mut self, params: &SpawnCreatureParams) -> Result<SpawnReply, String> {
        let room = self.room_named(params.room.as_deref())?;
        let mods = self.scene()?.game().sim().world().mods();
        let kind = CreatureType::from_value(&params.kind, mods)
            .ok_or_else(|| format!("unknown creature type {:?}", params.kind))?;
        let [x, y] = params.tile;
        self.scene_or_err()?
            .game_mut()
            .spawn_creature(kind, room, (x, y), params.seed_id)
    }

    /// Edits the rain timer (`edit_rain_timer`).
    ///
    /// # Errors
    /// Without a game.
    pub fn set_rain_timer(&mut self, params: &SetRainTimerParams) -> Result<RainState, String> {
        let edit = RainTimerEdit {
            cycle_length: params.cycle_length,
            restart: params.restart.unwrap_or(false),
            stop: params.stop.unwrap_or(false),
            timer: params.timer,
            shift: params.shift,
            paused: params.paused,
            speed: params.speed,
        };
        let game = self.scene_or_err()?.game_mut();
        game.edit_rain_timer(&edit);
        Ok(RainState::of(game.sim()))
    }

    /// Sends the player through the shortcut lobby to an exit of a room of this world
    /// (`dev_teleport_player`).
    ///
    /// # Errors
    /// Without a game, for an unknown room or node, or a player that cannot travel.
    pub fn teleport_player(
        &mut self,
        params: &TeleportPlayerParams,
    ) -> Result<TeleportReply, String> {
        let room = self.room_named(Some(&params.room))?;
        self.scene_or_err()?
            .game_mut()
            .teleport_player(room, params.node)
    }

    /// Adds a new item at a tile (`dev_spawn_object`): a rock or spear realized there
    /// when its room is loaded, another kind abstract.
    ///
    /// # Errors
    /// Without a game, for a kind that is not a spawnable item, an unknown room, or a
    /// tile outside a realized room.
    pub fn spawn_object(&mut self, params: &SpawnObjectParams) -> Result<SpawnReply, String> {
        let room = self.room_named(params.room.as_deref())?;
        let mods = self.scene()?.game().sim().world().mods();
        let kind = AbstractObjectType::from_value(&params.kind, mods)
            .ok_or_else(|| format!("unknown object type {:?}", params.kind))?;
        let [x, y] = params.tile;
        self.scene_or_err()?.game_mut().spawn_object(
            kind,
            room,
            (x, y),
            params.explosive.unwrap_or(false),
        )
    }

    /// Realized room `room` seen from its camera `camera`, with its level graphic in the
    /// level view.
    pub(in crate::mcp) fn room_shot(
        &self,
        room: i32,
        camera: usize,
        level: bool,
    ) -> Result<RoomShot, String> {
        let scene = self.scene()?;
        let game = scene.game();
        let map = game
            .sim()
            .world()
            .realized(room)
            .map(|r| Arc::clone(&r.room))
            .ok_or_else(|| format!("{} is not realized", Self::room_name(scene, room)))?;
        if camera >= map.cameras().len() {
            return Err(format!(
                "{} has cameras 0..{}",
                map.name(),
                map.cameras().len()
            ));
        }
        let mut cam = RoomCamera::new(map.cameras(), scene.camera().screen_size());
        cam.cut_to(camera);
        let visuals = if level {
            let timeline = game.sim().campaign().timeline.value();
            let mut v = Visuals::load(&self.assets, &self.resources, &map, timeline)
                .map_err(|e| format!("no level graphic for {}: {e}", map.name()))?;
            v.show(camera);
            v.fade(&game.sim().world().rain_cycle);
            Some(v)
        } else {
            None
        };
        Ok(RoomShot {
            room,
            map,
            camera: cam,
            visuals,
        })
    }

    /// Draws realized room `room` (not the camera's) as camera `params.camera` would
    /// show it.
    pub(super) fn room_screenshot(
        &mut self,
        room: i32,
        params: &ScreenshotParams,
        level: bool,
        scale: f32,
    ) -> Result<Screenshot, String> {
        let shot = self.room_shot(room, params.camera.unwrap_or(0), level)?;
        let (framing, size) = match params.crop_radius {
            Some(r) if r > 0.0 => {
                let centre = params.crop_center.unwrap_or_else(|| shot.middle());
                let origin = [centre[0] - r, centre[1] - r];
                let size = [2.0 * r, 2.0 * r];
                (Framing::Rect { origin, size }, size)
            }
            Some(_) => return Err("crop_radius must be positive".to_owned()),
            None => (Framing::Auto, shot.auto_size()),
        };
        let pixels = size.map(|s| (s * scale).round().clamp(1.0, super::MAX_IMAGE_SIDE) as u32);
        if self.room_renderer.is_none() {
            let renderer = WgpuRenderer::headless((pixels[0], pixels[1]))
                .map_err(|e| format!("no GPU renderer: {e}"))?;
            self.room_renderer = Some(FrontendRenderer::new(SceneRenderer::new(
                renderer,
                self.sprites.clone(),
            )));
        }
        let (Some(scene), Some(target)) = (self.scene.as_ref(), self.room_renderer.as_mut()) else {
            return Err("no game loaded; call load_room or new_game first".to_owned());
        };
        target.scene.renderer.set_frame_size((pixels[0], pixels[1]));
        let view = SceneView {
            level,
            layers: RoomLayers {
                tiles: true,
                grid: params.grid.unwrap_or(false),
            },
            framing,
        };
        shot.draw(
            scene.game(),
            &mut target.scene.renderer,
            view,
            self.sprites.is_some(),
        );
        Self::encode(target, params)
    }

    /// The tiles of `room` and the tiles the player's chunks are on there.
    pub(super) fn room_tiles(&self, room: i32) -> Result<RoomTiles, String> {
        let scene = self.scene()?;
        let sim = scene.game().sim();
        let map = sim
            .world()
            .realized(room)
            .map(|r| Arc::clone(&r.room))
            .ok_or_else(|| format!("{} is not realized", Self::room_name(scene, room)))?;
        let player = sim
            .player()
            .filter(|p| !p.in_shortcut() && sim.player_pos().room == room)
            .map(|p| p.chunks().iter().map(|c| c.pos.tile()).collect())
            .unwrap_or_default();
        Ok((map, player))
    }
}

/// The camera's own sprites for a room shot.
#[derive(Default)]
pub(in crate::mcp) struct CameraLists {
    pub(in crate::mcp) water_light: Option<DrawList>,
    pub(in crate::mcp) death_fall: Option<DrawList>,
    /// The full-screen effect and the container it sits in.
    pub(in crate::mcp) effect: Option<(Layer, DrawList)>,
}

impl CameraLists {
    fn draw_effect(&self, r: &mut WgpuRenderer, layer: Layer, origin: [f32; 2]) {
        if let Some((_, list)) = self.effect.as_ref().filter(|(l, _)| *l == layer) {
            r.draw_sprites(list, layer, origin);
        }
    }
}

/// A realized room seen from one of its cameras, apart from the scene's.
pub(in crate::mcp) struct RoomShot {
    room: i32,
    map: Arc<Room>,
    camera: RoomCamera,
    /// The level graphic (the level view only).
    visuals: Option<Visuals>,
}

impl RoomShot {
    fn middle(&self) -> [f32; 2] {
        [self.map.pixel_width() / 2.0, self.map.pixel_height() / 2.0]
    }

    /// The camera's screen in the level view, else the room with its margin.
    fn auto_size(&self) -> [f32; 2] {
        if self.visuals.is_some() {
            self.camera.screen_size()
        } else {
            [
                self.map.pixel_width() + 2.0 * MARGIN,
                self.map.pixel_height() + 2.0 * MARGIN,
            ]
        }
    }

    /// The room's item sprites (the shelter door, rocks and spears, sparks), drawn as
    /// the scene draws them; the weapons' cosmetic draws come from a copy of the stream.
    fn item_lists(&self, game: &Game, black: rw_graphics::Color) -> Vec<DrawList> {
        let sim = game.sim();
        let world = sim.world();
        let mut lists: Vec<DrawList> = sim
            .shelter_door(self.room)
            .map(|d| d.draw(1.0))
            .into_iter()
            .collect();
        let water = world.realized(self.room).and_then(|r| r.water.as_ref());
        let mut weapons = world.room_weapons(self.room);
        weapons.sort_unstable();
        let mut cosmetic = game.cosmetic_rng().clone();
        lists.extend(weapons.iter().filter_map(|&(_, id)| {
            let o = world.object(id);
            o.realized
                .as_deref()?
                .draw(o.id, self.room, 1.0, black, water, &mut cosmetic)
        }));
        if let Some(r) = world.realized(self.room) {
            let mut shown: Vec<_> = r.cosmetics.iter().filter(|c| !c.slated).collect();
            shown.sort_by_key(|c| c.update_order);
            let none = rw_graphics::NoLizards(black);
            lists.extend(
                shown
                    .iter()
                    .map(|c| c.draw(&r.room, 1.0, &none, &mut cosmetic)),
            );
        }
        lists
    }

    /// The room's creatures and items as debug bodies: an unviewed room's creatures
    /// have no graphics modules (the game builds them only in viewed rooms).
    fn debug_bodies<'a>(
        &self,
        game: &'a Game,
    ) -> Vec<(&'a rw_sim::PhysicalObject, BodyStatus, DebugBody)> {
        use rw_sim::creature::RealizedCreature as R;
        let world = game.sim().world();
        let mut list = Vec::new();
        for (_, c) in world.creatures() {
            let (body, stun, dead, kind) = match &c.realized {
                Some(R::Lizard(l)) if l.room == Some(self.room) => {
                    (&l.body, l.stunned(), l.dead, DebugBody::Lizard)
                }
                Some(R::Fly(f)) if f.room == Some(self.room) => {
                    (&f.body, f.stunned(), f.dead, DebugBody::Fly)
                }
                Some(R::Overseer(b)) if b.in_room && c.pos.room == self.room => {
                    (&b.body, b.stun >= 10, b.dead, DebugBody::Overseer)
                }
                _ => continue,
            };
            let status = BodyStatus {
                stunned: stun && !dead,
                dead,
            };
            list.push((body, status, kind));
        }
        list
    }

    /// The room's water surface for the debug view.
    fn water_surface(&self, game: &Game) -> Option<WaterSurface> {
        let water = game.sim().world().realized(self.room)?.water.as_ref()?;
        let front = water.draw_points(1.0).map(Vec2::to_array).collect();
        let back = water.back_draw_points(1.0).map(Vec2::to_array).collect();
        Some(WaterSurface::from_rows(front, back, water.still_level()))
    }

    /// The room's `DeathFallGraphic`, as the viewing camera's `Update` leaves it: its
    /// height under a camera 0 showing this camera position of the room.
    fn death_fall<'a>(&self, game: &'a Game) -> Option<(&'a rw_sim::DeathFall, f32)> {
        let fall = game
            .sim()
            .world()
            .realized(self.room)?
            .death_fall
            .as_ref()?;
        let current = self.camera.current();
        let view = rw_sim::CameraView {
            room: self.room,
            current,
            next: current,
        };
        // A camera drawing the room runs `InitiateSprites` first.
        let mut initiated = fall.clone();
        initiated.initiate_sprites(&self.map);
        Some((fall, initiated.height_under(&self.map, self.room, &view)))
    }

    /// The room's `_waterLevel` line: its water + 100, else its death fall's height +
    /// 180, else 0; `None` in gates and shelters.
    fn water_line(&self, game: &Game) -> Option<f32> {
        let world = game.sim().world();
        let data = world.abstract_room(self.room)?;
        if data.gate() || data.shelter() {
            return None;
        }
        let water = world.realized(self.room)?.water.as_ref();
        Some(match (water, self.death_fall(game)) {
            (Some(w), _) => w.still_level() + 100.0,
            (None, Some((_, height))) => height + 180.0,
            (None, None) => 0.0,
        })
    }

    /// The room's `WaterLight` and `DeathFallGraphic` sprites and the camera's
    /// full-screen effect, as the camera viewing the room would draw them, each with the
    /// container it joins.
    pub(in crate::mcp) fn camera_lists(
        &self,
        game: &Game,
        screen_width: f32,
        origin: Vec2,
        lightning: Option<&rw_sim::LightningFrame>,
    ) -> CameraLists {
        let mut lists = CameraLists::default();
        let Some(visuals) = &self.visuals else {
            return lists;
        };
        let world = game.sim().world();
        let Some(realized) = world.realized(self.room) else {
            return lists;
        };
        if let (Some(light), Some(water)) = (
            WaterLight::new(&self.map, realized.water.as_ref(), &realized.settings),
            realized.water.as_ref(),
        ) {
            lists.water_light = Some(light.draw_list(water, visuals.room_palette().darkness));
        }
        if let Some((fall, height)) = self.death_fall(game) {
            lists.death_fall = Some(DeathFallSprite::draw_list(
                fall,
                height,
                screen_width,
                origin,
                1.0,
            ));
        }
        if let Some(effect) = visuals.full_screen_effect() {
            let strength = visuals.effect_alpha(&effect, &world.rain_cycle, lightning);
            lists.effect = Some((
                effect.layer,
                effect.draw_list(screen_width, origin, strength),
            ));
        }
        lists
    }

    fn draw(&self, game: &Game, r: &mut WgpuRenderer, view: SceneView, sprites: bool) {
        r.set_room(&self.map);
        let time = (game.ticks() as f32 + 1.0) / TICK_RATE as f32;
        // The room's flash, flickering on a copy of the cosmetic stream.
        let lightning = self.visuals.as_ref().and_then(|v| {
            let l = game.sim().world().realized(self.room)?.lightning.as_ref()?;
            Some(v.lightning_frame(l, 1.0, &mut game.cosmetic_rng().clone()))
        });
        let level: Option<LevelView> = self.visuals.as_ref().map(|v| {
            v.upload(r, &self.camera);
            if let Some(frame) = &lightning {
                r.set_level_palette(&v.lightning_palette(frame));
            }
            v.view(
                &self.camera,
                1.0,
                time,
                &game.sim().world().rain_cycle,
                lightning.as_ref(),
                self.water_line(game),
            )
        });
        let camera = match (view.framing, &level) {
            (Framing::Rect { origin, size }, _) => Camera::window(origin, size, r.size()),
            (Framing::Auto, Some(level)) => Camera::window(level.origin, level.size, r.size()),
            (Framing::Auto, None) => Camera::fit(
                [self.map.pixel_width(), self.map.pixel_height()],
                r.size(),
                MARGIN,
            ),
        };
        let black = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::black_color);
        let items = match &level {
            Some(_) if sprites => self.item_lists(game, black),
            _ => Vec::new(),
        };
        let camera_lists = match &level {
            Some(level) if sprites => self.camera_lists(
                game,
                level.size[0],
                Vec2::from(level.origin),
                lightning.as_ref(),
            ),
            _ => CameraLists::default(),
        };
        r.begin_frame(camera);
        r.draw_room(RoomLayers {
            tiles: view.layers.tiles && level.is_none(),
            ..view.layers
        });
        if let Some(level) = level {
            // The room's `SunBlocker`s, then its spot lights, in `Shadows`.
            let blockers = self.visuals.as_ref().map_or(0, Visuals::sun_blockers);
            for _ in 0..blockers {
                let list = rw_graphics::SunBlocker::draw(Vec2::from(level.origin));
                r.draw_sprites(&list, Layer::Shadows, level.origin);
            }
            if let Some(realized) = game.sim().world().realized(self.room) {
                for spot in &realized.spot_lights {
                    r.draw_sprites(
                        &rw_graphics::SpotLightSprites::draw(spot),
                        Layer::Shadows,
                        level.origin,
                    );
                }
            }
            for layer in [Layer::Background, Layer::Midground, Layer::Items] {
                for list in &items {
                    r.draw_sprites(list, layer, level.origin);
                }
            }
            r.draw_level(level);
            // `Foreground`: the `WaterLight` joined it first, the full-screen effect
            // (`Fog`) is raised on top.
            if let Some(list) = &camera_lists.water_light {
                r.draw_sprites(list, Layer::Foreground, level.origin);
            }
            for list in &items {
                r.draw_sprites(list, Layer::Foreground, level.origin);
            }
            camera_lists.draw_effect(r, Layer::Foreground, level.origin);
            // `Bloom`: the death fall, then the full-screen effect.
            if let Some(list) = &camera_lists.death_fall {
                r.draw_sprites(list, Layer::Bloom, level.origin);
            }
            camera_lists.draw_effect(r, Layer::Bloom, level.origin);
        }
        for (body, status, kind) in self.debug_bodies(game) {
            let style = status.style(time).tinted(kind.color(), status.dead);
            r.draw_object_styled(body, 1.0, style);
        }
        if level.is_none() {
            for o in game.sim().world().objects() {
                if let Some(w) = o.1.realized.as_deref().filter(|w| w.room == self.room) {
                    let style = BodyStatus::ALIVE
                        .style(time)
                        .tinted(DebugBody::Item.color(), false);
                    r.draw_object_styled(&w.body, 1.0, style);
                }
            }
        }
        let sim = game.sim();
        if let Some(p) = sim
            .player()
            .filter(|p| !p.in_shortcut() && sim.player_pos().room == self.room)
        {
            r.draw_object(&p.body, 1.0, game.status(), time);
        }
        if level.is_none()
            && let Some(water) = self.water_surface(game)
        {
            r.draw_water(&water);
        }
    }
}
