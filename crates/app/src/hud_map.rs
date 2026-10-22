//! What the HUD map reads of the game: `MapData` of the world, the player as its owner
//! (`MapOwnerInRoomPosition`, `MapDiscoveryActive`, `MapInput`), the Hunter's cycle
//! label, and the link the player's exit discovery reads back.
//!
//! Mirrors: HUD/Map.cs

use rw_assets::{SlugcatName, StreamingAssets};
use rw_hud::{Hud, MapData, MapOwner, ShelterItems};
use rw_menu::Translator;
use rw_save::PlayerProgression;
use rw_sim::{HEAD, HudMapLink, Vec2};

use crate::game::Game;

impl Game {
    /// `new MapData(world)` with the save state's `deathPersistentSaveData.karma`, its
    /// karma flower (`karmaFlowerMapPos`, set as the world loads the saved flower) and
    /// the death positions the map's constructor reads.
    #[must_use]
    pub fn map_data(&self, assets: &StreamingAssets, saves: &PlayerProgression) -> Option<MapData> {
        let death_persistent = saves
            .current_save_state
            .as_ref()
            .map(|s| &s.death_persistent);
        let flower = death_persistent.and_then(|d| d.karma_flower_coordinate());
        let mut data = MapData::build(
            self.sim().world(),
            assets,
            self.sim().campaign().name.value(),
            self.sim().story().karma,
            flower.as_ref(),
        )?;
        data.death_positions = death_persistent
            .map(|d| d.death_position_list())
            .unwrap_or_default();
        Some(data)
    }

    /// `Map.CycleLabel`'s text for the Hunter: `RedsCycles(false) - cycleNumber` (19
    /// without Remix), red when none are left.
    #[must_use]
    pub fn map_cycle_label(&self, translator: &Translator) -> Option<(String, bool)> {
        let campaign = self.sim().campaign();
        if campaign.name != SlugcatName::Red {
            return None;
        }
        let n = 19 - self.sim().story_session().cycle_number;
        Some((format!("{} {n}", translator.translate("Cycle")), n <= 0))
    }

    /// The player as the map's owner this tick.
    #[must_use]
    pub fn map_owner(&self) -> MapOwner {
        let sim = self.sim();
        let Some(player) = sim.player() else {
            return MapOwner::default();
        };
        let room = sim.player_pos().room;
        // `OnScreenPositionOfInShortCutCreature`: the vessel's tile while inside.
        let in_room_pos = sim
            .player_vessel_tile(room)
            .map_or(player.chunks()[HEAD].pos, Vec2::middle_of_tile);
        let rooms_visited = sim
            .world()
            .region_state
            .as_ref()
            .map(|s| s.rooms_visited.clone())
            .unwrap_or_default();
        MapOwner {
            discovery_active: sim.map_discovery_active(),
            in_room_pos,
            room,
            complete_darkness: sim.complete_darkness(room, player.chunks()[HEAD].pos),
            input: player.map_input(),
            rooms_visited,
            now_ticks: MapOwner::now_ticks(),
            // `GetItemInShelter` reads the world while the player has a room (not in a
            // shortcut).
            shelter_items: (!player.in_shortcut()).then(|| ShelterItems::of_world(sim.world())),
        }
    }

    /// `PlayerProgression.LoadMapTexture(region)`, as the map's first update calls it
    /// when it has no discovery texture: the texture and time in memory, else the
    /// save's `MAP` and `MAPUPDATE` lines.
    pub fn load_map_texture(&mut self, saves: &mut PlayerProgression, region: &str) {
        let discovery = self.sim().map_discovery();
        let has_texture = discovery.texture(region).is_some();
        if has_texture && discovery.last_updated.contains_key(region) {
            return;
        }
        let memory_png = saves
            .map_discovery
            .iter()
            .find(|(r, _)| r == region)
            .map(|(_, png)| png.clone());
        let memory_time = saves
            .map_last_updated
            .iter()
            .find(|(r, _)| r == region)
            .map(|(_, t)| *t);
        let (stored_png, stored_time) = saves.stored_map(region);
        let discovery = self.sim_mut().map_discovery_mut();
        if !discovery.last_updated.contains_key(region)
            && let Some(time) = memory_time.or(stored_time)
        {
            discovery.last_updated.insert(region.to_owned(), time);
        }
        if !has_texture
            && let Some(png) = memory_png.or(stored_png)
            && !discovery.load_base64(region, &png)
        {
            log::warn!("stored map texture of {region} is corrupt");
        }
    }

    /// The progression's map fields before a save (`SaveToDisk` with `saveMaps`): each
    /// region's discovery texture as base64 PNG and its last-updated time.
    pub fn store_map_discovery(&self, saves: &mut PlayerProgression) {
        let discovery = self.sim().map_discovery();
        for region in discovery.textures.keys() {
            let Some(png) = discovery.store_base64(region) else {
                continue;
            };
            match saves.map_discovery.iter_mut().find(|(r, _)| r == region) {
                Some(entry) => entry.1 = png,
                None => saves.map_discovery.push((region.clone(), png)),
            }
        }
        for (region, &time) in &discovery.last_updated {
            match saves.map_last_updated.iter_mut().find(|(r, _)| r == region) {
                Some(entry) => entry.1 = time,
                None => saves.map_last_updated.push((region.clone(), time)),
            }
        }
    }

    /// Tells the sim about camera 0's HUD map (for the player's exit discovery).
    pub fn link_hud_map(&mut self, hud: Option<&Hud>) {
        let link = hud.map_or_else(HudMapLink::default, |h| HudMapLink {
            present: true,
            loaded: h.map.disc_loaded,
            region: h.map.data.region.clone(),
            map_size: h.map.map_size,
        });
        self.sim_mut().set_hud_map(link);
    }
}
