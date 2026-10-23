//! The player's part in map discovery (`Player.Update`): reporting the spawn room
//! visited and discovering the way to the first room's exits on the HUD map
//! (`ExternalExitDiscover`, `ExternalSmallDiscover`, `ExternalOnePixelDiscover`).
//!
//! Mirrors: Player.cs (Update), HUD/Map.cs (ExternalExitDiscover, ExternalSmallDiscover, ExternalOnePixelDiscover)

use rw_assets::{Room, ShortcutType};

use super::RainWorldGame;
use crate::map_discovery::{DISCOVER_RESOLUTION, MapRoom};
use crate::{HEAD, Vec2};

impl RainWorldGame {
    /// `RegionState.ReportRoomVisited` for global room `index`.
    pub(crate) fn report_room_visited(&mut self, index: i32) {
        let Some(name) = self.world.abstract_room(index).map(|r| r.name.clone()) else {
            return;
        };
        if let Some(state) = self.world.region_state.as_mut() {
            state.report_room_visited(&name);
        }
    }

    /// `Player.MapDiscoveryActive`: conscious, no danger grasp, in a realized room of a region world,
    /// the main chunk inside the room's bounds.
    #[must_use]
    pub fn map_discovery_active(&self) -> bool {
        let Some(realized) = self.world.realized(self.pos().room) else {
            return false;
        };
        let room = &realized.room;
        let Some(player) = self.player.as_ref() else {
            return false;
        };
        let p = player.chunks()[HEAD].pos;
        player.is_conscious()
            && player.danger_grasp().grasp.is_none()
            && !player.in_shortcut()
            && !self.world.single_room()
            && p.x > 0.0
            && p.x < room.pixel_width()
            && p.y > 0.0
            && p.y < room.pixel_height()
    }

    /// `Room.CompleteDarkness(pos, 0, 0.95, false)` for room `index`: the camera shows
    /// it at darkness 0.95 or more and no light source reaches `pos` (within its rad;
    /// the light sources' rad and position are what their owners last drew).
    #[must_use]
    pub fn complete_darkness(&self, index: i32, pos: Vec2) -> bool {
        if index != self.camera_room || self.camera_darkness < 0.95 {
            return false;
        }
        !self.world.realized(index).is_some_and(|r| {
            r.light_sources
                .iter()
                .any(|l| !l.slated && pos.dist_less(l.pos, l.rad))
        })
    }

    /// `Player.Update`'s first report of the room it is in (`addedSpawnRoomToDiscovery`).
    pub(crate) fn report_spawn_room(&mut self, index: i32) {
        let Some(player) = self.player.as_mut() else {
            return;
        };
        if !player.added_spawn_room_to_discovery {
            player.added_spawn_room_to_discovery = true;
            self.report_room_visited(index);
        }
    }

    /// The exit discovery of `Player.Update` (after [`Self::report_spawn_room`] and the
    /// sleep update), for the player in room
    /// `index`: `moved` is `coord != lastCoord`. The first time the map discovery sees
    /// the player move it lists the room's exits; after that, while the HUD map exists
    /// and the room is not dark, each move picks an exit (a draw) and, if the camera
    /// sees it, discovers up to 20 steps of 50 px towards it.
    pub(crate) fn player_map_discovery(&mut self, index: i32, room: &Room, moved: bool) {
        if !moved || !self.map_discovery_active() {
            return;
        }
        let shortcuts_ready = self
            .world
            .realized(index)
            .is_some_and(crate::world::RealizedRoom::shortcuts_ready);
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let Some(mut exits) = player.exits_to_be_discovered.take() else {
            if !shortcuts_ready {
                return;
            }
            player.exits_to_be_discovered = Some(
                room.shortcut_map()
                    .shortcuts()
                    .iter()
                    .filter(|s| s.kind == ShortcutType::RoomExit)
                    .map(|s| Vec2::middle_of_tile(s.start))
                    .collect(),
            );
            return;
        };
        let first = player.chunks()[HEAD].pos;
        if !exits.is_empty() && self.hud_map.present && !self.complete_darkness(index, first) {
            let i = self.rng.range_i32(0, exits.len() as i32) as usize;
            let exit = exits[i];
            if self.camera_view.viewed(index, room, exit, -10.0) {
                let mut pos = first;
                for _ in 0..20 {
                    if pos.dist_less(exit, 50.0) {
                        self.external_exit_discover((pos + exit) / 2.0, index);
                        self.external_discover_pixel(exit, index);
                        exits.remove(i);
                        break;
                    }
                    self.external_small_discover(pos, index);
                    pos += pos.direction_to(exit) * 50.0;
                }
            }
        }
        if let Some(player) = self.player.as_mut() {
            player.exits_to_be_discovered = Some(exits);
        }
    }

    /// The HUD map's discovery texture position of `pos` in room `index`, unscaled
    /// (`OnTexturePos(pos, room, true) / DiscoverResolution`), when the map has loaded
    /// its discovery.
    fn discovery_pos(&self, pos: Vec2, index: i32) -> Option<Vec2> {
        if !self.hud_map.loaded {
            return None;
        }
        let room = MapRoom::of(self.world.abstract_room(index)?);
        Some(room.on_texture_pos(pos, self.hud_map.map_size, true) / DISCOVER_RESOLUTION)
    }

    /// `Map.ExternalSmallDiscover`.
    fn external_small_discover(&mut self, pos: Vec2, index: i32) {
        let Some(p) = self.discovery_pos(pos, index) else {
            return;
        };
        if let Some(t) = self.map_discovery.texture_mut(&self.hud_map.region) {
            t.small_discover(MapRoom::pixel_of(p), &mut self.rng);
        }
    }

    /// `Map.ExternalExitDiscover`: a small discovery there and half a pixel down-left.
    fn external_exit_discover(&mut self, pos: Vec2, index: i32) {
        let Some(p) = self.discovery_pos(pos, index) else {
            return;
        };
        if let Some(t) = self.map_discovery.texture_mut(&self.hud_map.region) {
            t.small_discover(MapRoom::pixel_of(p), &mut self.rng);
            t.small_discover(MapRoom::pixel_of(p - Vec2::new(0.5, 0.5)), &mut self.rng);
        }
    }

    /// `Map.ExternalOnePixelDiscover`.
    fn external_discover_pixel(&mut self, pos: Vec2, index: i32) {
        let Some(p) = self.discovery_pos(pos, index) else {
            return;
        };
        if let Some(t) = self.map_discovery.texture_mut(&self.hud_map.region) {
            t.one_pixel_discover(MapRoom::pixel_of(p));
        }
    }
}
