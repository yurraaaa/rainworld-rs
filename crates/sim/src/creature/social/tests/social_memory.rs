use super::*;

#[test]
fn cycle_tick_fades_weak_feelings_and_forgets_faint_ones() {
    // `like *= LerpMap(|like|, 0.5, 0, 1, 0.85)`: 0.4 is a fifth of the way to 0, so
    // x0.97 = 0.388; 0.2 is x0.91 = 0.182, and with no fear it is forgotten. 0.8
    // keeps (x1). `know *= LerpMap(know, 0.5, 0, 1, 0.15)`: 0.3 -> x0.66 = 0.198.
    let mut m = SocialMemory::default();
    let a = m.get_or_initiate(EntityId::new(-1, 1));
    a.like = 0.4;
    a.know = 0.3;
    m.get_or_initiate(EntityId::new(-1, 2)).like = 0.2;
    let c = m.get_or_initiate(EntityId::new(-1, 3));
    c.like = -0.8;
    c.know = 0.9;
    let d = m.get_or_initiate(EntityId::new(-1, 4));
    d.like = 0.1;
    d.fear = 0.3;
    m.cycle_tick();
    assert_eq!(m.relationships.len(), 3);
    let a = &m.relationships[0];
    assert!((a.like - 0.388).abs() < 1e-6);
    assert!((a.know - 0.198).abs() < 1e-6);
    assert_eq!(m.relationships[1].like, -0.8);
    assert_eq!(m.relationships[1].know, 0.9);
    // Fear 0.3 -> x0.94 = 0.282 keeps the faint like (0.1 x0.88 = 0.088).
    assert!((m.relationships[2].fear - 0.282).abs() < 1e-6);
    assert!((m.relationships[2].like - 0.088).abs() < 1e-6);
    assert_eq!(m.like(EntityId::new(-1, 2)), 0.0);
}

#[test]
fn ids_match_by_number_as_entity_id_equality_does() {
    let mut m = SocialMemory::default();
    m.get_or_initiate(EntityId::new(-1, 7)).like = 1.0;
    assert_eq!(m.like(EntityId::new(5, 7)), 1.0);
    m.discard(EntityId::new(3, 7));
    assert!(m.relationships.is_empty());
}

#[test]
fn influences_clamp_and_knowledge_damps_like() {
    let mut r = Relationship::new(EntityId::new(-1, 0));
    r.know = 1.0;
    r.influence_like(0.5);
    assert!((r.like - 0.05).abs() < 1e-6);
    r.influence_temp_like(-3.0);
    assert_eq!(r.temp_like, -1.0);
    r.influence_know(2.0);
    assert_eq!(r.know, 1.0);
    r.even_out_temps(0.5);
    assert_eq!(r.temp_like, -0.5);
}

#[test]
fn save_strings_round_trip_with_unknown_entries() {
    let s = "REL<rA>ID.-1.0<rA>L<rB>1<rA>K<rB>0.5<rA>X<rB>y<smA>REL<rA>ID.3.12<rA>F<rB>-0.4444444";
    let m = SocialMemory::from_save_string(s).expect("parse");
    assert_eq!(m.relationships.len(), 2);
    let r = &m.relationships[0];
    assert_eq!((r.like, r.temp_like, r.know), (1.0, 1.0, 0.5));
    assert_eq!(r.unrecognized, ["X<rB>y"]);
    assert_eq!(m.relationships[1].temp_fear, -0.444_444_4);
    assert_eq!(m.save_string(), s);
}

#[test]
fn an_unsaved_last_relationship_leaves_a_trailing_separator() {
    let mut m = SocialMemory::default();
    m.get_or_initiate(EntityId::new(-1, 1)).like = 0.5;
    m.get_or_initiate(EntityId::new(-1, 2)).know = 0.5;
    assert_eq!(m.save_string(), "REL<rA>ID.-1.1<rA>L<rB>0.5<smA>");
}
