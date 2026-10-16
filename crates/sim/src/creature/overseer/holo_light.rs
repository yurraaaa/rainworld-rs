//! `HologramLight` (227621): the guide's projected light that follows the Monk through
//! dark rooms, made by `HologramLight.TryCreate` from `Player.Update`.

use rw_random::UnityRandom;

use crate::creature::CreatureId;
use crate::{FloatExt, Vec2};

/// A `HologramLight` in its room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct HologramLight {
    /// Unique; also its place in the update list.
    pub id: u64,
    pub room: i32,
    /// The guide projecting it.
    pub overseer: CreatureId,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub displace: Vec2,
    pub last_displace: Vec2,
    pub displace_goal: Vec2,
    pub push_around_pos: Vec2,
    pub last_push_around_pos: Vec2,
    /// The overseer's eye.
    pub proj_pos: Vec2,
    pub last_proj_pos: Vec2,
    pub power: f32,
    pub last_power: f32,
    pub out_of_range_counter: i32,
    pub not_needed_counter: i32,
    pub flicker: f32,
    pub last_flicker: f32,
    pub flicker_fac: f32,
    pub in_front: Vec2,
    pub power_down_and_kill: bool,
    pub respawn: bool,
    pub active_linger: i32,
    /// Its `LightSource` was made (on the first update); it only lights the drawing.
    pub light_source: bool,
}

/// What `HologramLight.Update` reads of its overseer and player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoloLightTick {
    /// `OverseerEyePos(1)`: the graphics' eye while it has graphics and a room, else
    /// the main chunk; `None` once the overseer is gone (the light keeps the last).
    pub eye: Option<Vec2>,
    /// The overseer's first chunk now and last tick.
    pub overseer_chunk: (Vec2, Vec2),
    pub overseer_extended: f32,
    /// `projectorActive`: the overseer is in the light's room, not zipping.
    pub projector_active: bool,
    /// `overseer.AI.communication.holoLight == this` (true once the overseer is gone:
    /// the game's stale module keeps pointing at the light).
    pub still_the_guides: bool,
    /// The player is in the light's room (not in a shortcut).
    pub player_here: bool,
    /// The player is in another room (not in a shortcut).
    pub player_elsewhere: bool,
    pub player_entering_shortcut: bool,
    /// The player's main chunk now and last tick, and `bodyChunks[1]`.
    pub main_chunk: (Vec2, Vec2),
    pub hips: Vec2,
    /// `HologramLight.Needed(player)`.
    pub needed: f32,
}

impl HologramLight {
    #[must_use]
    pub fn new(id: u64, room: i32, overseer: CreatureId) -> Self {
        Self {
            id,
            room,
            overseer,
            slated: false,
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            displace: Vec2::ZERO,
            last_displace: Vec2::ZERO,
            displace_goal: Vec2::ZERO,
            push_around_pos: Vec2::ZERO,
            last_push_around_pos: Vec2::ZERO,
            proj_pos: Vec2::ZERO,
            last_proj_pos: Vec2::ZERO,
            power: 0.0,
            last_power: 0.0,
            out_of_range_counter: 0,
            not_needed_counter: 0,
            flicker: 0.0,
            last_flicker: 0.0,
            flicker_fac: 0.0,
            in_front: Vec2::ZERO,
            power_down_and_kill: false,
            respawn: false,
            active_linger: 0,
            light_source: false,
        }
    }

    /// `GoalPos`: a little in front of the player's upper body.
    fn goal_pos(&self, t: &HoloLightTick) -> Vec2 {
        t.main_chunk.0.lerp(t.hips, 0.3) + (self.in_front * 20.0).clamp_magnitude(60.0) * self.power
    }

    /// `Update`: three to nine draws (the displacement goal's chance, two more when it
    /// moves and three more when it leaves 60 px; the flicker's two chances and a value
    /// after each that passes).
    pub fn update(&mut self, t: &HoloLightTick, rng: &mut UnityRandom) {
        self.last_pos = self.pos;
        self.last_power = self.power;
        self.last_displace = self.displace;
        self.last_flicker = self.flicker;
        self.last_proj_pos = self.proj_pos;
        self.last_push_around_pos = self.push_around_pos;
        if let Some(eye) = t.eye {
            self.proj_pos = eye;
        }
        self.push_around_pos *= 0.8;
        if t.overseer_extended > 0.0 {
            self.push_around_pos += (t.overseer_chunk.0 - t.overseer_chunk.1) * t.overseer_extended;
        }
        if self.proj_pos.dist_less(self.pos, 1700.0) {
            self.out_of_range_counter -= 10;
        } else {
            self.out_of_range_counter += 1;
        }
        self.out_of_range_counter = self.out_of_range_counter.clamp(0, 200);
        self.displace = self
            .displace
            .move_towards(self.displace_goal, 2.0)
            .lerp(self.displace_goal, 0.05);
        if rng.value() < 1.0 / 15.0 {
            let dir = Vec2::random_unit(rng);
            self.displace_goal += dir * (rng.value() * 20.0);
            if self.displace_goal.length() > 60.0 {
                let dir = Vec2::random_unit(rng);
                let a = rng.value();
                self.displace_goal = dir * (a * rng.value() * 60.0);
            }
        }
        let chance = self
            .pos
            .distance(self.proj_pos)
            .lerp_map(500.0, 1200.0, 0.005, 1.0 / 30.0);
        if rng.value() < chance {
            self.flicker_fac = rng.value();
        }
        self.flicker_fac = (self.flicker_fac - 1.0 / 30.0).max(0.0);
        if rng.value() < 0.6 * self.flicker_fac {
            self.flicker = self.flicker.lerp(0.5 + 0.5 * self.flicker_fac, rng.value());
        } else {
            self.flicker = (self.flicker - 0.1).max(0.0);
        }
        if t.projector_active {
            self.active_linger = 20;
        } else if self.active_linger > 0 {
            self.active_linger -= 1;
        }
        self.light_source = true;
        if t.player_here
            && !t.player_entering_shortcut
            && self.active_linger > 0
            && !self.power_down_and_kill
        {
            self.follow_player(t);
        } else {
            self.respawn = true;
            self.power = (self.power - 1.0 / 30.0).max(0.0);
            if self.power_down_and_kill && self.power <= 0.0 && self.last_power <= 0.0 {
                self.slated = true;
            }
            if !t.projector_active {
                self.active_linger = 0;
            }
        }
        if t.needed < 0.1 {
            self.not_needed_counter += 1;
        } else if t.needed > 0.4 {
            self.not_needed_counter -= 10;
        } else if t.needed > 0.2 {
            self.not_needed_counter -= 1;
        }
        self.not_needed_counter = self.not_needed_counter.clamp(0, 500);
        if t.player_elsewhere || !t.still_the_guides || self.not_needed_counter >= 500 {
            self.power_down_and_kill = true;
        }
    }

    /// The light fades out before it jumps to the player, then follows it and powers
    /// up (less while the projector rests, out of range or not needed).
    fn follow_player(&mut self, t: &HoloLightTick) {
        if self.respawn {
            if self.power <= 0.0 && self.last_power <= 0.0 {
                self.respawn = false;
                self.pos = self.goal_pos(t);
                self.in_front *= 0.0;
            } else {
                self.power = (self.power - 1.0 / 30.0).max(0.0);
            }
            return;
        }
        self.in_front = self.in_front.lerp(t.main_chunk.0 - t.main_chunk.1, 0.1);
        let goal = self.goal_pos(t);
        self.pos = self.pos.move_towards(goal, 7.0).lerp(goal, 0.1);
        let projector = if t.projector_active { 1.0 } else { 0.9 };
        let cap = projector
            * (self.out_of_range_counter as f32).inverse_lerp(200.0, 150.0)
            * (self.not_needed_counter as f32).inverse_lerp(80.0, 50.0);
        self.power = cap.min(self.power + 1.0 / 30.0);
    }

    /// `HologramLight.Needed`: how dark it is around the player in the camera's room
    /// (`InverseLerp(0.85, 0.92, darkness)`), 0 when dead or below 0.85. No light
    /// sources or lanterns are ported, so none lighten it.
    #[must_use]
    pub fn needed(in_camera_room: bool, darkness: f32, dead: bool) -> f32 {
        if !in_camera_room || darkness < 0.85 || dead {
            return 0.0;
        }
        darkness.inverse_lerp(0.85, 0.92)
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;

    use super::{HoloLightTick, HologramLight};
    use crate::Vec2;
    use crate::creature::CreatureId;

    fn tick() -> HoloLightTick {
        HoloLightTick {
            eye: Some(Vec2::new(100.0, 200.0)),
            overseer_chunk: (Vec2::new(100.0, 200.0), Vec2::new(100.0, 200.0)),
            overseer_extended: 1.0,
            projector_active: true,
            still_the_guides: true,
            player_here: true,
            player_elsewhere: false,
            player_entering_shortcut: false,
            main_chunk: (Vec2::new(100.0, 100.0), Vec2::new(100.0, 100.0)),
            hips: Vec2::new(100.0, 90.0),
            needed: 0.7,
        }
    }

    /// The first seed (from `from`) whose first `value` is below 1/15 or not.
    fn seed(from: i32, moves: bool) -> i32 {
        (from..)
            .find(|&s| (UnityRandom::new(s).value() < 1.0 / 15.0) == moves)
            .expect("a seed")
    }

    /// A still goal and no flicker: the goal's chance, the flicker's chance (0.005
    /// within 500 px of the eye; a 0 flicker factor fails the third) and the flicker
    /// chance: three draws. A moving goal adds `RNV` and a `value`.
    #[test]
    fn update_draws() {
        let mut light = HologramLight::new(1, 0, CreatureId(1));
        light.pos = Vec2::new(100.0, 150.0);
        let s = (0..)
            .map(|from| seed(from, false))
            .find(|&s| {
                let mut r = UnityRandom::new(s);
                r.value();
                r.value() >= 0.005
            })
            .expect("a seed");
        let mut rng = UnityRandom::new(s);
        let mut expected = rng.clone();
        for _ in 0..3 {
            expected.value();
        }
        light.update(&tick(), &mut rng);
        assert_eq!(rng, expected);

        let s = (0..)
            .map(|from| seed(from, true))
            .find(|&s| {
                let mut r = UnityRandom::new(s);
                r.value();
                r.value();
                r.value();
                r.value() >= 0.005
            })
            .expect("a seed");
        let mut light = HologramLight::new(1, 0, CreatureId(1));
        light.pos = Vec2::new(100.0, 150.0);
        let mut rng = UnityRandom::new(s);
        let mut expected = rng.clone();
        for _ in 0..5 {
            expected.value();
        }
        light.update(&tick(), &mut rng);
        assert_eq!(rng, expected);
        assert!(light.displace_goal.length() <= 20.0);
        assert!(light.displace_goal.length() > 0.0);
    }

    /// Following the player: `GoalPos` is 30% of the way from the main chunk to the
    /// hips (no `inFront` yet); the light moves 7 px and a tenth of the rest toward it
    /// and powers up by 1/30 a tick. The projector 20 ticks; out of the dark (`Needed`
    /// below 0.1) 500 ticks power it down, then it is destroyed.
    #[test]
    fn follows_the_player_and_powers_down_unneeded() {
        let mut light = HologramLight::new(1, 0, CreatureId(1));
        light.pos = Vec2::new(100.0, 150.0);
        let mut rng = UnityRandom::new(3);
        light.update(&tick(), &mut rng);
        let goal = Vec2::new(100.0, 97.0);
        let expected = Vec2::new(100.0, 143.0).lerp(goal, 0.1);
        assert!(light.pos.distance(expected) < 1e-3);
        assert!((light.power - 1.0 / 30.0).abs() < 1e-6);
        assert_eq!(light.active_linger, 20);
        assert!(!light.respawn);

        let dark = HoloLightTick {
            needed: 0.0,
            ..tick()
        };
        for _ in 0..499 {
            light.update(&dark, &mut rng);
        }
        assert_eq!(light.not_needed_counter, 499);
        assert!(!light.power_down_and_kill);
        light.update(&dark, &mut rng);
        assert!(light.power_down_and_kill);
        let mut ticks = 0;
        while !light.slated {
            light.update(&dark, &mut rng);
            ticks += 1;
            assert!(ticks < 100);
        }
        assert_eq!(light.power, 0.0);
    }

    /// `Needed`: `InverseLerp(0.85, 0.92, darkness)` in the camera's room, alive.
    #[test]
    fn needed_by_darkness() {
        assert_eq!(HologramLight::needed(true, 0.8, false), 0.0);
        assert!((HologramLight::needed(true, 0.9, false) - 0.05 / 0.07).abs() < 1e-6);
        assert_eq!(HologramLight::needed(true, 1.0, false), 1.0);
        assert_eq!(HologramLight::needed(false, 1.0, false), 0.0);
        assert_eq!(HologramLight::needed(true, 1.0, true), 0.0);
    }
}
