//! `FliesRoomAI` (183857-183981): a realized room's flies and the ones resting in its
//! hives, which emerge now and then. Also the realization and placing of a fly
//! (`AbstractCreature.Realize`, `Creature.PlaceInRoom`, `Room.AddObject`, `Fly.NewRoom`).
//!
//! Mirrors: FliesRoomAI.cs, AbstractCreature.cs (Realize), Creature.cs (PlaceInRoom), Room.cs (AddObject), Fly.cs (NewRoom)

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use super::{Fly, FlyAi};
use crate::creature::{CreatureId, CreatureTemplate, RealizedCreature};
use crate::item::Entity;
use crate::world::World;
use crate::{PlayerObserver, SoundId, Sounds, Vec2};

/// `FliesRoomAI`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FliesRoomAi {
    pub flies: Vec<CreatureId>,
    pub in_hive: Vec<CreatureId>,
}

/// What placing a fly in a room reads and changes besides the world.
pub(crate) struct FlyRoomTick<'a> {
    pub rng: &'a mut UnityRandom,
    pub observer: &'a mut dyn PlayerObserver,
    /// `cameras[0].room`: `Room.BeingViewed`.
    pub camera_room: i32,
}

impl World {
    /// `AbstractCreature.Realize` of a fly: the `Fly` constructor's draws (its
    /// `FlyAI` and `FlockBehavior`), then the realized objects it is stuck to.
    pub(crate) fn realize_fly(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        if c.realized.is_some() || c.kind() != CreatureType::Fly {
            return;
        }
        let rain = self.rain_approaching();
        let dead = !c.state.alive;
        let (mut fly, flock) = Fly::new(id, self.flocks.len(), rain, rng);
        // `Creature`'s constructor: `dead = State.dead`.
        fly.dead = dead;
        self.flocks.push(flock);
        let c = self.creature_mut(id);
        c.realized = Some(RealizedCreature::Fly(Box::new(fly)));
        c.kill_tag = None;
        c.kill_tag_counter = 0;
        c.grasps = vec![None];
        c.grabbed_by.clear();
        for s in self.creature(id).sticks.clone() {
            for other in [Some(s.a), s.b.creature()].into_iter().flatten() {
                if other != id {
                    self.realize_fly(other, rng);
                }
            }
        }
        for s in self.spear_sticks_of(Entity::Creature(id)) {
            self.realize_object(s.spear, rng);
        }
    }

    /// `Room.AddObject` of a fly: the end of the room's update list, and in a viewed room
    /// a graphics module if it has none (`FlyGraphics`' constructor draws).
    pub(crate) fn fly_add_object(&mut self, id: CreatureId, room: i32, cx: &mut FlyRoomTick<'_>) {
        let seq = self.next_update_seq();
        let fly = self.fly_at(id);
        fly.room = Some(room);
        fly.update_order = seq;
        fly.slated = false;
        if room == cx.camera_room && !fly.graphics {
            fly.graphics = true;
            let view = self.fly_view(id, None);
            cx.observer.fly_added(&view, cx.rng);
        }
    }

    /// `Fly.NewRoom`: `Creature.NewRoom` (the graphics reset, `newToRoomInvinsibility`),
    /// `ReportToFliesRoomAI`, `FlyAI.NewRoom`.
    pub(crate) fn fly_new_room(&mut self, id: CreatureId, room: i32, cx: &mut FlyRoomTick<'_>) {
        let fly = self.fly_at(id);
        fly.new_to_room_invinsibility = 40;
        if fly.graphics {
            let view = self.fly_view(id, None);
            cx.observer.fly_reset(&view);
        }
        let Some(realized) = self.realized_mut(room) else {
            return;
        };
        let height = realized.room.pixel_height();
        let ai = realized
            .flies_room_ai
            .get_or_insert_with(FliesRoomAi::default);
        if !ai.flies.contains(&id) {
            ai.flies.push(id);
        }
        self.fly_ai_new_room(id, height);
    }

    /// `Creature.PlaceInRoom` of a fly at its abstract tile: added to the room, its
    /// chunk within a pixel of the tile's middle (a draw), the flies it holds placed on
    /// it, then `NewRoom`.
    pub(crate) fn place_fly(&mut self, id: CreatureId, room: i32, cx: &mut FlyRoomTick<'_>) {
        self.fly_add_object(id, room, cx);
        let tile = self.creature(id).pos.tile;
        let middle = Vec2::middle_of_tile(tile);
        let fly = self.fly_at(id);
        for c in &mut fly.body.chunks {
            c.pos = middle + Vec2::from_degrees(cx.rng.value() * 360.0);
            c.last_pos = c.pos;
            c.last_last_pos = c.pos;
            c.vel = Vec2::ZERO;
        }
        let main = fly.pos();
        let held: Vec<CreatureId> = self
            .creature(id)
            .grasps
            .iter()
            .flatten()
            .filter_map(|g| g.grabbed_creature())
            .collect();
        for other in held {
            if self.creature(other).fly().is_none() {
                continue;
            }
            self.fly_add_object(other, room, cx);
            for c in &mut self.fly_at(other).body.chunks {
                c.reset_at(main);
            }
        }
        self.fly_new_room(id, room, cx);
    }

    /// `AbstractCreature.RealizeInRoom` of abstract fly `id` in room `room`: a node drawn
    /// if it has none, `Realize`, its tile from the node if it has none, then every
    /// connected object at its position and placed.
    pub(crate) fn realize_fly_in_room(
        &mut self,
        id: CreatureId,
        room: i32,
        cx: &mut FlyRoomTick<'_>,
    ) {
        let mut pos = self.creature(id).pos;
        if !pos.node_defined() {
            let nodes = self.fly_nodes(room);
            let n = i32::try_from(nodes.len()).unwrap_or(0);
            let specific = cx.rng.range_i32(0, n);
            pos.node = usize::try_from(specific)
                .ok()
                .and_then(|s| nodes.get(s))
                .map_or(-1, |&(c, _)| c);
        }
        self.creature_mut(id).pos = pos;
        self.realize_fly(id, cx.rng);
        if !pos.tile_defined() {
            pos.tile = self.local_coordinate_of_node(room, pos.node, cx.rng);
            self.creature_mut(id).pos = pos;
        }
        for o in self.connected_objects(Entity::Creature(id)) {
            match o {
                Entity::Creature(o) => {
                    self.creature_mut(o).pos = pos;
                    if self.creature(o).fly().is_some() {
                        self.place_fly(o, room, cx);
                    }
                }
                Entity::Object(o) => {
                    self.object_mut(o).pos = pos;
                    self.place_object(o, cx.rng);
                }
            }
        }
    }

    /// `Fly.SpitOutOfShortCut` after the base's stuck objects: `shortcutDelay` 40, out of
    /// the entrance middle `middle` 5 px into the room against hole direction `hole`,
    /// moving `hole * 15`, the graphics reset.
    pub(crate) fn fly_spat_out(
        &mut self,
        id: CreatureId,
        middle: Vec2,
        hole: Vec2,
        cx: &mut FlyRoomTick<'_>,
    ) {
        let fly = self.fly_at(id);
        fly.in_shortcut = false;
        fly.shortcut_delay = 40;
        let c = fly.chunk_mut();
        c.pos = middle - hole * 5.0;
        c.last_pos = c.pos;
        c.vel = hole * 15.0;
        if fly.graphics {
            let view = self.fly_view(id, None);
            cx.observer.fly_reset(&view);
        }
    }

    /// `FliesRoomAI.Update` of room `room`: each fly resting in a hive may emerge (a
    /// draw each), then the flies that left the room are forgotten.
    pub(crate) fn update_flies_room_ai(
        &mut self,
        room: i32,
        cx: &mut FlyRoomTick<'_>,
        sounds: &mut Sounds,
    ) {
        let Some(in_hive) = self
            .realized(room)
            .and_then(|r| r.flies_room_ai.as_ref())
            .map(|a| a.in_hive.clone())
        else {
            return;
        };
        for &id in in_hive.iter().rev() {
            if self
                .creature(id)
                .fly()
                .is_some_and(|f| f.room == Some(room))
            {
                self.fly_at(id).room = None;
            }
            if cx.rng.value() < 0.025 {
                self.fly_emerge_from_hive(id, room, cx, sounds);
            }
        }
        let gone: Vec<CreatureId> = self
            .realized(room)
            .and_then(|r| r.flies_room_ai.as_ref())
            .map(|a| a.flies.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&f| self.creature(f).fly().is_none_or(|x| x.room != Some(room)))
            .collect();
        if let Some(ai) = self
            .realized_mut(room)
            .and_then(|r| r.flies_room_ai.as_mut())
        {
            ai.flies.retain(|f| !gone.contains(f));
        }
    }

    /// `FlyEmergeFromHive`: not while the rain is near (or over) in a room the rain
    /// endangers (no void weaver in the base game); the hive scoring
    /// highest (a draw each, less with feared creatures near it) lets the fly out on
    /// one of its tiles.
    fn fly_emerge_from_hive(
        &mut self,
        id: CreatureId,
        room: i32,
        cx: &mut FlyRoomTick<'_>,
        sounds: &mut Sounds,
    ) {
        let hazard = self
            .realized(room)
            .is_some_and(|r| !r.settings.not_a_cycle_hazard(self.mods));
        let game_over = self.rain_cycle.timer >= self.rain_cycle.cycle_length;
        if hazard && (self.rain_approaching() < 0.3 || game_over) {
            return;
        }
        let Some(realized) = self.realized(room) else {
            return;
        };
        let hives = realized.room.hives().to_vec();
        let exits = i32::try_from(realized.room.shortcut_map().nodes().len()).unwrap_or(0);
        let map = realized.ai_map.clone();
        let feared: Vec<(i32, i32)> = self
            .room_creatures(room)
            .iter()
            .map(|&c| self.creature(c))
            .filter(|c| FlyAi::afraid_of(c.kind()))
            .map(|c| c.pos.tile)
            .collect();
        let fly = CreatureTemplate::of(CreatureType::Fly, rw_assets::ActiveMods::NONE);
        let mut best = 0.0f32;
        let mut pick = None;
        for (i, _) in hives.iter().enumerate() {
            let mut score = cx.rng.value() * 50.0;
            let node = exits + i32::try_from(i).unwrap_or(0);
            for &tile in &feared {
                let d = map
                    .as_ref()
                    .map_or(-1, |m| m.exit_distance_for_creature(tile, node, &fly));
                score += d as f32;
                if (1..20).contains(&d) {
                    score -= 1000.0;
                }
            }
            if score > best {
                best = score;
                pick = Some(i);
            }
        }
        let Some(hive) = pick.and_then(|i| hives.get(i)) else {
            return;
        };
        if hive.is_empty() {
            return;
        }
        let k = cx.rng.range_i32(0, i32::try_from(hive.len()).unwrap_or(0));
        let tile = hive[usize::try_from(k).unwrap_or(0)];
        self.creature_mut(id).pos.tile = tile;
        let at = Vec2::middle_of_tile(tile);
        sounds.play_at(SoundId::BatEmergeFromGrass, at, 1.0, 1.0, cx.rng);
        self.place_fly(id, room, cx);
        if let Some(ai) = self
            .realized_mut(room)
            .and_then(|r| r.flies_room_ai.as_mut())
        {
            ai.in_hive.retain(|&f| f != id);
        }
    }

    /// `FliesRoomAI.GetRandomFly` of room `room` (a draw unless it has none).
    pub(crate) fn random_room_fly(&self, room: i32, rng: &mut UnityRandom) -> Option<CreatureId> {
        let flies = &self.realized(room)?.flies_room_ai.as_ref()?.flies;
        if flies.is_empty() {
            return None;
        }
        let i = rng.range_i32(0, i32::try_from(flies.len()).unwrap_or(0));
        flies.get(usize::try_from(i).unwrap_or(0)).copied()
    }
}
