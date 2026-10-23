//! `Room.Update`'s ceiling drip and `Room.Loaded`'s `DrippingSound`. Expected values
//! from `Room.cs` (`Update`, `Loaded`) and `WaterDrip`'s constructor.

use rw_assets::DangerType;
use rw_random::UnityRandom;

use crate::creature::lizard::perception_turn::tests::{HALL, game};
use crate::{FloatExt, Input, Vec2, WaterDrip};

/// The roll in `HALL` (28 ceiling tiles, `CeilingDrips` 0.5) at `CycleStartUp`
/// `start_up` from `seed`, replayed by hand: `value < gravity` (1), then `value >
/// (1 - Max((1 - start_up) * InverseLerp(0, 0.5, 0.5), 0.5^7) * 0.05) ^ 28` (in
/// doubles), then the tile, the x jitter and the life.
fn expected(seed: i32, start_up: f32) -> (UnityRandom, Option<WaterDrip>) {
    let g = game(&HALL, (3, 1));
    let room = g.world().realized(0).expect("realized");
    let mut e = UnityRandom::new(seed);
    assert!(e.value() < 1.0);
    let chance = ((1.0 - start_up) * 1.0_f32).max(0.5_f32.mathf_pow(7.0)) * 0.05;
    let none = f64::from(1.0 - chance).powf(28.0);
    if f64::from(e.value()) <= none {
        return (e, None);
    }
    let tile = room.ceiling_tiles[e.range_i32(0, 28) as usize];
    let pos = Vec2::middle_of_tile(tile) + Vec2::new((-10.0).lerp(10.0, e.value()), 9.0);
    let drip = WaterDrip::new(pos, Vec2::ZERO, false, &mut e);
    (e, Some(drip))
}

/// Early in the cycle (`(1 - 0) * 1 * 0.05` a tile: 76 % for the room) most ticks
/// drip, each with 5 draws; the others make 2.
#[test]
fn a_ceiling_drip_draws_two_or_five() {
    let g = game(&HALL, (3, 1));
    let room = g.world().realized(0).expect("realized");
    assert_eq!(room.settings.ceiling_drips, 0.5);
    let (mut dripped, mut dry) = (0, 0);
    for seed in 0..40 {
        let (expect, want) = expected(seed, 0.0);
        let mut rng = UnityRandom::new(seed);
        let drip = room.ambient_ceiling_drip(true, false, 0.0, &mut rng);
        assert_eq!(rng, expect, "seed {seed}");
        assert_eq!(drip, want, "seed {seed}");
        if drip.is_some() {
            dripped += 1;
        } else {
            dry += 1;
        }
    }
    assert!(dripped > 0 && dry > 0, "{dripped} / {dry}");
}

/// After the start-up only `CeilingDrips ^ 7` counts: `0.5^7 * 0.05` a tile, about one
/// drip in 92 ticks here; still two draws a tick.
#[test]
fn after_the_start_up_drips_are_rare() {
    let g = game(&HALL, (3, 1));
    let room = g.world().realized(0).expect("realized");
    let mut drips = 0;
    for seed in 0..200 {
        let (expect, want) = expected(seed, 1.0);
        let mut rng = UnityRandom::new(seed);
        let drip = room.ambient_ceiling_drip(true, false, 1.0, &mut rng);
        assert_eq!(rng, expect, "seed {seed}");
        assert_eq!(drip, want, "seed {seed}");
        drips += usize::from(drip.is_some());
    }
    assert!(drips < 10, "{drips}");
}

/// Unviewed, a shelter, a `None` danger type or no ceiling tiles: no draw at all.
#[test]
fn no_draw_where_it_cannot_drip() {
    let mut g = game(&HALL, (3, 1));
    let mut rng = UnityRandom::new(3);
    let before = rng.clone();
    let room = g.world().realized(0).expect("realized");
    assert!(
        room.ambient_ceiling_drip(false, false, 0.0, &mut rng)
            .is_none()
    );
    assert!(
        room.ambient_ceiling_drip(true, true, 0.0, &mut rng)
            .is_none()
    );
    let room = g.world_mut().realized_mut(0).expect("realized");
    room.settings.danger_type = Some(DangerType::None);
    assert!(
        room.ambient_ceiling_drip(true, false, 0.0, &mut rng)
            .is_none()
    );
    room.settings.danger_type = Some(DangerType::Flood);
    room.ceiling_tiles.clear();
    assert!(
        room.ambient_ceiling_drip(true, false, 0.0, &mut rng)
            .is_none()
    );
    assert_eq!(rng, before);
}

/// In play the viewed room gets drips falling from its ceiling. A new game's first
/// cycle has no start-up (`CycleStartUp` 1), so `Room.Loaded` added no `DrippingSound`;
/// at `CeilingDrips` 1 each ceiling tile drips at 5 % a tick.
#[test]
fn the_viewed_room_drips() {
    let mut g = game(&HALL, (3, 1));
    assert_eq!(g.world().rain_cycle.cycle_start_up(), 1.0);
    let room = g.world_mut().realized_mut(0).expect("realized");
    assert!(room.dripping_sound.is_none());
    room.settings.ceiling_drips = 1.0;
    let mut seen = 0;
    for _ in 0..40 {
        g.tick(Input::NONE, &mut ());
        let room = g.world().realized(0).expect("realized");
        seen += room
            .drips
            .iter()
            .filter(|d| d.pos.y > 3.0 * 20.0 && d.vel.x == 0.0)
            .count();
    }
    assert!(seen > 0);
}

/// `Room.Loaded` during a start-up adds the `DrippingSound` at the end of the update
/// list; once `CycleStartUp` reaches 1 it is destroyed and, its emitter gone, leaves.
#[test]
fn loaded_adds_the_dripping_sound_during_the_start_up() {
    let mut g = game(&HALL, (3, 1));
    g.world_mut().rain_cycle.start_up_ticks = 2400;
    g.world_mut().rain_cycle.timer = 2390;
    let before = g.world().update_seq();
    g.add_dripping_sound(0);
    let sound = g
        .world()
        .realized(0)
        .and_then(|r| r.dripping_sound.as_ref());
    assert_eq!(sound.map(|s| s.update_order), Some(before + 1));
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    assert!(g.world().rain_cycle.cycle_start_up() >= 1.0);
    assert!(
        g.world()
            .realized(0)
            .is_some_and(|r| r.dripping_sound.is_none())
    );
}
