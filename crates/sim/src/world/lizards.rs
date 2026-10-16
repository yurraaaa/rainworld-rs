//! A lizard's life between its abstract creature and its body: `AbstractCreature.Realize`
//! with the stand-in `LizardAI`, `RealizeInRoom` with `AbstractCreatureAI.
//! PlaceInRealizedRoom` (`RandomizeSpawnPositionInRoom`, the `QuickPathFinder`),
//! `Creature.PlaceInRoom`, `Room.AddObject`, `NewRoom` and `AbstractCreature.Abstractize`.
//! Decompile: `AbstractCreature.cs` 426-602 and 715-860, `AbstractCreatureAI.cs` 249-316
//! and 770-808, `Creature.cs` 317-367.

use rw_random::UnityRandom;

use super::{QuickPathFinder, RealizedRoom, World};
use crate::Vec2;
use crate::creature::{CreatureId, Lizard, RealizedCreature, WorldCoordinate};
use crate::item::Entity;

impl World {
    /// Realized lizard `id`.
    ///
    /// # Panics
    /// If `id` has no lizard body.
    pub(crate) fn lizard_at(&mut self, id: CreatureId) -> &mut Lizard {
        self.creature_mut(id)
            .lizard_mut()
            .expect("a realized lizard")
    }

    /// Realized lizard `id`.
    ///
    /// # Panics
    /// If `id` has no lizard body.
    #[must_use]
    pub fn lizard_ref(&self, id: CreatureId) -> &Lizard {
        self.creature(id).lizard().expect("a realized lizard")
    }

    /// `AbstractCreature.Realize` of a lizard: the body (the constructor's `effectColor`
    /// makes no net draw), no kill tag, a grasp per template grasp, then `InitiateAI`
    /// (the stand-in AI draws nothing) and every stuck object realized. False for a
    /// breed without data.
    pub(crate) fn realize_lizard(&mut self, id: CreatureId, rng: &mut UnityRandom) -> bool {
        let c = self.creature(id);
        if c.realized.is_some() {
            return false;
        }
        let Some(lizard) = Lizard::new(c, rng) else {
            return false;
        };
        let grasps = usize::try_from(c.template.grasps).unwrap_or(0);
        let c = self.creature_mut(id);
        c.realized = Some(RealizedCreature::Lizard(Box::new(lizard)));
        c.kill_tag = None;
        c.kill_tag_counter = 0;
        c.grasps = vec![None; grasps];
        c.grabbed_by.clear();
        if let Some(ai) = self.ai_of_mut(id) {
            ai.real_ai = true;
        }
        for s in self.creature(id).sticks.clone() {
            for other in [Some(s.a), s.b.creature()].into_iter().flatten() {
                if other == id || self.creature(other).is_realized() {
                    continue;
                }
                if self.creature(other).kind() == rw_assets::CreatureType::Fly {
                    self.realize_fly(other, rng);
                } else {
                    self.realize_lizard(other, rng);
                }
            }
            if let Entity::Object(o) = s.b {
                self.realize_object(o, rng);
            }
        }
        for s in self.spear_sticks_of(Entity::Creature(id)) {
            self.realize_object(s.spear, rng);
        }
        true
    }

    /// `AbstractCreature.RealizeInRoom` of an abstract lizard in its loaded room: a
    /// shelter puts it on the door's spawn tile, an undefined node is drawn, the
    /// abstract AI's `PlaceInRealizedRoom`, `Realize`, a tile from the node if it has
    /// none, and every connected object placed. The path finder's accessibility
    /// mapping is lizard step 4.
    pub(crate) fn realize_lizard_in_room(
        &mut self,
        id: CreatureId,
        shelter_spawn: Option<(i32, i32)>,
        rng: &mut UnityRandom,
    ) {
        let room = self.creature(id).pos.room;
        if let Some(tile) = shelter_spawn {
            self.creature_mut(id).pos.tile = tile;
        }
        if !self.creature(id).pos.node_defined() {
            let t = self.creature(id).template;
            let Some(data) = self.abstract_room(room) else {
                return;
            };
            let specific = rng.range_i32(0, t.nodes_relevant(data));
            self.creature_mut(id).pos.node = t.common_node_index(data, specific);
        }
        self.place_in_realized_room(id, rng);
        if self.creature(id).in_den {
            return;
        }
        if !self.realize_lizard(id, rng) && self.creature(id).lizard().is_none() {
            return;
        }
        if self.realized(room).is_none() {
            log::warn!("TRYING TO REALIZE IN NON REALIZED ROOM!");
            return;
        }
        let mut pos = self.creature(id).pos;
        if !pos.tile_defined() {
            pos.tile = self.local_coordinate_of_node(room, pos.node, rng);
            self.creature_mut(id).pos = pos;
        }
        for o in self.connected_objects(Entity::Creature(id)) {
            match o {
                Entity::Creature(o) => {
                    self.creature_mut(o).pos = pos;
                    if self.creature(o).lizard().is_some() {
                        self.place_lizard(o, room, rng);
                    }
                }
                Entity::Object(o) => {
                    self.object_mut(o).pos = pos;
                    self.place_object(o, rng);
                }
            }
        }
    }

    /// `AbstractCreatureAI.PlaceInRealizedRoom`: near its destination the creature takes
    /// `timeBuffer * offScreenSpeed` random steps from its tile; else it is put along a
    /// `QuickPathFinder` path toward the destination (or the exit its path leaves by)
    /// as far as the time buffer carries it.
    fn place_in_realized_room(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        let (parent, t) = (c.pos, c.template);
        let Some(ai) = self.ai_of(id) else { return };
        let (dest, time_buffer, real_ai) = (ai.destination, ai.time_buffer, ai.real_ai);
        let Some(realized) = self.realized(parent.room) else {
            log::warn!("TRYING TO PLACE IN REALIZED ROOM WITH NO REALIZED ROOM");
            return;
        };
        let (w, h) = (realized.room.width(), realized.room.height());
        let mut pos = parent;
        if !parent.tile_defined() && parent.node_defined() {
            pos.tile = self.local_coordinate_of_node(parent.room, parent.node, rng);
        }
        pos.tile = (pos.tile.0.clamp(0, w - 1), pos.tile.1.clamp(0, h - 1));
        let manhattan = (pos.tile.0 - dest.tile.0).abs() + (pos.tile.1 - dest.tile.1).abs();
        if pos.room == dest.room
            && pos.node == dest.node
            && (manhattan < 20 || !dest.tile_defined())
        {
            let repeats = (time_buffer as f32 * t.off_screen_speed) as i32;
            let tile = self.randomize_spawn_position(id, pos.tile, repeats, rng);
            self.creature_mut(id).pos.tile = tile;
            return;
        }
        let mut target = WorldCoordinate::new(parent.room, -1, -1, -1);
        if dest.room == parent.room {
            target = dest;
            if !target.tile_defined() {
                target.tile = self.local_coordinate_of_node(parent.room, dest.node, rng);
            }
        } else {
            if real_ai {
                log::debug!("the realized AI's path finder `DestinationExit` is lizard step 4");
            }
            let exit = self.destination_exit(id, parent.room, parent.node);
            if exit > -1 {
                target.tile = self.local_coordinate_of_node(parent.room, exit, rng);
            }
        }
        if !target.tile_defined() {
            return;
        }
        let Some(map) = self.realized(parent.room).and_then(|r| r.ai_map.clone()) else {
            return;
        };
        let Some(path) = QuickPathFinder::new(pos.tile, target.tile, &map, &t).run() else {
            return;
        };
        if path.tiles.is_empty() {
            return;
        }
        // `offscreenSpeedFac` is 1 for every base-game AI.
        let len = i32::try_from(path.tiles.len()).unwrap_or(i32::MAX);
        let steps = ((time_buffer as f32 * t.off_screen_speed) as i32).clamp(0, len - 1);
        self.creature_mut(id).pos.tile = path.tiles[usize::try_from(steps).unwrap_or(0)];
    }

    /// `AbstractCreatureAI.DestinationExit` without a realized path finder: the first
    /// step of the path in `room` reachable from `entered_by`.
    fn destination_exit(&self, id: CreatureId, room: i32, entered_by: i32) -> i32 {
        let t = self.creature(id).template;
        let Some(ai) = self.ai_of(id) else { return -1 };
        ai.path
            .iter()
            .find(|p| {
                p.room == room
                    && (p.node == entered_by
                        || self.connection_possible(room, entered_by, p.node, &t))
            })
            .map_or(-1, |p| p.node)
    }

    /// `RandomizeSpawnPositionInRoom`: a random target tile, then up to 500 steps, each
    /// trying every outgoing connection of the tile (a draw for each try) and taking the
    /// allowed one nearest the target that can come back.
    fn randomize_spawn_position(
        &self,
        id: CreatureId,
        mut tile: (i32, i32),
        repeats: i32,
        rng: &mut UnityRandom,
    ) -> (i32, i32) {
        let room = self.creature(id).pos.room;
        let t = self.creature(id).template;
        let Some(realized) = self.realized(room) else {
            return tile;
        };
        let target = Vec2::new(
            rng.range_i32(0, realized.room.width()) as f32,
            rng.range_i32(0, realized.room.height()) as f32,
        );
        let Some(map) = realized.ai_map.as_ref() else {
            return tile;
        };
        for _ in 0..repeats.min(500) {
            let paths = &map.tile(tile.0, tile.1).outgoing_paths;
            let mut best = None;
            let mut dist = f32::MAX;
            for _ in 0..paths.len() {
                let len = i32::try_from(paths.len()).unwrap_or(i32::MAX);
                let c = &paths[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)];
                let d = target.distance(Vec2::new(c.dest.tile.0 as f32, c.dest.tile.1 as f32));
                if d >= dist || d.is_nan() || !map.is_connection_allowed_for_creature(c, &t) {
                    continue;
                }
                let back = map
                    .tile(c.dest.tile.0, c.dest.tile.1)
                    .outgoing_paths
                    .iter()
                    .any(|b| t.connection_resistance(b.kind).allowed() && b.dest.tile == tile);
                if back {
                    best = Some(c.dest.tile);
                    dist = d;
                }
            }
            if let Some(b) = best {
                tile = b;
            }
        }
        tile
    }

    /// `Creature.PlaceInRoom` of a realized lizard: added to the room, each chunk on its
    /// tile's middle a random degree off (a draw each), still; what it holds on its
    /// head; then `NewRoom`.
    pub(crate) fn place_lizard(&mut self, id: CreatureId, room: i32, rng: &mut UnityRandom) {
        self.lizard_add_object(id, room);
        let middle = Vec2::middle_of_tile(self.creature(id).pos.tile);
        let Some(l) = self.creature_mut(id).lizard_mut() else {
            return;
        };
        for c in &mut l.body.chunks {
            c.reset_at(middle + Vec2::from_degrees(rng.value() * 360.0));
            c.set_pos = None;
        }
        let main = l.head().pos;
        let held: Vec<Entity> = self
            .creature(id)
            .grasps
            .iter()
            .flatten()
            .map(|g| g.grabbed)
            .collect();
        for e in held {
            match e {
                Entity::Object(o) => self.object_joins_room(o, room),
                Entity::Creature(o) => match &mut self.creature_mut(o).realized {
                    Some(RealizedCreature::Fly(f)) => f.room = Some(room),
                    Some(RealizedCreature::Lizard(l)) => l.room = Some(room),
                    _ => continue,
                },
            }
            if let Some(chunks) = self.body_chunks_mut(e) {
                for c in chunks {
                    c.reset_at(main);
                }
            }
        }
        self.lizard_new_room(id);
    }

    /// `Room.AddObject` of a lizard: the end of the room's update list. In a viewed
    /// room the game builds its `LizardGraphics` (lizard step 5).
    pub(crate) fn lizard_add_object(&mut self, id: CreatureId, room: i32) {
        let seq = self.next_update_seq();
        if let Some(l) = self.creature_mut(id).lizard_mut() {
            l.room = Some(room);
            l.update_order = seq;
            l.slated = false;
        }
    }

    /// `Creature.NewRoom`: the stand-in AI's `NewRoom` does nothing; 40 ticks of
    /// `newToRoomInvinsibility`.
    pub(crate) fn lizard_new_room(&mut self, id: CreatureId) {
        if let Some(l) = self.creature_mut(id).lizard_mut() {
            l.new_to_room_invinsibility = 40;
        }
    }

    /// `AbstractCreature.Abstractize(coord)` of a realized lizard: at its own position
    /// `distanceToMyNode` is its exit distance (or the walk to its node's entrance);
    /// `Move(coord)`, what it holds abstracted with it; the realized AI's destination is
    /// defined again and its path rebuilt (the abstract `FindPath` stands in for the
    /// path finder's `CreatePathForAbstractreature`, lizard step 4), and the AI dropped.
    pub(crate) fn abstractize_lizard(
        &mut self,
        id: CreatureId,
        coord: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        if c.lizard().is_none() {
            return;
        }
        let room = c.pos.room;
        if !self.realized(room).is_some_and(RealizedRoom::ready_for_ai) {
            self.creature_mut(id).realized = None;
            return;
        }
        let t = c.template;
        let mut pos = c.pos;
        let mut distance = 0;
        if coord == pos {
            pos = self.define_node(pos, &t, rng);
            self.creature_mut(id).pos = pos;
            distance = self.lizard_node_distance(pos, &t);
        }
        let c = self.creature_mut(id);
        c.time_spent_here = 0;
        c.distance_to_my_node = distance;
        self.move_creature(id, coord);
        let held: Vec<Entity> = self
            .creature(id)
            .grasps
            .iter()
            .flatten()
            .map(|g| g.grabbed)
            .collect();
        // Cleared first: a circular chain stops here.
        let body = self.creature_mut(id).realized.take();
        for e in held {
            match e {
                Entity::Object(o) => self.abstractize_object(o, coord),
                Entity::Creature(o) if self.creature(o).fly().is_some() => {
                    self.abstractize_fly(o, coord);
                }
                Entity::Creature(o) if self.creature(o).lizard().is_some() => {
                    self.abstractize_lizard(o, coord, rng);
                }
                Entity::Creature(_) => log::warn!("a lizard holds a creature it cannot abstract"),
            }
        }
        self.creature_mut(id).realized = body;
        let real_ai = self.ai_of(id).is_some_and(|a| a.real_ai);
        if t.ai && real_ai {
            let dest = self.ai_of(id).map(|a| a.destination).unwrap_or(pos);
            let dest = self.define_node(dest, &t, rng);
            self.set_destination(id, dest, rng);
            let pos = self.creature(id).pos;
            if let Some(ai) = self.ai_of_mut(id) {
                ai.time_buffer = 0;
                if ai.destination.same_room_and_node(pos) {
                    ai.path.clear();
                }
            }
            let dest = self.ai_of(id).map(|a| a.destination);
            if let Some(dest) = dest
                && !dest.same_room_and_node(self.creature(id).pos)
            {
                self.find_path(id, dest, rng);
            }
            if let Some(ai) = self.ai_of_mut(id) {
                ai.real_ai = false;
            }
        }
        self.creature_mut(id).realized = None;
    }

    /// `distanceToMyNode` of a lizard abstracted where it stands: its exit distance to
    /// its node, else the walk to the node's entrance when it stands on or by an
    /// accessible tile of an exit or den, else 0 at a border exit.
    fn lizard_node_distance(
        &self,
        pos: WorldCoordinate,
        t: &crate::creature::CreatureTemplate,
    ) -> i32 {
        let (Some(realized), Some(data)) = (self.realized(pos.room), self.abstract_room(pos.room))
        else {
            return 0;
        };
        let Some(map) = realized.ai_map.as_ref() else {
            return 0;
        };
        let specific = t.specific_node_index(data, pos.node);
        let d = map.exit_distance_for_creature_and_check_neighbours(pos.tile, specific, t);
        if d != -1 {
            return d;
        }
        let node = usize::try_from(pos.node).unwrap_or(usize::MAX);
        let exits = realized.room.shortcut_map().nodes().len();
        if node < exits && map.tile_or_neighbors_accessible_to_creature(pos.tile, t) {
            if let Some(s) = realized.room.shortcut_map().leading_to_node(node) {
                return (pos.tile.0 - s.start.0).abs() + (pos.tile.1 - s.start.1).abs();
            }
            return 0;
        }
        if data
            .nodes
            .get(node)
            .is_some_and(|n| n.kind.is_border_exit())
        {
            return 0;
        }
        -1
    }

    /// The realized lizards in room `room`, with their update order.
    #[must_use]
    pub fn room_lizards(&self, room: i32) -> Vec<(u64, CreatureId)> {
        self.creatures()
            .filter_map(|(id, c)| {
                let l = c.lizard()?;
                (l.room == Some(room)).then_some((l.update_order, id))
            })
            .collect()
    }
}
