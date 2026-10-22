//! Water, swimming, breath and drowning. Expected values are derived by hand from
//! `rainworld-re/notes/water.md` sections 3-5 (with `setupValues.lungs` = 128, see the
//! note corrections), derivation next to each assertion. The pool's surface is flat at
//! 410 px (water row 20) until the water is updated.

use crate::Sounds;
use rw_assets::{Room, RoomSettings};

use super::{assert_close, assert_vec, test_rng};
use crate::player::swim::DROWN_THRESHOLD;
use crate::player::{Animation, BodyMode, HEAD, HIPS, Input, Player};
use crate::tests::room_with_water;
use crate::{FloatExt, Vec2, Water};
use rw_random::UnityRandom;

const UP: Input = Input::new(0, 1, false);
const DOWN: Input = Input::new(0, -1, false);
const RIGHT: Input = Input::new(1, 0, false);
const JUMP: Input = Input::new(0, 0, true);

/// Surface height of the pool.
const LEVEL: f32 = 410.0;
/// Air drained per tick under water: `1 / (40 * 9 * 1.28)`.
const DRAIN: f32 = 1.0 / 460.8;

/// 30 x 30 open tiles over two Solid rows, water up to row 20; column 5 has a
/// vertical pole.
fn pool() -> (Room, Water) {
    let mut rows = vec!["....|.........................".to_string(); 28];
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let room = room_with_water(&refs, 20);
    let water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    (room, water)
}

impl Player {
    /// At rest with the head at `head` and the hips at `hips`, holding `input`.
    fn swimmer(head: Vec2, hips: Vec2, input: Input) -> Self {
        let mut p = Self::new();
        p.body.chunks[HEAD].reset_at(head);
        p.body.chunks[HIPS].reset_at(hips);
        p.input[0] = input;
        p
    }

    /// Upright at x 300, head `depth` px below the surface.
    fn upright_under(depth: f32, input: Input) -> Self {
        let head = Vec2::new(300.0, LEVEL - depth);
        Self::swimmer(head, head - Vec2::new(0.0, 17.0), input)
    }

    /// Surface-swimming at x 300 with the head `depth` px under.
    fn surface_swimmer_under(depth: f32, input: Input) -> Self {
        Self::upright_under(depth, input).with_mode(BodyMode::Swimming, Animation::SurfaceSwim)
    }

    /// Surface-swimming in the pole column, head `depth` px under, holding up.
    fn pole_swimmer_under(depth: f32) -> Self {
        let head = Vec2::new(90.0, LEVEL - depth);
        Self::swimmer(head, head - Vec2::new(0.0, 17.0), UP)
            .with_mode(BodyMode::Swimming, Animation::SurfaceSwim)
    }

    fn with_mode(mut self, mode: BodyMode, animation: Animation) -> Self {
        self.body_mode = mode;
        self.animation = animation;
        self
    }

    fn classified(mut self, room: &Room, water: &Water) -> (BodyMode, Animation) {
        self.classify_swimming(room, Some(water));
        (self.body_mode, self.animation)
    }
}

// ---- entering the swimming modes ----------------------------------------------------

#[test]
fn deep_needs_the_head_30_below_or_10_when_diving() {
    let (r, w) = pool();
    let deep = (BodyMode::Swimming, Animation::DeepSwim);
    let surface = (BodyMode::Swimming, Animation::SurfaceSwim);
    // Hips fully under (> 0.6), head 31 / 29 px below the surface.
    assert_eq!(
        Player::upright_under(31.0, Input::NONE).classified(&r, &w),
        deep
    );
    assert_eq!(
        Player::upright_under(29.0, Input::NONE).classified(&r, &w),
        surface
    );
    // Diving: 10 px is deep enough.
    assert_eq!(Player::upright_under(11.0, DOWN).classified(&r, &w), deep);
    assert_eq!(Player::upright_under(9.0, DOWN).classified(&r, &w), surface);
}

#[test]
fn deep_needs_the_hips_60_percent_under_unless_diving() {
    let (r, w) = pool();
    // Head 40 px down, hips level with the surface + 3: submersion (410 - 405) / 16.
    let head = Vec2::new(300.0, LEVEL - 40.0);
    let hips = Vec2::new(300.0, LEVEL + 3.0);
    let p = Player::swimmer(head, hips, Input::NONE);
    assert_close(p.hips().submersion(Some(&w)), 0.3125, "hips submersion");
    assert_eq!(p.classified(&r, &w).1, Animation::SurfaceSwim);
    let p = Player::swimmer(head, hips, DOWN);
    assert_eq!(p.classified(&r, &w).1, Animation::DeepSwim);
}

#[test]
fn surface_swim_hysteresis_until_80_px_or_diving() {
    let (r, w) = pool();
    for (depth, input, expected) in [
        (50.0, Input::NONE, Animation::SurfaceSwim),
        (79.0, Input::NONE, Animation::SurfaceSwim),
        (81.0, Input::NONE, Animation::DeepSwim),
        (50.0, DOWN, Animation::DeepSwim),
    ] {
        let p = Player::surface_swimmer_under(depth, input);
        assert_eq!(p.classified(&r, &w).1, expected, "depth {depth}");
    }
}

#[test]
fn barely_wet_bodies_and_wall_climbs_are_not_swimming() {
    let (r, w) = pool();
    // Both chunks at most 20% under: the head (bottom 416) is dry, the hips (bottom
    // 408) are (410 - 408) / 16 = 0.125 under.
    let dry = Player::swimmer(
        Vec2::new(300.0, 425.0),
        Vec2::new(300.0, 416.0),
        Input::NONE,
    );
    assert_eq!(dry.classified(&r, &w), (BodyMode::Default, Animation::None));
    // WallClimb is kept unless the head is fully under.
    let p = Player::upright_under(5.0, Input::NONE).with_mode(BodyMode::WallClimb, Animation::None);
    assert_eq!(p.classified(&r, &w).0, BodyMode::WallClimb);
    let p =
        Player::upright_under(40.0, Input::NONE).with_mode(BodyMode::WallClimb, Animation::None);
    assert_eq!(p.classified(&r, &w).0, BodyMode::Swimming);
    // A beam animation keeps the body out of SurfaceSwim.
    let p = Player::upright_under(20.0, Input::NONE)
        .with_mode(BodyMode::ClimbingOnBeam, Animation::ClimbOnBeam);
    assert_eq!(p.classified(&r, &w).1, Animation::ClimbOnBeam);
}

// ---- SurfaceSwim ----------------------------------------------------------------------

#[test]
fn surface_swim_holds_the_head_15_px_up() {
    let (_, w) = pool();
    // vel.y += clamp((head.y - 425) * -0.1, -0.5, 1.5); hips -0.5; both *0.8 first.
    for (head_y, expected) in [(410.0, 1.5), (427.0, -0.2), (440.0, -0.5)] {
        let head = Vec2::new(300.0, head_y);
        let mut p = Player::swimmer(head, head - Vec2::new(17.0, 0.0), Input::NONE)
            .with_mode(BodyMode::Swimming, Animation::SurfaceSwim);
        p.surface_swim(Some(&w));
        assert_close(p.head().vel.y, expected, "head");
        assert_close(p.hips().vel.y, -0.5, "hips");
        assert_eq!(p.dynamic_run_speed, [2.7, 0.0]);
        assert_eq!(p.water_friction, 0.96);
    }
    // Down: head -0.2, hips +0.1. Up while rising fast (3.5 is outside the hold range):
    // +0.5.
    let mut p =
        Player::upright_under(-10.0, DOWN).with_mode(BodyMode::Swimming, Animation::SurfaceSwim);
    p.surface_swim(Some(&w));
    assert_close(p.head().vel.y, -0.2, "down head");
    assert_close(p.hips().vel.y, 0.1, "down hips");
    let mut p =
        Player::upright_under(-10.0, UP).with_mode(BodyMode::Swimming, Animation::SurfaceSwim);
    p.body.chunks[HEAD].vel.y = 3.5;
    p.surface_swim(Some(&w));
    assert_close(p.head().vel.y, 4.0, "up head");
}

#[test]
fn surface_jump_and_its_delay() {
    let (_, w) = pool();
    let mut p =
        Player::upright_under(-10.0, JUMP).with_mode(BodyMode::Swimming, Animation::SurfaceSwim);
    // Both chunks slower than 2 up: +6 each (the hold is skipped while jump is held).
    p.surface_swim(Some(&w));
    assert_close(p.head().vel.y, 6.0, "head");
    assert_close(p.hips().vel.y, 6.0, "hips");
    assert_eq!(p.water_jump_delay, 17);
    // A press during the delay: at least 10, head +4, hips x * 0.75.
    p.input[1] = Input::NONE;
    p.water_jump_delay = 5;
    p.body.chunks[HIPS].vel.x = 2.0;
    p.surface_swim(Some(&w));
    assert_eq!(p.water_jump_delay, 10);
    assert_close(p.head().vel.y, 10.0, "head hop");
    assert_close(p.hips().vel.x, 1.5, "hips x");
    // The delay counts down once per tick in `Player.Update`.
    let (r, w) = pool();
    p.update(
        &r,
        Some(&w),
        Input::NONE,
        &mut test_rng(),
        &mut Sounds::default(),
    );
    assert_eq!(p.water_jump_delay, 9);
}

// ---- DeepSwim -------------------------------------------------------------------------

#[test]
fn deep_swim_boost_costs_air_and_cools_down() {
    let (_, w) = pool();
    let mut p =
        Player::upright_under(100.0, JUMP).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.deep_swim(Some(&w), &mut test_rng(), &mut Sounds::default());
    // Along the body (straight up) * 3; air 1 - 0.2; 20-tick cooldown; cycle 2.7 + 0.01.
    assert_vec(p.head().vel, 0.0, 3.0);
    assert_close(p.air_in_lungs, 0.8, "air");
    assert_eq!(p.water_jump_delay, 20);
    assert_close(p.swim_cycle, 2.71, "cycle");
    // Alignment measured before the boost (both at rest): 0, so friction 0.92.
    assert_close(p.water_friction, 0.92, "friction");
    assert!(p.body.chunks[HEAD].go_through_floors && !p.standing && p.can_jump == 0);
    // Pressed again during the cooldown: the stroke restarts, no push, no air.
    p.input[1] = Input::NONE;
    p.deep_swim(Some(&w), &mut test_rng(), &mut Sounds::default());
    assert_vec(p.head().vel, 0.0, 3.0);
    assert_close(p.air_in_lungs, 0.8, "air kept");
    assert_close(p.swim_cycle, 0.01, "cycle restarted");
    assert_eq!(p.water_jump_delay, 20);
    // Not more than half the air: no boost at all.
    let mut p =
        Player::upright_under(100.0, JUMP).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.air_in_lungs = 0.5;
    p.deep_swim(Some(&w), &mut test_rng(), &mut Sounds::default());
    assert_vec(p.head().vel, 0.0, 0.0);
    assert_eq!(p.water_jump_delay, 0);
}

#[test]
fn deep_swim_stroke_pushes_toward_the_held_direction() {
    let (_, w) = pool();
    let mut p =
        Player::upright_under(100.0, RIGHT).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.deep_swim(Some(&w), &mut test_rng(), &mut Sounds::default());
    // No turn: target 0.2, swim_force lerp(0, 0.2, 0.7) = 0.14. Alignment 0 (at rest).
    // Head += (1, 0) * 0.5 * 0.14 * lerp(0, 1, 0.5) = 0.035; the along-body push is
    // 0 (alignment 0); speed 0.035 < 1.5 adds the full assist 0.2. Hips -0.1 - 0.1.
    assert_close(p.swim_force, 0.14, "force");
    assert_vec(p.head().vel, 0.235, 0.0);
    assert_vec(p.hips().vel, -0.2, 0.0);
    // Cycle: 0.01 + lerp(0.14, 1, 0.5) / 10.
    assert_close(p.swim_cycle, 0.01 + 0.057, "cycle");
    // Short of breath (air 1/6): the direction is bent halfway to straight up.
    let mut p =
        Player::upright_under(100.0, RIGHT).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.air_in_lungs = DROWN_THRESHOLD / 2.0;
    p.deep_swim(Some(&w), &mut test_rng(), &mut Sounds::default());
    let s = std::f32::consts::FRAC_1_SQRT_2;
    assert_vec(p.head().vel, 0.235 * s, 0.235 * s);
    // At the top of the cycle the stroke pulses along the body:
    // 0.7 * lerp(0.14, 1, 0.5) * 1 = 0.399 up.
    let mut p =
        Player::upright_under(100.0, RIGHT).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.swim_cycle = 3.0;
    p.deep_swim(Some(&w), &mut test_rng(), &mut Sounds::default());
    assert_vec(p.head().vel, 0.235, 0.399);
}

#[test]
fn swimming_mode_drops_through_floors() {
    let (r, _) = pool();
    let mut p = Player::upright_under(100.0, Input::NONE)
        .with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.swimming_mode(&r);
    assert!(p.body.chunks.iter().all(|c| c.go_through_floors));
}

#[test]
fn no_pole_grab_mostly_under_water() {
    let (r, w) = pool();
    // Head in the pole column (tile 4) holding up.
    // Submersion 1: no grab.
    let mut p = Player::pole_swimmer_under(60.0);
    p.grab_beam(&r, Some(&w));
    assert_eq!(p.animation, Animation::SurfaceSwim);
    // Head out, hips half in: Submersion (0 + 0.5) / 2 <= 0.9, so it grabs.
    let head = Vec2::new(90.0, LEVEL + 17.0);
    let mut p = Player::swimmer(head, Vec2::new(90.0, LEVEL), UP)
        .with_mode(BodyMode::Swimming, Animation::SurfaceSwim);
    p.grab_beam(&r, Some(&w));
    assert_eq!(p.animation, Animation::ClimbOnBeam);
}

// ---- breath -----------------------------------------------------------------------------

#[test]
fn lungs_drain_under_water_and_refill_above() {
    let (r, w) = pool();
    let mut rng = test_rng();
    let mut p = Player::upright_under(100.0, Input::NONE);
    p.body.chunks[HEAD].vel.x = 4.0;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_close(p.air_in_lungs, 1.0 - DRAIN, "drain");
    // First tick under: swim force from the head's x speed, InverseLerp(0, 8, 4).
    assert_close(p.swim_force, 0.5, "swim force");
    assert!(p.submerged);
    // Exhausted lungs drain twice as fast: 1 / (40 * 4.5 * 1.28).
    p.lungs_exhausted = true;
    let before = p.air_in_lungs;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_close(before - p.air_in_lungs, 1.0 / 230.4, "exhausted drain");
    // Holding straight up short of breath drains 1.5 times slower.
    let mut p = Player::upright_under(100.0, UP);
    p.air_in_lungs = 0.3;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_close(p.air_in_lungs, 0.3 - 1.0 / 691.2, "holding up");

    // Above water: +1/60, snapped to full above 0.9.
    let mut p = Player::upright_under(-40.0, Input::NONE);
    p.air_in_lungs = 0.5;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_close(p.air_in_lungs, 0.5 + 1.0 / 60.0, "refill");
    p.air_in_lungs = 0.91;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_eq!(p.air_in_lungs, 1.0);
    // Surfacing with less than a third: exhausted, +1/240, slowed until full.
    let mut p = Player::upright_under(-40.0, Input::NONE);
    p.submerged = true;
    p.air_in_lungs = 0.2;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert!(p.lungs_exhausted);
    assert_close(p.air_in_lungs, 0.2 + 1.0 / 240.0, "exhausted refill");
    assert_eq!(p.slow_movement_stun, 5);
    // Full again: recovered.
    p.air_in_lungs = 0.999;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert!(!p.lungs_exhausted && p.air_in_lungs == 1.0);
}

#[test]
fn full_lungs_last_461_ticks() {
    let (r, w) = pool();
    let mut p = Player::upright_under(100.0, Input::NONE);
    let mut rng = test_rng();
    // 1 / 460.8 per tick: still breathing after 460 ticks, out on the 461st.
    for _ in 0..460 {
        p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    }
    assert!(p.air_in_lungs > 0.0 && p.stun == 0);
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_eq!(p.air_in_lungs, 0.0);
    assert_eq!(p.stun, 10);
}

#[test]
fn out_of_air_stuns_and_drowns_after_120_steps() {
    let (r, mut w) = pool();
    let mut p = Player::upright_under(200.0, Input::NONE);
    p.air_in_lungs = 0.0;
    p.submerged = true;
    let mut rng = test_rng();
    // Each tick: Stun(10), drown += 1/120. 120 f32 steps of 1/120 sum to just under 1,
    // so `drown >= 1` (death) comes on the 121st.
    for tick in 1..=121 {
        w.update(
            &r,
            crate::SurfaceChunk::player(&mut p.body.chunks),
            &mut rng,
            &mut Sounds::default(),
        );
        p.update(&r, Some(&w), Input::NONE, &mut rng, &mut Sounds::default());
        assert_eq!(p.is_dead(), tick == 121, "tick {tick}");
        if tick < 121 {
            // Stun(10) before `Creature.Update` counts it to 9: no input or movement,
            // but not `Stunned` (>= 10), so the body keeps conscious friction.
            assert_eq!(p.stun_ticks(), 9, "tick {tick}");
            assert!(!p.is_stunned());
            assert_close(p.drown, tick as f32 / 120.0, "drown");
        }
    }
}

#[test]
fn heading_up_to_open_surface_allows_0_3_more() {
    let (r, w) = pool();
    let mut rng = test_rng();
    // Out of air, holding up, 100 px under open water: drain 1 / 691.2 per tick and no
    // drowning until the air reaches -0.3 (207.36 ticks).
    let mut p = Player::upright_under(100.0, UP);
    p.air_in_lungs = 0.0;
    p.submerged = true;
    for _ in 0..207 {
        p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    }
    assert_eq!(p.stun, 0);
    assert_close(p.air_in_lungs, -207.0 / 691.2, "air");
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_eq!(p.stun, 10);
    assert_eq!(p.air_in_lungs, 0.0);
    // More than 200 px down: drowning starts at once.
    let mut p = Player::upright_under(201.0, UP);
    p.air_in_lungs = 0.0;
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert_eq!(p.stun, 10);
}

#[test]
fn a_ceiling_over_the_head_blocks_the_surface_exception() {
    // Row 18 solid in column 15 (above the head, below the surface row 20).
    let mut rows = vec!["..............................".to_string(); 28];
    rows[29 - 18] = format!("{}#{}", ".".repeat(15), ".".repeat(14));
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room_with_water(&refs, 20);
    let w = Water::new(&r, &RoomSettings::ANCESTOR).expect("water");
    assert!(r.tile(15, 18).is_solid());
    let mut p = Player::upright_under(100.0, UP);
    p.air_in_lungs = 0.0;
    p.lung_update(&r, Some(&w), &mut test_rng(), &mut Sounds::default());
    assert_eq!(p.stun, 10);
}

#[test]
fn a_dropped_body_floats_up_and_surface_swims() {
    let (r, mut w) = pool();
    let mut rng = test_rng();
    let mut p = Player::upright_under(-60.0, Input::NONE);
    let mut modes = Vec::new();
    for _ in 0..400 {
        w.update(
            &r,
            crate::SurfaceChunk::player(&mut p.body.chunks),
            &mut rng,
            &mut Sounds::default(),
        );
        p.update(&r, Some(&w), Input::NONE, &mut rng, &mut Sounds::default());
        modes.push(p.animation);
    }
    assert!(modes.contains(&Animation::DeepSwim) || modes.contains(&Animation::SurfaceSwim));
    assert_eq!(p.animation, Animation::SurfaceSwim);
    // Bobbing at the surface: the head stays above it.
    assert!((p.head().pos.y - LEVEL).abs() < 25.0, "{:?}", p.head());
    assert!(!p.is_dead());
}

#[test]
fn out_of_breath_follows_the_air_left() {
    let mut p = Player::new();
    p.air_in_lungs = 0.35;
    p.update_out_of_breath();
    // [0] = InverseLerp(1/3 + 1/6, 0.2, 0.35) = 0.5; [2] += 1 / lerp(40, 4, 0.5).
    let f = p.out_of_breath();
    assert_close(f[0], 0.5, "shortness");
    assert_close(f[1], 0.0, "last shortness");
    assert_close(f[2], 1.0 / 22.0, "phase");
    p.air_in_lungs = 1.0;
    p.update_out_of_breath();
    let f = p.out_of_breath();
    assert_close(f[0], 0.0, "full lungs");
    assert_close(f[1], 0.5, "last shortness");
    assert_close(f[3], 1.0 / 22.0, "last phase");
    assert_close(f[2], 1.0 / 22.0 + 1.0 / 40.0, "phase");
    // Dead: LerpAndTick(0.5, 0, 0.2, 0.05) = 0.4 - 0.05.
    p.out_of_breath[0] = 0.5;
    p.dead = true;
    p.update_out_of_breath();
    assert_close(p.out_of_breath()[0], 0.35, "dead");
}

// ---- bubbles (`LungUpdate`, the deep-swim boost) -----------------------------------------

/// Air crossing two thirds under water: `Bubble(firstChunk.pos, firstChunk.vel)`, its
/// constructor's two draws and nothing else.
#[test]
fn lungs_crossing_two_thirds_let_out_a_bubble() {
    let (r, w) = pool();
    let mut rng = UnityRandom::new(3);
    let mut p = Player::upright_under(100.0, Input::NONE);
    p.body.chunks[HEAD].vel = Vec2::new(1.0, -2.0);
    p.air_in_lungs = 2.0 / 3.0 + DRAIN / 2.0;
    let mut copy = rng.clone();
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    let bubbles = p.take_bubbles();
    assert_eq!(bubbles.len(), 1);
    let (drips_before, b) = &bubbles[0];
    assert_eq!(*drips_before, 0);
    assert_eq!(b.pos, p.head().pos);
    assert_eq!(b.vel, Vec2::new(1.0, -2.0));
    assert_eq!(b.full_size, 0.5.lerp(1.5, copy.value()));
    copy.value();
    assert_eq!(rng.state(), copy.state());
    // The next tick, already under two thirds: nothing.
    p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
    assert!(p.take_bubbles().is_empty());
}

/// Short of breath (air 0.2): the flail roll (`value < 0.5`, then direction and
/// strength), then `(value > air * 2 || lungsExhausted) && value > 0.5` lets out a
/// bubble pushed `DegToVec(value * 360) * Lerp(6, 0, air)` (plus the constructor's two).
#[test]
fn short_of_breath_bubbles_draw_as_the_game() {
    let (r, w) = pool();
    let mut made = 0;
    for seed in 0..40 {
        let mut rng = UnityRandom::new(seed);
        let mut p = Player::upright_under(100.0, UP);
        p.air_in_lungs = 0.2 + DRAIN;
        p.lungs_exhausted = seed % 2 == 0;
        let mut copy = rng.clone();
        p.lung_update(&r, Some(&w), &mut rng, &mut Sounds::default());
        let air = p.air_in_lungs;
        let mut head_vel = Vec2::ZERO;
        if copy.value() < 0.5 {
            let dir = Vec2::from_degrees(copy.value() * 360.0);
            head_vel += dir * copy.value();
        }
        let first = copy.value() > air * 2.0;
        let bubble = (first || p.lungs_exhausted) && copy.value() > 0.5;
        let bubbles = p.take_bubbles();
        if bubble {
            made += 1;
            let push = Vec2::from_degrees(copy.value() * 360.0) * 6.0.lerp(0.0, air);
            copy.value();
            copy.value();
            assert_eq!(bubbles.len(), 1, "seed {seed}");
            assert_eq!(bubbles[0].1.vel, head_vel + push, "seed {seed}");
        } else {
            assert!(bubbles.is_empty(), "seed {seed}");
        }
        assert_eq!(rng.state(), copy.state(), "seed {seed}");
    }
    assert!(made > 0 && made < 40);
}

/// A boost in a viewed room lets out a bubble at the head with its new speed (two
/// draws); in a room nobody views, none and no draws.
#[test]
fn deep_swim_boost_bubble_only_when_viewed() {
    let (_, w) = pool();
    let mut p =
        Player::upright_under(100.0, JUMP).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    let mut rng = UnityRandom::new(1);
    let copy = rng.clone();
    p.deep_swim(Some(&w), &mut rng, &mut Sounds::default());
    let bubbles = p.take_bubbles();
    assert_eq!(bubbles.len(), 1);
    assert_eq!(bubbles[0].1.pos, p.head().pos);
    assert_vec(bubbles[0].1.vel, 0.0, 3.0);
    let mut two = copy.clone();
    two.value();
    two.value();
    assert_eq!(rng.state(), two.state());

    let mut p =
        Player::upright_under(100.0, JUMP).with_mode(BodyMode::Swimming, Animation::DeepSwim);
    p.set_emitter_place(crate::EmitterPlace {
        room: 1,
        camera_room: 0,
        tick: 0,
        update_seq: 0,
    });
    let mut rng = UnityRandom::new(1);
    p.deep_swim(Some(&w), &mut rng, &mut Sounds::default());
    assert!(p.take_bubbles().is_empty());
    assert_eq!(rng.state(), copy.state());
}
