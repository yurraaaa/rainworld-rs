//! `InteractiveMenuScene.Update` in flat mode: the depth-of-field focus and the idle
//! camera drift. Nothing in flat mode is drawn with them, but they make the game's
//! random draws every tick (D1), so they run as the game runs them. With no depth
//! illustrations or idle depths the focus picks and the depth under the mouse draw
//! nothing; the gamepad camera input is zero with the keyboard preset.

use rw_sim::{FloatExt, Vec2};

use super::object::{Kind, ObjectId};
use super::{Menu, MenuCx};

impl Menu {
    pub(super) fn update_scene(&mut self, id: ObjectId, cx: &mut MenuCx<'_>) {
        let (mouse, last_mouse) = (self.mouse_pos, self.last_mouse_pos);
        let mouse_mode_selection = cx.shared.mouse_mode && self.selected.is_some();
        let rng = &mut cx.shared.random;
        let Kind::Scene(s) = &mut self.objects[id.0].kind else {
            return;
        };
        s.last_focus = s.focus;
        let num = (((s.mouse_inactive_counter as f32).inverse_lerp(10.0, 200.0)
            * std::f32::consts::PI)
            .sin())
        .inverse_lerp(0.4, 1.0)
            * (s.time_since_mouse_on_button_counter as f32).inverse_lerp(40.0, 100.0);
        s.focus = s.focus.lerp_and_tick(
            s.get_to_focus,
            0.011_f32.lerp(0.05, num),
            1.0 / 3000.0_f32.lerp(900.0, num),
        );
        // `Random.value < 0.02 && |focus - getToFocus| < 0.002`: the draw is made first;
        // with no idle depths and no depth illustrations nothing more is drawn.
        let _ = rng.value() < 0.02 && (s.focus - s.get_to_focus).abs() < 0.002;
        if num > 0.3 && s.mouse_inactive_counter > 20 {
            s.get_to_focus = f32::MAX.inverse_lerp(1.0, 10.0);
        }
        s.last_cam_pos = s.cam_pos;
        s.cam_pos += s.cam_vel;
        s.cam_pos *= s.cam_pos.length().lerp_map(0.8, 1.0, 1.0, 0.96);
        s.cam_pos = s.cam_pos.clamp_length(1.0);
        s.cam_vel *= last_mouse.distance(mouse).lerp_map(
            200.0,
            0.0,
            0.98,
            0.99_f32.lerp(0.9, s.mouse_active),
        );
        let mid = (s.cam_get_to + s.cam_get_to2) / 2.0;
        s.cam_vel *= s.cam_pos.distance(mid).lerp_map(
            0.0,
            0.2,
            0.9_f32.max(s.mouse_active).max(s.game_pad_active),
            0.999,
        );
        let near = (s.cam_pos - s.cam_get_to).length() < 0.2;
        if rng.value() < 1.0 / if near { 10.0 } else { 300.0 } {
            let dir = Vec2::random_unit(rng);
            s.cam_get_to = dir * rng.value();
        }
        if rng.value() < 0.2 {
            let dir = Vec2::random_unit(rng);
            s.cam_get_to += dir * rng.value() * 0.05;
        }
        let gap = s.cam_get_to2.distance(s.cam_get_to);
        let speed = gap.lerp_map(0.0, 1.5, 0.01 + 0.01 * s.idle_cam_speed, 0.002);
        s.cam_get_to2 = Self::move_towards(s.cam_get_to2, s.cam_get_to, speed);
        let pad = Vec2::ZERO;
        let mouse_target = Vec2::new(
            -1.0 + 2.0 * mouse.x.lerp_map(0.0, 1366.0, 0.0, 1.0),
            -1.0 + 2.0 * mouse.y.lerp_map(0.0, 768.0, 0.0, 1.0),
        );
        s.cam_vel += (mouse_target - s.cam_pos) * 0.01 * s.mouse_active * (1.0 - s.game_pad_active);
        s.cam_vel += (pad - s.cam_pos)
            * 0.01
            * (pad.length() + s.cam_pos.distance(pad).inverse_lerp(0.3, 0.1))
                .lerp_map(0.7, 1.0, 1.0, 0.2)
            * s.game_pad_active
            * (1.0 - s.mouse_active);
        s.cam_vel += ((s.cam_get_to2 + s.cam_get_to) / 2.0 - s.cam_pos).clamp_length(0.02)
            * 0.005_f32.lerp(0.025, s.idle_cam_speed)
            * (1.0 - s.mouse_active)
            * (1.0 - s.game_pad_active);
        s.idle_cam_speed =
            s.idle_cam_speed
                .lerp_and_tick(s.idle_cam_speed_get_to, 0.002, 1.0 / 120.0);
        if rng.value() < 0.0125 {
            s.idle_cam_speed_get_to = rng.value();
        }
        if s.cam_pos.length() > 0.9 {
            s.cam_vel -= s.cam_pos * s.cam_pos.length().inverse_lerp(0.9, 1.0) * 0.05;
            if s.cam_get_to.length() > 0.9 {
                s.cam_get_to *= 0.95;
            }
        }
        if pad.length() > 0.1 {
            s.game_pad_inactive_counter = 0;
        } else {
            s.game_pad_inactive_counter += 1;
        }
        if s.game_pad_inactive_counter > 100 {
            s.game_pad_active *= 0.99;
        } else {
            s.game_pad_active =
                s.game_pad_active
                    .lerp_and_tick(pad.length().inverse_lerp(0.0, 0.2), 0.1, 0.025);
        }
        let mouse_moved = last_mouse.distance(mouse);
        if mouse_moved > 5.0 {
            s.mouse_inactive_counter = 0;
        } else {
            s.mouse_inactive_counter += 1;
        }
        if s.mouse_inactive_counter > 100 {
            s.mouse_active *= 0.99;
        } else {
            s.mouse_active =
                s.mouse_active
                    .lerp_and_tick(mouse_moved.inverse_lerp(3.0, 20.0), 0.1, 0.025);
        }
        if s.timer >= 0 {
            s.timer += 1;
        }
        s.time_since_mouse_on_button_counter += 1;
        if mouse_mode_selection {
            s.time_since_mouse_on_button_counter = 0;
        }
    }

    /// `Custom.MoveTowards`.
    fn move_towards(a: Vec2, b: Vec2, speed: f32) -> Vec2 {
        if (a - b).length() < speed {
            b
        } else {
            a + a.direction_to(b) * speed
        }
    }
}
