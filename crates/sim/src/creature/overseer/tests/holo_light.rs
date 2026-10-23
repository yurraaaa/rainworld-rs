use rw_random::UnityRandom;

use super::{HoloLightTick, HologramLight, LightSourceStep};
use crate::creature::CreatureId;
use crate::{LightSource, Vec2};

fn tick() -> HoloLightTick {
    HoloLightTick {
        eye: Some(Vec2::new(100.0, 200.0)),
        overseer_chunk: (Vec2::new(100.0, 200.0), Vec2::new(100.0, 200.0)),
        overseer_extended: 1.0,
        projector_active: true,
        still_the_guides: true,
        player_here: true,
        player_elsewhere: false,
        player_entering_shortcut: false,
        main_chunk: (Vec2::new(100.0, 100.0), Vec2::new(100.0, 100.0)),
        hips: Vec2::new(100.0, 90.0),
        needed: 0.7,
        light_source_gone: false,
    }
}

/// The first seed (from `from`) whose first `value` is below 1/15 or not.
fn seed(from: i32, moves: bool) -> i32 {
    (from..)
        .find(|&s| (UnityRandom::new(s).value() < 1.0 / 15.0) == moves)
        .expect("a seed")
}

/// A still goal and no flicker: the goal's chance, the flicker's chance (0.005
/// within 500 px of the eye; a 0 flicker factor fails the third) and the flicker
/// chance: three draws. A moving goal adds `RNV` and a `value`.
#[test]
fn update_draws() {
    let mut light = HologramLight::new(1, 0, CreatureId(1));
    light.pos = Vec2::new(100.0, 150.0);
    let s = (0..)
        .map(|from| seed(from, false))
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value();
            r.value() >= 0.005
        })
        .expect("a seed");
    let mut rng = UnityRandom::new(s);
    let mut expected = rng.clone();
    for _ in 0..3 {
        expected.value();
    }
    let _ = light.update(&tick(), &mut rng);
    assert_eq!(rng, expected);

    let s = (0..)
        .map(|from| seed(from, true))
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value();
            r.value();
            r.value();
            r.value() >= 0.005
        })
        .expect("a seed");
    let mut light = HologramLight::new(1, 0, CreatureId(1));
    light.pos = Vec2::new(100.0, 150.0);
    let mut rng = UnityRandom::new(s);
    let mut expected = rng.clone();
    for _ in 0..5 {
        expected.value();
    }
    let _ = light.update(&tick(), &mut rng);
    assert_eq!(rng, expected);
    assert!(light.displace_goal.length() <= 20.0);
    assert!(light.displace_goal.length() > 0.0);
}

/// Following the player: `GoalPos` is 30% of the way from the main chunk to the
/// hips (no `inFront` yet); the light moves 7 px and a tenth of the rest toward it
/// and powers up by 1/30 a tick. The projector 20 ticks; out of the dark (`Needed`
/// below 0.1) 500 ticks power it down, then it is destroyed.
#[test]
fn follows_the_player_and_powers_down_unneeded() {
    let mut light = HologramLight::new(1, 0, CreatureId(1));
    light.pos = Vec2::new(100.0, 150.0);
    let mut rng = UnityRandom::new(3);
    let _ = light.update(&tick(), &mut rng);
    let goal = Vec2::new(100.0, 97.0);
    let expected = Vec2::new(100.0, 143.0).lerp(goal, 0.1);
    assert!(light.pos.distance(expected) < 1e-3);
    assert!((light.power - 1.0 / 30.0).abs() < 1e-6);
    assert_eq!(light.active_linger, 20);
    assert!(!light.respawn);

    let dark = HoloLightTick {
        needed: 0.0,
        ..tick()
    };
    for _ in 0..499 {
        let _ = light.update(&dark, &mut rng);
    }
    assert_eq!(light.not_needed_counter, 499);
    assert!(!light.power_down_and_kill);
    let _ = light.update(&dark, &mut rng);
    assert!(light.power_down_and_kill);
    let mut ticks = 0;
    while !light.slated {
        let _ = light.update(&dark, &mut rng);
        ticks += 1;
        assert!(ticks < 100);
    }
    assert_eq!(light.power, 0.0);
}

/// The first update asks for a light source where the light was before it moved;
/// with one it is kept, and one gone (slated or moved) is destroyed and forgotten.
#[test]
fn makes_and_drops_its_light_source() {
    let mut light = HologramLight::new(1, 0, CreatureId(1));
    light.pos = Vec2::new(100.0, 150.0);
    let mut rng = UnityRandom::new(3);
    let step = light.update(&tick(), &mut rng);
    assert_eq!(step, LightSourceStep::Create(Vec2::new(100.0, 150.0)));
    assert_ne!(light.pos, Vec2::new(100.0, 150.0));
    light.light_source = Some(7);
    assert_eq!(light.update(&tick(), &mut rng), LightSourceStep::Keep);
    let gone = HoloLightTick {
        light_source_gone: true,
        ..tick()
    };
    assert_eq!(light.update(&gone, &mut rng), LightSourceStep::Destroy(7));
    assert_eq!(light.light_source, None);
}

/// `Needed`: `InverseLerp(0.85, 0.92, darkness)` in the camera's room, alive; a
/// light of rad 200 and alpha 0.5 takes `InverseLerp(400, 160, distance)` off it
/// within 400 px; the guide light's own (alpha at most 0.1) none.
#[test]
fn needed_by_darkness() {
    let p = Vec2::ZERO;
    assert_eq!(HologramLight::needed(true, 0.8, false, p, &[], 1.0), 0.0);
    assert!((HologramLight::needed(true, 0.9, false, p, &[], 1.0) - 0.05 / 0.07).abs() < 1e-6);
    assert_eq!(HologramLight::needed(true, 1.0, false, p, &[], 1.0), 1.0);
    assert_eq!(HologramLight::needed(false, 1.0, false, p, &[], 1.0), 0.0);
    assert_eq!(HologramLight::needed(true, 1.0, true, p, &[], 1.0), 0.0);
    let mut light = LightSource::new(1, 0, Vec2::new(280.0, 0.0), false, [1.0; 3], None);
    light.rad = 200.0;
    light.alpha = 0.5;
    let n = HologramLight::needed(true, 1.0, false, p, std::slice::from_ref(&light), 1.0);
    assert!((n - 0.5).abs() < 1e-6);
    light.alpha = 0.1;
    let n = HologramLight::needed(true, 1.0, false, p, std::slice::from_ref(&light), 1.0);
    assert_eq!(n, 1.0);
    light.alpha = 0.5;
    light.pos = Vec2::new(100.0, 0.0);
    let n = HologramLight::needed(true, 1.0, false, p, std::slice::from_ref(&light), 1.0);
    assert_eq!(n, 0.0);
    // A placed light fading with the sun counts by `Alpha`: at `ShaderLight` -0.4
    // it is 0.5 * 0.2^1.2 (below 0.2), at -0.1 0.5 * 0.8^1.2 (above).
    light.fade_with_sun = true;
    let n = HologramLight::needed(true, 1.0, false, p, std::slice::from_ref(&light), -0.4);
    assert_eq!(n, 1.0);
    let n = HologramLight::needed(true, 1.0, false, p, std::slice::from_ref(&light), -0.1);
    assert_eq!(n, 0.0);
}
