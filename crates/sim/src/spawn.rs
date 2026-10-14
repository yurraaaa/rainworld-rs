//! Spawning: choosing a safe tile (`Room.DetermineSafeSpawnTile`) and placing an
//! object on it (`Creature.PlaceInRoom`).
//!
//! Randomness comes from a caller-supplied [`rand::Rng`], so a seeded generator gives
//! deterministic spawns. Unity's random sequence is not reproduced
//! (`docs/DIVERGENCES.md` D1).

use rand::{Rng, RngExt};
use rw_assets::{Room, Shortcut, Terrain, tile_centre};

use crate::room_ext::RoomExt;
use crate::{PhysicalObject, Vec2};

/// The game's `Custom.fourDirections` order.
const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// Random tries around exit and den shortcuts before the fallbacks.
const SPAWN_TRIES: usize = 1000;

/// Picks a spawn tile for a non-shelter room, as `Room.DetermineSafeSpawnTile(room, false)`.
///
/// 1. Up to 1000 random tiles near the entrance of a shortcut leading to an exit or den
///    node; accepted if inside the room, `y > 0`, not Solid, the three tiles below
///    (`x-1..=x+1`) all Solid, and not at or below the water level.
/// 2. A random tile, `y` from 1 (or water level + 1), not Solid with three Solid below.
/// 3. A random non-Solid tile with `y >= 1` and Solid directly below.
/// 4. A random non-Solid tile with `y >= 1`.
/// 5. `(0, 0)`.
///
/// The shelter branch is not ported: room files do not say whether a room is a shelter.
pub fn determine_safe_spawn_tile<R: Rng + ?Sized>(room: &Room, rng: &mut R) -> (i32, i32) {
    let water_level = room.water_level();

    let shortcuts = ShortcutMap::new(room);
    let exit_and_den_count = shortcuts.exit_and_den_count;
    for _ in 0..SPAWN_TRIES {
        let node = (rng.random::<f32>() * exit_and_den_count as f32) as usize;
        // The game falls back to its first shortcut, and throws when there is none.
        let Some(start) = shortcuts.entrance_leading_to(node) else {
            break;
        };
        let (dx, dy) = hole_direction(room, start);
        let along = (rng.random::<f32>() * 10.0) as i32;
        let across = (rng.random::<f32>() * 6.0 - 3.0) as i32;
        // Perpendicular of `dir * across`: (y, -x).
        let x = start.0 + dx * along + dy * across;
        let y = start.1 + dy * along - dx * across;
        if !room.contains(x, y) || y <= 0 || room.is_solid_tile(x, y) {
            continue;
        }
        let under_water = water_level.is_some_and(|w| y <= w);
        if room.has_ground_below(x, y) && !under_water {
            return (x, y);
        }
    }

    let first_dry_row = water_level.map_or(1, |w| w + 1);
    for fallback in SpawnFallback::ALL {
        let candidates: Vec<(i32, i32)> = (0..room.width())
            .flat_map(|x| (1..room.height()).map(move |y| (x, y)))
            .filter(|&(x, y)| fallback.accepts(room, first_dry_row, x, y))
            .collect();
        if !candidates.is_empty() {
            return candidates[rng.random_range(0..candidates.len())];
        }
    }
    (0, 0)
}

/// The three progressively looser random-tile searches after the shortcut tries.
#[derive(Clone, Copy)]
enum SpawnFallback {
    /// At or above the first dry row, not Solid, with three Solid tiles below.
    DryWithGroundBelow,
    /// Not Solid with Solid directly below.
    OnSolid,
    /// Not Solid.
    Open,
}

impl SpawnFallback {
    const ALL: [Self; 3] = [Self::DryWithGroundBelow, Self::OnSolid, Self::Open];

    fn accepts(self, room: &Room, first_dry_row: i32, x: i32, y: i32) -> bool {
        match self {
            Self::DryWithGroundBelow => {
                y >= first_dry_row && !room.is_solid_tile(x, y) && room.has_ground_below(x, y)
            }
            Self::OnSolid => !room.is_solid_tile(x, y) && room.is_solid_tile(x, y - 1),
            Self::Open => !room.is_solid_tile(x, y),
        }
    }
}

/// Puts every chunk at the middle of `tile` plus an independent random unit vector,
/// with no velocity and no position history, as `Creature.PlaceInRoom`.
pub fn place_in_room<R: Rng + ?Sized>(obj: &mut PhysicalObject, tile: (i32, i32), rng: &mut R) {
    let centre = Vec2::from(tile_centre(tile.0, tile.1));
    for chunk in &mut obj.chunks {
        let deg = rng.random::<f32>() * 360.0;
        chunk.reset_at(centre + deg_to_vec(deg));
    }
}

/// Unit vector for an angle in degrees, clockwise from +y (`Custom.DegToVec`).
fn deg_to_vec(deg: f32) -> Vec2 {
    let rad = deg * (std::f32::consts::PI / 180.0);
    Vec2::new(rad.sin(), rad.cos())
}

/// First of the four directions whose neighbour is not Solid
/// (`Room.ShorcutEntranceHoleDirection`); `(0, 0)` when enclosed.
fn hole_direction(room: &Room, (x, y): (i32, i32)) -> (i32, i32) {
    FOUR_DIRECTIONS
        .into_iter()
        .find(|&(dx, dy)| room.tile(x + dx, y + dy).terrain != Terrain::Solid)
        .unwrap_or((0, 0))
}

/// Shortcut entrances and the node each leads to, derived from the tile flags.
///
/// The game builds this in `ShortcutMapper`, which has not been decompiled; the node
/// order (room exits, then dens, then region transports, each scanned top row first,
/// left to right) is from `room-format.md` section 5, and the shortcut order and
/// path tracing here are inferred.
struct ShortcutMap {
    /// Number of leading Exit/Den nodes.
    exit_and_den_count: usize,
    /// `(entrance tile, destination node)` in entrance scan order.
    shortcuts: Vec<((i32, i32), Option<usize>)>,
}

impl ShortcutMap {
    fn new(room: &Room) -> Self {
        let nodes: Vec<(i32, i32)> = room
            .tiles_with_shortcut(Shortcut::RoomExit)
            .chain(room.tiles_with_shortcut(Shortcut::CreatureDen))
            .chain(room.tiles_with_shortcut(Shortcut::RegionTransportation))
            .collect();
        let exit_and_den_count = room.tiles_with_shortcut(Shortcut::RoomExit).count()
            + room.tiles_with_shortcut(Shortcut::CreatureDen).count();
        let shortcuts = room
            .scan_top_down()
            .filter(|&(x, y)| room.tile(x, y).terrain == Terrain::ShortcutEntrance)
            .map(|entrance| {
                let dest = trace_shortcut(room, entrance)
                    .and_then(|end| nodes.iter().position(|&n| n == end));
                (entrance, dest)
            })
            .collect();
        Self {
            exit_and_den_count,
            shortcuts,
        }
    }

    /// Entrance of the shortcut leading to `node`, else of the first shortcut
    /// (`Room.ShortcutLeadingToNode`); `None` if the room has no shortcuts.
    fn entrance_leading_to(&self, node: usize) -> Option<(i32, i32)> {
        self.shortcuts
            .iter()
            .find(|(_, dest)| *dest == Some(node))
            .or_else(|| self.shortcuts.first())
            .map(|&(entrance, _)| entrance)
    }
}

/// Follows shortcut-flagged tiles from an entrance to the tile that ends the tube:
/// a node tile (exit, den, transport) or `None` for another entrance or a dead end.
pub(crate) fn trace_shortcut(room: &Room, entrance: (i32, i32)) -> Option<(i32, i32)> {
    let (mut prev, mut cur) = (entrance, entrance);
    for _ in 0..room.tiles().len() {
        let next = FOUR_DIRECTIONS
            .into_iter()
            .map(|(dx, dy)| (cur.0 + dx, cur.1 + dy))
            .find(|&p| p != prev && room.is_in_shortcut_tube(p.0, p.1))?;
        let tile = room.tile(next.0, next.1);
        match tile.shortcut {
            Shortcut::RoomExit
            | Shortcut::CreatureDen
            | Shortcut::NpcTransportation
            | Shortcut::RegionTransportation => return Some(next),
            Shortcut::None | Shortcut::Path => {}
        }
        if tile.terrain == Terrain::ShortcutEntrance {
            return None;
        }
        (prev, cur) = (cur, next);
    }
    None
}
