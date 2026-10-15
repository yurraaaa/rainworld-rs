//! `HUD.Map` of a story game in its base-game branches: the region map image drawn in
//! three depth layers through the `Map` shaders, revealed where the discovery texture
//! says the player has been, panned with the directions and layered with jump and
//! throw while the map button is held; the player marker, the gate, shelter, death and
//! karma flower markers, the room connection lines, the swarm-room circles and the
//! Hunter's cycle label. Owned by the player, or by the sleep and death screens (which
//! show it at once, without discovery). Behaviour: `rainworld-re/notes/hud.md`
//! section 5.

mod connection;
mod data;
mod markers;

#[cfg(test)]
pub(crate) mod tests;

use std::sync::Arc;

use rw_assets::PngImage;
use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_menu::Font;
use rw_menu::{
    Alignment, AtlasManager, HslColor, Label, MenuColor, NodeId, Sprite, StageColor, StageTree,
};
use rw_random::UnityRandom;
use rw_render::{MapGlobals, StageShader, StageTextureId, TextureFilter, TextureSampling};
use rw_sim::map_discovery::{DIAGONALS, DISCOVER_RESOLUTION};
use rw_sim::{DiscoveryTexture, FloatExt, Input, MapDiscovery, MapRoom, UnityTexture, Vec2};

use crate::{CustomFloat, CustomInt, HudCircle, HudOwnerType, SnapGraphic};
pub use connection::{ConnectionView, MapConnection};
pub use data::{GateData, GateRequirement, MapData, MapDataRoom, ShelterData, SwarmRoomData};

pub use markers::{FadeInMarker, MarkerKind, MarkerView};

/// `Custom.eightDirectionsAndZero`.
const EIGHT_DIRECTIONS_AND_ZERO: [(i32, i32); 9] = [
    (0, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// `Map.MapScale`.
const MAP_SCALE: f32 = 4.0;

/// What the map reads of its owner, the player, each tick (`IOwnAHUD`).
#[derive(Debug, Clone, Default)]
pub struct MapOwner {
    /// `MapDiscoveryActive`.
    pub discovery_active: bool,
    /// `MapOwnerInRoomPosition`.
    pub in_room_pos: Vec2,
    /// `MapOwnerRoom`.
    pub room: i32,
    /// `room.CompleteDarkness(firstChunk.pos, 0, 0.95, false)` in the player's room.
    pub complete_darkness: bool,
    /// `MapInput`.
    pub input: Input,
    /// `roomsVisited` of the region, read when the discovery texture is made.
    pub rooms_visited: Vec<String>,
    /// `DateTime.Now.Ticks`, stored as `mapLastUpdatedTime` when the map loads.
    pub now_ticks: i64,
}

/// `Map.SwarmCircle`: a blue ring that expands and fades over a hive.
#[derive(Debug, Clone)]
pub struct SwarmCircle {
    pub circle: HudCircle,
    pub pos: Vec2,
    pub room: i32,
    pub rad: f32,
    pub rad_speed: f32,
    pub life: f32,
}

/// `Map.CycleLabel` (the Hunter's cycles left).
#[derive(Debug, Clone)]
pub struct CycleLabel {
    label: NodeId,
    fade: f32,
    last_fade: f32,
    pub reveal_timer: i32,
    /// `red`: counting up while the cycles ran out, -1 otherwise.
    red: i32,
}

impl CycleLabel {
    /// At (20.01, 754.1), left aligned, medium grey, transparent.
    fn new(
        stage: &mut StageTree,
        container: NodeId,
        font: Arc<Font>,
        text: &str,
        red: bool,
    ) -> Self {
        let mut label = Label::new(font, text);
        label.transform.x = 20.01;
        label.transform.y = 754.1;
        label.set_alignment(Alignment::Left);
        label.color = MenuColor::MediumGrey.rgb();
        label.transform.alpha = 0.0;
        Self {
            label: stage.add_label(container, label),
            fade: 0.0,
            last_fade: 0.0,
            reveal_timer: 0,
            red: if red { 1 } else { -1 },
        }
    }

    fn update(&mut self, map_fade: f32) {
        self.last_fade = self.fade;
        let step = if map_fade > 0.9 {
            1
        } else if map_fade < 0.05 {
            -20
        } else {
            -1
        };
        self.reveal_timer = (self.reveal_timer + step).int_clamp(0, 120);
        let to = if self.reveal_timer > 30 { 1.0 } else { 0.0 };
        self.fade = self.fade.lerp_and_tick(to, 0.04, 1.0 / 60.0);
        if self.red > 0 {
            self.red += 1;
        }
    }

    fn draw(&self, stage: &mut StageTree, t: f32, alpha: f32) {
        let label = stage.label_mut(self.label);
        label.transform.alpha = (alpha * self.last_fade.lerp(self.fade, t)).s_curve(0.7);
        if self.red > 0 {
            let pulse = 0.5 + 0.5 * ((self.red as f32 + t) / 7.0).sin();
            label.color = MenuColor::MediumGrey
                .rgb()
                .lerp(StageColor::rgb(1.0, 0.0, 0.0), pulse);
        }
    }
}

/// The sprites, textures and markers of a loaded map (`mapLoaded`).
#[derive(Debug)]
struct MapView {
    /// The map atlas, `map_<region>`.
    element: String,
    /// The map texture's size in pixels.
    texture_size: Vec2,
    background_black: NodeId,
    sprites: [NodeId; 3],
    /// Not on a death screen's map.
    player_marker_fade: Option<NodeId>,
    player_marker: Option<HudCircle>,
    in_front: NodeId,
    container: NodeId,
    cycle_label: Option<CycleLabel>,
    /// `revealTexture` (`None` until the discovery loads).
    reveal: Option<UnityTexture>,
    /// The reveal texture as last applied, waiting for the renderer.
    applied: Option<PngImage>,
    fog: Option<StageTextureId>,
}

/// `HUD.Map`.
#[derive(Debug)]
pub struct Map {
    pub data: MapData,
    pub owner: HudOwnerType,
    view: Option<MapView>,
    /// `mapSize`: the texture's width and a third of its height.
    pub map_size: Vec2,
    pub fade: f32,
    pub last_fade: f32,
    pub fade_counter: i32,
    pub visible: bool,
    pub counter: i32,
    pub visible_counter: i32,
    pub invisible_counter: i32,
    pub disc_loaded: bool,
    pub pan_pos: Vec2,
    pub last_pan_pos: Vec2,
    pub pan_vel: Vec2,
    pub speed_up: f32,
    pub layer: i32,
    pub depth: f32,
    pub last_depth: f32,
    layer_button_a: bool,
    layer_button_b: bool,
    pub reset_reveal_counter: i32,
    pub player_stand_pos: Vec2,
    pub last_on_texture_pos: (i32, i32),
    pub reveal_pixels: Vec<(i32, i32)>,
    pub reveal_fade_pixels: Vec<(i32, i32)>,
    pub markers: Vec<FadeInMarker>,
    /// `notRevealedFadeMarkers`: indices into `markers`.
    not_revealed: Vec<usize>,
    pub connections: Vec<MapConnection>,
    pub swarm_circles: Vec<SwarmCircle>,
    screen_size: Vec2,
    /// The owner's position and room at the last update, for drawing.
    owner_pos: Vec2,
    owner_room: i32,
}

impl Map {
    /// The constructor and the first update's loading (synchronous in the game): the
    /// connection lines; the gate, shelter, death and flower markers in `in_front` (the
    /// map's `inFrontContainer`), then on `container` the black background, the three
    /// map sprites, the cycle label, the connection sprites and the player marker (none
    /// for a death screen). Without a map image the map never loads, as in the game.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data: MapData,
        owner: HudOwnerType,
        stage: &mut StageTree,
        container: NodeId,
        in_front: NodeId,
        atlases: &mut AtlasManager,
        screen_size: [f32; 2],
        cycle_label: Option<(Arc<Font>, String, bool)>,
    ) -> Self {
        let screen_size = Vec2::new(screen_size[0], screen_size[1]);
        let mut map = Self {
            connections: Self::connections_of(&data),
            data,
            owner,
            view: None,
            map_size: Vec2::ZERO,
            fade: 0.0,
            last_fade: 0.0,
            fade_counter: 0,
            visible: false,
            counter: 0,
            visible_counter: 0,
            invisible_counter: 0,
            disc_loaded: false,
            pan_pos: Vec2::ZERO,
            last_pan_pos: Vec2::ZERO,
            pan_vel: Vec2::ZERO,
            speed_up: 0.0,
            layer: 1,
            depth: 0.0,
            last_depth: 0.0,
            layer_button_a: false,
            layer_button_b: false,
            reset_reveal_counter: -1,
            player_stand_pos: Vec2::ZERO,
            last_on_texture_pos: (-1, -1),
            reveal_pixels: Vec::new(),
            reveal_fade_pixels: Vec::new(),
            markers: Vec::new(),
            not_revealed: Vec::new(),
            swarm_circles: Vec::new(),
            screen_size,
            owner_pos: Vec2::ZERO,
            owner_room: 0,
        };
        for gate in &map.data.gates {
            // A new game's save state: open when the karma suffices.
            let open = gate
                .karma
                .is_none_or(|k| k.karma().is_some_and(|n| n - 1 <= map.data.current_karma));
            let marker = FadeInMarker::gate(stage, in_front, gate.room, gate.karma, open);
            map.markers.push(marker);
        }
        for shelter in &map.data.shelters {
            let marker = FadeInMarker::shelter(stage, in_front, shelter.room, shelter.marker_pos);
            map.markers.push(marker);
        }
        if matches!(
            owner,
            HudOwnerType::Player | HudOwnerType::SleepScreen | HudOwnerType::DeathScreen
        ) {
            for d in &map.data.death_positions {
                // Valid and in this region.
                let Some(room) = map.data.index_of(&d.room) else {
                    continue;
                };
                let pos = Vec2::new(d.x as f32, d.y as f32) * 20.0;
                let marker = FadeInMarker::death(stage, in_front, room, pos, d.node);
                map.markers.push(marker);
            }
        }
        if let Some(flower) = map.data.karma_flower_pos {
            let pos = Vec2::new(flower.tile.0 as f32, flower.tile.1 as f32) * 20.0;
            let marker = FadeInMarker::flower(stage, in_front, flower.room, pos);
            map.markers.push(marker);
        }
        map.reset_not_revealed_markers();
        map.load(stage, container, in_front, atlases, cycle_label);
        map
    }

    /// `LoadConnectionPositions`: the file's lines between rooms the world connects.
    fn connections_of(data: &MapData) -> Vec<MapConnection> {
        data.connection_lines
            .iter()
            .filter(|c| data.has_connection(&c.room_a, &c.room_b))
            .filter_map(|c| {
                let a = data.index_of(&c.room_a)?;
                let b = data.index_of(&c.room_b)?;
                (a > 0 && b > 0)
                    .then(|| MapConnection::new([a, b], [c.pos_a, c.pos_b], [c.dir_a, c.dir_b]))
            })
            .collect()
    }

    /// The first update's `mapLoaded` part.
    fn load(
        &mut self,
        stage: &mut StageTree,
        container: NodeId,
        in_front: NodeId,
        atlases: &mut AtlasManager,
        cycle_label: Option<(Arc<Font>, String, bool)>,
    ) {
        let Some(image) = self.data.image.take() else {
            return;
        };
        let element = format!("map_{}", self.data.region);
        let texture_size = Vec2::new(image.width() as f32, image.height() as f32);
        let sampling = TextureSampling {
            filter: TextureFilter::Bilinear,
            clamp: true,
        };
        if let Err(e) = atlases.add_image_atlas(&element, image, sampling) {
            log::warn!("map atlas {element}: {e}");
        }
        let mut black = Sprite::new(AtlasManager::WHITE);
        black.color = StageColor::BLACK;
        black.anchor = [0.0, 0.0];
        black.transform.scale_x = self.screen_size.x / 16.0;
        black.transform.scale_y = 48.0;
        black.transform.visible = false;
        let background_black = stage.add_sprite(container, black);
        let sprites = self.data.sky_layers.map(|sky| {
            let mut s = Sprite::new(&element);
            s.shader = if sky {
                StageShader::MapAerial
            } else {
                StageShader::Map
            };
            s.transform.visible = false;
            stage.add_sprite(container, s)
        });
        let cycle_label = cycle_label
            .map(|(font, text, red)| CycleLabel::new(stage, container, font, &text, red));
        self.map_size = Vec2::new(texture_size.x, (texture_size.y as i32 / 3) as f32);
        for i in 0..self.connections.len() {
            let c = &self.connections[i];
            let a =
                self.data
                    .map_room(c.room_a)
                    .on_texture_pos(c.room_pos_a(), self.map_size, false);
            let b =
                self.data
                    .map_room(c.room_b)
                    .on_texture_pos(c.room_pos_b(), self.map_size, false);
            let across = self.data.layer_of_room(c.room_a) != self.data.layer_of_room(c.room_b);
            self.connections[i].initiate_sprites(stage, container, a.distance(b), across);
        }
        self.pan_pos = self.map_size / 2.0;
        let (player_marker_fade, player_marker) = if self.owner == HudOwnerType::DeathScreen {
            (None, None)
        } else {
            let mut fade = Sprite::new(AtlasManager::WHITE);
            fade.color = StageColor::BLACK;
            fade.shader = StageShader::FlatLight;
            fade.transform.visible = false;
            let fade = stage.add_sprite(container, fade);
            let marker = HudCircle::new(stage, container, SnapGraphic::FoodCircleA, 1);
            (Some(fade), Some(marker))
        };
        self.view = Some(MapView {
            element,
            texture_size,
            background_black,
            sprites,
            player_marker_fade,
            player_marker,
            in_front,
            container,
            cycle_label,
            reveal: None,
            applied: None,
            fog: None,
        });
    }

    /// `ResetNotRevealedMarkers`.
    fn reset_not_revealed_markers(&mut self) {
        self.not_revealed.clear();
        for (i, m) in self.markers.iter_mut().enumerate() {
            m.set_invisible();
            self.not_revealed.push(i);
        }
    }

    #[must_use]
    pub fn loaded(&self) -> bool {
        self.view.is_some()
    }

    /// `Map.Update`.
    pub fn update(
        &mut self,
        owner: &MapOwner,
        reveal_map: bool,
        hide_general_hud: bool,
        stage: &mut StageTree,
        discovery: &mut MapDiscovery,
        rng: &mut UnityRandom,
    ) {
        self.counter += 1;
        self.owner_pos = owner.in_room_pos;
        self.owner_room = owner.room;
        if self.visible {
            self.invisible_counter = 0;
            self.visible_counter += 1;
        } else {
            self.visible_counter = 0;
            self.invisible_counter += 1;
        }
        if self.view.is_some() && !self.disc_loaded {
            self.load_discovery(owner, discovery);
        }
        self.last_pan_pos = self.pan_pos;
        self.last_fade = self.fade;
        self.last_depth = self.depth;
        let layer = self.layer as f32;
        self.depth = self.depth.lerp(layer, 0.1);
        self.depth = if self.depth < layer {
            (self.depth + 0.02).min(layer)
        } else {
            (self.depth - 0.02).max(layer)
        };
        let brake_at = 2.5 + 4.0 * self.speed_up.inverse_lerp(0.5, 1.0).s_curve(0.65);
        self.pan_vel *= self.pan_vel.length().lerp_map(0.0, brake_at, 1.0, 0.85);
        for i in (0..self.swarm_circles.len()).rev() {
            if self.swarm_circles[i].life < 0.0 {
                let gone = self.swarm_circles.remove(i);
                stage.remove_from_container(gone.circle.sprite);
            } else {
                self.update_swarm_circle(i);
            }
        }
        for m in self.markers.iter_mut().rev() {
            m.update();
        }
        if self.view.is_some() && self.disc_loaded {
            self.update_loaded(owner, reveal_map, hide_general_hud, stage, discovery, rng);
        }
        if self.fade == 0.0 && self.last_fade == 0.0 && !self.swarm_circles.is_empty() {
            for c in self.swarm_circles.drain(..) {
                stage.remove_from_container(c.circle.sprite);
            }
        }
        let fade = self.fade;
        if let Some(label) = self.view.as_mut().and_then(|v| v.cycle_label.as_mut()) {
            label.update(fade);
        }
    }

    /// The `!discLoaded` part: a discovery texture of the wrong size is remade, a missing
    /// one made from the visited rooms; then the reveal texture.
    fn load_discovery(&mut self, owner: &MapOwner, discovery: &mut MapDiscovery) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        let w = (view.texture_size.x / DISCOVER_RESOLUTION) as i32;
        let h = (view.texture_size.y / DISCOVER_RESOLUTION) as i32;
        let region = self.data.region.clone();
        let valid = discovery
            .texture(&region)
            .is_some_and(|t| t.0.width() == w && t.0.height() == h);
        if !valid {
            let texture = self.discovery_from_visited_rooms(w, h, &owner.rooms_visited);
            discovery.textures.insert(region.clone(), texture);
        }
        discovery.last_updated.insert(region, owner.now_ticks);
        self.disc_loaded = true;
        if let Some(view) = self.view.as_mut() {
            view.reveal = Some(UnityTexture::black(w, h));
        }
    }

    /// `CreateDiscoveryTextureFromVisitedRooms`: black, with each visited room's
    /// rectangle white.
    #[must_use]
    pub fn discovery_from_visited_rooms(
        &self,
        w: i32,
        h: i32,
        visited: &[String],
    ) -> DiscoveryTexture {
        let mut t = DiscoveryTexture(UnityTexture::black(w, h));
        for name in visited {
            for room in self.data.rooms.iter().filter(|r| &r.name == name) {
                let corner = MapRoom::pixel_of(
                    room.map.on_texture_pos(Vec2::ZERO, self.map_size, true) / DISCOVER_RESOLUTION,
                );
                let (sx, sy) = (
                    room.map.size.0 as f32 / DISCOVER_RESOLUTION,
                    room.map.size.1 as f32 / DISCOVER_RESOLUTION,
                );
                let mut n = 0;
                while n as f32 <= sx {
                    let mut m = 0;
                    while m as f32 <= sy {
                        t.0.set(corner.0 + n, corner.1 + m, [1.0, 0.0, 0.0]);
                        m += 1;
                    }
                    n += 1;
                }
            }
        }
        t
    }

    /// The `mapLoaded && discLoaded` part of `Update`.
    fn update_loaded(
        &mut self,
        owner: &MapOwner,
        reveal_map: bool,
        hide_general_hud: bool,
        stage: &mut StageTree,
        discovery: &mut MapDiscovery,
        rng: &mut UnityRandom,
    ) {
        let region = self.data.region.clone();
        if owner.discovery_active {
            let p = self.discovery_pixel(owner.in_room_pos, owner.room);
            if self.last_on_texture_pos != p
                && let Some(t) = discovery.texture_mut(&region)
            {
                t.discover(p, owner.complete_darkness, rng);
            }
            self.last_on_texture_pos = p;
        }
        let discover = discovery.texture(&region);
        if !self.reveal_pixels.is_empty() {
            self.reveal_routine(discover, rng);
        }
        if !self.reveal_fade_pixels.is_empty() {
            self.fade_routine(discover, rng);
        }
        let pending = self.reveal_pixels.len();
        for c in &mut self.connections {
            c.update(pending);
        }
        self.pan(owner.input);
        if reveal_map && discover.is_some() {
            self.fade_counter += 1;
            let input = owner.input;
            if input.jump && !self.layer_button_a {
                self.layer = (self.layer - 1).max(0);
            }
            if input.throw && !self.layer_button_b {
                self.layer = (self.layer + 1).min(2);
            }
            self.layer_button_a = input.jump;
            self.layer_button_b = input.throw;
            if self.fade_counter > 30 || self.owner != HudOwnerType::Player {
                self.fade = (self.fade + 1.0 / 30.0).min(1.0);
                self.fade = self.fade.lerp(1.0, 0.1);
                if self.last_fade == 0.0 {
                    self.initiate_map_view(owner);
                }
            }
        } else {
            self.fade_counter = (self.fade_counter - 5).max(0);
            self.fade = (self.fade - 0.05).max(0.0);
            self.fade = self.fade.lerp(0.0, 0.1);
        }
        if hide_general_hud {
            self.fade = 0.0;
        }
        self.visible = self.fade > 0.0 && self.last_fade > 0.0;
        if self.owner == HudOwnerType::Player {
            self.count_reset_reveal(owner);
        }
        let marker_pos = self.room_to_map_pos(owner.in_room_pos, owner.room, 1.0);
        let fade = self.fade;
        if let Some(m) = self.view.as_mut().and_then(|v| v.player_marker.as_mut()) {
            m.update();
            m.pos = marker_pos;
            m.fade = fade;
            if m.rad > m.snap_rad {
                m.rad = (m.rad - 4.0).max(m.snap_rad);
                m.rad = m.rad.lerp(m.snap_rad, 0.005);
            }
            if m.thickness < m.snap_thickness {
                m.thickness = (m.thickness + 0.125).min(m.snap_thickness);
            }
        }
        self.swarm_draw(stage, rng);
    }

    /// A player's map resets its reveal once the player has walked far enough with the
    /// map closed.
    fn count_reset_reveal(&mut self, owner: &MapOwner) {
        if self.fade == 0.0 {
            if self.reset_reveal_counter > 0 {
                let here = self.on_texture_pos(owner.in_room_pos, owner.room, true);
                let d = self.player_stand_pos.distance(here) as i32 / 2;
                self.reset_reveal_counter -= d.int_clamp(1, 100);
                if self.reset_reveal_counter < 1 {
                    self.reset_reveal();
                    self.reset_reveal_counter = 0;
                }
            }
        } else if self.fade > 0.5 {
            self.reset_reveal_counter = 100;
        }
    }

    /// The pan from the map input: speeding up while held in one direction.
    fn pan(&mut self, input: Input) {
        let mut v = Vec2::ZERO;
        if input.x != 0 || input.y != 0 {
            v = Vec2::ZERO.direction_to(Vec2::new(f32::from(input.x), f32::from(input.y)));
        }
        if v.length() > 0.1 {
            let along = self.pan_vel.normalized().dot(v.normalized()) * v.length();
            let slow = -1.0 / self.speed_up.lerp_map(0.5, 1.0, 7.0, 30.0);
            self.speed_up =
                (self.speed_up + along.lerp_map(0.6, 0.8, slow, 1.0 / 140.0)).clamp(0.0, 1.0);
            self.pan_vel += v * (0.45 + 0.1 * self.speed_up.inverse_lerp(0.5, 1.0));
        } else {
            self.speed_up = (self.speed_up - 1.0 / 30.0).max(0.0);
            self.pan_vel *= 0.8;
        }
        self.pan_pos += self.pan_vel;
        self.pan_pos.x = self.pan_pos.x.clamp(0.0, self.map_size.x);
        self.pan_pos.y = self.pan_pos.y.clamp(0.0, self.map_size.y);
    }

    /// The swarm-room draw: one value every tick; on success a room and a hive, and a
    /// circle over a revealed hive on screen.
    fn swarm_draw(&mut self, stage: &mut StageTree, rng: &mut UnityRandom) {
        let n = self.data.swarm_rooms.len();
        if !(rng.value() < n as f32 / 20.0 && self.fade > 0.95) {
            return;
        }
        let s = rng.range_i32(0, n as i32) as usize;
        let room = &self.data.swarm_rooms[s];
        if !room.active || room.hives.is_empty() {
            return;
        }
        let (hx, hy) = room.hives[rng.range_i32(0, room.hives.len() as i32) as usize];
        let index = room.room;
        let pos = Vec2::new(hx as f32, hy as f32) * 20.0 - Vec2::new(10.0, 10.0);
        let screen = self.room_to_map_pos(pos, index, 1.0);
        if !(screen.x > 0.0
            && screen.x < self.screen_size.x
            && screen.y > 0.0
            && screen.y < self.screen_size.y)
        {
            return;
        }
        let p = self.on_texture_pos(pos, index, true) / DISCOVER_RESOLUTION;
        let revealed = self
            .view
            .as_ref()
            .and_then(|v| v.reveal.as_ref())
            .is_some_and(|r| r.r(p.x as i32, p.y as i32) == 1.0);
        if !revealed {
            return;
        }
        let Some(container) = self.view.as_ref().map(|v| v.container) else {
            return;
        };
        let mut circle = HudCircle::new(stage, container, SnapGraphic::None, 3);
        circle.pos = self.room_to_map_pos(pos, index, 1.0);
        circle.last_pos = circle.pos;
        self.swarm_circles.push(SwarmCircle {
            circle,
            pos,
            room: index,
            rad: 2.5,
            rad_speed: 2.0,
            life: 1.0,
        });
    }

    fn update_swarm_circle(&mut self, i: usize) {
        let room = self.swarm_circles[i].room;
        let alpha = self.alpha(self.data.layer_of_room(room), 1.0, true);
        let pos = self.room_to_map_pos(self.swarm_circles[i].pos, room, 1.0);
        let fade = self.fade;
        let c = &mut self.swarm_circles[i];
        c.rad += c.rad_speed;
        c.rad_speed *= 0.95;
        c.life -= 1.0 / 35.0;
        c.circle.update();
        c.circle.rad = c.rad;
        c.circle.fade = c.life.mathf_pow(0.5) * fade * alpha.lerp(1.0, 0.25);
        c.circle.thickness = c.life * 4.0;
        c.circle.pos = pos;
    }

    /// `InitiateMapView`: on the first open (or after a reset) the pan centres on the
    /// player and the marker shrinks in; the reveal starts from the player's pixel.
    fn initiate_map_view(&mut self, owner: &MapOwner) {
        if self.reset_reveal_counter == -1 {
            self.reset_reveal();
            self.reset_reveal_counter = 0;
        }
        if self.reset_reveal_counter < 1 {
            self.pan_pos = self.on_texture_pos(owner.in_room_pos, owner.room, false);
            self.last_pan_pos = self.pan_pos;
            self.layer = self.data.layer_of_room(owner.room);
            self.depth = self.layer as f32;
            self.last_depth = self.depth;
            if let Some(m) = self.view.as_mut().and_then(|v| v.player_marker.as_mut()) {
                m.rad = 80.0;
                m.thickness = 0.0;
            }
        }
        let p = self.discovery_pixel(owner.in_room_pos, owner.room);
        for (dx, dy) in EIGHT_DIRECTIONS_AND_ZERO {
            self.add_pixel_to_reveal_list((p.0 + dx, p.1 + dy));
        }
        let stand = self.on_texture_pos(owner.in_room_pos, owner.room, true);
        let Some(reveal) = self.view.as_mut().and_then(|v| v.reveal.as_mut()) else {
            return;
        };
        reveal.set(p.0, p.1, [1.0, 0.0, 0.0]);
        for (dx, dy) in FOUR_DIRECTIONS {
            if reveal.r(p.0 + dx, p.1 + dy) < 0.5 {
                reveal.set(p.0 + dx, p.1 + dy, [0.5, 0.0, 0.0]);
            }
        }
        for ((dx, dy), (ex, ey)) in FOUR_DIRECTIONS.into_iter().zip(DIAGONALS) {
            // The game tests the side pixel and sets the diagonal one.
            if reveal.r(p.0 + dx, p.1 + dy) < 0.3 {
                reveal.set(p.0 + ex, p.1 + ey, [0.3, 0.0, 0.0]);
            }
        }
        self.apply_reveal();
        self.player_stand_pos = stand;
    }

    /// `ResetReveal`.
    fn reset_reveal(&mut self) {
        if let Some(reveal) = self.view.as_mut().and_then(|v| v.reveal.as_mut()) {
            *reveal = UnityTexture::black(reveal.width(), reveal.height());
        }
        self.reveal_pixels.clear();
        self.reveal_fade_pixels.clear();
        for c in &mut self.connections {
            c.reset();
        }
        self.reset_not_revealed_markers();
    }

    /// `Texture2D.Apply` of the reveal texture.
    fn apply_reveal(&mut self) {
        if let Some(view) = self.view.as_mut()
            && let Some(reveal) = &view.reveal
        {
            view.applied = Some(reveal.to_image());
        }
    }

    fn reveal_r(&self, p: (i32, i32)) -> f32 {
        self.view
            .as_ref()
            .and_then(|v| v.reveal.as_ref())
            .map_or(0.0, |r| r.r(p.0, p.1))
    }

    /// `AddPixelToRevealList`.
    fn add_pixel_to_reveal_list(&mut self, p: (i32, i32)) {
        if let Some(reveal) = self.view.as_mut().and_then(|v| v.reveal.as_mut()) {
            reveal.set(p.0, p.1, [0.05, 0.0, 0.0]);
        }
        self.reveal_pixels.push(p);
        self.reveal_fade_pixels.push(p);
    }

    /// `ShouldPixelBeRevealed`: discovered and not yet in the reveal.
    fn should_reveal(&self, discover: Option<&DiscoveryTexture>, p: (i32, i32)) -> bool {
        discover.is_some_and(|d| d.0.r(p.0, p.1) > 0.0) && self.reveal_r(p) == 0.0
    }

    /// `RevealRoutine`: up to six random pending pixels (one draw each), then the one
    /// nearest the pan on the shown layer.
    fn reveal_routine(&mut self, discover: Option<&DiscoveryTexture>, rng: &mut UnityRandom) {
        let n = (self.reveal_pixels.len() as f32).lerp_map(1.0, 100.0, 6.0, 1.0) as i32;
        for _ in 0..n {
            if self.reveal_pixels.is_empty() {
                break;
            }
            let i = rng.range_i32(0, self.reveal_pixels.len() as i32) as usize;
            let p = self.reveal_pixels.remove(i);
            self.reveal_pixel(discover, p);
        }
        if !self.reveal_pixels.is_empty()
            && let Some(i) = self.next_reveal_pixel_closest_to_pan()
        {
            let p = self.reveal_pixels.remove(i);
            self.reveal_pixel(discover, p);
        }
    }

    /// `RevealPixel`: spreads to discovered neighbours and across connections, starts
    /// the connections' and markers' fade-ins.
    fn reveal_pixel(&mut self, discover: Option<&DiscoveryTexture>, p: (i32, i32)) {
        for (dx, dy) in FOUR_DIRECTIONS {
            let q = (p.0 + dx, p.1 + dy);
            if self.should_reveal(discover, q) {
                self.add_pixel_to_reveal_list(q);
            }
        }
        for j in 0..self.connections.len() {
            let c = &self.connections[j];
            let a = self.discovery_pixel(c.room_pos_a(), c.room_a);
            let b = self.discovery_pixel(c.room_pos_b(), c.room_b);
            if a == p {
                if self.should_reveal(discover, b) {
                    self.add_pixel_to_reveal_list(b);
                }
                let pending = self.reveal_pixels.len() as i32;
                let c = &mut self.connections[j];
                if c.start_reveal_a < 0 {
                    c.start_reveal_a = 1 + pending;
                    if c.start_reveal_b < 0 {
                        c.direction = 0.0;
                    }
                }
            } else if b == p {
                if self.should_reveal(discover, a) {
                    self.add_pixel_to_reveal_list(a);
                }
                let pending = self.reveal_pixels.len() as i32;
                let c = &mut self.connections[j];
                if c.start_reveal_b < 0 {
                    c.start_reveal_b = 1 + pending;
                }
                if c.start_reveal_a < 0 {
                    c.direction = 1.0;
                }
            }
        }
        for k in (0..self.not_revealed.len()).rev() {
            let m = &self.markers[self.not_revealed[k]];
            let q = self.discovery_pixel(m.in_room_pos, m.room);
            let d = Vec2::new((q.0 - p.0) as f32, (q.1 - p.1) as f32).length();
            if d < m.fade_in_rad {
                let ticks = 30.0 + self.reveal_pixels.len() as f32;
                let i = self.not_revealed.remove(k);
                self.markers[i].fade_in(ticks);
            }
        }
    }

    /// `NextRevealPixelClosestToPan`.
    fn next_reveal_pixel_closest_to_pan(&self) -> Option<usize> {
        let mut best = f32::MAX;
        let mut result = None;
        for (i, &(x, y)) in self.reveal_pixels.iter().enumerate() {
            let mut a = Vec2::new(
                x as f32 * DISCOVER_RESOLUTION,
                y as f32 * DISCOVER_RESOLUTION,
            );
            let layer = (a.y / self.map_size.y).floor() as i32;
            if layer == self.layer {
                a.y -= self.map_size.y * layer as f32;
                let d = a.distance(self.pan_pos);
                if d < best {
                    result = Some(i);
                    best = d;
                }
            }
        }
        result
    }

    /// `FadeRoutine`: each fading pixel rises by a draw over the pending count, up to
    /// its discovery level.
    fn fade_routine(&mut self, discover: Option<&DiscoveryTexture>, rng: &mut UnityRandom) {
        let Some(discover) = discover else {
            return;
        };
        let pending = self.reveal_pixels.len();
        let Some(reveal) = self.view.as_mut().and_then(|v| v.reveal.as_mut()) else {
            return;
        };
        for i in (0..self.reveal_fade_pixels.len()).rev() {
            let (x, y) = self.reveal_fade_pixels[i];
            let r = discover.0.r(x, y);
            let r2 = reveal.r(x, y) + rng.value() / (1 + pending) as f32;
            reveal.set(x, y, [r2.min(r), 0.0, 0.0]);
            if r2 >= r {
                self.reveal_fade_pixels.remove(i);
            }
        }
        self.apply_reveal();
    }

    /// `OnTexturePos`.
    #[must_use]
    pub fn on_texture_pos(&self, pos: Vec2, room: i32, account_for_layer: bool) -> Vec2 {
        self.data
            .map_room(room)
            .on_texture_pos(pos, self.map_size, account_for_layer)
    }

    /// The discovery pixel of a room position.
    #[must_use]
    pub fn discovery_pixel(&self, pos: Vec2, room: i32) -> (i32, i32) {
        MapRoom::pixel_of(self.on_texture_pos(pos, room, true) / DISCOVER_RESOLUTION)
    }

    /// `RoomToMapPos`: a room position on screen, through the pan, the scale, the
    /// layer's depth zoom and the lens.
    #[must_use]
    pub fn room_to_map_pos(&self, pos: Vec2, room: i32, t: f32) -> Vec2 {
        let r = self.data.map_room(room);
        let screen = self.screen_size;
        let mut result = r.on_texture_pos(pos, self.map_size, false)
            - self.last_pan_pos.lerp(self.pan_pos, t.clamp(0.0, 1.0));
        result = result * MAP_SCALE + screen / 2.0;
        let mut v3 = Vec2::new(
            result.x.inverse_lerp(0.0, screen.x),
            result.y.inverse_lerp(0.0, screen.y),
        );
        let depth = self.last_depth.lerp(self.depth, t);
        let zoom = 3.25_f32.lerp(4.75, (2 - r.layer) as f32 / 3.0 + depth / 2.0 / 3.0) / 4.0;
        let v4 = v3;
        v3 = (v3 - Vec2::new(0.5, 0.5)) * zoom + Vec2::new(0.5, 0.5);
        result += Vec2::new((v3.x - v4.x) * screen.x, (v3.y - v4.y) * screen.y);
        v3 = Vec2::new(
            result.x.inverse_lerp(0.0, screen.x),
            result.y.inverse_lerp(0.0, screen.y),
        );
        let f = v3.distance(Vec2::new(0.5, 0.5)).clamp(0.0, 1.0);
        let mut lens = (v3 - Vec2::new(0.5, 0.5)).normalized() * (1.32 * f.mathf_pow(4.81));
        lens.y *= 0.5;
        lens.x /= self.map_size.x / 500.0;
        lens.y /= self.map_size.y / 500.0;
        lens.x *= self.map_size.x * zoom * 4.0;
        lens.y *= self.map_size.y * zoom * 4.0;
        result + lens
    }

    /// `Alpha(layer, t, compensateForLayersInFront)`.
    #[must_use]
    pub fn alpha(&self, layer: i32, t: f32, compensate: bool) -> f32 {
        let fade = self.last_fade.lerp(self.fade, t);
        let depth = self.last_depth.lerp(self.depth, t);
        let far = if layer == 2 { 0.5 } else { 0.35 };
        let mut a = fade * (layer as f32 - depth).abs().lerp_map(0.45, 0.9, 1.0, far);
        if compensate {
            match layer {
                1 => a *= 0.3_f32.lerp(1.0, self.out_of_focus(0, t)),
                2 => a *= 0.3_f32.lerp(1.0, self.out_of_focus(0, t).min(self.out_of_focus(1, t))),
                _ => {}
            }
        }
        a * fade
    }

    /// `OutOfFocus`.
    #[must_use]
    pub fn out_of_focus(&self, layer: i32, t: f32) -> f32 {
        (layer as f32 - self.last_depth.lerp(self.depth, t))
            .abs()
            .inverse_lerp(0.45, 0.9)
    }

    /// `AlphaOfDefaultMaterial`.
    fn alpha_of_default_material(&self, layer: i32, t: f32) -> f32 {
        0.7 * self.alpha(layer, t, false) * (1.0 - self.out_of_focus(layer, t))
    }

    /// `Map.Draw`.
    pub fn draw(&mut self, stage: &mut StageTree, t: f32) {
        if !self.disc_loaded {
            return;
        }
        let visible = self.visible;
        if let Some(m) = self.view.as_mut().and_then(|v| v.player_marker.as_mut()) {
            m.visible = visible;
        }
        let Some(view) = self.view.as_ref() else {
            return;
        };
        for id in view.sprites {
            stage.transform_mut(id).visible = self.visible;
        }
        for c in &self.connections {
            c.set_visible(stage, self.visible);
        }
        if let (Some(m), Some(fade)) = (&view.player_marker, view.player_marker_fade) {
            m.draw(stage, t);
            stage.transform_mut(fade).visible = self.visible;
        }
        stage.transform_mut(view.background_black).visible = self.visible;
        let marker_view = MarkerView {
            visible: self.visible,
            map_fade: self.last_fade.lerp(self.fade, t),
            time: self.counter as f32 + t,
            screen_pos: Vec2::ZERO,
            death_screen: self.owner == HudOwnerType::DeathScreen,
        };
        for m in &self.markers {
            let v = MarkerView {
                screen_pos: self.room_to_map_pos(m.in_room_pos, m.room, t),
                ..marker_view
            };
            m.draw(stage, &v, t);
        }
        if !self.visible {
            return;
        }
        for c in &self.swarm_circles {
            c.circle.draw(stage, t);
        }
        let fade = self.last_fade.lerp(self.fade, t);
        let depth = self.last_depth.lerp(self.depth, t);
        let mut product = 1.0;
        for i in 0..3 {
            product *= 1.0 - self.alpha_of_default_material(2 - i, t);
        }
        let shown = (1.0 - product).inverse_lerp(1.2, 0.7);
        let mut sky = 0.0_f32;
        for i in 0..3 {
            let s = stage.sprite_mut(view.sprites[i]);
            s.transform.alpha = self.alpha(2 - i as i32, t, false) * shown;
            if self.data.sky_layers[i] {
                sky = sky.max(1.0 - self.out_of_focus(2 - i as i32, t));
            }
        }
        stage.transform_mut(view.background_black).alpha = sky * 0.5 * fade;
        let d = depth / 2.0 / 3.0;
        let colors = [
            StageColor::rgb(self.out_of_focus(2, t), d, 1.0),
            StageColor::rgb(self.out_of_focus(1, t), 1.0 / 3.0 + d, 0.5),
            StageColor::rgb(self.out_of_focus(0, t), 2.0 / 3.0 + d, 0.0),
        ];
        for (i, color) in colors.into_iter().enumerate() {
            let s = stage.sprite_mut(view.sprites[i]);
            s.color = color;
            s.transform.scale_x = self.screen_size.x / view.texture_size.x;
            s.transform.scale_y = self.screen_size.y / view.texture_size.y;
            s.transform.x = self.screen_size.x / 2.0;
            s.transform.y = self.screen_size.y / 2.0;
        }
        if let Some(fade_sprite) = view.player_marker_fade {
            let at = self.room_to_map_pos(self.owner_pos, self.owner_room, t);
            let fade_sprite = stage.sprite_mut(fade_sprite);
            fade_sprite.transform.x = at.x;
            fade_sprite.transform.y = at.y;
            fade_sprite.transform.alpha = 0.5 * fade.mathf_pow(2.0);
            fade_sprite.transform.set_scale(20.0);
        }
        let map_color = Self::map_color();
        for c in &self.connections {
            let cv = ConnectionView {
                pos_a: self.room_to_map_pos(c.room_pos_a(), c.room_a, t),
                pos_b: self.room_to_map_pos(c.room_pos_b(), c.room_b, t),
                alpha_a: self.alpha(self.data.layer_of_room(c.room_a), t, true),
                alpha_b: self.alpha(self.data.layer_of_room(c.room_b), t, true),
                map_color,
            };
            c.draw(stage, &cv, t);
        }
        if let Some(label) = &view.cycle_label {
            label.draw(stage, t, fade);
        }
    }

    /// `RainWorld.MapColor`.
    #[must_use]
    pub fn map_color() -> StageColor {
        HslColor::new(0.730_555_5, 0.2, 0.4).rgb()
    }

    /// Hands a newly applied reveal texture to the atlases (uploaded on the next sync).
    pub fn sync_textures(&mut self, atlases: &mut AtlasManager) {
        let Some(view) = self.view.as_mut() else {
            return;
        };
        let Some(image) = view.applied.take() else {
            return;
        };
        match view.fog {
            Some(id) => atlases.replace_texture(id, image),
            None => {
                let sampling = TextureSampling {
                    filter: TextureFilter::Bilinear,
                    clamp: false,
                };
                view.fog = Some(atlases.add_texture(image, sampling));
            }
        }
    }

    /// The map shaders' globals at `t` between ticks.
    #[must_use]
    pub fn globals(&self, t: f32) -> MapGlobals {
        let pan = self.last_pan_pos.lerp(self.pan_pos, t);
        let c = Self::map_color();
        // `RainWorld.DefaultWaterColor`: Remix off, so the region's water colour is unused.
        MapGlobals {
            size: self.map_size.to_array(),
            pan: [
                pan.x.inverse_lerp(0.0, self.map_size.x),
                pan.y.inverse_lerp(0.0, self.map_size.y),
            ],
            color: c.to_array(),
            water_color: [0.05, 0.05, 0.8, 1.0],
            fog: self.view.as_ref().and_then(|v| v.fog),
        }
    }

    /// The player marker, for tests and tools.
    #[must_use]
    pub fn player_marker(&self) -> Option<&HudCircle> {
        self.view.as_ref().and_then(|v| v.player_marker.as_ref())
    }

    /// The reveal texture, for tests and tools.
    #[must_use]
    pub fn reveal(&self) -> Option<&UnityTexture> {
        self.view.as_ref().and_then(|v| v.reveal.as_ref())
    }

    /// The map atlas element and the in-front container, for tests and tools.
    #[must_use]
    pub fn element(&self) -> Option<&str> {
        self.view.as_ref().map(|v| v.element.as_str())
    }

    #[must_use]
    pub fn in_front(&self) -> Option<NodeId> {
        self.view.as_ref().map(|v| v.in_front)
    }
}
