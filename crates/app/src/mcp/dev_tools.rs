//! Dev tools: `spawn_creature`, `spawn_object`, and `screenshot` / `get_tiles` of a
//! realized room other than the camera's.
//!
//! A room shot reads the world and draws with its own renderer and its own room camera,
//! so the camera's renderer uploads, the room camera, the cosmetic stream and the
//! simulation are untouched.

use std::sync::Arc;

use rw_assets::{AbstractObjectType, CreatureType, Room};
use rw_graphics::{CosmeticSprites, DrawList, Layer, ShelterDoorSprites, WeaponSprites};
use rw_render::{
    BodyStatus, Camera, LevelView, Renderer, RoomCamera, RoomLayers, WaterSurface, WgpuRenderer,
};
use rw_sim::Vec2;

use super::Session;
use crate::dev_spawn::SpawnReply;
use crate::frontend::FrontendRenderer;
use crate::game::{DebugBody, Game};
use crate::mcp::params::{ScreenshotParams, SpawnCreatureParams, SpawnObjectParams};
use crate::mcp::reply::Screenshot;
use crate::scene::{Framing, MARGIN, SceneRenderer, SceneView};
use crate::timestep::TICK_RATE;
use crate::visuals::Visuals;

/// A room's tiles and the tiles the player's chunks are on there.
type RoomTiles = (Arc<Room>, Vec<(i32, i32)>);

impl Session {
    /// The global index of room `name`, or the camera's room.
    pub(super) fn room_named(&self, name: Option<&str>) -> Result<i32, String> {
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

    /// Adds a new rock or spear at a tile (`dev_spawn_object`), realized there when its
    /// room is loaded.
    ///
    /// # Errors
    /// Without a game, for a kind other than a rock or spear, an unknown room, or a tile
    /// outside a realized room.
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

    /// Draws realized room `room` (not the camera's) as camera `params.camera` would
    /// show it.
    pub(super) fn room_screenshot(
        &mut self,
        room: i32,
        params: &ScreenshotParams,
        level: bool,
        scale: f32,
    ) -> Result<Screenshot, String> {
        let scene = self.scene()?;
        let game = scene.game();
        let map = game
            .sim()
            .world()
            .realized(room)
            .map(|r| Arc::clone(&r.room))
            .ok_or_else(|| format!("{} is not realized", Self::room_name(scene, room)))?;
        let camera = params.camera.unwrap_or(0);
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
            Some(v)
        } else {
            None
        };
        let shot = RoomShot {
            room,
            map,
            camera: cam,
            visuals,
        };
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

/// A realized room seen from one of its cameras, apart from the scene's.
struct RoomShot {
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

    fn draw(&self, game: &Game, r: &mut WgpuRenderer, view: SceneView, sprites: bool) {
        r.set_room(&self.map);
        let time = (game.ticks() as f32 + 1.0) / TICK_RATE as f32;
        let level: Option<LevelView> = self.visuals.as_ref().map(|v| {
            v.upload(r, &self.camera);
            Visuals::view(&self.camera, 1.0, time)
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
        r.begin_frame(camera);
        r.draw_room(RoomLayers {
            tiles: view.layers.tiles && level.is_none(),
            ..view.layers
        });
        if let Some(level) = level {
            for layer in [Layer::Background, Layer::Midground, Layer::Items] {
                for list in &items {
                    r.draw_sprites(list, layer, level.origin);
                }
            }
            r.draw_level(level);
            for list in &items {
                r.draw_sprites(list, Layer::Foreground, level.origin);
            }
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
