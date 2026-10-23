//! A realized lizard's AI as it runs: `LizardAI.Update` (the clocks, the abstract
//! behaviour roll, the moods from the utility comparer, `DetermineBehavior`, the
//! behaviours' bodies, `runSpeed`, the hearing skill, the casual aggression) and the
//! base `ArtificialIntelligence.Update` with its modules in the constructor's order;
//! `TravelUtility`, `RoomLike`, `IdleSpotScore` and `ComfortableIdlePosition`.
//! Hunt and Flee are `creature/lizard/hunt_turn.rs`; Fighting and ActingOutMission
//! `creature/lizard/fight_turn.rs`. Decompile: `LizardAI.cs` (Update, DetermineBehavior, TravelUtility,
//! RoomLike, IdleSpotScore, ComfortableIdlePosition), `ArtificialIntelligence.cs`
//! (Update). Behaviour: `rainworld-re/notes/lizard-ai.md`.
//!
//! Mirrors: LizardAI.cs (Update, DetermineBehavior, TravelUtility, RoomLike, IdleSpotScore, ComfortableIdlePosition), ArtificialIntelligence.cs (Update)

use rw_assets::CreatureType;

use crate::creature::FloatTween;
use crate::creature::lizard::{LizardBehavior, LizardEmotion};
use crate::creature::{
    CreatureId, LizardAnimation, RainTracker, RawUtilities, RelationshipType, Tracker,
    UtilityModule, WorldCoordinate,
};
use crate::game::RainWorldGame;
use crate::physics::room_ext::RoomExt;
use crate::world::Accessibility;
use crate::{FloatExt, Vec2};

/// A lizard's utility comparer as read now.
#[derive(Debug, Clone, PartialEq)]
pub struct UtilityReadings {
    /// Each module, its smoothed utility and weight.
    pub modules: Vec<(UtilityModule, f32, f32)>,
    pub highest: Option<UtilityModule>,
    pub highest_utility: f32,
    /// `TravelUtility`.
    pub travel: f32,
}

impl RainWorldGame {
    /// `LizardAI.Update`. A lizard that fled last update runs it (the base AI's modules
    /// too) only when `RequestHeavyAi` grants it. A dev run speed (D18) replaces the
    /// behaviours' bodies.
    pub(crate) fn lizard_ai_update(&mut self, index: i32, id: CreatureId) {
        let granted = (self.world.lizard_ref(id).ai.behavior == LizardBehavior::Flee)
            .then(|| self.heavy_ai.request(self.world.creature(id).id));
        self.world.lizard_at(id).ai.heavy_ai_granted = granted;
        if granted == Some(false) {
            return;
        }
        let l = self.world.lizard_at(id);
        if l.ai.panic > 0 {
            l.ai.panic -= 1;
        }
        l.ai.lizard_time_in_room += 1;
        let kind = self.world.creature(id).kind();
        let chance = if kind == CreatureType::RedLizard {
            50.0
        } else {
            self.lizard_room_like(index, id)
                .lerp_map(0.35, 1.0, 10.0, 1000.0)
        };
        if self.rng.value() < 1.0 / chance {
            self.world.realized_abstract_behavior(id, 1, &mut self.rng);
        }
        if let Some(m) = &mut self.world.creature_mut(id).state.social_memory {
            m.even_out_all_temps(0.0005);
        }
        let pf = self.world.path_finder_of(id);
        let den = self.world.den_position_of(id);
        let stranded = self.world.lizard_ref(id).ai.stranded;
        let walk_past = stranded
            || den.is_none_or(|d| {
                !pf.and_then(|p| self.world.path_finders.get(p))
                    .is_some_and(|f| f.coordinate_possible_to_get_back_from(d))
            });
        if let Some(f) = pf.and_then(|p| self.world.path_finders.get_mut(p)) {
            f.walk_past_point_of_no_return = walk_past;
        }
        let raw = self.lizard_utilities_now(id);
        let health = self.world.creature(id).state.health;
        let story = self.world.story;
        let ai = &mut self.world.lizard_at(id).ai;
        ai.fear = ai
            .utility
            .smoothed_non_weighted(UtilityModule::ThreatTracker, &raw);
        ai.hunger = ai
            .utility
            .smoothed_non_weighted(UtilityModule::PreyTracker, &raw);
        ai.rain_fear = ai
            .utility
            .smoothed_non_weighted(UtilityModule::RainTracker, &raw);
        ai.excitement = ai.excitement.lerp(ai.hunger.max(ai.combined_fear()), 0.1);
        if ai.fear > 0.8 {
            ai.last_distress_length = (ai.last_distress_length + 1).clamp(0, 500);
        } else if ai.fear < 0.2 {
            ai.last_distress_length = 0;
        }
        let down = 1.0 / ((ai.last_distress_length + 20) as f32 * 3.0);
        if let Some(FloatTween::UpAndDown { down: d, .. }) = ai
            .utility
            .tracker_mut(UtilityModule::ThreatTracker)
            .and_then(|t| t.smoother.as_mut())
        {
            d.speed = down;
        }
        if let Some(t) = ai.utility.tracker_mut(UtilityModule::AgressionTracker) {
            t.weight = if story { 0.25 } else { 0.125 } * health;
        }
        // `friendTracker.CareAboutRain()`: true without a friend (taming is skipped).
        if let Some(t) = ai.utility.tracker_mut(UtilityModule::RainTracker) {
            t.weight = 0.9;
        }
        let behavior = self.lizard_determine_behavior(index, id, &raw);
        self.world.lizard_at(id).ai.behavior = behavior;
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_at(id);
        let jaw = l.read_jaw_open(holding) - 0.01;
        l.set_jaw_open(jaw, holding);
        match self.world.lizard_ref(id).ai.dev_run_speed {
            Some(s) => self.world.lizard_at(id).ai.run_speed = s,
            None => self.lizard_behave(index, id, behavior),
        }
        let ai = &mut self.world.lizard_at(id).ai;
        if behavior != LizardBehavior::Idle && kind == CreatureType::RedLizard {
            ai.run_speed = ai.run_speed.mathf_pow(0.7);
        }
        ai.run_speed = ai.run_speed.max(ai.stuck.utility());
        ai.set_hearing_skill(kind);
        self.lizard_casual_aggression(id);
        self.lizard_base_ai_update(id);
    }

    /// The behaviour's body.
    fn lizard_behave(&mut self, index: i32, id: CreatureId, behavior: LizardBehavior) {
        match behavior {
            LizardBehavior::Idle => self.lizard_idle(index, id),
            LizardBehavior::Travelling => {
                let travel = self.lizard_travel_utility(index, id);
                let ai = &mut self.world.lizard_at(id).ai;
                ai.run_speed = ai.run_speed.lerp(0.5 + 0.5 * travel, 0.01);
                let c = self.world.creature(id);
                if let Some(dest) = c.ai.as_ref().map(|a| a.migration_target(c.pos)) {
                    self.world.set_destination(id, dest, &mut self.rng);
                }
            }
            LizardBehavior::EscapeRain => {
                self.lizard_go_to_den(id);
                let ai = &mut self.world.lizard_at(id).ai;
                ai.run_speed = ai.run_speed.lerp(1.0, 0.1);
            }
            LizardBehavior::ReturnPrey => {
                self.lizard_go_to_den(id);
                let ai = &mut self.world.lizard_at(id).ai;
                let to = ai.combined_fear().max(0.5).mathf_pow(0.5);
                ai.run_speed = ai.run_speed.lerp(to, 0.1);
            }
            LizardBehavior::Injured => {
                self.lizard_go_to_den(id);
                let ai = &mut self.world.lizard_at(id).ai;
                ai.run_speed = ai.run_speed.lerp(1.0, 0.01);
                if self.rng.value() < 0.0125 {
                    let health = self.world.creature(id).state.health;
                    self.lizard_voice_with(id, LizardEmotion::PainIdle, 1.0 - health);
                }
            }
            LizardBehavior::Frustrated => self.lizard_frustrated(index, id),
            LizardBehavior::InvestigateSound => {
                let n = &self.world.lizard_ref(id).ai.noise;
                if let Some(pos) = n.sound_to_examine().map(|s| s.pos) {
                    let (x, y) = pos.tile();
                    let dest = WorldCoordinate::new(n.room, x, y, -1);
                    self.world.set_destination(id, dest, &mut self.rng);
                }
                let ai = &mut self.world.lizard_at(id).ai;
                ai.run_speed = ai.run_speed.lerp(0.6, 0.2);
            }
            LizardBehavior::Hunt => self.lizard_hunt(id),
            LizardBehavior::Flee => self.lizard_flee(id),
            LizardBehavior::Fighting => self.lizard_fighting(id),
            LizardBehavior::ActingOutMission => self.lizard_acting_out_mission(id),
            // Other breeds' (Lurk, GoToSpitPos) and taming's (FollowFriend): a
            // stand-in no green lizard reaches.
            LizardBehavior::Lurk | LizardBehavior::GoToSpitPos | LizardBehavior::FollowFriend => {
                let ai = &mut self.world.lizard_at(id).ai;
                ai.run_speed = ai.run_speed.lerp(0.0, 0.5);
            }
        }
    }

    /// The den behaviours' `SetDestination(denFinder.GetDenPosition())`.
    fn lizard_go_to_den(&mut self, id: CreatureId) {
        if let Some(den) = self.world.den_position_of(id) {
            self.world.set_destination(id, den, &mut self.rng);
        }
    }

    /// The Idle behaviour: near its idle spot the counter runs down faster and the
    /// restlessness ebbs; there it settles unless restless or uncomfortable (40 ticks
    /// of discomfort give up the spot), pressed to a floor beneath all three chunks;
    /// now and then (1/600) restless for 1-39 ticks. A spot that will not do, or a
    /// spent counter, makes it try a random tile (two draws) against its destination;
    /// 50 losses in a row keep the spot for 1000-4000 ticks. It walks (0.25) while
    /// restless and its destination is not the spot it left, else stops; bored 1/800.
    fn lizard_idle(&mut self, index: i32, id: CreatureId) {
        let Some(room) = self
            .world
            .realized(index)
            .map(|r| std::sync::Arc::clone(&r.room))
        else {
            return;
        };
        let pos = self.world.creature(id).pos;
        let pf_dest = self
            .world
            .path_finder(id)
            .map_or(pos, crate::creature::PathFinder::get_destination);
        let mut flag = true;
        let l = self.world.lizard_at(id);
        let num = l.ai.idle_counter;
        l.ai.idle_counter -= 1;
        if pos.room == pf_dest.room {
            if !pf_dest.tile_defined() {
                log::debug!("lizard idle spot in den. Setting to critter pos");
                self.world.set_destination(id, pos, &mut self.rng);
                self.world.lizard_at(id).ai.idle_counter = 0;
            } else if (pos.tile.0 - pf_dest.tile.0).abs() + (pos.tile.1 - pf_dest.tile.1).abs() < 5
            {
                let comfortable = self.lizard_comfortable_idle_position(index, id);
                let l = self.world.lizard_at(id);
                l.ai.idle_counter -= l.params.idle_counter_subtract_when_close_to_idle_pos;
                l.ai.idle_restlessness -= 1;
                flag = l.ai.idle_counter < 1 || l.ai.idle_restlessness > 0;
                if !flag && !comfortable {
                    l.ai.unable_to_find_comfortable_position += 1;
                    flag = true;
                } else if l.swim < 0.5 && l.body.chunks.iter().any(|c| c.vel.length() > 2.0) {
                    l.ai.unable_to_find_comfortable_position += 1;
                } else {
                    l.ai.unable_to_find_comfortable_position = 0;
                }
                if l.ai.unable_to_find_comfortable_position > 40 {
                    l.ai.idle_counter = 0;
                    l.ai.unable_to_find_comfortable_position = 0;
                } else if (0..3).all(|i| l.is_tile_solid(&room, i, 0, -1)) {
                    for c in &mut l.body.chunks {
                        c.vel.y -= 0.1;
                    }
                }
                if l.ai.idle_restlessness < 40 && self.rng.value() < 0.001_666_666_7 {
                    let r = self.rng.range_i32(1, 40);
                    self.world.lizard_at(id).ai.idle_restlessness = r;
                }
            }
        }
        let dest = self.lizard_abstract_destination(id);
        if self.lizard_idle_spot_score(index, id, dest) == f32::MAX {
            self.world.lizard_at(id).ai.idle_counter = 0;
        }
        if self.world.lizard_ref(id).ai.idle_counter < 1 {
            if num > 0 {
                let d = self
                    .world
                    .path_finder(id)
                    .map_or(pos, crate::creature::PathFinder::get_destination);
                self.world.lizard_at(id).ai.forbidden_idle_spot = d;
            }
            let x = self.rng.range_i32(0, room.width());
            let y = self.rng.range_i32(0, room.height());
            let test = WorldCoordinate::new(pos.room, x, y, -1);
            let dest = self.lizard_abstract_destination(id);
            if self.lizard_idle_spot_score(index, id, test)
                < self.lizard_idle_spot_score(index, id, dest)
            {
                self.world.set_destination(id, test, &mut self.rng);
                self.world.lizard_at(id).ai.idle_spot_win_streak = 0;
            } else {
                let ai = &mut self.world.lizard_at(id).ai;
                ai.idle_spot_win_streak += 1;
                if ai.idle_spot_win_streak > 50 {
                    let c = self.rng.range_i32(1000, 4000);
                    let ai = &mut self.world.lizard_at(id).ai;
                    ai.idle_counter = c;
                    ai.idle_spot_win_streak = 0;
                }
            }
        }
        let dest = self.lizard_abstract_destination(id);
        let ai = &mut self.world.lizard_at(id).ai;
        if flag {
            if !ai.forbidden_idle_spot.same_room_and_tile(dest) {
                ai.run_speed = ai.run_speed.lerp(0.25, 0.5);
            }
        } else {
            ai.run_speed = ai.run_speed.lerp(0.0, 0.5);
        }
        if self.rng.value() < 0.00125 {
            self.lizard_voice(id, LizardEmotion::Boredom);
        }
        self.world.lizard_at(id).ai.focus_creature = None;
    }

    /// The Frustrated behaviour: it runs (lerp to 1 by half), now and then (1/10) or
    /// near its destination tries a random tile it can reach and come back from (then
    /// restless 30-99 ticks), re-spots its prey, wiggles and growls (each 1/80).
    fn lizard_frustrated(&mut self, index: i32, id: CreatureId) {
        let ai = &mut self.world.lizard_at(id).ai;
        ai.focus_creature = ai.prey.current;
        ai.run_speed = ai.run_speed.lerp(1.0, 0.5);
        let pos = self.world.creature(id).pos;
        let dest = self.lizard_abstract_destination(id);
        let near = (pos.tile.0 - dest.tile.0).abs() + (pos.tile.1 - dest.tile.1).abs() < 10
            && pos.room == dest.room;
        if self.rng.value() < 0.1 || near {
            let (w, h) = self
                .world
                .realized(index)
                .map_or((0, 0), |r| (r.room.width(), r.room.height()));
            let x = self.rng.range_i32(0, w);
            let y = self.rng.range_i32(0, h);
            let test = WorldCoordinate::new(pos.room, x, y, -1);
            if self.world.path_finder(id).is_some_and(|f| {
                f.coordinate_reachable(test) && f.coordinate_possible_to_get_back_from(test)
            }) {
                self.world.set_destination(id, test, &mut self.rng);
                let r = self.rng.range_i32(30, 100);
                self.world.lizard_at(id).ai.idle_restlessness = r;
            }
        }
        if self.rng.value() < 0.0125 {
            self.lizard_enter_animation(id, LizardAnimation::PreyReSpotted, false);
        }
        if self.rng.value() < 0.0125 {
            let w = (self.rng.value() * 100.0) as i32;
            let l = self.world.lizard_at(id);
            l.set_body_wiggle_counter(l.body_wiggle_counter.max(w));
        }
        if self.rng.value() < 0.0125 && self.world.creature(id).kind() != CreatureType::BlackLizard
        {
            self.lizard_voice(id, LizardEmotion::Frustration);
        }
    }

    /// `creature.abstractAI.destination`.
    fn lizard_abstract_destination(&self, id: CreatureId) -> WorldCoordinate {
        let c = self.world.creature(id);
        c.ai.as_ref().map_or(c.pos, |a| a.destination)
    }

    /// `DetermineBehavior`: the highest utility's module (none or under 0.05: Idle at
    /// 0.05); prey in its jaws it eats is ReturnPrey (1) unless fleeing; a frustrated
    /// hunt (0.75) of prey likely found within 350 px or in another room is
    /// Frustrated (not for red lizards), as is going home stranded; a travel utility
    /// above the rest makes it travel unless it is going home, fleeing or on a mission.
    fn lizard_determine_behavior(
        &mut self,
        index: i32,
        id: CreatureId,
        raw: &RawUtilities,
    ) -> LizardBehavior {
        let ai = &self.world.lizard_ref(id).ai;
        let mut behavior = match ai.utility.highest_module() {
            None => LizardBehavior::Idle,
            Some(UtilityModule::ThreatTracker) => LizardBehavior::Flee,
            Some(UtilityModule::RainTracker) => LizardBehavior::EscapeRain,
            Some(UtilityModule::PreyTracker) => LizardBehavior::Hunt,
            Some(UtilityModule::LizardInjuryTracker) => LizardBehavior::Injured,
            Some(UtilityModule::AgressionTracker) => LizardBehavior::Fighting,
            Some(UtilityModule::MissionTracker) => LizardBehavior::ActingOutMission,
            Some(UtilityModule::NoiseTracker) => LizardBehavior::InvestigateSound,
            Some(UtilityModule::FriendTracker) => LizardBehavior::FollowFriend,
        };
        let mut current = ai.utility.highest_utility(raw);
        if current < 0.05 {
            current = 0.05;
            behavior = LizardBehavior::Idle;
        }
        let prey = ai.prey.current;
        let frustration = ai.prey.frustration;
        let stranded = ai.stranded;
        let kind = self.world.creature(id).kind();
        if behavior != LizardBehavior::Flee
            && let Some(other) = self
                .world
                .creature(id)
                .grasps
                .first()
                .copied()
                .flatten()
                .and_then(|g| g.grabbed.creature())
            && self.world.dynamic_relationship(id, other).kind == RelationshipType::Eats
        {
            behavior = LizardBehavior::ReturnPrey;
            current = 1.0;
        }
        if behavior == LizardBehavior::Hunt
            && let Some(k) = prey
            && frustration > 0.75
            && kind != CreatureType::RedLizard
        {
            let (mut t, rel) = self.take_lizard_perception(id);
            let guess = match t.rep_mut(k) {
                Some(r) => self.rep_best_guess(id, r),
                None => WorldCoordinate::new(-1, -1, -1, -1),
            };
            let chance = t.rep(k).map_or(0.0, |r| t.chance_of_finding(r));
            self.put_lizard_perception(id, t, rel);
            let main = self.world.lizard_ref(id).head().pos;
            let pos = self.world.creature(id).pos;
            if (chance > 0.4 && main.dist_less(Vec2::middle_of_tile(guess.tile), 350.0))
                || pos.room != guess.room
            {
                behavior = LizardBehavior::Frustrated;
            }
        }
        if stranded
            && matches!(
                behavior,
                LizardBehavior::ReturnPrey | LizardBehavior::EscapeRain
            )
        {
            behavior = LizardBehavior::Frustrated;
        }
        if !matches!(
            behavior,
            LizardBehavior::ReturnPrey
                | LizardBehavior::Injured
                | LizardBehavior::EscapeRain
                | LizardBehavior::Flee
                | LizardBehavior::ActingOutMission
        ) {
            let travel = self.lizard_travel_utility(index, id);
            if current < travel {
                current = travel;
                behavior = LizardBehavior::Travelling;
            }
        }
        self.world.lizard_at(id).ai.current_utility = current;
        behavior
    }

    /// `TravelUtility`: 0 unless migrating to a reachable destination in a room at
    /// least as attractive as this one; then `Lerp(0.99, 0.1, attraction^0.25)`, halved
    /// between equally attractive rooms.
    fn lizard_travel_utility(&self, index: i32, id: CreatureId) -> f32 {
        let c = self.world.creature(id);
        let Some(ai) = &c.ai else { return 0.0 };
        if !ai.want_to_migrate(c.pos) {
            return 0.0;
        }
        let md = ai.migration_target(c.pos);
        if !self
            .world
            .path_finder(id)
            .is_some_and(|f| f.coordinate_reachable(md))
        {
            return 0.0;
        }
        let kind = c.kind();
        let here = self.world.attraction_value(index, kind);
        let there = self.world.attraction_value(md.room, kind);
        if here > there {
            return 0.0;
        }
        let num = 0.99f32.lerp(0.1, here.mathf_pow(0.25));
        if here == there { num * 0.5 } else { num }
    }

    /// `LizardAI.RoomLike`: the room's size-dependent attraction, times 0.9 for each
    /// other lizard in it (a friend's room would be 1: taming is skipped).
    fn lizard_room_like(&self, index: i32, id: CreatureId) -> f32 {
        let kind = self.world.creature(id).kind();
        let mut like = self.world.size_dependent_attraction(index, kind);
        for &c in self.world.room_creatures(index) {
            if c != id
                && self.world.creature(c).template.top_ancestor() == CreatureType::LizardTemplate
            {
                like *= 0.9;
            }
        }
        like
    }

    /// `IdleSpotScore(coord)` (lower is better): 50 in another room or off screen, the
    /// most for a spot it cannot stand on, reach and come back from or just left; else
    /// 10 over the tile's visibility (barely), more for narrow spaces and water, the
    /// breed's taste (green: floors halved, narrow spaces +20), the threat near it and
    /// less the discomfort. A swimmer's taste comes with the salamander.
    fn lizard_idle_spot_score(&self, index: i32, id: CreatureId, coord: WorldCoordinate) -> f32 {
        let c = self.world.creature(id);
        if coord.room != c.pos.room || !coord.tile_defined() {
            return 50.0;
        }
        let Some(map) = self.world.realized(index).and_then(|r| r.ai_map.as_deref()) else {
            return f32::MAX;
        };
        let t = c.template;
        let ai = &self.world.lizard_ref(id).ai;
        if !map.world_coordinate_accessible_to_creature(coord, &t)
            || !self
                .world
                .path_finder(id)
                .is_some_and(|f| f.coordinate_reachable_and_getbackable(coord))
            || coord.same_room_and_tile(ai.forbidden_idle_spot)
        {
            return f32::MAX;
        }
        let tile = map.tile_at(coord);
        let mut num = 10.0;
        num /= (tile.visibility as f32).lerp(1.0, 0.9995);
        if tile.narrow_space {
            num += 10.0;
        }
        if map.tile(coord.tile.0, coord.tile.1).any_water() {
            num += 100.0;
        }
        match t.kind {
            CreatureType::GreenLizard => {
                if tile.acc == Accessibility::Floor {
                    num *= 0.5;
                }
                if tile.narrow_space {
                    num += 20.0;
                }
            }
            CreatureType::BlueLizard => {
                num += (10.0 - tile.floor_altitude as f32).clamp(0.0, 10.0);
                let prox = map.terrain_proximity(coord.tile.0, coord.tile.1);
                num += (3.0 - prox as f32).clamp(0.0, 3.0);
                if matches!(tile.acc, Accessibility::Climb | Accessibility::Wall) {
                    num *= 0.5;
                }
            }
            CreatureType::PinkLizard => {
                num += tile.floor_altitude as f32 * 2.0;
                if tile.narrow_space {
                    num -= 15.0;
                }
            }
            CreatureType::BlackLizard => {
                if tile.narrow_space {
                    num -= 20.0;
                }
                num += tile.visibility as f32 * 0.1;
            }
            _ => {}
        }
        if ai.threat.utility() > 0.0 {
            num += self.world.threat_of_area(id, coord).clamp(0.0, 100.0);
        }
        if let Some(pf) = self.world.path_finder(id) {
            num -= 1000.0 * self.world.discomfort_of_tile(id, coord, pf);
        }
        num
    }

    /// `ComfortableIdlePosition`: out of water (the game's swimmers, salamanders, are
    /// comfortable in it; none is ported, so water is never comfortable), not facing
    /// a wall 30 px ahead, head and tail on one row (a blue lizard: or one column).
    fn lizard_comfortable_idle_position(&self, index: i32, id: CreatureId) -> bool {
        let Some(r) = self.world.realized(index) else {
            return false;
        };
        let l = self.world.lizard_ref(id);
        let c0 = l.body.chunks[0].pos;
        let c2 = l.body.chunks[2].pos;
        if let Some(map) = r.ai_map.as_deref() {
            let (x, y) = c0.tile();
            if map.clamped_tile(x, y).any_water() {
                return false;
            }
        }
        let ahead = c0 + l.body.chunks[1].pos.direction_to(c0) * 30.0;
        if r.room.is_solid_at(ahead) {
            return false;
        }
        let (t0, t2) = (c0.tile(), c2.tile());
        t0.1 == t2.1 || (self.world.creature(id).kind() == CreatureType::BlueLizard && t0.0 == t2.0)
    }

    /// Each compared module's utility now (the tracker taken out for the prey's).
    fn lizard_utilities_now(&mut self, id: CreatureId) -> RawUtilities {
        let (mut t, rel) = self.take_lizard_perception(id);
        let raw = self.lizard_utilities(id, &mut t);
        self.put_lizard_perception(id, t, rel);
        raw
    }

    /// The compared modules' utilities: threat, prey, rain, injury, aggression,
    /// mission, noise; the friend tracker's is 0 (taming is skipped).
    fn lizard_utilities(&self, id: CreatureId, t: &mut Tracker) -> RawUtilities {
        let c = self.world.creature(id);
        let ai = &self.world.lizard_ref(id).ai;
        let mut raw = RawUtilities::default();
        raw.set(UtilityModule::ThreatTracker, ai.threat.utility());
        raw.set(UtilityModule::PreyTracker, self.world.prey_utility(id, t));
        raw.set(
            UtilityModule::RainTracker,
            RainTracker::utility(
                c.night_creature || c.ignore_cycle,
                self.world.story,
                self.world.rain_cycle.time_until_rain(),
            ),
        );
        raw.set(
            UtilityModule::LizardInjuryTracker,
            crate::creature::lizard::LizardAi::injury_utility(c.state.health),
        );
        raw.set(UtilityModule::AgressionTracker, ai.agression.utility());
        raw.set(UtilityModule::MissionTracker, ai.missions.utility());
        raw.set(UtilityModule::NoiseTracker, ai.noise.utility());
        raw
    }

    /// `ArtificialIntelligence.Update`: the room clock, then the modules in the
    /// constructor's order: path finder, tracker, noise tracker, den finder, (rain:
    /// nothing), prey, threat and aggression trackers, utility comparer, mission
    /// tracker, relationship tracker, stuck tracker (the discomfort tracker has nothing
    /// to update), the friend tracker's gift (following a friend is taming's).
    fn lizard_base_ai_update(&mut self, id: CreatureId) {
        self.world.lizard_at(id).ai.time_in_room += 1;
        let pf = self.world.path_finder_of(id);
        if let Some(p) = pf {
            self.world
                .with_path_finders(|pfs, w| pfs.update(p, w, &mut self.rng));
        }
        let (mut t, mut rel) = self.take_lizard_perception(id);
        self.tracker_update(id, &mut t, &mut rel);
        self.world.noise_tracker_update(id, &t);
        self.world.den_finder_update(id, &mut self.rng);
        self.world.prey_tracker_update(id, &mut t);
        self.world.threat_tracker_update(id, &mut t, &mut rel);
        self.world.agression_tracker_update(id, &mut t);
        let raw = self.lizard_utilities(id, &mut t);
        self.world.lizard_at(id).ai.utility.update(&raw);
        self.world.lizard_at(id).ai.missions.update();
        self.relationships_update(id, &mut t, &mut rel);
        self.put_lizard_perception(id, t, rel);
        let pos = self.world.creature(id).pos;
        let (dest, effectual) = pf
            .and_then(|p| self.world.path_finders.get(p))
            .map_or((pos, pos), |f| {
                (f.get_destination(), f.get_effectual_destination())
            });
        self.world
            .lizard_at(id)
            .ai
            .stuck
            .update(pos, dest, effectual);
        self.world.friend_tracker_update(id);
    }

    /// What lizard `id`'s utility comparer reads now (for MCP): each compared module
    /// with its smoothed (weighted) utility and weight, in order, and the highest.
    #[must_use]
    pub fn lizard_utility_readings(&self, id: CreatureId) -> Option<UtilityReadings> {
        let l = self.world.creature(id).lizard()?;
        let mut t = l.ai.tracker.clone();
        let raw = self.lizard_utilities(id, &mut t);
        let c = &l.ai.utility;
        Some(UtilityReadings {
            modules: c
                .trackers
                .iter()
                .map(|u| (u.module, u.smoothed(&raw), u.weight))
                .collect(),
            highest: c.highest_module(),
            highest_utility: c.highest_utility(&raw),
            travel: self.lizard_travel_utility(self.world.creature(id).pos.room, id),
        })
    }

    /// A dev override (MCP `set_destination`, D18): `AbstractCreatureAI.SetDestination`
    /// of a realized lizard, and the run speed its AI keeps until set again (the
    /// behaviours' bodies are skipped meanwhile; `None` hands it back to them).
    /// Returns whether `id` is a realized lizard.
    pub fn dev_set_lizard_destination(
        &mut self,
        id: CreatureId,
        dest: WorldCoordinate,
        run_speed: Option<f32>,
    ) -> bool {
        if self.world.creature(id).lizard().is_none() {
            return false;
        }
        self.world.lizard_at(id).ai.dev_run_speed = run_speed;
        self.world.set_destination(id, dest, &mut self.rng);
        true
    }

    /// Ends the dev override of `dev_set_lizard_destination`: the behaviours set run
    /// speed and destination again. Returns whether `id` is a realized lizard.
    pub fn dev_release_lizard(&mut self, id: CreatureId) -> bool {
        if self.world.creature(id).lizard().is_none() {
            return false;
        }
        self.world.lizard_at(id).ai.dev_run_speed = None;
        true
    }
}

#[cfg(test)]
#[path = "tests/ai_turn.rs"]
mod tests;
