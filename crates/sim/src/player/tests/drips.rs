//! The `WaterDrip`s of the skid (`Player.UpdateBodyMode`) and the corridor shoots: where they are queued,
//! their gate, positions, velocities and draws. Tile
//! `(x, y)` has its middle at `(20 x + 10, 20 y + 10)`; the hips' radius is 8.

use rw_random::UnityRandom;

use super::corridor::{shaft_room, tunnel_room};
use crate::player::drips::DripBurst;
use crate::player::sounds::QueuedSound;
use crate::player::{HEAD, Input, Player};
use crate::{SoundId, Vec2, WaterDrip};

const LEFT: Input = Input::new(-1, 0, false);

/// The drips a burst makes from `seed` in a room with `ceiling_drips`, and the rng after.
fn made(burst: DripBurst, ceiling_drips: f32, seed: i32) -> (Vec<WaterDrip>, UnityRandom) {
    let mut rng = UnityRandom::new(seed);
    let mut out = Vec::new();
    burst.make(ceiling_drips, &mut rng, &mut out);
    (out, rng)
}

fn advanced(seed: i32, draws: usize) -> UnityRandom {
    let mut rng = UnityRandom::new(seed);
    for _ in 0..draws {
        rng.value();
    }
    rng
}

const SKID: DripBurst = DripBurst::Skid {
    pos: Vec2::new(100.0, 41.0),
    slide_direction: -1,
};
const SHOOT_UP: DripBurst = DripBurst::ShootUp {
    pos: Vec2::new(110.0, 41.0),
};
const SIDE: DripBurst = DripBurst::SideShoot {
    pos: Vec2::new(61.0, 110.0),
    flip: 1,
};

/// `CeilingDrips` 0 still draws each gate (`value < InverseLerp(0, 0.5, 0)` = `< 0`):
/// one for the skid, four for a shoot, and makes no drip.
#[test]
fn dry_rooms_draw_the_gates_only() {
    for (burst, gates) in [(SKID, 1), (SHOOT_UP, 4), (SIDE, 4)] {
        let (drips, rng) = made(burst, 0.0, 11);
        assert!(drips.is_empty());
        assert_eq!(rng, advanced(11, gates), "{burst:?}");
    }
}

/// `CeilingDrips` >= 0.5 passes every gate: the skid's drip is gate, angle, speed and
/// the constructor's `Range(10, 120)` (4 draws); each shoot drip gate, the free
/// velocity component, the constructor (3 draws, 12 for the four).
#[test]
fn wet_rooms_make_every_drip_with_its_draws() {
    let (drips, rng) = made(SKID, 0.5, 11);
    let mut e = UnityRandom::new(11);
    e.value();
    let deg = -(30.0 + 40.0 * e.value());
    let vel = Vec2::from_degrees(deg) * (6.0 + 5.0 * e.value());
    let want = WaterDrip::new(Vec2::new(100.0, 41.0), vel, false, &mut e);
    assert_eq!(drips, vec![want]);
    assert_eq!(rng, e);

    let (drips, rng) = made(SHOOT_UP, 0.7, 11);
    let mut e = UnityRandom::new(11);
    let mut want = Vec::new();
    for _ in 0..4 {
        e.value();
        let vel = Vec2::new(-3.0 + 6.0 * e.value(), 5.0);
        want.push(WaterDrip::new(Vec2::new(110.0, 41.0), vel, false, &mut e));
    }
    assert_eq!(drips, want);
    assert_eq!(rng, e);

    let (drips, rng) = made(SIDE, 1.0, 11);
    let mut e = UnityRandom::new(11);
    let mut want = Vec::new();
    for _ in 0..4 {
        e.value();
        let vel = Vec2::new(5.0, -3.0 + 6.0 * e.value());
        want.push(WaterDrip::new(Vec2::new(61.0, 110.0), vel, false, &mut e));
    }
    assert_eq!(drips, want);
    assert_eq!(rng, e);
}

/// `CeilingDrips` 0.25: each drip only when its gate value is below 0.5; the failed
/// gates draw nothing more.
#[test]
fn half_wet_rooms_gate_each_drip() {
    let (drips, rng) = made(SHOOT_UP, 0.25, 4);
    let mut e = UnityRandom::new(4);
    let mut want = Vec::new();
    for _ in 0..4 {
        if e.value() < 0.5 {
            let vel = Vec2::new(-3.0 + 6.0 * e.value(), 5.0);
            want.push(WaterDrip::new(Vec2::new(110.0, 41.0), vel, false, &mut e));
        }
    }
    assert_eq!(drips, want);
    assert_eq!(rng, e);
}

/// The skid queues its drip on slide counters 4, 7 and 11 only, from under the hips
/// (`pos + (0, -rad + 1)`) toward the slide direction.
#[test]
fn skid_sheds_on_counters_4_7_and_11() {
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.slide_direction = 1;
    p.slide_counter = 1;
    p.input[0] = LEFT;
    for c in 2..=20 {
        p.skid();
        let queued = std::mem::take(&mut p.sound_queue);
        if matches!(c, 4 | 7 | 11) {
            let pos = p.body.chunks[crate::player::HIPS].pos + Vec2::new(0.0, -7.0);
            let want = QueuedSound::Drips(DripBurst::Skid {
                pos,
                slide_direction: 1,
            });
            assert_eq!(queued, vec![want], "counter {c}");
        } else {
            assert!(queued.is_empty(), "counter {c}");
        }
    }
}

/// A successful shaft shoot queues its drips from under the hips' tile middle before
/// the success sound; a failed one none.
#[test]
fn shaft_shoot_success_sheds_before_its_sound() {
    let r = shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 67.0), Vec2::new(110.0, 50.0));
    p.input[0] = Input::new(0, 1, true);
    p.corridor_climb_mode(&r, None);
    assert_eq!(
        p.sound_queue,
        vec![
            QueuedSound::Drips(DripBurst::ShootUp {
                pos: Vec2::new(110.0, 41.0)
            }),
            QueuedSound::Sound {
                id: SoundId::SlugcatVerticalChuteJumpSuccess,
                chunk: HEAD,
                volume: 1.0,
                pitch: 1.0,
            },
        ]
    );

    let mut p = Player::in_corridor_at(Vec2::new(110.0, 227.0), Vec2::new(110.0, 210.0));
    p.input[0] = Input::new(0, 0, true);
    p.corridor_climb_mode(&r, None);
    assert!(
        p.sound_queue
            .iter()
            .all(|q| !matches!(q, QueuedSound::Drips(_)))
    );
}

/// A successful tunnel slide queues its drips 9 px behind the hips' tile middle, before
/// the body moves and before the success sound.
#[test]
fn tunnel_slide_success_sheds_behind_the_hips() {
    let r = tunnel_room();
    let mut p = Player::in_corridor_at(Vec2::new(87.0, 112.0), Vec2::new(70.0, 112.0));
    p.input[0] = Input::new(1, 0, true);
    p.corridor_climb_mode(&r, None);
    assert_eq!(
        p.sound_queue[..2],
        [
            QueuedSound::Drips(DripBurst::SideShoot {
                pos: Vec2::new(61.0, 110.0),
                flip: 1,
            }),
            QueuedSound::Sound {
                id: SoundId::SlugcatCorridorHorizontalSlideSuccess,
                chunk: HEAD,
                volume: 1.0,
                pitch: 1.0,
            },
        ]
    );
}

/// Through a whole update the shoot's drips reach the room: four in a wet room.
#[test]
fn shaft_shoot_drips_reach_the_room() {
    let r = shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 67.0), Vec2::new(110.0, 50.0));
    p.set_ceiling_drips(0.5);
    p.step(&r, Input::new(0, 1, true));
    let drips = p.take_drips();
    assert_eq!(drips.len(), 4);
    for d in drips {
        assert_eq!(d.pos, Vec2::new(110.0, 41.0));
        assert_eq!(d.vel.y, 5.0);
        assert!((-3.0..=3.0).contains(&d.vel.x));
    }
}
