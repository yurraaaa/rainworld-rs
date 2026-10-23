//! `RoomBorderPushBack` (the `BorderPushBack` room effect, 20 Sunlit Lands rooms from
//! `sl_settingstemplate_fog.txt`): a slugcat past the outermost cameras' span by more than
//! the margin is pushed back sideways, harder the longer it stays out. The game's side
//! (the player's chunks and what it holds) is `effects/gravity/border_push_back_turn.rs`.
//!
//! Mirrors: RoomBorderPushBack.cs

use crate::{FloatExt, Vec2};

/// A `RoomBorderPushBack` in a realized room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderPushBack {
    pub update_order: u64,
    pub margin: f32,
    pub leftmost_camera_pos: f32,
    pub rightmost_camera_pos: f32,
    /// Per player.
    pub push_power: Vec<f32>,
}

impl BorderPushBack {
    /// The constructor: `margin = (options.ScreenSize.x - 1400) / 2 - 100` for the chosen
    /// screen width and the extremes of the room's camera positions.
    #[must_use]
    pub fn new(
        update_order: u64,
        camera_positions: &[[f32; 2]],
        players: usize,
        screen_width: f32,
    ) -> Self {
        let margin = (screen_width - 1400.0) / 2.0 - 100.0;
        let mut leftmost = f32::MAX;
        let mut rightmost = f32::MIN;
        for p in camera_positions {
            leftmost = leftmost.min(p[0]);
            rightmost = rightmost.max(p[0]);
        }
        Self {
            update_order,
            margin,
            leftmost_camera_pos: leftmost,
            rightmost_camera_pos: rightmost,
            push_power: vec![0.0; players],
        }
    }

    /// The push `Update` gives the player `i` at `x` (its main chunk), if it is out of
    /// bounds, and the new `pushPower` (`outside` tracks whether it was out). `None` when
    /// the player is not there to test (not in the room, or held).
    pub fn update_player(&mut self, i: usize, testable: Option<f32>) -> Option<f32> {
        let mut push = None;
        let mut outside = false;
        if let Some(x) = testable {
            let power = self.push_power[i].inverse_lerp(0.3, 1.0);
            if x < self.leftmost_camera_pos - self.margin {
                push = Some(3.0 * power);
                outside = true;
            } else if x > self.rightmost_camera_pos + 1400.0 + self.margin {
                push = Some(-3.0 * power);
                outside = true;
            }
        }
        let (to, tick) = if outside {
            (1.0, 1.0 / 600.0)
        } else {
            (0.0, 1.0 / 80.0)
        };
        self.push_power[i] = self.push_power[i].lerp_and_tick(to, 0.01, tick);
        push
    }

    /// `pushPower` grown to `players` entries.
    pub fn grow_to(&mut self, players: usize) {
        if self.push_power.len() < players {
            self.push_power = vec![0.0; players];
        }
    }

    /// The span a player's main chunk may be in without a push: left and right bounds.
    #[must_use]
    pub fn bounds(&self) -> Vec2 {
        Vec2::new(
            self.leftmost_camera_pos - self.margin,
            self.rightmost_camera_pos + 1400.0 + self.margin,
        )
    }
}

#[cfg(test)]
#[path = "tests/border_push_back.rs"]
mod tests;
