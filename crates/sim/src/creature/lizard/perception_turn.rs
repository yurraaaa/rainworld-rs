//! What `LizardAI` adds to the shared tracker and relationship tracker
//! (`creature/tracker_turn.rs`): its eyes (`VisualScore` with the angle off its head), the
//! representation it makes of a creature, `CreatureSpotted` (its graphics' looker and
//! eyes, the spotted animations),
//! `CreateTrackedCreatureState`, `UpdateDynamicRelationship`, the casual aggression of
//! its `Update` and `Lizard.Collide`'s `SeeCreature` and temporary dislike. Decompile:
//! `LizardAI.cs` 1084-1095, 1184-1192, 1317-1488, 1667-1683; `Lizard.cs` 1408-1432;
//! `LizardGraphics.cs` 1474-1489.
//!
//! Mirrors: LizardAI.cs, Lizard.cs (Collide), LizardGraphics.cs

use rw_assets::{AbstractObjectType, CreatureType};

use crate::creature::ai_modules::tracker_turn::TrackedBody;
use crate::creature::lizard::SpottedEye;
use crate::creature::{
    CreatureId, CreatureRep, ElaborateRep, Eyes, LizardAnimation, Relationship,
    RelationshipTracker, RelationshipType, RepKey, RepKind, TrackedCreatureState, Tracker,
};
use crate::game::RainWorldGame;
use crate::item::Entity;
use crate::{FloatExt, Vec2};

impl RainWorldGame {
    /// The eyes of AI `ai`.
    pub(crate) fn ai_eyes(&self, ai: CreatureId) -> Option<Eyes> {
        self.world.ai_eyes(ai)
    }

    /// `AI.creature.realizedCreature.mainBodyChunk.pos`.
    pub(crate) fn ai_main_pos(&self, ai: CreatureId) -> Vec2 {
        self.world.ai_main_pos(ai)
    }

    /// Body chunks 0 and 1 (`CreatureInterestBonus`).
    pub(crate) fn ai_head_and_neck(&self, ai: CreatureId) -> (Vec2, Vec2) {
        self.world
            .creature(ai)
            .lizard()
            .map_or((Vec2::ZERO, Vec2::ZERO), |l| {
                (l.body.chunks[0].pos, l.body.chunks[1].pos)
            })
    }

    /// The representations AI `ai` and its graphics refer to.
    pub(crate) fn ai_held_reps(&self, ai: CreatureId) -> Vec<RepKey> {
        self.world.ai_held_reps(ai)
    }

    /// `CreateTrackerRepresentationForCreature`: a lizard keeps a simple representation
    /// of a small creature (not forgotten out of sight) and an elaborate one with up to
    /// 5 ghosts of the others (the first ghost draws once).
    pub(crate) fn create_tracker_rep(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        c: CreatureId,
        body: &TrackedBody,
    ) -> CreatureRep {
        let key = t.next_key();
        let at = self.world.creature(c).pos;
        let small = self.world.creature(c).template.small_creature;
        if self.world.creature(ai).lizard().is_none() {
            return CreatureRep::new(
                key,
                c,
                at,
                0.0,
                RepKind::Simple {
                    forget_when_not_visible: true,
                },
            );
        }
        if small {
            return CreatureRep::new(
                key,
                c,
                at,
                0.0,
                RepKind::Simple {
                    forget_when_not_visible: false,
                },
            );
        }
        let e = ElaborateRep::new(
            at,
            body.main().pos,
            body.main().vel,
            5,
            t.ghosts,
            &mut self.rng,
        );
        CreatureRep::new(key, c, at, 1.0, RepKind::Elaborate(e))
    }

    /// `CreateTrackedCreatureState`: a lizard's `LizardTrackState` of a slugcat.
    pub(crate) fn ai_tracked_creature_state(
        &self,
        ai: CreatureId,
        c: CreatureId,
    ) -> Option<TrackedCreatureState> {
        self.world.creature(ai).lizard()?;
        (self.world.creature(c).kind() == CreatureType::Slugcat)
            .then(|| TrackedCreatureState::Lizard(crate::creature::LizardTrackState::default()))
    }

    /// `CreatureSpotted(firstSpot, rep)` of AI `ai` (`rep` may not be listed yet): the
    /// lizard's graphics look again, favouring it (2, or 4 when seen again), and flash
    /// an eye on it (one draw for which) when the lizard eats it, cares more than half,
    /// or looks at nothing; a creature it goes for (over 0.2, or 0.45 again) and does
    /// not hold makes it enter `PreySpotted` (or `PreyReSpotted`). `HeadLightsUpFromNoise`
    /// is the black lizard's.
    pub(crate) fn ai_creature_spotted(
        &mut self,
        ai: CreatureId,
        first: bool,
        rep: &CreatureRep,
        t: &mut Tracker,
    ) {
        let Some(l) = self.world.creature(ai).lizard() else {
            return;
        };
        if let Some(link) = &l.graphics {
            let looker = link.looker;
            let mut looker_after = looker;
            if let Some(mut lk) = looker {
                let cands = self.look_candidates(ai, t, lk.look_creature);
                let at = self.world.creature(ai).pos;
                let bonus = if first { 2.0 } else { 4.0 };
                lk.reevaluate(&cands, Some(rep.key), bonus, at, &mut self.rng);
                self.apply_look_reads(ai, t, &cands);
                looker_after = Some(lk);
            }
            let rel = self
                .world
                .creature(ai)
                .template
                .relationship_to(self.world.creature(rep.creature).kind());
            let looks_at_nothing = looker_after.is_some_and(|lk| lk.look_creature.is_none());
            let mut spotted = None;
            if rel.kind == RelationshipType::Eats || rel.intensity > 0.5 || looks_at_nothing {
                let eye = self.rng.range_i32(0, 2) as usize;
                spotted = self.tracked_body(rep.creature).map(|b| SpottedEye {
                    eye,
                    pos: b.main().pos,
                });
            }
            if let Some(link) = self.world.lizard_at(ai).graphics.as_mut() {
                link.looker = looker_after;
                link.spotted.extend(spotted);
            }
        }
        let rel = rep
            .current_relationship()
            .unwrap_or_else(|| self.world.static_relationship(ai, rep.creature));
        let holds = self
            .world
            .creature(ai)
            .grasps
            .first()
            .and_then(Option::as_ref)
            .is_some_and(|g| g.grabbed == Entity::Creature(rep.creature));
        if rel.go_for_kill() && rel.intensity > if first { 0.2 } else { 0.45 } && !holds {
            let anim = if first {
                LizardAnimation::PreySpotted
            } else {
                LizardAnimation::PreyReSpotted
            };
            self.lizard_enter_animation(ai, anim, false);
        }
    }

    /// `UpdateDynamicRelationship` of AI `ai` for representation `i`: a lizard's
    /// relationship to a creature other than a slugcat is the static one; to a slugcat
    /// in sight it notes whether a spear is held (vulture masks are not ported, so the
    /// mask's fear never applies), then eats it with
    /// `Pow(InverseLerp(0.5, -1, like), aggressionCurveExponent)` while its like of the
    /// player is under 0.5, else ignores it. An active gift offered to it is eaten
    /// (1 dead, else 0.65). The Remix, Jolly and Watcher branches are not ported.
    pub(crate) fn ai_update_dynamic_relationship(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        i: usize,
    ) -> Relationship {
        let c = t.reps[i].creature;
        if let Some(gift) = self
            .world
            .creature(ai)
            .lizard()
            .and_then(|l| l.ai.friend.gift_offered_to_me)
            && gift.item == Entity::Creature(c)
            && self.world.gift_active(&gift)
        {
            let dead = !self.world.creature(c).state.alive;
            return Relationship {
                kind: RelationshipType::Eats,
                intensity: if dead { 1.0 } else { 0.65 },
            };
        }
        if self.world.creature(c).kind() != CreatureType::Slugcat
            || self.world.creature(ai).lizard().is_none()
        {
            return self.world.static_relationship(ai, c);
        }
        let has_state = t.reps[i]
            .dynamic_relationship
            .as_ref()
            .is_some_and(|d| d.state.is_some());
        if has_state {
            if self.rep_visual_contact(ai, &mut t.reps[i]) && self.tracked_body(c).is_some() {
                let spear = self.world.creature(c).grasps.iter().flatten().any(|g| {
                    matches!(g.grabbed, Entity::Object(o)
                        if self.world.object(o).kind == AbstractObjectType::Spear)
                });
                if let Some(s) = t.reps[i]
                    .dynamic_relationship
                    .as_mut()
                    .and_then(|d| d.lizard_state_mut())
                {
                    s.spear = spear;
                    s.vulture_mask = 0;
                }
            }
            let mask = t.reps[i]
                .dynamic_relationship
                .as_mut()
                .and_then(|d| d.lizard_state_mut())
                .map_or(0, |s| s.vulture_mask);
            if let Some(r) = self.lizard_vulture_mask_fear(ai, mask) {
                return r;
            }
        }
        let like = self.world.lizard_like_of_player(ai, Some(&t.reps[i]));
        if like < 0.5 {
            let exp = self.world.lizard_ref(ai).params.aggression_curve_exponent;
            Relationship {
                kind: RelationshipType::Eats,
                intensity: like.inverse_lerp(0.5, -1.0).mathf_pow(exp),
            }
        } else {
            Relationship {
                kind: RelationshipType::Ignores,
                intensity: 0.0,
            }
        }
    }

    /// The vulture mask's fear (`usedToVultureMask` counting up to 700, a king's 1200),
    /// for every breed but black and red lizards; a green one ignores a plain mask.
    fn lizard_vulture_mask_fear(&mut self, ai: CreatureId, mask: i32) -> Option<Relationship> {
        let kind = self.world.creature(ai).kind();
        let limit = if mask == 2 { 1200 } else { 700 };
        let l = self.world.lizard_at(ai);
        if mask <= 0
            || matches!(kind, CreatureType::BlackLizard | CreatureType::RedLizard)
            || l.ai.used_to_vulture_mask >= limit
        {
            return None;
        }
        l.ai.used_to_vulture_mask += 1;
        if kind == CreatureType::GreenLizard && mask < 2 {
            return Some(Relationship {
                kind: RelationshipType::Ignores,
                intensity: 0.0,
            });
        }
        let fac = match (mask == 2, kind) {
            (false, CreatureType::BlueLizard) => 0.8,
            (false, _) => 0.6,
            (true, CreatureType::GreenLizard) => 0.4,
            (true, _) => 0.9,
        };
        Some(Relationship {
            kind: RelationshipType::Afraid,
            intensity: (l.ai.used_to_vulture_mask as f32).inverse_lerp(limit as f32, 600.0) * fac,
        })
    }

    /// The tracker and relationship tracker of lizard `id`, taken out.
    pub(crate) fn take_lizard_perception(
        &mut self,
        id: CreatureId,
    ) -> (Tracker, RelationshipTracker) {
        let l = self.world.lizard_at(id);
        (
            std::mem::replace(&mut l.ai.tracker, Tracker::lizard()),
            std::mem::take(&mut l.ai.relationships),
        )
    }

    /// Puts them back, dropping the representations nothing holds any more.
    pub(crate) fn put_lizard_perception(
        &mut self,
        id: CreatureId,
        mut t: Tracker,
        rel: RelationshipTracker,
    ) {
        let held = self.ai_held_reps(id);
        t.collect_detached(&held);
        let l = self.world.lizard_at(id);
        l.ai.tracker = t;
        l.ai.relationships = rel;
    }

    /// `LizardAI.Update`'s casual aggression: with its jaws free, a random tracked
    /// creature in sight it wants to bite (or, as health fails, any) becomes the target
    /// when closer than the last; then half the time a target in sight within
    /// `attemptBiteRadius` is bitten at (`AttemptBite`).
    pub(crate) fn lizard_casual_aggression(&mut self, id: CreatureId) {
        let holding = self.lizard_holding(id);
        let (mut t, rel) = self.take_lizard_perception(id);
        if !holding && !t.reps.is_empty() {
            let i = self.rng.range_i32(0, t.reps.len() as i32) as usize;
            let health = self.world.creature(id).state.health;
            let want = self.lizard_wants_to_bite(id, &t.reps[i], health)
                || (self.rng.value() > health && self.rng.value() < 0.05);
            if want && self.rep_visual_contact(id, &mut t.reps[i]) {
                let me = self.ai_main_pos(id);
                let theirs = self.tracked_body(t.reps[i].creature).map(|b| b.main().pos);
                let current = self
                    .world
                    .lizard_ref(id)
                    .ai
                    .casual_aggression_target
                    .and_then(|k| t.rep(k))
                    .and_then(|r| self.tracked_body(r.creature))
                    .map(|b| b.main().pos);
                let closer = match (current, theirs) {
                    (Some(cur), Some(th)) => me.distance(th) < me.distance(cur),
                    _ => true,
                };
                if closer {
                    self.world.lizard_at(id).ai.casual_aggression_target = Some(t.reps[i].key);
                }
            }
        }
        let mut bite = None;
        if let Some(k) = self.world.lizard_ref(id).ai.casual_aggression_target
            && self.rng.value() < 0.5
            && let Some(r) = t.rep_mut(k)
        {
            let c = r.creature;
            if self.rep_visual_contact(id, r)
                && let Some(b) = self.tracked_body(c)
                && b.main().pos.dist_less(
                    self.ai_main_pos(id),
                    self.world.lizard_ref(id).params.attempt_bite_radius,
                )
            {
                bite = Some(c);
            }
        }
        self.put_lizard_perception(id, t, rel);
        if let Some(c) = bite {
            self.lizard_attempt_bite(id, Some(c));
        }
    }

    /// `DoIWantToBiteThisCreature`: a tenth of the time as health fails (two draws, the
    /// second only on the first's tenth), anything it goes for, or half the time a rival.
    fn lizard_wants_to_bite(&mut self, id: CreatureId, rep: &CreatureRep, health: f32) -> bool {
        if self.rng.value() < 0.1 && self.rng.value() > health {
            return true;
        }
        let rel = rep
            .current_relationship()
            .unwrap_or_else(|| self.world.static_relationship(id, rep.creature));
        if rel.go_for_kill() {
            return true;
        }
        self.rng.value() < 0.5 && rel.kind == RelationshipType::AgressiveRival
    }

    /// `Lizard.Collide`'s tracking: it sees the creature (`SeeCreature`), and touching a
    /// player it dislikes (and temporarily likes above -0.5) lowers its temporary like
    /// by 0.05; touching a rival angers it (`lizard_rival_touch`).
    pub(crate) fn lizard_collide_tracking(&mut self, me: CreatureId, other: CreatureId) {
        let (mut t, mut rel) = self.take_lizard_perception(me);
        self.tracker_see_creature(me, &mut t, &mut rel, other);
        self.put_lizard_perception(me, t, rel);
        self.lizard_rival_touch(me, other);
        if other != self.player_id {
            return;
        }
        let like = {
            let t = &self.world.lizard_ref(me).ai.tracker;
            self.world
                .lizard_like_of_player(me, t.representation_for(other))
        };
        let subject = self.world.creature(other).id;
        let c = self.world.creature_mut(me);
        if let Some(m) = &mut c.state.social_memory
            && like < 0.5
            && m.temp_like(subject) > -0.5
        {
            m.get_or_initiate(subject).influence_temp_like(-0.05);
        }
    }

    /// The looker's candidates of lizard `id` for its graphics' update, its forced look
    /// creature (`focusCreature`) and where it is.
    pub(crate) fn lizard_look_view(
        &self,
        id: CreatureId,
    ) -> (Vec<crate::creature::LookCandidate>, Option<RepKey>) {
        let l = self.world.lizard_ref(id);
        let look = l
            .graphics
            .as_ref()
            .and_then(|g| g.looker)
            .and_then(|lk| lk.look_creature);
        (
            self.look_candidates(id, &l.ai.tracker, look),
            l.ai.focus_creature,
        )
    }

    /// Applies the graphics' reads of the tracker (`apply_look_reads`).
    pub(crate) fn lizard_apply_look_reads(
        &mut self,
        id: CreatureId,
        candidates: &[crate::creature::LookCandidate],
    ) {
        let (mut t, rel) = self.take_lizard_perception(id);
        self.apply_look_reads(id, &mut t, candidates);
        self.put_lizard_perception(id, t, rel);
    }
}

#[cfg(test)]
#[path = "tests/perception_turn.rs"]
pub(crate) mod tests;
