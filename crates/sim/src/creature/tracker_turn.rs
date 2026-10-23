//! A realized AI's `Tracker` and `RelationshipTracker` as they run: `Tracker.Update`
//! (dismissing guesses it can see are wrong, forgetting the dead, each
//! representation's update, noticing a random creature of the room), the
//! representations' and ghosts' updates, `CreatureNoticed`, `SeeCreature`,
//! `BestGuessForPosition` and `RelationshipTracker.Update` with
//! `EstablishDynamicRelationship`. The tracker is taken out of the AI while it runs;
//! what is particular to an AI (its eyes, the representation it makes, what it does
//! on spotting a creature, its dynamic relationships) is in its own file
//! (`creature/lizard/perception_turn.rs`). Decompile: `Tracker.cs`, `RelationshipTracker.cs`,
//! `ArtificialIntelligence.cs` 159-231 and 363-407. Behaviour:
//! `rainworld-re/notes/creature-model/lizard-senses.md` section 17.
//!
//! Mirrors: Tracker.cs, RelationshipTracker.cs, ArtificialIntelligence.cs

use rw_assets::ShortcutType;
use rw_assets::shortcut::FOUR_DIRECTIONS;

use super::RainWorldGame;
use crate::creature::{
    CreatureId, CreatureRep, DynamicRelationship, Ghost, GhostCloneCx, RelationshipTracker,
    RelationshipType, RepKey, RepKind, Tracker, WorldCoordinate,
};
use crate::world::MovementConnection;
use crate::{BodyChunk, FloatExt, Player, TileExt, Vec2};

/// A realized creature's body as a tracker reads it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrackedBody {
    /// `realizedCreature.room`: `None` in a shortcut.
    pub room: Option<i32>,
    pub in_shortcut: bool,
    /// Its chunks, the main one first.
    pub chunks: Vec<BodyChunk>,
    /// The owner's `VisibilityBonus`.
    pub visibility: f32,
}

impl TrackedBody {
    /// `mainBodyChunk` (chunk 0 of every creature realized so far).
    pub fn main(&self) -> &BodyChunk {
        &self.chunks[0]
    }

    /// The chunks' mean velocity.
    pub fn mean_vel(&self) -> Vec2 {
        let mut v = self.chunks[0].vel;
        for c in &self.chunks[1..] {
            v += c.vel;
        }
        v / self.chunks.len() as f32
    }
}

impl Vec2 {
    /// `IntVector2.ToVector2` of a tile.
    fn of_tile(t: (i32, i32)) -> Self {
        Self::new(t.0 as f32, t.1 as f32)
    }
}

impl RainWorldGame {
    /// Creature `c`'s body, while realized.
    pub(crate) fn tracked_body(&self, c: CreatureId) -> Option<TrackedBody> {
        if c == self.player_id {
            let p = self.player.as_ref().filter(|p| !p.is_destroyed())?;
            let in_shortcut = p.in_shortcut();
            let room = (!in_shortcut).then(|| self.pos().room);
            let water = room
                .and_then(|r| self.world.realized(r))
                .and_then(|r| r.water.as_ref());
            let visibility = crate::creature::PlayerVisibility {
                general: self.character_stats.general_visibility_bonus,
                stealth_in_sneak_mode: self.character_stats.visual_stealth_in_sneak_mode,
                priv_sneak: p.priv_sneak(),
                submersion: p.body.submersion(water),
            }
            .bonus();
            return Some(TrackedBody {
                room,
                in_shortcut,
                chunks: p.body.chunks.to_vec(),
                visibility,
            });
        }
        let a = self.world.creature(c);
        let (room, in_shortcut, chunks) = match a.realized.as_ref()? {
            crate::creature::RealizedCreature::Player => return None,
            crate::creature::RealizedCreature::Lizard(l) => {
                (l.room, l.in_shortcut, l.body.chunks.to_vec())
            }
            crate::creature::RealizedCreature::Fly(f) => {
                (f.room, f.in_shortcut, vec![f.chunk().clone()])
            }
            crate::creature::RealizedCreature::Overseer(o) => (
                (o.in_room && !o.in_shortcut).then_some(a.pos.room),
                o.in_shortcut,
                vec![o.chunk().clone()],
            ),
        };
        Some(TrackedBody {
            room: room.filter(|_| !in_shortcut),
            in_shortcut,
            chunks,
            visibility: 0.0,
        })
    }

    /// `AI.creature.pos` of AI `ai`.
    fn ai_pos(&self, ai: CreatureId) -> WorldCoordinate {
        self.world.creature(ai).pos
    }

    /// `VisualContact(lookAtPoint, bonus)` of AI `ai` in its abstract room.
    pub(crate) fn ai_sees_point(&self, ai: CreatureId, look: Vec2, bonus: f32) -> bool {
        let has_water = self
            .world
            .realized(self.ai_pos(ai).room)
            .is_some_and(|r| r.water.is_some());
        self.world.ai_sees_point(ai, look, bonus, has_water)
    }

    /// `VisualContact(lookAtCoord, bonus)`: its tile's middle, in the AI's room only.
    pub(crate) fn ai_sees_coord(&self, ai: CreatureId, c: WorldCoordinate, bonus: f32) -> bool {
        c.room == self.ai_pos(ai).room
            && c.tile_defined()
            && self.ai_sees_point(ai, Vec2::middle_of_tile(c.tile), bonus)
    }

    /// `VisualContact(chunk)`: at the chunk, with its `VisibilityBonus`.
    pub(crate) fn ai_sees_chunk(
        &self,
        ai: CreatureId,
        chunk: &BodyChunk,
        owner_bonus: f32,
    ) -> bool {
        let Some(eyes) = self.ai_eyes(ai) else {
            return false;
        };
        let bonus = crate::creature::ChunkVisibility {
            pos: chunk.pos,
            last_pos: chunk.last_pos,
            owner_bonus,
        }
        .bonus(eyes.vision.movement_based_vision);
        self.ai_sees_point(ai, chunk.pos, bonus)
    }

    /// Whether AI `ai` sees any chunk of `body`, in order.
    fn ai_sees_body(&self, ai: CreatureId, body: &TrackedBody) -> bool {
        body.chunks
            .iter()
            .any(|c| self.ai_sees_chunk(ai, c, body.visibility))
    }

    /// The `VisualContact` getter's check: realized in the AI's room.
    pub(crate) fn rep_in_view(&self, ai: CreatureId, c: CreatureId) -> bool {
        self.tracked_body(c)
            .and_then(|b| b.room)
            .is_some_and(|r| r == self.ai_pos(ai).room)
    }

    /// The `VisualContact` getter: false (and the field cleared) out of view, or for a
    /// hidden player whose `VisibilityBonus` is -1 or less.
    pub(crate) fn rep_visual_contact(&self, ai: CreatureId, rep: &mut CreatureRep) -> bool {
        if !self.rep_in_view(ai, rep.creature) {
            rep.visual_contact = false;
        }
        if rep.creature == self.player_id
            && self.player.as_ref().is_some_and(Player::is_hidden)
            && self
                .tracked_body(rep.creature)
                .is_some_and(|b| b.visibility <= -1.0)
        {
            rep.visual_contact = false;
        }
        rep.visual_contact
    }

    /// `BestGuessForPosition()` (`World::rep_best_guess` with the AI's path finder).
    pub(crate) fn rep_best_guess(&self, ai: CreatureId, rep: &mut CreatureRep) -> WorldCoordinate {
        self.world
            .rep_best_guess(ai, rep, self.world.path_finder(ai))
    }

    /// What `BestGuessForPosition()` would return now, without settling the guess.
    pub(crate) fn peek_best_guess(&self, ai: CreatureId, rep: &CreatureRep) -> WorldCoordinate {
        let mut copy = rep.clone();
        self.rep_best_guess(ai, &mut copy)
    }

    /// `Tracker.Update` of AI `ai` (`useTrackedCreatures` off): a random representation
    /// is checked (an elaborate one's random ghost pushed on when the AI sees its spot
    /// empty; a simple one forgotten likewise; a dead one in sight forgotten unless
    /// eaten), every representation updates (last first) and the deleted ones go, then a
    /// random creature of the room is noticed when the AI sees one of its chunks.
    pub(super) fn tracker_update(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
    ) {
        let ai_pos = self.ai_pos(ai);
        if !t.reps.is_empty() {
            let index = self.rng.range_i32(0, t.reps.len() as i32) as usize;
            self.tracker_check_rep(ai, t, index);
        }
        let room_creatures = self.world.room_creatures(ai_pos.room).to_vec();
        if room_creatures.is_empty() {
            return;
        }
        let pick = room_creatures[self.rng.range_i32(0, room_creatures.len() as i32) as usize];
        let mut notice = pick != ai;
        for i in (0..t.reps.len()).rev() {
            self.rep_update(ai, t, i);
            if t.reps[i].creature == pick {
                notice = false;
            }
            if t.reps[i].delete_me_next_frame {
                let held = self.ai_held_reps(ai);
                t.remove_rep(i, &held);
            }
        }
        if !notice {
            return;
        }
        let Some(body) = self.tracked_body(pick).filter(|b| !b.in_shortcut) else {
            return;
        };
        if self.ai_sees_body(ai, &body) {
            self.creature_noticed(ai, t, rel, pick);
        }
    }

    /// `Tracker.Update`'s check of representation `index`.
    fn tracker_check_rep(&mut self, ai: CreatureId, t: &mut Tracker, index: usize) {
        let ai_pos = self.ai_pos(ai);
        let range = t.ghosts.dismissal_range as f32;
        let ghosts = match &t.reps[index].kind {
            RepKind::Elaborate(e) => e.ghosts.len(),
            RepKind::Simple { .. } => 0,
        };
        if ghosts > 0 {
            let gi = self.rng.range_i32(0, ghosts as i32) as usize;
            let visual = self.rep_visual_contact(ai, &mut t.reps[index]);
            let RepKind::Elaborate(e) = &t.reps[index].kind else {
                return;
            };
            let (gpos, gcoord, gone) = (
                e.ghosts[gi].pos,
                e.ghosts[gi].coord,
                e.ghosts[gi].delete_me_next_frame,
            );
            if !visual
                && !gone
                && ai_pos.room == gcoord.room
                && self.ai_sees_point(ai, gpos, 0.0)
                && (ai_pos.tile.manhattan(gcoord.tile) < 2
                    || (Vec2::of_tile(ai_pos.tile).distance(Vec2::of_tile(gcoord.tile)) < range
                        && self.ai_sees_coord(ai, gcoord, 0.0)))
            {
                self.ghost_push(ai, t, index, gi);
            }
        } else if !self.rep_visual_contact(ai, &mut t.reps[index])
            && self.world.realized(ai_pos.room).is_some()
        {
            let guess = self.rep_best_guess(ai, &mut t.reps[index]);
            if self.ai_sees_point(ai, Vec2::middle_of_tile(guess.tile), 0.0)
                && (ai_pos.tile.manhattan(guess.tile) < 2
                    || (Vec2::of_tile(ai_pos.tile).distance(Vec2::of_tile(guess.tile)) < range
                        && self.ai_sees_coord(ai, guess, 0.0)))
            {
                t.reps[index].destroy();
            }
        }
        let c = t.reps[index].creature;
        if self.rep_visual_contact(ai, &mut t.reps[index])
            && !self.world.creature(c).state.alive
            && self.tracker_to_discard_dead(ai, t, c)
        {
            t.reps[index].destroy();
        }
    }

    /// `TrackerToDiscardDeadCreature`: unless the AI eats it (no AI ported has an
    /// obstacle tracker).
    fn tracker_to_discard_dead(&self, ai: CreatureId, t: &Tracker, c: CreatureId) -> bool {
        self.world.dynamic_relationship_in(ai, t, c).kind != RelationshipType::Eats
    }

    /// `CreatureRepresentation.Update` (and the simple or elaborate one's own) of
    /// representation `i`.
    fn rep_update(&mut self, ai: CreatureId, t: &mut Tracker, i: usize) {
        let ai_pos = self.ai_pos(ai);
        let see = t.see_around_corners;
        let frames = t.frames_to_remember_creatures;
        let c = t.reps[i].creature;
        let cpos = self.world.creature(c).pos;
        let in_den = self.world.creature(c).in_den;
        let body = self.tracked_body(c);
        let rep = &mut t.reps[i];
        rep.age += 1;
        let was_visible = rep.visual_contact;
        rep.ticks_since_seen += 1;
        if rep.ticks_since_seen > see {
            rep.visual_contact = false;
            if !in_den
                && cpos.room == ai_pos.room
                && let Some(b) = &body
                && self.ai_sees_body(ai, b)
            {
                rep.ticks_since_seen = 0;
                rep.visual_contact = true;
            }
        }
        let mut spotted = false;
        let mut left_room = false;
        let unseen = !rep.visual_contact;
        if rep.visual_contact {
            if body.is_none() || cpos.room != ai_pos.room {
                rep.visual_contact = false;
            } else {
                rep.last_seen_coord = cpos.wash_node();
                spotted = !was_visible;
            }
        } else {
            rep.forget_counter += 1;
            if rep.gone_to_other_room_sense
                && cpos.room != rep.last_seen_coord.room
                && body.as_ref().is_some_and(|b| b.room.is_some())
            {
                left_room = true;
                rep.gone_to_other_room_sense = false;
            }
        }
        if spotted {
            let r = t.reps[i].clone();
            self.ai_creature_spotted(ai, false, &r, t);
            t.reps[i].forget_counter = 0;
        }
        if left_room {
            self.sense_left_room(t, i);
        }
        let rep = &mut t.reps[i];
        if unseen && frames > -1 && rep.forget_counter > frames {
            rep.destroy();
        }
        match &t.reps[i].kind {
            RepKind::Simple {
                forget_when_not_visible,
            } => {
                let forget = *forget_when_not_visible;
                if forget && !self.rep_visual_contact(ai, &mut t.reps[i]) {
                    t.reps[i].destroy();
                }
            }
            RepKind::Elaborate(_) => self.elaborate_update(ai, t, i),
        }
    }

    /// `ElaborateCreatureRepresentation.Update` after the base: in sight, back to one
    /// ghost on the creature; else the ghosts move (the deleted ones go, one left at
    /// least) and losing sight leaves two guesses.
    fn elaborate_update(&mut self, ai: CreatureId, t: &mut Tracker, i: usize) {
        let c = t.reps[i].creature;
        if self.rep_visual_contact(ai, &mut t.reps[i]) {
            let body = self.tracked_body(c);
            let cpos = self.world.creature(c).pos;
            let rep = &mut t.reps[i];
            let RepKind::Elaborate(e) = &mut rep.kind else {
                return;
            };
            if e.ghosts.len() > 1 {
                e.ghosts.truncate(1);
                e.best_ghost = Some(crate::creature::BestGhost::Listed(e.ghosts[0].serial));
                e.best_ghost_dirty = false;
            }
            let g = &mut e.ghosts[0];
            if let Some(b) = &body {
                g.pos = b.main().pos;
                g.vel = b.mean_vel();
            }
            g.coord = cpos.wash_node();
            g.last_coord = cpos.wash_node();
            g.generation = 0;
            g.stopped = false;
            rep.last_seen_coord = g.coord;
        } else {
            let n = match &t.reps[i].kind {
                RepKind::Elaborate(e) => e.ghosts.len(),
                RepKind::Simple { .. } => 0,
            };
            for gi in (0..n).rev() {
                self.ghost_update(ai, t, i, gi);
                if let RepKind::Elaborate(e) = &mut t.reps[i].kind
                    && e.ghosts[gi].delete_me_next_frame
                    && e.ghosts.len() > 1
                {
                    e.remove_ghost(gi);
                    e.best_ghost_dirty = true;
                }
            }
            if matches!(&t.reps[i].kind, RepKind::Elaborate(e) if e.last_visual_contact) {
                self.lost_visual_contact(ai, t, i);
            }
        }
        let visual = self.rep_visual_contact(ai, &mut t.reps[i]);
        if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
            e.last_visual_contact = visual;
        }
    }

    /// `LostVisualContact`: the first ghost and a clone of it (generation 500): one
    /// stays where the creature was last seen, the other goes on, by whether it could
    /// stand there and was slow.
    fn lost_visual_contact(&mut self, ai: CreatureId, t: &mut Tracker, i: usize) {
        let c = t.reps[i].creature;
        let croom = self.world.creature(c).pos.room;
        let template = self.world.creature(c).template;
        let ai_template = self.world.creature(ai).template;
        let map = self
            .world
            .realized(croom)
            .filter(|r| r.ready_for_ai())
            .and_then(|r| r.ai_map.clone());
        let Some(map) = map else {
            if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
                e.best_ghost_dirty = true;
            }
            return;
        };
        let body = self.tracked_body(c);
        let RepKind::Elaborate(e) = &t.reps[i].kind else {
            return;
        };
        let best = e.best().map_or((0, 0), |g| g.coord.tile);
        let mut stays = map.tile_accessible_to_creature(best, &template)
            || FOUR_DIRECTIONS
                .iter()
                .any(|d| map.tile_accessible_to_creature((best.0 + d.0, best.1 + d.1), &template));
        stays = stays
            && body.as_ref().is_some_and(|b| {
                b.main().last_last_pos.distance(b.main().last_pos)
                    < template.off_screen_speed.lerp(2.0, 0.5)
            });
        let first = e.ghosts[0].coord;
        let second = self.ghost_clone(t, i, 0, first, 500);
        let RepKind::Elaborate(e) = &mut t.reps[i].kind else {
            return;
        };
        if stays {
            let g = &mut e.ghosts[0];
            g.stopped = true;
            g.coord.tile = map.try_for_accessible_neighbor(g.coord.tile, &ai_template);
            g.pos = Vec2::middle_of_tile(g.coord.tile);
            e.ghosts[second].stopped = false;
        } else {
            e.ghosts[0].stopped = false;
            let first_tile = e.ghosts[0].coord.tile;
            let g2 = &mut e.ghosts[second];
            g2.stopped = true;
            g2.coord.tile = map.try_for_accessible_neighbor(g2.coord.tile, &ai_template);
            g2.pos = Vec2::middle_of_tile(first_tile);
        }
        e.best_ghost_dirty = true;
    }

    /// `SenseThatCreatureHasLeftRoom`: ghosts in the creature's new room become
    /// generation 0, the others 100 older; with none there, a ghost at the entrance it
    /// came in by (`LocalCoordinateOfNode`), facing out of the hole.
    fn sense_left_room(&mut self, t: &mut Tracker, i: usize) {
        let c = t.reps[i].creature;
        let cpos = self.world.creature(c).pos;
        let params = t.ghosts;
        let RepKind::Elaborate(e) = &mut t.reps[i].kind else {
            return;
        };
        let mut any_there = false;
        for g in &mut e.ghosts {
            if g.coord.room == cpos.room {
                any_there = true;
                g.generation = 0;
            } else {
                g.generation += 100;
            }
        }
        if !any_there && self.world.realized(cpos.room).is_some() {
            let tile = self
                .world
                .local_coordinate_of_node(cpos.room, cpos.node, &mut self.rng);
            let dir = self
                .world
                .realized(cpos.room)
                .map_or((0, 0), |r| r.room.shortcut_hole_direction(tile.0, tile.1));
            let serial = e.next_serial();
            let mut g = Ghost::new(
                serial,
                cpos,
                Vec2::middle_of_tile(tile),
                Vec2::of_tile(dir),
                params,
                &mut self.rng,
            );
            g.forbidden_room_exit = g.coord;
            e.ghosts.push(g);
        }
        e.best_ghost_dirty = true;
    }

    /// `Ghost.Clone(dest, generationAdd)` of ghost `gi` of representation `i`; returns
    /// the new ghost's index.
    fn ghost_clone(
        &mut self,
        t: &mut Tracker,
        i: usize,
        gi: usize,
        dest: WorldCoordinate,
        generation_add: i32,
    ) -> usize {
        let cx = GhostCloneCx {
            creature_pos: self.world.creature(t.reps[i].creature).pos,
            params: t.ghosts,
            dest_realized: self.world.realized(dest.room).is_some(),
        };
        let RepKind::Elaborate(e) = &mut t.reps[i].kind else {
            return 0;
        };
        e.clone_ghost(gi, dest, generation_add, cx, &mut self.rng)
    }

    /// `Ghost.Push`: an old ghost with no pushes left goes; else it is pushed
    /// `ghostPushSpeed` moves on, each away from the AI or in a random direction (two
    /// draws, or one).
    fn ghost_push(&mut self, ai: CreatureId, t: &mut Tracker, i: usize, gi: usize) {
        let params = t.ghosts;
        let RepKind::Elaborate(e) = &mut t.reps[i].kind else {
            return;
        };
        let count = e.ghosts.len();
        let g = &mut e.ghosts[gi];
        if g.push_forward < 1 && count > 1 {
            g.delete_me_next_frame = true;
        } else {
            g.push_forward -= 1;
            g.generation += 1;
            g.last_coord = g.coord;
            g.allowed_to_move_in_visible_areas = true;
            let ai_main = self.ai_main_pos(ai);
            for _ in 0..params.push_speed {
                let vel = if self.rng.value() < 0.5 {
                    let RepKind::Elaborate(e) = &t.reps[i].kind else {
                        return;
                    };
                    ai_main.direction_to(e.ghosts[gi].pos) * 10.0
                } else {
                    Vec2::from_degrees(self.rng.value() * 360.0) * 10.0
                };
                if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
                    let g = &mut e.ghosts[gi];
                    g.vel = vel;
                    g.move_buffer = 1.0;
                    g.stopped = false;
                }
                self.ghost_update(ai, t, i, gi);
                let RepKind::Elaborate(e) = &t.reps[i].kind else {
                    return;
                };
                if self.ai_pos(ai).room != e.ghosts[gi].coord.room {
                    break;
                }
            }
            if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
                e.ghosts[gi].allowed_to_move_in_visible_areas = false;
            }
        }
        if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
            e.best_ghost_dirty = true;
        }
    }

    /// `Ghost.Update`: a ghost off the creature's accessible tiles falls (a flier's
    /// glides) until it lands; one on them moves on its buffer: through a room exit
    /// into the next room (a clone there), or along the outgoing connection closest to
    /// its heading (the second best cloned while there are fewer than `MaxGhosts`),
    /// two ghosts on one tile leaving the older.
    fn ghost_update(&mut self, ai: CreatureId, t: &mut Tracker, i: usize, gi: usize) {
        let params = t.ghosts;
        let chance = t.chance_of_finding(&t.reps[i]);
        let c = t.reps[i].creature;
        let template = self.world.creature(c).template;
        let ai_template = self.world.creature(ai).template;
        let RepKind::Elaborate(e) = &t.reps[i].kind else {
            return;
        };
        let g = e.ghosts[gi].clone();
        let Some(r) = self.world.realized(g.coord.room) else {
            return;
        };
        if g.stopped || !r.ready_for_ai() {
            return;
        }
        let Some(map) = r.ai_map.clone() else {
            return;
        };
        let room = std::sync::Arc::clone(&r.room);
        let shortcuts_room_index = g.coord.room;
        if !map.tile_accessible_to_creature(g.pos.tile(), &template) {
            let mut g = g;
            g.pos += g.vel;
            if !template.can_fly {
                g.vel.y -= 0.9;
            }
            g.coord.tile = g.pos.tile();
            g.last_coord = g.coord;
            if g.vel.x != 0.0
                && !self.ghost_neighbour_legal(
                    &g,
                    &template,
                    (if g.vel.x < 0.0 { -1 } else { 1 }, 0),
                )
            {
                g.pos.x = Vec2::middle_of_tile(g.pos.tile()).x;
                g.vel.x = 0.0;
            }
            if g.vel.y != 0.0
                && !self.ghost_neighbour_legal(
                    &g,
                    &template,
                    (0, if g.vel.y < 0.0 { -1 } else { 1 }),
                )
            {
                g.pos.y = Vec2::middle_of_tile(g.pos.tile()).y;
                g.vel.y = 0.0;
            }
            self.set_ghost(t, i, gi, g);
            return;
        }
        let mut g = g;
        g.move_buffer += template.off_screen_speed * params.speed;
        if g.move_buffer <= 1.0 {
            self.set_ghost(t, i, gi, g);
            return;
        }
        g.move_buffer = 0.0;
        let tile = g.pos.tile();
        if room.tile(tile.0, tile.1).terrain == rw_assets::Terrain::ShortcutEntrance
            && g.forbidden_room_exit != g.coord
            && let Some(data) = room.shortcut_map().data_at(tile)
            && data.kind == ShortcutType::RoomExit
        {
            let conn = data.dest_node.and_then(|n| {
                self.world
                    .abstract_room(shortcuts_room_index)
                    .and_then(|a| a.connections.get(n).copied())
            });
            g.delete_me_next_frame = true;
            self.set_ghost(t, i, gi, g.clone());
            if let Some(next) = conn.and_then(|n| self.world.abstract_room(n)) {
                let node = next.exit_index(g.coord.room);
                let mut dest = WorldCoordinate::new(next.index, -1, -1, node);
                if let Some(nr) = self
                    .world
                    .realized(next.index)
                    .filter(|nr| nr.shortcuts_ready())
                {
                    dest.tile = usize::try_from(node)
                        .ok()
                        .and_then(|n| nr.room.shortcut_map().leading_to_node(n))
                        .map_or(dest.tile, |s| s.start);
                }
                let n = self.ghost_clone(t, i, gi, dest, 0);
                if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
                    e.ghosts[n].forbidden_room_exit = e.ghosts[n].coord;
                }
            }
            return;
        }
        let pf = self.world.path_finder(ai);
        let mut best: Option<(f32, MovementConnection)> = None;
        let mut second: Option<(f32, MovementConnection)> = None;
        for conn in &map.tile(tile.0, tile.1).outgoing_paths {
            if !self.ghost_connection_legal(ai, &g, &template, conn) {
                continue;
            }
            let mut objections = 0;
            if !ai_template.connection_resistance(conn.kind).allowed() {
                objections += 1;
            }
            if let Some(p) = pf {
                if !p.coordinate_reachable(conn.dest) {
                    objections += 1;
                }
                if !p.coordinate_possible_to_get_back_from(conn.dest) {
                    objections += 1;
                }
            }
            let s = g.connection_score(conn, objections);
            if best.is_none_or(|(b, _)| s < b) {
                best = Some((s, *conn));
            } else if second.is_none_or(|(b, _)| s < b) {
                second = Some((s, *conn));
            }
        }
        let Some((_, bc)) = best else {
            g.stopped = true;
            self.set_ghost(t, i, gi, g);
            return;
        };
        if let Some((_, sc)) = second {
            let (count, max) = match &t.reps[i].kind {
                RepKind::Elaborate(e) => (e.ghosts.len() as i32, e.max_ghosts_now(chance)),
                RepKind::Simple { .. } => (0, 0),
            };
            self.set_ghost(t, i, gi, g.clone());
            if count < max {
                self.ghost_clone(t, i, gi, sc.dest, 1);
            } else {
                g.stopped = true;
            }
        }
        g.vel = (g.vel * 4.0
            + Vec2::of_tile(bc.start.tile).direction_to(Vec2::of_tile(bc.dest.tile)))
            / 5.0;
        g.last_coord = g.coord;
        g.coord.tile = bc.dest.tile;
        g.pos = Vec2::middle_of_tile(g.coord.tile);
        g.connections_followed += 1;
        let RepKind::Elaborate(e) = &mut t.reps[i].kind else {
            return;
        };
        let serial = g.serial;
        for other in e.ghosts.iter_mut().filter(|o| o.serial != serial) {
            if other.coord.room == g.coord.room && other.coord.tile == g.coord.tile {
                if other.generation <= g.generation && !other.delete_me_next_frame {
                    g.delete_me_next_frame = true;
                } else if !g.delete_me_next_frame {
                    other.delete_me_next_frame = true;
                }
            }
        }
        e.ghosts[gi] = g;
    }

    /// Writes ghost `gi` of representation `i` back.
    fn set_ghost(&self, t: &mut Tracker, i: usize, gi: usize, g: Ghost) {
        if let RepKind::Elaborate(e) = &mut t.reps[i].kind {
            e.ghosts[gi] = g;
        }
    }

    /// `Ghost.NeighbourLegal(nb)`.
    fn ghost_neighbour_legal(
        &self,
        g: &Ghost,
        template: &crate::creature::CreatureTemplate,
        nb: (i32, i32),
    ) -> bool {
        let Some(r) = self.world.realized(g.coord.room) else {
            return false;
        };
        let t = g.pos.tile();
        let n = (t.0 + nb.0, t.1 + nb.1);
        match r.ai_map.as_deref() {
            Some(map) if map.tile_accessible_to_creature(t, template) => {
                map.tile_accessible_to_creature(n, template)
            }
            _ => !crate::RoomExt::is_solid_tile(&*r.room, n.0, n.1),
        }
    }

    /// `Ghost.ConnectionLegal`: a move the creature may make onto a tile it can use
    /// that does not turn straight back, out of the AI's sight unless pushed.
    fn ghost_connection_legal(
        &self,
        ai: CreatureId,
        g: &Ghost,
        template: &crate::creature::CreatureTemplate,
        conn: &MovementConnection,
    ) -> bool {
        let dest_ok = self
            .world
            .realized(conn.dest.room)
            .and_then(|r| r.ai_map.as_deref())
            .is_none_or(|m| m.tile_accessible_to_creature(conn.dest.tile, template));
        if conn.dest.tile != g.last_coord.tile
            && template.connection_resistance(conn.kind).allowed()
            && dest_ok
            && g.not_turning_back(conn)
        {
            return g.allowed_to_move_in_visible_areas || !self.ai_sees_coord(ai, conn.dest, 0.0);
        }
        false
    }

    /// `Tracker.SeeCreature(crit)`: the representations of it are made visible, else it
    /// is noticed.
    pub(super) fn tracker_see_creature(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        c: CreatureId,
    ) {
        let mut found = false;
        for r in t.reps.iter_mut().filter(|r| r.creature == c) {
            r.make_visible();
            found = true;
        }
        if !found {
            self.creature_noticed(ai, t, rel, c);
        }
    }

    /// `CreatureNoticed(crit)`: a realized creature that is neither a dead one the AI
    /// discards nor one it does not track gets a representation (the AI's kind), which
    /// the AI spots (first time unless they are stuck together) and relates to; past
    /// `maxTrackedCreatures` the least interesting representation is forgotten.
    fn creature_noticed(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        c: CreatureId,
    ) -> Option<RepKey> {
        let body = self.tracked_body(c)?;
        if !self.world.creature(c).state.alive && self.tracker_to_discard_dead(ai, t, c) {
            return None;
        }
        let stat = self.world.static_relationship(ai, c);
        if stat.kind == RelationshipType::DoesntTrack {
            return None;
        }
        let me = self.world.creature(ai);
        let stuck = me.template.grasps > 0
            && me
                .sticks
                .iter()
                .any(|s| s.a == c || s.b == crate::item::Entity::Creature(c));
        let rep = self.create_tracker_rep(ai, t, c, &body);
        self.ai_creature_spotted(ai, !stuck, &rep, t);
        let key = rep.key;
        // The game relates to it before listing it; the modules it is sorted into
        // find it by key either way.
        t.reps.push(rep);
        self.establish_dynamic_relationship(ai, t, rel, key);
        if t.reps.len() > t.max_tracked_creatures as usize {
            let ai_tile = self.ai_pos(ai).tile;
            let mut least = f32::MAX;
            let mut worst = None;
            for i in 0..t.reps.len() {
                let rc = t.reps[i].creature;
                let intensity = t.reps[i]
                    .current_relationship()
                    .unwrap_or_else(|| self.world.static_relationship(ai, rc))
                    .intensity;
                let visual = self.rep_visual_contact(ai, &mut t.reps[i]);
                let guess = self.rep_best_guess(ai, &mut t.reps[i]);
                let mut s = intensity * 100_000.0
                    + if visual { 2.0 } else { 1.0 }
                        / (1.0 + Vec2::of_tile(guess.tile).distance(Vec2::of_tile(ai_tile)));
                s /= (t.reps[i].forget_counter as f32).lerp(100.0, 0.7);
                if s < least {
                    least = s;
                    worst = Some(i);
                }
            }
            if let Some(w) = worst {
                t.reps[w].destroy();
                if t.reps[w].key == key {
                    return None;
                }
            }
        }
        Some(key)
    }

    /// `RelationshipTracker.EstablishDynamicRelationship(rep)`: unless the AI does not
    /// track it or already relates to it, a relationship starting from the static one,
    /// sorted into its module (before the representation has it), with the AI's state
    /// for it.
    fn establish_dynamic_relationship(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
        key: RepKey,
    ) {
        let Some(c) = t.rep(key).map(|r| r.creature) else {
            return;
        };
        let initial = self.world.static_relationship(ai, c);
        if initial.kind == RelationshipType::DoesntTrack || rel.relationships.contains(&key) {
            return;
        }
        let module = self
            .world
            .sort_creature_into_module(ai, t, rel, key, None, initial);
        let state = self.ai_tracked_creature_state(ai, c);
        if let Some(r) = t.rep_mut(key) {
            r.dynamic_relationship = Some(DynamicRelationship::new(initial, module, state));
        }
        rel.relationships.push(key);
    }

    /// `RelationshipTracker.Update`: relationships whose representation is deleted go,
    /// the others update (last first): the AI's relationship this tick, re-sorted on a
    /// change of type, and the representation's priority its intensity.
    pub(super) fn relationships_update(
        &mut self,
        ai: CreatureId,
        t: &mut Tracker,
        rel: &mut RelationshipTracker,
    ) {
        for n in (0..rel.relationships.len()).rev() {
            // A module change may have forgotten relationships above `n`.
            let Some(&key) = rel.relationships.get(n) else {
                continue;
            };
            let Some(i) = t.index_of(key).filter(|&i| !t.reps[i].delete_me_next_frame) else {
                rel.relationships.remove(n);
                continue;
            };
            let new = self.ai_update_dynamic_relationship(ai, t, i);
            let Some(d) = t.reps[i].dynamic_relationship.as_ref() else {
                continue;
            };
            let (old_kind, old_module) = (d.current.kind, d.tracked_by_module);
            if new.kind != old_kind {
                let module = self
                    .world
                    .sort_creature_into_module(ai, t, rel, key, old_module, new);
                if let Some(d) = t.rep_mut(key).and_then(|r| r.dynamic_relationship.as_mut()) {
                    d.tracked_by_module = module;
                }
            }
            if let Some(r) = t.rep_mut(key) {
                let weight = r
                    .dynamic_relationship
                    .as_ref()
                    .map_or(1.0, |d| d.tracked_by_module_weight);
                r.priority = new.intensity * weight;
                if let Some(d) = &mut r.dynamic_relationship {
                    d.current = new;
                }
            }
        }
    }

    /// The listed representations and the dropped ones still held, for the looker,
    /// with what reading them now gives.
    pub(crate) fn look_candidates(
        &self,
        ai: CreatureId,
        t: &Tracker,
        held: Option<RepKey>,
    ) -> Vec<crate::creature::LookCandidate> {
        let me = self.world.creature(ai);
        let (head, neck) = self.ai_head_and_neck(ai);
        let grabbed: Vec<CreatureId> = me
            .grasps
            .iter()
            .flatten()
            .filter_map(|g| g.grabbed.creature())
            .collect();
        let listed = t.reps.iter().map(|r| (r, true));
        let detached = t
            .detached
            .iter()
            .filter(|r| Some(r.key) == held)
            .map(|r| (r, false));
        listed
            .chain(detached)
            .map(|(r, listed)| {
                let c = self.world.creature(r.creature);
                let body = self.tracked_body(r.creature);
                let guess = self.peek_best_guess(ai, r);
                let to = (Vec2::middle_of_tile(guess.tile) - neck).normalized();
                let dot = (head - neck).normalized().dot(to);
                crate::creature::LookCandidate::new(crate::creature::LookCandidateParts {
                    key: r.key,
                    creature: r.creature,
                    listed,
                    best_guess: guess,
                    visual_field: r.visual_contact,
                    in_view: self.rep_in_view(ai, r.creature),
                    lowest_generation: r.lowest_generation(),
                    ticks_since_seen: r.ticks_since_seen(t.see_around_corners),
                    body_size: c.template.body_size,
                    scaryness: c.template.scaryness,
                    relationship: me.template.relationship_to(c.kind()),
                    main_pos: body.as_ref().map(|b| b.main().pos),
                    main_vel: body.as_ref().map(|b| b.main().vel),
                    interest: dot.clamp(0.1, 1.0).lerp(1.0, 0.5),
                    grabbed: grabbed.contains(&r.creature),
                })
            })
            .collect()
    }

    /// The representations of AI `ai`'s tracker as they read now (nothing settled).
    #[must_use]
    pub fn tracker_readings(&self, ai: CreatureId) -> Vec<crate::creature::RepReading> {
        let Some(t) = self.world.tracker_of(ai) else {
            return Vec::new();
        };
        t.reps
            .iter()
            .map(|r| {
                let mut sighting = r.sighting(t.see_around_corners);
                sighting.visual_contact &= self.rep_in_view(ai, r.creature);
                crate::creature::RepReading {
                    creature: r.creature,
                    elaborate: matches!(r.kind, RepKind::Elaborate(_)),
                    age: r.age,
                    ticks_since_seen: r.ticks_since_seen,
                    forget_counter: r.forget_counter,
                    visual_contact: sighting.visual_contact,
                    estimated_chance_of_finding: sighting.estimated_chance_of_finding(),
                    best_guess: self.peek_best_guess(ai, r),
                    last_seen: r.last_seen_coord,
                    priority: r.priority,
                    delete_me_next_frame: r.delete_me_next_frame,
                    ghosts: match &r.kind {
                        RepKind::Elaborate(e) => e.ghosts.clone(),
                        RepKind::Simple { .. } => Vec::new(),
                    },
                    relationship: r.dynamic_relationship.clone(),
                }
            })
            .collect()
    }

    /// Applies what the looker's reads changed: the getter's cleared fields and the
    /// guesses it settled.
    pub(crate) fn apply_look_reads(
        &self,
        ai: CreatureId,
        t: &mut Tracker,
        candidates: &[crate::creature::LookCandidate],
    ) {
        for c in candidates {
            if let Some(r) = t.rep_mut(c.key) {
                r.visual_contact = c.visual_field.get();
                if c.guessed.get() {
                    self.rep_best_guess(ai, r);
                }
            }
        }
    }
}

impl crate::world::World {
    /// `Custom.BetweenRoomsDistance(a, b)`: tiles apart in one room; else through the
    /// exits joining the rooms (each side's distance to its exit, or the room's
    /// diagonal while it is not realized); `f32::MAX` for rooms not joined.
    #[must_use]
    pub fn between_rooms_distance(&self, a: WorldCoordinate, b: WorldCoordinate) -> f32 {
        if a.room == b.room {
            return a.tile.float_dist(b.tile);
        }
        let (Some(ra), Some(rb)) = (self.abstract_room(a.room), self.abstract_room(b.room)) else {
            return f32::MAX;
        };
        let (ea, eb) = (ra.exit_index(b.room), rb.exit_index(a.room));
        if ea < 0 || eb < 0 {
            return f32::MAX;
        }
        self.exit_distance(ra, a, ea) + self.exit_distance(rb, b, eb)
    }

    /// `BetweenRoomsDistance`'s one side: from `at` to exit `exit` of `room` while its
    /// shortcuts are ready, else the room's diagonal.
    fn exit_distance(
        &self,
        room: &rw_assets::world::AbstractRoomData,
        at: WorldCoordinate,
        exit: i32,
    ) -> f32 {
        let diagonal = Vec2::new(room.size.0 as f32, room.size.1 as f32).length();
        self.realized(room.index)
            .filter(|r| r.shortcuts_ready())
            .and_then(|r| {
                r.room
                    .shortcut_map()
                    .leading_to_node(usize::try_from(exit).ok()?)
                    .map(|s| at.tile.float_dist(s.start))
            })
            .unwrap_or(diagonal)
    }
}
