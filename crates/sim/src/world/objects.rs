//! The world's objects other than creatures: the arena, each abstract room's `entities`
//! list beside its creatures, the items a room makes when it is realized
//! (`LoadFromDataString`'s file items, `Room.Loaded`'s `MultiplayerItem`s, reliable
//! spears and random items), `Move`, and the region state's saved objects
//! (`AdaptWorldToRegionState`, `AdaptRegionStateToWorld`, `StuckInWallTick`).
//! Behaviour: `rainworld-re/notes/items.md`.
//!
//! Mirrors: AbstractRoom.cs, Room.cs, RegionState.cs

use rw_assets::{
    AbstractObjectType, MultiplayerItemData, MultiplayerItemType, PlacedObject, PlacedObjectType,
    Room, RoomSettings,
};
use rw_random::UnityRandom;

use super::World;
use crate::creature::{IdIssuer, WorldCoordinate};
use crate::item::{AbstractObject, Entity, ObjectId, SavedObject};
use crate::{FloatExt, Vec2};

impl World {
    /// The object `id` (panics on an index the world did not hand out).
    #[must_use]
    pub fn object(&self, id: ObjectId) -> &AbstractObject {
        &self.objects[id.0]
    }

    pub fn object_mut(&mut self, id: ObjectId) -> &mut AbstractObject {
        &mut self.objects[id.0]
    }

    /// Every object, in creation order.
    pub fn objects(&self) -> impl Iterator<Item = (ObjectId, &AbstractObject)> {
        self.objects
            .iter()
            .enumerate()
            .map(|(i, o)| (ObjectId(i), o))
    }

    /// `AbstractRoom.entities` of global room `room`: creatures and objects in the
    /// order they joined.
    #[must_use]
    pub fn room_entities(&self, room: i32) -> &[Entity] {
        self.local(room)
            .map_or(&[], |l| self.lists[l].entities.as_slice())
    }

    /// `entities[Random.Range(0, entities.Count)]` of global room `room`: one draw over
    /// its creatures and objects (`None` for an empty list, where the game throws).
    pub fn random_room_entity(&self, room: i32, rng: &mut UnityRandom) -> Option<Entity> {
        let entities = self.room_entities(room);
        let i = rng.range_i32(0, i32::try_from(entities.len()).unwrap_or(i32::MAX));
        entities.get(usize::try_from(i).ok()?).copied()
    }

    /// The objects among a room's entities, in list order.
    pub fn room_objects(&self, room: i32) -> impl Iterator<Item = ObjectId> + '_ {
        self.room_entities(room).iter().filter_map(|e| match e {
            Entity::Object(o) => Some(*o),
            Entity::Creature(_) => None,
        })
    }

    /// The entity's `pos`.
    #[must_use]
    pub fn entity_pos(&self, e: Entity) -> WorldCoordinate {
        match e {
            Entity::Creature(id) => self.creature(id).pos,
            Entity::Object(id) => self.object(id).pos,
        }
    }

    #[must_use]
    pub fn entity_in_den(&self, e: Entity) -> bool {
        match e {
            Entity::Creature(id) => self.creature(id).in_den,
            Entity::Object(id) => self.object(id).in_den,
        }
    }

    pub(crate) fn entity_even_update(&self, e: Entity) -> bool {
        match e {
            Entity::Creature(id) => self.creature(id).even_update,
            Entity::Object(id) => self.object(id).even_update,
        }
    }

    /// The realized object's `UpdatableAndDeletable.evenUpdate` (the `eu` of its last
    /// update), which `MoveFromOutsideMyUpdate` compares; not the abstract entity's flag.
    pub(crate) fn realized_even_update(&self, e: Entity) -> bool {
        match e {
            Entity::Creature(id) => self
                .creature(id)
                .fly()
                .map_or(self.creature(id).even_update, |f| f.even_update),
            Entity::Object(id) => self
                .object(id)
                .realized
                .as_deref()
                .map_or(self.object(id).even_update, |w| w.even_update),
        }
    }

    /// Puts a new object into the arena and `AddEntity`s it to its room.
    pub(crate) fn add_object(&mut self, object: AbstractObject) -> ObjectId {
        let room = object.pos.room;
        self.objects.push(object);
        let id = ObjectId(self.objects.len() - 1);
        if let Some(l) = self.local(room) {
            self.lists[l].add_entity(Entity::Object(id));
        }
        id
    }

    /// Puts a new object into the arena in no room's entities (a swallowed object read
    /// from the save).
    pub(crate) fn add_detached_object(&mut self, object: AbstractObject) -> ObjectId {
        self.objects.push(object);
        ObjectId(self.objects.len() - 1)
    }

    /// `AbstractRoom.AddEntity` of object `id` into the room of its `pos`.
    pub(crate) fn add_object_entity(&mut self, id: ObjectId) {
        if let Some(l) = self.local(self.object(id).pos.room) {
            self.lists[l].add_entity(Entity::Object(id));
        }
    }

    /// `AbstractRoom.RemoveEntity` of object `id` from the room of its `pos`.
    pub(crate) fn remove_object_entity(&mut self, id: ObjectId) {
        if let Some(l) = self.local(self.object(id).pos.room) {
            self.lists[l].remove_entity(Entity::Object(id));
        }
    }

    /// `AbstractPhysicalObject.Move(newCoord)`: `ChangeRooms` between the rooms'
    /// entity lists. (Objects stuck to it come with the sticks, step 3.)
    pub fn move_object(&mut self, id: ObjectId, mut to: WorldCoordinate) {
        let from = self.object(id).pos;
        if to.same_room_and_node(from) {
            return;
        }
        let o = self.object_mut(id);
        o.time_spent_here = 0;
        if to.room != from.room {
            if let Some(l) = self.local(from.room) {
                self.lists[l].remove_entity(Entity::Object(id));
            }
            if let Some(l) = self.local(to.room) {
                self.lists[l].add_entity(Entity::Object(id));
            }
        }
        if !to.tile_defined() && from.room == to.room {
            to.tile = from.tile;
        }
        self.object_mut(id).pos = to;
    }

    /// `LoadFromDataString`'s items of realized room `index` on its first realization
    /// in the cycle (`abstractRoom.firstTimeRealized`), before `Room.Loaded`.
    pub(crate) fn place_loaded_file_items(
        &mut self,
        index: i32,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        let Some(r) = self.realized(index).filter(|r| r.first_time_realized()) else {
            return;
        };
        let room = std::sync::Arc::clone(&r.room);
        self.place_file_items(index, &room, ids, rng);
    }

    /// `Room.Loaded`'s `firstTimeRealized` pass of realized room `index` (after its
    /// scripts): its `ReliableSpear`s, then the random items (not in a shelter or gate).
    pub(crate) fn place_first_time_items(
        &mut self,
        index: i32,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        let Some(r) = self.realized(index).filter(|r| r.first_time_realized()) else {
            return;
        };
        let (room, settings) = (std::sync::Arc::clone(&r.room), r.settings.clone());
        let placed = settings.placed_objects_for(self.timeline, self.mods);
        self.place_reliable_spears(index, &placed, ids);
        let Some(data) = self.abstract_room(index) else {
            return;
        };
        if !data.shelter() && !data.gate() {
            self.place_random_items(index, &room, &settings, ids, rng);
        }
    }

    /// Every item realized room `index` makes when it loads, in the game's order without
    /// what comes between (world-only tests): the file items, a story's
    /// `MultiplayerItem`s, the `firstTimeRealized` pass.
    #[cfg(test)]
    pub(crate) fn place_loaded_room_items(
        &mut self,
        index: i32,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        self.place_loaded_file_items(index, ids, rng);
        let placed = self.realized(index).map_or_else(Vec::new, |r| {
            r.settings.placed_objects_for(self.timeline, self.mods)
        });
        if self.story {
            for o in placed.iter().filter(|o| o.active) {
                self.spawn_multiplayer_item(index, o, ids, rng);
            }
        }
        self.place_first_time_items(index, ids, rng);
    }

    /// `Room.GetWorldCoordinate(pos)`: the tile under a room position, no node.
    fn placed_coordinate(index: i32, o: &PlacedObject) -> WorldCoordinate {
        let (x, y) = Vec2::new(o.pos[0], o.pos[1]).tile();
        WorldCoordinate::new(index, x, y, -1)
    }

    /// `SpawnMultiplayerItem` of placed object `o` when it is a `MultiplayerItem`
    /// (the placed-object loop of a story's `Room.Loaded`): one draw against its chance;
    /// then an ID (`GetNewID()`) and a rock, a spear, an explosive spear, a scavenger bomb
    /// or a spore plant. The last two are not realized by the port, and the spore plant is
    /// a plain object without its consumable fields (docs/divergences/ D14).
    pub(crate) fn spawn_multiplayer_item(
        &mut self,
        index: i32,
        o: &PlacedObject,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        if o.object_type(self.mods) != Some(PlacedObjectType::MultiplayerItem) {
            return;
        }
        let Some(data) = MultiplayerItemData::parse(&o.data, self.mods) else {
            return;
        };
        if rng.value() > data.chance {
            return;
        }
        let pos = Self::placed_coordinate(index, o);
        let id = ids.new_id(-1);
        let object = match data.kind {
            Some(MultiplayerItemType::Spear) => AbstractObject::new_spear(id, pos, false),
            Some(MultiplayerItemType::ExplosiveSpear) => AbstractObject::new_spear(id, pos, true),
            Some(MultiplayerItemType::Bomb) => {
                AbstractObject::new_plain(AbstractObjectType::ScavengerBomb, id, pos)
            }
            Some(MultiplayerItemType::SporePlant) => {
                AbstractObject::new_plain(AbstractObjectType::SporePlant, id, pos)
            }
            Some(MultiplayerItemType::Rock) | None => AbstractObject::new_rock(id, pos),
        };
        self.add_object(object);
    }

    /// `Room.Loaded`'s `ReliableSpear`s: every active one is a spear (an ID
    /// from `GetNewID()`) at its tile, remembering the placed object it came from.
    fn place_reliable_spears(&mut self, index: i32, placed: &[PlacedObject], ids: &mut IdIssuer) {
        let name = self
            .abstract_room(index)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        for (n, o) in placed.iter().enumerate().filter(|(_, o)| o.active) {
            if o.object_type(self.mods) != Some(PlacedObjectType::ReliableSpear) {
                continue;
            }
            let pos = Self::placed_coordinate(index, o);
            let mut spear = AbstractObject::new_spear(ids.new_id(-1), pos, false);
            spear.placed_object_origin = Some(format!("{name}:{n}"));
            self.add_object(spear);
        }
    }

    /// `LoadFromDataString`'s items: each entry draws whether it appears (a spear 0.6, any
    /// other type 0.75); one that does takes an ID, and a rock (0) or spear (1) joins
    /// the room.
    fn place_file_items(
        &mut self,
        index: i32,
        room: &Room,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        for item in room.items() {
            let chance = if item.kind == 1 { 0.6 } else { 0.75 };
            if rng.value() >= chance {
                continue;
            }
            let id = ids.new_id(-index);
            let (x, y) = item.tile(room.height());
            let pos = WorldCoordinate::new(index, x, y, -1);
            match item.kind {
                0 => {
                    self.add_object(AbstractObject::new_rock(id, pos));
                }
                1 => {
                    self.add_object(AbstractObject::new_spear(id, pos, false));
                }
                _ => {}
            }
        }
    }

    /// `Room.Loaded`'s random items (Remix off): `(int)(W * H * density^2 / 5) + 1`
    /// attempts, each a `RandomTile` (two draws); on a free tile with the three tiles
    /// below it Solid, an ID and a draw for a spear (`SpearSpawnModifier`), a spear also
    /// one for `explosive`.
    fn place_random_items(
        &mut self,
        index: i32,
        room: &Room,
        settings: &RoomSettings,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        let (w, h) = (room.width(), room.height());
        let attempts =
            (w as f32 * h as f32 * settings.random_item_density.mathf_pow(2.0) / 5.0) as i32;
        let spear_chance = self
            .timeline
            .spear_spawn_modifier(settings.random_item_spear_chance);
        let explosive_chance = self.timeline.spear_spawn_explosive_random_chance();
        for _ in 0..=attempts {
            let x = rng.range_i32(0, w);
            let y = rng.range_i32(0, h);
            let below = (-1..=1).all(|dx| room.tile(x + dx, y - 1).is_solid());
            if room.tile(x, y).is_solid() || !below {
                continue;
            }
            let id = ids.new_id(-index);
            let pos = WorldCoordinate::new(index, x, y, -1);
            let object = if rng.value() < spear_chance {
                AbstractObject::new_spear(id, pos, rng.value() < explosive_chance)
            } else {
                AbstractObject::new_rock(id, pos)
            };
            self.add_object(object);
        }
    }

    /// `AdaptWorldToRegionState`'s objects: each saved Rock or Spear whose room is in
    /// this world joins it (kept in `savedObjects`); every other string is kept as text
    /// (`unrecognizedSavedObjects`, where the game also puts objects it cannot place).
    pub(crate) fn load_saved_objects(&mut self, saved: Vec<String>) -> (Vec<String>, Vec<String>) {
        let mut loaded = Vec::new();
        let mut unrecognized = Vec::new();
        for s in saved {
            let object = SavedObject::parse(&s, self.mods).and_then(|o| {
                let room = self.room_from_save_name(o.room)?;
                Some(o.into_object(room))
            });
            match object {
                Some(o) => {
                    self.add_object(o);
                    loaded.push(s);
                }
                None => unrecognized.push(s),
            }
        }
        (loaded, unrecognized)
    }

    /// `RainCycleTick`'s objects: every stuck spear among the rooms' entities counts
    /// down `ticks` cycles.
    pub(crate) fn stuck_in_wall_tick(&mut self, ticks: i32) {
        if ticks <= 0 {
            return;
        }
        for l in 0..self.lists.len() {
            for i in 0..self.lists[l].entities.len() {
                if let Entity::Object(id) = self.lists[l].entities[i]
                    && let Some(spear) = self.objects[id.0].spear_mut()
                    && spear.stuck_in_wall()
                {
                    spear.stuck_in_wall_tick(ticks);
                }
            }
        }
    }

    /// `AdaptRegionStateToWorld`'s objects, room by room in entity order: spears stuck
    /// in a wall anywhere, and every object in a shelter.
    pub(crate) fn objects_to_save(&self) -> Vec<String> {
        let mut saved = Vec::new();
        for (l, lists) in self.lists.iter().enumerate() {
            let shelter = self.data.rooms[l].shelter();
            for e in &lists.entities {
                let Entity::Object(id) = *e else { continue };
                let o = self.object(id);
                if o.stuck_in_wall() || shelter {
                    saved.push(o.save_string(&self.coordinate_save_string(o.pos)));
                }
            }
        }
        saved
    }
}

#[cfg(test)]
#[path = "tests/objects.rs"]
mod tests;
