//! The player's side of the rain: `Creature.Update`'s `rainDeath` decay, the cap
//! `LungUpdate` starts with, and `Creature.TerrainImpact`'s call of
//! `CreatureSmashedInGround`. Expected values from `Creature.cs` (`Update`,
//! `TerrainImpact`), `Player.cs` (`LungUpdate`) and `RoomRain.cs`.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::*;
use crate::world::{GlobalRain, RoomRain};

/// `rainDeath = Clamp(rainDeath - 0.0125, 0, 1)` each tick.
#[test]
fn rain_death_decays_each_tick() {
    let mut p = Player::upright_at(Vec2::new(100.0, 52.0));
    let mut rng = test_rng();
    p.rain_death = 0.5;
    p.creature_update(None, &mut rng);
    assert_eq!(p.rain_death, 0.5 - 0.0125);
    p.rain_death = 0.01;
    p.creature_update(None, &mut rng);
    assert_eq!(p.rain_death, 0.0);
    p.rain_death = 3.0;
    p.creature_update(None, &mut rng);
    assert_eq!(p.rain_death, 1.0);
}

/// `LungUpdate` starts with `airInLungs = Min(airInLungs, 1 - rainDeath)`; dry, the lungs
/// then refill by 1/60 (`airInLungs += 1 / 60`).
#[test]
fn the_lungs_are_capped_by_rain_death() {
    let r = room(&["######", "#....#", "######"]);
    let mut p = Player::upright_at(Vec2::new(50.0, 32.0));
    p.rain_death = 0.3;
    p.lung_update(&r, None, &mut test_rng(), &mut Sounds::default());
    assert!((p.air_in_lungs - (0.7 + 1.0 / 60.0)).abs() < 1e-6);
    p.rain_death = 0.0;
    p.air_in_lungs = 0.5;
    p.lung_update(&r, None, &mut test_rng(), &mut Sounds::default());
    assert!((p.air_in_lungs - (0.5 + 1.0 / 60.0)).abs() < 1e-6);
}

fn smash_view() -> crate::world::SmashView {
    let rows = ["######", "#....#", "######"];
    let rain = RoomRain::new(0, &room(&rows), rw_assets::DangerType::Rain, false);
    let global = GlobalRain {
        intensity: 0.9,
        ..GlobalRain::default()
    };
    rain.smash_view(&global, 1.0).expect("pushing")
}

/// How many draws one impact makes, with the room rain's view or without, and the
/// resulting `rainDeath`.
fn impact_draws(view: Option<crate::world::SmashView>, impact: TerrainImpact) -> (usize, f32) {
    let mut p = Player::upright_at(Vec2::new(100.0, 52.0));
    p.rain_smash = view;
    let mut chunks = std::mem::take(&mut p.body.chunks);
    let mut rng = UnityRandom::new(14);
    p.terrain_impact_with(
        &mut chunks,
        impact,
        &mut rng,
        &mut Sounds::default(),
        &mut (),
    );
    let at = rng.value();
    let mut probe = UnityRandom::new(14);
    let mut n = 0;
    while probe.value() != at {
        n += 1;
        assert!(n < 100, "the streams never meet");
    }
    (n, p.rain_death)
}

/// A first floor contact under a pushing rain makes `CreatureSmashedInGround`'s three
/// draws (a chunk by `Range`, two jitters) on top of the impact's own; a slower hit, a
/// ceiling or a repeated contact make none. The speed is positive, so `rainDeath` gets
/// nothing.
#[test]
fn a_floor_smash_in_the_rain_makes_three_draws() {
    let (base, _) = impact_draws(None, TerrainImpact::floor(HIPS, 5.0, true, -1));
    let (with, death) = impact_draws(
        Some(smash_view()),
        TerrainImpact::floor(HIPS, 5.0, true, -1),
    );
    assert_eq!(with, base + 3);
    assert_eq!(death, 0.0);
    for impact in [
        TerrainImpact::floor(HIPS, 2.4, true, -1),
        TerrainImpact::floor(HIPS, 5.0, true, 1),
        TerrainImpact::floor(HIPS, 5.0, false, -1),
    ] {
        let (a, _) = impact_draws(None, impact);
        let (b, _) = impact_draws(Some(smash_view()), impact);
        assert_eq!(b, a, "{impact:?}");
    }
}
