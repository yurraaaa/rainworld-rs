//! A lizard on the hunt and on the run: the Hunt and Flee bodies of `LizardAI.Update`,
//! `AggressiveBehavior` (the destination at the target's guess, the jaw, the lounge
//! decision, the bite after two seconds of tracking), `UnpleasantFallRisk`,
//! `FallRisk`, `BitCreature`, and the AI's `focusCreature` as the body's animations
//! read it. Decompile: `LizardAI.cs` (Update, AggressiveBehavior, UnpleasantFallRisk,
//! FallRisk, BitCreature). Behaviour: `rainworld-re/notes/lizard-ai.md` section 9.
//!
//! Mirrors: LizardAI.cs (Update, AggressiveBehavior, UnpleasantFallRisk, FallRisk, BitCreature)

use rw_assets::CreatureType;

use super::RainWorldGame;
use crate::creature::lizard::LizardEmotion;
use crate::creature::{CreatureId, LizardAnimation, RelationshipType, RepKey, WorldCoordinate};
use crate::room_ext::RoomExt;
use crate::vec2::TileExt;
use crate::{FloatExt, Vec2};

/// The AI's `focusCreature` as the body reads it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LizardFocus {
    pub creature: CreatureId,
    /// `VisualContact`.
    pub visual: bool,
    /// `representedCreature.realizedCreature`'s chunks (main first), when realized.
    pub chunks: Option<Vec<Vec2>>,
    /// The template's relationship to the focus creature's.
    pub static_relationship: RelationshipType,
}

impl LizardFocus {
    /// `realizedCreature.mainBodyChunk.pos` (and `DangerPos`: no ported creature's
    /// differs from it).
    pub fn main(&self) -> Option<Vec2> {
        self.chunks.as_ref().and_then(|c| c.first().copied())
    }
}

impl RainWorldGame {
    /// The Hunt body: `AggressiveBehavior` toward the most attractive prey (tongue
    /// chance 1), `runSpeed` toward `Max(hunger, 0.3)` by a tenth, the blood-lust voice
    /// on a 1/80 draw (not for black lizards).
    pub(super) fn lizard_hunt(&mut self, id: CreatureId) {
        if let Some(prey) = self.world.lizard_ref(id).ai.prey.most_attractive() {
            self.lizard_aggressive_behavior(id, prey, 1.0);
        }
        let ai = &mut self.world.lizard_at(id).ai;
        ai.run_speed = ai.run_speed.lerp(ai.hunger.max(0.3), 0.1);
        if self.rng.value() < 0.0125 && self.world.creature(id).kind() != CreatureType::BlackLizard
        {
            self.lizard_voice(id, LizardEmotion::BloodLust);
        }
    }

    /// The Flee body: the destination `FleeTo(pos, 5, 20, fear > 1/3)`; on a tile
    /// threatened over 1 the panic is at least 40 (else up by 2) and, on a 1/20 draw
    /// with the most threatening creature guessed within 15 tiles, `ThreatSpotted`;
    /// `runSpeed` halfway to `CombinedFear^0.1`; the focus the most threatening
    /// creature; the graphics look at the path finder's destination.
    pub(super) fn lizard_flee(&mut self, id: CreatureId) {
        let most = self.world.lizard_ref(id).ai.threat.most_threatening;
        let most_guess = most.map(|k| self.lizard_rep_guess(id, k));
        let pos = self.world.creature(id).pos;
        let leave = self.world.lizard_ref(id).ai.fear > 1.0 / 3.0;
        let dest =
            self.world
                .threat_flee_to(id, pos, 5, 20, leave, false, most_guess, &mut self.rng);
        self.world.lizard_at(id).ai.flee_to = Some(dest);
        self.world.set_destination(id, dest, &mut self.rng);
        if self.world.threat_of_tile(id, pos) > 1.0 {
            let ai = &mut self.world.lizard_at(id).ai;
            if ai.panic < 40 {
                ai.panic = 40;
            } else {
                ai.panic += 2;
            }
            if self.rng.value() < 0.05
                && most.is_some()
                && most_guess.is_some_and(|g| g.tile.float_dist(pos.tile) < 15.0)
            {
                self.lizard_enter_animation(id, LizardAnimation::ThreatSpotted, false);
            }
        }
        let pf_dest = self
            .world
            .path_finder(id)
            .map_or(pos, crate::creature::PathFinder::get_destination);
        let l = self.world.lizard_at(id);
        let to = l.ai.combined_fear().mathf_pow(0.1);
        l.ai.run_speed = l.ai.run_speed.lerp(to, 0.5);
        l.ai.focus_creature = most;
        if let Some(g) = l.graphics.as_mut() {
            g.set_look_pos = Some(Vec2::middle_of_tile(pf_dest.tile));
        }
    }

    /// `rep.BestGuessForPosition()` of lizard `id`'s representation `k`.
    fn lizard_rep_guess(&mut self, id: CreatureId, k: RepKey) -> WorldCoordinate {
        let (mut t, rel) = self.take_lizard_perception(id);
        let guess = match t.rep_mut(k) {
            Some(r) => self.rep_best_guess(id, r),
            None => WorldCoordinate::new(-1, -1, -1, -1),
        };
        self.put_lizard_perception(id, t, rel);
        guess
    }

    /// `AggressiveBehavior(target, tongueChance)`: no casual target; the destination the
    /// target's guess; the jaw eases shut while crouching, opens leaping, and otherwise
    /// opens as the seen target nears (or as hunger grows unseen); the focus the target.
    /// Seen: with the lounge delay out, a draw against `loungeTendensy` and the ground
    /// for it (no unpleasant fall here or there, no fall here, within `loungeDistance`,
    /// no narrow spaces, a green lizard on solid ground, three legs gripping), it
    /// crouches to lounge; the tongue (and `tongueChance`) is the tongued breeds',
    /// none of them ported. Tracked over 120 ticks
    /// with the jaw ready, it bites the target when a chunk of it is in reach; else
    /// more than five such frames end in a snap of the jaws.
    pub(super) fn lizard_aggressive_behavior(
        &mut self,
        id: CreatureId,
        target: RepKey,
        _tongue_chance: f32,
    ) {
        self.world.lizard_at(id).ai.casual_aggression_target = None;
        let (mut t, rel) = self.take_lizard_perception(id);
        let Some(rep) = t.rep_mut(target) else {
            self.put_lizard_perception(id, t, rel);
            return;
        };
        let guess = self.rep_best_guess(id, rep);
        let visual = self.rep_visual_contact(id, rep);
        let (age, creature) = (rep.age, rep.creature);
        self.put_lizard_perception(id, t, rel);
        self.world.set_destination(id, guess, &mut self.rng);
        let body = self.tracked_body(creature);
        let theirs: Option<Vec<Vec2>> = body.map(|b| b.chunks.iter().map(|c| c.pos).collect());
        let their_main = theirs.as_ref().and_then(|c| c.first().copied());
        let holding = self.lizard_holding(id);
        let kind = self.world.creature(id).kind();
        let l = self.world.lizard_at(id);
        let main = l.head().pos;
        match l.animation {
            LizardAnimation::PrepareToLounge => {
                let j = l.read_jaw_open(holding).lerp(0.2, 0.2);
                l.set_jaw_open(j, holding);
            }
            LizardAnimation::Lounge => {
                let j = l.read_jaw_open(holding) + 0.2;
                l.set_jaw_open(j, holding);
            }
            // No ported breed has a tongue to be out.
            LizardAnimation::ShootTongue => {
                let j = l.read_jaw_open(holding);
                let v = self.rng.value();
                self.world
                    .lizard_at(id)
                    .set_jaw_open(j.lerp(v, 0.2), holding);
            }
            _ if visual => {
                if let Some(m) = their_main {
                    let j = main
                        .distance(m)
                        .inverse_lerp(400.0, 90.0 + l.params.bite_in_front);
                    l.set_jaw_open(j, holding);
                }
            }
            _ => {
                let h = l.ai.hunger;
                let to = (h * h).inverse_lerp(0.6, 0.9) * 0.7;
                let j = l.read_jaw_open(holding).lerp(to, 0.2);
                l.set_jaw_open(j, holding);
            }
        }
        let l = self.world.lizard_at(id);
        if kind == CreatureType::RedLizard && visual {
            let j = (l.read_jaw_open(holding) + 0.1).clamp(0.0, 1.0);
            l.set_jaw_open(j, holding);
        }
        l.ai.focus_creature = Some(target);
        if !visual {
            return;
        }
        if self.world.lizard_ref(id).lounge_delay < 1
            && self.rng.value() < self.world.lizard_ref(id).params.lounge_tendensy
            && self.lizard_lounge_ground(id, creature, their_main)
        {
            self.lizard_enter_animation(id, LizardAnimation::PrepareToLounge, false);
        }
        let l = self.world.lizard_at(id);
        if age > 120 && l.jaw_ready_for_bite(holding) {
            l.ai.attempt_bite_frames += 1;
            let reach = l.params.attempt_bite_radius;
            let in_reach = theirs
                .as_ref()
                .is_some_and(|c| c.iter().any(|p| p.dist_less(main, reach)));
            if in_reach {
                self.lizard_attempt_bite(id, Some(creature));
            }
        } else {
            if l.ai.attempt_bite_frames > 5
                && let Some(m) = their_main
            {
                self.lizard_jaws_snap_shut(id, m);
            }
            self.world.lizard_at(id).ai.attempt_bite_frames = 0;
        }
    }

    /// The lounge's ground after its draw: no unpleasant fall from its tile or the
    /// focus creature's, no fall from its own, the focus within `loungeDistance`, neither
    /// tile a narrow space, a green lizard's head over solid ground, three legs gripping.
    fn lizard_lounge_ground(
        &self,
        id: CreatureId,
        focus: CreatureId,
        their_main: Option<Vec2>,
    ) -> bool {
        let pos = self.world.creature(id).pos;
        let focus_tile = self.world.creature(focus).pos.tile;
        let l = self.world.lizard_ref(id);
        let main = l.head().pos;
        let Some(r) = self.world.realized(self.lizard_room(id)) else {
            return false;
        };
        let Some(map) = r.ai_map.as_deref() else {
            return false;
        };
        !self.lizard_unpleasant_fall_risk(id, pos.tile)
            && !self.lizard_fall_risk(id, pos.tile)
            && !self.lizard_unpleasant_fall_risk(id, focus_tile)
            && their_main.is_some_and(|m| main.dist_less(m, l.params.lounge_distance))
            && !map.tile(focus_tile.0, focus_tile.1).narrow_space
            && !map.tile_at(pos).narrow_space
            && (self.world.creature(id).kind() != CreatureType::GreenLizard
                || r.room.is_solid_at(main + Vec2::new(0.0, -20.0)))
            && l.legs_gripping() > 2
    }

    /// `UnpleasantFallRisk(tile)`: the tile's fall would land in water, out of the room,
    /// more than 20 tiles down, or more than 10 down where it cannot get back from.
    pub(super) fn lizard_unpleasant_fall_risk(&self, id: CreatureId, tile: (i32, i32)) -> bool {
        let room = self.lizard_room(id);
        let Some(map) = self.world.realized(room).and_then(|r| r.ai_map.as_deref()) else {
            return false;
        };
        let fall = map.tile(tile.0, tile.1).fall_risk_tile;
        if map.any_water(fall.0, fall.1) || fall.1 < 0 || fall.1 < tile.1 - 20 {
            return true;
        }
        fall.1 < tile.1 - 10
            && !self.world.path_finder(id).is_some_and(|p| {
                p.coordinate_possible_to_get_back_from(WorldCoordinate::new(
                    room, fall.0, fall.1, -1,
                ))
            })
    }

    /// `FallRisk(tile)`: the tile's fall lands more than 5 tiles below the creature.
    fn lizard_fall_risk(&self, id: CreatureId, tile: (i32, i32)) -> bool {
        let Some(map) = self
            .world
            .realized(self.lizard_room(id))
            .and_then(|r| r.ai_map.as_deref())
        else {
            return false;
        };
        map.tile(tile.0, tile.1).fall_risk_tile.1 < self.world.creature(id).pos.tile.1 - 5
    }

    /// `BitCreature(chunk)`: unless its tile risks an unpleasant fall, it shakes the
    /// prey (forced). The obstacle tracker is never made for lizards.
    pub(super) fn lizard_bit_creature(&mut self, id: CreatureId) {
        let tile = self.world.creature(id).pos.tile;
        if !self.lizard_unpleasant_fall_risk(id, tile) {
            self.lizard_enter_animation(id, LizardAnimation::ShakePrey, true);
        }
    }

    /// The AI's `focusCreature` as the body reads it: its `VisualContact`, realized
    /// chunks and the template's relationship to it.
    pub(super) fn lizard_focus(&mut self, id: CreatureId) -> Option<LizardFocus> {
        let k = self.world.lizard_ref(id).ai.focus_creature?;
        let (mut t, rel) = self.take_lizard_perception(id);
        let seen = t
            .rep_mut(k)
            .map(|r| (r.creature, self.rep_visual_contact(id, r)));
        self.put_lizard_perception(id, t, rel);
        let (creature, visual) = seen?;
        let chunks = self
            .tracked_body(creature)
            .map(|b| b.chunks.iter().map(|c| c.pos).collect());
        let static_relationship = self
            .world
            .creature(id)
            .template
            .relationship_to(self.world.creature(creature).kind())
            .kind;
        Some(LizardFocus {
            creature,
            visual,
            chunks,
            static_relationship,
        })
    }
}

#[cfg(test)]
#[path = "game_lizard_hunt_tests.rs"]
mod tests;
