//! A lizard AI's modules where they read the world and its tracker: `PreyTracker`
//! (`Update`, `TrackedPrey.Update`, `Reachable`, `PathFinderCanGetToPrey`,
//! `Attractiveness`, `Utility`, `DistanceEstimation`), `ThreatTracker` (`Update`,
//! `NewRoom`, the threat creatures, `ThreatOfTile` for the AI's callers),
//! `AgressionTracker.Update`, `DenFinder` (`NodeFinder.Update`, `Initiate`,
//! `CheckOriginalDenReachability`, `ResetMapping`, `NewArea`) and
//! `RelationshipTracker.SortCreatureIntoModule`. The tracker is out of the AI while
//! these run (the caller's `t`). Decompile: `PreyTracker.cs`, `ThreatTracker.cs`,
//! `AgressionTracker.cs`, `DenFinder.cs`, `NodeFinder.cs`, `RelationshipTracker.cs`,
//! `ArtificialIntelligence.cs` (NewArea). Behaviour: `rainworld-re/notes/lizard-ai.md`.
//!
//! Mirrors: PreyTracker.cs, ThreatTracker.cs, AgressionTracker.cs, DenFinder.cs, NodeFinder.cs, RelationshipTracker.cs (SortCreatureIntoModule, ModuleHasAbandonedCreature), ArtificialIntelligence.cs (NewArea)

use rw_assets::NodeType;
use rw_random::UnityRandom;

use super::World;
use super::abstract_path::{AbstractSpaceNodeFinder, FloodMethod, SearchingFor};
use crate::FloatExt;
use crate::creature::path_finder::EIGHT_DIRECTIONS_AND_ZERO;
use crate::creature::{
    AbstractMovement, CreatureId, DenSearch, NodeFinderStatus, PathFinder, Relationship,
    RelationshipTracker, RelationshipType, RepKey, ThreatCreature, TrackedPrey, Tracker,
    TrackingModule, WorldCoordinate,
};
use crate::room_ext::RoomExt;
use crate::vec2::TileExt;
use crate::world::ai_map::FOUR_DIRECTIONS;
use crate::world::{Legality, MovementConnection, MovementType, PathCost};

impl World {
    /// `BestGuessForPosition()` of representation `k` of AI `ai`'s tracker `t` (an
    /// undefined coordinate once it is gone).
    fn guess(&self, ai: CreatureId, t: &mut Tracker, k: RepKey) -> WorldCoordinate {
        let pf = self.path_finder(ai);
        match t.rep_mut(k) {
            Some(r) => self.rep_best_guess(ai, r, pf),
            None => WorldCoordinate::new(-1, -1, -1, -1),
        }
    }

    /// Representation `k`'s `deleteMeNextFrame` (gone counts as deleted).
    fn rep_deleted(t: &Tracker, k: RepKey) -> bool {
        t.rep(k).is_none_or(|r| r.delete_me_next_frame)
    }

    /// `DynamicRelationship(rep)`.
    fn rep_relationship(&self, ai: CreatureId, t: &Tracker, k: RepKey) -> Relationship {
        match t.rep(k) {
            Some(r) => r
                .current_relationship()
                .unwrap_or_else(|| self.static_relationship(ai, r.creature)),
            None => Relationship {
                kind: RelationshipType::Ignores,
                intensity: 0.0,
            },
        }
    }

    fn rep_chance(t: &Tracker, k: RepKey) -> f32 {
        t.rep(k).map_or(0.0, |r| t.chance_of_finding(r))
    }

    /// `room.GetTile(coord).Solid` in the AI's realized room.
    fn solid_in_room(&self, coord: WorldCoordinate) -> bool {
        self.realized(coord.room)
            .is_some_and(|r| r.room.is_solid_tile(coord.tile.0, coord.tile.1))
    }

    /// `Room.VisualContact(a, b)` of two coordinates in realized room `room`.
    fn room_visual_contact(&self, room: i32, a: WorldCoordinate, b: WorldCoordinate) -> bool {
        if a.room != room || b.room != room {
            return false;
        }
        self.realized(room)
            .is_some_and(|r| r.room.visual_contact_tiles(a.tile, b.tile))
    }

    /// `PreyTracker.DistanceEstimation(from, to, crit)`: tiles apart in one room; to a
    /// neighbouring room the creature's exit distance there while the room is ready
    /// for AI (at most 50 with sky access both sides for a creature allowed the sky
    /// highway), else 50.
    fn prey_distance_estimation(
        &self,
        from: WorldCoordinate,
        to: WorldCoordinate,
        crit: &crate::creature::CreatureTemplate,
    ) -> f32 {
        if from.room == to.room {
            return from.tile.float_dist(to.tile);
        }
        let Some(data) = self.abstract_room(from.room) else {
            return 50.0;
        };
        let exit = data.exit_index(to.room);
        if let Some(map) = self
            .realized(from.room)
            .filter(|r| r.ready_for_ai())
            .and_then(|r| r.ai_map.as_ref())
            && exit > -1
        {
            let specific = crit.specific_node_index(data, exit);
            let mut num =
                map.exit_distance_for_creature_and_check_neighbours(from.tile, specific, crit);
            if crit
                .abstract_resistance(AbstractMovement::SkyHighway)
                .allowed
                && num > -1
                && self.any_sky_access(from.room)
                && self.any_sky_access(to.room)
            {
                num = num.min(50);
            }
            if num > -1 {
                return num as f32;
            }
        }
        50.0
    }

    /// `AbstractRoom.AnySkyAccess`.
    fn any_sky_access(&self, room: i32) -> bool {
        self.abstract_room(room)
            .is_some_and(|r| r.nodes.iter().any(|n| n.kind == NodeType::SkyExit))
    }

    /// `TrackedPrey.Reachable`: 0 in a Solid tile of the AI's room; else fading with the
    /// ticks unreachable (`giveUpOnUnreachablePrey`, negative: always 1) and at its spot
    /// unseen, halved in a room less attractive than the AI's.
    pub(crate) fn prey_reachable(&self, ai: CreatureId, t: &mut Tracker, p: &TrackedPrey) -> f32 {
        let pos = self.creature(ai).pos;
        let guess = self.guess(ai, t, p.rep);
        if guess.room == pos.room && self.solid_in_room(guess) {
            return 0.0;
        }
        let l = self.lizard_ref(ai);
        let give_up = l.ai.prey.give_up_on_unreachable_prey;
        let mut num = if give_up >= 0 {
            (p.unreachable_counter as f32).inverse_lerp(give_up as f32, 0.0)
                * (p.at_position_but_cant_see_counter as f32).inverse_lerp(200.0, 100.0)
        } else {
            1.0
        };
        let kind = self.creature(ai).kind();
        if self.attraction_value(guess.room, kind) < self.attraction_value(pos.room, kind) {
            num *= 0.5;
        }
        num
    }

    /// `TrackedPrey.PathFinderCanGetToPrey`: the guess or a neighbour (eight and zero),
    /// or a tile two off (four), reachable and getbackable.
    fn prey_path_finder_can_get_to(&self, ai: CreatureId, t: &mut Tracker, k: RepKey) -> bool {
        let wc = self.guess(ai, t, k);
        let Some(pf) = self.path_finder(ai) else {
            return false;
        };
        EIGHT_DIRECTIONS_AND_ZERO
            .iter()
            .any(|&d| pf.coordinate_reachable_and_getbackable(wc.add_tile(d)))
            || FOUR_DIRECTIONS.iter().any(|&(dx, dy)| {
                pf.coordinate_reachable_and_getbackable(wc.add_tile((dx * 2, dy * 2)))
            })
    }

    /// `TrackedPrey.Attractiveness`: the relationship's intensity, halved when the path
    /// finder cannot reach the guess, get back from it, or get near it; times the
    /// chance of finding it and its reachability, the attraction of another room, over
    /// `Lerp(dist^1.5, 1, 0.5)`, and faded by the ghosts' generation. The mud colour
    /// (Watcher), Remix's other-creature catches and More Slugcats' Artificer
    /// scavengers are their DLCs'.
    fn prey_attractiveness(&self, ai: CreatureId, t: &mut Tracker, p: &TrackedPrey) -> f32 {
        let k = p.rep;
        let mut num = self.rep_relationship(ai, t, k).intensity;
        let guess = self.guess(ai, t, k);
        let pos = self.creature(ai).pos;
        let Some(crit) = t.rep(k).map(|r| self.creature(r.creature).template) else {
            return 0.0;
        };
        let mut f = self.prey_distance_estimation(pos, guess, &crit);
        f = f.mathf_pow(1.5);
        f = f.lerp(1.0, 0.5);
        if let Some(pf) = self.path_finder(ai) {
            if !pf.coordinate_reachable(guess) {
                num /= 2.0;
            }
            if !pf.coordinate_possible_to_get_back_from(guess) {
                num /= 2.0;
            }
            if !self.prey_path_finder_can_get_to(ai, t, k) {
                num /= 2.0;
            }
        }
        num *= Self::rep_chance(t, k);
        num *= self.prey_reachable(ai, t, p);
        if guess.room != pos.room {
            let kind = self.creature(ai).kind();
            num *= self
                .attraction_value(guess.room, kind)
                .inverse_lerp(0.0, 0.5);
        }
        num /= f;
        let generation = t.rep(k).map_or(0, |r| r.lowest_generation());
        num * self.lizard_ref(ai).ai.prey.ghost_generation_fac(generation)
    }

    /// `TrackedPrey.Update`: unreachable (nor a neighbour) once the path finder has
    /// mapped counts up, else resets; at the guess, heading there and seeing it, the
    /// unseen-there count rises by 5, else falls (0-200).
    fn tracked_prey_update(&self, ai: CreatureId, t: &mut Tracker, p: &mut TrackedPrey) {
        let pf = self.path_finder(ai);
        let guess = self.guess(ai, t, p.rep);
        if let Some(pf) = pf.filter(|f| f.done_mapping_accessibility()) {
            let mut ok = pf.coordinate_reachable_and_getbackable(guess);
            for &d in &FOUR_DIRECTIONS {
                if ok {
                    break;
                }
                ok = pf.coordinate_reachable_and_getbackable(guess.add_tile(d));
            }
            p.unreachable_counter = if ok { 0 } else { p.unreachable_counter + 1 };
        }
        let pos = self.creature(ai).pos;
        let at_it = p.last_best_guess_pos == guess
            && pos.room == guess.room
            && pos.tile.float_dist(guess.tile) < 5.0
            && pf.is_some_and(|f| {
                let d = f.get_destination();
                d.room == guess.room && d.tile.float_dist(guess.tile) < 5.0
            })
            && self.room_visual_contact(pos.room, pos, guess);
        if at_it {
            p.at_position_but_cant_see_counter += 5;
        } else {
            p.at_position_but_cant_see_counter -= 1;
        }
        p.at_position_but_cant_see_counter = p.at_position_but_cant_see_counter.clamp(0, 200);
        p.last_best_guess_pos = guess;
    }

    /// `PreyTracker.Update`: each prey (last first) updates and is rated (the current
    /// one by `persistanceBias` more; never forgotten by the tracker meanwhile); deleted
    /// ones go; the best becomes the current prey. The frustration rises while the
    /// current prey is in the room and the path finder cannot get to it.
    pub(crate) fn prey_tracker_update(&mut self, ai: CreatureId, t: &mut Tracker) {
        let mut prey = std::mem::take(&mut self.lizard_at(ai).ai.prey.prey);
        let (current, bias) = {
            let p = &self.lizard_ref(ai).ai.prey;
            (p.current, p.persistance_bias)
        };
        let mut best = f32::MIN;
        let mut pick = None;
        for i in (0..prey.len()).rev() {
            let mut p = prey[i].clone();
            self.tracked_prey_update(ai, t, &mut p);
            let mut a = self.prey_attractiveness(ai, t, &p);
            if let Some(r) = t.rep_mut(p.rep) {
                r.forget_counter = 0;
            }
            if Some(p.rep) == current {
                a *= bias;
            }
            if Self::rep_deleted(t, p.rep) {
                prey.remove(i);
            } else {
                prey[i] = p;
                if a > best {
                    best = a;
                    pick = Some(prey[i].rep);
                }
            }
        }
        let rising = match (pick, self.path_finder(ai)) {
            (Some(k), Some(_)) => {
                let pos = self.creature(ai).pos;
                pos.room == self.guess(ai, t, k).room && !self.prey_path_finder_can_get_to(ai, t, k)
            }
            _ => false,
        };
        let pt = &mut self.lizard_at(ai).ai.prey;
        pt.prey = prey;
        pt.current = pick;
        pt.step_frustration(rising);
    }

    /// `PreyTracker.Utility`: 0 without prey or for prey outside the room and the
    /// migration's room while migrating; else the distance term capped by the chance
    /// of finding it (faded by its ghosts' generation), times the relationship's
    /// intensity^0.75 and its reachability.
    pub(crate) fn prey_utility(&self, ai: CreatureId, t: &mut Tracker) -> f32 {
        let Some(k) = self.lizard_ref(ai).ai.prey.current else {
            return 0.0;
        };
        let Some(p) = self
            .lizard_ref(ai)
            .ai
            .prey
            .prey
            .iter()
            .find(|p| p.rep == k)
            .cloned()
        else {
            return 0.0;
        };
        let c = self.creature(ai);
        let pos = c.pos;
        let guess = self.guess(ai, t, k);
        if let Some(a) = &c.ai
            && a.want_to_migrate(pos)
            && guess.room != a.migration_target(pos).room
            && guess.room != pos.room
        {
            return 0.0;
        }
        let template = c.template;
        let pt = &self.lizard_ref(ai).ai.prey;
        let num = pt.distance_term(self.prey_distance_estimation(pos, guess, &template));
        let generation = t.rep(k).map_or(0, |r| r.lowest_generation());
        let chance = Self::rep_chance(t, k) * pt.ghost_generation_fac(generation);
        num.min(chance)
            * self.rep_relationship(ai, t, k).intensity.mathf_pow(0.75)
            * self.prey_reachable(ai, t, &p)
    }

    /// `PreyTracker.AddPrey`: once per representation; past `maxRememberedCreatures`
    /// the least attractive is abandoned (its creature forgotten) and dropped; then an
    /// `Update`.
    fn prey_add(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        k: RepKey,
    ) {
        if self.lizard_ref(ai).ai.prey.prey.iter().any(|p| p.rep == k) {
            return;
        }
        self.lizard_at(ai).ai.prey.prey.push(TrackedPrey::new(k));
        let max = self.lizard_ref(ai).ai.prey.max_remembered_creatures;
        let prey = self.lizard_ref(ai).ai.prey.prey.clone();
        if prey.len() > max {
            let mut least = f32::MAX;
            let mut worst = None;
            for p in &prey {
                let a = self.prey_attractiveness(ai, t, p);
                if a < least {
                    least = a;
                    worst = Some(p.rep);
                }
            }
            if let Some(w) = worst {
                rel.module_has_abandoned(t, w);
                let pt = &mut self.lizard_at(ai).ai.prey;
                if let Some(i) = pt.prey.iter().position(|p| p.rep == w) {
                    pt.prey.remove(i);
                }
            }
        }
        self.prey_tracker_update(ai, t);
    }

    /// `ThreatTracker.NewRoom`: its AI map is the room's, and the saved flee
    /// destination a random node of it (a draw).
    pub(crate) fn threat_tracker_new_room(
        &mut self,
        ai: CreatureId,
        room: i32,
        rng: &mut UnityRandom,
    ) {
        let nodes = self
            .abstract_room(room)
            .map_or(0, |r| i32::try_from(r.nodes.len()).unwrap_or(i32::MAX));
        let node = rng.range_i32(0, nodes);
        let th = &mut self.lizard_at(ai).ai.threat;
        th.map_room = room;
        th.saved_flee_dest = WorldCoordinate::new(room, -1, -1, node);
    }

    /// `ThreatOfTile(coord)` of AI `ai`'s threat tracker.
    #[must_use]
    pub fn threat_of_tile(&self, ai: CreatureId, coord: WorldCoordinate) -> f32 {
        let Some(l) = self.creature(ai).lizard() else {
            return 0.0;
        };
        let th = &l.ai.threat;
        let map = self.realized(th.map_room).and_then(|r| r.ai_map.as_deref());
        th.threat_of_tile(coord, self.creature(ai).pos.room, map)
    }

    /// `ThreatOfArea(coord)` of AI `ai`'s threat tracker.
    #[must_use]
    pub fn threat_of_area(&self, ai: CreatureId, coord: WorldCoordinate) -> f32 {
        let Some(l) = self.creature(ai).lizard() else {
            return 0.0;
        };
        let th = &l.ai.threat;
        let map = self.realized(th.map_room).and_then(|r| r.ai_map.as_deref());
        th.threat_of_area(coord, self.creature(ai).pos.room, map)
    }

    /// `LizardAI.TravelPreference`'s threat terms of a connection for AI `ai`: while
    /// its threat tracker senses a threat, `threat^3 * 2 * fear^0.5` of the
    /// destination tile; afraid above 0.2, 100 for a drop to climb or to the floor into
    /// an area more threatened than the start's.
    #[must_use]
    pub fn lizard_threat_travel_cost(
        &self,
        ai: CreatureId,
        connection: &MovementConnection,
        mut cost: PathCost,
    ) -> PathCost {
        let Some(l) = self.creature(ai).lizard() else {
            return cost;
        };
        let fear = l.ai.fear;
        if l.ai.threat.utility() > 0.0 {
            let f = self.threat_of_tile(ai, connection.dest);
            cost = cost
                + PathCost::new(
                    f.mathf_pow(3.0) * 2.0 * fear.mathf_pow(0.5),
                    Legality::Allowed,
                );
        }
        if fear > 0.2
            && matches!(
                connection.kind,
                MovementType::DropToClimb | MovementType::DropToFloor
            )
            && self.threat_of_area(ai, connection.dest) > self.threat_of_area(ai, connection.start)
        {
            cost = cost + PathCost::new(100.0, Legality::Allowed);
        }
        cost
    }

    /// `ThreatCreature.Update`: its severity from the relationship; in the AI's room its
    /// point is at the guess with the current threat, in a neighbouring room at the
    /// shortcut leading there with half of it, elsewhere none.
    fn threat_creature_update(&mut self, ai: CreatureId, t: &mut Tracker, i: usize) {
        let mut c = self.lizard_ref(ai).ai.threat.creatures[i];
        c.severity = self.rep_relationship(ai, t, c.rep).intensity;
        if let Some(r) = t.rep_mut(c.rep) {
            r.forget_counter = 0;
        }
        let guess = self.guess(ai, t, c.rep);
        let pos = self.creature(ai).pos;
        let current = c.current_threat(Self::rep_chance(t, c.rep), Self::rep_deleted(t, c.rep));
        let mut point = None;
        let severity = if guess.room == pos.room {
            point = Some(guess);
            current
        } else {
            let exit = self
                .abstract_room(pos.room)
                .map_or(-1, |r| r.exit_index(guess.room));
            if exit > -1 {
                point = self.realized(pos.room).and_then(|r| {
                    let map = r.room.shortcut_map();
                    let s = usize::try_from(exit)
                        .ok()
                        .and_then(|e| map.leading_to_node(e))
                        .or_else(|| map.shortcuts().first())?;
                    Some(WorldCoordinate::new(pos.room, s.start.0, s.start.1, -1))
                });
                current * 0.5
            } else {
                0.0
            }
        };
        let th = &mut self.lizard_at(ai).ai.threat;
        th.creatures[i] = c;
        if let Some(p) = th.point_mut(c.point) {
            p.severity = severity;
            if let Some(at) = point {
                p.pos = at;
            }
        }
    }

    /// `ThreatCreature.Destroy`: the relationship tracker told, its point and itself
    /// removed.
    fn threat_creature_destroy(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        rep: RepKey,
        ignore_abandon: bool,
    ) {
        if !ignore_abandon {
            rel.module_has_abandoned(t, rep);
        }
        self.lizard_at(ai).ai.threat.remove_creature(rep);
    }

    /// `ThreatTracker.Update`: the threat of the AI's area; with 200 ticks of none the
    /// threat creatures are dropped, else each updates (last first), deleted ones go,
    /// the most threatening is kept and, in the AI's room, sharpens the threat.
    pub(crate) fn threat_tracker_update(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
    ) {
        let pos = self.creature(ai).pos;
        let current = self.threat_of_area(ai, pos);
        let th = &mut self.lizard_at(ai).ai.threat;
        th.current_threat = current;
        if current <= 0.0 {
            th.reset_counter += 1;
        } else {
            th.reset_counter = 0;
        }
        if th.anti_flicker_counter > 0 {
            th.anti_flicker_counter -= 1;
        }
        if th.reset_counter >= 200 {
            for c in th.reps().into_iter().rev() {
                self.threat_creature_destroy(ai, t, rel, c, false);
            }
            return;
        }
        th.most_threatening = None;
        let mut best = 0.0;
        let mut most = None;
        for i in (0..self.lizard_ref(ai).ai.threat.creatures.len()).rev() {
            self.threat_creature_update(ai, t, i);
            let c = self.lizard_ref(ai).ai.threat.creatures[i];
            if Self::rep_deleted(t, c.rep) {
                self.threat_creature_destroy(ai, t, rel, c.rep, false);
            } else {
                let v = c.current_threat(Self::rep_chance(t, c.rep), false);
                if v > best {
                    best = v;
                    most = Some(c.rep);
                }
            }
        }
        let most_in_room = most.is_some_and(|k| self.guess(ai, t, k).room == pos.room);
        let th = &mut self.lizard_at(ai).ai.threat;
        th.most_threatening = most;
        if most_in_room {
            th.current_threat = th.current_threat.mathf_pow(1.0 / (1.0 + best * 3.0));
        }
    }

    /// `ThreatTracker.AddThreatCreature`: once per representation, with a point at its
    /// guess; past `maxRememberedCreatures` the least threatening is destroyed.
    fn threat_add(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        k: RepKey,
    ) {
        if self
            .lizard_ref(ai)
            .ai
            .threat
            .creatures
            .iter()
            .any(|c| c.rep == k)
        {
            return;
        }
        let severity = self.rep_relationship(ai, t, k).intensity;
        let crit = t.rep(k).map(|r| self.creature(r.creature).template);
        let guess = self.guess(ai, t, k);
        let th = &mut self.lizard_at(ai).ai.threat;
        let point = th.add_point(crit, guess, severity);
        th.creatures.push(ThreatCreature {
            rep: k,
            severity,
            point,
        });
        th.reset_counter = 0;
        if th.creatures.len() <= th.max_remembered_creatures {
            return;
        }
        let mut least = f32::MAX;
        let mut worst = None;
        for c in self.lizard_ref(ai).ai.threat.creatures.clone() {
            let v = c.current_threat(Self::rep_chance(t, c.rep), Self::rep_deleted(t, c.rep));
            if v < least {
                least = v;
                worst = Some(c.rep);
            }
        }
        if let Some(w) = worst {
            self.threat_creature_destroy(ai, t, rel, w, false);
        }
    }

    /// `AgressionTracker.Update`: each anger (last first) drifts toward its target
    /// (`baseAnger` near the AI, pulled toward the prey's nearness, times the room's
    /// attraction), the representation kept from being forgotten; deleted ones go, the
    /// angriest is the target.
    pub(crate) fn agression_tracker_update(&mut self, ai: CreatureId, t: &mut Tracker) {
        let pos = self.creature(ai).pos;
        let kind = self.creature(ai).kind();
        let prey = self.lizard_ref(ai).ai.prey.current;
        let prey_guess = prey.map(|k| self.guess(ai, t, k));
        let mut list = std::mem::take(&mut self.lizard_at(ai).ai.agression.creatures);
        let mut highest = 0.0;
        let mut target = None;
        for i in (0..list.len()).rev() {
            let c = &mut list[i];
            let guess = self.guess(ai, t, c.rep);
            let mut num = c.base_anger;
            num *= (pos.manhattan_distance(guess) as f32)
                .inverse_lerp(10.0 + c.base_anger * 70.0, 5.0);
            if let Some(pg) = prey_guess {
                let b = (guess.manhattan_distance(pg) as f32).inverse_lerp(10.0, 3.0);
                num = num.lerp(b, 0.5);
            }
            num *= self
                .attraction_value(guess.room, kind)
                .inverse_lerp(0.3, 0.6);
            c.anger = self.lizard_ref(ai).ai.agression.step_anger(c.anger, num);
            if let Some(r) = t.rep_mut(c.rep) {
                r.forget_counter = 0;
            }
            if Self::rep_deleted(t, c.rep) {
                list.remove(i);
            } else if list[i].anger > highest {
                highest = list[i].anger;
                target = Some(list[i].rep);
            }
        }
        let a = &mut self.lizard_at(ai).ai.agression;
        a.creatures = list;
        a.highest_anger = highest;
        a.highest_target = target;
    }

    /// `RelationshipTracker.SortCreatureIntoModule(rep, new)` of a relationship tracked
    /// by `old`: unless the module stays, the old one forgets it (its abandonment
    /// ignored), then a relationship it does not track forgets the creature, else the
    /// new module takes it (aggression starting at 0.1 toward the intensity). Returns
    /// the new module.
    pub(crate) fn sort_creature_into_module(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        k: RepKey,
        old: Option<TrackingModule>,
        new: Relationship,
    ) -> Option<TrackingModule> {
        let module = self
            .creature(ai)
            .lizard()
            .and_then(|_| TrackingModule::for_lizard(new));
        if module == old {
            return old;
        }
        let creature = t.rep(k).map(|r| r.creature);
        match old {
            Some(TrackingModule::PreyTracker) => {
                let reps: Vec<RepKey> = t
                    .reps
                    .iter()
                    .chain(&t.detached)
                    .filter(|r| Some(r.creature) == creature)
                    .map(|r| r.key)
                    .collect();
                self.lizard_at(ai).ai.prey.forget(&reps);
            }
            Some(TrackingModule::ThreatTracker) => {
                let hit = self
                    .lizard_ref(ai)
                    .ai
                    .threat
                    .creatures
                    .iter()
                    .rev()
                    .find(|c| t.rep(c.rep).map(|r| r.creature) == creature);
                if let Some(c) = hit.copied() {
                    self.threat_creature_destroy(ai, t, rel, c.rep, true);
                }
            }
            Some(TrackingModule::AgressionTracker) => {
                let a = &mut self.lizard_at(ai).ai.agression;
                if let Some(i) = a
                    .creatures
                    .iter()
                    .position(|c| t.rep(c.rep).map(|r| r.creature) == creature)
                {
                    a.creatures.remove(i);
                }
            }
            None => {}
        }
        if new.kind == RelationshipType::DoesntTrack {
            if let Some(c) = creature {
                rel.forget_creature_and_stop_tracking(t, c);
            }
            return module;
        }
        match module {
            Some(TrackingModule::PreyTracker) => self.prey_add(ai, t, rel, k),
            Some(TrackingModule::ThreatTracker) => self.threat_add(ai, t, rel, k),
            Some(TrackingModule::AgressionTracker) => {
                self.lizard_at(ai).ai.agression.add(k, 0.1, new.intensity);
            }
            None => {}
        }
        module
    }

    /// `DenFinder.Update`: the running search steps (its end assigns the den), or a
    /// working finder starts one; once the path finder has mapped, the original den is
    /// checked.
    pub(crate) fn den_finder_update(&mut self, ai: CreatureId, rng: &mut UnityRandom) {
        let df = &mut self.lizard_at(ai).ai.den_finder;
        if df.search.is_some() {
            if let Some(path) = df.step_search() {
                let den = df.finished(path.as_deref());
                if let Some(a) = self.ai_of_mut(ai) {
                    a.den_position = den;
                }
            }
        } else if df.status == NodeFinderStatus::Working {
            self.den_finder_initiate(ai, rng);
        }
        if self
            .path_finder(ai)
            .is_some_and(PathFinder::done_mapping_accessibility)
        {
            self.check_original_den_reachability(ai);
        }
    }

    /// `DenFinder.Initiate`: a cost flood for a den from the creature's position (its
    /// node defined, with draws).
    fn den_finder_initiate(&mut self, ai: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(ai);
        let t = c.template;
        let start = self.define_node(c.pos, &t, rng);
        let finder = AbstractSpaceNodeFinder {
            world: self,
            template: &t,
            searching_for: SearchingFor::Den,
            flood_method: FloodMethod::Cost,
        };
        let (path, updates) = finder.run_counted(start, rng);
        let df = &mut self.lizard_at(ai).ai.den_finder;
        df.search = Some(DenSearch {
            path,
            updates_left: updates,
        });
        df.status = NodeFinderStatus::Working;
    }

    /// `CheckOriginalDenReachability`: the spawner's den (not in a single-room world),
    /// else the creature's spawn den, is tried when the den position is another.
    pub(crate) fn check_original_den_reachability(&mut self, ai: CreatureId) {
        let c = self.creature(ai);
        let spawner = self
            .spawner(c.id)
            .map(|s| s.den())
            .map(|d| WorldCoordinate::new(d.room, -1, -1, d.node));
        // A creature not made in a den keeps `spawnDen`'s initial (-1, -1, -1, -1),
        // which `TryAssigningDen` turns down (no node).
        let spawn_den = c.spawn_den.unwrap_or(WorldCoordinate::new(-1, -1, -1, -1));
        if let Some(s) = spawner
            && self.ai_of(ai).and_then(|a| a.den_position) != Some(s)
        {
            self.try_assigning_den(ai, s);
        }
        let d = self.ai_of(ai).and_then(|a| a.den_position);
        if (spawner.is_none() || d.is_none() || d != spawner) && d != Some(spawn_den) {
            self.try_assigning_den(ai, spawn_den);
        }
    }

    /// `TryAssigningDen`: a den on a node the creature maps that its path finder can
    /// reach becomes its den (the running search dropped).
    pub(crate) fn try_assigning_den(&mut self, ai: CreatureId, den: WorldCoordinate) {
        if !den.node_defined() {
            return;
        }
        let t = self.creature(ai).template;
        let Some(kind) = self.node_kind(den) else {
            return;
        };
        if !t.maps_node(kind)
            || !self
                .path_finder(ai)
                .is_some_and(|f| f.coordinate_reachable(den))
        {
            return;
        }
        log::debug!(
            "{} found its original spawn den, resetting den position!",
            t.kind
        );
        let df = &mut self.lizard_at(ai).ai.den_finder;
        df.status = NodeFinderStatus::Found;
        df.search = None;
        if let Some(a) = self.ai_of_mut(ai) {
            a.den_position = Some(den);
        }
    }

    /// `DenFinder.ResetMapping(stranded)`: the creature's node defined (draws), the
    /// finder working again, or with no den when stranded from every exit.
    pub(crate) fn den_finder_reset_mapping(
        &mut self,
        ai: CreatureId,
        stranded: bool,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(ai);
        if !c.pos.node_defined() {
            let t = c.template;
            let pos = self.define_node(c.pos, &t, rng);
            self.creature_mut(ai).pos = pos;
        }
        self.lizard_at(ai).ai.den_finder.status = if stranded {
            NodeFinderStatus::NoAccessible
        } else {
            NodeFinderStatus::Working
        };
        if stranded {
            log::warn!(
                "{} stranded From All Exits, can't find den",
                self.creature(ai).kind()
            );
            if let Some(a) = self.ai_of_mut(ai) {
                a.den_position = None;
            }
        }
    }

    /// `ArtificialIntelligence.NewArea`: the AI is stranded or not; without a den its
    /// path finder can reach (`den_reachable`), the den finder maps again.
    pub(crate) fn ai_new_area(
        &mut self,
        id: CreatureId,
        stranded: bool,
        den_reachable: bool,
        rng: &mut UnityRandom,
    ) {
        let Some(l) = self.creature_mut(id).lizard_mut() else {
            return;
        };
        l.ai.stranded = stranded;
        if !den_reachable {
            self.den_finder_reset_mapping(id, stranded, rng);
        }
    }
}
