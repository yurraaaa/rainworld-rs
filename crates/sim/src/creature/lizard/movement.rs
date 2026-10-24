//! A lizard's movement state: `Lizard.MovementAnimation` (the scripted reaches and the
//! turn) and the body numbers `Act` steers by (`BodyForce`, `BodyDesperation`,
//! `BodyWiggleFac`, `BodyDirection`, the legs' grip). Decompile `Lizard.MovementAnimation`,
//! `Lizard.BodyStiff` and `Lizard.Collide`.
//!
//! Mirrors: Lizard.cs (MovementAnimation)

use rw_random::UnityRandom;

use super::Lizard;
use crate::world::{MovementConnection, MovementType};
use crate::{FloatExt, Vec2};

/// `MovementAnimation.BodyPartDestination`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyPartDestination {
    pub body_part: usize,
    pub destination: Vec2,
    pub finished_dist: f32,
    /// The distance when it was made plus 10: past it the animation gives up.
    pub max_distance: f32,
}

impl BodyPartDestination {
    fn new(chunks: &[Vec2; 3], body_part: usize, destination: Vec2, finished_dist: f32) -> Self {
        Self {
            body_part,
            destination,
            finished_dist,
            max_distance: chunks[body_part].distance(destination) + 10.0,
        }
    }
}

/// `Lizard.MovementAnimation`: chunks pulled to tile middles in one or two steps.
#[derive(Debug, Clone, PartialEq)]
pub struct MovementAnimation {
    pub connection: MovementConnection,
    pub step: i32,
    pub total_steps: i32,
    pub destinations: Vec<BodyPartDestination>,
    pub friction: f32,
    pub speed: f32,
}

/// What a movement animation's update ended in.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationOutcome {
    Running,
    /// The next step's animation replaces it.
    NextStep(MovementAnimation),
    /// `MovementAnimationEnded`.
    Ended,
}

impl MovementAnimation {
    /// The constructor for `connection`'s type at `step` (1 or 2): Standard, the reaches
    /// over a gap, up and down, the semi-diagonal and double reach, the turn and the
    /// drop to a pole. Any other type has no destinations.
    #[must_use]
    pub fn new(
        chunks: [Vec2; 3],
        connection: MovementConnection,
        step: i32,
        friction: f32,
        speed: f32,
    ) -> Self {
        let start = Vec2::middle_of_tile(connection.start_tile());
        let dest = Vec2::middle_of_tile(connection.dest_tile());
        let (s, d) = (connection.start_tile(), connection.dest_tile());
        let above_start = Vec2::middle_of_tile((s.0, s.1 + 1));
        let above_dest = Vec2::middle_of_tile((d.0, d.1 + 1));
        let c = &chunks;
        let (total_steps, destinations) = match connection.kind {
            MovementType::Standard => (
                1,
                vec![
                    BodyPartDestination::new(c, 0, dest, 5.0),
                    BodyPartDestination::new(c, 2, start, 5.0),
                ],
            ),
            MovementType::ReachOverGap | MovementType::ReachUp | MovementType::ReachDown => {
                let mid = (start + dest) * 0.5;
                if step == 1 {
                    (2, vec![BodyPartDestination::new(c, 0, mid, 10.0)])
                } else {
                    (
                        2,
                        vec![
                            BodyPartDestination::new(c, 0, dest, 5.0),
                            BodyPartDestination::new(c, 1, mid, 100.0),
                            BodyPartDestination::new(c, 2, start, 12.0),
                        ],
                    )
                }
            }
            MovementType::SemiDiagonalReach => (
                1,
                vec![
                    BodyPartDestination::new(c, 0, dest, 5.0),
                    BodyPartDestination::new(c, 2, start, 15.0),
                ],
            ),
            MovementType::DoubleReachUp => (
                1,
                vec![
                    BodyPartDestination::new(c, 0, dest, 5.0),
                    BodyPartDestination::new(c, 2, dest.lerp(start, 0.5), 15.0),
                ],
            ),
            MovementType::LizardTurn => {
                if step == 1 {
                    (
                        2,
                        vec![
                            BodyPartDestination::new(c, 0, start, 8.0),
                            BodyPartDestination::new(c, 1, above_start, 9.0),
                            BodyPartDestination::new(c, 2, above_dest, 10.0),
                        ],
                    )
                } else {
                    (
                        2,
                        vec![
                            BodyPartDestination::new(c, 0, dest, 5.0),
                            BodyPartDestination::new(c, 1, start, 5.0),
                            BodyPartDestination::new(c, 2, above_start, 8.0),
                        ],
                    )
                }
            }
            MovementType::DropToClimb => (
                1,
                vec![
                    BodyPartDestination::new(c, 0, dest + Vec2::new(0.0, 5.0), 5.0),
                    BodyPartDestination::new(c, 1, start, 12.0),
                ],
            ),
            _ => (0, Vec::new()),
        };
        Self {
            connection,
            step,
            total_steps,
            destinations,
            friction,
            speed,
        }
    }
}

impl Lizard {
    /// `BodyForce`: desperation's push, 1 to the breed's muscle power.
    #[must_use]
    pub fn body_force(&self) -> f32 {
        (self.desperation_smoother * 0.025).clamp(1.0, self.params.max_muscle_power)
    }

    /// `BodyDesperation`.
    #[must_use]
    pub fn body_desperation(&self) -> f32 {
        self.desperation_smoother.inverse_lerp(120.0, 400.0)
    }

    /// `BodyWiggleFac`.
    #[must_use]
    pub fn body_wiggle_fac(&self) -> f32 {
        let delay = self.params.wiggle_delay as f32;
        ((self.body_wiggle_counter as f32 - delay) / (50.0 + delay)).clamp(0.0, 1.0)
    }

    /// `bodyWiggleCounter`'s setter: 0 to 100.
    pub fn set_body_wiggle_counter(&mut self, v: i32) {
        self.body_wiggle_counter = v.clamp(0, 100);
    }

    /// `BodyDirection`: the head's direction from the middle chunk as an index into
    /// `Custom.fourDirections`.
    #[must_use]
    pub fn body_direction(&self) -> usize {
        let aim = (self.body.chunks[0].pos - self.body.chunks[1].pos).to_degrees();
        let i = ((-aim + 180.0) / 90.0).round_ties_even() as i32 + 1;
        usize::try_from(i.rem_euclid(4)).unwrap_or(0)
    }

    /// `LegsGripping`: the graphics' legs that hold on, 2 without a graphics module.
    #[must_use]
    pub fn legs_gripping(&self) -> i32 {
        self.graphics.as_ref().map_or(2, |g| g.legs_grabbing)
    }

    /// `NoGripCounter`: the graphics', 0 without them.
    #[must_use]
    pub fn no_grip_counter(&self) -> i32 {
        self.graphics.as_ref().map_or(0, |g| g.no_grip_counter)
    }

    /// `MovementAnimation.Update(speedFac)`: each destination's chunk is slowed by the
    /// friction and pulled toward it (the head by `BodyForce`, all by
    /// `runSpeed^0.2`, desperation adding a random share and a random shove: three
    /// draws per destination); one past its give-up distance ends the animation
    /// (later destinations are left alone). All within their finish distances goes to
    /// the next step, or ends it.
    pub fn movement_animation_update(
        &mut self,
        speed_fac: f32,
        run_speed: f32,
        rng: &mut UnityRandom,
    ) -> AnimationOutcome {
        let Some(anim) = self.movement_animation.clone() else {
            return AnimationOutcome::Ended;
        };
        let mut finished = true;
        let mut gave_up = false;
        let desperation = self.body_desperation() * self.params.max_muscle_power * 2.0;
        let force = self.body_force();
        for d in &anim.destinations {
            let chunk = &mut self.body.chunks[d.body_part];
            if finished && !chunk.pos.dist_less(d.destination, d.finished_dist) {
                finished = false;
            }
            chunk.vel *= anim.friction;
            let head = if d.body_part == 0 { force } else { 1.0 };
            let mut pull = anim.speed * head * run_speed.mathf_pow(0.2);
            if chunk.pos.dist_less(d.destination, pull) {
                pull = chunk.pos.distance(d.destination);
            }
            pull *= desperation * rng.value() + 1.0;
            chunk.vel += chunk.pos.direction_to(d.destination) * (pull * speed_fac);
            let shove = Vec2::from_degrees(rng.value() * 360.0);
            chunk.vel += shove * (desperation * rng.value() * anim.speed);
            if !chunk.pos.dist_less(d.destination, d.max_distance) {
                finished = true;
                gave_up = true;
                break;
            }
        }
        if !finished {
            return AnimationOutcome::Running;
        }
        if anim.step < anim.total_steps && !gave_up {
            let chunks = [
                self.body.chunks[0].pos,
                self.body.chunks[1].pos,
                self.body.chunks[2].pos,
            ];
            return AnimationOutcome::NextStep(MovementAnimation::new(
                chunks,
                anim.connection,
                anim.step + 1,
                anim.friction,
                anim.speed,
            ));
        }
        AnimationOutcome::Ended
    }
}
