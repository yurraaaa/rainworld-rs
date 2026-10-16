//! Outskirts room settings' `Triggers:` and `BkgDroneVolume` against the installed game
//! (read-only). Expected values are read from the `world/su-rooms` settings files and
//! `world/su/su_settingstemplate_*.txt`. Set `RW_DIR` to override the install; tests
//! skip when it is absent.

use std::path::PathBuf;

use rw_assets::{EventType, RoomSettings, StreamingAssets, TriggerType, TriggeredEvent};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed;

impl Installed {
    fn assets() -> Option<StreamingAssets> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let assets = StreamingAssets::new(root.join("RainWorld_Data/StreamingAssets"));
        if !assets.file("world/su-rooms").is_dir() {
            eprintln!("skipping: no world/su-rooms");
            return None;
        }
        Some(assets)
    }

    fn settings(assets: &StreamingAssets, room: &str) -> RoomSettings {
        RoomSettings::load(assets, room, Some("su"), Some("white")).expect("settings load")
    }

    /// Every `su_*_settings.txt` room name.
    fn su_rooms(assets: &StreamingAssets) -> Vec<String> {
        let mut rooms: Vec<String> = std::fs::read_dir(assets.file("world/su-rooms"))
            .expect("su-rooms")
            .filter_map(|e| {
                let name = e.ok()?.file_name().into_string().ok()?;
                name.strip_suffix("_settings.txt").map(str::to_owned)
            })
            .collect();
        rooms.sort();
        rooms
    }

    fn count(kinds: &[(String, Option<EventType>)], kind: EventType) -> usize {
        kinds.iter().filter(|(_, e)| *e == Some(kind)).count()
    }
}

#[test]
fn outskirts_triggers() {
    let Some(assets) = Installed::assets() else {
        return;
    };
    let mut music = Vec::new();
    let mut kinds = Vec::new();
    for room in Installed::su_rooms(&assets) {
        for t in Installed::settings(&assets, &room).triggers {
            assert_eq!(t.trigger_type, Some(TriggerType::Spot), "{room}");
            let kind = t.event.as_ref().and_then(TriggeredEvent::event_type);
            kinds.push((room.clone(), kind));
            if let Some(TriggeredEvent::Music(m)) = t.event {
                music.push((
                    room.clone(),
                    m.song_name,
                    t.fire_chance,
                    t.entrance,
                    m.rooms_range,
                ));
            }
        }
    }
    assert_eq!(kinds.len(), 11);

    assert_eq!(
        Installed::count(&kinds, EventType::ShowProjectedImageEvent),
        5
    );
    assert_eq!(
        Installed::count(&kinds, EventType::PickUpObjectInstruction),
        1
    );
    assert_eq!(
        Installed::count(&kinds, EventType::BringPlayerGuideToRoom),
        2
    );
    assert_eq!(
        music,
        [
            (
                "su_a07".to_owned(),
                "NA_01 - Proxima".to_owned(),
                1.0,
                -1,
                6
            ),
            (
                "su_a53".to_owned(),
                "NA_40 - Unseen Lands".to_owned(),
                1.0,
                3,
                -1
            ),
            (
                "su_b04".to_owned(),
                "RW_1 - Urban Jungle".to_owned(),
                0.456_521_7,
                -1,
                -1
            ),
        ]
    );
}

#[test]
fn su_a07_trigger_and_drone_volumes() {
    let Some(assets) = Installed::assets() else {
        return;
    };
    let a07 = Installed::settings(&assets, "su_a07");
    assert_eq!(a07.bkg_drone_volume, 0.276_956_5);
    let t = &a07.triggers[0];
    let spot = t.spot.expect("spot");
    assert_eq!(spot.pos, [710.0718, 289.2014]);
    assert_eq!(spot.handle, [-103.3553, -215.3553]);
    let Some(TriggeredEvent::Music(m)) = &t.event else {
        panic!("music");
    };
    assert_eq!((m.volume, m.fade_in_time), (0.310_869_3, 202.173_9));
    // su_a31 has no BkgDroneVolume line: its template's.
    assert_eq!(
        Installed::settings(&assets, "su_a31").bkg_drone_volume,
        0.276_956_5
    );
    // su_b13's guide trigger needs karma 2; su_a23's pickup lasts to cycle 7.
    let b13 = Installed::settings(&assets, "su_b13");
    assert_eq!(b13.triggers[0].karma, 2);
    let a23 = Installed::settings(&assets, "su_a23");
    assert_eq!(a23.triggers[0].active_to_cycle, 7);
    assert_eq!(a23.triggers[1].entrance, 2);
}
