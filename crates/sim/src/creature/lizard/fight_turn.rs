//! Lizards among themselves: the Fighting behaviour (the angriest rival as the focus,
//! the fighting stance, the aggression signal, `AggressiveBehavior` at it), the signals
//! (`SendCommunication`, `RecieveCommunication`: dominance and submission, the anger
//! they raise or settle, the submitter's mission to leave the room), a rival's anger
//! on touch (`Lizard.Collide`), and the ActingOutMission behaviour. Behaviour:
//! `rainworld-re/notes/lizard-ai.md` section 11.
//!
//! Mirrors: LizardAI.cs (Update's Fighting and ActingOutMission bodies, SendCommunication, RecieveCommunication), Lizard.cs (Collide's rival anger), MissionTracker.cs (ActOnMostImportantMission)

use crate::FloatExt;
use crate::creature::lizard::{LizardCommunication, LizardEmotion};
use crate::creature::{
    CreatureId, LizardAnimation, Mission, MissionData, RelationshipType, RepKey,
};
use crate::game::RainWorldGame;

impl RainWorldGame {
    /// The Fighting body: the focus is the aggression target. Seen within 160 px and
    /// (a lizard) nearer its head than its tail, a 1/20 draw takes the fighting stance
    /// and `runSpeed` eases toward 0.2; otherwise toward 0.8 with a 1/180 draw for the
    /// stance. At a realized lizard a 1/20 draw, then one under the relationship's
    /// intensity, sends it the aggression signal; then `AggressiveBehavior` (tongue
    /// chance 0.025).
    pub(crate) fn lizard_fighting(&mut self, id: CreatureId) {
        let target = self.world.lizard_ref(id).ai.agression.target();
        self.world.lizard_at(id).ai.focus_creature = target;
        let Some(k) = target else { return };
        let Some(focus) = self.lizard_focus(id) else {
            return;
        };
        let other = focus.creature;
        let is_lizard = self.world.creature(other).lizard().is_some();
        let main = self.world.lizard_ref(id).head().pos;
        let close = focus.visual
            && focus.chunks.as_ref().is_some_and(|c| {
                main.dist_less(c[0], 160.0)
                    && (!is_lizard || main.distance(c[0]) < main.distance(c[2]))
            });
        if close {
            if self.rng.value() < 0.05 {
                self.lizard_enter_animation(id, LizardAnimation::FightingStance, false);
            }
            let ai = &mut self.world.lizard_at(id).ai;
            ai.run_speed = ai.run_speed.lerp(0.2, 0.15);
        } else {
            let ai = &mut self.world.lizard_at(id).ai;
            ai.run_speed = ai.run_speed.lerp(0.8, 0.15);
            if self.rng.value() < 1.0 / 180.0 {
                self.lizard_enter_animation(id, LizardAnimation::FightingStance, false);
            }
        }
        if is_lizard
            && self.rng.value() < 0.05
            && self.rng.value() < self.world.dynamic_relationship(id, other).intensity
        {
            self.lizard_send_communication(id, other, LizardCommunication::Agression);
        }
        self.lizard_aggressive_behavior(id, k, 0.025);
    }

    /// `SendCommunication(otherLizard, signal)`: only to a living lizard in its room.
    /// It looks at the other; with a graphics module its dominance display grows
    /// (aggression) or shrinks (submission) by a draw; it voices dominance or
    /// submission; the other receives the signal.
    pub(crate) fn lizard_send_communication(
        &mut self,
        id: CreatureId,
        other: CreatureId,
        signal: LizardCommunication,
    ) {
        let my_room = self.world.lizard_ref(id).room;
        let Some(o) = self.world.creature(other).lizard() else {
            return;
        };
        if o.dead || o.room != my_room {
            return;
        }
        let rep = self.lizard_rep_key(id, other);
        let has_graphics = self.world.lizard_ref(id).graphics.is_some();
        self.world.lizard_at(id).ai.focus_creature = rep;
        let (sign, emotion) = match signal {
            LizardCommunication::Agression => (1.0, LizardEmotion::Dominance),
            LizardCommunication::Submission => (-1.0, LizardEmotion::Submission),
        };
        if has_graphics {
            let v = self.rng.value();
            if let Some(g) = self.world.lizard_at(id).graphics.as_mut() {
                g.show_dominance_add += sign * v;
            }
        }
        self.lizard_voice(id, emotion);
        self.world.lizard_at(id).ai.last_signal = Some((signal, other, self.tick));
        self.lizard_recieve_communication(other, id, signal);
    }

    /// `RecieveCommunication(otherLizard, signal)`, the game's spelling: nothing unless
    /// it sees the sender's head; it looks at the sender. Aggression from one that has
    /// not submitted to it angers it by 0.1 (unless it submitted to that one), and on a
    /// 1/24 draw it submits: the submission signal back, its anger at the sender and
    /// base anger 0, the sender remembered as the one submitted to, and a mission to
    /// leave the room (0.4, forgotten after 1000 ticks). Submission settles its anger
    /// at the sender to 0, the base kept.
    pub(crate) fn lizard_recieve_communication(
        &mut self,
        id: CreatureId,
        sender: CreatureId,
        signal: LizardCommunication,
    ) {
        if self.world.creature(id).lizard().is_none() {
            return;
        }
        let head = self.world.lizard_ref(sender).head().clone();
        if !self.ai_sees_chunk(id, &head, 0.0) {
            return;
        }
        let rep = self.lizard_rep_key(id, sender);
        self.world.lizard_at(id).ai.focus_creature = rep;
        match signal {
            LizardCommunication::Agression => {
                if self.world.lizard_ref(sender).ai.submitted_to == Some(id) {
                    return;
                }
                if self.world.lizard_ref(id).ai.submitted_to != Some(sender) {
                    self.world
                        .lizard_at(id)
                        .ai
                        .agression
                        .increment_anger(rep, 0.1);
                }
                if self.rng.value() < 1.0 / 24.0 {
                    self.lizard_send_communication(id, sender, LizardCommunication::Submission);
                    let rep = self.lizard_rep_key(id, sender);
                    self.world
                        .lizard_at(id)
                        .ai
                        .agression
                        .set_anger(rep, 0.0, 0.0);
                    if rep.is_none() {
                        // D14: the game reads `representedCreature` of a null
                        // representation here and throws (NullReference).
                        log::warn!(
                            "RecieveCommunication: no representation of the sender, the game throws here"
                        );
                        return;
                    }
                    let single = self.world.single_room();
                    let ai = &mut self.world.lizard_at(id).ai;
                    ai.submitted_to = Some(sender);
                    ai.missions
                        .add(Mission::leave_room(0.4, 1000, single), false);
                }
            }
            LizardCommunication::Submission => {
                self.world
                    .lizard_at(id)
                    .ai
                    .agression
                    .set_anger(rep, 0.0, -1.0);
            }
        }
    }

    /// `Lizard.Collide`'s rivalry: touching an aggressive rival it has not submitted to
    /// angers it by `0.05 * intensity` (after it has seen it).
    pub(crate) fn lizard_rival_touch(&mut self, id: CreatureId, other: CreatureId) {
        let rel = self.world.dynamic_relationship(id, other);
        if rel.kind != RelationshipType::AgressiveRival
            || self.world.lizard_ref(id).ai.submitted_to == Some(other)
        {
            return;
        }
        let rep = self.lizard_rep_key(id, other);
        self.world
            .lizard_at(id)
            .ai
            .agression
            .increment_anger(rep, 0.05 * rel.intensity);
    }

    /// The ActingOutMission body: the most relevant mission acts, and `runSpeed` eases
    /// toward 0.6 by a twentieth.
    pub(crate) fn lizard_acting_out_mission(&mut self, id: CreatureId) {
        let current = self
            .world
            .lizard_ref(id)
            .ai
            .missions
            .most_relevant()
            .copied();
        if let Some(mut m) = current {
            let abandon = match &mut m.data {
                MissionData::LeaveRoom(lr) => self.world.leave_room_act(id, lr, &mut self.rng),
            };
            if abandon {
                m.abandon();
            }
            if let Some(slot) = self.world.lizard_at(id).ai.missions.most_relevant_mut() {
                *slot = m;
            }
        }
        let ai = &mut self.world.lizard_at(id).ai;
        ai.run_speed = ai.run_speed.lerp(0.6, 0.05);
    }

    /// `tracker.RepresentationForObject(other, AddIfMissing: false)` of lizard `id`.
    fn lizard_rep_key(&self, id: CreatureId, other: CreatureId) -> Option<RepKey> {
        self.world
            .lizard_ref(id)
            .ai
            .tracker
            .representation_for(other)
            .map(|r| r.key)
    }
}

#[cfg(test)]
#[path = "tests/fight_turn.rs"]
mod tests;
