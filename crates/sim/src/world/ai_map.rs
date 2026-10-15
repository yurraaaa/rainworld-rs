//! The parts of a room's `AImap` the overseer reads: terrain proximity
//! (`AImapper.TerrainProximityMapper`), `narrowSpace` (`FindPassagesOfCurrentTile`) and
//! the baked `visibility` (room file line 10). Accessibility, passages, fall risk and the
//! creature-specific maps are not ported (`rainworld-re/notes/ai-map.md`).

use std::collections::VecDeque;

use rw_assets::{Room, Terrain};

/// `Custom.fourDirections`.
const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

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

/// A room's AI map (x-major grids, `ExtraExtentions.ind(x, y, height)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiMap {
    width: i32,
    height: i32,
    terrain_proximity: Vec<i32>,
    narrow_space: Vec<bool>,
    visibility: Vec<i32>,
}

impl AiMap {
    /// The mapper's result for `room` (made on the room preparer's thread before
    /// `ReadyForAI`).
    #[must_use]
    pub fn new(room: &Room) -> Self {
        let (width, height) = (room.width(), room.height());
        let cells = usize::try_from(width * height).unwrap_or(0);
        let mut map = Self {
            width,
            height,
            terrain_proximity: vec![-1; cells],
            narrow_space: vec![false; cells],
            visibility: vec![0; cells],
        };
        map.map_terrain_proximity(room);
        map.map_narrow_spaces(room);
        map.set_visibility(room);
        map
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.height).contains(&y))
            .then(|| usize::try_from(x * self.height + y).unwrap_or(0))
    }

    /// `TerrainProximityMapper`: Solid tiles 0, open tiles touching a Solid one (four
    /// directions, clamped at the edges) 1, the rest by breadth-first steps from those,
    /// queued in x-major order.
    fn map_terrain_proximity(&mut self, room: &Room) {
        let mut queue = VecDeque::new();
        for x in 0..self.width {
            for y in 0..self.height {
                let i = x as usize * self.height as usize + y as usize;
                if room.tile(x, y).is_solid() {
                    self.terrain_proximity[i] = 0;
                } else if FOUR_DIRECTIONS
                    .iter()
                    .any(|(dx, dy)| room.tile(x + dx, y + dy).is_solid())
                {
                    self.terrain_proximity[i] = 1;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let here = self.terrain_proximity(x, y);
            for (dx, dy) in FOUR_DIRECTIONS {
                if let Some(i) = self.index(x + dx, y + dy)
                    && self.terrain_proximity[i] < 0
                {
                    self.terrain_proximity[i] = here + 1;
                    queue.push_back((x + dx, y + dy));
                }
            }
        }
    }

    /// `narrowSpace`: a tile with Solid on both sides along one of the first four
    /// `eightDirections` (clamped lookups).
    fn map_narrow_spaces(&mut self, room: &Room) {
        for x in 0..self.width {
            for y in 0..self.height {
                let narrow = EIGHT_DIRECTIONS[..4].iter().any(|(dx, dy)| {
                    room.tile(x + dx, y + dy).terrain == Terrain::Solid
                        && room.tile(x - dx, y - dy).terrain == Terrain::Solid
                });
                let i = x as usize * self.height as usize + y as usize;
                self.narrow_space[i] = narrow;
            }
        }
    }

    /// `SetVisibilityMapFromCompressedArray`: the baked values over the non-Solid tiles.
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
                let i = x as usize * self.height as usize + y as usize;
                self.visibility[i] = v;
            }
        }
    }

    /// `getTerrainProximity`: -1 outside the room.
    #[must_use]
    pub fn terrain_proximity(&self, x: i32, y: i32) -> i32 {
        self.index(x, y).map_or(-1, |i| self.terrain_proximity[i])
    }

    /// `getAItile(x, y).narrowSpace` (false for the standard tile outside the room).
    #[must_use]
    pub fn narrow_space(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.narrow_space[i])
    }

    /// `getAItile(x, y).visibility` (0 outside the room and on Solid tiles).
    #[must_use]
    pub fn visibility(&self, x: i32, y: i32) -> i32 {
        self.index(x, y).map_or(0, |i| self.visibility[i])
    }
}
