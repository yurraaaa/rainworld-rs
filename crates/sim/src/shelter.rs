//! `ShelterDoor`, of which only where it puts a player starting in the shelter is ported
//! (`playerSpawnPos`, which `AbstractCreature.RealizeInRoom` uses in a shelter). The
//! door itself (closing, opening at cycle start, hibernation) is not ported yet.

use rw_assets::shortcut::{FOUR_DIRECTIONS, ShortcutType};
use rw_assets::{ActiveMods, PlacedObjectType, Room, RoomSettings, Terrain};

use crate::room_ext::RoomExt;

/// `ShelterDoor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShelterDoor;

impl ShelterDoor {
    /// The constructor's `playerSpawnPos`: from the last column's room exit, out along
    /// its first open side to the wall, (when that side is up) left to the wall, then
    /// down to the floor; (20, 13) in an ancient shelter; a `ShelterSpawnPoint` object
    /// overrides either.
    #[must_use]
    pub fn player_spawn_pos(
        room: &Room,
        settings: &RoomSettings,
        ancient: bool,
        mods: ActiveMods,
    ) -> (i32, i32) {
        let mut spawn = (0, 0);
        if ancient {
            spawn = (20, 13);
        } else {
            for l in 0..room.width() {
                for m in 0..room.height() {
                    if room.tile(l, m).terrain != Terrain::ShortcutEntrance
                        || room.shortcut_map().type_at((l, m)) != ShortcutType::RoomExit
                    {
                        continue;
                    }
                    if let Some(&(dx, dy)) = FOUR_DIRECTIONS
                        .iter()
                        .find(|(dx, dy)| !room.is_solid_tile(l + dx, m + dy))
                    {
                        let mut p = (l, m);
                        while !room.is_solid_tile(p.0 + dx, p.1 + dy) {
                            p = (p.0 + dx, p.1 + dy);
                        }
                        if dy == 1 {
                            while !room.is_solid_tile(p.0 - 1, p.1) {
                                p.0 -= 1;
                            }
                        }
                        while !room.is_solid_tile(p.0, p.1 - 1) {
                            p.1 -= 1;
                        }
                        spawn = p;
                    }
                    break;
                }
            }
        }
        if let Some(o) = settings
            .placed_objects
            .iter()
            .find(|o| o.object_type(mods) == Some(PlacedObjectType::ShelterSpawnPoint))
        {
            spawn = rw_assets::tile_position(o.pos);
        }
        spawn
    }
}
