//! Spawning: choosing a safe tile (`Room.DetermineSafeSpawnTile`) and placing an
//! object on it (`Creature.PlaceInRoom`).
//!
//! Both draw from the game's `UnityEngine.Random` ([`UnityRandom`]) with the game's calls.

use rw_assets::{Room, Shortcut, tile_centre};
use rw_random::UnityRandom;

use crate::room_ext::RoomExt;
use crate::{PhysicalObject, Vec2};

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
pub fn determine_safe_spawn_tile(room: &Room, rng: &mut UnityRandom) -> (i32, i32) {
    let water_level = room.water_level();

    let map = room.shortcut_map();
    // The leading Exit and Den nodes of the abstract room: one per room-exit and den tile.
    let exit_and_den_count = map
        .nodes()
        .iter()
        .filter(|&&(x, y)| {
            matches!(
                room.tile(x, y).shortcut,
                Shortcut::RoomExit | Shortcut::CreatureDen
            )
        })
        .count();
    for _ in 0..SPAWN_TRIES {
        let node = (rng.value() * exit_and_den_count as f32) as usize;
        // The game throws here when the room has no shortcuts (docs/DIVERGENCES.md D2).
        let Some(shortcut) = map.leading_to_node(node) else {
            break;
        };
        let start = shortcut.start;
        let (dx, dy) = room.shortcut_hole_direction(start.0, start.1);
        let along = (rng.value() * 10.0) as i32;
        let across = (rng.value() * 6.0 - 3.0) as i32;
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
            let len = i32::try_from(candidates.len()).expect("tile count fits i32");
            return candidates[rng.range_i32(0, len) as usize];
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
pub fn place_in_room(obj: &mut PhysicalObject, tile: (i32, i32), rng: &mut UnityRandom) {
    let centre = Vec2::from(tile_centre(tile.0, tile.1));
    for chunk in &mut obj.chunks {
        chunk.reset_at(centre + Vec2::random_unit(rng));
    }
}
