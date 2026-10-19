//! What the holograms and the communication module read of the world's processes
//! (`OverseersWorldAI`'s finders, `FliesWorldAI`'s swarm rooms).
//!
//! Mirrors: OverseersWorldAI.cs, FliesWorldAI.cs

use crate::creature::WorldCoordinate;
use crate::world::World;

/// What the guide reads of `OverseersWorldAI.DirectionFinder`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DirectionFinderView {
    pub done: bool,
    pub show_to_room: i32,
    pub min_karma: i32,
    /// `DestinationRoomVisisted`.
    pub destination_room_visited: bool,
}

impl World {
    pub(crate) fn guide_shelter_distance(&self, shelter: usize, coord: WorldCoordinate) -> f32 {
        self.distance_to_shelter(shelter, coord)
    }

    pub(crate) fn guide_direction_distance(&self, coord: WorldCoordinate) -> f32 {
        self.distance_to_destination(coord)
    }

    pub(crate) fn guide_direction_finder(&self) -> Option<DirectionFinderView> {
        let f = self.direction_finder()?;
        Some(DirectionFinderView {
            done: f.done,
            show_to_room: f.show_to_room,
            min_karma: f.min_karma,
            destination_room_visited: self.destination_room_visited(),
        })
    }

    pub(crate) fn guide_active_swarm_room(&self, room: i32) -> bool {
        self.active_swarm_room(room)
    }

    /// `world.shelters`.
    pub(crate) fn guide_shelters(&self) -> Vec<i32> {
        self.data().shelters.clone()
    }

    /// `BatPointer.FliesInRoom`: a realized room's abstract creatures of `kind`, else
    /// the quantified count.
    pub(crate) fn flies_in_room(&self, room: i32, kind: rw_assets::CreatureType) -> i32 {
        if self.realized(room).is_none() {
            return self.number_of_quantified_creature_in_room(room, kind);
        }
        let n = self
            .room_creatures(room)
            .iter()
            .filter(|&&c| self.creature(c).template.kind == kind)
            .count();
        i32::try_from(n).unwrap_or(i32::MAX)
    }

    pub(crate) fn guide_swarm_room_distance(&self, coord: WorldCoordinate, swarm: usize) -> f32 {
        self.swarm_room_distance(coord, swarm)
    }
}
