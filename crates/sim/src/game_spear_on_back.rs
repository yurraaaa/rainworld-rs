//! The Hunter's spear on the back in the game: `SpearOnBack.Update` (holding pick-up
//! stashes a held spear or takes the back one into a hand), `SpearToBack`,
//! `SpearToHand`, a `DropSpear` applied to the spear, `GraphicsModuleUpdated` carrying
//! it on the back, and `CanPutSpearToBack` / `CanRetrieveSpearFromBack`. The slugcat's
//! own part is `player/spear_on_back.rs`. Base game: the Hunter only (no Expedition
//! back-spear unlock, no slugcat on the back).
//!
//! Mirrors: Player.cs (SpearOnBack, CanPutSpearToBack, CanRetrieveSpearFromBack)

use super::items::Grabability;
use super::{PlayerHands, PlayerObserver, RainWorldGame};
use crate::item::{Entity, ObjectId, WeaponMode};
use crate::player::BackSpearDrop;
use crate::{Animation, BodyMode, FloatExt, SoundId, SpearOnBack, Vec2};

/// `counter` passes this to stash or retrieve.
const SWAP_TICKS: i32 = 20;

impl RainWorldGame {
    /// The player's `spearOnBack`, copied.
    pub(super) fn spear_on_back(&self) -> Option<SpearOnBack> {
        self.player
            .as_ref()
            .and_then(|p| p.spear_on_back().copied())
    }

    fn spear_on_back_mut(&mut self) -> Option<&mut SpearOnBack> {
        self.player
            .as_mut()
            .and_then(crate::Player::spear_on_back_mut)
    }

    /// `CanPutSpearToBack`.
    pub(super) fn can_put_spear_to_back(&self) -> bool {
        self.spear_on_back()
            .is_some_and(|s| !s.interaction_locked && s.spear.is_none())
    }

    /// `CanRetrieveSpearFromBack`: a spear on the back, unlocked, a free hand and nothing
    /// big held.
    pub(super) fn can_retrieve_spear_from_back(&self) -> bool {
        let Some(s) = self.spear_on_back() else {
            return false;
        };
        let grasps = self.player_grasps();
        if s.spear.is_none() || s.interaction_locked || grasps.iter().all(Option::is_some) {
            return false;
        }
        !self.holds_something_big()
    }

    /// A grasp with `Grabability >= BigOneHand`.
    fn holds_something_big(&self) -> bool {
        self.player_grasps()
            .iter()
            .flatten()
            .any(|&e| self.grabability(e) >= Grabability::BigOneHand)
    }

    /// `spearOnBack.increment = on`.
    pub(super) fn set_spear_increment(&mut self, on: bool) {
        if let Some(s) = self.spear_on_back_mut() {
            s.increment = on;
        }
    }

    /// `spearOnBack.interactionLocked = true`.
    pub(super) fn lock_spear_on_back(&mut self) {
        if let Some(s) = self.spear_on_back_mut() {
            s.interaction_locked = true;
        }
    }

    /// `SpearOnBack.Update`, first in `GrabUpdate`: while `increment` holds the counter
    /// runs; past 20 the back spear goes to a hand, or else the first held spear goes
    /// on the back (the head pushed 2 px away from it). Letting go of pick-up unlocks.
    pub(super) fn spear_on_back_update(
        &mut self,
        index: i32,
        eu: bool,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(s) = self.spear_on_back_mut() else {
            return;
        };
        if s.increment {
            s.counter += 1;
            let (counter, has) = (s.counter, s.spear.is_some());
            if has && counter > SWAP_TICKS {
                self.spear_to_hand(index, eu, observer);
                self.set_spear_counter(0);
            } else if !has && counter > SWAP_TICKS {
                let grasps = self.player_grasps();
                let held = grasps
                    .iter()
                    .flatten()
                    .find(|&&e| self.world.is_spear(e))
                    .and_then(|e| e.object());
                if let Some(id) = held {
                    let at = self
                        .world
                        .chunk_pos(Entity::Object(id), 0)
                        .unwrap_or_default();
                    let head = &mut self.player_mut().body.chunks[0];
                    head.pos += at.direction_to(head.pos) * 2.0;
                    self.spear_to_back(index, id);
                    self.set_spear_counter(0);
                }
            }
        } else {
            s.counter = 0;
        }
        let pickup = self.player_ref().input_history()[0].pickup;
        let Some(s) = self.spear_on_back_mut() else {
            return;
        };
        if !pickup {
            s.interaction_locked = false;
        }
        s.increment = false;
    }

    fn set_spear_counter(&mut self, counter: i32) {
        if let Some(s) = self.spear_on_back_mut() {
            s.counter = counter;
        }
    }

    /// `SpearToHand`: with nothing big held, the back spear goes to the first free hand
    /// (moved to that hand when there are graphics), `Slugcat_Pick_Up_Spear`, the stick
    /// ends.
    pub(super) fn spear_to_hand(
        &mut self,
        index: i32,
        eu: bool,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(spear) = self.spear_on_back().and_then(|s| s.spear) else {
            return;
        };
        if self.holds_something_big() {
            return;
        }
        let Some(num) = (0..2).find(|&i| self.player_grasps()[i].is_none()) else {
            return;
        };
        let obj = Entity::Object(spear);
        if let Some(h) = observer.player_hands() {
            let even = self.world.realized_even_update(obj);
            if let Some(c) = self.world.chunk_mut(obj, 0) {
                c.move_from_outside_my_update(even, eu, h.hands[num].0);
            }
        }
        self.slugcat_grab(obj, num, observer);
        let Some(s) = self.spear_on_back_mut() else {
            return;
        };
        s.spear = None;
        s.interaction_locked = true;
        let stick = s.stick.take();
        self.player_mut().grab_mut().no_pick_up_on_release = 20;
        self.play_on_player(index, SoundId::SlugcatPickUpSpear);
        if let Some(serial) = stick {
            self.world.deactivate_on_back_stick(serial);
        }
        log::info!("took the spear from the back");
    }

    /// `SpearToBack`: the hand holding the spear lets go, the spear goes `OnBack`,
    /// `Slugcat_Stash_Spear_On_Back`, and a new on-back stick replaces the old one.
    pub(super) fn spear_to_back(&mut self, index: i32, spear: ObjectId) {
        if self.spear_on_back().is_none_or(|s| s.spear.is_some()) {
            return;
        }
        if let Some(i) = (0..2).find(|&i| self.player_grasps()[i] == Some(Entity::Object(spear))) {
            self.player_release_grasp(i);
        }
        let Some(s) = self.spear_on_back_mut() else {
            return;
        };
        s.spear = Some(spear);
        let old = s.stick.take();
        self.world
            .change_weapon_mode(spear, WeaponMode::OnBack, &mut self.rng);
        // A spear pulled out of a creature plays `Spear_Dislodged_From_Creature` inside
        // `ChangeMode`.
        self.flush_dislodged();
        if let Some(s) = self.spear_on_back_mut() {
            s.interaction_locked = true;
        }
        self.player_mut().grab_mut().no_pick_up_on_release = 20;
        self.play_on_player(index, SoundId::SlugcatStashSpearOnBack);
        if let Some(serial) = old {
            self.world.deactivate_on_back_stick(serial);
        }
        let serial = self.world.add_on_back_stick(self.player_id, spear);
        if let Some(s) = self.spear_on_back_mut() {
            s.stick = Some(serial);
        }
        log::info!("put a spear on the back");
    }

    /// The `DropSpear` the slugcat made (`Stun`, `Die`, or laying it down), applied to
    /// the spear: its velocity, `ChangeMode(Free)` with the spin drawn then, the stick
    /// ended.
    pub(super) fn apply_back_spear_drop(&mut self) {
        let Some(drop) = self
            .player
            .as_mut()
            .and_then(crate::Player::take_back_spear_drop)
        else {
            return;
        };
        let BackSpearDrop {
            spear,
            stick,
            vel,
            spin,
        } = drop;
        if let Some(c) = self.world.chunk_mut(Entity::Object(spear), 0) {
            c.vel = vel;
        }
        self.world
            .change_weapon_mode_spun(spear, WeaponMode::Free, Some(spin), &mut self.rng);
        if let Some(serial) = stick {
            self.world.deactivate_on_back_stick(serial);
        }
        log::info!("dropped the spear from the back");
    }

    /// `SpearOnBack.GraphicsModuleUpdated`: a spear gone or grabbed by anything is
    /// forgotten; otherwise it rides between the hips and the shoulders (the draw
    /// positions and head when there are graphics, else the chunks), leaning with
    /// `flip`, spun round while being retrieved or stashed with sideways input, in front
    /// of the body when the body points down. Unconscious, or floating (`ZeroG`, gravity 0),
    /// it eases `flip` to 0 and lies along the back.
    pub(super) fn spear_on_back_graphics_updated(&mut self, hands: Option<PlayerHands>) {
        let Some(s) = self.spear_on_back() else {
            return;
        };
        let Some(spear) = s.spear else { return };
        let gone = self.world.object(spear).realized.is_none();
        if gone || !self.world.object(spear).grabbed_by.is_empty() {
            if let Some(sob) = self.spear_on_back_mut() {
                sob.spear = None;
                let stick = sob.stick.take();
                if let Some(serial) = stick {
                    self.world.deactivate_on_back_stick(serial);
                }
            }
            return;
        }
        let eu = self.even_update();
        let p = self.player_ref();
        let (v, v2) = match hands {
            Some(h) => (h.draw_positions[0].lerp(h.head, 0.2), h.draw_positions[1]),
            None => (p.head().pos, p.hips().pos),
        };
        let v3 = v2.direction_to(v);
        let input = p.input_history()[0];
        let mut flip = s.flip;
        let rotation;
        let overlap;
        if p.is_conscious() && p.body_mode() != BodyMode::ZeroG && p.effective_room_gravity() > 0.0
        {
            flip = if p.body_mode() == BodyMode::Default
                && p.animation() == Animation::None
                && p.is_standing()
                && p.hips().pos.y < p.head().pos.y - 6.0
            {
                flip.lerp_and_tick(input.x as f32 * 0.3, 0.05, 0.02)
            } else if p.body_mode() == BodyMode::Stand && input.x != 0 {
                flip.lerp_and_tick(input.x as f32, 0.02, 0.1)
            } else {
                flip.lerp_and_tick(p.flip_direction() as f32 * v3.x.abs(), 0.15, 1.0 / 6.0)
            };
            rotation = if s.counter > 12 && !s.interaction_locked && input.x != 0 && p.is_standing()
            {
                let grasps = self.player_grasps();
                let num = match (0..2).find(|&i| grasps[i].is_none()) {
                    Some(0) => -1.0,
                    Some(_) => 1.0,
                    None => 0.0,
                };
                let spin = (s.counter as f32).lerp_map(12.0, 20.0, 0.0, 360.0 * num);
                Vec2::from_degrees((v - v2).to_degrees() + spin)
            } else {
                (v3 - v3.perpendicular() * (0.9 * (1.0 - flip.abs()))).normalized()
            };
            overlap = v3.y < -0.1 && p.body_mode() != BodyMode::ClimbingOnBeam;
        } else {
            flip = flip.lerp_and_tick(0.0, 0.15, 1.0 / 7.0);
            rotation = v3 - v3.perpendicular() * 0.9;
            overlap = false;
        }
        let main_vel = p.head().vel;
        let to = v2.lerp(v, 0.6) - (v2 - v).perpendicular() * (7.5 * flip);
        if let Some(sob) = self.spear_on_back_mut() {
            sob.flip = flip;
        }
        self.world.set_held_rotation(spear, rotation);
        self.world.change_overlap(spear, overlap);
        let obj = Entity::Object(spear);
        let even = self.world.realized_even_update(obj);
        if let Some(c) = self.world.chunk_mut(obj, 0) {
            c.move_from_outside_my_update(even, eu, to);
            c.vel = main_vel;
        }
    }
}

#[cfg(test)]
#[path = "game_spear_on_back_tests.rs"]
mod tests;
