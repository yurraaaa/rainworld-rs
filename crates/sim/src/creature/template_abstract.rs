//! The template fields the abstract creature AI reads: the off-screen connection
//! resistances (from [`super::pathing`]), `stowFoodInDen` (`InitStaticWorld`, 125287),
//! and which templates' abstract AI is ported.

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
    /// Whether the plain `AbstractCreatureAI` or the template's own abstract AI runs:
    /// every base type with an AI but the overseer (its own update) and the scavengers
    /// (not ported: they stay in their dens, docs/DIVERGENCES.md D14).
    #[must_use]
    pub fn abstract_ai_ported(&self) -> bool {
        self.ai && self.kind != CreatureType::Overseer && !self.held_in_den()
    }

    /// Kept in its den, its abstract AI not ported: the scavengers.
    #[must_use]
    pub fn held_in_den(&self) -> bool {
        self.top_ancestor() == CreatureType::Scavenger
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

    /// `stowFoodInDen`: set by these root templates and copied by their children (the
    /// centipede sets it and then clears it).
    #[must_use]
    pub fn stow_food_in_den(&self) -> bool {
        use CreatureType as T;
        matches!(
            self.top_ancestor(),
            T::LizardTemplate
                | T::Vulture
                | T::LanternMouse
                | T::CicadaA
                | T::JetFish
                | T::BigEel
                | T::DaddyLongLegs
                | T::TentaclePlant
                | T::MirosBird
                | T::TempleGuard
                | T::EggBug
                | T::BigSpider
                | T::BigNeedleWorm
                | T::DropBug
        )
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

#[cfg(test)]
mod tests {
    use rw_assets::ActiveMods;

    use super::*;

    fn cost(kind: CreatureType, m: AbstractMovement) -> AbstractCost {
        CreatureTemplate::of(kind, ActiveMods::NONE).abstract_resistance(m)
    }

    fn allowed(resistance: f32) -> AbstractCost {
        AbstractCost {
            resistance,
            allowed: true,
        }
    }

    fn stow(kind: CreatureType) -> bool {
        CreatureTemplate::of(kind, ActiveMods::NONE).stow_food_in_den()
    }

    /// The abstract lists of `InitStaticWorld` (125287..) for the roaming types.
    #[test]
    fn abstract_connection_costs_follow_init_static_world() {
        use AbstractMovement as M;
        use CreatureType as T;
        assert_eq!(cost(T::KingVulture, M::SkyHighway), allowed(1.0));
        assert!(!cost(T::Vulture, M::ShortCut).allowed);
        // Cicada B clears its list: Cicada A's.
        assert_eq!(cost(T::CicadaB, M::ShortCut), allowed(1.0));
        assert_eq!(cost(T::CicadaB, M::BetweenRooms), allowed(10.0));
        assert_eq!(cost(T::Scavenger, M::OffScreenMovement), allowed(1.2));
        assert_eq!(cost(T::Scavenger, M::RegionTransportation), allowed(400.0));
        assert!(!cost(T::TempleGuard, M::BetweenRooms).allowed);
        assert_eq!(cost(T::SmallNeedleWorm, M::BetweenRooms), allowed(1.0));
        assert_eq!(cost(T::DropBug, M::BetweenRooms), allowed(5.0));
        assert_eq!(cost(T::Snail, M::ShortCut), allowed(0.2));
        assert_eq!(cost(T::BigEel, M::SeaHighway), allowed(100_000.0));
        assert_eq!(cost(T::Deer, M::SideHighway), allowed(100_000.0));
        assert_eq!(cost(T::MirosBird, M::SideHighway), allowed(100.0));
        assert!(!cost(T::GarbageWorm, M::OffScreenMovement).allowed);
    }

    #[test]
    fn stow_food_in_den_per_template() {
        use CreatureType as T;
        assert!(stow(T::CicadaB) && stow(T::PoleMimic) && stow(T::SmallNeedleWorm));
        assert!(!stow(T::Centipede) && !stow(T::Centiwing) && !stow(T::Deer));
        assert!(!stow(T::Scavenger) && !stow(T::Snail));
    }
}
