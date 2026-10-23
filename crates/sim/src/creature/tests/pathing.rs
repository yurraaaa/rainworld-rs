
use rw_assets::{ActiveMods, NodeType};

use super::*;

fn template(kind: CreatureType) -> CreatureTemplate {
    CreatureTemplate::of(kind, ActiveMods::NONE)
}

/// The green lizard: its own Floor and Corridor, the lizard template's OffScreen
/// and CurvedFloor, Climb left at the default illegal cost (40) since neither list
/// allows it.
#[test]
fn green_lizard_tables() {
    let g = template(CreatureType::GreenLizard);
    let p = g.pathing();
    assert_eq!(
        g.accessibility_resistance(Accessibility::Floor),
        PathCost::new(1.0, A)
    );
    assert_eq!(
        g.accessibility_resistance(Accessibility::Corridor),
        PathCost::new(2.0, A)
    );
    assert_eq!(
        g.accessibility_resistance(Accessibility::OffScreen),
        PathCost::new(1.0, A)
    );
    assert_eq!(
        g.accessibility_resistance(Accessibility::CurvedFloor),
        PathCost::new(1.0, A)
    );
    assert_eq!(
        g.accessibility_resistance(Accessibility::Climb),
        PathCost::new(40.0, Legality::IllegalTile)
    );
    assert_eq!(
        g.accessibility_resistance(Accessibility::Solid),
        PathCost::new(100.0, Legality::SolidTile)
    );
    assert_eq!(
        g.connection_resistance(MovementType::DropToFloor),
        PathCost::new(40.0, A)
    );
    assert_eq!(
        g.connection_resistance(MovementType::LizardTurn),
        PathCost::new(60.0, A)
    );
    assert_eq!(
        g.connection_resistance(MovementType::ShortCut),
        PathCost::new(1.5, A)
    );
    assert!(!g.connection_resistance(MovementType::DropToClimb).allowed());
    assert!(
        !g.connection_resistance(MovementType::CeilingSlope)
            .allowed()
    );
    assert_eq!(p.water_pathing_resistance, 1.5);
    assert!(p.can_swim);
    assert_eq!(
        p.double_reach_up,
        Some([
            Accessibility::Floor,
            Accessibility::Wall,
            Accessibility::Floor
        ])
    );
}

/// The Fly allows only Air in its list; the cap makes every less open class cost
/// what Air does, so all but Solid and Sand are allowed.
#[test]
fn the_fly_allows_every_class_below_air() {
    let f = template(CreatureType::Fly);
    for acc in &Accessibility::ALL[..8] {
        assert_eq!(
            f.accessibility_resistance(*acc),
            PathCost::new(1.0, A),
            "{acc:?}"
        );
    }
    assert!(!f.accessibility_resistance(Accessibility::Solid).allowed());
    assert!(!f.accessibility_resistance(Accessibility::Sand).allowed());
}

/// The constructor's node types follow the connection resistances (`SetNodeType`),
/// which the abstract layer already reads from the template's own flags.
#[test]
fn mapped_node_types_follow_the_connections() {
    for kind in &CreatureType::registered(ActiveMods::NONE) {
        let t = template(*kind);
        let p = t.pathing();
        if *kind == CreatureType::GarbageWorm {
            continue;
        }
        let shortcut = p.connection_resistance(MovementType::ShortCut).allowed();
        assert_eq!(t.maps_node(NodeType::Exit), shortcut, "{kind:?}");
        assert_eq!(
            t.maps_node(NodeType::SkyExit),
            p.connection_resistance(MovementType::SkyHighway).allowed(),
            "{kind:?}"
        );
        assert_eq!(
            t.maps_node(NodeType::SeaExit),
            p.connection_resistance(MovementType::SeaHighway).allowed(),
            "{kind:?}"
        );
        assert_eq!(
            t.maps_node(NodeType::SideExit),
            p.connection_resistance(MovementType::SideHighway).allowed(),
            "{kind:?}"
        );
    }
}

#[test]
fn the_pre_baked_list_matches_the_pathing_indices() {
    for (i, kind) in CreatureTemplate::PRE_BAKED_PATHING_CREATURES
        .iter()
        .enumerate()
    {
        assert_eq!(
            template(*kind).pre_baked_pathing_index(),
            i32::try_from(i).expect("index")
        );
    }
}
