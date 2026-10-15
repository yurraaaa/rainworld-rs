//! What the HUD map reads of the game: `MapData` of the world, the player as its owner
//! (`MapOwnerInRoomPosition`, `MapDiscoveryActive`, `MapInput`), the Hunter's cycle
//! label, and the link the player's exit discovery reads back.

use rw_assets::{SlugcatName, StreamingAssets};
use rw_hud::{Hud, MapData, MapOwner};
use rw_menu::Translator;
use rw_sim::{HEAD, HudMapLink, Vec2};

use crate::game::Game;

impl Game {
    /// `new MapData(world)` with the new game's karma.
    #[must_use]
    pub fn map_data(&self, assets: &StreamingAssets) -> Option<MapData> {
        let campaign = self.sim().campaign();
        MapData::build(
            self.sim().world(),
            assets,
            campaign.name.value(),
            campaign.start_karma,
        )
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
        let in_room_pos = player
            .shortcut_vessel_tile()
            .map_or(player.chunks()[HEAD].pos, Vec2::middle_of_tile);
        let rooms_visited = sim
            .world()
            .region_state
            .as_ref()
            .map(|s| s.rooms_visited.iter().cloned().collect())
            .unwrap_or_default();
        MapOwner {
            discovery_active: sim.map_discovery_active(),
            in_room_pos,
            room,
            complete_darkness: sim.complete_darkness(room),
            input: player.map_input(),
            rooms_visited,
            now_ticks: Self::now_ticks(),
        }
    }

    /// `DateTime.Now.Ticks` (UTC): 100 ns since 0001-01-01.
    fn now_ticks() -> i64 {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() / 100);
        i64::try_from(unix).unwrap_or(0) + 621_355_968_000_000_000
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
