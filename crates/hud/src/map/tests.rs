//! `HUD.Map` against the decompiled 1.11.8 `Assembly-CSharp` (`Map.Update`,
//! `CreateDiscoveryTextureFromVisitedRooms`, `RevealRoutine`,
//! `InitiateMapView`, `DiscoverMap`, `RoomToMapPos`,
//! `GateMarker`); excerpts in `rainworld-re/notes/hud.md`.

use rw_assets::PngImage;
use rw_menu::{AtlasManager, StageTree};
use rw_random::UnityRandom;
use rw_sim::{Input, MapDiscovery, MapRoom, Vec2};

use super::{GateData, GateRequirement, Map, MapData, MapDataRoom, MapOwner, ShelterData};
use rw_save::SavedCoordinate;
use rw_sim::WorldCoordinate;

use super::MarkerKind;
use crate::HudOwnerType;

/// One room (global index 1, map position (300, 300), 50 x 40 tiles, layer 1) on a
/// 210 x 630 map image (three 210 px layers), a shelter and a gate.
pub(crate) fn test_map_data() -> MapData {
    let mut data = MapData::empty();
    data.region = "SU".to_owned();
    data.first_room_index = 1;
    data.rooms = vec![
        MapDataRoom::test(1, "SU_C04", (50, 40)),
        MapDataRoom::test(2, "SU_S01", (30, 20)),
        MapDataRoom::test(3, "GATE_SU_DS", (48, 35)),
    ];
    data.shelters = vec![ShelterData {
        room: 2,
        marker_pos: Vec2::new(130.0, 70.0),
        items: None,
    }];
    data.gates = vec![
        GateData {
            room: 3,
            karma: Some(GateRequirement::FourKarma),
        },
        GateData {
            room: 3,
            karma: None,
        },
    ];
    data.image = Some(PngImage::from_pixels(
        210,
        630,
        vec![[0, 0, 0, 255]; 210 * 630],
    ));
    data
}

impl MapDataRoom {
    /// A room at map position (300, 300) on layer 1.
    fn test(index: i32, name: &str, size: (i32, i32)) -> Self {
        Self {
            name: name.to_owned(),
            map: MapRoom {
                index,
                pos: Vec2::new(300.0, 300.0),
                size,
                layer: 1,
            },
        }
    }
}

struct MapRig {
    map: Map,
    stage: StageTree,
    discovery: MapDiscovery,
    rng: UnityRandom,
    owner: MapOwner,
}

impl MapRig {
    fn new(visited: &[&str]) -> Self {
        Self::owned(HudOwnerType::Player, test_map_data(), visited)
    }

    fn owned(owner: HudOwnerType, data: MapData, visited: &[&str]) -> Self {
        let mut stage = StageTree::new();
        let container = stage.add_container(StageTree::ROOT);
        let in_front = stage.add_container(StageTree::ROOT);
        let mut atlases = AtlasManager::new();
        let map = Map::new(
            data,
            owner,
            &mut stage,
            container,
            in_front,
            &mut atlases,
            [1366.0, 768.0],
            None,
        );
        Self {
            map,
            stage,
            discovery: MapDiscovery::default(),
            rng: UnityRandom::new(7),
            owner: MapOwner {
                in_room_pos: Vec2::new(500.0, 400.0),
                room: 1,
                rooms_visited: visited.iter().map(|s| (*s).to_owned()).collect(),
                ..MapOwner::default()
            },
        }
    }

    /// One update; returns the number of draws it made.
    fn tick(&mut self, reveal: bool) -> usize {
        let before = self.rng.clone();
        self.map.update(
            &self.owner,
            reveal,
            false,
            &mut self.stage,
            &mut self.discovery,
            &mut self.rng,
        );
        let mut probe = before;
        (0..1000)
            .find(|_| {
                let same = probe.state() == self.rng.state();
                probe.value();
                same
            })
            .expect("draws counted")
    }

    fn discovered(&self, x: i32, y: i32) -> f32 {
        self.discovery.texture("SU").unwrap().0.r(x, y)
    }
}

#[test]
fn map_size_and_discovery_from_visited_rooms() {
    let mut rig = MapRig::new(&["SU_C04"]);
    // `mapSize = (width, height / 3)`.
    assert_eq!(rig.map.map_size, Vec2::new(210.0, 210.0));
    assert!(!rig.map.disc_loaded);
    // Loading makes no draws; the update draws once, the swarm check.
    assert_eq!(rig.tick(false), 1);
    let t = &rig.discovery.texture("SU").unwrap().0;
    // `(int)(210 / 7)` by `(int)(630 / 7)`.
    assert_eq!((t.width(), t.height()), (30, 90));
    // Corner: (300 / 3 + 10 - 25, 300 / 3 + 10 - 20 + 210) / 7 = (12.1, 42.9);
    // `n <= 50 / 7` and `m <= 40 / 7`: columns 12..=19, rows 42..=47 in red.
    assert_eq!(rig.discovered(12, 42), 1.0);
    assert_eq!(rig.discovered(19, 47), 1.0);
    assert_eq!(rig.discovered(20, 47), 0.0);
    assert_eq!(rig.discovered(12, 48), 0.0);
    assert_eq!(rig.discovered(11, 42), 0.0);
}

#[test]
fn walking_discovers_a_cross_around_the_player() {
    let mut rig = MapRig::new(&[]);
    rig.owner.discovery_active = true;
    assert_eq!(rig.tick(false), 1);
    // (300 / 3 + 10, 300 / 3 + 10 + 210) / 7 = (15.7, 45.7).
    for (x, y) in [(15, 45), (14, 45), (16, 45), (15, 44), (15, 46)] {
        assert_eq!(rig.discovered(x, y), 1.0, "({x}, {y})");
    }
    for (x, y) in [(14, 44), (16, 46), (14, 46), (16, 44)] {
        assert_eq!(rig.discovered(x, y), 128.0 / 255.0, "({x}, {y})");
    }
    assert_eq!(rig.discovered(17, 45), 0.0);
    // In complete darkness: `SmallDiscoverMap`, four draws.
    rig.owner.complete_darkness = true;
    rig.owner.in_room_pos = Vec2::new(700.0, 400.0);
    assert_eq!(rig.tick(false), 5);
    // (700 - 500) / 20 = 10 map pixels right: (120 / 7, 320 / 7).
    assert_eq!(rig.discovered(17, 45), 1.0);
}

#[test]
fn opening_centres_the_player_and_reveals_from_there() {
    let mut rig = MapRig::new(&[]);
    for _ in 0..30 {
        rig.tick(true);
    }
    assert_eq!(rig.map.fade, 0.0);
    // Tick 31: past the 30-tick hold the fade starts and `InitiateMapView` runs.
    assert_eq!(rig.tick(true), 1);
    assert!(rig.map.fade > 0.0);
    assert_eq!(rig.map.pan_pos, Vec2::new(110.0, 110.0));
    assert_eq!(rig.map.layer, 1);
    let marker = rig.map.player_marker().unwrap();
    assert_eq!(marker.pos, Vec2::new(683.0, 384.0));
    assert_eq!(marker.rad, 80.0 - 4.0 + (11.5 - 76.0) * 0.005);
    assert_eq!(rig.map.reveal_pixels.len(), 9);
    let reveal = rig.map.reveal().unwrap();
    assert_eq!(reveal.r(15, 45), 1.0);
    assert_eq!(reveal.r(14, 45), 128.0 / 255.0);
    // Tick 32: `RevealRoutine` takes (int)LerpMap(9, 1, 100, 6, 1) = 5 random pixels
    // and the one nearest the pan; nothing is discovered, so `FadeRoutine` draws once
    // for each of the 9 fading pixels and drops them; then the swarm check.
    assert_eq!(rig.tick(true), 5 + 9 + 1);
    assert_eq!(rig.map.reveal_pixels.len(), 3);
    assert!(rig.map.reveal_fade_pixels.is_empty());
}

#[test]
fn panning_and_layers_follow_the_map_input() {
    let mut rig = MapRig::new(&[]);
    for _ in 0..31 {
        rig.tick(true);
    }
    rig.owner.input = Input {
        x: 1,
        ..Input::NONE
    };
    rig.tick(true);
    // From rest: `panVel += dir * 0.45`.
    assert!((rig.map.pan_pos.x - 110.45).abs() < 1e-4);
    // Jump lowers the layer once per press, throw raises it (0 to 2).
    rig.owner.input = Input {
        jump: true,
        ..Input::NONE
    };
    rig.tick(true);
    rig.tick(true);
    assert_eq!(rig.map.layer, 0);
    rig.owner.input = Input {
        throw: true,
        ..Input::NONE
    };
    rig.tick(true);
    assert_eq!(rig.map.layer, 1);
}

#[test]
fn gate_and_shelter_markers() {
    let rig = MapRig::new(&[]);
    let symbols: Vec<&str> = rig
        .map
        .markers
        .iter()
        .map(|m| m.symbol(&rig.stage))
        .collect();
    // Karma 4 shows `smallKarmaNoRing3`; an unlocked gate `-1`; gates come first.
    assert_eq!(
        symbols,
        ["smallKarmaNoRing3", "smallKarmaNoRing-1", "ShelterMarker"]
    );
    assert_eq!(rig.map.markers[0].in_room_pos, Vec2::new(480.0, 240.0));
    assert_eq!(rig.map.markers[2].in_room_pos, Vec2::new(130.0, 70.0));
    assert!(
        rig.map
            .markers
            .iter()
            .all(|m| m.fade_in_rad == 3.0 && m.fade == 0.0)
    );
}

#[test]
fn room_to_map_pos_zooms_away_from_the_centre() {
    let mut rig = MapRig::new(&[]);
    for _ in 0..31 {
        rig.tick(true);
    }
    // A point one map pixel right of the pan, on the shown layer (depth 1, layer 1):
    // 4 px, times the depth zoom Lerp(3.25, 4.75, 1/3 + 1/6) / 4 = 1, plus the lens
    // 1.32 * f^4.81 (negligible this close to the centre).
    let p = rig.map.room_to_map_pos(Vec2::new(520.0, 400.0), 1, 1.0);
    assert!((p.x - 687.0).abs() < 1e-3, "{p:?}");
    assert_eq!(p.y, 384.0);
}

/// The test map with two deaths in SU (this cycle's and one 3 cycles old), one in
/// another region, and a karma flower.
fn map_data_with_deaths() -> MapData {
    let mut data = test_map_data();
    data.death_positions = [
        ("SU_C04", 10, 5, 0),
        ("HI_A01", 1, 1, 0),
        ("SU_S01", 3, 4, 3),
    ]
    .into_iter()
    .map(|(room, x, y, node)| SavedCoordinate {
        room: room.to_owned(),
        x,
        y,
        node,
    })
    .collect();
    data.karma_flower_pos = Some(WorldCoordinate::new(1, 12, 6, -1));
    data
}

#[test]
fn death_and_flower_markers_follow_the_shelters() {
    // `Map` constructor: deaths in the region (`DeathMarker`, fade-in
    // radius 3, bones over a black shade), then the flower (`FlowerMarker`, radius 5).
    for owner in [
        HudOwnerType::Player,
        HudOwnerType::SleepScreen,
        HudOwnerType::DeathScreen,
    ] {
        let rig = MapRig::owned(owner, map_data_with_deaths(), &[]);
        let symbols: Vec<&str> = rig
            .map
            .markers
            .iter()
            .map(|m| m.symbol(&rig.stage))
            .collect();
        assert_eq!(
            symbols,
            [
                "smallKarmaNoRing3",
                "smallKarmaNoRing-1",
                "ShelterMarker",
                "Multiplayer_Bones",
                "Multiplayer_Bones",
                "FlowerMarker"
            ]
        );
        let m = &rig.map.markers;
        assert_eq!((m[3].room, m[3].in_room_pos), (1, Vec2::new(200.0, 100.0)));
        // `ageFade = InverseLerp(6, 2, age)`.
        assert_eq!(
            m[3].kind,
            MarkerKind::Death {
                age: 0,
                age_fade: 1.0
            }
        );
        assert_eq!(
            m[4].kind,
            MarkerKind::Death {
                age: 3,
                age_fade: 0.75
            }
        );
        assert_eq!((m[5].room, m[5].in_room_pos), (1, Vec2::new(240.0, 120.0)));
        assert_eq!(m[5].fade_in_rad, 5.0);
        // The death screen's map has no player marker.
        let marker = rig.map.player_marker().is_some();
        assert_eq!(marker, owner != HudOwnerType::DeathScreen);
    }
}

#[test]
fn a_screen_map_opens_without_the_hold() {
    // `fadeCounter > 30 || owner != Player`: a sleep screen's map fades in at once.
    let mut rig = MapRig::owned(HudOwnerType::SleepScreen, test_map_data(), &[]);
    rig.tick(true);
    assert!(rig.map.fade > 0.0);
    assert_eq!(rig.map.reveal_pixels.len(), 9);
}
