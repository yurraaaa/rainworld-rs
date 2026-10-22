//! Hearing: `Room.InGameNoise` handing a noise to the room's creatures,
//! `Creature.HeardNoise`, and a `NoiseTracker` hearing it (`HeardNoise`, `NoiseMatch`,
//! `TheorizedSource.Refresh`, `UpdateExamineSound`) and updating; with the tracker reads
//! they share with the AI's own update (an AI's eyes and `VisualContact`, the
//! representations' `VisualContact` getter, `BestGuessForPosition`, `Ghost.Clone`).
//! Decompile: `Room.cs` `InGameNoise`, `Creature.cs` `HeardNoise`, `NoiseTracker.cs`,
//! `Tracker.cs`, `LizardAI.cs` (ReactToNoise). Behaviour:
//! `rainworld-re/notes/creature-model/lizard-senses.md` section 18.
//!
//! Mirrors: Room.cs, Creature.cs, NoiseTracker.cs, Tracker.cs, LizardAI.cs (ReactToNoise)

use rw_assets::{AbstractObjectType, CreatureType};
use rw_random::UnityRandom;

use super::World;
use crate::creature::fly::PlayerBody;
use crate::creature::lizard::{LizardBehavior, LizardEmotion, PendingSound};
use crate::creature::{
    CreatureId, CreatureRep, ElaborateRep, Eyes, GhostCloneCx, LizardAnimation, LizardSight,
    NoiseKind, NoiseTracker, PathFinder, RepKey, RepKind, Tracker, WorldCoordinate,
};
use crate::item::Entity;
use crate::{FloatExt, InGameNoise, Vec2, Water};

/// What every creature hearing one noise shares.
#[derive(Debug, Clone, Copy)]
struct NoiseHearing<'a> {
    noise: InGameNoise,
    room: i32,
    /// `room.PointSubmerged(noise.pos)`.
    submerged: bool,
    /// The room has water (the eyes' water terms).
    has_water: bool,
    /// The player as it is now (mid-update when it made the noise).
    player: Option<&'a PlayerBody>,
}

impl World {
    /// `Room.InGameNoise` in room `room`: every creature realized there hears it, in the
    /// room's order (no ported object is an `IReactToNoises`). `water` is the room's
    /// water when the caller has taken it out of the room.
    pub(crate) fn in_game_noise(
        &mut self,
        room: i32,
        noise: InGameNoise,
        water: Option<&Water>,
        player: Option<&PlayerBody>,
        rng: &mut UnityRandom,
    ) {
        if noise.is_default() {
            return;
        }
        let water = water.or_else(|| self.realized(room).and_then(|r| r.water.as_ref()));
        let h = NoiseHearing {
            noise,
            room,
            submerged: water.is_some_and(|w| noise.pos.y < w.level_at(noise.pos.x)),
            has_water: water.is_some(),
            player,
        };
        for c in self.room_creatures(room).to_vec() {
            if self.room_of(c, player) == Some(room) {
                self.creature_heard_noise(c, &h, rng);
            }
        }
    }

    /// `realizedCreature.room` of creature `c`, the player's from `player` when given.
    fn room_of(&self, c: CreatureId, player: Option<&PlayerBody>) -> Option<i32> {
        match player {
            Some(p) if p.id == c => p.room,
            _ => self.physical_room(c),
        }
    }

    /// `noise.sourceObject.room != null`.
    fn noise_source_in_room(&self, e: Entity, player: Option<&PlayerBody>) -> bool {
        match e {
            Entity::Creature(c) => self.room_of(c, player).is_some(),
            Entity::Object(o) => self.object(o).realized.is_some(),
        }
    }

    /// `Creature.HeardNoise`: a conscious creature with a realized AI hears a noise
    /// another object in a room makes. Of the ported AIs only a lizard's has a noise
    /// tracker (`ArtificialIntelligence.HeardNoise`).
    fn creature_heard_noise(&mut self, c: CreatureId, h: &NoiseHearing<'_>, rng: &mut UnityRandom) {
        let Some(src) = h.noise.source else {
            return;
        };
        if src == Entity::Creature(c) || !self.noise_source_in_room(src, h.player) {
            return;
        }
        if !self.creature(c).lizard().is_some_and(|l| l.conscious()) {
            return;
        }
        let mut t = std::mem::replace(&mut self.lizard_at(c).ai.tracker, Tracker::lizard());
        let mut n = std::mem::take(&mut self.lizard_at(c).ai.noise);
        self.noise_tracker_heard(c, &mut t, &mut n, h, rng);
        self.put_hearing(c, t, n);
    }

    /// Puts a lizard's tracker and noise tracker back, dropping the representations
    /// nothing holds any more.
    fn put_hearing(&mut self, ai: CreatureId, mut t: Tracker, n: NoiseTracker) {
        self.lizard_at(ai).ai.noise = n;
        let held = self.ai_held_reps(ai);
        t.collect_detached(&held);
        self.lizard_at(ai).ai.tracker = t;
    }

    /// The representations AI `ai` and its graphics refer to.
    pub(crate) fn ai_held_reps(&self, ai: CreatureId) -> Vec<RepKey> {
        self.creature(ai).lizard().map_or_else(Vec::new, |l| {
            let look = l
                .graphics
                .as_ref()
                .and_then(|g| g.looker)
                .and_then(|lk| lk.look_creature);
            l.ai.held_reps(look)
        })
    }

    /// `NoiseTracker.HeardNoise`: out of earshot (strength times hearing skill, a fifth
    /// under water) it is not heard, nor while the AI sees where it came from or the
    /// creature that made it (or holds what made it). Else the best matching source
    /// (by `NoiseMatch` and the age, within 300 or, for a tracked creature, 200 to 1000
    /// by how long it was unseen) is refreshed, a source unknown and older than 10 ticks
    /// making it mysterious; or a new source is guessed among the tracked creatures (no
    /// guess past 1000 down to 300 as more of them are out of sight: then mysterious).
    /// Deafness and the room's background noise are not ported (nothing makes them).
    fn noise_tracker_heard(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        n: &mut NoiseTracker,
        h: &NoiseHearing<'_>,
        rng: &mut UnityRandom,
    ) {
        let Some(map) = self.realized(h.room).and_then(|r| r.ai_map.clone()) else {
            return;
        };
        let main = self.ai_main_pos(ai);
        let (x, y) = main.tile();
        let water_fac = if h.submerged || map.clamped_tile(x, y).deep_water() {
            0.2
        } else {
            1.0
        };
        let noise = h.noise;
        if !main.dist_less(noise.pos, noise.strength * n.hearing_skill * water_fac) {
            return;
        }
        if n.ignore_seen_noises {
            if self.ai_sees_point(ai, noise.pos, 0.0, h.has_water) {
                return;
            }
            if let Some(i) = self.noise_source_rep(t, noise)
                && self.rep_visual_contact_now(ai, &mut t.reps[i], h.player)
            {
                return;
            }
        }
        let see = t.see_around_corners;
        let mut best = f32::MAX;
        let mut found = None;
        for j in 0..n.sources.len() {
            let (rep, age) = (n.sources[j].creature_rep, n.sources[j].age);
            let (m, limit) = match rep.and_then(|k| t.rep_mut(k)) {
                Some(r) => (
                    self.noise_match(ai, n, noise.pos, r, h.player) + age as f32 / 5.0,
                    Self::unseen_limit(r, see),
                ),
                None => (n.sources[j].unknown_match(noise.pos), 300.0),
            };
            if m < best && m < limit {
                best = m;
                found = Some(j);
            }
        }
        let interest = self.noise_kind(ai, noise).interest(noise.interesting);
        let mut mysterious = false;
        let j = match found {
            Some(j) => {
                if n.sources[j].creature_rep.is_none() && n.sources[j].age > 10 {
                    n.add_mysterious(interest);
                    mysterious = true;
                }
                j
            }
            None => {
                let mut best = f32::MAX;
                let mut guess = None;
                let mut unseen = 0;
                for k in 0..t.reps.len() {
                    let m = self.noise_match(ai, n, noise.pos, &mut t.reps[k], h.player);
                    if m < best && m < Self::unseen_limit(&t.reps[k], see) {
                        best = m;
                        guess = Some(t.reps[k].key);
                    }
                    if !self.rep_visual_contact_now(ai, &mut t.reps[k], h.player) {
                        unseen += 1;
                    }
                }
                let limit =
                    (unseen as f32).lerp_map(0.0, t.max_tracked_creatures as f32, 1000.0, 300.0);
                if best > limit {
                    guess = None;
                }
                if guess.is_none() {
                    n.add_mysterious(interest);
                    mysterious = true;
                }
                n.add_source(noise.pos, guess)
            }
        };
        n.log_heard(noise, mysterious);
        self.refresh_source(t, n, j, &map, noise, rng);
        self.update_examine_sound(ai, n);
        let source = (n.sources[j].serial, n.sources[j].creature_rep);
        self.lizard_react_to_noise(ai, source, n.sound_to_examine, rng);
    }

    /// `LizardAI.ReactToNoise(source)`, `source` by serial and creature: a black
    /// lizard's bubble for a creature it heard; its prey heard is re-spotted; the sound
    /// to examine, heard idle, travelling or investigating (or lurking), makes it
    /// listen; else a small noise of its own (a draw) and a bubble.
    fn lizard_react_to_noise(
        &mut self,
        ai: CreatureId,
        source: (u32, Option<RepKey>),
        sound_to_examine: Option<u32>,
        rng: &mut UnityRandom,
    ) {
        let black = self.creature(ai).kind() == CreatureType::BlackLizard;
        let holding = self
            .creature(ai)
            .grasps
            .first()
            .is_some_and(Option::is_some);
        let l = self.lizard_at(ai);
        let anim = if source.1.is_some() && black {
            l.bubble = l.bubble.max(4);
            return;
        } else if source.1.is_some() && source.1 == l.ai.prey.current {
            LizardAnimation::PreyReSpotted
        } else if matches!(
            l.ai.behavior,
            LizardBehavior::Idle
                | LizardBehavior::InvestigateSound
                | LizardBehavior::Lurk
                | LizardBehavior::Travelling
        ) && Some(source.0) == sound_to_examine
        {
            LizardAnimation::HearSound
        } else {
            let conscious = l.conscious();
            let intensity = 0.8f32.lerp(1.2, rng.value());
            let l = self.lizard_at(ai);
            l.make_sound_with(PendingSound {
                emotion: LizardEmotion::GeneralSmallNoise,
                intensity,
                conscious,
                holding,
            });
            l.bubble = l.bubble.max(5);
            return;
        };
        if !l.begin_animation(anim, false) {
            return;
        }
        if let Some(emotion) = l.cue_animation(anim, black) {
            let conscious = l.conscious();
            let intensity = 0.8f32.lerp(1.2, rng.value());
            self.lizard_at(ai).make_sound_with(PendingSound {
                emotion,
                intensity,
                conscious,
                holding,
            });
        }
    }

    /// `LerpMap(TicksSinceSeen, 20, 600, 200, 1000)`: how far off a noise may be to be a
    /// tracked creature's.
    fn unseen_limit(r: &CreatureRep, see_around_corners: i32) -> f32 {
        (r.ticks_since_seen(see_around_corners) as f32).lerp_map(20.0, 600.0, 200.0, 1000.0)
    }

    /// The tracked creature that made a noise, or failing that the first holding its
    /// source that is tracked (`RepresentationForObject` without adding).
    fn noise_source_rep(&self, t: &Tracker, noise: InGameNoise) -> Option<usize> {
        let src = noise.source?;
        if let Entity::Creature(c) = src
            && let Some(i) = t.reps.iter().position(|r| r.creature == c)
        {
            return Some(i);
        }
        self.grabbed_by(src)
            .iter()
            .find_map(|g| t.reps.iter().position(|r| r.creature == g.grabber))
    }

    /// What kind of noise it is to AI `ai` (`HowInterestingIsThisNoiseToMe`).
    fn noise_kind(&self, ai: CreatureId, noise: InGameNoise) -> NoiseKind {
        match noise.source {
            Some(Entity::Creature(c))
                if self.creature(c).template.top_ancestor()
                    == self.creature(ai).template.top_ancestor() =>
            {
                NoiseKind::Kin
            }
            Some(Entity::Object(o)) if self.object(o).kind == AbstractObjectType::Rock => {
                NoiseKind::Rock
            }
            _ => NoiseKind::Other,
        }
    }

    /// `NoiseMatch(noise, critRep)`: never a creature in sight (seen noises ignored);
    /// else the distance to its best guess, or to its nearest ghost in the room.
    fn noise_match(
        &self,
        ai: CreatureId,
        n: &NoiseTracker,
        pos: Vec2,
        rep: &mut CreatureRep,
        player: Option<&PlayerBody>,
    ) -> f32 {
        if self.rep_visual_contact_now(ai, rep, player) && n.ignore_seen_noises {
            return f32::MAX;
        }
        let guess = self.rep_best_guess(ai, rep, self.path_finder(ai));
        let mut d = pos.distance(Vec2::middle_of_tile(guess.tile));
        if let RepKind::Elaborate(e) = &rep.kind {
            for g in &e.ghosts {
                let at = Vec2::middle_of_tile(g.coord.tile);
                if g.coord.room == n.room && pos.dist_less(at, d) {
                    d = pos.distance(at);
                }
            }
        }
        d
    }

    /// `TheorizedSource.Refresh`: the source moves to the noise, young again. A tracked
    /// creature is heard (`HeardThisCreature`); a simple representation's last sighting
    /// moves there when it could be there, an elaborate one's nearest ghost in the room
    /// moves there (within 100 px, a still one given a random heading, one draw) or is
    /// cloned there (one draw), and becomes the newest guess.
    fn refresh_source(
        &self,
        t: &mut Tracker,
        n: &mut NoiseTracker,
        j: usize,
        map: &super::AiMap,
        noise: InGameNoise,
        rng: &mut UnityRandom,
    ) {
        let s = &mut n.sources[j];
        s.pos = noise.pos;
        s.age = 0;
        s.last_sound_strength = noise.strength;
        let Some(key) = s.creature_rep else {
            return;
        };
        let (see, params) = (t.see_around_corners, t.ghosts);
        let Some(rep) = t.rep_mut(key) else {
            return;
        };
        if rep.ticks_since_seen > see + 5 {
            rep.ticks_since_seen = (rep.ticks_since_seen - see) / 2;
        }
        let tile = noise.pos.tile();
        let at = WorldCoordinate::new(n.room, tile.0, tile.1, -1);
        let template = self.creature(rep.creature).template;
        let accessible = map.tile_or_neighbors_accessible_to_creature(tile, &template);
        let RepKind::Elaborate(e) = &mut rep.kind else {
            if accessible {
                rep.last_seen_coord = at;
            }
            return;
        };
        let mut closest = f32::MAX;
        let mut ghost = None;
        for (gi, g) in e.ghosts.iter().enumerate() {
            let mid = Vec2::middle_of_tile(g.coord.tile);
            if g.coord.room == n.room && noise.pos.dist_less(mid, closest) {
                closest = noise.pos.distance(mid);
                ghost = Some(gi);
            }
        }
        let Some(mut gi) = ghost else {
            return;
        };
        if accessible {
            if closest < 100.0 {
                let g = &mut e.ghosts[gi];
                g.coord = at;
                g.pos = noise.pos;
                if g.vel.length() == 0.0 {
                    g.vel = Vec2::random_unit(rng);
                }
                g.last_coord = WorldCoordinate::new(-1, -1, -1, -1);
            } else {
                let cx = GhostCloneCx {
                    creature_pos: self.creature(rep.creature).pos,
                    params,
                    dest_realized: self.realized(at.room).is_some(),
                };
                gi = e.clone_ghost(gi, at, 0, cx, rng);
            }
        }
        for g in &mut e.ghosts {
            g.generation += 1;
        }
        e.ghosts[gi].generation = 0;
        e.best_ghost_dirty = true;
    }

    /// `UpdateExamineSound`: the sound to examine is the unknown source nearest the AI's
    /// head, 500 further when it cannot get there and back, and up to 400 further over
    /// the second half of its memory.
    fn update_examine_sound(&self, ai: CreatureId, n: &mut NoiseTracker) {
        n.sound_to_examine = None;
        let main = self.ai_main_pos(ai);
        let room = self
            .creature(ai)
            .lizard()
            .and_then(|l| l.room)
            .unwrap_or(n.room);
        let pf = self.path_finder(ai);
        let half = (n.forget_time / 2) as f32;
        let mut best = f32::MAX;
        for s in &n.sources {
            if s.slated_for_deletion {
                continue;
            }
            let mut d = main.distance(s.pos);
            let (x, y) = s.pos.tile();
            if pf.is_some_and(|p| {
                !p.coordinate_reachable_and_getbackable(WorldCoordinate::new(room, x, y, -1))
            }) {
                d += 500.0;
            }
            d += (s.age as f32).lerp_map(half, n.forget_time as f32, 0.0, 400.0);
            if s.creature_rep.is_none() && d < best {
                best = d;
                n.sound_to_examine = Some(s.serial);
            }
        }
    }

    /// `NoiseTracker.Update` of lizard `ai` (its tracker `t` taken out): the deleted
    /// sources go and the others age; the sound to examine is done with once the AI is
    /// within 3 tiles of it, or 10 seeing it; the mysterious noises fade.
    pub(crate) fn noise_tracker_update(&mut self, ai: CreatureId, t: &Tracker) {
        let mut n = std::mem::take(&mut self.lizard_at(ai).ai.noise);
        for i in (0..n.sources.len()).rev() {
            if n.sources[i].slated_for_deletion {
                n.sources.remove(i);
                self.update_examine_sound(ai, &mut n);
            } else {
                let deleted = n.sources[i]
                    .creature_rep
                    .is_some_and(|k| t.rep(k).is_none_or(|r| r.delete_me_next_frame));
                n.sources[i].update(n.forget_time, deleted);
            }
        }
        if let Some(ex) = n.sound_to_examine().map(|s| s.pos) {
            let (x, y) = ex.tile();
            let d = self
                .creature(ai)
                .pos
                .manhattan_distance(WorldCoordinate::new(n.room, x, y, -1));
            let has_water = self.realized(n.room).is_some_and(|r| r.water.is_some());
            if d < 3 || (d < 10 && self.ai_sees_point(ai, ex, 0.0, has_water)) {
                let serial = n.sound_to_examine;
                for s in &mut n.sources {
                    if Some(s.serial) == serial {
                        s.slated_for_deletion = true;
                    }
                }
                self.update_examine_sound(ai, &mut n);
            }
        }
        n.fade_mysterious_noises();
        n.age_heard();
        self.lizard_at(ai).ai.noise = n;
    }

    /// `NewRoom` of a lizard's noise tracker.
    pub(crate) fn noise_tracker_new_room(&mut self, ai: CreatureId, room: i32) {
        if let Some(l) = self.creature_mut(ai).lizard_mut() {
            l.ai.noise.new_room(room);
        }
    }

    /// `AI.creature.realizedCreature.mainBodyChunk.pos`.
    pub(crate) fn ai_main_pos(&self, ai: CreatureId) -> Vec2 {
        self.creature(ai)
            .lizard()
            .map_or(Vec2::ZERO, |l| l.body.chunks[0].pos)
    }

    /// The eyes of AI `ai` (a lizard's: its breed's vision from the head, the angle off
    /// its head, never blinded: nothing that blinds is ported).
    pub(crate) fn ai_eyes(&self, ai: CreatureId) -> Option<Eyes> {
        let l = self.creature(ai).lizard()?;
        let (head, neck) = (l.body.chunks[0].pos, l.body.chunks[1].pos);
        Some(Eyes {
            vision: l.params.vision,
            point: head,
            lizard_angles: Some(LizardSight {
                perfect: l.params.perfect_vision_angle,
                periferal: l.params.periferal_vision_angle,
                head,
                neck,
            }),
            blinded: false,
        })
    }

    /// `VisualContact(lookAtPoint, bonus)` of AI `ai` in its abstract room (a room not
    /// ready for AI gives the score's caught exception: no contact). `has_water` is
    /// whether the room has water.
    pub(crate) fn ai_sees_point(
        &self,
        ai: CreatureId,
        look: Vec2,
        bonus: f32,
        has_water: bool,
    ) -> bool {
        let Some(eyes) = self.ai_eyes(ai) else {
            return false;
        };
        let Some(r) = self.realized(self.creature(ai).pos.room) else {
            return false;
        };
        let Some(map) = r.ai_map.as_deref() else {
            return false;
        };
        eyes.visual_contact(&r.room, map, has_water, look, bonus)
    }

    /// The `VisualContact` getter of a representation of AI `ai`: false (and the field
    /// cleared) unless the creature is realized in the AI's room.
    pub(crate) fn rep_visual_contact_now(
        &self,
        ai: CreatureId,
        rep: &mut CreatureRep,
        player: Option<&PlayerBody>,
    ) -> bool {
        if self.room_of(rep.creature, player) != Some(self.creature(ai).pos.room) {
            rep.visual_contact = false;
        }
        rep.visual_contact
    }

    /// `BestGuessForPosition()`: a simple representation's last seen coordinate; an
    /// elaborate one's best ghost (found again when dirty, by the AI's path finder `pf`),
    /// its node washed in the AI's room.
    pub(crate) fn rep_best_guess(
        &self,
        ai: CreatureId,
        rep: &mut CreatureRep,
        pf: Option<&PathFinder>,
    ) -> WorldCoordinate {
        let last_seen = rep.last_seen_coord;
        match &mut rep.kind {
            RepKind::Simple { .. } => last_seen,
            RepKind::Elaborate(e) => {
                if e.best_ghost_dirty {
                    self.find_best_ghost(ai, e, last_seen, pf);
                }
                let room = self.creature(ai).pos.room;
                e.best().map_or(WorldCoordinate::new(-1, -1, -1, -1), |g| {
                    if g.coord.room == room {
                        g.coord.wash_node()
                    } else {
                        g.coord
                    }
                })
            }
        }
    }

    /// `FindBestGhost`: most reachable and getbackable for the AI's path finder, then
    /// the lowest of generation and distance from the AI and the last sighting (a ghost
    /// in a Solid tile 1000 less).
    fn find_best_ghost(
        &self,
        ai: CreatureId,
        e: &mut ElaborateRep,
        last_seen: WorldCoordinate,
        pf: Option<&PathFinder>,
    ) {
        let mut best_score = -1.0;
        let mut best_ways = 0;
        e.best_ghost_dirty = false;
        e.lowest_generation_available = -1;
        let ai_pos = self.creature(ai).pos;
        let mut best = None;
        for g in &e.ghosts {
            let mut score = (g.generation * 20) as f32
                + self.between_rooms_distance(ai_pos, g.coord) * 0.5
                + self.between_rooms_distance(last_seen, g.coord);
            let ways = pf.map_or(0, |p| {
                i32::from(p.coordinate_reachable(g.coord))
                    + i32::from(p.coordinate_possible_to_get_back_from(g.coord))
            });
            if self.realized(g.coord.room).is_some_and(|r| {
                crate::RoomExt::is_solid_tile(&*r.room, g.coord.tile.0, g.coord.tile.1)
            }) {
                score -= 1000.0;
            }
            if ways > best_ways {
                best_ways = ways;
                best_score = score;
                best = Some(g.serial);
            } else if ways == best_ways && (best_score < 0.0 || score <= best_score) {
                best_score = score;
                best = Some(g.serial);
            }
            if g.generation < e.lowest_generation_available || e.lowest_generation_available < 0 {
                e.lowest_generation_available = g.generation;
            }
        }
        if let Some(s) = best {
            e.best_ghost = Some(crate::creature::BestGhost::Listed(s));
        }
    }
}
