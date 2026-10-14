//! `sounds.txt` against the installed game's file and `loadedsoundeffects` bundle
//! (read-only); expected values are from the file and `rainworld-re/notes/sound.md`
//! section 1.1. Set `RW_DIR` to override the install; tests skip when it is absent.

use std::path::PathBuf;

use rw_assets::unity::AssetBundle;
use rw_assets::{SoundTable, StreamingAssets, TriggerState};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed;

impl Installed {
    /// Sample, volume and pitch of an Outskirts room's ambient sounds.
    fn ambient(assets: &StreamingAssets, room: &str) -> Vec<(String, f32, f32)> {
        rw_assets::RoomSettings::load(assets, room, Some("su"), Some("white"))
            .unwrap()
            .ambient_sounds
            .iter()
            .map(|a| (a.sample.clone(), a.volume, a.pitch))
            .collect()
    }

    fn table() -> Option<SoundTable> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let assets = StreamingAssets::new(root.join("RainWorld_Data/StreamingAssets"));
        let bundle = assets.asset_bundle("loadedsoundeffects");
        if !bundle.is_file() {
            eprintln!("skipping: no {}", bundle.display());
            return None;
        }
        let bundle = AssetBundle::open(&bundle).expect("bundle opens");
        Some(SoundTable::load(&assets, &bundle).expect("sounds.txt loads"))
    }
}

#[test]
fn header_values() {
    let Some(t) = Installed::table() else { return };
    assert_eq!((t.volume, t.volume_exponent), (6.0, 1.8));
}

#[test]
fn ids_naming_missing_clips_are_silent() {
    let Some(t) = Installed::table() else { return };
    for id in [
        "Slugcat_Terrain_Impact_Death",
        "UI_Slugcat_Stunned_Init",
        "UI_Slugcat_Die",
        "Leech_Detatch_Player",
        "Leviathan_Jaws_Armed",
        "Shelter_Segment_Pair_Collide",
        "MENU_Karma_Ladder_Hit_Lower_Cap",
        "MENU_Karma_Ladder_Reinforcement_Dissipate_B",
        "Boomerang_Flying_LOOP",
    ] {
        assert_eq!(t.state(id), TriggerState::MissingClip, "{id}");
    }
    for id in [
        "Player_Enter_Shortcut",
        "Player_Exit_Shortcut",
        "Slugcat_Vertical_Chute_Jump_Success",
        "Slugcat_Regain_Footing",
    ] {
        assert_eq!(t.state(id), TriggerState::NoLine, "{id}");
    }
}

#[test]
fn variation_counts_come_from_the_bundle() {
    let Some(t) = Installed::table() else { return };
    let jump = t.trigger("slugcat_normal_jump").unwrap();
    let clip = &t.clips()[jump.samples[0].clip];
    assert_eq!(clip.name, "jump2");
    assert_eq!(clip.variations.len(), 7);
    assert_eq!(clip.variations[6].asset, "jump2_7");
    assert!(clip.variations.iter().all(|v| v.exists));
    let step = t.trigger("Slugcat_Step_A").unwrap();
    let walk = &t.clips()[step.samples[0].clip];
    assert_eq!(
        (walk.variations.len(), walk.variations[0].asset.as_str()),
        (1, "walk3A")
    );
    assert_eq!(
        (step.samples[0].min_vol, step.samples[0].max_vol),
        (0.3, 0.3)
    );
}

#[test]
fn shipped_typos_have_no_effect() {
    let Some(t) = Installed::table() else { return };
    assert_eq!(
        t.trigger("SL_AI_Protest_3").unwrap().samples[0].min_vol,
        1.0
    );
    assert_eq!(t.trigger("Fly_Wing_Flap").unwrap().range, 1.0);
    let slide = t.trigger("Slugcat_Slide_In_Narrow_Corridor_LOOP").unwrap();
    assert_eq!(
        (slide.samples[1].min_pitch, slide.samples[1].max_pitch),
        (1.0, 1.0)
    );
    let skid = t.trigger("Slugcat_Skid_On_Ground_Init").unwrap();
    assert_eq!(skid.samples.len(), 7);
    assert_eq!(t.clips()[skid.samples[1].clip].name, "Skitter1B");
    assert_eq!(skid.samples[1].min_vol, 0.5);
}

#[test]
fn trigger_options() {
    let Some(t) = Installed::table() else { return };
    let jump = t.trigger("Slugcat_Super_Jump").unwrap();
    assert!(jump.play_all);
    assert_eq!(jump.samples.len(), 2);
    assert_eq!(
        jump.doppler_fac, 1.0,
        "the sample's dopplerFac is not a sample option"
    );
    assert_eq!(
        t.trigger("Player_Coral_Circuit_Swim")
            .unwrap()
            .silent_chance,
        0.5
    );
    let pluck = t.trigger("Void_Weaver_Pluck_Thread").unwrap();
    assert_eq!(
        (pluck.range, pluck.doppler_fac, pluck.silent_chance),
        (0.5, 0.3, 0.7)
    );
    assert_eq!((pluck.min_vol, pluck.max_vol), (0.5, 0.5));
}

/// `AmbientSounds:` with template inheritance on Outskirts rooms (region templates
/// Outside, Inside; values from the installed settings files).
#[test]
fn room_ambient_sounds_inherit_from_templates() {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let assets = StreamingAssets::new(root.join("RainWorld_Data/StreamingAssets"));
    if !assets.file("world/su/properties.txt").is_file() {
        return;
    }
    let outside = ("AM_ENV-Outside.ogg".to_owned(), 0.5, 0.823_913);
    // No file entries, no Template line: the first template's sound.
    assert_eq!(
        Installed::ambient(&assets, "SU_A14"),
        std::slice::from_ref(&outside)
    );
    // Own entries first, then the template's sound with another sample.
    let a04 = Installed::ambient(&assets, "SU_A04");
    let samples: Vec<&str> = a04.iter().map(|a| a.0.as_str()).collect();
    assert_eq!(
        samples,
        [
            "AM_ENV-LightWind.ogg",
            "SO_IND-HeatDuct.ogg",
            "AM_IND-DeepHum.ogg",
            "AM_ENV-Outside.ogg"
        ]
    );
    assert_eq!(a04[3], outside);
    // Template: NONE: only its own two (the OMNI shares the sample but not the values).
    let a22 = Installed::ambient(&assets, "SU_A22");
    assert_eq!(a22.len(), 2);
    assert_eq!(
        a22[0],
        ("AM_ENV-Outside.ogg".to_owned(), 0.391_304_3, 0.686_956_5)
    );
}
