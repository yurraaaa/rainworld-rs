//! `Mycelium`: a hair of a `CoralNeuronSystem` owner (an overseer's lens, the room's
//! `WallMycelia`), a short chain of points pulled toward a fixed length, reaching out from
//! its owner and linking up with another owner's. The strands of a world live in one
//! [`MyceliumPool`] (an owner holds [`MyceliumId`]s), so a strand of one owner can pair
//! with a strand of another that is in the same room's system, as the game's `mycelia`
//! registry lets it. The sprites are `rw-graphics`'. Behaviour:
//! `rainworld-re/notes/superstructure-coral.md`.
//!
//! Mirrors: CoralBrain/Mycelium.cs, CoralBrain/JobData.cs (the Burst job's maths, run in
//! plain Rust), IOwnMycelia.cs, ExtraExtentions (Assembly-CSharp-firstpass: `normalized`,
//! `Indexer`)

use std::collections::HashMap;

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::neuron_spark::NeuronSpark;
use crate::world::AiMap;
use crate::{CameraView, FloatExt, Vec2};

/// A strand in a [`MyceliumPool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MyceliumId(pub u32);

/// A `CoralNeuronSystem` object (the game's object identity: a room's re-created system is
/// another one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemId(pub u32);

/// A room's system as a strand reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemState {
    pub id: SystemId,
    /// `Frozen`.
    pub frozen: bool,
}

/// Which `IOwnMycelia` a strand belongs to (the game compares owner references): one per
/// `WallMycelia`, one per `OverseerGraphics` module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnerKey(pub u32);

/// `Mycelium.MyceliaConnection`: two strands, as ordered when it was made. Both ends hold
/// a copy; a link is only whole while both copies are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyceliaConnection {
    pub a: MyceliumId,
    pub b: MyceliumId,
}

impl MyceliaConnection {
    /// `Other`.
    #[must_use]
    pub fn other(self, me: MyceliumId) -> MyceliumId {
        if me == self.a { self.b } else { self.a }
    }
}

/// What a strand asks of its owner (`IOwnMycelia`).
pub trait MyceliumOwner {
    /// `ConnectionPos(index, 1)`: where strand `index` hangs from.
    fn connection_pos(&self, index: usize) -> Vec2;
    /// `ResetDir(index)`: which way `Reset` reaches.
    fn reset_dir(&self, index: usize, rng: &mut UnityRandom) -> Vec2;
}

/// An owner whose strands all hang from one point and reach in a random direction (the
/// overseer's `ConnectionPos` and `ResetDir`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointOwner(pub Vec2);

impl MyceliumOwner for PointOwner {
    fn connection_pos(&self, _index: usize) -> Vec2 {
        self.0
    }

    fn reset_dir(&self, _index: usize, rng: &mut UnityRandom) -> Vec2 {
        Vec2::random_unit(rng)
    }
}

/// What `Mycelium.Update` reads of the room its owner is in.
#[derive(Clone, Copy)]
pub struct MyceliumEnv<'a> {
    pub room: &'a Room,
    pub room_index: i32,
    /// `OwnerRoom.aimap`.
    pub ai_map: Option<&'a AiMap>,
    pub camera: CameraView,
    /// `OwnerRoom.camerasChangedTick`.
    pub cameras_changed_tick: i32,
    /// The room's system, if it has one.
    pub system: Option<SystemState>,
}

/// The pool and the room an owner's strands are made and updated in.
pub struct MyceliaCx<'a> {
    pub pool: &'a mut MyceliumPool,
    pub env: MyceliumEnv<'a>,
}

/// One strand.
#[derive(Debug, Clone, PartialEq)]
pub struct Mycelium {
    /// Per point: position, last position, velocity.
    pub points: Vec<[Vec2; 3]>,
    pub con_rad: f32,
    pub length: f32,
    pub index: usize,
    pub owner: OwnerKey,
    /// `system`.
    pub system: Option<SystemId>,
    pub connection: Option<MyceliaConnection>,
    pub last_camera_cull_tick: i32,
    pub viewed_by_camera: bool,
    pub use_static_culling: bool,
    pub culled: bool,
    pub last_culled: bool,
    pub move_away_from_walls: bool,
    pub rest: i32,
    /// Its owner is gone: it lives on only as an entry of a live registry.
    pub owner_gone: bool,
}

impl Mycelium {
    /// `Base`.
    #[must_use]
    pub fn base(&self) -> Vec2 {
        self.points[0][0]
    }

    /// `Tip`.
    #[must_use]
    pub fn tip(&self) -> Vec2 {
        self.points[self.points.len() - 1][0]
    }

    /// `Reset`: a random curve from `at` (the owner's `ResetDir` first), every point
    /// jittered, every velocity random: `2 + 2 * points` draws and the owner's.
    pub fn reset(&mut self, at: Vec2, owner: &dyn MyceliumOwner, rng: &mut UnityRandom) {
        let end = at + owner.reset_dir(self.index, rng) * (self.length * 0.6);
        let half = at.distance(end) * 0.5;
        let ca = at + (at.direction_to(end) + Vec2::random_unit(rng)).normalized() * half;
        let cb = end + (end.direction_to(at) + Vec2::random_unit(rng)).normalized() * half;
        let last = (self.points.len() - 1) as f32;
        for (i, p) in self.points.iter_mut().enumerate() {
            let pos = at.bezier(ca, end, cb, i as f32 / last) + Vec2::random_unit(rng);
            *p = [pos, pos, Vec2::random_unit(rng)];
        }
    }

    /// `Phase1` and `Phase4`: each point moves by its velocity and is pulled to `conRad`
    /// from the one before, with a straightening push two apart; a strand that moves away
    /// from walls is also pushed off the ones within three tiles.
    pub(crate) fn step(&mut self, ai_map: Option<&AiMap>) {
        let mut next: Vec<[Vec2; 3]> = self
            .points
            .iter()
            .map(|&[pos, _, vel]| [pos + vel, pos, vel * 0.999])
            .collect();
        for i in 0..next.len() {
            if i > 0 {
                let f = next[i][0] - next[i - 1][0];
                let push = f.normalized() * ((self.con_rad - f.length()) * 0.5);
                next[i][0] += push;
                next[i][2] += push;
                next[i - 1][0] -= push;
                next[i - 1][2] -= push;
                if i > 1 {
                    let dir = (next[i][0] - next[i - 2][0]).normalized();
                    next[i][2] += dir * 0.2;
                    next[i - 2][2] -= dir * 0.2;
                }
            }
            if let (true, Some(map)) = (self.move_away_from_walls, ai_map) {
                let tile = next[i][0].tile();
                if let Some(near) = map.terrain_proximity_inside(tile.0, tile.1)
                    && near < 4
                {
                    next[i][2] += Self::wall_push(map, tile, near);
                }
            }
        }
        self.points = next;
    }

    /// The push off the walls around `tile`: the four directions weighted by the terrain
    /// proximity around each, as strong as `proximity` is small.
    fn wall_push(map: &AiMap, tile: (i32, i32), proximity: i32) -> Vec2 {
        const FOUR: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];
        let mut sum = Vec2::ZERO;
        for (jx, jy) in FOUR {
            let mut weight = 0;
            for (kx, ky) in FOUR {
                weight += map
                    .terrain_proximity_inside(tile.0 + jx + kx, tile.1 + jy + ky)
                    .unwrap_or(0);
            }
            sum += Vec2::new((jx * weight) as f32, (jy * weight) as f32);
        }
        sum.normalized() * (proximity as f32).lerp_map(0.0, 3.0, 2.0, 0.2)
    }
}

/// Every strand of a world, and each system's registry of them (`CoralNeuronSystem.mycelia`,
/// by system). Ids are never reused; a freed strand leaves an empty slot.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MyceliumPool {
    items: Vec<Option<Box<Mycelium>>>,
    registries: HashMap<SystemId, Vec<MyceliumId>>,
    next_owner: u32,
    next_system: u32,
    /// The `NeuronSpark`s strands added to their room since the caller last took them.
    pub sparks: Vec<NeuronSpark>,
}

impl MyceliumPool {
    /// A new `IOwnMycelia` identity.
    pub fn new_owner(&mut self) -> OwnerKey {
        self.next_owner += 1;
        OwnerKey(self.next_owner)
    }

    #[must_use]
    pub fn get(&self, id: MyceliumId) -> Option<&Mycelium> {
        self.items.get(id.0 as usize)?.as_deref()
    }

    pub fn get_mut(&mut self, id: MyceliumId) -> Option<&mut Mycelium> {
        self.items.get_mut(id.0 as usize)?.as_deref_mut()
    }

    /// How many strands are alive.
    #[must_use]
    pub fn live(&self) -> usize {
        self.items.iter().flatten().count()
    }

    /// Every live strand with its id.
    pub fn iter(&self) -> impl Iterator<Item = (MyceliumId, &Mycelium)> {
        self.items.iter().enumerate().filter_map(|(i, m)| {
            Some((
                MyceliumId(u32::try_from(i).unwrap_or(u32::MAX)),
                m.as_deref()?,
            ))
        })
    }

    /// A system's `mycelia`, in registration order.
    #[must_use]
    pub fn registry(&self, system: SystemId) -> &[MyceliumId] {
        self.registries.get(&system).map_or(&[], Vec::as_slice)
    }

    /// `new CoralNeuronSystem()`: a new system with an empty registry.
    pub fn create_registry(&mut self) -> SystemId {
        self.next_system += 1;
        let id = SystemId(self.next_system);
        self.registries.insert(id, Vec::new());
        id
    }

    /// A room is gone with its system: its registry goes, and the strands only it reached
    /// with it (a strand left holding it keeps pointing at nothing, as the game's keeps its
    /// dead system).
    pub fn drop_registry(&mut self, system: SystemId) {
        self.registries.remove(&system);
        self.sweep();
    }

    /// The owner is gone: its strands are freed (others' links to them read as broken).
    pub fn remove_owner(&mut self, owner: OwnerKey) {
        for slot in &mut self.items {
            if slot.as_ref().is_some_and(|m| m.owner == owner) {
                *slot = None;
            }
        }
    }

    /// The owner (an overseer's module) is gone. Its strands stay while a live registry can
    /// still pick them, as the game's do; the rest are freed.
    pub fn release_owner(&mut self, owner: OwnerKey) {
        for m in self.items.iter_mut().flatten().filter(|m| m.owner == owner) {
            m.owner_gone = true;
        }
        self.sweep();
    }

    /// Frees the strands of gone owners that no live registry holds.
    fn sweep(&mut self) {
        let registries = &self.registries;
        for slot in &mut self.items {
            if slot.as_ref().is_some_and(|m| {
                m.owner_gone && m.system.is_none_or(|s| !registries.contains_key(&s))
            }) {
                *slot = None;
            }
        }
    }

    /// `system.mycelia.Remove(strand)`.
    pub fn unregister(&mut self, system: SystemId, id: MyceliumId) {
        if let Some(r) = self.registries.get_mut(&system) {
            r.retain(|&x| x != id);
        }
    }

    /// Sets `connection` (an owner's own linking, as `OverseerGraphics.Update` does).
    pub fn set_connection(&mut self, id: MyceliumId, connection: Option<MyceliaConnection>) {
        if let Some(m) = self.get_mut(id) {
            m.connection = connection;
        }
    }

    /// `ConnectSystem`.
    pub fn connect_system(&mut self, id: MyceliumId, system: Option<SystemId>) {
        if let Some(system) = system
            && let Some(r) = self.registries.get_mut(&system)
        {
            r.push(id);
        }
        if let Some(m) = self.get_mut(id) {
            m.system = system;
        }
    }

    /// `OverseerGraphics.UpdateNeuronSystemForMycelia` for one strand: out of the registry
    /// it is in, into `room`'s.
    pub fn move_to_system(&mut self, id: MyceliumId, system: Option<SystemId>) {
        let Some(m) = self.get(id) else {
            return;
        };
        if m.system == system {
            return;
        }
        if let Some(old) = m.system
            && let Some(r) = self.registries.get_mut(&old)
        {
            r.retain(|&x| x != id);
        }
        self.connect_system(id, system);
    }

    /// The constructor: `Custom.IntClamp((int)(max(length, lerp(length, 40, 0.5)) / 15), 2,
    /// 20)` points, `Reset(init)`, and the registration in `system`'s registry.
    pub fn add(
        &mut self,
        system: Option<SystemId>,
        owner: (OwnerKey, &dyn MyceliumOwner),
        index: usize,
        length: f32,
        init: Vec2,
        rng: &mut UnityRandom,
    ) -> MyceliumId {
        let id = MyceliumId(u32::try_from(self.items.len()).unwrap_or(u32::MAX));
        let reach = length.max(length.lerp(40.0, 0.5));
        let count = ((reach / 15.0) as i32).clamp(2, 20);
        let n = usize::try_from(count).unwrap_or(2);
        let mut m = Mycelium {
            points: vec![[Vec2::ZERO; 3]; n],
            con_rad: length / count as f32,
            length,
            index,
            owner: owner.0,
            system: None,
            connection: None,
            last_camera_cull_tick: -1,
            viewed_by_camera: false,
            use_static_culling: true,
            culled: false,
            last_culled: false,
            move_away_from_walls: true,
            rest: 0,
            owner_gone: false,
        };
        self.items.push(None);
        self.connect_system(id, system);
        m.system = system;
        m.reset(init, owner.1, rng);
        self.items[id.0 as usize] = Some(Box::new(m));
        id
    }

    /// `Reset(at)` of strand `id`.
    pub fn reset(
        &mut self,
        id: MyceliumId,
        at: Vec2,
        owner: &dyn MyceliumOwner,
        rng: &mut UnityRandom,
    ) {
        if let Some(m) = self.get_mut(id) {
            m.reset(at, owner, rng);
        }
    }

    /// `Mycelium.Update` of strand `id`, whose owner is `owner`.
    pub fn update(
        &mut self,
        id: MyceliumId,
        owner: &dyn MyceliumOwner,
        env: &MyceliumEnv<'_>,
        rng: &mut UnityRandom,
    ) {
        let Some(m) = self.get_mut(id) else {
            return;
        };
        let frozen = m.system.is_some()
            && env
                .system
                .is_some_and(|s| Some(s.id) == m.system && s.frozen);
        m.culled = if m.use_static_culling {
            frozen || !m.viewed_by_camera
        } else {
            frozen
        };
        if m.last_camera_cull_tick != env.cameras_changed_tick {
            m.viewed_by_camera =
                env.camera
                    .viewed(env.room_index, env.room, m.base(), m.length + 50.0);
            m.last_camera_cull_tick = env.cameras_changed_tick;
        }
        let uncull = m.last_culled && !m.culled;
        m.last_culled = m.culled;
        if uncull {
            m.reset(owner.connection_pos(m.index), owner, rng);
        }
        if m.culled || env.ai_map.is_none() {
            return;
        }
        m.step(env.ai_map);
        let index = m.index;
        m.points[0][0] = owner.connection_pos(index);
        m.points[0][2] *= 0.0;
        if m.rest > 0 {
            m.rest -= 1;
        }
        match m.connection {
            Some(connection) => self.update_connected(id, connection, rng),
            None => self.pair_up(id, rng),
        }
    }

    /// The linked branch: breaks when the other end let go, when the bases are too far or
    /// by chance; else pulls the tips together, striking a spark where they meet.
    fn update_connected(
        &mut self,
        id: MyceliumId,
        connection: MyceliaConnection,
        rng: &mut UnityRandom,
    ) {
        let other = connection.other(id);
        let other_connection = self.get(other).and_then(|o| o.connection);
        let (base_a, base_b, length_sum) = match (self.get(connection.a), self.get(connection.b)) {
            (Some(a), Some(b)) => (a.base(), b.base(), a.length + b.length),
            _ => (Vec2::ZERO, Vec2::ZERO, 0.0),
        };
        if other_connection != Some(connection)
            || !base_a.dist_less(base_b, length_sum)
            || rng.value() < 0.005
        {
            if let Some(m) = self.get_mut(id) {
                m.connection = None;
                m.rest = rng.range_i32(20, 200);
            }
            return;
        }
        let (Some(me), Some(them)) = (self.get(id), self.get(other)) else {
            return;
        };
        let (tip, other_tip) = (me.tip(), them.tip());
        let (last, other_last) = (me.points.len() - 1, them.points.len() - 1);
        if other_tip.dist_less(tip, 10.0) {
            let push = tip.direction_to(other_tip) * ((tip.distance(other_tip) - 1.0) * 0.5);
            if let Some(m) = self.get_mut(id) {
                m.points[last][0] += push;
                m.points[last][2] += push;
            }
            if let Some(o) = self.get_mut(other) {
                o.points[other_last][0] -= push;
                o.points[other_last][2] -= push;
            }
            if rng.value() < 0.05 {
                let at = (self.tip_of(other) + self.tip_of(id)) / 2.0;
                self.sparks.push(NeuronSpark::new(at, rng));
            }
        } else if let Some(m) = self.get_mut(id) {
            let pull = (other_tip - tip).clamp_magnitude(5.0);
            m.points[last][2] = m.points[last][2].lerp(pull, 0.5);
        }
    }

    fn tip_of(&self, id: MyceliumId) -> Vec2 {
        self.get(id).map_or(Vec2::ZERO, Mycelium::tip)
    }

    /// The unlinked branch: with no rest left, a random strand of its system may become
    /// its partner when it is another owner's, free and near enough.
    fn pair_up(&mut self, id: MyceliumId, rng: &mut UnityRandom) {
        let Some(m) = self.get(id) else {
            return;
        };
        let Some(room) = m.system else {
            return;
        };
        let count = self.registry(room).len();
        if m.rest >= 1 || count == 0 {
            return;
        }
        let pick =
            usize::try_from(rng.range_i32(0, i32::try_from(count).unwrap_or(0))).unwrap_or(0);
        let candidate = self.registry(room)[pick];
        let (Some(me), Some(other)) = (self.get(id), self.get(candidate)) else {
            return;
        };
        if candidate != id
            && other.owner != me.owner
            && other.connection.is_none()
            && me
                .base()
                .dist_less(other.base(), (me.length + other.length) * 0.75)
        {
            let connection = MyceliaConnection {
                a: id,
                b: candidate,
            };
            if let Some(m) = self.get_mut(id) {
                m.connection = Some(connection);
            }
            if let Some(o) = self.get_mut(candidate) {
                o.connection = Some(connection);
            }
        }
    }
}

#[cfg(test)]
#[path = "mycelium_tests.rs"]
mod tests;
