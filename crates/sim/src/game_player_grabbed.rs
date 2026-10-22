//! The player held by a creature, in the game: `Creature.Grab` of the player
//! (`CanBeGrabbed`, `Player.Grabbed`), the `dangerGrasp` block of `Player.Update` with
//! its escapes `ThrowToGetFree` and `DangerGraspPickup`, its tick-30 `LoseAllGrasps` and
//! tick-60 `GameOver(dangerGrasp)`, and `Creature.Update`'s release under the room.
//! Omitted branches (DLC, the shortcut carry): docs/divergences/ D19.
//!
//! Mirrors: Player.cs (Update, Grabbed, ThrowToGetFree, DangerGraspPickup), Creature.cs (Grab, Update)

use rw_assets::Room;

use super::{PlayerObserver, RainWorldGame};
use crate::creature::{GrabRequest, Grasp};
use crate::item::{Entity, Throw};
use crate::{Input, Player};

impl RainWorldGame {
    /// `Creature.Grab` through the world, with the player's side: a player that
    /// `cantBeGrabbedCounter` protects refuses, a grabbed one gets `Player.Grabbed`.
    pub(super) fn creature_grab(&mut self, r: GrabRequest) -> bool {
        let player = r.grabbed == Entity::Creature(self.player_id);
        if player && !self.player.as_ref().is_some_and(Player::can_be_grabbed) {
            return false;
        }
        if !self.world.grab(r, &mut self.rng) {
            return false;
        }
        if player && let Some(g) = self.world.creature(r.grabber).grasps[r.grasp_used] {
            let by_lizard = self.world.creature(r.grabber).lizard().is_some();
            if let Some(p) = self.player.as_mut() {
                p.grabbed(g, by_lizard);
            }
        }
        true
    }

    /// The grasps on the player (`grabbedBy`).
    #[must_use]
    pub fn player_grabbed_by(&self) -> &[Grasp] {
        self.world.grabbed_by(Entity::Creature(self.player_id))
    }

    /// `Grasp.discontinued`: its grabber no longer holds it.
    fn grasp_discontinued(&self, g: &Grasp) -> bool {
        self.world
            .creature(g.grabber)
            .grasps
            .get(g.grasp_used)
            .copied()
            .flatten()
            .is_none_or(|h| h.serial != g.serial)
    }

    /// `Player.Update`'s `dangerGrasp` block (after `cantBeGrabbedCounter`, before the
    /// lungs and `checkInput`); `input` is this tick's controls (`RWInput.PlayerInput`).
    pub(super) fn player_danger_grasp_update(
        &mut self,
        index: i32,
        room: &Room,
        input: Input,
        observer: &mut dyn PlayerObserver,
    ) {
        let held = self.player_grabbed_by();
        let (any, pacified) = (!held.is_empty(), held.iter().any(|g| g.pacifying));
        let discontinued = self
            .player
            .as_ref()
            .and_then(|p| p.danger_grasp().grasp)
            .is_some_and(|g| self.grasp_discontinued(&g));
        let Some(p) = self.player.as_mut() else {
            return;
        };
        p.set_held(any, pacified);
        let step = p.danger_grasp_update(discontinued, input.throw, input.pickup);
        if step.throw_to_get_free {
            self.throw_to_get_free(index, observer);
        }
        if step.pickup {
            self.danger_grasp_pickup(index, room, observer);
        }
        if step.lose_all_grasps {
            self.release_all_grasps();
        }
        if step.game_over {
            let grasp = self.player.as_ref().and_then(|p| p.danger_grasp().grasp);
            let main = self.player_ref().head().pos;
            self.game_over(main, room, grasp.map(|g| g.serial));
        }
    }

    /// `Creature.Update`'s release under the room: every grasp on the player lets go,
    /// first to last.
    pub(super) fn apply_player_release_grabbed_by(&mut self) {
        if !self
            .player
            .as_mut()
            .is_some_and(Player::take_release_grabbed_by)
        {
            return;
        }
        log::info!("FORCE CREATURE RELEASE UNDER ROOM");
        while let Some(g) = self.player_grabbed_by().first().copied() {
            self.world.release(g);
        }
    }

    /// The hand to fight back with: the first holding a spear, else the first holding a
    /// weapon.
    fn throw_to_get_free_hand(&self) -> Option<usize> {
        let grasps = self.player_grasps();
        let spear = (0..2).find(|&i| grasps[i].is_some_and(|e| self.world.is_spear(e)));
        spear.or_else(|| (0..2).find(|&i| grasps[i].is_some_and(|e| e.object().is_some())))
    }

    /// `ThrowToGetFree`: without a weapon the first thing held is tossed and let go; a
    /// weapon is thrown as usual when nothing dangerous holds the player or, not a
    /// spear, half the time (one draw); otherwise it is thrown from the main chunk at
    /// the grabber's nearest chunk, which it will hit (`meleeHitChunk`), a spear with
    /// `ThrownSpear`'s bonus, and let go.
    pub(super) fn throw_to_get_free(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let Some(num) = self.throw_to_get_free_hand() else {
            let grasps = self.player_grasps();
            if let Some(k) = (0..2).find(|&k| grasps[k].is_some()) {
                if let Some(Entity::Creature(fly)) = grasps[k] {
                    self.toss_object(index, fly);
                }
                self.player_release_grasp(k);
            }
            return;
        };
        let Some(Entity::Object(id)) = self.player_grasps()[num] else {
            return;
        };
        let spear = self.world.is_spear(Entity::Object(id));
        let danger = self.player_ref().danger_grasp().grasp;
        let Some(danger) = danger else {
            self.throw_object(index, num, observer);
            return;
        };
        if !spear && self.rng.value() < 0.5 {
            self.throw_object(index, num, observer);
            return;
        }
        let main = self.player_ref().head().clone();
        let mut target = None;
        let mut dst = f32::MAX;
        for r in self.creature_chunk_refs(danger.grabber) {
            let Some(c) = self.chunk_of(r) else { continue };
            if main.pos.dist_less(c.pos, dst) {
                target = Some((r, c.pos));
                dst = main.pos.distance(c.pos);
            }
        }
        let Some((r, at)) = target else {
            return;
        };
        let throw = Throw {
            by: self.player_id,
            by_vel: main.vel,
            pos: main.pos,
            first_frame_trace_from_pos: Some(main.pos),
            dir: (if main.pos.x < at.x { 1 } else { -1 }, 0),
            frc: 1.0,
            eu: self.even_update(),
        };
        if let Some(sound) = self.world.throw_weapon(id, throw, &mut self.rng)
            && let Some(pos) = self.world.chunk_pos(Entity::Object(id), 0)
        {
            self.play_on_object(index, sound, pos);
        }
        if spear {
            self.thrown_spear(id);
        }
        if let Some(w) = self.world.weapon_mut(id) {
            w.melee_hit_chunk = r.owner.creature().map(|c| (c, r.index));
        }
        self.player_release_grasp(num);
        log::info!("threw a weapon to get free");
    }

    /// `DangerGraspPickup`: with a free hand (the last) and no spear in hand, the
    /// nearest object in reach, favouring spears by 100 px, is grabbed and picked up
    /// (`PickedUp`'s sound). The Hunter's spear on the back (`SpearToHand`) is not ported (D3).
    pub(super) fn danger_grasp_pickup(
        &mut self,
        index: i32,
        room: &Room,
        observer: &mut dyn PlayerObserver,
    ) {
        let grasps = self.player_grasps();
        if grasps.iter().flatten().any(|&e| self.world.is_spear(e)) {
            return;
        }
        let Some(num) = (0..2).rev().find(|&i| grasps[i].is_none()) else {
            return;
        };
        let Some(obj) = self.pickup_candidate(index, room, 100.0) else {
            return;
        };
        self.slugcat_grab(obj, num, observer);
        if let Entity::Object(id) = obj {
            let sound = self.world.weapon_picked_up(id, &mut self.rng);
            self.flush_dislodged();
            if let Some(pos) = self.world.chunk_pos(obj, 0) {
                self.play_on_object(index, sound, pos);
            }
        }
    }
}

#[cfg(test)]
#[path = "game_player_grabbed_tests.rs"]
mod tests;
