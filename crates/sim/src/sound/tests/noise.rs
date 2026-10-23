
use super::*;
use crate::Contact;
use crate::creature::CreatureId;

fn impact(speed: f32, first_contact: bool) -> TerrainImpact {
    TerrainImpact {
        chunk: 1,
        direction: Contact { x: 0, y: -1 },
        speed,
        first_contact,
    }
}

/// `PhysicalObject.cs` 629-631: a chunk of mass 0.3 and loudness 0.7 landing at 10:
/// `10 * 0.7 * Lerp(0.3, 1, 0.5)` = 4.55, so
/// `Lerp(350, Lerp(100, 1500, InverseLerp(0.5, 20, 4.55)), 0.5)` at 0.9 of the radius
/// below the chunk.
#[test]
fn terrain_impact_noise() {
    let mut c = BodyChunk::new(Vec2::new(100.0, 50.0), 9.0, 0.3);
    c.loudness = 0.7;
    let n = impact(10.0, true)
        .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
        .expect("loud enough");
    let x = 4.55f32;
    let t = (x - 0.5) / 19.5;
    let expected = (350.0 + (100.0 + 1400.0 * t)) / 2.0;
    assert!((n.strength - expected).abs() < 1e-3, "{}", n.strength);
    assert_eq!(n.pos, Vec2::new(100.0, 50.0 - 8.1));
    assert_eq!(n.source, Some(Entity::Creature(CreatureId(3)).into()));
    assert_eq!(n.interesting, 1.0);
    assert!(
        impact(10.0, false)
            .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
            .is_none()
    );
    assert!(
        impact(4.0, true)
            .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
            .is_none()
    );
    c.loudness = 0.0;
    assert!(
        impact(10.0, true)
            .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
            .is_none()
    );
}

#[test]
fn default_noise() {
    let n = InGameNoise {
        pos: Vec2::ZERO,
        strength: 0.0,
        source: None,
        interesting: 0.0,
    };
    assert!(n.is_default());
    assert!(!InGameNoise { strength: 1.0, ..n }.is_default());
}
