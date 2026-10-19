//! Room geometry files (`<room>.txt`): size, water, cameras and the tile grid.
//!
//! Format: `rainworld-re/notes/room-format.md`. Lines 0, 1, 3, 5 (the items placed on a
//! first realization), 9 (the node map), 10 (the baked AI data's visibility and creature
//! int grids) and 11 are read; the light angle, the baked float grids and the
//! `_settings.txt` file are skipped.
//! The shortcut map ([`crate::ShortcutMap`]) is derived from the grid on parse.
//!
//! Mirrors: Room.cs

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{ConnMap, ShortcutMap};

/// Side of a square tile in room pixels.
pub const TILE_SIZE: f32 = 20.0;

/// `Custom.eightDirections`.
const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// Index of the tile-grid line in a geometry file.
const GRID_LINE: usize = 11;

/// Base terrain of a tile (first number of a cell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Terrain {
    Air = 0,
    Solid = 1,
    /// Orientation is not stored; it is derived from neighbours.
    Slope = 2,
    /// One-way platform, solid only when landing from above.
    Floor = 3,
    ShortcutEntrance = 4,
}

impl TryFrom<i32> for Terrain {
    type Error = i32;

    fn try_from(v: i32) -> Result<Self, i32> {
        Ok(match v {
            0 => Self::Air,
            1 => Self::Solid,
            2 => Self::Slope,
            3 => Self::Floor,
            4 => Self::ShortcutEntrance,
            other => return Err(other),
        })
    }
}

/// Shortcut role of a tile (the game's `Tile.shortCut`, 0..=5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shortcut {
    #[default]
    None = 0,
    /// Flag 3: part of a shortcut tube.
    Path = 1,
    /// Flag 4: room exit.
    RoomExit = 2,
    /// Flag 5: creature den / hole.
    CreatureDen = 3,
    /// Flag 9: NPC transportation.
    NpcTransportation = 4,
    /// Flag 12: region transportation.
    RegionTransportation = 5,
}

/// Boolean tile flags from a cell (shortcut flags are in [`Shortcut`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TileFlags(u8);

impl TileFlags {
    pub const NONE: Self = Self(0);
    /// Flag 1.
    pub const VERTICAL_POLE: Self = Self(1 << 0);
    /// Flag 2.
    pub const HORIZONTAL_POLE: Self = Self(1 << 1);
    /// Flag 6: back wall.
    pub const WALL_BEHIND: Self = Self(1 << 2);
    /// Flag 7: bat hive.
    pub const HIVE: Self = Self(1 << 3);
    /// Flag 8.
    pub const WATERFALL: Self = Self(1 << 4);
    /// Flag 10.
    pub const GARBAGE_HOLE: Self = Self(1 << 5);
    /// Flag 11.
    pub const WORM_GRASS: Self = Self(1 << 6);

    /// Raw bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True if every flag in `other` is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    const fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Sets or clears every flag in `other`.
    pub const fn set(&mut self, other: Self, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }
}

/// One grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tile {
    pub terrain: Terrain,
    pub shortcut: Shortcut,
    pub flags: TileFlags,
}

impl Tile {
    /// `Terrain::Solid` only, as the game's `Tile.Solid`.
    #[must_use]
    pub fn is_solid(self) -> bool {
        self.terrain == Terrain::Solid
    }

    #[must_use]
    pub fn vertical_pole(self) -> bool {
        self.flags.contains(TileFlags::VERTICAL_POLE)
    }

    #[must_use]
    pub fn horizontal_pole(self) -> bool {
        self.flags.contains(TileFlags::HORIZONTAL_POLE)
    }

    /// Either pole (the game's `AnyBeam`).
    #[must_use]
    pub fn any_pole(self) -> bool {
        self.vertical_pole() || self.horizontal_pole()
    }

    #[must_use]
    pub fn wall_behind(self) -> bool {
        self.flags.contains(TileFlags::WALL_BEHIND)
    }

    /// Parses one `terrain[,flag...]` cell. Unknown flags are ignored, as the game does.
    fn parse(index: usize, cell: &str) -> Result<Self, RoomError> {
        let mut parts = cell.split(',');
        let first = parts.next().unwrap_or_default();
        let value: i32 = first.parse().map_err(|_| RoomError::BadCell {
            index,
            cell: cell.to_owned(),
        })?;
        let terrain =
            Terrain::try_from(value).map_err(|value| RoomError::UnknownTerrain { index, value })?;
        let mut tile = Self {
            terrain,
            shortcut: Shortcut::None,
            flags: TileFlags::NONE,
        };
        for flag in parts {
            match flag {
                "1" => tile.flags.insert(TileFlags::VERTICAL_POLE),
                "2" => tile.flags.insert(TileFlags::HORIZONTAL_POLE),
                // A plain path never overrides a stronger shortcut kind.
                "3" if tile.shortcut == Shortcut::None => tile.shortcut = Shortcut::Path,
                "4" => tile.shortcut = Shortcut::RoomExit,
                "5" => tile.shortcut = Shortcut::CreatureDen,
                "9" => tile.shortcut = Shortcut::NpcTransportation,
                "12" => tile.shortcut = Shortcut::RegionTransportation,
                "6" => tile.flags.insert(TileFlags::WALL_BEHIND),
                "7" => tile.flags.insert(TileFlags::HIVE),
                "8" => tile.flags.insert(TileFlags::WATERFALL),
                "10" => tile.flags.insert(TileFlags::GARBAGE_HOLE),
                "11" => tile.flags.insert(TileFlags::WORM_GRASS),
                _ => {}
            }
        }
        Ok(tile)
    }
}

/// Orientation of a usable slope tile (the game's `Room.SlopeDirection` minus `Broken`).
///
/// The names are the game's and are not geometric; see `slope-collision.md` section 2.
/// `lx`/`h` are measured from the tile's bottom-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slope {
    /// Right and below Solid: floor, solid lower-right triangle, `h = lx`.
    UpLeft,
    /// Left and below Solid: floor, solid lower-left triangle, `h = 20 - lx`.
    UpRight,
    /// Right and above Solid: ceiling, solid upper-right triangle, `h = 20 - lx`.
    DownLeft,
    /// Left and above Solid: ceiling, solid upper-left triangle, `h = lx`.
    DownRight,
}

/// Error from parsing or loading a room file.
#[derive(Debug)]
#[non_exhaustive]
pub enum RoomError {
    /// Reading the file failed.
    Io { path: PathBuf, source: io::Error },
    /// The file has fewer lines than the field needs.
    MissingLine { line: usize },
    /// A header field could not be parsed.
    BadField {
        line: usize,
        field: &'static str,
        value: String,
    },
    /// Width or height is not positive, or `W*H` overflows.
    BadSize { width: i32, height: i32 },
    /// The grid does not hold exactly `W*H` cells.
    CellCount { expected: usize, found: usize },
    /// The grid line does not end with `|`.
    MissingTrailingSeparator,
    /// A cell's terrain is not an integer.
    BadCell { index: usize, cell: String },
    /// A cell's terrain is outside 0..=4.
    UnknownTerrain { index: usize, value: i32 },
}

impl fmt::Display for RoomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } => write!(f, "failed to read {}", path.display()),
            Self::MissingLine { line } => write!(f, "missing line {line}"),
            Self::BadField { line, field, value } => {
                write!(f, "line {line}: invalid {field} {value:?}")
            }
            Self::BadSize { width, height } => write!(f, "invalid room size {width}*{height}"),
            Self::CellCount { expected, found } => {
                write!(f, "tile grid has {found} cells, expected {expected}")
            }
            Self::MissingTrailingSeparator => write!(f, "tile grid does not end with '|'"),
            Self::BadCell { index, cell } => write!(f, "cell {index}: invalid cell {cell:?}"),
            Self::UnknownTerrain { index, value } => {
                write!(f, "cell {index}: unknown terrain {value}")
            }
        }
    }
}

impl Error for RoomError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// One entry of line 5, as written: `LoadFromDataString` places it on the room's first
/// realization in a cycle (0 a rock, 1 a spear; other types still make the draw and take
/// an ID).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomItem {
    pub kind: i32,
    /// 1-based column.
    pub x: i32,
    /// Row counted from the top, 1-based.
    pub y: i32,
}

impl RoomItem {
    /// The tile it is placed on, `(x - 1, height - y)`.
    #[must_use]
    pub fn tile(self, height: i32) -> (i32, i32) {
        (self.x - 1, height - self.y)
    }

    /// Line 5 split on `|`, every part but the last one read (the game's loop bound).
    fn parse_list(line: &str) -> Result<Vec<Self>, RoomError> {
        let parts: Vec<&str> = line.split('|').collect();
        parts[..parts.len() - 1]
            .iter()
            .map(|part| {
                let fields: Vec<&str> = part.split(',').collect();
                Ok(Self {
                    kind: Self::field(&fields, 0, part)?,
                    x: Self::field(&fields, 1, part)?,
                    y: Self::field(&fields, 2, part)?,
                })
            })
            .collect()
    }

    /// `Convert.ToInt32` of field `i` (the game throws on a bad one).
    fn field(fields: &[&str], i: usize, part: &str) -> Result<i32, RoomError> {
        fields
            .get(i)
            .and_then(|f| f.trim().parse().ok())
            .ok_or_else(|| bad(5, "item", part))
    }
}

/// The lines of a geometry file, with checked access.
struct Lines<'a>(Vec<&'a str>);

impl<'a> Lines<'a> {
    fn line(&self, i: usize) -> Result<&'a str, RoomError> {
        self.0
            .get(i)
            .copied()
            .ok_or(RoomError::MissingLine { line: i })
    }
}

/// A room's geometry, read-only.
#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    name: String,
    width: i32,
    height: i32,
    water_level: Option<i32>,
    water_in_front: bool,
    cameras: Vec<[f32; 2]>,
    /// Column-major as in the file: x outer, y from the top row down.
    tiles: Box<[Tile]>,
    pub(crate) shortcut_map: ShortcutMap,
    /// Line 9, or why it does not parse.
    conn_map: Result<ConnMap, String>,
    /// The AI map's `visibility` of each non-Solid tile, x-major (line 10's first part,
    /// `SetVisibilityMapFromCompressedArray`); empty when the line is missing or bad.
    visibility: Vec<i32>,
    /// Line 10's pre-baked creature layers: each `<<DIV - A>>` part's int grid (before its
    /// `<<DIV - B>>`) and float grid (after it), in `preBakedPathingCreatures` order; both
    /// empty when either does not decode (the game skips the layer).
    baked_layers: Vec<(Vec<i32>, Vec<f32>)>,
    /// `Room.hives`: the hive tiles grouped as `AddHiveTile` groups them.
    hives: Vec<Vec<(i32, i32)>>,
    /// Line 5's items.
    items: Vec<RoomItem>,
}

impl Room {
    /// Parses the contents of a geometry file.
    ///
    /// # Errors
    /// Returns [`RoomError`] for missing lines, malformed header fields or cells,
    /// or a grid whose cell count is not `W*H`.
    pub fn parse(text: &str) -> Result<Self, RoomError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let lines = Lines(text.lines().collect());

        let name = lines.line(0)?.to_owned();

        // Line 1: `W*H|waterLevel|waterInFront`; the last two are optional.
        let mut header = lines.line(1)?.split('|');
        let size = header.next().unwrap_or_default();
        let (w, h) = size.split_once('*').ok_or_else(|| bad(1, "size", size))?;
        let width: i32 = w.parse().map_err(|_| bad(1, "width", w))?;
        let height: i32 = h.parse().map_err(|_| bad(1, "height", h))?;
        let cell_count = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .filter(|&(w, h)| w > 0 && h > 0)
            .and_then(|(w, h)| w.checked_mul(h))
            .ok_or(RoomError::BadSize { width, height })?;
        let water_level = match header.next() {
            Some(s) => Some(s.parse::<i32>().map_err(|_| bad(1, "water level", s))?),
            None => None,
        }
        .filter(|&level| level != -1);
        let water_in_front = match header.next() {
            Some(s) => s.parse::<i32>().map_err(|_| bad(1, "water in front", s))? == 1,
            None => false,
        };
        // The game reads this flag only when the room has water.
        let water_in_front = water_level.is_some() && water_in_front;

        // Line 3: cameras `x,y|x,y`, converted to room pixels (y up, image bottom-left).
        let cameras = lines
            .line(3)?
            .split('|')
            .map(|cam| {
                let (x, y) = cam.split_once(',').ok_or_else(|| bad(3, "camera", cam))?;
                let x: f32 = x.parse().map_err(|_| bad(3, "camera x", x))?;
                let y: f32 = y.parse().map_err(|_| bad(3, "camera y", y))?;
                Ok([x, -(800.0 - height as f32 * TILE_SIZE + y)])
            })
            .collect::<Result<Vec<_>, RoomError>>()?;

        // Line 11: `cell|cell|...|` with a trailing separator.
        let grid = lines
            .line(GRID_LINE)?
            .strip_suffix('|')
            .ok_or(RoomError::MissingTrailingSeparator)?;
        let cells: Vec<&str> = grid.split('|').collect();
        if cells.len() != cell_count {
            return Err(RoomError::CellCount {
                expected: cell_count,
                found: cells.len(),
            });
        }
        let tiles = cells
            .iter()
            .enumerate()
            .map(|(i, cell)| Tile::parse(i, cell))
            .collect::<Result<Box<[Tile]>, RoomError>>()?;

        // `VersionFix` moves an 8-line file's last lines down, leaving line 5 empty.
        let items = if lines.0.len() == 8 {
            Vec::new()
        } else {
            RoomItem::parse_list(lines.line(5)?)?
        };
        let conn_map = ConnMap::parse(lines.line(9)?);
        let ai_line = lines.line(10).unwrap_or_default();
        let visibility = Self::parse_visibility(ai_line);
        let baked_layers = Self::parse_baked_layers(ai_line);
        let hives = Self::group_hives(width, height, &tiles);
        let mut room = Self {
            items,
            conn_map,
            visibility,
            baked_layers,
            hives,
            name,
            width,
            height,
            water_level,
            water_in_front,
            cameras,
            tiles,
            shortcut_map: ShortcutMap::default(),
        };
        room.shortcut_map = ShortcutMap::new(&room);
        Ok(room)
    }

    /// Reads and parses a geometry file. Never writes.
    ///
    /// # Errors
    /// [`RoomError::Io`] if the file cannot be read, otherwise as [`Room::parse`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RoomError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| RoomError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Loads `<streaming_assets>/levels/<name>.txt`, lowercasing `name` as the files on disk are.
    ///
    /// # Errors
    /// As [`Room::load`].
    pub fn load_level(streaming_assets: impl AsRef<Path>, name: &str) -> Result<Self, RoomError> {
        let file = format!("{}.txt", name.to_lowercase());
        Self::load(streaming_assets.as_ref().join("levels").join(file))
    }

    /// The baked node map (line 10, `roomText[9]`).
    ///
    /// # Errors
    /// Why the line does not parse.
    pub fn conn_map(&self) -> Result<&ConnMap, String> {
        self.conn_map.as_ref().map_err(Clone::clone)
    }

    /// `RoomPreprocessor.StringToIntArray` of line 10 up to the first `<<DIV - A>>`.
    fn parse_visibility(line: &str) -> Vec<i32> {
        Self::string_to_int_array(line.split("<<DIV - A>>").next().unwrap_or_default())
    }

    /// `DecompressStringToAImaps`' grids: each part after a `<<DIV - A>>` holds the int
    /// grid up to its `<<DIV - B>>` and the float grid after it.
    fn parse_baked_layers(line: &str) -> Vec<(Vec<i32>, Vec<f32>)> {
        line.split("<<DIV - A>>")
            .skip(1)
            .map(|part| {
                let mut halves = part.split("<<DIV - B>>");
                let ints = Self::decode_base64(halves.next().unwrap_or(""));
                let floats = Self::decode_base64(halves.next().unwrap_or(""));
                match (ints, floats) {
                    (Some(i), Some(f)) => (
                        i.as_chunks::<4>()
                            .0
                            .iter()
                            .map(|&b| i32::from_le_bytes(b))
                            .collect(),
                        f.as_chunks::<4>()
                            .0
                            .iter()
                            .map(|&b| f32::from_le_bytes(b))
                            .collect(),
                    ),
                    _ => (Vec::new(), Vec::new()),
                }
            })
            .collect()
    }

    /// `StringToIntArray`: base64 of little-endian `int32`s; empty when it does not decode.
    fn string_to_int_array(text: &str) -> Vec<i32> {
        Self::decode_base64(text).map_or_else(Vec::new, |bytes| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&b| i32::from_le_bytes(b))
                .collect()
        })
    }

    /// `Convert.FromBase64String`; `None` where it throws.
    fn decode_base64(text: &str) -> Option<Vec<u8>> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(text.trim())
            .ok()
    }

    /// `LoadFromDataString`'s hive grouping in file order (x outer, top row first): a hive
    /// tile joins the group of the first earlier hive tile among its eight neighbours,
    /// else starts a new group. Groups never merge.
    fn group_hives(width: i32, height: i32, tiles: &[Tile]) -> Vec<Vec<(i32, i32)>> {
        let mut seen: Vec<((i32, i32), usize)> = Vec::new();
        let mut groups: Vec<Vec<(i32, i32)>> = Vec::new();
        for (i, tile) in tiles.iter().enumerate() {
            if !tile.flags.contains(TileFlags::HIVE) {
                continue;
            }
            let i = i32::try_from(i).unwrap_or(0);
            let at = (i / height, height - 1 - i % height);
            debug_assert!(at.0 < width);
            let group = seen
                .iter()
                .find(|&&(t, _)| {
                    EIGHT_DIRECTIONS
                        .iter()
                        .any(|&(dx, dy)| (at.0 + dx, at.1 + dy) == t)
                })
                .map(|&(_, g)| g);
            let group = group.unwrap_or_else(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[group].push(at);
            seen.push((at, group));
        }
        groups
    }

    /// The int grid of pre-baked creature layer `index` (`PreBakedPathingIndex`), as
    /// stored: one value per accessible tile and node.
    #[must_use]
    pub fn baked_layer(&self, index: usize) -> Option<&[i32]> {
        self.baked_layers
            .get(index)
            .map(|(ints, _)| ints.as_slice())
    }

    /// The float grid of pre-baked creature layer `index` (`AccessibilityDijkstraMapper`'s
    /// accessibility), one value per tile of the room, x-major.
    #[must_use]
    pub fn baked_float_layer(&self, index: usize) -> Option<&[f32]> {
        self.baked_layers
            .get(index)
            .map(|(_, floats)| floats.as_slice())
    }

    /// How many pre-baked creature layers line 10 holds.
    #[must_use]
    pub fn baked_layer_count(&self) -> usize {
        self.baked_layers.len()
    }

    /// Line 5's items, in file order.
    #[must_use]
    pub fn items(&self) -> &[RoomItem] {
        &self.items
    }

    /// `Room.hives`.
    #[must_use]
    pub fn hives(&self) -> &[Vec<(i32, i32)>] {
        &self.hives
    }

    /// `Room.garbageHoles`: the garbage hole tiles in file order.
    #[must_use]
    pub fn garbage_holes(&self) -> Vec<(i32, i32)> {
        let h = self.height;
        self.tiles
            .iter()
            .zip(0..)
            .filter(|(t, _)| t.flags.contains(TileFlags::GARBAGE_HOLE))
            .map(|(_, i): (&Tile, i32)| (i / h, h - 1 - i % h))
            .collect()
    }

    /// `Tile.waterInt` as `LoadFromDataString` sets it: 1 below the water level (deep
    /// water), 2 on it (the surface), else 0. `ModifyTileWaterLevels` (the WaterCutoff
    /// and AirPocket objects, which no vanilla region room places) is not applied. 0
    /// outside the room.
    #[must_use]
    pub fn water_int(&self, x: i32, y: i32) -> u8 {
        if !self.contains(x, y) {
            return 0;
        }
        match self.water_level {
            Some(level) if y < level => 1,
            Some(level) if y == level => 2,
            _ => 0,
        }
    }

    /// The baked `AItile.visibility` of each non-Solid tile in x-major order (x outer, y
    /// up), as `AImap.SetVisibilityMapFromCompressedArray` reads it; empty when the room
    /// file has none.
    #[must_use]
    pub fn visibility(&self) -> &[i32] {
        &self.visibility
    }

    /// Display name from line 0.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Width in tiles.
    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Height in tiles.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Tile row of the water surface (y up), or `None` for no water (`-1` in the file).
    #[must_use]
    pub fn water_level(&self) -> Option<i32> {
        self.water_level
    }

    /// Water drawn in front of terrain; always false without water.
    #[must_use]
    pub fn water_in_front(&self) -> bool {
        self.water_in_front
    }

    /// Camera image bottom-left corners in room pixels (y up). Each image is 1400x800.
    #[must_use]
    pub fn cameras(&self) -> &[[f32; 2]] {
        &self.cameras
    }

    /// All tiles in file order: column-major, x outer, y from `height - 1` down to 0.
    #[must_use]
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// Number of tiles whose terrain is `terrain`.
    #[must_use]
    pub fn count_terrain(&self, terrain: Terrain) -> usize {
        self.tiles.iter().filter(|t| t.terrain == terrain).count()
    }

    /// True if `(x, y)` is inside the room.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        (0..self.width).contains(&x) && (0..self.height).contains(&y)
    }

    /// Tile at `(x, y)` (y up), or `None` outside the room.
    #[must_use]
    pub fn get(&self, x: i32, y: i32) -> Option<Tile> {
        self.contains(x, y).then(|| self.tiles[self.index(x, y)])
    }

    /// Tile at `(x, y)` (y up), clamping out-of-bounds coordinates to the nearest edge tile.
    ///
    /// As the game's `Room.GetTile`, a clamped tile loses its shortcut and a clamped
    /// `ShortcutEntrance` reads as `Solid`.
    #[must_use]
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if let Some(tile) = self.get(x, y) {
            return tile;
        }
        let edge = self.tiles[self.index(x.clamp(0, self.width - 1), y.clamp(0, self.height - 1))];
        Tile {
            terrain: match edge.terrain {
                Terrain::ShortcutEntrance => Terrain::Solid,
                other => other,
            },
            shortcut: Shortcut::None,
            flags: edge.flags,
        }
    }

    /// `GetTile(x, y).Terrain = terrain` for an in-bounds tile (a shelter door's close
    /// tiles); out of bounds does nothing.
    pub fn set_terrain(&mut self, x: i32, y: i32, terrain: Terrain) {
        if self.contains(x, y) {
            let i = self.index(x, y);
            self.tiles[i].terrain = terrain;
        }
    }

    /// `GetTile(x, y).verticalBeam` / `.horizontalBeam = on` (`flag` is
    /// [`TileFlags::VERTICAL_POLE`] or [`TileFlags::HORIZONTAL_POLE`]): a spear stuck in a
    /// wall makes a pole. Out of bounds it writes the clamped edge tile, as the game's
    /// `GetTile` hands that tile out, unless the edge tile is a shortcut (the game hands
    /// out a copy, so the write is lost).
    pub fn set_pole(&mut self, x: i32, y: i32, flag: TileFlags, on: bool) {
        let (cx, cy) = (x.clamp(0, self.width - 1), y.clamp(0, self.height - 1));
        let i = self.index(cx, cy);
        let tile = self.tiles[i];
        let copy = tile.terrain == Terrain::ShortcutEntrance || tile.shortcut != Shortcut::None;
        if self.contains(x, y) || !copy {
            self.tiles[i].flags.set(flag, on);
        }
    }

    /// Tile containing the room-pixel position `pos`, clamped as [`Room::tile`].
    #[must_use]
    pub fn tile_at(&self, pos: [f32; 2]) -> Tile {
        let (x, y) = tile_position(pos);
        self.tile(x, y)
    }

    /// Orientation of the slope tile at `(x, y)`, derived from its neighbours as the
    /// game's `Room.IdentifySlope`; `None` is the game's `Broken`.
    ///
    /// Rules (`rainworld-re/notes/slope-collision.md` section 1): the tile must be
    /// [`Terrain::Slope`]. If the left neighbour is Solid the result is `UpRight` (below
    /// Solid) or `DownRight` (above Solid) or `None`; the right neighbour is consulted
    /// only when the left one is not Solid, giving `UpLeft` / `DownLeft` / `None`.
    /// Neighbours are read with [`Room::tile`], so out-of-bounds lookups clamp.
    #[must_use]
    pub fn identify_slope(&self, x: i32, y: i32) -> Option<Slope> {
        if self.tile(x, y).terrain != Terrain::Slope {
            return None;
        }
        let (floor, ceiling) = if self.is_solid_at(x, y, -1, 0) {
            (Slope::UpRight, Slope::DownRight)
        } else if self.is_solid_at(x, y, 1, 0) {
            (Slope::UpLeft, Slope::DownLeft)
        } else {
            return None;
        };
        if self.is_solid_at(x, y, 0, -1) {
            Some(floor)
        } else if self.is_solid_at(x, y, 0, 1) {
            Some(ceiling)
        } else {
            None
        }
    }

    /// Whether the tile offset by `(dx, dy)` from `(x, y)` is Solid, clamped as [`Room::tile`].
    fn is_solid_at(&self, x: i32, y: i32, dx: i32, dy: i32) -> bool {
        self.tile(x + dx, y + dy).is_solid()
    }

    /// Room width in pixels (the game's `PixelWidth`).
    #[must_use]
    pub fn pixel_width(&self) -> f32 {
        self.width as f32 * TILE_SIZE
    }

    /// Room height in pixels (the game's `PixelHeight`).
    #[must_use]
    pub fn pixel_height(&self) -> f32 {
        self.height as f32 * TILE_SIZE
    }

    /// Index into [`Room::tiles`]; `(x, y)` must be in bounds.
    fn index(&self, x: i32, y: i32) -> usize {
        debug_assert!(self.contains(x, y));
        // In bounds, so all three are non-negative.
        let (x, y, h) = (x as usize, y as usize, self.height as usize);
        x * h + (h - 1 - y)
    }
}

/// Centre of tile `(x, y)` in room pixels (the game's `MiddleOfTile`).
#[must_use]
pub fn tile_centre(x: i32, y: i32) -> [f32; 2] {
    [
        TILE_SIZE / 2.0 + x as f32 * TILE_SIZE,
        TILE_SIZE / 2.0 + y as f32 * TILE_SIZE,
    ]
}

/// Tile containing a room-pixel position, with the game's formula `(int)((p + 20) / 20) - 1`.
///
/// The cast truncates toward zero, so this equals `floor(p / 20)` only for `p > -20`;
/// e.g. `p = -30` gives `-1`, not `-2`.
#[must_use]
pub fn tile_position(pos: [f32; 2]) -> (i32, i32) {
    let [x, y] = pos.map(|p| ((p + TILE_SIZE) / TILE_SIZE) as i32 - 1);
    (x, y)
}

fn bad(line: usize, field: &'static str, value: &str) -> RoomError {
    RoomError::BadField {
        line,
        field,
        value: value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a 12-line room file around `header` (line 1) and `grid` (line 11).
    fn file(header: &str, cams: &str, grid: &str) -> String {
        format!("Test\n{header}\n1*1|0|0\n{cams}\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{grid}\n")
    }

    /// Namespace for test helpers that wrap the parser under test.
    struct Case;

    impl Case {
        fn shortcut(cell: &str) -> Shortcut {
            Tile::parse(0, cell).expect("cell parses").shortcut
        }

        fn parse(header: &str, cams: &str, grid: &str) -> Result<Room, RoomError> {
            Room::parse(&file(header, cams, grid))
        }

        /// Slope orientation of the centre tile of a 3x3 `grid`.
        fn slope(rows: &[&str]) -> Option<Slope> {
            grid(rows).identify_slope(1, 1)
        }
    }

    // 2 wide, 3 high. File order: column x=0 rows y=2,1,0 then column x=1.
    // x=0: y2 Air+vpole, y1 Air, y0 Solid. x=1: y2 Entrance+path, y1 Slope, y0 Floor+hpole.
    const GRID: &str = "0,1|0|1|4,3|2|3,2|";

    fn sample() -> Room {
        Room::parse(&file("2*3|-1|0", "-220,-30", GRID)).expect("sample parses")
    }

    /// Line 5 of the sample room replaced by `items`.
    fn items(items: &str) -> Result<Vec<RoomItem>, RoomError> {
        let text = file("2*3|-1|0", "-220,-30", GRID).replacen(
            "Border: Solid\n\n",
            &format!("Border: Solid\n{items}\n"),
            1,
        );
        Room::parse(&text).map(|r| r.items().to_vec())
    }

    #[test]
    fn items_read_every_part_before_the_last() {
        const ROCK: RoomItem = RoomItem {
            kind: 0,
            x: 16,
            y: 13,
        };
        const SPEAR: RoomItem = RoomItem {
            kind: 1,
            x: 2,
            y: 3,
        };
        assert_eq!(items("").expect("empty"), []);
        assert_eq!(items("0,16,13|").expect("one"), [ROCK]);
        assert_eq!(items("1,2,3|0,16,13|7,1,1").expect("two"), [SPEAR, ROCK]);
        assert!(items("0,x,1|").is_err());
        assert_eq!(ROCK.tile(30), (15, 17));
    }

    #[test]
    fn parses_header_and_cameras() {
        let room = sample();
        assert_eq!(room.name(), "Test");
        assert_eq!((room.width(), room.height()), (2, 3));
        assert_eq!(room.water_level(), None);
        assert!(!room.water_in_front());
        // y = -(800 - 3*20 + -30) = -710
        assert_eq!(room.cameras(), &[[-220.0, -710.0]]);
    }

    #[test]
    fn grid_is_column_major_top_down_with_y_up_access() {
        let room = sample();
        assert!(room.tile(0, 2).vertical_pole());
        assert_eq!(room.tile(0, 2).terrain, Terrain::Air);
        assert_eq!(room.tile(0, 0).terrain, Terrain::Solid);
        assert_eq!(room.tile(1, 2).terrain, Terrain::ShortcutEntrance);
        assert_eq!(room.tile(1, 2).shortcut, Shortcut::Path);
        assert_eq!(room.tile(1, 1).terrain, Terrain::Slope);
        assert_eq!(room.tile(1, 0).terrain, Terrain::Floor);
        assert!(room.tile(1, 0).horizontal_pole());
        assert_eq!(room.tiles()[0], room.tile(0, 2));
    }

    #[test]
    fn out_of_bounds_clamps_to_edge_and_strips_shortcuts() {
        let room = sample();
        assert_eq!(room.get(-1, 0), None);
        assert_eq!(room.tile(-5, -5), room.tile(0, 0));
        assert_eq!(room.tile(0, 99), room.tile(0, 2));
        let clamped = room.tile(9, 9);
        assert_eq!(clamped.terrain, Terrain::Solid);
        assert_eq!(clamped.shortcut, Shortcut::None);
    }

    #[test]
    fn shortcut_precedence_matches_game() {
        assert_eq!(Case::shortcut("1,4,3"), Shortcut::RoomExit);
        assert_eq!(Case::shortcut("1,3,4"), Shortcut::RoomExit);
        assert_eq!(Case::shortcut("1,3,5"), Shortcut::CreatureDen);
        assert_eq!(Case::shortcut("0,4,5"), Shortcut::CreatureDen);
        assert_eq!(Case::shortcut("0,3,12"), Shortcut::RegionTransportation);
        assert_eq!(Case::shortcut("0,9"), Shortcut::NpcTransportation);
        assert_eq!(Case::shortcut("0,99,x"), Shortcut::None);
    }

    #[test]
    fn water_and_optional_header_fields() {
        let room = Room::parse(&file("2*3|1|1", "0,0", GRID)).expect("parses");
        assert_eq!(room.water_level(), Some(1));
        assert!(room.water_in_front());
        let room = Room::parse(&file("2*3", "0,0", GRID)).expect("parses");
        assert_eq!(room.water_level(), None);
        let room = Room::parse(&file("2*3|-1|1", "0,0", GRID)).expect("parses");
        assert!(!room.water_in_front());
    }

    #[test]
    fn multiple_cameras_and_crlf() {
        let text = file("2*3|-1|0", "-160,-30|864,-40", GRID).replace('\n', "\r\n");
        let room = Room::parse(&text).expect("parses");
        assert_eq!(room.cameras().len(), 2);
        assert_eq!(room.cameras()[1], [864.0, -700.0]);
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(matches!(
            Case::parse("2*3|-1|0", "0,0", "0|0|0|0|0|"),
            Err(RoomError::CellCount {
                expected: 6,
                found: 5
            })
        ));
        assert!(matches!(
            Case::parse("2*3|-1|0", "0,0", "0|0|0|0|0|0|0|"),
            Err(RoomError::CellCount {
                expected: 6,
                found: 7
            })
        ));
        assert!(matches!(
            Case::parse("2*3|-1|0", "0,0", "0|0|0|0|0|0"),
            Err(RoomError::MissingTrailingSeparator)
        ));
        assert!(matches!(
            Case::parse("2*3|-1|0", "0,0", "0|0|0|0|0|7|"),
            Err(RoomError::UnknownTerrain { index: 5, value: 7 })
        ));
        assert!(matches!(
            Case::parse("2*3|-1|0", "0,0", "0|0||0|0|0|"),
            Err(RoomError::BadCell { index: 2, .. })
        ));
        assert!(matches!(
            Case::parse("0*3|-1|0", "0,0", "|"),
            Err(RoomError::BadSize { .. })
        ));
        assert!(matches!(
            Case::parse("2x3|-1|0", "0,0", GRID),
            Err(RoomError::BadField { line: 1, .. })
        ));
        assert!(matches!(
            Case::parse("2*3|-1|0", "", GRID),
            Err(RoomError::BadField { line: 3, .. })
        ));
        assert!(matches!(
            Case::parse("70000*70000|-1|0", "0,0", GRID),
            Err(RoomError::CellCount { .. } | RoomError::BadSize { .. })
        ));
        assert!(matches!(
            Room::parse("Test\n2*3"),
            Err(RoomError::MissingLine { line: 3 })
        ));
        assert!(matches!(
            Room::parse(""),
            Err(RoomError::MissingLine { line: 0 })
        ));
    }

    #[test]
    fn load_reports_io_error() {
        let err = Room::load("/nonexistent/room.txt").expect_err("missing file");
        assert!(matches!(err, RoomError::Io { .. }));
        assert!(err.source().is_some());
    }

    /// Builds a room from ASCII rows (top first): `.` Air, `#` Solid, `/` Slope.
    fn grid(rows: &[&str]) -> Room {
        let (w, h) = (rows[0].len(), rows.len());
        let mut cells = String::new();
        for x in 0..w {
            for row in rows {
                cells.push(match row.as_bytes()[x] {
                    b'#' => '1',
                    b'/' => '2',
                    _ => '0',
                });
                cells.push('|');
            }
        }
        Room::parse(&file(&format!("{w}*{h}|-1|0"), "0,0", &cells)).expect("grid parses")
    }

    #[test]
    fn identify_slope_from_solid_neighbours() {
        // Centre tile (1,1) in a 3x3 grid; neighbours as drawn.
        assert_eq!(Case::slope(&["...", "#/.", ".#."]), Some(Slope::UpRight));
        assert_eq!(Case::slope(&[".#.", "#/.", "..."]), Some(Slope::DownRight));
        assert_eq!(Case::slope(&["...", "./#", ".#."]), Some(Slope::UpLeft));
        assert_eq!(Case::slope(&[".#.", "./#", "..."]), Some(Slope::DownLeft));
        // Below wins over above.
        assert_eq!(Case::slope(&[".#.", "#/.", ".#."]), Some(Slope::UpRight));
        // Left is tested before right.
        assert_eq!(Case::slope(&["...", "#/#", ".#."]), Some(Slope::UpRight));
        assert_eq!(Case::slope(&[".#.", "#/#", "..."]), Some(Slope::DownRight));
    }

    #[test]
    fn identify_slope_broken_cases() {
        // Not a slope tile.
        assert_eq!(Case::slope(&["...", "#.#", ".#."]), None);
        assert_eq!(Case::slope(&["...", "###", ".#."]), None);
        // Side wall but nothing above or below.
        assert_eq!(Case::slope(&["...", "#/#", "..."]), None);
        // Only diagonal Solids.
        assert_eq!(Case::slope(&["#.#", "./.", "#.#"]), None);
        // A Slope neighbour does not count as Solid.
        assert_eq!(Case::slope(&["...", "//.", ".#."]), None);
        // Floor/below without a side wall.
        assert_eq!(Case::slope(&[".#.", "./.", ".#."]), None);
    }

    #[test]
    fn identify_slope_clamps_at_room_edges() {
        // Out-of-bounds neighbours read the clamped edge tile, i.e. the slope itself.
        // Left edge: the left neighbour is the slope (not Solid), so the right one counts.
        assert_eq!(
            grid(&["...", "/#.", "#.."]).identify_slope(0, 1),
            Some(Slope::UpLeft)
        );
        // Right edge with left wall: UpRight.
        assert_eq!(
            grid(&["...", ".#/", "..#"]).identify_slope(2, 1),
            Some(Slope::UpRight)
        );
        // Right edge, no left wall: the right neighbour is the slope itself => Broken.
        assert_eq!(grid(&["...", "../", "..#"]).identify_slope(2, 1), None);
        // Bottom row: below is the slope itself, so a wall plus Solid above => DownRight.
        assert_eq!(
            grid(&[".#.", "#/."]).identify_slope(1, 0),
            Some(Slope::DownRight)
        );
        // Top row: above is the slope itself; nothing below => Broken.
        assert_eq!(grid(&["#/.", "..."]).identify_slope(1, 1), None);
        // Fully outside: the tile and both side neighbours clamp to the edge slope,
        // so no side is Solid => Broken, unlike the in-room (0,1).
        let r = grid(&["...", "/#.", "#.."]);
        assert_eq!(r.identify_slope(-3, 1), None);
    }

    #[test]
    fn tile_geometry_helpers() {
        assert_eq!(tile_centre(0, 0), [10.0, 10.0]);
        assert_eq!(tile_centre(3, 2), [70.0, 50.0]);
        assert_eq!(tile_position([0.0, 19.9]), (0, 0));
        assert_eq!(tile_position([20.0, 45.0]), (1, 2));
        assert_eq!(tile_position([-0.1, -19.9]), (-1, -1));
        // Truncation, not floor, below -20.
        assert_eq!(tile_position([-30.0, -45.0]), (-1, -2));
        let room = sample();
        assert_eq!(room.tile_at([25.0, 25.0]), room.tile(1, 1));
    }
}
