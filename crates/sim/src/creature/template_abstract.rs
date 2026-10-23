//! The template fields the abstract creature AI reads: the off-screen connection
//! resistances (from [`super::pathing`]), `stowFoodInDen` (`InitStaticWorld`, 125287),
//! and which templates' abstract AI is ported.
//!
//! Mirrors: StaticWorld.cs (InitStaticWorld), CreatureTemplate.cs

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
    /// (not ported: they stay in their dens, docs/divergences/ D14).
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

    /// `socialMemory` (`InitStaticWorld`, copied by children): the lizards, vultures,
    /// cicadas, jet fish, scavengers and the big needle worm (the small one clears it);
    /// of the DLC templates those the game sets (124013.., 123064..).
    #[must_use]
    pub fn social_memory(&self) -> bool {
        use CreatureType as T;
        match self.kind {
            T::SmallNeedleWorm | T::StowawayBug => false,
            T::MirosVulture
            | T::ScavengerElite
            | T::ScavengerKing
            | T::SlugNpc
            | T::BigMoth
            | T::FireSprite
            | T::ScavengerTemplar
            | T::ScavengerDisciple
            | T::Rat
            | T::Frog
            | T::RippleSpider => true,
            _ => matches!(
                self.top_ancestor(),
                T::LizardTemplate
                    | T::Vulture
                    | T::CicadaA
                    | T::JetFish
                    | T::Scavenger
                    | T::BigNeedleWorm
            ),
        }
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

    /// `AbstractRoom.CommonToCreatureSpecificNodeIndex`: -1 for a node the template
    /// does not map.
    #[must_use]
    pub fn specific_node_index(&self, room: &AbstractRoomData, common: i32) -> i32 {
        let Some(node) = usize::try_from(common).ok().and_then(|n| room.nodes.get(n)) else {
            return -1;
        };
        if !self.maps_node(node.kind) {
            return -1;
        }
        let before = room.nodes[..usize::try_from(common).unwrap_or(0)]
            .iter()
            .filter(|n| self.maps_node(n.kind))
            .count();
        i32::try_from(before).unwrap_or(-1)
    }
}

#[cfg(test)]
#[path = "tests/template_abstract.rs"]
mod tests;
