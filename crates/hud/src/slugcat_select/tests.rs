//! `SlugcatSelectMenu.MineForSaveData` (555228) and `SlugcatPage.AddImage` (553799).

use super::*;

/// A stored Hunter state cut down to what the miner reads.
const RED: &str = "SAV STATE NUMBER<svB>Red<svA>SEED<svB>77<svA>DENPOS<svB>LF_S02<svA>\
    LASTVDENPOS<svB>LF_S03<svA>CYCLENUM<svB>7<svA>FOOD<svB>11<svA>HASTHEGLOW<svA>\
    REDEXTRACYCLES<svA>DEATHPERSISTENTSAVEDATA<svB>KARMA<dpB>8<dpA>KARMACAP<dpB>6<dpA>\
    REINFORCEDKARMA<dpB>1<dpA>HASTHEMARK<dpA>REDSDEATH<dpA><svA>";

fn real_room(room: &str) -> bool {
    room == "LF_S02" || room == "LF_S03"
}

#[test]
fn mines_the_stored_values() {
    let d = SaveGameData::from_stored(Slugcat::Red, RED, real_room);
    assert_eq!(d.shelter_name, "LF_S02");
    assert_eq!((d.cycle, d.food, d.karma, d.karma_cap), (7, 11, 8, 6));
    assert!(d.karma_reinforced && d.has_glow && d.has_mark);
    assert!(d.reds_extra_cycles && d.reds_death && !d.ascended);
    assert_eq!(d.ripple_level, 0.0);
}

#[test]
fn reds_death_is_only_the_hunters() {
    let line = RED.replace("Red<svA>SEED", "White<svA>SEED");
    let d = SaveGameData::from_stored(Slugcat::White, &line, real_room);
    assert!(!d.reds_death);
}

#[test]
fn the_den_falls_back_to_the_last_vanilla_den_then_the_final_shelter() {
    // `DENPOS` is no room: `LASTVDENPOS`.
    let line = RED.replace("DENPOS<svB>LF_S02", "DENPOS<svB>XX_S99");
    let d = SaveGameData::from_stored(Slugcat::Red, &line, real_room);
    assert_eq!(d.shelter_name, "LF_S03");
    // Neither: `GetFinalFallbackShelter`.
    let d = SaveGameData::from_stored(Slugcat::Yellow, &line, |_| false);
    assert_eq!(d.shelter_name, "SU_S01");
    let d = SaveGameData::from_stored(Slugcat::Red, &line, |_| false);
    assert_eq!(d.shelter_name, "LF_S02");
}

#[test]
fn a_bad_number_stays_zero() {
    let line = RED.replace("FOOD<svB>11", "FOOD<svB>x");
    assert_eq!(
        SaveGameData::from_stored(Slugcat::Red, &line, real_room).food,
        0
    );
}

#[test]
fn ascended_and_dead_hunter_images() {
    assert_eq!(
        SlugcatPage::image(Slugcat::White, false, true),
        (SceneId::SlugcatWhite, Vec2::new(-10.0, 100.0))
    );
    assert_eq!(
        SlugcatPage::image(Slugcat::Yellow, true, false).0,
        SceneId::GhostYellow
    );
    assert_eq!(
        SlugcatPage::image(Slugcat::Red, false, true),
        (SceneId::SlugcatDeadRed, Vec2::new(10.0, 45.0))
    );
    // Ascended wins over dead.
    assert_eq!(
        SlugcatPage::image(Slugcat::Red, true, true).0,
        SceneId::GhostRed
    );
}
