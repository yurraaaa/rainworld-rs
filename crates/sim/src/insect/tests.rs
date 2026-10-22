//! Draw counts and spawn rules of the cosmetic insects, from `InsectCoordinator.cs`,
//! `CosmeticInsect.cs` and each kind's constructor and update.

use rw_assets::{PlacedObject, Room};
use rw_random::UnityRandom;

use super::*;
use crate::tests::room;

const SEED: i32 = 1234;

/// A room and a stream for an insect's context.
struct Bench {
    room: Room,
    rng: UnityRandom,
    time_until_rain: i32,
    cycle_start_up: f32,
}

impl Bench {
    fn new(room: Room) -> Self {
        Self {
            room,
            rng: UnityRandom::new(SEED),
            time_until_rain: 40_000,
            cycle_start_up: 1.0,
        }
    }

    fn cx(&mut self) -> InsectCx<'_> {
        InsectCx {
            room: &self.room,
            ai_map: None,
            water: InsectWater::new(None, &self.room),
            time_until_rain: self.time_until_rain,
            cycle_start_up: self.cycle_start_up,
            viewed: true,
            darkness: 0.0,
            region_si: false,
            creatures: &[],
            rng: &mut self.rng,
        }
    }

    /// The stream's state after `n` draws from the seed.
    fn after(n: usize) -> [u32; 4] {
        let mut r = UnityRandom::new(SEED);
        for _ in 0..n {
            r.value();
        }
        r.state()
    }
}

/// A 10 x 8 room of Air over a Solid floor row.
fn floored() -> Room {
    let mut rows = vec![".........."; 7];
    rows.push("##########");
    room(&rows)
}

/// `CosmeticInsect`: `Range(400, 1600)` and `ClampedRandomVariation` (two draws).
#[test]
fn the_base_constructor_draws_three() {
    let mut b = Bench::new(floored());
    let base = InsectBase::new(Vec2::new(50.0, 90.0), &mut b.cx());
    assert_eq!(b.rng.state(), Bench::after(3));
    assert!(base.burrow_pos.is_none());
    assert!(!base.want_to_burrow);
}

/// Early in the cycle (`CycleStartUp < emergeAfterRainTime`, at least 0.1) an insect
/// starts buried in the middle of the tile it walked down to, 9 px up (the walk stops
/// at row 0 without testing it).
#[test]
fn an_insect_made_at_the_cycle_start_is_buried() {
    let mut b = Bench::new(floored());
    b.cycle_start_up = 0.0;
    let base = InsectBase::new(Vec2::new(50.0, 90.0), &mut b.cx());
    assert_eq!(base.burrow_pos, Some(Vec2::new(50.0, 19.0)));
    assert!(base.want_to_burrow);
    assert_eq!((base.in_ground, base.last_in_ground), (1.0, 1.0));
}

/// Each kind's constructor draws after the base's three.
#[test]
fn constructors_draw_as_the_game() {
    let cases = [
        (InsectType::StandardFly, 3),
        // `col` and `sin`.
        (InsectType::FireFly, 5),
        // `hoverTimer`, the avoider's timer, `hue`, `goalDir`.
        (InsectType::TinyDragonFly, 7),
        // The avoider, `size` (two), `colorFac`.
        (InsectType::RockFlea, 7),
        // The avoider, `colorFac`, `colorFac2`.
        (InsectType::GrassHopper, 6),
        // `hoverTimer`, the avoider, `hue`.
        (InsectType::RedSwarmer, 6),
        // `value < 0.65`, then one more either way.
        (InsectType::Ant, 5),
        // The avoider, `rot`.
        (InsectType::Beetle, 5),
        // `colorFac`, `bigFlyDir`.
        (InsectType::Wasp, 5),
        // The avoider, `rot`, `sitStill`.
        (InsectType::Moth, 6),
    ];
    for (kind, draws) in cases {
        let mut b = Bench::new(floored());
        let insect = CosmeticInsect::new(kind, Vec2::new(50.0, 90.0), &mut b.cx());
        assert_eq!(insect.kind(), kind);
        assert_eq!(b.rng.state(), Bench::after(draws), "{kind:?}");
    }
}

/// The glowworm: the avoider, `breath`, `Range(2, 4)` segments, then `Reset`: the
/// base's two draws and three per segment.
#[test]
fn a_glowworm_draws_for_each_segment() {
    let mut b = Bench::new(floored());
    let insect = CosmeticInsect::new(
        InsectType::WaterGlowworm,
        Vec2::new(50.0, 90.0),
        &mut b.cx(),
    );
    let InsectBody::WaterGlowworm(w) = &insect.body else {
        panic!("not a glowworm");
    };
    let n = w.segments.len();
    assert!((2..4).contains(&n));
    assert_eq!(b.rng.state(), Bench::after(3 + 3 + 2 + 3 * n));
    // `Reset` cleared the burrow even if the base had dug one.
    assert!(insect.base.burrow_pos.is_none());
}

/// A mini fly's tick before the room is ready for AI, with no creatures: the
/// direction's three draws, no tile scores, the 1/120 creature check, and the base's
/// rain check.
#[test]
fn a_mini_fly_tick_without_an_ai_map_draws_five() {
    let mut b = Bench::new(floored());
    let mut insect =
        CosmeticInsect::new(InsectType::StandardFly, Vec2::new(50.0, 90.0), &mut b.cx());
    let before = b.rng.state();
    insect.update(&mut b.cx());
    let mut r = UnityRandom::from_state(before);
    for _ in 0..5 {
        r.value();
    }
    assert_eq!(b.rng.state(), r.state());
    assert!(!insect.base.slated);
}

/// An insect in a room no camera views destroys itself after its update.
#[test]
fn an_unviewed_insect_destroys_itself() {
    let mut b = Bench::new(floored());
    let mut insect = CosmeticInsect::new(InsectType::Wasp, Vec2::new(50.0, 90.0), &mut b.cx());
    let mut cx = b.cx();
    cx.viewed = false;
    insect.update(&mut cx);
    assert!(insect.base.slated);
}

/// `TileLegalForInsect` passed, `Range(1200, 1600)` is drawn and, with the rain near,
/// no insect is made.
#[test]
fn no_insects_near_the_rain() {
    let mut b = Bench::new(floored());
    b.time_until_rain = 1199;
    let mut c = InsectCoordinator {
        effects: vec![(InsectType::Wasp, 1.0)],
        ..InsectCoordinator::default()
    };
    // (10 * 8) * 1 * 0.15 / 50 = 0.24: one attempt; a Wasp needs no chance draw.
    let made = c.now_viewed(&mut b.cx());
    assert_eq!(made, 0);
    let mut r = UnityRandom::new(SEED);
    let pos = Vec2::new(r.value() * 200.0, r.value() * 160.0);
    let solid = b.room.is_solid_at(pos);
    let expected = if solid { 2 } else { 3 };
    assert_eq!(b.rng.state(), Bench::after(expected));
}

/// The `Flies` effect: `(int)(W * H * amount / 50)` + 1 attempts, each a spot (two
/// draws), and on Air the chance draw against terrain proximity 5 (no AI map); a kept
/// spot draws the rain test and makes a mini fly (three).
#[test]
fn the_flies_effect_rolls_its_spots_as_the_game() {
    let mut b = Bench::new(floored());
    let mut c = InsectCoordinator {
        effects: vec![(InsectType::StandardFly, 2.0)],
        ..InsectCoordinator::default()
    };
    let made = c.now_viewed(&mut b.cx());
    // 80 * 2 / 50 = 3.2: four attempts.
    let mut r = UnityRandom::new(SEED);
    let mut flies = 0;
    for _ in 0..4 {
        let pos = Vec2::new(r.value() * 200.0, r.value() * 160.0);
        if b.room.is_solid_at(pos) {
            continue;
        }
        if r.value().powf(0.5) > 5.0 * 0.05 {
            r.range_i32(1200, 1600);
            for _ in 0..3 {
                r.value();
            }
            flies += 1;
        }
    }
    assert_eq!(made, flies);
    assert_eq!(c.insects.len(), flies);
    assert_eq!(b.rng.state(), r.state());
    assert!(
        c.insects
            .iter()
            .all(|i| i.kind() == InsectType::StandardFly)
    );
}

/// An `InsectGroup` of rock fleas (radius 100, density 0.5): `1 + (int)(2pi * 125 *
/// 0.5 / 50)` = 8, so nine attempts of three draws; each on Air draws the rain test and
/// makes a flea brought down onto the floor.
#[test]
fn a_rock_flea_group_lands_its_fleas_on_the_floor() {
    let mut b = Bench::new(floored());
    let mut c = InsectCoordinator {
        groups: vec![InsectGroup {
            center: Vec2::new(100.0, 80.0),
            rad: 100.0,
            kind: Some(InsectType::RockFlea),
            density: 0.5,
        }],
        ..InsectCoordinator::default()
    };
    c.now_viewed(&mut b.cx());
    let mut r = UnityRandom::new(SEED);
    let mut fleas = 0;
    for _ in 0..9 {
        let dir = Vec2::random_unit(&mut r);
        let reach = 100.0 * r.value() + 10.0 * r.value().powf(3.0);
        let pos = Vec2::new(100.0, 80.0) + dir * reach;
        if b.room.is_solid_at(pos) {
            continue;
        }
        r.range_i32(1200, 1600);
        for _ in 0..7 {
            r.value();
        }
        fleas += 1;
    }
    assert_eq!(c.insects.len(), fleas);
    assert_eq!(b.rng.state(), r.state());
    for f in &c.insects {
        assert!(f.base.swarm.is_some());
        // On the floor row's top (row 0's middle + 10) or below the room.
        assert!(
            f.base.pos.y == 20.0 || f.base.pos.y == -10.0,
            "{:?}",
            f.base.pos
        );
    }
}

#[test]
fn insect_group_data_reads_handle_type_and_density() {
    let o = PlacedObject {
        kind: "InsectGroup".to_owned(),
        pos: [300.0, 200.0],
        data: "0~-120~-50~40~Moth~0.25".to_owned(),
        active: true,
    };
    let g = InsectGroup::from_placed(&o);
    assert_eq!(g.center, Vec2::new(300.0, 200.0));
    assert_eq!(g.rad, 120.0);
    assert_eq!(g.kind, Some(InsectType::Moth));
    assert_eq!(g.density, 0.25);
    let o = PlacedObject {
        data: "30~40~0~0~Butterfly~1".to_owned(),
        ..o
    };
    let g = InsectGroup::from_placed(&o);
    assert_eq!((g.rad, g.kind), (50.0, None));
}

#[test]
fn species_densities_and_effect_names() {
    assert_eq!(InsectType::Ant.species_density(), 6.0);
    assert_eq!(InsectType::WaterGlowworm.species_density(), 0.33);
    assert_eq!(
        InsectType::from_value("TinyDragonFly"),
        Some(InsectType::TinyDragonFly)
    );
    assert_eq!(
        InsectType::from_effect(rw_assets::RoomEffectType::Flies),
        Some(InsectType::StandardFly)
    );
    assert_eq!(
        InsectType::from_effect(rw_assets::RoomEffectType::Bloom),
        None
    );
}

/// `Vector3.RotateTowards` from a zero vector moves toward the target by at most the
/// magnitude step; otherwise it turns by at most `maxRadians`.
#[test]
fn rotate_towards_turns_by_at_most_the_step() {
    let goal = Vec2::new(0.0, 1.0);
    assert_eq!(Vec2::ZERO.rotate_towards(goal, 0.4, 1.0), goal);
    let turned = Vec2::new(1.0, 0.0).rotate_towards(goal, 0.4, 1.0);
    assert!((turned.x - 0.4f32.cos()).abs() < 1e-6);
    assert!((turned.y - 0.4f32.sin()).abs() < 1e-6);
}

#[test]
fn cycle_start_up_runs_over_the_start_up_ticks() {
    let mut c = crate::world::RainCycle::default();
    assert_eq!(c.cycle_start_up(), 1.0);
    c.start_up_ticks = 2400;
    c.timer = 600;
    assert_eq!(c.cycle_start_up(), 0.25);
}
