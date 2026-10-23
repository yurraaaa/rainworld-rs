//! The `WaterDrip`s the slugcat's own moves shed: the skid in `Stand`
//! (`UpdateBodyMode`), the shaft shoot and the tunnel slide in `CorridorClimb`. Each
//! drip is gated by `Random.value < InverseLerp(0, 0.5, CeilingDrips)`, drawn even in
//! a dry room. They wait in the sound queue so their draws keep their place among the
//! tick's sounds and noises.
//!
//! Mirrors: Player.cs (UpdateBodyMode)

use rw_random::UnityRandom;

use crate::float_ext::FloatExt;
use crate::{Vec2, WaterDrip};

/// One drip site's drips, positions fixed where the game computes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DripBurst {
    /// The skid's drip at slide counter 4, 7 or 11, from under the hips: one gated drip
    /// thrown 30-70 degrees off vertical toward `slide_direction`, speed 6-11.
    Skid { pos: Vec2, slide_direction: i32 },
    /// A successful shaft shoot: four gated drips from just under the hips' tile
    /// middle, straight up at 5 with an x speed of -3..3.
    ShootUp { pos: Vec2 },
    /// A successful tunnel slide: four gated drips from 9 px behind the hips' tile
    /// middle, at 5 the way it faces with a y speed of -3..3.
    SideShoot { pos: Vec2, flip: i32 },
    /// The zero-gravity kick off the back of a corridor: four gated drips thrown 5 px
    /// along `dir` plus a random direction at 3.
    ZeroGKick { pos: Vec2, dir: Vec2 },
}

impl DripBurst {
    /// Makes the drips with their draws, in the room with `ceiling_drips`.
    pub(super) fn make(self, ceiling_drips: f32, rng: &mut UnityRandom, out: &mut Vec<WaterDrip>) {
        let chance = ceiling_drips.inverse_lerp(0.0, 0.5);
        let tries = if matches!(self, Self::Skid { .. }) {
            1
        } else {
            4
        };
        for _ in 0..tries {
            if rng.value() < chance {
                out.push(self.drip(rng));
            }
        }
    }

    fn drip(self, rng: &mut UnityRandom) -> WaterDrip {
        let (pos, vel) = match self {
            Self::Skid {
                pos,
                slide_direction,
            } => {
                let deg = slide_direction as f32 * 30.0_f32.lerp(70.0, rng.value());
                (
                    pos,
                    Vec2::from_degrees(deg) * 6.0_f32.lerp(11.0, rng.value()),
                )
            }
            Self::ShootUp { pos } => (pos, Vec2::new((-3.0_f32).lerp(3.0, rng.value()), 5.0)),
            Self::SideShoot { pos, flip } => (
                pos,
                Vec2::new(flip as f32 * 5.0, (-3.0_f32).lerp(3.0, rng.value())),
            ),
            Self::ZeroGKick { pos, dir } => (pos, dir * 5.0 + Vec2::random_unit(rng) * 3.0),
        };
        WaterDrip::new(pos, vel, false, rng)
    }
}
