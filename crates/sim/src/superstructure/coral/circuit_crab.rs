//! `CircuitCrab` and its `CrabLeg`s: a small creature on the circuit's bits that walks from
//! bit to bit (a hop of under 50 px, now and then) with four to seven legs that reach for
//! spots on the bit under it. Cosmetic only; no one touches it. The sprites are
//! `rw-graphics`' `superstructure/coral/circuit.rs`.
//!
//! Mirrors: CoralBrain/CircuitCrab.cs (constructor, Update, OnCircuitPos, ConnectionPos), CoralBrain/CrabLeg.cs (constructor, Update)

use rw_random::UnityRandom;

use crate::superstructure::coral::circuit::{CircuitBit, CircuitCx};
use crate::{FloatExt, Vec2};

/// One `CrabLeg`: a short chain of `[pos, last pos, vel]`.
#[derive(Debug, Clone, PartialEq)]
pub struct CrabLeg {
    pub points: Vec<[Vec2; 3]>,
    pub con_rad: f32,
}

impl CrabLeg {
    /// The constructor: `IntClamp((int)(length / 10), 1, 100)` points a unit from `init`
    /// (two draws each: position, then velocity).
    fn new(length: f32, init: Vec2, rng: &mut UnityRandom) -> Self {
        let count = ((length / 10.0) as i32).clamp(1, 100);
        let points = (0..count)
            .map(|_| {
                let pos = init + Vec2::random_unit(rng);
                [pos, pos, Vec2::random_unit(rng)]
            })
            .collect();
        Self {
            points,
            con_rad: length / count as f32,
        }
    }

    /// `Update`: drift, then the chain's constraints, then the root at `root`.
    fn update(&mut self, root: Vec2) {
        for p in &mut self.points {
            p[1] = p[0];
            p[0] += p[2];
            p[2] *= 0.999;
        }
        for j in 1..self.points.len() {
            let along = (self.points[j][0] - self.points[j - 1][0]).normalized();
            let gap = self.points[j][0].distance(self.points[j - 1][0]);
            let push = along * ((self.con_rad - gap) * 0.5);
            self.points[j][0] += push;
            self.points[j][2] += push;
            self.points[j - 1][0] -= push;
            self.points[j - 1][2] -= push;
            if j > 1 {
                let spread = (self.points[j][0] - self.points[j - 2][0]).normalized();
                self.points[j][2] += spread * 0.2;
                self.points[j - 2][2] -= spread * 0.2;
            }
        }
        self.points[0][0] = root;
        self.points[0][2] *= 0.0;
    }
}

/// A `CircuitCrab` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitCrab {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub c_pos: Vec2,
    pub next_c_pos: Vec2,
    pub prog: f32,
    pub last_prog: f32,
    pub move_speed: f32,
    /// The bit it stands on and the bit it walks to, as indices into the circuit's `bits`.
    pub bit: usize,
    pub next_bit: usize,
    pub legs: Vec<CrabLeg>,
    pub leg_grab_positions: Vec<Vec2>,
    pub leg_bits: Vec<Option<usize>>,
}

impl CircuitCrab {
    /// The constructor (and `room.AddObject`): the place on the bit (two draws), the number
    /// of legs (`Range(4, 8)`), each leg's points.
    pub fn new(
        bit: usize,
        bits: &[CircuitBit],
        cx: &mut CircuitCx<'_>,
        rng: &mut UnityRandom,
    ) -> Self {
        let size = bits[bit].size;
        let x = rng.value() * size.x;
        let y = rng.value() * size.y;
        let c_pos = Vec2::new(x, y) - size * 0.5;
        let count = usize::try_from(rng.range_i32(4, 8)).unwrap_or(4);
        let at = bits[bit].on_circuit_pos(c_pos, 1.0);
        let legs: Vec<CrabLeg> = (0..count).map(|_| CrabLeg::new(30.0, at, rng)).collect();
        Self {
            update_order: cx.next_seq(),
            c_pos,
            next_c_pos: c_pos,
            prog: 1.0,
            last_prog: 1.0,
            move_speed: 0.0,
            bit,
            next_bit: bit,
            leg_grab_positions: vec![Vec2::ZERO; count],
            leg_bits: vec![None; count],
            legs,
        }
    }

    /// `ConnectionPos(index, 1)`: where the crab is, between its two spots by `prog`.
    #[must_use]
    pub fn body_pos(&self, bits: &[CircuitBit], time_stacker: f32) -> Vec2 {
        let from = bits[self.bit].on_circuit_pos(self.c_pos, time_stacker);
        let to = bits[self.next_bit].on_circuit_pos(self.next_c_pos, time_stacker);
        from.lerp(to, self.last_prog.lerp(self.prog, time_stacker))
    }

    /// `Update`: at rest it may start a hop (about 1 tick in 20; half of them to a neighbour
    /// bit), else it walks on; each leg updates, spokes out of the body, and may reach for
    /// a new spot or let go.
    pub fn update(&mut self, bits: &[CircuitBit], rng: &mut UnityRandom) {
        self.last_prog = self.prog;
        if self.prog == 1.0 {
            if rng.value() < 0.05 {
                self.start_hop(bits, rng);
            }
        } else {
            self.prog = (self.prog + self.move_speed).min(1.0);
            if self.prog == 1.0 {
                self.bit = self.next_bit;
                self.c_pos = self.next_c_pos;
            }
        }
        let body = bits[self.bit].on_circuit_pos(self.c_pos, 1.0).lerp(
            bits[self.next_bit].on_circuit_pos(self.next_c_pos, 1.0),
            self.prog.clamp(0.0, 1.0),
        );
        let count = self.legs.len();
        for i in 0..count {
            let leg = &mut self.legs[i];
            leg.update(body);
            leg.points[0][0] = body;
            leg.points[0][2] = Vec2::from_degrees(i as f32 / count as f32 * 360.0);
            let spoke = leg.points[0][2];
            leg.points[1][2] += spoke * 0.1;
            if rng.value() < 0.025 {
                let size = bits[self.next_bit].size;
                let x = size.x * rng.value();
                let y = size.y * rng.value();
                self.leg_grab_positions[i] = Vec2::new(x, y);
                self.leg_bits[i] = Some(self.next_bit);
            } else if rng.value() < 1.0 / 60.0 {
                self.leg_bits[i] = None;
            }
            if let Some(held) = self.leg_bits[i] {
                let goal = bits[held].on_circuit_pos(self.leg_grab_positions[i], 1.0);
                let tip = leg.points.len() - 1;
                leg.points[tip][2] *= 0.8;
                let pull = leg.points[tip][0].direction_to(goal);
                leg.points[tip][2] += pull;
                if leg.points[tip][0].dist_less(goal, 5.0) {
                    leg.points[tip][2] *= 0.0;
                    leg.points[tip][0] = goal;
                }
            }
        }
    }

    /// A hop: to a neighbour bit (half the time, if it has any) or the same bit, to a random
    /// spot of it within 50 px.
    fn start_hop(&mut self, bits: &[CircuitBit], rng: &mut UnityRandom) {
        let here = &bits[self.bit];
        let target = if rng.value() >= 0.5 || here.neighbors.is_empty() {
            self.bit
        } else {
            let pick = rng.range_i32(0, i32::try_from(here.neighbors.len()).unwrap_or(0));
            here.neighbors[usize::try_from(pick).unwrap_or(0)]
        };
        let size = bits[target].size;
        let x = size.x * rng.value();
        let y = size.y * rng.value();
        let rel = Vec2::new(x, y);
        let from = here.on_circuit_pos(self.c_pos, 1.0);
        let to = bits[target].on_circuit_pos(rel, 1.0);
        if from.distance(to) < 50.0 {
            self.next_c_pos = rel;
            self.next_bit = target;
            self.prog = 0.0;
            self.last_prog = 0.0;
            self.move_speed = 0.5 / from.distance(to);
        }
    }
}
