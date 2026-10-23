//! A room's AI map (`AImap`): per tile the accessibility class, the movement
//! connections in and out, floor altitudes, fall risk, water and the baked visibility,
//! the terrain proximity grid, and the pre-baked creature maps decoded from the room
//! file. [`super::ai_mapper`] builds it as `AImapper` does; the queries here are the
//! game's `TileAccessibleToCreature`, `IsConnectionAllowedForCreature` and the costs
//! the pathfinders read (`rainworld-re/notes/ai-map.md`).
//!
//! Mirrors: AImap.cs, AItile.cs

use rw_assets::{CreatureType, NodeType, Room, RoomBorderExit, ShortcutMap, ShortcutType};

use super::CreatureSpecificAiMap;
use super::movement::{Accessibility, Legality, MovementConnection, MovementType, PathCost};
use crate::creature::{CreatureTemplate, WorldCoordinate};

/// `Custom.fourDirections`.
pub(crate) const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// `Custom.fourDirectionsAndZero`.
pub(crate) const FOUR_DIRECTIONS_AND_ZERO: [(i32, i32); 5] =
    [(0, 0), (-1, 0), (0, -1), (1, 0), (0, 1)];

/// `Custom.eightDirections`.
pub(crate) const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// `AItile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTile {
    pub acc: Accessibility,
    pub incoming_paths: Vec<MovementConnection>,
    pub outgoing_paths: Vec<MovementConnection>,
    /// Neither Air nor Solid.
    pub walkable: bool,
    pub narrow_space: bool,
    /// Tiles above the floor below, 100000 without one.
    pub floor_altitude: i32,
    pub smoothed_floor_altitude: i32,
    /// `waterInt`: 1 deep water, 2 the surface.
    water: u8,
    pub visibility: i32,
    /// The tile a fall from here lands on, (-1, -1) out of the room's bottom.
    pub fall_risk_tile: (i32, i32),
}

impl AiTile {
    pub(crate) fn new(acc: Accessibility, water: u8) -> Self {
        Self {
            acc,
            incoming_paths: Vec::new(),
            outgoing_paths: Vec::new(),
            walkable: acc != Accessibility::Air && acc != Accessibility::Solid,
            narrow_space: false,
            floor_altitude: 100_000,
            smoothed_floor_altitude: 100_000,
            water,
            visibility: 0,
            fall_risk_tile: (0, 0),
        }
    }

    #[must_use]
    pub fn deep_water(&self) -> bool {
        self.water == 1
    }

    #[must_use]
    pub fn water_surface(&self) -> bool {
        self.water == 2
    }

    #[must_use]
    pub fn any_water(&self) -> bool {
        self.water != 0
    }
}

/// A room's AI map (x-major grids, `ExtraExtentions.ind(x, y, height)`).
#[derive(Debug, Clone, PartialEq)]
pub struct AiMap {
    pub(crate) width: i32,
    pub(crate) height: i32,
    /// The room's global index (`WrldCrd`; `getAItile(WorldCoordinate)` checks it).
    room_index: i32,
    pub(crate) tiles: Vec<AiTile>,
    pub(crate) terrain_proximity: Vec<i32>,
    /// `standardTile`, what lookups outside the room read. The mapper's fall risk pass
    /// can write its `fallRiskTile`, as the game's shared instance.
    pub(crate) standard: AiTile,
    shortcuts: ShortcutMap,
    border_exits: Vec<RoomBorderExit>,
    hives: Vec<Vec<(i32, i32)>>,
    garbage_holes: bool,
    /// The room's `Water` access modifier with `WaterIsLethal` (the LethalWater effect).
    lethal_water: bool,
    /// `creatureSpecificAImaps`, by `PreBakedPathingIndex`.
    creature_maps: Vec<CreatureSpecificAiMap>,
}

impl AiMap {
    /// The `AImapper`'s map for `room`, then `DecompressStringToAImaps` of the room
    /// file's baked visibility and creature layers. `room_index` is its global index;
    /// shelters and gates have no border exits; `lethal_water` is a room with water
    /// and the LethalWater effect.
    #[must_use]
    pub fn new(room: &Room, room_index: i32, shelter_or_gate: bool, lethal_water: bool) -> Self {
        let (width, height) = (room.width(), room.height());
        let cells = usize::try_from(width * height).unwrap_or(0);
        let mut map = Self {
            width,
            height,
            room_index,
            tiles: Vec::with_capacity(cells),
            terrain_proximity: vec![-1; cells],
            standard: AiTile::new(Accessibility::Solid, 0),
            shortcuts: room.shortcut_map().clone(),
            border_exits: room.border_exits(shelter_or_gate),
            hives: room.hives().to_vec(),
            garbage_holes: !room.garbage_holes().is_empty(),
            lethal_water: lethal_water && room.water_level().is_some(),
            creature_maps: Vec::new(),
        };
        map.map_room(room);
        map.set_visibility(room);
        map.creature_maps = CreatureTemplate::PRE_BAKED_PATHING_CREATURES
            .iter()
            .enumerate()
            .map(|(i, &kind)| {
                let template = CreatureTemplate::of(kind, rw_assets::ActiveMods::NONE);
                CreatureSpecificAiMap::new(&map, room, &template, i)
            })
            .collect();
        map
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    #[must_use]
    pub fn room_index(&self) -> i32 {
        self.room_index
    }

    pub(crate) fn index(&self, x: i32, y: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.height).contains(&y))
            .then(|| usize::try_from(x * self.height + y).unwrap_or(0))
    }

    /// `WrldCrd`: the tile in this room, node undefined.
    #[must_use]
    pub fn world_coordinate(&self, tile: (i32, i32)) -> WorldCoordinate {
        WorldCoordinate::new(self.room_index, tile.0, tile.1, -1)
    }

    /// `getAItile(x, y)`: the standard tile (Solid) outside the room.
    #[must_use]
    pub fn tile(&self, x: i32, y: i32) -> &AiTile {
        self.index(x, y).map_or(&self.standard, |i| &self.tiles[i])
    }

    pub(crate) fn tile_mut(&mut self, x: i32, y: i32) -> &mut AiTile {
        match self.index(x, y) {
            Some(i) => &mut self.tiles[i],
            None => &mut self.standard,
        }
    }

    /// `getAItile(WorldCoordinate)`: the standard tile for another room or an undefined
    /// tile.
    #[must_use]
    pub fn tile_at(&self, pos: WorldCoordinate) -> &AiTile {
        if pos.room == self.room_index && pos.tile_defined() {
            self.tile(pos.tile.0, pos.tile.1)
        } else {
            &self.standard
        }
    }

    /// A tile clamped into the map.
    #[must_use]
    pub fn clamp_tile(&self, t: (i32, i32)) -> (i32, i32) {
        (
            t.0.clamp(0, self.width() - 1),
            t.1.clamp(0, self.height() - 1),
        )
    }

    /// `getClampedAItile`.
    #[must_use]
    pub fn clamped_tile(&self, x: i32, y: i32) -> &AiTile {
        self.tile(x.clamp(0, self.width - 1), y.clamp(0, self.height - 1))
    }

    /// `getTerrainProximity`: -1 outside the room.
    #[must_use]
    pub fn terrain_proximity(&self, x: i32, y: i32) -> i32 {
        self.index(x, y).map_or(-1, |i| self.terrain_proximity[i])
    }

    /// `getTerrainProximity(Vector2 pos)`.
    #[must_use]
    pub fn terrain_proximity_at(&self, pos: crate::Vec2) -> i32 {
        let (x, y) = pos.tile();
        self.terrain_proximity(x, y)
    }

    /// `terrainProximity[ind(x, y, height)]` for a tile inside the room (a tile the mapper
    /// has not reached holds -1, as `getTerrainProximity` outside the room returns).
    #[must_use]
    pub fn terrain_proximity_inside(&self, x: i32, y: i32) -> Option<i32> {
        self.index(x, y).map(|i| self.terrain_proximity[i])
    }

    /// `getAItile(x, y).narrowSpace`.
    #[must_use]
    pub fn narrow_space(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).narrow_space
    }

    /// `getAItile(x, y).visibility` (0 outside the room and on Solid tiles).
    #[must_use]
    pub fn visibility(&self, x: i32, y: i32) -> i32 {
        self.tile(x, y).visibility
    }

    #[must_use]
    pub fn floor_altitude(&self, x: i32, y: i32) -> i32 {
        self.tile(x, y).floor_altitude
    }

    #[must_use]
    pub fn smoothed_floor_altitude(&self, x: i32, y: i32) -> i32 {
        self.tile(x, y).smoothed_floor_altitude
    }

    #[must_use]
    pub fn any_water(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).any_water()
    }

    #[must_use]
    pub fn deep_water(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).deep_water()
    }

    #[must_use]
    pub fn water_surface(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).water_surface()
    }

    /// `room.GetTile(tile).Terrain == ShortcutEntrance` (an entrance only reads as one
    /// inside the room).
    #[must_use]
    pub fn is_shortcut_entrance(&self, tile: (i32, i32)) -> bool {
        self.shortcuts.index_of(tile).is_some()
    }

    /// `room.shortcutData(tile).shortCutType`.
    #[must_use]
    pub fn shortcut_type(&self, tile: (i32, i32)) -> ShortcutType {
        self.shortcuts.type_at(tile)
    }

    #[must_use]
    pub fn shortcuts(&self) -> &ShortcutMap {
        &self.shortcuts
    }

    /// `Room.borderExits`.
    #[must_use]
    pub fn border_exits(&self) -> &[RoomBorderExit] {
        &self.border_exits
    }

    /// `Room.hives`.
    #[must_use]
    pub fn hives(&self) -> &[Vec<(i32, i32)>] {
        &self.hives
    }

    /// `SetVisibilityMapFromCompressedArray`: the baked values over the non-Solid tiles,
    /// x-major; stops where the array runs out.
    fn set_visibility(&mut self, room: &Room) {
        let mut values = room.visibility().iter();
        for x in 0..self.width {
            for y in 0..self.height {
                if room.tile(x, y).is_solid() {
                    continue;
                }
                let Some(&v) = values.next() else {
                    return;
                };
                self.tile_mut(x, y).visibility = v;
            }
        }
    }
}

/// Creature queries.
impl AiMap {
    /// `TileAccessibleToCreature`: the template's water rule; a pre-baked layer; the
    /// lethal-water modifier (no water on the tile or under it); the template's
    /// `isTooCloseToTerrain`; then its resistance for the tile's class, a swimmer also
    /// taking any non-Solid water tile. Worm grass is not ported (docs/divergences/ D14).
    #[must_use]
    pub fn tile_accessible_to_creature(
        &self,
        tile: (i32, i32),
        template: &CreatureTemplate,
    ) -> bool {
        let t = self.tile(tile.0, tile.1);
        if !template
            .water_relationship
            .movement_legal(t.deep_water(), t.water_surface())
        {
            return false;
        }
        if template.pre_baked_pathing_index() == -1 {
            return false;
        }
        if self.lethal_water
            && (self.clamped_tile(tile.0, tile.1).any_water()
                || self.clamped_tile(tile.0, tile.1 - 1).any_water())
        {
            return false;
        }
        let pathing = template.pathing();
        match pathing.too_close_to_terrain.verdict(self, tile) {
            1 => return false,
            2 => return true,
            _ => {}
        }
        if !pathing.accessibility_resistance(t.acc).allowed() {
            return pathing.can_swim && t.acc != Accessibility::Solid && t.any_water();
        }
        true
    }

    /// `WorldCoordinateAccessibleToCreature`: an undefined tile is off screen.
    #[must_use]
    pub fn world_coordinate_accessible_to_creature(
        &self,
        pos: WorldCoordinate,
        template: &CreatureTemplate,
    ) -> bool {
        if !pos.tile_defined() {
            return template
                .accessibility_resistance(Accessibility::OffScreen)
                .allowed();
        }
        self.tile_accessible_to_creature(pos.tile, template)
    }

    /// `TileOrNeighborsAccessibleToCreature`.
    #[must_use]
    pub fn tile_or_neighbors_accessible_to_creature(
        &self,
        tile: (i32, i32),
        template: &CreatureTemplate,
    ) -> bool {
        FOUR_DIRECTIONS_AND_ZERO
            .iter()
            .any(|&(dx, dy)| self.tile_accessible_to_creature((tile.0 + dx, tile.1 + dy), template))
    }

    /// `ClampedTileAccessibleToCreature`.
    #[must_use]
    pub fn clamped_tile_accessible_to_creature(
        &self,
        tile: (i32, i32),
        template: &CreatureTemplate,
    ) -> bool {
        let clamped = (
            tile.0.clamp(0, self.width - 1),
            tile.1.clamp(0, self.height - 1),
        );
        self.tile_accessible_to_creature(clamped, template)
    }

    /// `TryForAccessibleNeighbor`: the tile, else its first accessible four-neighbour,
    /// else the tile.
    #[must_use]
    pub fn try_for_accessible_neighbor(
        &self,
        tile: (i32, i32),
        template: &CreatureTemplate,
    ) -> (i32, i32) {
        if self.tile_accessible_to_creature(tile, template) {
            return tile;
        }
        FOUR_DIRECTIONS
            .iter()
            .map(|&(dx, dy)| (tile.0 + dx, tile.1 + dy))
            .find(|&t| self.tile_accessible_to_creature(t, template))
            .unwrap_or(tile)
    }

    /// `IsConnectionAllowedForCreature`: both ends accessible and the type allowed, then
    /// the reaches and drops must cross a gap (the tile next to the start inaccessible),
    /// a floor-to-floor reach up must start under an inaccessible tile, a double reach
    /// up must fit the template's classes, and an open diagonal must have both corners
    /// closed. The snail's drops and the long legs' squeezes are allowed outright.
    #[must_use]
    pub fn is_connection_allowed_for_creature(
        &self,
        c: &MovementConnection,
        template: &CreatureTemplate,
    ) -> bool {
        if let Some(allowed) = self.connection_force_allowed(c, template) {
            return allowed;
        }
        if !self.world_coordinate_accessible_to_creature(c.start, template)
            || !self.world_coordinate_accessible_to_creature(c.dest, template)
            || !template.connection_resistance(c.kind).allowed()
        {
            return false;
        }
        let (start, dest) = (c.start_tile(), c.dest_tile());
        let step = (
            (dest.0 - start.0).clamp(-1, 1),
            (dest.1 - start.1).clamp(-1, 1),
        );
        match c.kind {
            MovementType::ReachUp
                if self.tile(start.0, start.1).acc == Accessibility::Floor
                    && self.tile(dest.0, dest.1).acc == Accessibility::Floor =>
            {
                if self.tile_accessible_to_creature((start.0, start.1 + 1), template) {
                    return false;
                }
                if self.tile_accessible_to_creature((start.0, start.1 - 1), template) {
                    return true;
                }
                self.tile(start.0, start.1 - 1).acc == Accessibility::Solid
            }
            MovementType::DropToClimb
            | MovementType::DropToFloor
            | MovementType::ReachDown
            | MovementType::ReachOverGap
            | MovementType::ReachUp => {
                !self.tile_accessible_to_creature((start.0 + step.0, start.1 + step.1), template)
            }
            MovementType::SemiDiagonalReach => {
                let step = if (dest.0 - start.0).abs() > (dest.1 - start.1).abs() {
                    (step.0, 0)
                } else {
                    (0, step.1)
                };
                !self.tile_accessible_to_creature((start.0 + step.0, start.1 + step.1), template)
            }
            MovementType::DoubleReachUp => {
                let Some(params) = template.pathing().double_reach_up else {
                    return false;
                };
                let acc = [0, 1, 2, 3].map(|dy| self.tile(start.0, start.1 + dy).acc);
                if acc[0] > params[0]
                    || acc[1] > params[1]
                    || acc[2] > params[1]
                    || acc[3] > params[2]
                {
                    return false;
                }
                !self.tile_accessible_to_creature((start.0, start.1 + 1), template)
                    && !self.tile_accessible_to_creature((start.0, start.1 + 2), template)
            }
            MovementType::OpenDiagonal => {
                !self.tile_accessible_to_creature((start.0, dest.1), template)
                    && !self.tile_accessible_to_creature((dest.0, start.1), template)
            }
            _ => true,
        }
    }

    /// `IsConnectionForceAllowedForCreature` (Remix's zero-gravity lizard drops are not
    /// ported): the snail may drop onto any tile out of deep water; the long legs may
    /// take shortcuts and squeezes at Normal shortcuts.
    fn connection_force_allowed(
        &self,
        c: &MovementConnection,
        template: &CreatureTemplate,
    ) -> Option<bool> {
        match template.kind {
            CreatureType::Snail => (c.kind == MovementType::DropToFloor
                && !self.clamped_tile(c.dest.tile.0, c.dest.tile.1).deep_water())
            .then_some(true),
            CreatureType::DaddyLongLegs | CreatureType::BrotherLongLegs => match c.kind {
                MovementType::ShortCut => (self.normal_shortcut_at(c.start)
                    || self.normal_shortcut_at(c.dest))
                .then_some(true),
                MovementType::BigCreatureShortCutSqueeze => (self.normal_entrance_at(c.start)
                    || self.normal_entrance_at(c.dest))
                .then_some(true),
                _ => None,
            },
            _ => None,
        }
    }

    /// A defined tile whose `shortcutData` is a Normal shortcut.
    fn normal_shortcut_at(&self, pos: WorldCoordinate) -> bool {
        pos.tile_defined() && self.shortcut_type(pos.tile) == ShortcutType::Normal
    }

    /// A shortcut entrance tile of a Normal shortcut.
    fn normal_entrance_at(&self, pos: WorldCoordinate) -> bool {
        self.is_shortcut_entrance(pos.tile) && self.shortcut_type(pos.tile) == ShortcutType::Normal
    }

    /// `ConnectionCostForCreature`: illegal when not allowed, plus the type's resistance
    /// times the distance, plus the destination's tile cost.
    #[must_use]
    pub fn connection_cost_for_creature(
        &self,
        c: &MovementConnection,
        template: &CreatureTemplate,
    ) -> PathCost {
        let mut cost = PathCost::new(0.0, Legality::Allowed);
        if !self.is_connection_allowed_for_creature(c, template) {
            cost = cost + PathCost::new(0.0, Legality::IllegalConnection);
        }
        let mut resistance = template.connection_resistance(c.kind);
        resistance.resistance *= c.distance as f32;
        cost = cost + resistance;
        cost + self.tile_cost_for_creature(c.dest.tile, template)
    }

    /// `TileCostForCreature`: illegal off the accessible tiles, else the class's
    /// resistance, a swimmer's water resistance when cheaper, and the Fly's
    /// `100 / terrainProximity`.
    #[must_use]
    pub fn tile_cost_for_creature(
        &self,
        tile: (i32, i32),
        template: &CreatureTemplate,
    ) -> PathCost {
        if !self.tile_accessible_to_creature(tile, template) {
            return PathCost::new(0.0, Legality::IllegalTile);
        }
        let pathing = template.pathing();
        let t = self.tile(tile.0, tile.1);
        let mut cost = pathing.accessibility_resistance(t.acc);
        let water = PathCost::new(pathing.water_pathing_resistance, Legality::Allowed);
        if t.any_water() && pathing.can_swim && water.less_than(cost) {
            cost = water;
        }
        if template.kind == CreatureType::Fly {
            cost.resistance = 100.0 / self.terrain_proximity(tile.0, tile.1) as f32;
        }
        cost
    }

    /// `CreatureSpecificAImap(crit)`: the map of the template's `PreBakedPathingIndex`.
    #[must_use]
    pub fn creature_map(&self, template: &CreatureTemplate) -> Option<&CreatureSpecificAiMap> {
        usize::try_from(template.pre_baked_pathing_index())
            .ok()
            .and_then(|i| self.creature_maps.get(i))
    }

    /// The map of pre-baked layer `index`.
    #[must_use]
    pub fn creature_map_at(&self, index: usize) -> Option<&CreatureSpecificAiMap> {
        self.creature_maps.get(index)
    }

    /// `TriangulateDistance`: the straight distance plus one.
    #[must_use]
    pub fn triangulate_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
        let (dx, dy) = ((a.0 - b.0) as f32, (a.1 - b.1) as f32);
        (dx * dx + dy * dy).sqrt() as i32 + 1
    }

    /// `AccessibilityForCreature`: the baked accessibility, 0 without pre-baked pathing.
    #[must_use]
    pub fn accessibility_for_creature(&self, tile: (i32, i32), template: &CreatureTemplate) -> f32 {
        self.creature_map(template)
            .map_or(0.0, |m| m.accessibility(tile.0, tile.1))
    }

    /// `ExitDistanceForCreatureAndCheckNeighbours`: the first of the tile and its four
    /// neighbours with a distance (-1 when none; node indices are creature-specific).
    #[must_use]
    pub fn exit_distance_for_creature_and_check_neighbours(
        &self,
        tile: (i32, i32),
        node: i32,
        template: &CreatureTemplate,
    ) -> i32 {
        let Some(map) = self.creature_map(template) else {
            return -1;
        };
        if node < 0 {
            return -1;
        }
        FOUR_DIRECTIONS_AND_ZERO
            .iter()
            .map(|&(dx, dy)| map.distance_to_exit(tile.0 + dx, tile.1 + dy, node))
            .find(|&d| d > -1)
            .unwrap_or(-1)
    }

    /// `ExitDistanceForCreature`: -1 on a tile the template cannot reach.
    #[must_use]
    pub fn exit_distance_for_creature(
        &self,
        tile: (i32, i32),
        node: i32,
        template: &CreatureTemplate,
    ) -> i32 {
        if !self.tile_accessible_to_creature(tile, template) {
            return -1;
        }
        self.creature_map(template)
            .map_or(-1, |m| m.distance_to_exit(tile.0, tile.1, node))
    }

    /// `AnyExitReachableFromTile`: a positive distance to some node.
    #[must_use]
    pub fn any_exit_reachable_from_tile(
        &self,
        tile: (i32, i32),
        template: &CreatureTemplate,
    ) -> bool {
        self.creature_map(template).is_some_and(|m| {
            (0..m.number_of_nodes()).any(|n| m.distance_to_exit(tile.0, tile.1, n) > 0)
        })
    }

    /// `AIdataPreprocessor.NodesRelevantToCreature`: the room exits while the shortcut
    /// leading to node `i` is a room exit, the dens, the region transports, the border
    /// exits of mapped types, the hives, the garbage holes, as the template maps them.
    #[must_use]
    pub fn nodes_relevant_to_creature(&self, template: &CreatureTemplate) -> i32 {
        let n = self.creature_specific_nodes(template).len();
        i32::try_from(n).unwrap_or(i32::MAX)
    }

    /// `CreatureSpecificToCommonNodeIndex`: the room's common node index of the
    /// template's node `n`, -1 for none. The common order is the shortcut nodes, the
    /// border exits, the hives, then the garbage holes.
    #[must_use]
    pub fn creature_specific_to_common_node_index(
        &self,
        n: i32,
        template: &CreatureTemplate,
    ) -> i32 {
        usize::try_from(n)
            .ok()
            .and_then(|n| self.common_nodes_of(template).get(n).copied())
            .unwrap_or(-1)
    }

    /// The template's nodes in creature-specific order, as `NodesRelevantToCreature`
    /// counts them (the leading room exits stop at the first node that is not one).
    fn creature_specific_nodes(&self, template: &CreatureTemplate) -> Vec<i32> {
        let nodes = self.shortcuts.nodes().len();
        let mut out = Vec::new();
        if template.maps_node(NodeType::Exit) {
            out.extend(
                (0..nodes)
                    .take_while(|&i| self.leading_kind(i) == Some(ShortcutType::RoomExit))
                    .map(Self::node_index),
            );
        }
        self.push_typed_nodes(template, &mut out);
        out
    }

    /// `CreatureSpecificToCommonNodeIndex`'s order: every room exit (not only the
    /// leading ones), then as [`Self::creature_specific_nodes`].
    fn common_nodes_of(&self, template: &CreatureTemplate) -> Vec<i32> {
        let nodes = self.shortcuts.nodes().len();
        let mut out = Vec::new();
        if template.maps_node(NodeType::Exit) {
            out.extend(
                (0..nodes)
                    .filter(|&i| self.leading_kind(i) == Some(ShortcutType::RoomExit))
                    .map(Self::node_index),
            );
        }
        self.push_typed_nodes(template, &mut out);
        out
    }

    /// The dens, region transports, border exits, hives and garbage holes the template
    /// maps, as common node indices.
    fn push_typed_nodes(&self, template: &CreatureTemplate, out: &mut Vec<i32>) {
        let nodes = self.shortcuts.nodes().len();
        for (node, kind) in [
            (NodeType::Den, ShortcutType::CreatureHole),
            (
                NodeType::RegionTransportation,
                ShortcutType::RegionTransportation,
            ),
        ] {
            if template.maps_node(node) {
                out.extend(
                    (0..nodes)
                        .filter(|&i| self.leading_kind(i) == Some(kind))
                        .map(Self::node_index),
                );
            }
        }
        for (l, exit) in self.border_exits.iter().enumerate() {
            if template.maps_node(exit.kind) {
                out.push(Self::node_index(nodes + l));
            }
        }
        let borders = self.border_exits.len();
        if template.maps_node(NodeType::BatHive) {
            out.extend((0..self.hives.len()).map(|m| Self::node_index(nodes + borders + m)));
        }
        if template.maps_node(NodeType::GarbageHoles) && self.garbage_holes {
            out.push(Self::node_index(nodes + borders + self.hives.len()));
        }
    }

    /// The type of the shortcut leading to node `i` (`ShortcutLeadingToNode`).
    fn leading_kind(&self, i: usize) -> Option<ShortcutType> {
        self.shortcuts.leading_to_node(i).map(|s| s.kind)
    }

    fn node_index(i: usize) -> i32 {
        i32::try_from(i).unwrap_or(-1)
    }

    /// The tiles a node's map starts from (`NodeMapper`): a shortcut node's entrance,
    /// a border exit's tiles or a hive's tiles, by common index.
    #[must_use]
    pub fn node_start_tiles(&self, common: i32) -> Vec<(i32, i32)> {
        let Ok(common) = usize::try_from(common) else {
            return Vec::new();
        };
        let nodes = self.shortcuts.nodes().len();
        if common < nodes {
            return self
                .shortcuts
                .leading_to_node(common)
                .map(|s| vec![s.start])
                .unwrap_or_default();
        }
        if let Some(exit) = self.border_exits.get(common - nodes) {
            return exit.tiles.clone();
        }
        self.hives
            .get(common - nodes - self.border_exits.len())
            .cloned()
            .unwrap_or_default()
    }
}
