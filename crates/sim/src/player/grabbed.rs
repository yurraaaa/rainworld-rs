//! The player held by a creature: `dangerGrasp` and its timers, `cantBeGrabbedCounter`,
//! the pacifying hold's stun (`Creature.Update`), the release under the room, and the
//! `WALLSTUCK` unsticking that only a free body does. The escapes (`ThrowToGetFree`,
//! `DangerGraspPickup`) and `GameOver(dangerGrasp)` reach the world and live in
//! `player/grabbed_turn.rs`. Omitted branches: docs/divergences/ D19.
//!
//! Mirrors: Player.cs (Grabbed, CanBeGrabbed, Update), Creature.cs (Update)

use rw_assets::Room;

use super::{HEAD, HIPS, Player};
use crate::creature::Grasp;
use crate::physics::room_ext::RoomExt;

/// `Player.SpitOutOfShortCut` sets `cantBeGrabbedCounter` to this.
pub(super) const CANT_BE_GRABBED_AFTER_SPIT_OUT: i32 = 30;

/// The player's side of being held.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DangerGraspState {
    /// `dangerGrasp`: the grasp of a lizard (the base game's other danger grabbers,
    /// vultures, big spiders, drop bugs, are not realized by the port) on the player.
    pub grasp: Option<Grasp>,
    /// `dangerGraspTime`: ticks held; 30 drops what the hands hold, 60 asks for
    /// `GameOver(dangerGrasp)`.
    pub time: i32,
    /// `dangerGraspLastThrowButton`.
    pub last_throw_button: bool,
    /// `dangerGraspPickupButton`.
    pub pickup_button: bool,
    /// `cantBeGrabbedCounter`.
    pub cant_be_grabbed_counter: i32,
    /// `grabbedBy` as the game sets it before the update: anything holds the body, and
    /// whether one of those grasps is pacifying.
    pub held: bool,
    pub pacified: bool,
}

/// What the `dangerGrasp` block of `Player.Update` asks the game to do this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DangerGraspStep {
    /// `ThrowToGetFree`: a fresh throw press while free enough to struggle.
    pub throw_to_get_free: bool,
    /// `DangerGraspPickup`: a fresh pick-up press while free enough to struggle.
    pub pickup: bool,
    /// Tick 30: `LoseAllGrasps`.
    pub lose_all_grasps: bool,
    /// Tick 60: `GameOver(dangerGrasp)`.
    pub game_over: bool,
}

impl Player {
    /// The `dangerGrasp` state.
    #[must_use]
    pub fn danger_grasp(&self) -> &DangerGraspState {
        &self.danger
    }

    /// `Player.Grabbed` after the base `PhysicalObject.Grabbed`: the body goes through
    /// floors, and a lizard's grasp becomes the danger grasp with its time reset.
    pub(crate) fn grabbed(&mut self, grasp: Grasp, by_lizard: bool) {
        if by_lizard {
            self.danger.time = 0;
            self.danger.grasp = Some(grasp);
        }
    }

    /// `Player.IsHidden`: Watcher's camouflage (`isCamo`), its one-way placement
    /// cooldown and final warp; none exists in the base game (D19).
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        false
    }

    /// `Player.CanBeGrabbed`.
    #[must_use]
    pub fn can_be_grabbed(&self) -> bool {
        self.danger.cant_be_grabbed_counter <= 0
    }

    /// `grabbedBy` as this update starts: whether anything holds the body (which also
    /// lifts `BodyChunk.Update`'s room-range clamp) and whether a holder pacifies.
    pub(crate) fn set_held(&mut self, held: bool, pacified: bool) {
        self.danger.held = held;
        self.danger.pacified = pacified;
        self.body.held = held;
    }

    /// `ShortcutHelper.CanBePulledIntoShortcut(player)`, read by the chunks' `goThroughFloors`.
    pub(crate) fn set_pulled_into_shortcut(&mut self, pulled: bool) {
        self.body.set_pulled_into_shortcut(pulled);
    }

    pub(crate) fn set_cant_be_grabbed_after_spit_out(&mut self) {
        self.danger.cant_be_grabbed_counter = CANT_BE_GRABBED_AFTER_SPIT_OUT;
    }

    /// `Player.Update`'s `cantBeGrabbedCounter` countdown and its `dangerGrasp` block:
    /// no grasp resets the time; a discontinued one (`discontinued`, the grabber let go)
    /// is forgotten; else the time grows and, before tick 30, a living player stunned
    /// less than 30 struggles on a fresh press of throw or pick-up read straight from the
    /// controls (`RWInput.PlayerInput`, so the stun's neutral input does not hide it),
    /// at 30 lets go of what it holds, at 60 asks for the game over. A player AI's
    /// nervous draws are not made: the player is never an AI in the port.
    pub(crate) fn danger_grasp_update(
        &mut self,
        discontinued: bool,
        throw: bool,
        pickup: bool,
    ) -> DangerGraspStep {
        if self.danger.cant_be_grabbed_counter > 0 {
            self.danger.cant_be_grabbed_counter -= 1;
        }
        let mut step = DangerGraspStep::default();
        if self.danger.grasp.is_none() {
            self.danger.time = 0;
            return step;
        }
        if discontinued {
            self.danger.grasp = None;
            self.danger.time = 0;
            return step;
        }
        self.danger.time += 1;
        if self.danger.time == 30 {
            step.lose_all_grasps = true;
        } else if self.danger.time < 30 {
            if !self.dead && self.stun < 30 {
                step.throw_to_get_free = throw && !self.danger.last_throw_button;
                step.pickup = pickup && !self.danger.pickup_button;
            }
            self.danger.last_throw_button = throw;
            self.danger.pickup_button = pickup;
        } else if self.danger.time == 60 {
            step.game_over = true;
        }
        step
    }

    /// `Creature.Update`: under 10 stun ticks, a pacifying grasp holds it at 10.
    pub(super) fn pacifying_stun(&mut self) {
        if self.stun < 10 && self.danger.held && self.danger.pacified {
            self.stun = 10;
        }
    }

    /// `Creature.Update`'s `FORCE CREATURE RELEASE UNDER ROOM`: the head within 10 px of
    /// the out-of-room limit while held makes every grasp on the body let go.
    pub(super) fn release_under_room(&mut self, limit: f32) {
        if self.body.chunks[HEAD].pos.y < limit + 10.0 && self.danger.held {
            self.release_grabbed_by = true;
            // The game lets go at once: the rest of the update sees an empty `grabbedBy`.
            self.set_held(false, false);
            self.set_pulled_into_shortcut(false);
        }
    }

    /// Whether the body asked for every grasp on it to let go since the last call.
    pub(crate) fn take_release_grabbed_by(&mut self) -> bool {
        std::mem::take(&mut self.release_grabbed_by)
    }

    /// `Player.Update`'s `WALLSTUCK`: a free body whose chunks touch opposite floor and
    /// ceiling with a Solid tile between them and no line of sight between them (along
    /// the middle or either side) puts the hips 2 px from the head.
    pub(super) fn unstick_from_wall(&mut self, room: &Room) {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let cy = head.contact().y;
        if cy == 0 || cy != -hips.contact().y || self.danger.held {
            return;
        }
        if !room.is_solid_at((head.pos + hips.pos) / 2.0) {
            return;
        }
        let side = (head.pos - hips.pos).perpendicular();
        let (a, b) = (head.pos, hips.pos);
        let (ra, rb) = (head.rad, hips.rad);
        if room.visual_contact(a, b)
            || room.visual_contact(a + side * ra, b + side * rb)
            || room.visual_contact(a - side * ra, b - side * rb)
        {
            return;
        }
        log::warn!("WALLSTUCK");
        let to = a + a.direction_to(b) * 2.0;
        self.body.chunks[HIPS].hard_set_position(to);
    }
}
