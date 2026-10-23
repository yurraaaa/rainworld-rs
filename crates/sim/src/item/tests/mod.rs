use super::*;

#[test]
fn stuck_in_wall_tick_counts_toward_zero() {
    let mut s = AbstractSpear {
        stuck_in_wall_cycles: 5,
        explosive: false,
    };
    s.stuck_in_wall_tick(2);
    assert_eq!(s.stuck_in_wall_cycles, 3);
    s.stuck_in_wall_tick(7);
    assert_eq!(s.stuck_in_wall_cycles, 0);
    assert!(!s.stuck_in_wall());
    let mut v = AbstractSpear {
        stuck_in_wall_cycles: -4,
        explosive: false,
    };
    v.stuck_in_wall_tick(1);
    assert_eq!(v.stuck_in_wall_cycles, -3);
    v.stuck_in_wall_tick(3);
    assert_eq!(v.stuck_in_wall_cycles, 0);
}

#[test]
fn spear_string_round_trips_with_extra_fields() {
    // A More Slugcats spear (hue, electric, charge, needle) read with the DLCs off:
    // the four DLC fields are unrecognized attributes, written back after ours.
    let s = "ID.-1.1234<oB>0<oA>Spear<oA>SU_S01.12.5.-1<oA>-3<oA>0<oA>0<oA>0<oA>0<oA>0";
    let saved = SavedObject::parse(s, ActiveMods::NONE).expect("parses");
    assert_eq!(saved.room, "SU_S01");
    assert_eq!((saved.tile, saved.node), ((12, 5), -1));
    assert_eq!(saved.unrecognized_attributes.len(), 4);
    let o = saved.into_object(7);
    assert_eq!(o.spear().map(|s| s.stuck_in_wall_cycles), Some(-3));
    assert!(o.stuck_in_wall());
    assert_eq!(o.save_string("SU_S01.12.5.-1"), s);
}

#[test]
fn rock_without_ripple_layer_gets_one() {
    let saved =
        SavedObject::parse("ID.5.77<oA>Rock<oA>SU_A13.3.4.2", ActiveMods::NONE).expect("rock");
    assert_eq!(saved.id, EntityId::new(5, 77));
    let o = saved.into_object(1);
    assert_eq!(
        o.save_string("SU_A13.3.4.2"),
        "ID.5.77<oB>0<oA>Rock<oA>SU_A13.3.4.2"
    );
}

#[test]
fn other_kinds_and_bad_strings_stay_text() {
    let none = ActiveMods::NONE;
    assert!(
        SavedObject::parse(
            "ID.-1.5<oB>0<oA>DangleFruit<oA>SU_S01.1.1.0<oA>3<oA>2",
            none
        )
        .is_none()
    );
    assert!(SavedObject::parse("ID.-1.5<oB>0<oA>LillyPuck<oA>SU_S01.1.1.0", none).is_none());
    assert!(SavedObject::parse("ID.-1.5<oB>0<oA>Spear<oA>SU_S01.1.1.0<oA>2", none).is_none());
    assert!(SavedObject::parse("ID.-1.x<oB>0<oA>Rock<oA>SU_S01.1.1.0", none).is_none());
    let inv = SavedObject::parse("ID.-1.5<oB>0<oA>Rock<oA>INV.XX_A01.1.1.0", none).expect("inv");
    assert_eq!(inv.room, "XX_A01");
}
