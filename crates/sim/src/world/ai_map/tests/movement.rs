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
