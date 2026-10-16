//! The template fields the abstract creature AI reads: the off-screen connection
//! resistances, `stowFoodInDen`, and which templates' abstract AI is ported.

use rw_assets::CreatureType;
use rw_assets::world::AbstractRoomData;

use super::CreatureTemplate;
use crate::world::MovementType;

/// The `MovementConnection.MovementType`s of the abstract path costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractMovement {
    OffScreenMovement,
    ShortCut,
    BetweenRooms,
    SkyHighway,
    SeaHighway,
    SideHighway,
    RegionTransportation,
}

/// `PathCost` of a connection type: its resistance and whether it is `Allowed`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbstractCost {
    pub resistance: f32,
    pub allowed: bool,
}

impl AbstractMovement {
    #[must_use]
    pub fn movement_type(self) -> MovementType {
        match self {
            Self::OffScreenMovement => MovementType::OffScreenMovement,
            Self::ShortCut => MovementType::ShortCut,
            Self::BetweenRooms => MovementType::BetweenRooms,
            Self::SkyHighway => MovementType::SkyHighway,
            Self::SeaHighway => MovementType::SeaHighway,
            Self::SideHighway => MovementType::SideHighway,
            Self::RegionTransportation => MovementType::RegionTransportation,
        }
    }
}

impl CreatureTemplate {
    /// Whether the plain `AbstractCreatureAI` behaviour runs for this template: the
    /// lizards so far (other types stay in their dens, docs/DIVERGENCES.md D14).
    #[must_use]
    pub fn abstract_ai_ported(&self) -> bool {
        self.top_ancestor() == CreatureType::LizardTemplate
    }

    /// `ConnectionResistance(type)` for the abstract movement types.
    #[must_use]
    pub fn abstract_resistance(&self, movement: AbstractMovement) -> AbstractCost {
        let cost = self.connection_resistance(movement.movement_type());
        AbstractCost {
            resistance: cost.resistance,
            allowed: cost.allowed(),
        }
    }

    /// `stowFoodInDen` (set by `LizardTemplate`; the other templates' values are not
    /// ported).
    #[must_use]
    pub fn stow_food_in_den(&self) -> bool {
        self.top_ancestor() == CreatureType::LizardTemplate
    }

    /// `mappedNodeTypes[node.type]` of node `node` of `room` (false out of range).
    #[must_use]
    pub fn maps_room_node(&self, room: &AbstractRoomData, node: i32) -> bool {
        usize::try_from(node)
            .ok()
            .and_then(|n| room.nodes.get(n))
            .is_some_and(|n| self.maps_node(n.kind))
    }

    /// `AbstractRoom.NodesRelevantToCreature`.
    #[must_use]
    pub fn nodes_relevant(&self, room: &AbstractRoomData) -> i32 {
        let n = room.nodes.iter().filter(|n| self.maps_node(n.kind)).count();
        i32::try_from(n).unwrap_or(i32::MAX)
    }

    /// `AbstractRoom.CreatureSpecificToCommonNodeIndex`.
    #[must_use]
    pub fn common_node_index(&self, room: &AbstractRoomData, specific: i32) -> i32 {
        room.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| self.maps_node(n.kind))
            .nth(usize::try_from(specific).unwrap_or(usize::MAX))
            .map_or(-1, |(i, _)| i32::try_from(i).unwrap_or(-1))
    }
}
