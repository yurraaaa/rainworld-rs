//! The AI map's vocabulary: tile accessibility classes (`AItile.Accessibility`),
//! movement connections (`MovementConnection`) and path costs (`PathCost`).

use std::cmp::Ordering;

use crate::creature::WorldCoordinate;

/// `AItile.Accessibility`, in the game's order (the mapper and the templates compare
/// the classes as integers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Accessibility {
    OffScreen,
    Floor,
    CurvedFloor,
    Corridor,
    Climb,
    Wall,
    Ceiling,
    Air,
    Solid,
    Sand,
}

impl Accessibility {
    pub const ALL: [Self; 10] = [
        Self::OffScreen,
        Self::Floor,
        Self::CurvedFloor,
        Self::Corridor,
        Self::Climb,
        Self::Wall,
        Self::Ceiling,
        Self::Air,
        Self::Solid,
        Self::Sand,
    ];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn wall_or_ceiling(self) -> bool {
        self == Self::Wall || self == Self::Ceiling
    }

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::OffScreen => "OffScreen",
            Self::Floor => "Floor",
            Self::CurvedFloor => "CurvedFloor",
            Self::Corridor => "Corridor",
            Self::Climb => "Climb",
            Self::Wall => "Wall",
            Self::Ceiling => "Ceiling",
            Self::Air => "Air",
            Self::Solid => "Solid",
            Self::Sand => "Sand",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.value() == value)
    }
}

/// `MovementConnection.MovementType`, in the game's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MovementType {
    Standard,
    ReachOverGap,
    ReachUp,
    DoubleReachUp,
    ReachDown,
    SemiDiagonalReach,
    DropToFloor,
    DropToClimb,
    DropToWater,
    LizardTurn,
    OpenDiagonal,
    Slope,
    CeilingSlope,
    ShortCut,
    NpcTransportation,
    BigCreatureShortCutSqueeze,
    OutsideRoom,
    SideHighway,
    SkyHighway,
    SeaHighway,
    RegionTransportation,
    BetweenRooms,
    OffScreenMovement,
    OffScreenUnallowed,
}

impl MovementType {
    pub const ALL: [Self; 24] = [
        Self::Standard,
        Self::ReachOverGap,
        Self::ReachUp,
        Self::DoubleReachUp,
        Self::ReachDown,
        Self::SemiDiagonalReach,
        Self::DropToFloor,
        Self::DropToClimb,
        Self::DropToWater,
        Self::LizardTurn,
        Self::OpenDiagonal,
        Self::Slope,
        Self::CeilingSlope,
        Self::ShortCut,
        Self::NpcTransportation,
        Self::BigCreatureShortCutSqueeze,
        Self::OutsideRoom,
        Self::SideHighway,
        Self::SkyHighway,
        Self::SeaHighway,
        Self::RegionTransportation,
        Self::BetweenRooms,
        Self::OffScreenMovement,
        Self::OffScreenUnallowed,
    ];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::ReachOverGap => "ReachOverGap",
            Self::ReachUp => "ReachUp",
            Self::DoubleReachUp => "DoubleReachUp",
            Self::ReachDown => "ReachDown",
            Self::SemiDiagonalReach => "SemiDiagonalReach",
            Self::DropToFloor => "DropToFloor",
            Self::DropToClimb => "DropToClimb",
            Self::DropToWater => "DropToWater",
            Self::LizardTurn => "LizardTurn",
            Self::OpenDiagonal => "OpenDiagonal",
            Self::Slope => "Slope",
            Self::CeilingSlope => "CeilingSlope",
            Self::ShortCut => "ShortCut",
            Self::NpcTransportation => "NPCTransportation",
            Self::BigCreatureShortCutSqueeze => "BigCreatureShortCutSqueeze",
            Self::OutsideRoom => "OutsideRoom",
            Self::SideHighway => "SideHighway",
            Self::SkyHighway => "SkyHighway",
            Self::SeaHighway => "SeaHighway",
            Self::RegionTransportation => "RegionTransportation",
            Self::BetweenRooms => "BetweenRooms",
            Self::OffScreenMovement => "OffScreenMovement",
            Self::OffScreenUnallowed => "OffScreenUnallowed",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.value() == value)
    }

    /// `MovementConnection.IsDrop`.
    #[must_use]
    pub fn is_drop(self) -> bool {
        matches!(
            self,
            Self::DropToFloor | Self::DropToClimb | Self::DropToWater
        )
    }
}

/// `MovementConnection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementConnection {
    pub kind: MovementType,
    pub start: WorldCoordinate,
    pub dest: WorldCoordinate,
    pub distance: i32,
}

impl MovementConnection {
    #[must_use]
    pub fn new(
        kind: MovementType,
        start: WorldCoordinate,
        dest: WorldCoordinate,
        distance: i32,
    ) -> Self {
        Self {
            kind,
            start,
            dest,
            distance,
        }
    }

    /// `here -> there` when `outgoing`, else `there -> here`.
    #[must_use]
    pub fn directed(
        outgoing: bool,
        kind: MovementType,
        here: WorldCoordinate,
        there: WorldCoordinate,
        distance: i32,
    ) -> Self {
        if outgoing {
            Self::new(kind, here, there, distance)
        } else {
            Self::new(kind, there, here, distance)
        }
    }

    /// `StartTile`.
    #[must_use]
    pub fn start_tile(&self) -> (i32, i32) {
        self.start.tile
    }

    /// `DestTile`.
    #[must_use]
    pub fn dest_tile(&self) -> (i32, i32) {
        self.dest.tile
    }
}

/// `PathCost.Legality`, from best to worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Legality {
    Allowed,
    Unwanted,
    IllegalConnection,
    IllegalTile,
    SolidTile,
    Unallowed,
}

impl Legality {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Allowed => "Allowed",
            Self::Unwanted => "Unwanted",
            Self::IllegalConnection => "IllegalConnection",
            Self::IllegalTile => "IllegalTile",
            Self::SolidTile => "SolidTile",
            Self::Unallowed => "Unallowed",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::Allowed,
            Self::Unwanted,
            Self::IllegalConnection,
            Self::IllegalTile,
            Self::SolidTile,
            Self::Unallowed,
        ]
        .into_iter()
        .find(|l| l.value() == value)
    }
}

/// `PathCost`: ordered by legality, then resistance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathCost {
    pub resistance: f32,
    pub legality: Legality,
}

impl PathCost {
    #[must_use]
    pub const fn new(resistance: f32, legality: Legality) -> Self {
        Self {
            resistance,
            legality,
        }
    }

    /// Unwanted is still allowed.
    #[must_use]
    pub fn allowed(self) -> bool {
        self.legality <= Legality::Unwanted
    }

    #[must_use]
    pub fn considerable(self) -> bool {
        self.legality != Legality::Unallowed
    }

    /// `PathCost.CompareTo` (`<` and `>` on legality first; equal resistances, or a NaN
    /// one, compare equal).
    #[must_use]
    pub fn compare(self, other: Self) -> Ordering {
        if self.legality != other.legality {
            return self.legality.cmp(&other.legality);
        }
        if self.resistance < other.resistance {
            Ordering::Less
        } else if self.resistance > other.resistance {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }

    /// `operator >`.
    #[must_use]
    pub fn greater_than(self, other: Self) -> bool {
        self.compare(other) == Ordering::Greater
    }

    /// `operator <`.
    #[must_use]
    pub fn less_than(self, other: Self) -> bool {
        self.compare(other) == Ordering::Less
    }

    /// `operator <=` (false for a NaN resistance at equal legality).
    #[must_use]
    pub fn less_or_equal(self, other: Self) -> bool {
        if self.legality == other.legality {
            return self.resistance <= other.resistance;
        }
        self.legality < other.legality
    }
}

impl std::ops::Add for PathCost {
    type Output = Self;

    /// The resistances add; the worse legality wins.
    fn add(self, other: Self) -> Self {
        Self::new(
            self.resistance + other.resistance,
            self.legality.max(other.legality),
        )
    }
}

impl std::ops::Mul<f32> for PathCost {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self::new(self.resistance * factor, self.legality)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legality_orders_before_resistance() {
        let cheap_illegal = PathCost::new(0.0, Legality::IllegalTile);
        let dear_allowed = PathCost::new(500.0, Legality::Allowed);
        assert!(cheap_illegal.greater_than(dear_allowed));
        assert!(PathCost::new(1.0, Legality::Allowed).less_than(dear_allowed));
        assert!(PathCost::new(3.0, Legality::Unwanted).allowed());
        assert!(!PathCost::new(3.0, Legality::IllegalConnection).allowed());
        let sum = PathCost::new(1.5, Legality::Allowed) + PathCost::new(2.0, Legality::SolidTile);
        assert_eq!(sum, PathCost::new(3.5, Legality::SolidTile));
    }

    #[test]
    fn enum_orders_and_spellings_follow_the_game() {
        assert_eq!(Accessibility::Air.index(), 7);
        assert_eq!(Accessibility::Solid.index(), 8);
        assert!(Accessibility::Wall > Accessibility::Climb);
        assert_eq!(MovementType::ShortCut.index(), 13);
        assert_eq!(MovementType::OffScreenUnallowed.index(), 23);
        assert_eq!(
            MovementType::from_value("NPCTransportation"),
            Some(MovementType::NpcTransportation)
        );
        assert_eq!(
            Accessibility::from_value("CurvedFloor"),
            Some(Accessibility::CurvedFloor)
        );
        assert_eq!(Legality::from_value("SolidTile"), Some(Legality::SolidTile));
    }
}
