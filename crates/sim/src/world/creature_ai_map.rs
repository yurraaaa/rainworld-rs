//! `CreatureSpecificAImap`: one pre-baked pathing creature's accessible tiles, the
//! per-node exit distances and the accessibility grid baked into the room file, as
//! `RoomPreprocessor.DecompressStringToAImaps` loads them.

use rw_assets::Room;

use super::AiMap;
use crate::creature::CreatureTemplate;

/// One creature's baked exit-distance grid over the bounding box of its accessible
/// tiles, and its accessibility grid over the room.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureSpecificAiMap {
    /// `coveredArea`: left, bottom, right, top (inclusive).
    covered: (i32, i32, i32, i32),
    /// The grid's size in x and y.
    size: (i32, i32),
    number_of_nodes: i32,
    /// `intGrid[x, y, node]`, node innermost; -1 off the accessible tiles.
    grid: Vec<i32>,
    /// `floatGrid[x, y]` over the room (1 by 1 without accessible tiles).
    float_size: (i32, i32),
    float_grid: Vec<f32>,
    accessible_tiles: Vec<(i32, i32)>,
    /// How many ints the room file's layer held.
    baked_ints: usize,
}

impl CreatureSpecificAiMap {
    /// The constructor (the bounding box of the template's accessible tiles, 0 on them)
    /// then `LoadFromCompressedIntGrid` and `LoadFromCompressedFloatGrid` of the room
    /// file's layer `layer`.
    #[must_use]
    pub fn new(ai_map: &AiMap, room: &Room, template: &CreatureTemplate, layer: usize) -> Self {
        let number_of_nodes = ai_map.nodes_relevant_to_creature(template);
        let accessible_tiles: Vec<(i32, i32)> = (0..room.width())
            .flat_map(|x| (0..room.height()).map(move |y| (x, y)))
            .filter(|&t| ai_map.tile_accessible_to_creature(t, template))
            .collect();
        let covered = accessible_tiles.iter().fold(
            (room.width(), room.height(), 0, 0),
            |(l, b, r, t), &(x, y)| (l.min(x), b.min(y), r.max(x), t.max(y)),
        );
        let (size, float_size) = if accessible_tiles.is_empty() {
            ((1, 1), (1, 1))
        } else {
            (
                (covered.2 - covered.0 + 1, covered.3 - covered.1 + 1),
                (room.width(), room.height()),
            )
        };
        let cells = usize::try_from(size.0 * size.1 * number_of_nodes).unwrap_or(0);
        // Without an accessible tile the game's -1 fill loop runs over an empty range, so
        // its one cell per node stays 0 and loads a baked value (never read).
        let fill = if accessible_tiles.is_empty() { 0 } else { -1 };
        let mut map = Self {
            covered,
            size,
            number_of_nodes,
            grid: vec![fill; cells],
            float_size,
            float_grid: vec![0.0; usize::try_from(float_size.0 * float_size.1).unwrap_or(0)],
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
        if let Some(values) = room.baked_layer(layer) {
            map.load_from_compressed_int_grid(values);
        }
        if let Some(values) = room.baked_float_layer(layer) {
            map.float_grid
                .iter_mut()
                .zip(values)
                .for_each(|(cell, &v)| *cell = v);
        }
        map
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

    /// `GetAccessibility`: -1 outside the grid.
    #[must_use]
    pub fn accessibility(&self, x: i32, y: i32) -> f32 {
        if (0..self.float_size.0).contains(&x) && (0..self.float_size.1).contains(&y) {
            let i = usize::try_from(x * self.float_size.1 + y).unwrap_or(0);
            self.float_grid[i]
        } else {
            -1.0
        }
    }

    /// `accessableTiles`, x-major.
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

    /// The accessibility grid, x-major.
    #[must_use]
    pub fn float_grid(&self) -> &[f32] {
        &self.float_grid
    }
}
