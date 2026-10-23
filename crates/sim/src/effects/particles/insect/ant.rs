//! `Ant`: runs and stops in turns near where it spawned, drawn at a depth in the
//! foreground.
//!
//! Mirrors: Ant.cs

use super::{InsectBase, InsectBehavior, InsectCx};
use crate::{FloatExt, Vec2};

/// `Ant`.
#[derive(Debug, Clone, PartialEq)]
pub struct Ant {
    pub dir: Vec2,
    pub last_dir: Vec2,
    /// `dp`, its depth: 11/30 or 0.7, else 0 (the game's constructor computes a third
    /// depth and drops it).
    pub depth: f32,
    run_dir: Vec2,
    spawn_pos: Vec2,
    running: bool,
    run_counter: i32,
}

impl Ant {
    /// The constructor after the base's (two draws).
    pub fn new(pos: Vec2, cx: &mut InsectCx) -> Self {
        let depth = if cx.value() < 0.65 {
            if cx.value() < 0.5 { 11.0 / 30.0 } else { 0.7 }
        } else {
            // `Mathf.Lerp(1/30, 29/30, Random.value)`, result unused.
            let _ = cx.value();
            0.0
        };
        Self {
            dir: Vec2::ZERO,
            last_dir: Vec2::ZERO,
            depth,
            run_dir: Vec2::ZERO,
            spawn_pos: pos,
            running: false,
            run_counter: 0,
        }
    }

    /// `Update`.
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        self.last_dir = self.run_dir;
        b.vel *= 0.8;
        self.base_update(b, cx);
    }
}

impl InsectBehavior for Ant {
    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
        if self.running {
            b.vel += self.run_dir * 0.4;
            if b.want_to_burrow {
                self.run_counter = 100;
                self.run_dir = self.run_dir.slerp(Vec2::new(0.0, -1.0), 0.05 * cx.value());
                if b.pos.x < 0.0 {
                    b.destroy();
                }
            } else {
                let r = cx.rnv();
                self.run_dir += r * (cx.value() * 0.9);
                let home = b.pos.distance(self.spawn_pos).inverse_lerp(300.0, 400.0);
                self.run_dir = self.run_dir.lerp(b.pos.direction_to(self.spawn_pos), home);
                self.run_dir = self.run_dir.normalized();
            }
            self.dir = b.last_pos.direction_to(b.pos);
        }
        self.run_counter -= 1;
        if self.run_counter < 1 {
            self.running = !self.running;
            self.run_counter = cx.rng.range_i32(5, if self.running { 200 } else { 80 });
        }
    }

    fn emerge_from_ground(&mut self, b: &mut InsectBase, pos: Vec2, _cx: &mut InsectCx) {
        b.pos = pos + Vec2::new(0.0, 3.0);
    }
}
