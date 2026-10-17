//! A realized lizard's AI and movement: the stand-in `LizardAI.Update` with the base
//! `ArtificialIntelligence.Update` (its path finder, stuck and discomfort trackers),
//! `Lizard.Act` (`FollowPath`, the movement animations, `FollowConnection`,
//! `GripPointBehavior`, `SwimBehavior`, `GetFrameSpeed`, the wiggle, desperation and
//! shake), `ActAnimation`, `EnterAnimation`, and entering shortcuts (`Creature.Update`'s
//! `enteringShortCut` branch, `SuckedIntoShortCut`, `ShortcutHandler.SuckInCreature`).
//! Decompile: `Lizard.cs` 497-666 and 1730-2985, `LizardAI.cs` 755-1160,
//! `ArtificialIntelligence.cs` 245-334, `Creature.cs` 635-690 and 1022-1070,
//! `ShortcutHandler.cs` 365-380. Behaviour: `rainworld-re/notes/creature-model.md`
//! section 14.

use std::sync::Arc;

use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_assets::{CreatureType, Room, ShortcutType, Terrain};

use super::{PlayerObserver, RainWorldGame};
use crate::creature::lizard::{AnimationOutcome, LizardEmotion, MovementAnimation};
use crate::creature::{CreatureId, LizardAnimation, WorldCoordinate};
use crate::item::Entity;
use crate::room_ext::RoomExt;
use crate::shortcut::{CreatureVessel, ShortcutVessel, Transport};
use crate::world::{Accessibility, AiMap, MovementConnection, MovementType};
use crate::{FloatExt, SoundId, Vec2, Water};

/// `Custom.eightDirectionsDiagonalsLast`.
const EIGHT_DIRECTIONS_DIAGONALS_LAST: [(i32, i32); 8] = [
    (-1, 0),
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, -1),
    (1, -1),
    (1, 1),
    (-1, 1),
];

/// What `Act` reads of the lizard's room.
struct ActRoom {
    index: i32,
    room: Arc<Room>,
    map: Arc<AiMap>,
    water: Option<Water>,
}

impl ActRoom {
    /// `Room.GetWorldCoordinate(pos)`.
    fn coordinate(&self, pos: Vec2) -> WorldCoordinate {
        let t = pos.tile();
        WorldCoordinate::new(self.index, t.0, t.1, -1)
    }

    /// `Room.GetTile(tile).AnyWater`.
    fn any_water(&self, tile: (i32, i32)) -> bool {
        self.map.tile(tile.0, tile.1).any_water()
    }

    fn accessible(&self, tile: (i32, i32), template: &crate::creature::CreatureTemplate) -> bool {
        self.map.tile_accessible_to_creature(tile, template)
    }

    /// `Room.FloatWaterLevel(x)`.
    fn float_water_level(&self, x: f32) -> f32 {
        self.water.as_ref().map_or(f32::MIN, |w| w.level_at(x))
    }
}

impl crate::creature::Lizard {
    /// `PhysicalObject.IsTileSolid`: Solid, or a one-way floor below a chunk not going
    /// through floors.
    fn is_tile_solid(&self, room: &Room, i: usize, dx: i32, dy: i32) -> bool {
        let (x, y) = self.body.chunks[i].pos.tile();
        match room.tile(x + dx, y + dy).terrain {
            Terrain::Solid => true,
            Terrain::Floor => dy < 0 && !self.body.chunks[i].go_through_floors,
            _ => false,
        }
    }
}

impl RainWorldGame {
    fn act_room(&self, index: i32) -> Option<ActRoom> {
        let r = self.world.realized(index)?;
        Some(ActRoom {
            index,
            room: Arc::clone(&r.room),
            map: r.ai_map.clone()?,
            water: r.water.clone(),
        })
    }

    /// `LizardAI.Update` as the stand-in runs it: the panic count down, the room clock,
    /// the abstract AI's `AbstractBehavior(1)` (a realized creature's
    /// `MigrationBehavior`) on a roll of `1 / LerpMap(RoomLike, 0.35, 1, 10, 1000)`
    /// (red lizards 1/50), the social memory's temps evened out, walking past the point
    /// of no return when stranded or the den is not getbackable, the jaw closing a
    /// little, `runSpeed` at least the stuck tracker's utility, the casual aggression;
    /// then the base update: the room clock again and the modules (the path finder, the
    /// tracker, the relationship tracker, the stuck tracker; the discomfort tracker has
    /// nothing to update). `DetermineBehavior` and the behaviours are lizard step 7.
    pub(super) fn lizard_ai_update(&mut self, index: i32, id: CreatureId) {
        let l = self.world.lizard_at(id);
        if l.ai.panic > 0 {
            l.ai.panic -= 1;
        }
        l.ai.time_in_room += 1;
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
        // `fear`, `hunger` and `rainFear` are the smoothed utilities of trackers that
        // track nothing until lizard step 6, so 0.
        let l = self.world.lizard_at(id);
        l.ai.excitement = l.ai.excitement.lerp(0.0, 0.1);
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_at(id);
        let jaw = l.read_jaw_open(holding) - 0.01;
        l.set_jaw_open(jaw, holding);
        // The behaviours that set `runSpeed` are lizard step 7: 0, or the dev override.
        let base = l.ai.dev_run_speed.unwrap_or(0.0);
        l.ai.run_speed = base.max(l.ai.stuck.utility());
        self.lizard_casual_aggression(id);
        self.world.lizard_at(id).ai.time_in_room += 1;
        if let Some(p) = pf {
            self.world
                .with_path_finders(|pfs, w| pfs.update(p, w, &mut self.rng));
        }
        self.lizard_perception_update(id);
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
    }

    /// `LizardAI.RoomLike`: the room's size-dependent attraction, times 0.9 for each
    /// other lizard in it (a friend's room, lizard step 7, would be 1).
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

    /// `Lizard.Act`: the AI's update; footing (time on allowed terrain, gravity until
    /// footing is regained, the swim fading); the animation's speed; the path's next
    /// move from the head's tile (or a reachable neighbour) and the connections and
    /// terrain squeeze it sets; how long it has tried this move, desperation and the
    /// wiggle count; a shortcut move starts entering it, a reach starts its movement
    /// animation, and the five moves ahead are read for narrow spaces, poles and
    /// shortcuts. Then the animation clock, the bubble roll, and the movement itself:
    /// gripping a pole, the movement animation, swimming or following the connection;
    /// without a move, a random step onto accessible ground or a nudge of the middle.
    /// Last the desperate shove, the shake and the wiggle.
    pub(super) fn lizard_act(&mut self, index: i32, id: CreatureId) {
        self.lizard_ai_update(index, id);
        if self
            .world
            .creature(id)
            .lizard()
            .is_none_or(|l| l.room != Some(index))
        {
            return;
        }
        let Some(r) = self.act_room(index) else {
            return;
        };
        let t = self.world.creature(id).template;
        let l = self.world.lizard_at(id);
        let mut head_ok = false;
        let mut rest_ok = false;
        for (i, c) in l.body.chunks.iter().enumerate() {
            if r.accessible(c.pos.tile(), &t) {
                head_ok |= i == 0;
                rest_ok |= i > 0;
            }
        }
        if (head_ok || rest_ok || l.movement_animation.is_some())
            && l.in_allowed_terrain_counter < 100
        {
            l.in_allowed_terrain_counter += 1;
        } else {
            let near = l.body.chunks.iter().any(|c| {
                let (x, y) = c.pos.tile();
                FOUR_DIRECTIONS.iter().any(|&(dx, dy)| {
                    let n = (x + dx, y + dy);
                    r.accessible(n, &t) && !r.any_water(n)
                })
            });
            l.in_allowed_terrain_counter = if near {
                (l.in_allowed_terrain_counter - 10).max(0)
            } else {
                0
            };
        }
        l.apply_gravity = l.in_allowed_terrain_counter < l.params.regain_footing_counter
            || l.no_grip_counter() > 10
            || l.commited_to_drop_connection.is_some();
        l.swim = (l.swim - 0.05).clamp(0.0, 1.0);
        l.ai.stuck.satisfied_with_this_position = l.swim > 0.5;
        let speed = self.lizard_act_animation(index, id, None);
        let pf = self.world.path_finder_of(id);
        let l = self.world.lizard_ref(id);
        let mut coord = r.coordinate(l.head().pos);
        if l.movement_animation.is_none()
            && coord.tile_defined()
            && !self.reachable_and_getbackable(pf, coord)
        {
            for (dx, dy) in EIGHT_DIRECTIONS_DIAGONALS_LAST {
                let n = WorldCoordinate::new(index, coord.tile.0 + dx, coord.tile.1 + dy, -1);
                if self.reachable_and_getbackable(pf, n) {
                    coord = n;
                    break;
                }
            }
        }
        let body_dir = l.body_direction();
        let mut following = self.lizard_follow_path(id, coord, Some(body_dir), true);
        let l = self.world.lizard_at(id);
        if let Some(c) = l.commited_to_drop_connection.take() {
            following = Some(c);
        }
        let head = l.head().pos;
        let farther = following.is_none_or(|c| {
            let m = Vec2::middle_of_tile(c.dest_tile());
            head.distance(m) > l.body.chunks[1].pos.distance(m)
        });
        let pull = crate::object::ConnectionKind::Pull;
        if farther && l.time_spent_trying_this_move > 10 {
            l.body.connections[0].kind = pull;
            l.body.connections[1].kind = pull;
        } else {
            l.body.connections[0].kind = crate::object::ConnectionKind::Normal;
            l.body.connections[1].kind = pull;
        }
        let squeeze = (l.time_spent_trying_this_move as f32).lerp_map(10.0, 30.0, 1.0, 0.05);
        for c in &mut l.body.chunks {
            c.terrain_squeeze = squeeze;
            c.collide_with_slopes = l.time_spent_trying_this_move < 30;
        }
        let back = (1.0f32).lerp(20.0, speed) as i32;
        if speed > 0.1
            && (following.is_none() || l.ai.stuck.move_backlog.is_move_in_log(following, back))
        {
            l.time_spent_trying_this_move = (l.time_spent_trying_this_move + 1).clamp(0, 600);
        } else {
            l.time_spent_trying_this_move = 0;
        }
        let target = (l.time_spent_trying_this_move as f32 + (-300.0f32).lerp(0.0, speed))
            .max(l.ai.stuck.utility() * 100.0);
        l.desperation_smoother = l.desperation_smoother.lerp_and_tick(target, 0.05, 0.5);
        if l.desperation_smoother > 40.0f32.lerp(10.0, speed) {
            l.set_body_wiggle_counter(l.body_wiggle_counter + 2);
        } else {
            l.set_body_wiggle_counter(l.body_wiggle_counter - 1);
        }
        l.ai.stuck.move_backlog.report_new_move(following);
        l.upcoming_connections.clear();
        l.following_connection = following;
        if let Some(fc) = following {
            l.limbs_aim_for = Vec2::middle_of_tile(fc.dest_tile());
            match fc.kind {
                MovementType::ShortCut | MovementType::NpcTransportation => {
                    l.entering_shortcut = Some(fc.start_tile());
                    if fc.kind == MovementType::NpcTransportation {
                        l.npc_transportation_destination = Some(fc.dest);
                    }
                }
                MovementType::ReachUp
                | MovementType::ReachOverGap
                | MovementType::ReachDown
                | MovementType::LizardTurn
                | MovementType::SemiDiagonalReach
                | MovementType::DoubleReachUp => {
                    let chunks = [
                        l.body.chunks[0].pos,
                        l.body.chunks[1].pos,
                        l.body.chunks[2].pos,
                    ];
                    l.movement_animation = Some(MovementAnimation::new(chunks, fc, 1, 0.99, 0.75));
                }
                _ => {}
            }
            self.lizard_scan_upcoming(&r, id, fc);
        }
        self.lizard_animation_clock(id);
        self.lizard_bubble_roll(id);
        let l = self.world.lizard_ref(id);
        let submersion = l.body.submersion(r.water.as_ref());
        let pos = self.world.creature(id).pos;
        let following = l.following_connection;
        if l.grip_point.is_some() {
            self.lizard_grip_point_behavior(&r, id);
        } else if l.movement_animation.is_some() {
            let run = l.ai.run_speed;
            let fac = speed * l.params.base_speed * 0.5;
            let outcome =
                self.world
                    .lizard_at(id)
                    .movement_animation_update(fac, run, &mut self.rng);
            let l = self.world.lizard_at(id);
            match outcome {
                AnimationOutcome::Running => {}
                AnimationOutcome::NextStep(a) => l.movement_animation = Some(a),
                AnimationOutcome::Ended => l.movement_animation = None,
            }
        } else if following.is_some_and(|fc| {
            !r.map.tile_at(pos).narrow_space && !r.any_water(fc.dest_tile()) && submersion > 0.5
        }) || following
            .is_some_and(|fc| fc.dest.tile_defined() && r.any_water(fc.dest_tile()))
            || (following.is_none() && submersion > 0.5 && submersion > 0.2)
        {
            self.lizard_swim_behavior(&r, id);
        } else if following.is_some() {
            self.lizard_follow_connection(&r, id, speed);
        }
        self.lizard_act_end(&r, id, head_ok, rest_ok);
    }

    /// A dev override (MCP `set_destination`): `AbstractCreatureAI.SetDestination` of a
    /// realized lizard, and the run speed its AI keeps until set again (`None` returns it
    /// to the stand-in's 0, which the stuck tracker's utility raises). Returns whether
    /// `id` is a realized lizard.
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

    /// `CoordinateReachableAndGetbackable` of path finder `pf`.
    fn reachable_and_getbackable(&self, pf: Option<usize>, c: WorldCoordinate) -> bool {
        pf.and_then(|p| self.world.path_finders.get(p))
            .is_some_and(|f| f.coordinate_reachable_and_getbackable(c))
    }

    /// `FollowPath` of lizard `id`'s path finder.
    fn lizard_follow_path(
        &mut self,
        id: CreatureId,
        origin: WorldCoordinate,
        body_direction: Option<usize>,
        actually: bool,
    ) -> Option<MovementConnection> {
        let pf = self.world.path_finder_of(id)?;
        let rng = &mut self.rng;
        self.world.with_path_finders(|pfs, w| {
            pfs.follow_path(pf, w, origin, body_direction, actually, rng)
        })
    }

    /// The scan of up to five moves ahead (`upcomingConnections`): a destination between
    /// two solid tiles is narrow, a pole is a climb, a shortcut move is a shortcut; each
    /// next move is the path's from the last one's destination, stopping at a repeated
    /// destination or none.
    fn lizard_scan_upcoming(&mut self, r: &ActRoom, id: CreatureId, first: MovementConnection) {
        let mut next = Some(first);
        let l = self.world.lizard_at(id);
        l.narrow_upcoming = false;
        l.climb_upcoming = false;
        l.shortcut_upcoming = false;
        for _ in 0..5 {
            let Some(c) = next else { break };
            let l = self.world.lizard_at(id);
            let (x, y) = c.dest_tile();
            let s = &r.room;
            if !l.narrow_upcoming
                && ((s.is_solid_tile(x, y + 1) && s.is_solid_tile(x, y - 1))
                    || (s.is_solid_tile(x + 1, y) && s.is_solid_tile(x - 1, y)))
            {
                l.narrow_upcoming = true;
            }
            if !l.climb_upcoming && r.map.tile_at(c.dest).acc == Accessibility::Climb {
                l.climb_upcoming = true;
            }
            l.upcoming_connections.push(c);
            if !l.shortcut_upcoming && c.kind == MovementType::ShortCut {
                l.shortcut_upcoming = true;
            }
            next = self.lizard_follow_path(id, c.dest, None, false);
            let l = self.world.lizard_ref(id);
            if next.is_some_and(|n| l.upcoming_connections.iter().any(|u| u.dest == n.dest)) {
                next = None;
            }
        }
    }

    /// The animation clock: past its time the animation returns to Standard.
    fn lizard_animation_clock(&mut self, id: CreatureId) {
        let l = self.world.lizard_at(id);
        l.time_in_animation += 1;
        if l.time_to_remain_in_animation > -1 && l.time_in_animation > l.time_to_remain_in_animation
        {
            self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
        }
    }

    /// Without a bubble, a 5% roll then two rolls under the AI's excitement start one
    /// (a fourth draw for its intensity).
    fn lizard_bubble_roll(&mut self, id: CreatureId) {
        let l = self.world.lizard_ref(id);
        let excitement = l.ai.excitement;
        if l.bubble == 0
            && self.rng.value() < 0.05
            && self.rng.value() < excitement
            && self.rng.value() < excitement
        {
            let v = self.rng.value();
            let l = self.world.lizard_at(id);
            l.bubble_intensity = excitement * excitement * v;
            l.bubble = 1;
        }
    }

    /// The end of `Act`: without a move or an animation, a random step (one draw) onto
    /// accessible ground from an inaccessible head while held up, or the middle nudged
    /// toward ground when only the head has it after 20 tries; the desperate shove (two
    /// draws), the stiffness, the shake, the wiggle and the remembered move.
    fn lizard_act_end(&mut self, r: &ActRoom, id: CreatureId, head_ok: bool, rest_ok: bool) {
        let t = self.world.creature(id).template;
        let l = self.world.lizard_ref(id);
        if l.following_connection.is_none() && l.movement_animation.is_none() {
            let head = l.head().pos;
            if !l.apply_gravity && !r.accessible(head.tile(), &t) {
                let d = FOUR_DIRECTIONS[usize::try_from(self.rng.range_i32(0, 4)).unwrap_or(0)];
                let to = (head.tile().0 + d.0, head.tile().1 + d.1);
                if r.accessible(to, &t) {
                    let from = r.coordinate(head);
                    let dest = WorldCoordinate::new(r.index, to.0, to.1, -1);
                    self.world.lizard_at(id).following_connection = Some(MovementConnection::new(
                        MovementType::Standard,
                        from,
                        dest,
                        1,
                    ));
                }
            } else if head_ok && !rest_ok && l.time_spent_trying_this_move > 20 {
                let l = self.world.lizard_at(id);
                let c = &mut l.body.chunks[1];
                let (x, y) = c.pos.tile();
                for (dx, dy) in FOUR_DIRECTIONS {
                    if r.accessible((x + dx, y + dy), &t) {
                        c.vel += c.pos.direction_to(Vec2::middle_of_tile((x + dx, y + dy)));
                        break;
                    }
                }
            }
        }
        let l = self.world.lizard_ref(id);
        if l.body_desperation() > 0.0 && l.swim == 0.0 {
            let dir = Vec2::from_degrees(self.rng.value() * 360.0);
            let l = self.world.lizard_at(id);
            let v =
                dir * (l.body_desperation() * l.params.max_muscle_power * 2.0 * self.rng.value());
            l.body.chunks[0].vel += v * 0.5;
            l.body.chunks[1].vel -= v;
            l.body.chunks[2].vel += v * 0.5;
        }
        let l = self.world.lizard_at(id);
        l.body.connections[2].elasticity = 0.7 + 0.3 / l.body_force();
        if l.animation == LizardAnimation::ShakePrey {
            self.lizard_shake(id);
        }
        let l = self.world.lizard_at(id);
        let fac = l.body_wiggle_fac();
        if fac > 0.0 && l.swim < 0.5 {
            let ws = l.params.wiggle_speed;
            l.body_wiggle += 0.05f32.lerp(0.15, fac) * 0.5f32.lerp(0.8, ws);
            let s = f64::from(l.body_wiggle * 2.0 * std::f32::consts::PI).sin() as f32;
            let c = &l.body.chunks;
            let a = c[1].pos.direction_to(c[0].pos);
            let b = c[2].pos.direction_to(c[1].pos);
            let perp = a.slerp(b, 0.5).perpendicular();
            let amp = (fac + 2.0) / ((1.0 - ws).mathf_pow(2.0) + 2.0);
            let v = perp * (s * amp);
            l.body.chunks[0].vel += v;
            l.body.chunks[1].vel -= v * 2.0;
            l.body.chunks[2].vel += v;
        }
        if l.following_connection.is_some() {
            l.last_following_connection = l.following_connection;
        }
        l.straighten_out_needed = l.straighten_out_needed.lerp_and_tick(0.0, 0.1, 0.05);
    }

    /// The shake of `ShakePrey`: the wiggle counts down from 50; a random shove (two
    /// draws), halved with nothing held, else scaled by how close the held chunk is to
    /// half the head's mass (a held weight far from it may end the shake, a draw).
    fn lizard_shake(&mut self, id: CreatureId) {
        let l = self.world.lizard_at(id);
        l.set_body_wiggle_counter((50 - l.time_in_animation).clamp(0, 100));
        let dir = Vec2::from_degrees(self.rng.value() * 360.0);
        let mut v = dir * (12.0 * self.rng.value());
        let grasp = self.world.creature(id).grasps.first().copied().flatten();
        match grasp {
            None => v *= 0.5,
            Some(g) => {
                let mass = self.grasped_chunk_mass(g.grabbed, g.chunk_grabbed);
                let l = self.world.lizard_at(id);
                let half = l.head().mass * 0.5;
                let fit = (mass - half).abs().inverse_lerp(half, 0.0).mathf_pow(1.5);
                v *= fit;
                let w = (l.body_wiggle_counter as f32 * fit.lerp(1.0, 0.5)) as i32;
                l.set_body_wiggle_counter(w);
                if fit == 0.0 && self.rng.value() < 0.025 {
                    self.world.lizard_at(id).set_body_wiggle_counter(0);
                    self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
                }
            }
        }
        let l = self.world.lizard_at(id);
        l.body.chunks[0].vel -= v * 0.5;
        l.body.chunks[1].vel += v;
        l.body.chunks[2].vel -= v * 0.5;
    }

    /// The mass of chunk `i` of what a grasp holds.
    fn grasped_chunk_mass(&self, e: Entity, i: usize) -> f32 {
        if e == Entity::Creature(self.player_id) {
            return self
                .player
                .as_ref()
                .and_then(|p| p.body.chunks.get(i))
                .map_or(0.0, |c| c.mass);
        }
        self.world
            .body_chunks(e)
            .and_then(|c| c.get(i))
            .map_or(0.0, |c| c.mass)
    }

    /// `ActAnimation`: the animation's run speed and its physics. Standard runs at the
    /// AI's `runSpeed`; the spotted and threat animations stop or slow and set the
    /// jaw (draws as the game's); the fighting stance and the threat leaning need the
    /// focus creature (lizard step 7); the tongue, spit and jump animations belong to
    /// breeds not entered yet and fall back to Standard; the lounge's crouch pushes at
    /// the focus and its leap propels the body along `loungeDir`, ending in a snap of
    /// the jaws; the shake runs at 0.75. A post-lounge stun slows it to a tenth.
    pub(super) fn lizard_act_animation(
        &mut self,
        index: i32,
        id: CreatureId,
        focus: Option<Vec2>,
    ) -> f32 {
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_at(id);
        let run = l.ai.run_speed;
        let mut num = 0.0;
        match l.animation {
            LizardAnimation::Standard => num = run,
            LizardAnimation::HearSound => l.set_jaw_open(0.2, holding),
            LizardAnimation::PreySpotted => l.set_jaw_open(1.0, holding),
            LizardAnimation::PreyReSpotted => {
                num = run * 0.3;
                let jaw = l.read_jaw_open(holding);
                let t = self.rng.value() * self.rng.value();
                self.world
                    .lizard_at(id)
                    .set_jaw_open(jaw.lerp(1.0, t), holding);
            }
            LizardAnimation::ThreatSpotted => {
                num = 0.5;
                let v = self.rng.value();
                self.world.lizard_at(id).set_jaw_open(v, holding);
            }
            LizardAnimation::FightingStance => l.set_jaw_open(1.0, holding),
            LizardAnimation::ThreatReSpotted => {
                num = run * 0.8;
                let jaw = l.read_jaw_open(holding);
                let to = self.rng.value();
                let t = self.rng.value();
                self.world
                    .lizard_at(id)
                    .set_jaw_open(jaw.lerp(to, t), holding);
            }
            LizardAnimation::ShootTongue => {
                l.set_body_wiggle_counter(0);
                num = 0.2;
                // Not hunting: back to Standard (no tongue on the entered breeds).
                self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
            }
            LizardAnimation::Spit | LizardAnimation::PrepareToJump => {
                log::warn!("{:?} on a breed without it", l.animation);
                self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
            }
            LizardAnimation::Jumping => {}
            LizardAnimation::PrepareToLounge => {
                l.set_body_wiggle_counter(0);
                // `AI.behavior == Hunt` with a seen focus creature: lizard step 7.
                if !l.params.can_exit_lounge_warm_up {
                    match focus {
                        Some(at) => {
                            if l.legs_gripping() > 0 {
                                let push = l.params.pre_lounge_crouch_movement;
                                for c in &mut l.body.chunks {
                                    c.vel += c.pos.direction_to(at) * push;
                                }
                            }
                            if l.time_in_animation >= l.params.pre_lounge_crouch {
                                self.lizard_enter_animation(
                                    id,
                                    LizardAnimation::Lounge,
                                    false,
                                    focus,
                                );
                            }
                        }
                        None => {
                            self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
                        }
                    }
                } else {
                    self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
                }
            }
            LizardAnimation::Lounge => {
                // `canExitLounge` keeps leaping only while hunting a seen focus (step 7).
                let leaping = l.time_in_animation < l.params.lounge_maximum_frames
                    && !l.params.can_exit_lounge;
                if leaping {
                    let jaw = l.read_jaw_open(holding) + 0.1;
                    l.set_jaw_open(jaw, holding);
                    if l.time_in_animation < l.params.lounge_propulsion_frames
                        && l.legs_gripping() > 0
                    {
                        let push = l.lounge_dir * l.params.lounge_speed;
                        for (k, c) in l.body.chunks.iter_mut().enumerate() {
                            c.vel += push / (k + 1) as f32;
                        }
                    }
                } else {
                    let head = l.head().pos;
                    let at = head + l.body.chunks[1].pos.direction_to(head) * 10.0;
                    self.lizard_jaws_snap_shut(id, at);
                    self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
                    let l = self.world.lizard_at(id);
                    l.post_lounge_stun = 40;
                    l.in_allowed_terrain_counter = 0;
                }
            }
            LizardAnimation::ShakePrey => {
                num = 0.75;
                if !holding && self.rng.value() < 0.025 {
                    self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
                }
            }
        }
        let _ = index;
        let l = self.world.lizard_ref(id);
        if l.post_lounge_stun > 0 {
            num *= 0.1;
        }
        if self.world.creature(id).kind() == CreatureType::RedLizard
            && l.ai.run_speed > 0.1
            && l.animation != LizardAnimation::Spit
        {
            // `AI.hunger` is the prey tracker's (lizard step 6): 0.
            num = num.lerp(1.0, 0.2);
        }
        num
    }

    /// `EnterAnimation`: a lower-priority animation unless forced, or the same one, is
    /// ignored; else the clock restarts and the animation sets its time, bubble and
    /// voice. The lounge sets its delay (a draw), its direction
    /// (toward `focus` when there is one), leaves at once when that points backward for
    /// a breed that can exit lounges, and plays its sounds.
    pub(super) fn lizard_enter_animation(
        &mut self,
        id: CreatureId,
        anim: LizardAnimation,
        force: bool,
        focus: Option<Vec2>,
    ) {
        let l = self.world.lizard_at(id);
        if (!force && anim < l.animation) || l.animation == anim {
            return;
        }
        l.time_in_animation = 0;
        l.animation = anim;
        let black = self.world.creature(id).kind() == CreatureType::BlackLizard;
        let room = self.lizard_room(id);
        let l = self.world.lizard_at(id);
        match anim {
            LizardAnimation::Standard => l.time_to_remain_in_animation = -1,
            LizardAnimation::HearSound => {
                l.time_to_remain_in_animation = 40;
                l.bubble = 40;
                l.bubble_intensity = 0.1;
                if !black {
                    self.lizard_voice(id, LizardEmotion::Curious);
                }
            }
            LizardAnimation::PreySpotted => {
                l.bubble = 30;
                l.bubble_intensity = 1.0;
                l.time_to_remain_in_animation = 30;
                self.lizard_voice(id, LizardEmotion::SpottedPreyFirstTime);
            }
            LizardAnimation::PreyReSpotted => {
                l.bubble = 10;
                l.bubble_intensity = 0.4;
                l.time_to_remain_in_animation = 10;
                // A yellow lizard's pack voices are its breed's (not entered yet).
                if !black {
                    self.lizard_voice(id, LizardEmotion::ReSpottedPrey);
                }
            }
            LizardAnimation::ThreatSpotted => {
                l.bubble_intensity = 1.0;
                let t = self.rng.range_i32(10, 50);
                self.world.lizard_at(id).time_to_remain_in_animation = t;
                self.lizard_voice(id, LizardEmotion::Fear);
            }
            LizardAnimation::ThreatReSpotted => {
                l.bubble_intensity = 1.0;
                l.time_to_remain_in_animation = 10;
            }
            LizardAnimation::ShootTongue => {
                // `tongueWarmUp` of a breed with a tongue (none entered yet).
                l.time_to_remain_in_animation = 0;
                let head = l.head().pos;
                self.play_on_object(room, SoundId::LizardPrepareToShootTongue, head);
            }
            LizardAnimation::Spit => l.time_to_remain_in_animation = 200,
            LizardAnimation::PrepareToJump => {
                l.bubble = 3;
                l.bubble_intensity = 0.1;
            }
            LizardAnimation::Jumping => {
                l.time_to_remain_in_animation = -1;
                l.body.set_go_through_floors(true);
            }
            LizardAnimation::PrepareToLounge => {
                l.bubble = 3;
                l.bubble_intensity = 0.1;
                l.time_to_remain_in_animation = l.params.pre_lounge_crouch + 5;
                let head = l.head().pos;
                if let Some(at) = focus {
                    l.lounge_dir = head.direction_to(at);
                }
                self.play_on_object(room, SoundId::LizardPrepareLungeAttackInit, head);
            }
            LizardAnimation::Lounge => self.lizard_enter_lounge(id, focus),
            LizardAnimation::ShakePrey => {
                l.bubble_intensity = 0.0;
                let n = l.params.shake_prey;
                let t = self.rng.range_i32(n, n * 2);
                let other = self
                    .world
                    .creature(id)
                    .grasps
                    .first()
                    .copied()
                    .flatten()
                    .and_then(|g| g.grabbed.creature());
                let intensity =
                    other.map_or(0.0, |o| self.world.dynamic_relationship(id, o).intensity);
                self.world.lizard_at(id).time_to_remain_in_animation =
                    (t as f32 * intensity) as i32;
            }
            LizardAnimation::FightingStance => {
                l.bubble = 10;
                l.bubble_intensity = 1.0;
                let t = self.rng.range_i32(20, 50);
                self.world.lizard_at(id).time_to_remain_in_animation = t;
            }
        }
    }

    /// `EnterAnimation(Lounge)`.
    fn lizard_enter_lounge(&mut self, id: CreatureId, focus: Option<Vec2>) {
        let room = self.lizard_room(id);
        let l = self.world.lizard_ref(id);
        let double = self.rng.value() < l.params.risk_of_double_lounge_delay;
        let l = self.world.lizard_at(id);
        l.lounge_delay =
            l.params.lounge_delay * if double { 2 } else { 1 } + l.params.lounge_maximum_frames;
        let head = l.head().pos;
        if let Some(at) = focus {
            l.lounge_dir = l
                .lounge_dir
                .slerp(head.direction_to(at), l.params.find_lounge_direction);
        }
        let body = l.body.chunks[1].pos.direction_to(head);
        if body.dot(l.lounge_dir) < 0.0 {
            if l.params.can_exit_lounge || l.params.can_exit_lounge_warm_up {
                self.lizard_enter_animation(id, LizardAnimation::Standard, true, None);
            } else {
                l.lounge_dir = body;
            }
        }
        let l = self.world.lizard_at(id);
        if l.lounge_dir.y > 0.0 {
            l.lounge_dir.y += l.params.lounge_jumpyness;
        }
        l.lounge_dir.y *= 0.5;
        l.time_to_remain_in_animation = l.params.lounge_maximum_frames + 5;
        let head = l.head().pos;
        self.play_on_object(room, SoundId::LizardLungeAttackInit, head);
    }

    /// `GetFrameSpeed`: the legs' grip share (desperation lifting it), a wiggling body
    /// against terrain still moving a little, the wiggle count, the breed's speed by the
    /// run speed and a health roll (a draw), and the three chunks' terrain speeds for
    /// the move's direction.
    fn lizard_frame_speed(&mut self, r: &ActRoom, id: CreatureId, run_speed: f32) -> f32 {
        let health = self.world.creature(id).state.health;
        let l = self.world.lizard_at(id);
        let mut num = l.legs_gripping() as f32 / 4.0;
        num = num * (1.0 - l.params.no_grip_speed) + l.params.no_grip_speed;
        if l.no_grip_counter() < 1 {
            num = num.lerp(1.0 + l.body_desperation(), l.body_force() - 1.0);
        }
        if num == 0.0 && l.body_wiggle_fac() > 0.0 {
            let touching = l
                .body
                .chunks
                .iter()
                .any(|c| c.contact().x != 0 || c.contact().y != 0);
            if touching {
                let s = f64::from(l.body_wiggle * 2.0 * std::f32::consts::PI).sin() as f32;
                num = l.body_wiggle_fac() * (0.5 + 0.5 * s) * 0.2;
            }
        }
        if num < 0.1 || l.body_force() > 1.2 {
            l.set_body_wiggle_counter(l.body_wiggle_counter + 1);
        } else {
            l.set_body_wiggle_counter(l.body_wiggle_counter - 1);
        }
        let mut speed = l.params.base_speed * num * run_speed.lerp(1.0, l.body_force() - 1.0);
        speed *= self.rng.value().lerp(1.0, (health - 0.5) * 2.0);
        let l = self.world.lizard_ref(id);
        let terrain = l
            .body
            .chunks
            .iter()
            .map(|c| {
                let t = c.pos.tile();
                l.params.terrain_speed(r.map.tile(t.0, t.1).acc)
            })
            .fold(
                None,
                |acc: Option<crate::creature::lizard::SpeedMultiplier>, s| {
                    Some(acc.map_or(s, |a| a + s))
                },
            );
        let Some(m) = terrain.map(|t| t / 3.0) else {
            return 0.0;
        };
        speed *= m.speed;
        let Some(fc) = l.following_connection else {
            return speed * m.down;
        };
        let (dx, dy) = (
            fc.dest_tile().0 - fc.start_tile().0,
            fc.dest_tile().1 - fc.start_tile().1,
        );
        if dx != 0 {
            speed * m.horizontal
        } else if dy > 0 {
            speed * m.up
        } else {
            speed * m.down
        }
    }

    /// `FollowConnection`: a sharp turn into a narrow space ahead slows the body; a
    /// walking move (Standard, slopes, open diagonals) pulls the head toward the
    /// farthest upcoming tile in line of sight (with a pole's sideways offset or the
    /// floor's leverage, chunks against a wall eased off it) and the middle after it, and
    /// running straightens a doubled-back body; a drop commits once near and pulls the
    /// body over the edge, catching a pole or a floor tile on the way down.
    fn lizard_follow_connection(&mut self, r: &ActRoom, id: CreatureId, run_speed: f32) {
        let frame = self.lizard_frame_speed(r, id, run_speed);
        let mut run_speed = run_speed;
        let l = self.world.lizard_at(id);
        let Some(fc) = l.following_connection else {
            return;
        };
        if l.time_spent_trying_this_move < 20
            && l.narrow_upcoming
            && frame > 0.2
            && l.upcoming_connections.len() > 1
            && r.map
                .tile(
                    l.upcoming_connections[1].dest_tile().0,
                    l.upcoming_connections[1].dest_tile().1,
                )
                .narrow_space
        {
            let a = Self::clamp_at_one(fc.dest_tile(), fc.start_tile());
            let u = l.upcoming_connections[1];
            let b = Self::clamp_at_one(u.dest_tile(), u.start_tile());
            if a.0 != b.0 && a.1 != b.1 {
                l.body.chunks[0].vel *= 0.75;
                l.body.chunks[0].vel += Vec2::new(b.0 as f32, b.1 as f32) * 2.0;
                run_speed = run_speed.min(0.2);
                l.body.chunks[1].vel *= 0.75;
                l.body.chunks[2].vel *= 0.5;
            }
        }
        let _ = run_speed;
        match fc.kind {
            MovementType::Standard
            | MovementType::Slope
            | MovementType::CeilingSlope
            | MovementType::OpenDiagonal => self.lizard_walk(r, id, fc, frame),
            MovementType::DropToFloor | MovementType::DropToClimb => {
                self.lizard_drop(r, id, fc, frame);
            }
            _ => {}
        }
    }

    /// `IntVector2.ClampAtOne(a - b)`.
    fn clamp_at_one(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
        ((a.0 - b.0).clamp(-1, 1), (a.1 - b.1).clamp(-1, 1))
    }

    /// The walking branch of `FollowConnection`.
    fn lizard_walk(&mut self, r: &ActRoom, id: CreatureId, fc: MovementConnection, frame: f32) {
        let pos = self.world.creature(id).pos;
        let pf = self.world.path_finder_of(id);
        let l = self.world.lizard_ref(id);
        let room = &r.room;
        let mut offset = Vec2::ZERO;
        let head = l.head().pos;
        let head_mid = Vec2::middle_of_tile(head.tile());
        if r.map.tile_at(fc.dest).acc == Accessibility::Climb {
            let mut d = match l.upcoming_connections.len() {
                0 => Self::clamp_at_one(fc.dest_tile(), fc.start_tile()),
                n => Self::clamp_at_one(
                    l.upcoming_connections[2.min(n - 1)].dest_tile(),
                    fc.start_tile(),
                ),
            };
            if fc.dest.tile.1 == fc.start.tile.1 {
                offset.y = d.1 as f32 * 8.0;
            } else if fc.dest.tile.0 == fc.start.tile.0 {
                if r.map.tile_at(fc.start).acc != Accessibility::Climb {
                    d.0 = if l.body.chunks[2].pos.x < head_mid.x {
                        -1
                    } else {
                        1
                    };
                }
                offset.x = d.0 as f32 * 8.0;
            }
            if l.is_tile_solid(room, 0, d.0, 0) {
                offset.x = 0.0;
            }
            if l.is_tile_solid(room, 0, 0, d.1) {
                offset.y = 0.0;
            }
        } else {
            let side =
                i32::from(fc.dest.tile.0 > pos.tile.0) - i32::from(fc.dest.tile.0 < pos.tile.0);
            if l.no_grip_counter() == 0
                && (fc.dest.tile.1 == fc.start.tile.1
                    || r.map.tile_at(fc.dest).acc == Accessibility::CurvedFloor)
                && !l.is_tile_solid(room, 0, 0, 1)
                && !l.is_tile_solid(room, 0, side, 1)
            {
                offset.y = l.params.floor_leverage;
            }
        }
        let horizontal = fc.start.tile.0 != fc.dest.tile.0 && fc.start.tile.1 == fc.dest.tile.1;
        let vertical = fc.start.tile.1 != fc.dest.tile.1 && fc.start.tile.0 == fc.dest.tile.0;
        let solid_below = l.is_tile_solid(room, 0, 0, -1);
        let solid_above = l.is_tile_solid(room, 0, 0, 1);
        let solid_left = l.is_tile_solid(room, 0, -1, 0);
        let solid_right = l.is_tile_solid(room, 0, 1, 0);
        let mut step = (
            fc.dest_tile().0 - fc.start_tile().0,
            fc.dest_tile().1 - fc.start_tile().1,
        );
        if r.map.tile_at(fc.start).acc == Accessibility::CurvedFloor {
            step.1 = 0;
        }
        let solids: Vec<(bool, bool)> = (0..3)
            .map(|i| {
                let c = l.body.chunks[i].contact();
                (
                    c.y != 0 && !l.is_tile_solid(room, i, 0, i32::from(c.y)),
                    c.x != 0 && !l.is_tile_solid(room, i, i32::from(c.x), 0),
                )
            })
            .collect();
        let mut target = Vec2::middle_of_tile(fc.dest_tile());
        let upcoming = l.upcoming_connections.clone();
        if let Some(last) = upcoming.last()
            && r.map.tile_at(last.dest).acc < Accessibility::Climb
        {
            let head_tile = head.tile();
            for u in upcoming.iter().rev() {
                let d = u.dest_tile();
                let (dx, dy) = ((head_tile.0 - d.0) as f32, (head_tile.1 - d.1) as f32);
                let close = (dx * dx + dy * dy).sqrt() < 2.0
                    && room.visual_contact(head, Vec2::middle_of_tile(d));
                let traced = pf
                    .and_then(|p| self.world.path_finders.get(p))
                    .is_some_and(|f| f.ray_trace_in_accessible_tiles(head_tile, d));
                if close || traced {
                    target = Vec2::middle_of_tile(d);
                    break;
                }
            }
        }
        let l = self.world.lizard_at(id);
        if horizontal {
            let mid = head_mid.y;
            let y = l.body.chunks[0].pos.y;
            if (solid_below && y < mid) || (solid_above && y > mid) {
                l.body.chunks[0].vel.y -= (y - mid) * 0.5;
            }
        } else if vertical {
            let mid = head_mid.x;
            let x = l.body.chunks[0].pos.x;
            if (solid_left && x < mid) || (solid_right && x > mid) {
                l.body.chunks[0].vel.x -= (x - mid) * 0.5;
            }
        }
        if step.1 != 0 {
            for (i, &(off, _)) in solids.iter().enumerate() {
                if off {
                    let c = &mut l.body.chunks[i];
                    c.vel.x -= (c.pos.x - Vec2::middle_of_tile(c.pos.tile()).x) * 0.25;
                    offset.x *= 0.0;
                }
            }
        } else if step.0 != 0 {
            for (i, &(_, off)) in solids.iter().enumerate() {
                if off {
                    let c = &mut l.body.chunks[i];
                    c.vel.y -= (c.pos.y - Vec2::middle_of_tile(c.pos.tile()).y) * 0.25;
                    offset.y *= 0.0;
                }
            }
        }
        target += offset;
        let force = l.body_force();
        let rod = l.body.connections[0].distance;
        let p = target + target.direction_to(l.body.chunks[0].pos) * rod;
        let h = l.body.chunks[0].pos;
        l.body.chunks[0].vel += h.direction_to(target) * (frame * force);
        let m = l.body.chunks[1].pos;
        let lean = m
            .direction_to(target)
            .dot(m.direction_to(p))
            .lerp_map(-1.0, 1.0, 0.5, 1.0);
        l.body.chunks[1].vel += m.direction_to(p) * (frame * force * lean);
        if l.animation == LizardAnimation::Standard && l.ai.run_speed > 0.1 {
            let h = l.body.chunks[0].pos;
            let back = h
                .direction_to(target)
                .dot(m.direction_to(h))
                .inverse_lerp(0.0, -1.0);
            if back > 0.5 {
                l.straighten_out_needed = l.straighten_out_needed.lerp_and_tick(1.0, 0.1, 0.1);
            }
            let tries = l.time_spent_trying_this_move as f32;
            let mut s = back.lerp(1.0, tries.inverse_lerp(20.0, 40.0));
            let ahead: f32 = if l.narrow_upcoming || l.shortcut_upcoming {
                1.0
            } else {
                0.2
            };
            let climb = if l.climb_upcoming { 0.5 } else { 0.2 };
            s *= ahead
                .max(climb)
                .max(tries.inverse_lerp(5.0, 20.0))
                .max(l.straighten_out_needed);
            let push = s * 2.0 * force + l.body_desperation() * 2.0;
            let dir = m.direction_to(target);
            l.body.chunks[0].vel += dir * (push * frame);
            let v = dir * (push * 0.5 * tries.lerp_map(20.0, 60.0, 1.0, 0.5) * frame);
            l.body.chunks[1].vel -= v;
            l.body.chunks[2].vel -= v;
        }
    }

    /// The drop branch of `FollowConnection`.
    fn lizard_drop(&mut self, r: &ActRoom, id: CreatureId, fc: MovementConnection, frame: f32) {
        let pos = self.world.creature(id).pos;
        let room_index = r.index;
        let l = self.world.lizard_at(id);
        let mut commit =
            !((pos.tile.0 - fc.start.tile.0).abs() > 4 && (pos.tile.0 - fc.dest.tile.0).abs() > 4);
        if l.body.chunks.iter().any(|c| c.contact().y < 0) {
            commit = false;
        }
        if commit {
            l.commited_to_drop_connection = Some(fc);
        }
        l.body.chunks[0].vel.y -= 0.01;
        l.body.chunks[2].vel.y += 0.01;
        let dest_mid = Vec2::middle_of_tile(fc.dest_tile());
        let start_mid = Vec2::middle_of_tile(fc.start_tile());
        let head = &mut l.body.chunks[0];
        if (head.pos.x - dest_mid.x).abs() < 5.0 {
            if head.vel.x.abs() < 5.0 {
                head.vel.x *= 0.8;
            }
        } else if head.pos.x < dest_mid.x {
            head.vel.x += 0.5;
        } else {
            head.vel.x -= 0.5;
        }
        let force = l.body_force();
        let mut landed = false;
        let mut pulled_over = false;
        let mut above = false;
        for (i, c) in l.body.chunks.iter_mut().enumerate() {
            if c.contact().y < 0 {
                landed = true;
            }
            let t = c.pos.tile();
            let f = if i == 0 { force } else { 1.0 };
            if t.1 >= fc.start_tile().1 {
                above = true;
                if t.0 == fc.start_tile().0 {
                    c.vel += c.pos.direction_to(dest_mid) * (frame * f);
                    pulled_over = true;
                } else {
                    c.vel += c.pos.direction_to(start_mid) * (frame * f);
                }
            }
            if t.0 == fc.start_tile().0 {
                c.vel.y -= 1.0;
            }
        }
        if pulled_over {
            return;
        }
        if let Some(last) = l.upcoming_connections.last() {
            let d = (fc.dest.tile.0 - last.dest.tile.0) as f32;
            l.body.chunks[0].vel.x -= 0.3 * d;
            l.body.chunks[2].vel.x += 0.3 * d;
        }
        let (head_pos, head_last) = (l.body.chunks[0].pos, l.body.chunks[0].last_pos);
        let mut grabbed_pole = false;
        if head_pos.y <= head_last.y {
            let from = head_last.tile().1 + 1;
            let to = head_pos.tile().1 - 1;
            let x = head_pos.tile().0;
            if fc.kind == MovementType::DropToClimb {
                for y in (to..=from).rev() {
                    if y <= fc.dest.tile.1 && r.map.tile(x, y).acc == Accessibility::Climb {
                        grabbed_pole = true;
                        l.grip_point = Some(Vec2::middle_of_tile((x, y)));
                        l.body.chunks[0].vel.y = 0.0;
                        break;
                    }
                }
            } else if fc.dest.tile.0 != fc.start.tile.0 && (x - fc.dest.tile.0).abs() < 2 {
                for y in (to..=from).rev() {
                    if y <= fc.dest.tile.1
                        && r.map.tile(fc.dest.tile.0, y).acc == Accessibility::Floor
                    {
                        l.grip_point = Some(Vec2::middle_of_tile((fc.dest.tile.0, y)));
                        l.body.chunks[0].vel.y = 0.0;
                        break;
                    }
                }
            }
        }
        if landed && !above && l.grip_point.is_none() {
            l.following_connection = None;
        }
        if grabbed_pole {
            self.play_on_object(room_index, SoundId::LizardGrabPole, head_pos);
        }
    }

    /// `GripPointBehavior`: a fast fall with footing lets go; held up by the legs it
    /// takes the path from the head (a sideways step off a pole after dropping onto it
    /// becomes a reach) and lets go; else it hangs, the body swinging (draws) toward
    /// the path ahead. The head stays at the grip point.
    fn lizard_grip_point_behavior(&mut self, r: &ActRoom, id: CreatureId) {
        let l = self.world.lizard_ref(id);
        if l.head().vel.length() > 10.0 && l.in_allowed_terrain_counter > 5 {
            let l = self.world.lizard_at(id);
            l.grip_point = None;
            l.following_connection = None;
        } else if !l.apply_gravity {
            let head = l.head().pos;
            let next = self.lizard_follow_path(id, r.coordinate(head), Some(0), true);
            let l = self.world.lizard_at(id);
            l.movement_animation = None;
            if let (Some(fc), Some(mut c)) = (l.following_connection, next)
                && fc.kind == MovementType::DropToClimb
                && c.dest_tile().1 - c.start_tile().1 == 0
                && (c.dest_tile().0 - c.start_tile().0).abs() == 1
            {
                c.kind = MovementType::DropToClimb;
                let chunks = [
                    l.body.chunks[0].pos,
                    l.body.chunks[1].pos,
                    l.body.chunks[2].pos,
                ];
                l.movement_animation = Some(MovementAnimation::new(chunks, c, 1, 0.99, 0.75));
            }
            l.grip_point = None;
            l.following_connection = None;
        } else {
            if l.body.chunks[1].pos.y < l.body.chunks[0].pos.y {
                let v = self.rng.value();
                self.world.lizard_at(id).body.chunks[1].vel.y += v * 2.0;
            }
            let l = self.world.lizard_ref(id);
            if let Some(last) = l.upcoming_connections.last() {
                let d = Self::clamp_at_one(l.head().pos.tile(), last.dest_tile()).0 as f32;
                let v = self.rng.value();
                let l = self.world.lizard_at(id);
                l.body.chunks[1].vel.x += 3.0 * d;
                l.body.chunks[2].vel.x += v * 3.0 * d;
            } else {
                let v = self.rng.value();
                self.world.lizard_at(id).body.chunks[1].vel.x += -2.0 + v * 4.0;
            }
        }
        let l = self.world.lizard_at(id);
        l.body.chunks[0].vel *= 0.0;
        if let Some(g) = l.grip_point {
            let p = l.body.chunks[0].pos;
            l.body.chunks[0].vel -= p - g;
        }
    }

    /// `SwimBehavior`: swimming builds up and desperation settles at 30; a head at the
    /// surface bobs to it. A swimmer heads for a destination in water it sees, else
    /// along the path; any other lizard paddles toward the path's next tile from the
    /// surface. A move onto dry land ends the swim with footing and a reach up out of
    /// the water when the tile above is accessible.
    fn lizard_swim_behavior(&mut self, r: &ActRoom, id: CreatureId) {
        let kind = self.world.creature(id).kind();
        let swimmer = kind == CreatureType::Salamander;
        let water_level = r.room.water_level().unwrap_or(-1);
        let l = self.world.lizard_at(id);
        l.swim = (l.swim + 1.0 / 15.0).clamp(0.0, 1.0);
        l.desperation_smoother = l.desperation_smoother.lerp(30.0, 0.1);
        let head_sub = r
            .water
            .as_ref()
            .map_or(0.0, |w| w.submersion(&l.body.chunks[0]));
        if head_sub > 0.0
            && head_sub < 1.0
            && l.following_connection
                .is_none_or(|fc| fc.dest_tile().1 == water_level)
        {
            let c = &mut l.body.chunks[0];
            c.vel.y *= 0.8;
            c.vel.y += (r.float_water_level(c.pos.x) - c.pos.y).clamp(-10.0, 10.0) * 0.1;
        }
        let mut coord = r.coordinate(l.head().pos);
        if swimmer {
            self.lizard_swimmer_stroke(r, id, coord);
        } else {
            coord.tile.1 = coord.tile.1.clamp(water_level, r.room.height());
            if l.following_connection.is_none() {
                let fc = self.lizard_follow_path(id, coord, None, true);
                let l = self.world.lizard_at(id);
                l.following_connection = fc;
                l.time_spent_trying_this_move = 0;
            }
            let l = self.world.lizard_at(id);
            if l.swim > 0.5 {
                l.in_allowed_terrain_counter = 0;
            }
            if let Some(fc) = l.following_connection {
                let force = l.body_force();
                let swim_speed = l.params.swim_speed;
                let head = &mut l.body.chunks[0];
                let to = Vec2::middle_of_tile(fc.dest_tile()).x - head.pos.x;
                head.vel.x += to.clamp(-1.0, 1.0) * swim_speed * force;
                if r.any_water(fc.dest_tile()) && head.pos.tile() == fc.dest_tile() {
                    l.following_connection = None;
                }
            }
        }
        let t = self.world.creature(id).template;
        let l = self.world.lizard_at(id);
        if let Some(fc) = l.following_connection
            && ((coord.tile.0 != fc.start_tile().0 && coord.tile.0 != fc.dest_tile().0)
                || coord.tile == fc.dest_tile())
        {
            l.following_connection = None;
        }
        if let Some(fc) = l.following_connection
            && fc.dest.tile_defined()
            && !r.any_water(fc.dest_tile())
        {
            l.in_allowed_terrain_counter = l.params.regain_footing_counter + 2;
            l.swim = 0.0;
            let up = (fc.dest_tile().0, fc.dest_tile().1 + 1);
            if r.accessible(up, &t) {
                let reach = MovementConnection::new(
                    MovementType::ReachUp,
                    fc.start,
                    WorldCoordinate::new(fc.dest.room, up.0, up.1, -1),
                    2,
                );
                let chunks = [
                    l.body.chunks[0].pos,
                    l.body.chunks[1].pos,
                    l.body.chunks[2].pos,
                ];
                l.movement_animation = Some(MovementAnimation::new(chunks, reach, 1, 0.99, 0.75));
            }
        }
    }

    /// A swimmer's stroke in `SwimBehavior` (its lurking is lizard step 7): toward a
    /// destination in water it sees, else toward the path's next tile, at
    /// `1.3 * Lerp(0.5, 1, runSpeed)` (less at the surface).
    fn lizard_swimmer_stroke(&mut self, r: &ActRoom, id: CreatureId, coord: WorldCoordinate) {
        let pf = self.world.path_finder_of(id);
        let dest = pf
            .and_then(|p| self.world.path_finders.get(p))
            .map(crate::creature::PathFinder::get_destination);
        let l = self.world.lizard_ref(id);
        let head = l.head().pos;
        let mut dir = Vec2::ZERO;
        if let Some(d) = dest
            && !d.node_defined()
            && d.room == r.index
            && r.any_water(d.tile)
            && r.room.visual_contact(head, Vec2::middle_of_tile(d.tile))
        {
            dir = head.direction_to(Vec2::middle_of_tile(d.tile));
        } else {
            if l.following_connection.is_none() {
                let fc = self.lizard_follow_path(id, coord, None, true);
                self.world.lizard_at(id).following_connection = fc;
            }
            if let Some(fc) = self.world.lizard_ref(id).following_connection {
                dir = head.direction_to(Vec2::middle_of_tile(fc.dest_tile()));
            }
        }
        let l = self.world.lizard_at(id);
        let surface = if r.map.tile(head.tile().0, head.tile().1).water_surface() {
            0.7
        } else {
            1.0
        };
        let s = 0.5f32.lerp(1.0, l.ai.run_speed) * surface;
        l.body.chunks[0].vel += dir * (1.3 * s);
        l.body.chunks[1].vel -= dir * (0.3 * s);
    }

    /// `Creature.Update`'s `enteringShortCut` branch for a lizard: every connected
    /// object's chunks are pulled into the entrance (4.5 px a tick, slowed to 6%),
    /// `SuckedIntoShortCut` once none of the lizard's own chunks still moves; a stun
    /// cancels. Returns false when it is not entering.
    pub(super) fn lizard_pull_into_shortcut(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        let Some(entrance) = self.world.lizard_ref(id).entering_shortcut else {
            self.world.lizard_at(id).short_cut_rad_add = 0.9;
            return false;
        };
        let Some(room) = self.world.realized(index).map(|r| Arc::clone(&r.room)) else {
            return true;
        };
        self.world.lizard_at(id).short_cut_rad_add += 0.2;
        self.lizard_graphics_update(index, id, observer);
        let radius = 10.0f32.max(self.world.lizard_ref(id).short_cut_rad_add);
        let hole = room.shortcut_hole_direction(entrance.0, entrance.1);
        let target =
            Vec2::middle_of_tile(entrance) + Vec2::new(hole.0 as f32, hole.1 as f32) * -5.0;
        let mut moving = 0;
        for other in self.world.connected_objects(Entity::Creature(id)) {
            let Some(chunks) = self.world.body_chunks_mut(other) else {
                continue;
            };
            for c in chunks {
                c.vel *= 0.06;
                if c.pos.dist_less(target, radius) {
                    c.pos = target;
                    c.last_pos = target;
                } else {
                    c.last_pos = c.pos;
                    c.pos += c.pos.direction_to(target) * 4.5;
                    if other == Entity::Creature(id) {
                        moving += 1;
                    }
                }
            }
        }
        if moving == 0 {
            self.lizard_sucked_into_shortcut(index, id, entrance, &room);
        }
        self.lizard_graphics_sucked(id, target, observer);
        if let Some(l) = self.world.creature_mut(id).lizard_mut()
            && l.stunned()
        {
            log::debug!("{id:?} cancel shortcut enter");
            l.entering_shortcut = None;
        }
        true
    }

    /// `SuckedIntoShortCut` and `ShortcutHandler.SuckInCreature`: the enter sound, a
    /// vessel at the entrance (or, through an NPC transport whose destination entrance
    /// exists, at that entrance, waiting the distance between them), and every connected
    /// object out of the room.
    fn lizard_sucked_into_shortcut(
        &mut self,
        index: i32,
        id: CreatureId,
        entrance: (i32, i32),
        room: &Room,
    ) {
        if room.tile(entrance.0, entrance.1).terrain != Terrain::ShortcutEntrance {
            log::debug!("attempted suck into a non-existent shortcut entrance");
            return;
        }
        let data = room.shortcut_map().data_at(entrance).cloned();
        if let Some(d) = &data
            && d.kind == ShortcutType::Normal
        {
            self.world.creature_mut(id).pos.tile = d.dest;
        }
        let template = self.world.creature(id).template;
        let head = self.world.lizard_ref(id).head().pos;
        let enter = template.npc_shortcut_sound(0);
        self.play_on_object(index, enter, head);
        let start = data.as_ref().map_or(entrance, |d| d.start);
        let dest_tile = data.as_ref().map_or(entrance, |d| d.dest);
        let segments = template.shortcut_segments();
        let npc = self
            .world
            .lizard_ref(id)
            .npc_transportation_destination
            .filter(|n| room.shortcut_map().index_of(n.tile).is_some());
        let vessel = match (data.as_ref().map(|d| d.kind), npc) {
            (Some(ShortcutType::NpcTransportation), Some(n)) => {
                let (dx, dy) = (
                    (dest_tile.0 - n.tile.0) as f32,
                    (dest_tile.1 - n.tile.1) as f32,
                );
                let wait = (dx * dx + dy * dy).sqrt() as i32;
                ShortcutVessel::with_segments(n.tile, segments, wait)
            }
            _ => ShortcutVessel::with_segments(start, segments, 0),
        };
        self.shortcuts
            .transport
            .push(Transport::Creature(CreatureVessel {
                creature: id,
                room: index,
                vessel,
                entrance_node: -1,
            }));
        self.world.lizard_at(id).entering_shortcut = None;
        for other in self.world.connected_objects(Entity::Creature(id)) {
            match other {
                Entity::Creature(o) => match &mut self.world.creature_mut(o).realized {
                    Some(crate::creature::RealizedCreature::Lizard(l)) => {
                        l.in_shortcut = true;
                        l.room = None;
                    }
                    Some(crate::creature::RealizedCreature::Fly(f)) => {
                        f.in_shortcut = true;
                        f.room = None;
                    }
                    _ => {}
                },
                Entity::Object(o) => self.world.object_leaves_room(o),
            }
        }
    }
}

#[cfg(test)]
#[path = "game_lizard_act_tests.rs"]
mod tests;
