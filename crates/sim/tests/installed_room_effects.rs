//! `ElectricDeath`'s and `BorderPushBack`'s data in the installed game (read-only): the
//! four `Death_Lightning_*` lines of `sounds.txt` with clips in the sound bundle, and the
//! rooms whose effective settings (own file, else the region's template) carry each effect.
//! Expected values are from `sounds.txt` and the settings files, counted by hand
//! (`grep ElectricDeath world/uw-rooms/*_settings.txt`, `BorderPushBack-1-5-79` in
//! `world/sl/sl_settingstemplate_fog.txt`). Set `RW_DIR` to override the install; skipped
//! when absent.

use std::path::PathBuf;

use rw_assets::unity::AssetBundle;
use rw_assets::{RoomEffectType, RoomSettings, SoundTable, StreamingAssets, TriggerState};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed;

impl Installed {
    fn assets() -> Option<StreamingAssets> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data/StreamingAssets");
        if !sa.join("world/regions.txt").is_file() {
            eprintln!("skipping: no world under {}", sa.display());
            return None;
        }
        Some(StreamingAssets::new(sa))
    }

    /// Rooms of `region` with a geometry file, and each one's effective amount of `kind`.
    fn amounts(assets: &StreamingAssets, region: &str, kind: RoomEffectType) -> Vec<(String, f32)> {
        let dir = assets.file(&format!("world/{region}-rooms"));
        let mut rooms: Vec<String> = std::fs::read_dir(dir)
            .expect("room folder")
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".txt") && !n.contains("_settings") && n.starts_with(region))
            .map(|n| n.trim_end_matches(".txt").to_owned())
            .collect();
        rooms.sort();
        rooms
            .into_iter()
            .filter_map(|room| {
                let s = RoomSettings::load(assets, &room, Some(region), Some("white")).ok()?;
                let amount = s.effect_amount(kind);
                (amount > 0.0).then_some((room, amount))
            })
            .collect()
    }
}

#[test]
fn the_death_lightning_sounds_have_clips() {
    let Some(assets) = Installed::assets() else {
        return;
    };
    let bundle = assets.asset_bundle("loadedsoundeffects");
    if !bundle.is_file() {
        eprintln!("skipping: no {}", bundle.display());
        return;
    }
    let bundle = AssetBundle::open(&bundle).expect("bundle opens");
    let table = SoundTable::load(&assets, &bundle).expect("sounds.txt loads");
    // `Death_Lightning_Early_Sizzle_LOOP : GreenElecLOOP/vol=0.6`, `..._Heavy_Lightning_LOOP
    // : GreenElecLOOP2/vol=0.8`, `..._Spark_Spontaneous : Snap2/vol=0.5`, `..._Spark_Object
    // : Snap2/vol=0.7`.
    for (id, clip, vol) in [
        ("Death_Lightning_Early_Sizzle_LOOP", "greenelecloop", 0.6),
        (
            "Death_Lightning_Heavy_Lightning_LOOP",
            "greenelecloop2",
            0.8,
        ),
        ("Death_Lightning_Spark_Spontaneous", "snap2", 0.5),
        ("Death_Lightning_Spark_Object", "snap2", 0.7),
    ] {
        let TriggerState::Working(t) = table.state(id) else {
            panic!("{id} does not work");
        };
        assert_eq!(t.samples.len(), 1, "{id}");
        let sample = &t.samples[0];
        assert_eq!(table.clips()[sample.clip].name.to_lowercase(), clip, "{id}");
        assert_eq!((sample.min_vol, sample.max_vol), (vol, vol), "{id}");
    }
}

/// The 28 Underhang rooms with the effect (24 at amount 1, uw_c05 0.88, uw_a07 0.837,
/// uw_c06 0.772, uw_e02 0.446, uw_s06 0.098, uw_d02 0.054 in the survey's count of the
/// settings files).
#[test]
fn twenty_eight_underhang_rooms_have_electric_death() {
    let Some(assets) = Installed::assets() else {
        return;
    };
    let rooms = Installed::amounts(&assets, "uw", RoomEffectType::ElectricDeath);
    assert_eq!(rooms.len(), 28, "{rooms:?}");
    let full = rooms.iter().filter(|(_, a)| *a == 1.0).count();
    assert_eq!(full, 22, "{rooms:?}");
    for (room, amount) in [("uw_c05", 0.8804348), ("uw_d02", 0.054)] {
        let got = rooms.iter().find(|(r, _)| r == room).map(|(_, a)| *a);
        assert!(
            got.is_some_and(|a| (a - amount).abs() < 0.001),
            "{room} {got:?}"
        );
    }
}

/// The Sunlit Lands rooms whose effective effects include `BorderPushBack` (the fog
/// template's `BorderPushBack-1-5-79`; an own `Effects:` line adds to the template's in
/// the port's settings reading): the survey's 20 rooms of `world_sl.txt`, and `sl_f03` and
/// `sl_s01`, which have a geometry file and no settings file but are not in the world.
#[test]
fn sunlit_lands_rooms_have_border_push_back() {
    let Some(assets) = Installed::assets() else {
        return;
    };
    let rooms = Installed::amounts(&assets, "sl", RoomEffectType::BorderPushBack);
    let names: Vec<&str> = rooms.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(rooms.len(), 22, "{names:?}");
    assert!(rooms.iter().all(|(_, a)| *a == 1.0));
    for room in ["sl_c01", "sl_c03", "sl_s02", "sl_f03", "sl_s01"] {
        assert!(names.contains(&room), "{room}");
    }
    let world = std::fs::read_to_string(assets.file("world/sl/world_sl.txt")).expect("world file");
    let in_world = names
        .iter()
        .filter(|n| world.to_lowercase().contains(&format!("{n} :")))
        .count();
    assert_eq!(in_world, 20, "{names:?}");
}
