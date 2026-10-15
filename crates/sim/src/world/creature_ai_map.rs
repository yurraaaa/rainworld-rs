//! `CreatureSpecificAImap` for the Fly: its accessible tiles and the per-node exit
//! distances baked into the room file (layer 6), as `RoomPreprocessor.DecompressStringToAImaps`
//! loads them. Only the Fly's is built; the other creatures' layers wait for their AI
//! (`rainworld-re/notes/ai-map.md`).

use rw_assets::{Room, ShortcutType};

use super::AiMap;
use crate::creature::CreatureTemplate;

/// One creature's baked exit-distance grid over the bounding box of its accessible tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureSpecificAiMap {
    /// `coveredArea`: left, bottom, right, top (inclusive).
    covered: (i32, i32, i32, i32),
    /// The grid's size in x and y.
    size: (i32, i32),
    number_of_nodes: i32,
    /// `intGrid[x, y, node]`, node innermost; -1 off the accessible tiles.
    grid: Vec<i32>,
    accessible_tiles: Vec<(i32, i32)>,
    /// How many ints the room file's layer held.
    baked_ints: usize,
}

impl CreatureSpecificAiMap {
    /// The constructor (the bounding box of the template's accessible tiles, 0 on them)
    /// then `LoadFromCompressedIntGrid` of the room file's layer for the template.
    #[must_use]
    pub fn new(ai_map: &AiMap, room: &Room, template: &CreatureTemplate) -> Self {
        let number_of_nodes = Self::nodes_relevant_to_creature(room, template);
        let accessible_tiles: Vec<(i32, i32)> = (0..room.width())
            .flat_map(|x| (0..room.height()).map(move |y| (x, y)))
            .filter(|&(x, y)| ai_map.tile_accessible_to_creature(x, y, template))
            .collect();
        let covered = accessible_tiles.iter().fold(
            (room.width(), room.height(), 0, 0),
            |(l, b, r, t), &(x, y)| (l.min(x), b.min(y), r.max(x), t.max(y)),
        );
        let size = if accessible_tiles.is_empty() {
            (1, 1)
        } else {
            (covered.2 - covered.0 + 1, covered.3 - covered.1 + 1)
        };
        let cells = usize::try_from(size.0 * size.1 * number_of_nodes).unwrap_or(0);
        let mut map = Self {
            covered,
            size,
            number_of_nodes,
            grid: vec![-1; cells],
            accessible_tiles,
            baked_ints: 0,
        };
        for i in 0..map.accessible_tiles.len() {
            let (x, y) = map.accessible_tiles[i];
            for node in 0..number_of_nodes {
                if let Some(c) = map.cell(x - covered.0, y - covered.1, node) {
                    map.grid[c] = 0;
                }
            }
        }
        let layer = usize::try_from(template.pre_baked_pathing_index())
            .ok()
            .and_then(|l| room.baked_layer(l));
        if let Some(values) = layer {
            map.load_from_compressed_int_grid(values);
        }
        map
    }

    /// `AIdataPreprocessor.NodesRelevantToCreature`: the room exits while the shortcut
    /// leading to node `i` is a room exit, the dens, the region transports, then the
    /// hives, as the template maps them (border exits and garbage holes are not mapped by
    /// the creatures this is built for).
    fn nodes_relevant_to_creature(room: &Room, template: &CreatureTemplate) -> i32 {
        use rw_assets::NodeType;
        let map = room.shortcut_map();
        let nodes = map.nodes().len();
        let mut n = 0;
        if template.maps_node(NodeType::Exit) {
            n += (0..nodes)
                .take_while(|&i| {
                    map.leading_to_node(i).map(|s| s.kind) == Some(ShortcutType::RoomExit)
                })
                .count();
        }
        if template.maps_node(NodeType::Den) {
            n += (0..nodes)
                .filter(|&i| {
                    map.leading_to_node(i).map(|s| s.kind) == Some(ShortcutType::CreatureHole)
                })
                .count();
        }
        if template.maps_node(NodeType::RegionTransportation) {
            n += (0..nodes)
                .filter(|&i| {
                    map.leading_to_node(i).map(|s| s.kind)
                        == Some(ShortcutType::RegionTransportation)
                })
                .count();
        }
        if template.maps_node(NodeType::BatHive) {
            n += room.hives().len();
        }
        i32::try_from(n).unwrap_or(i32::MAX)
    }

    /// The flat index of grid cell `(gx, gy, node)`.
    fn cell(&self, gx: i32, gy: i32, node: i32) -> Option<usize> {
        let inside = (0..self.size.0).contains(&gx)
            && (0..self.size.1).contains(&gy)
            && (0..self.number_of_nodes).contains(&node);
        inside.then(|| {
            usize::try_from((gx * self.size.1 + gy) * self.number_of_nodes + node).unwrap_or(0)
        })
    }

    /// `LoadFromCompressedIntGrid`: the values in x, y, node order over the cells that are
    /// not -1, stopping when the array runs out (the rest stay 0).
    fn load_from_compressed_int_grid(&mut self, values: &[i32]) {
        self.baked_ints = values.len();
        let mut next = values.iter();
        for cell in self.grid.iter_mut().filter(|c| **c != -1) {
            let Some(&v) = next.next() else { return };
            *cell = v;
        }
    }

    /// `GetDistanceToExit` (the node is creature-specific; -1 outside the covered area).
    #[must_use]
    pub fn distance_to_exit(&self, x: i32, y: i32, node: i32) -> i32 {
        let (left, bottom, right, top) = self.covered;
        if node == -1 || !(left..=right).contains(&x) || !(bottom..=top).contains(&y) {
            return -1;
        }
        self.cell(x - left, y - bottom, node)
            .map_or(-1, |c| self.grid[c])
    }

    #[must_use]
    pub fn accessible_tiles(&self) -> &[(i32, i32)] {
        &self.accessible_tiles
    }

    #[must_use]
    pub fn number_of_nodes(&self) -> i32 {
        self.number_of_nodes
    }

    /// `coveredArea` as (left, bottom, right, top).
    #[must_use]
    pub fn covered_area(&self) -> (i32, i32, i32, i32) {
        self.covered
    }

    /// The grid cells that hold a baked value (not -1).
    #[must_use]
    pub fn cells(&self) -> usize {
        self.grid.iter().filter(|&&c| c != -1).count()
    }

    /// The ints the room file's layer held.
    #[must_use]
    pub fn baked_ints(&self) -> usize {
        self.baked_ints
    }

    /// Every grid cell in x, y, node order (-1 off the accessible tiles).
    #[must_use]
    pub fn grid(&self) -> &[i32] {
        &self.grid
    }
}
