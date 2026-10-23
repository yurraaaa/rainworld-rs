//! `BackgroundScene`: the elements the constructors make (counted from
//! `AboveCloudsView.cs` and `RoofTopView.cs`), the private stream, the first turn's
//! order, `DrawPos`, and `DistantLightning`'s draws.

use rw_random::UnityRandom;

use crate::above_clouds_view::AboveCloudsContext;
use crate::background_scene::{BackgroundScene, ElementKind, SCREEN_SIZE, SceneElement};
use crate::{DistantLightning, Vec2};

fn clouds(region: Option<&str>, room: &str) -> BackgroundScene {
    BackgroundScene::above_clouds_view(&AboveCloudsContext {
        region,
        room_name: room,
        room_origin: Vec2::new(1000.0, 2000.0),
        arena: false,
        effect_amount: 1.0,
        clouds: (None, None),
    })
}

/// Counts by kind: fog, skies, buildings, lightnings, close, distant, flying.
fn counts(scene: &BackgroundScene) -> [usize; 7] {
    let mut c = [0; 7];
    for e in &scene.elements {
        let i = match e.kind {
            ElementKind::Fog => 0,
            ElementKind::Sky { .. } => 1,
            ElementKind::DistantBuilding { .. } => 2,
            ElementKind::DistantLightning(_) => 3,
            ElementKind::CloseCloud { .. } => 4,
            ElementKind::DistantCloud { .. } => 5,
            ElementKind::FlyingCloud { .. } => 6,
            _ => continue,
        };
        c[i] += 1;
    }
    c
}

/// How many draws `before` to `after` took.
fn draws(before: &UnityRandom, after: &UnityRandom) -> usize {
    let mut probe = before.clone();
    (0..40)
        .find(|_| {
            if probe == *after {
                return true;
            }
            probe.next_u32();
            false
        })
        .expect("within 40 draws")
}

/// The first `AtC_Structure1`'s position.
fn first_structure(s: &BackgroundScene) -> Vec2 {
    s.elements
        .iter()
        .find(|e| {
            matches!(
                &e.kind,
                ElementKind::DistantBuilding {
                    asset: "AtC_Structure1",
                    ..
                }
            )
        })
        .map(|e| e.pos)
        .unwrap()
}

/// The depths of the elements `pick` takes, in element order.
fn depths(scene: &BackgroundScene, pick: fn(&ElementKind) -> bool) -> Vec<f32> {
    scene
        .elements
        .iter()
        .filter(|e| pick(&e.kind))
        .map(|e| e.depth)
        .collect()
}

#[test]
fn underground_city_room_has_the_constructors_fifty_elements() {
    // AboveCloudsView.cs: Fog, 3 skies, 22 DistantBuildings (Structure1-6, Spire1-9,
    // then Structure1@120, Spire2@160, Spire5@365, Spire6@285, Spire7@375, Spire8@380,
    // Spire9@390), 4 DistantLightnings, 7 CloseClouds, 11 DistantClouds, 2 FlyingClouds.
    let scene = clouds(Some("UW"), "UW_A01");
    assert_eq!(counts(&scene), [1, 3, 22, 4, 7, 11, 2]);
    assert_eq!(scene.elements.len(), 50);
    assert_eq!(scene.scene_origo, Vec2::new(2514.0, 25700.0));
}

#[test]
fn sky_islands_rooms_have_no_distant_or_flying_clouds() {
    // SI_A06: Spire1@50, Structure1-6, Spire1-9, 3 lightnings, 7 CloseClouds.
    let scene = clouds(Some("SI"), "SI_A06");
    assert_eq!(counts(&scene), [1, 3, 16, 3, 7, 0, 0]);
    assert_eq!(scene.scene_origo, Vec2::new(2514.0, 17700.0));
    // SI_A07 has the wide rows, shifted 200 to the right, and its Spire1@50 too.
    let a07 = clouds(Some("SI"), "SI_A07");
    assert_eq!(counts(&a07), [1, 3, 23, 4, 7, 0, 0]);
    // PosFromDrawPos with the constructor's zero convergence point: input * depth.
    assert_eq!(
        first_structure(&clouds(Some("SI"), "SI_B02")),
        Vec2::new(-520.0 * 160.0, -85.0 * 160.0)
    );
    assert_eq!(
        first_structure(&a07),
        Vec2::new((-520.0 + 200.0) * 160.0, -85.0 * 160.0)
    );
}

#[test]
fn the_clouds_draw_their_offsets_from_the_seed_12_stream() {
    // 7 CloseClouds, 11 DistantClouds, 2 FlyingClouds, in that order.
    let scene = clouds(Some("UW"), "UW_A01");
    let mut stream = UnityRandom::new(12);
    for e in &scene.elements {
        let cloud = match &e.kind {
            ElementKind::CloseCloud { cloud, .. }
            | ElementKind::DistantCloud { cloud, .. }
            | ElementKind::FlyingCloud { cloud, .. } => cloud,
            _ => continue,
        };
        assert_eq!(cloud.random_offset, stream.value());
    }
    let si = clouds(Some("SI"), "SI_B04");
    let mut stream = UnityRandom::new(12);
    for e in &si.elements {
        if let ElementKind::CloseCloud { cloud, .. } = &e.kind {
            assert_eq!(cloud.random_offset, stream.value());
        }
    }
}

#[test]
fn clouds_have_the_constructors_depths() {
    let scene = clouds(Some("UW"), "UW_A01");
    // CloudDepth(f) = Lerp(5, 40, f); DistantCloudDepth(f) = Lerp(40, 200, f^1.5).
    let close = depths(&scene, |k| matches!(k, ElementKind::CloseCloud { .. }));
    assert_eq!(close[0], 5.0);
    assert_eq!(close[6], 40.0);
    assert!((close[3] - 22.5).abs() < 1e-4);
    let distant = depths(&scene, |k| matches!(k, ElementKind::DistantCloud { .. }));
    assert_eq!(distant[0], 40.0);
    assert!((distant[10] - 200.0).abs() < 1e-3);
    assert!((distant[5] - (40.0 + 160.0 * 0.5_f32.powf(1.5))).abs() < 1e-3);
}

#[test]
fn the_first_turn_adds_far_to_near_with_equal_depths_in_construction_order() {
    let scene = clouds(Some("UW"), "UW_A01");
    let order = scene.add_order();
    // The three skies (float.MaxValue) first, night, dusk, day; the fog (0) last.
    assert!(matches!(
        scene.elements[order[0]].kind,
        ElementKind::Sky {
            name: "AtC_NightSky",
            ..
        }
    ));
    assert!(matches!(
        scene.elements[order[1]].kind,
        ElementKind::Sky {
            name: "AtC_DuskSky",
            ..
        }
    ));
    assert!(matches!(
        scene.elements[order[2]].kind,
        ElementKind::Sky {
            name: "AtC_Sky",
            ..
        }
    ));
    assert!(matches!(
        scene.elements[*order.last().unwrap()].kind,
        ElementKind::Fog
    ));
    let depths: Vec<f32> = order.iter().map(|&i| scene.elements[i].depth).collect();
    assert!(depths.windows(2).all(|w| w[0] >= w[1]));
    // A DistantLightning sorts by depth less its minusDepthForLayering: Light1@160-55.
    let light1 = order
        .iter()
        .map(|&i| &scene.elements[i])
        .find(|e| matches!(&e.kind, ElementKind::DistantLightning(l) if l.minus_depth_for_layering == 55.0))
        .unwrap();
    assert_eq!(light1.depth, 105.0);
}

#[test]
fn draw_pos_puts_an_element_at_its_input_offset_from_the_convergence_point() {
    let mut scene = clouds(Some("UW"), "UW_A01");
    scene.convergence_point = BackgroundScene::convergence_for(SCREEN_SIZE);
    assert_eq!(scene.convergence_point, Vec2::new(683.0, 512.0));
    let input = Vec2::new(-520.0, -85.0);
    let pos = scene.pos_from_draw_pos_at_neutral_cam_pos(input, 160.0);
    // With the camera where RoomToWorldPos(camPos) = sceneOrigo (and hDisplace 8).
    let cam = scene.scene_origo - scene.room_origin;
    let at = scene.draw_pos(pos, 160.0, cam, 8.0);
    assert!((at.x - (input.x + 683.0)).abs() < 1e-3);
    assert!((at.y - (input.y + 512.0)).abs() < 1e-3);
}

#[test]
fn the_roof_top_view_has_the_constructors_elements_and_four_smoke_draws() {
    let mut rng = UnityRandom::new(3);
    let before = rng.clone();
    let scene = BackgroundScene::roof_top_view(Vec2::new(500.0, 600.0), (400, 80), &mut rng);
    // RoofTopView.cs: 3 skies, Floor, 16 Rubble, Rf_HoleFix, 2 Buildings, 3 city
    // DistantBuildings, 4 Smoke.
    assert_eq!(scene.elements.len(), 3 + 1 + 16 + 1 + 2 + 3 + 4);
    // The Smoke draws are the only ones on the shared stream (Rubble's SeededRandom
    // restores it): 4.
    assert_eq!(draws(&before, &rng), 4);
    let mut check = before;
    let offsets: Vec<f32> = scene
        .elements
        .iter()
        .filter_map(|e| match e.kind {
            ElementKind::Smoke { random_offset, .. } => Some(random_offset),
            _ => None,
        })
        .collect();
    assert_eq!(
        offsets,
        [check.value(), check.value(), check.value(), check.value()]
    );
    // sceneOrigo = RoomToWorldPos(size * 10).
    assert_eq!(scene.scene_origo, Vec2::new(500.0 + 4000.0, 600.0 + 800.0));
    // Rubble i = 3: SeededRandom(3), depth Lerp(1.5, 8, (3/15)^1.5).
    let rubble = scene
        .elements
        .iter()
        .filter_map(|e| match e.kind {
            ElementKind::Rubble { random_value, .. } => Some((random_value, e.depth)),
            _ => None,
        })
        .nth(3)
        .unwrap();
    assert_eq!(rubble.0, UnityRandom::new(3).value());
    assert!((rubble.1 - (1.5 + 6.5 * 0.2_f32.powf(1.5))).abs() < 1e-4);
}

fn light() -> DistantLightning {
    DistantLightning::new("AtC_Light1", 55.0)
}

#[test]
fn a_lightnings_first_turn_draws_five() {
    // randomLevelChange 0 -> Range(1, 6) and value (2); thunder -1 -> Reset (3).
    let mut l = light();
    let mut rng = UnityRandom::new(7);
    let before = rng.clone();
    assert_eq!(l.update(&mut rng), 55.0);
    assert_eq!(draws(&before, &rng), 5);
    // The depth is restored once.
    assert_eq!(l.update(&mut rng), 0.0);
    assert!(l.wait >= 9);
    assert!((1..32).contains(&l.thunder_length));
    assert!((0.7..=1.0).contains(&l.power));
}

#[test]
fn a_lightnings_tiny_thunder_starts_on_the_fifth_turn() {
    let mut l = light();
    let mut rng = UnityRandom::new(11);
    for _ in 0..4 {
        l.update(&mut rng);
    }
    assert_eq!((l.tiny_thunder_wait, l.tiny_thunder), (1, 0));
    l.update(&mut rng);
    // tinyThunderWait = Range(10, 80); tinyThunderLength = Range(5, wait); one tick used.
    assert!((10..80).contains(&l.tiny_thunder_wait));
    assert!((5..l.tiny_thunder_wait).contains(&l.tiny_thunder_length));
    assert_eq!(l.tiny_thunder, l.tiny_thunder_length - 1);
}

#[test]
fn a_lightnings_turn_draws_by_state() {
    // Per turn: 2 when randomLevelChange runs out, 3 for a Reset when a bolt ends,
    // 2 when tinyThunderWait runs out and 1 per turn while tinyThunder is above 0 after
    // that (a new tiny thunder included).
    let mut l = light();
    let mut rng = UnityRandom::new(5);
    for turn in 0..400 {
        let before = rng.clone();
        let level = l.random_level_change - 1 < 1;
        let ends_bolt = l.wait <= 0 && l.thunder - 1 < 1;
        let starts_tiny = l.tiny_thunder_wait == 1;
        let tiny_going = l.tiny_thunder > 0;
        l.update(&mut rng);
        let expected = 2 * usize::from(level)
            + 3 * usize::from(ends_bolt)
            + 2 * usize::from(starts_tiny)
            + usize::from(starts_tiny || tiny_going);
        assert_eq!(draws(&before, &rng), expected, "turn {turn}");
    }
}

#[test]
fn light_intensity_draws_one_or_three() {
    let mut l = light();
    l.last_intensity = 0.4;
    l.intensity = 0.6;
    let (mut one, mut three) = (false, false);
    for seed in 0..60 {
        let mut rng = UnityRandom::new(seed);
        let before = rng.clone();
        let first = rng.clone().value();
        let v = l.light_intensity(0.5, &mut rng);
        let n = draws(&before, &rng);
        if first < 1.0 / 3.0 {
            assert_eq!(n, 3, "seed {seed}");
            three = true;
        } else {
            assert_eq!(n, 1, "seed {seed}");
            // Custom.SCurve(0.5, 0.5) of the lerp 0.5 between 0.4 and 0.6.
            assert!((v - 0.5).abs() < 0.05);
            one = true;
        }
    }
    assert!(one && three);
}

#[test]
fn scene_element_new_has_no_place_until_added() {
    let e = SceneElement::new(Vec2::ZERO, 3.0, ElementKind::Fog);
    assert_eq!(e.update_order, 0);
}

/// `convergencePoint = (ScreenSize.x / 2, ScreenSize.y * 2 / 3)`.
#[test]
fn the_convergence_point_follows_the_chosen_screen() {
    assert_eq!(
        BackgroundScene::convergence_for(Vec2::new(1280.0, 768.0)),
        Vec2::new(640.0, 512.0)
    );
    assert_eq!(
        BackgroundScene::convergence_for(Vec2::new(1229.0, 768.0)),
        Vec2::new(614.5, 512.0)
    );
}
