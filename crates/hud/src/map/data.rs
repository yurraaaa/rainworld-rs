//! `HUD.Map.MapData` built from a region world (`MapData(World, RainWorld)`), and what
//! `Map` loads with it: the map image, the connection lines and the sky layers.
//!
//! Mirrors: HUD/Map.cs (MapData)

use rw_assets::room::Room;
use rw_assets::world::AbstractRoomData;
use rw_assets::{MapConnectionLine, PngImage, RegionMapFiles, StreamingAssets};
use rw_save::SavedCoordinate;
use rw_sim::world::World;
use rw_sim::{MapRoom, Vec2, WorldCoordinate};

use super::shelter_items::ItemInShelterData;

/// `RegionGate.GateRequirement` (base game, More Slugcats' locks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateRequirement {
    OneKarma,
    TwoKarma,
    ThreeKarma,
    FourKarma,
    FiveKarma,
    DemoLock,
    /// More Slugcats.
    OeLock,
    /// More Slugcats.
    RoboLock,
}

impl GateRequirement {
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::OneKarma => "1",
            Self::TwoKarma => "2",
            Self::ThreeKarma => "3",
            Self::FourKarma => "4",
            Self::FiveKarma => "5",
            Self::DemoLock => "D",
            Self::OeLock => "L",
            Self::RoboLock => "R",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::OneKarma,
            Self::TwoKarma,
            Self::ThreeKarma,
            Self::FourKarma,
            Self::FiveKarma,
            Self::DemoLock,
            Self::OeLock,
            Self::RoboLock,
        ]
        .into_iter()
        .find(|g| g.value() == value)
    }

    /// `int.TryParse(karma.value)`.
    #[must_use]
    pub fn karma(self) -> Option<i32> {
        self.value().parse().ok()
    }

    /// `Region.EquivalentRegion` (base pairs; no modded equivalences).
    #[must_use]
    pub fn equivalent_region(a: &str, b: &str) -> bool {
        const PAIRS: [[&str; 2]; 5] = [
            ["SL", "LM"],
            ["SS", "RM"],
            ["DS", "UG"],
            ["SH", "CL"],
            ["DM", "MS"],
        ];
        a == b || PAIRS.iter().any(|p| p.contains(&a) && p.contains(&b))
    }

    /// `MapData.KarmaOfGate` with no unlocked gates (a new game): the gate's
    /// `locks.txt` line, the side of the region the map shows; `OneKarma` without one.
    #[must_use]
    pub fn of_gate(locks: &[Vec<String>], gate: &str, region: &str) -> Option<Self> {
        let Some(line) = locks
            .iter()
            .find(|l| l.first().map(String::as_str) == Some(gate))
        else {
            return Some(Self::OneKarma);
        };
        let swap = line.get(3).is_some_and(|v| v == "SWAPMAPSYMBOL");
        let side = gate.split('_').nth(1).unwrap_or("");
        let value = if Self::equivalent_region(side, region) == swap {
            line.get(2)
        } else {
            line.get(1)
        };
        value.and_then(|v| Self::from_value(v.trim()))
    }
}

/// `MapData.ShelterData`.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelterData {
    pub room: i32,
    pub marker_pos: Vec2,
    /// `itemsData`: `None` until `UpdateData` puts the items in.
    pub items: Option<Vec<ItemInShelterData>>,
}

/// `MapData.GateData`: `karma` is `None` for an unlocked gate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateData {
    pub room: i32,
    pub karma: Option<GateRequirement>,
}

/// `MapData.SwarmRoomData`.
#[derive(Debug, Clone, PartialEq)]
pub struct SwarmRoomData {
    pub room: i32,
    pub hives: Vec<(i32, i32)>,
    pub active: bool,
}

/// One room of `MapData`.
#[derive(Debug, Clone, PartialEq)]
pub struct MapDataRoom {
    pub name: String,
    pub map: MapRoom,
}

/// `HUD.Map.MapData` of a region, with the map's files.
#[derive(Debug, Clone)]
pub struct MapData {
    pub region: String,
    pub first_room_index: i32,
    pub rooms: Vec<MapDataRoom>,
    /// `roomConnections`: "A,B" room pairs, each once.
    pub room_connections: Vec<(String, String)>,
    pub shelters: Vec<ShelterData>,
    pub gates: Vec<GateData>,
    pub swarm_rooms: Vec<SwarmRoomData>,
    /// `currentKarma`.
    pub current_karma: i32,
    /// `karmaFlowerPos`: a flower in this region.
    pub karma_flower_pos: Option<WorldCoordinate>,
    /// The save state's `deathPositions`, which the map's constructor reads
    /// (`GetSaveState`) for its death markers.
    pub death_positions: Vec<SavedCoordinate>,
    /// `STANDARDELEMENTLIST`: which of the three map sprites draw as sky.
    pub sky_layers: [bool; 3],
    /// `map_<region>.png`.
    pub image: Option<PngImage>,
    pub connection_lines: Vec<MapConnectionLine>,
}

impl MapData {
    /// `new MapData(world, rainWorld)` with `UpdateData` of a story world: rooms from the
    /// world (disabled map rooms leave out their connections, shelter, gate and swarm
    /// data); shelter markers six tiles out of the shelter's door, gate requirements from
    /// `locks.txt`; the session's `karmaFlowerMapPos`. `slugcat` picks
    /// `map_<region>-<slugcat>` files when they exist.
    #[must_use]
    pub fn build(
        world: &World,
        assets: &StreamingAssets,
        slugcat: &str,
        karma: i32,
        karma_flower: Option<&SavedCoordinate>,
    ) -> Option<Self> {
        let data = world.data();
        let region = data.region.as_ref()?.name.clone();
        let mut map = Self {
            first_room_index: data.first_room_index(),
            rooms: data
                .rooms
                .iter()
                .map(|r| MapDataRoom {
                    name: r.name.clone(),
                    map: MapRoom::of(r),
                })
                .collect(),
            room_connections: Vec::new(),
            shelters: Vec::new(),
            gates: Vec::new(),
            swarm_rooms: Vec::new(),
            current_karma: karma,
            karma_flower_pos: None,
            death_positions: Vec::new(),
            sky_layers: Self::sky_layers(&region, &data.properties.params.other),
            image: RegionMapFiles::find(assets, &region, slugcat, "png")
                .and_then(|p| PngImage::load(&p).ok()),
            connection_lines: RegionMapFiles::connections(assets, &region, slugcat),
            region,
        };
        for room in &data.rooms {
            if data.disabled_map_rooms.contains(&room.name) {
                continue;
            }
            map.add_connections(world, room);
            if room.swarm_room_index > -1 {
                let hives = Self::load_room(assets, room)
                    .map(|r| r.hives().iter().flatten().copied().collect())
                    .unwrap_or_default();
                let active = world
                    .region_state
                    .as_ref()
                    .is_some_and(|s| s.swarm_room_active(data, room.swarm_room_index as usize));
                map.swarm_rooms.push(SwarmRoomData {
                    room: room.index,
                    hives,
                    active,
                });
            } else if room.shelter()
                && !data
                    .broken_shelters
                    .get(room.shelter_index as usize)
                    .copied()
                    .unwrap_or(false)
            {
                if let Some(pos) = Self::shelter_marker_pos(assets, room) {
                    map.shelters.push(ShelterData {
                        room: room.index,
                        marker_pos: pos,
                        items: None,
                    });
                }
            } else if room.gate() {
                map.gates.push(GateData {
                    room: room.index,
                    karma: GateRequirement::of_gate(&data.gate_locks, &room.name, &map.region),
                });
            }
        }
        map.karma_flower_pos = map.flower_in_region(karma_flower);
        Some(map)
    }

    /// `UpdateData(world, foodTicks, currKarm, kfPos, putItemsInShelters)`: the karma
    /// the gate markers compare with, the flower if in this region, a swarm room active
    /// once its counter is within `food_ticks` of 0 (the game pairs the map's swarm
    /// rooms with the world's by index), and with `put_items_in_shelters` each
    /// shelter's items as the world holds them now.
    pub fn update_data(
        &mut self,
        world: &World,
        food_ticks: i32,
        karma: i32,
        karma_flower: Option<&SavedCoordinate>,
        put_items_in_shelters: bool,
    ) {
        self.current_karma = karma;
        self.karma_flower_pos = self.flower_in_region(karma_flower);
        if world.single_room() {
            return;
        }
        let data = world.data();
        // Without a region state (where the game would throw on a swarm room) the
        // swarm rooms are left as they are.
        if let Some(state) = world.region_state.as_ref() {
            let n = self.swarm_rooms.len().min(data.swarm_rooms.len());
            for i in 0..n {
                let name = data
                    .room(data.swarm_rooms[i])
                    .map_or("", |r| r.name.as_str());
                let counter = state
                    .swarm_room_counters
                    .iter()
                    .find(|(r, _)| r == name)
                    .map_or(0, |(_, c)| *c);
                self.swarm_rooms[i].active =
                    state.swarm_room_active(data, i) || counter - food_ticks <= 0;
            }
        }
        if !put_items_in_shelters {
            return;
        }
        for s in &mut self.shelters {
            s.items = Some(Vec::new());
        }
        let first = world.first_room_index();
        let rooms = i32::try_from(world.number_of_rooms()).unwrap_or(i32::MAX);
        for k in first..first.saturating_add(rooms) {
            if !world
                .abstract_room(k)
                .is_some_and(AbstractRoomData::shelter)
            {
                continue;
            }
            if let Some(s) = self.shelters.iter_mut().find(|s| s.room == k) {
                s.items = Some(ItemInShelterData::list_from_world(world, k));
            }
        }
    }

    /// A saved flower position resolved in this region: its tile, at node -1 when the
    /// save has none.
    fn flower_in_region(&self, flower: Option<&SavedCoordinate>) -> Option<WorldCoordinate> {
        let f = flower?;
        let room = self.index_of(&f.room)?;
        Some(WorldCoordinate::new(room, f.x, f.y, f.node))
    }

    /// Loads the map image again (`map_<region>[-<slugcat>].png`) if a map took it:
    /// another map of the same data (a sleep or death screen's) loads its own.
    pub fn reload_image(&mut self, assets: &StreamingAssets, slugcat: &str) {
        if self.image.is_none() && !self.region.is_empty() {
            self.image = RegionMapFiles::find(assets, &self.region, slugcat, "png")
                .and_then(|p| PngImage::load(&p).ok());
        }
    }

    /// A map with no region and no image: it never loads.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            region: String::new(),
            first_room_index: 0,
            rooms: Vec::new(),
            room_connections: Vec::new(),
            shelters: Vec::new(),
            gates: Vec::new(),
            swarm_rooms: Vec::new(),
            current_karma: 0,
            karma_flower_pos: None,
            death_positions: Vec::new(),
            sky_layers: [false; 3],
            image: None,
            connection_lines: Vec::new(),
        }
    }

    fn add_connections(&mut self, world: &World, room: &AbstractRoomData) {
        let disabled = &world.data().disabled_map_rooms;
        for &c in &room.connections {
            let Some(other) = world
                .abstract_room(c)
                .filter(|o| !disabled.contains(&o.name))
            else {
                continue;
            };
            let pair = (room.name.clone(), other.name.clone());
            if !self.has_connection(&pair.0, &pair.1) {
                self.room_connections.push(pair);
            }
        }
    }

    #[must_use]
    pub fn has_connection(&self, a: &str, b: &str) -> bool {
        self.room_connections
            .iter()
            .any(|(x, y)| (x == a && y == b) || (x == b && y == a))
    }

    fn load_room(assets: &StreamingAssets, room: &AbstractRoomData) -> Option<Room> {
        let path = assets.find_room_file(&room.file_name().to_lowercase(), ".txt")?;
        Room::load(path).ok()
    }

    /// The shelter marker: six tiles out of the entrance of the shortcut to node 0
    /// (`MiddleOfTile`), or near the middle of an ancient shelter.
    fn shelter_marker_pos(assets: &StreamingAssets, room: &AbstractRoomData) -> Option<Vec2> {
        if room.ancient_shelter {
            let tile = (room.size.0 / 2 - 2, room.size.1 / 2 + 2);
            return Some(Vec2::middle_of_tile(tile));
        }
        let r = Self::load_room(assets, room)?;
        let start = r.shortcut_map().leading_to_node(0)?.start;
        let dir = r.shortcut_hole_direction(start.0, start.1);
        Some(Vec2::middle_of_tile((
            start.0 + dir.0 * 6,
            start.1 + dir.1 * 6,
        )))
    }

    /// `STANDARDELEMENTLIST`: CC and SI draw their two upper map sprites as sky; other
    /// regions read `mapSkyLayers` (layer numbers 0 to 2) from their properties.
    fn sky_layers(region: &str, other: &[(String, String)]) -> [bool; 3] {
        if region == "CC" || region == "SI" {
            return [false, true, true];
        }
        let mut layers = [false; 3];
        let value = other
            .iter()
            .find(|(k, _)| k == "mapSkyLayers")
            .map(|(_, v)| v);
        for n in value.into_iter().flat_map(|v| v.split(',')) {
            if let Ok(i @ 0..=2) = n.trim().parse::<usize>() {
                layers[i] = true;
            }
        }
        layers
    }

    /// The room of global index `index`, if in the region.
    #[must_use]
    pub fn room(&self, index: i32) -> Option<&MapDataRoom> {
        usize::try_from(index - self.first_room_index)
            .ok()
            .and_then(|i| self.rooms.get(i))
    }

    /// `PositionOfRoom`, `SizeOfRoom` and `LayerOfRoom` of room `index`: zero position
    /// and size and layer 1 outside the region.
    #[must_use]
    pub fn map_room(&self, index: i32) -> MapRoom {
        self.room(index).map_or(
            MapRoom {
                index,
                pos: Vec2::ZERO,
                size: (0, 0),
                layer: 1,
            },
            |r| r.map,
        )
    }

    /// `LayerOfRoom`.
    #[must_use]
    pub fn layer_of_room(&self, index: i32) -> i32 {
        self.map_room(index).layer
    }

    /// The global index of the room named `name` (first match).
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<i32> {
        self.rooms
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.map.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_karma_reads_this_regions_side() {
        let locks = vec![
            vec!["GATE_SU_DS".to_owned(), "4".to_owned(), "2".to_owned()],
            vec!["GATE_LF_SU".to_owned(), "2".to_owned(), "5".to_owned()],
        ];
        // From SU the first side of GATE_SU_DS, the second of GATE_LF_SU.
        assert_eq!(
            GateRequirement::of_gate(&locks, "GATE_SU_DS", "SU"),
            Some(GateRequirement::FourKarma)
        );
        assert_eq!(
            GateRequirement::of_gate(&locks, "GATE_LF_SU", "SU"),
            Some(GateRequirement::FiveKarma)
        );
        assert_eq!(
            GateRequirement::of_gate(&locks, "GATE_SU_DS", "DS"),
            Some(GateRequirement::TwoKarma)
        );
        assert_eq!(
            GateRequirement::of_gate(&locks, "GATE_X_Y", "SU"),
            Some(GateRequirement::OneKarma)
        );
        assert_eq!(GateRequirement::FourKarma.karma(), Some(4));
        assert_eq!(
            GateRequirement::from_value("L"),
            Some(GateRequirement::OeLock)
        );
    }

    #[test]
    fn sky_layers_of_cc_and_properties() {
        assert_eq!(MapData::sky_layers("CC", &[]), [false, true, true]);
        let other = vec![("mapSkyLayers".to_owned(), "0, 2".to_owned())];
        assert_eq!(MapData::sky_layers("XX", &other), [true, false, true]);
        assert_eq!(MapData::sky_layers("SU", &[]), [false; 3]);
    }
}
