//! `ProjectedCircle`: a spoked ring of the superstructure's hologram drawn behind a coral
//! neuron's free end (and a stem's, a swarmer's later), joined by lines to up to five
//! other circles and a few points off the screen. It blinks, breathes and turns by chance
//! (the draws of `Update`); its depth is read from the camera's picture when it is drawn
//! (`UpdateDepth`, the app's `update_depth`). Only the base game's branches are ported,
//! D40. The sprites are `rw-graphics`' `superstructure/projections/projected_circle.rs`.
//!
//! Mirrors: CoralBrain/ProjectedCircle.cs (constructor, Update, UpdateDepth, Position, Rad), IOwnProjectedCircles.cs

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// The `IOwnProjectedCircles` of a circle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircleOwner {
    /// A `CoralNeuron`, by its place in the update list.
    Neuron { order: u64 },
    /// A `CoralStem`, by its place in its system's list.
    Stem { stem: u32 },
}

/// The `ProjectedCircle` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedCircle {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub owner: CircleOwner,
    pub index: usize,
    pub base_rad: f32,
    pub rad: f32,
    pub last_rad: f32,
    pub get_to_rad: f32,
    pub last_depth_offset: f32,
    pub depth_offset: f32,
    /// `updateDepth`: set by `Update`, cleared by the next draw's `UpdateDepth`.
    pub update_depth: bool,
    pub blink: f32,
    pub last_blink: f32,
    pub blink_get_to: f32,
    pub blink_counter: i32,
    pub rotation: f32,
    pub last_rotation: f32,
    pub rotation_speed: f32,
    pub connections_blink: Vec<i32>,
    /// `connectedCircles`, by their places in the update list.
    pub connected_circles: Vec<u64>,
    pub off_screen_connections: Vec<Vec2>,
    pub spokes: i32,
    pub slated: bool,
}

impl ProjectedCircle {
    /// The constructor (`size` 0 to 1) in `room`, whose update list holds the circles
    /// `existing` (in order) before it joins: each may become a connection.
    pub fn new(
        update_order: u64,
        owner: CircleOwner,
        index: usize,
        size: f32,
        room: &Room,
        existing: &[u64],
        rng: &mut UnityRandom,
    ) -> Self {
        let base_rad = 60.0f32.lerp(180.0, size);
        let spokes = (std::f32::consts::PI * 2.0 * base_rad / 50.0).to_int_cs();
        let mut connected_circles = Vec::new();
        for &other in existing {
            if connected_circles.len() >= 5 {
                break;
            }
            if rng.value() < 0.5 {
                connected_circles.push(other);
            }
        }
        let (height, width) = (room.height() as f32 * 20.0, room.width() as f32 * 20.0);
        let off_screen_connections = if rng.value() < 0.5 {
            Vec::new()
        } else {
            let count = usize::try_from(rng.range_i32(1, 4)).unwrap_or(0);
            let mut points = vec![Vec2::ZERO; count];
            for p in &mut points {
                let tall = rng.value() < height / (height + width);
                let low = rng.value() < 0.5;
                if tall {
                    p.y = height * rng.value();
                    p.x = if low { -10.0 } else { width + 10.0 };
                } else {
                    p.x = width * rng.value();
                    p.y = if low { -10.0 } else { height + 10.0 };
                }
            }
            points
        };
        let connections = connected_circles.len() + off_screen_connections.len();
        let speed = 0.2f32.lerp(1.0, rng.value());
        let rotation_speed = speed * if rng.value() < 0.5 { -1.0 } else { 1.0 };
        Self {
            update_order,
            owner,
            index,
            base_rad,
            rad: base_rad,
            last_rad: base_rad,
            get_to_rad: base_rad,
            last_depth_offset: 0.0,
            depth_offset: 0.0,
            update_depth: false,
            blink: 0.0,
            last_blink: 0.0,
            blink_get_to: 0.0,
            blink_counter: 0,
            rotation: 0.0,
            last_rotation: 0.0,
            rotation_speed,
            connections_blink: vec![0; connections],
            connected_circles,
            off_screen_connections,
            spokes,
            slated: false,
        }
    }

    /// `Update`; the owner's `CanHostCircle()` is `owner_can_host`.
    pub fn update(&mut self, owner_can_host: bool, rng: &mut UnityRandom) {
        self.last_rad = self.rad;
        if self.rad < self.get_to_rad {
            self.rad = (self.rad + 0.8).min(self.get_to_rad);
        } else if self.rad > self.get_to_rad {
            self.rad = (self.rad - 0.8).max(self.get_to_rad);
        }
        self.rad = self.rad.lerp(self.get_to_rad, 0.02);
        if rng.value() < 1.0 / 120.0 {
            self.get_to_rad = (self.base_rad * 0.8).lerp(self.base_rad * 1.2, rng.value());
        }
        self.update_depth = true;
        self.last_depth_offset = self.depth_offset;
        if rng.value() < 0.000_980_392_2 {
            self.blink_counter = rng.range_i32(7, 230);
            self.blink_get_to = 1.0;
        }
        if self.blink_counter > 0 {
            self.blink_counter -= 1;
        } else {
            self.blink_get_to = 0.0;
        }
        self.last_blink = self.blink;
        if self.blink < self.blink_get_to {
            self.blink = (self.blink + 1.0 / 30.0).min(self.blink_get_to);
        } else if self.blink > self.blink_get_to {
            self.blink = (self.blink - 1.0 / 30.0).max(self.blink_get_to);
        }
        self.blink = self.blink.lerp(self.blink_get_to, 0.02);
        for blink in &mut self.connections_blink {
            if *blink > 0 {
                *blink -= 1;
            } else if rng.value() < 0.003_333_333_4 {
                *blink = rng.range_i32(10, 50);
            }
        }
        self.last_rotation = self.rotation;
        self.rotation += self.rotation_speed * 140.0 * self.rotation_speed / self.rad;
        if !owner_can_host {
            self.slated = true;
        }
    }

    /// `UpdateDepth` given the camera's depth at the owner's centre; run by the first draw
    /// after an `Update`.
    pub fn apply_depth(&mut self, depth: f32) {
        self.depth_offset = self.depth_offset.lerp(depth, 0.5);
        self.update_depth = false;
    }

    /// `Rad(timeStacker)`.
    #[must_use]
    pub fn rad_at(&self, time_stacker: f32) -> f32 {
        self.last_rad.lerp(self.rad, time_stacker)
            * 1.0f32.lerp(0.6, self.last_blink.lerp(self.blink, time_stacker))
    }
}

#[cfg(test)]
#[path = "tests/projected_circle.rs"]
mod tests;
