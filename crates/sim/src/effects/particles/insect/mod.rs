//! The cosmetic insects (`CosmeticInsect` and its eleven kinds) a room's
//! `InsectCoordinator` spawns when the room becomes viewed. Every draw of their
//! constructors and updates is a `UnityEngine.Random` call on the shared stream, so
//! they live in the sim and update in the room's update list; their sprites are
//! `rw-graphics`' (`effects/particles/insect.rs`). Nothing here touches gameplay but the
//! fireflies' lights.
//!
//! Mirrors: CosmeticInsect.cs, CosmeticSprite.cs (Update)

mod ant;
mod avoider;
mod beetle;
mod coordinator;
mod fire_fly;
mod grass_hopper;
mod mini_fly;
mod moth;
mod red_swarmer;
mod rock_flea;
mod tiny_dragonfly;
mod wasp;
mod water_glowworm;

#[cfg(test)]
mod tests;
pub(crate) mod turn;

use rw_assets::{Room, RoomEffectType};
use rw_random::UnityRandom;

pub use ant::Ant;
pub use avoider::{CreatureAvoider, InsectCreature};
pub use beetle::Beetle;
pub use coordinator::{InsectCoordinator, InsectGroup};
pub use fire_fly::{FireFly, FireFlyLight};
pub use grass_hopper::GrassHopper;
pub use mini_fly::{BuzzTarget, MiniFly};
pub use moth::Moth;
pub use red_swarmer::RedSwarmer;
pub use rock_flea::RockFlea;
pub use tiny_dragonfly::TinyDragonfly;
pub use wasp::Wasp;
pub use water_glowworm::WaterGlowworm;

use crate::color::RandomVariation;
use crate::creature::CreatureId;
use crate::physics::shared_physics::TerrainCollisionData;
use crate::world::AiMap;
use crate::{FloatExt, RoomExt, Vec2, Water};

/// `Custom.fourDirections`.
pub(crate) const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// `Custom.eightDirections`.
pub(crate) const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// `CosmeticInsect.Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsectType {
    StandardFly,
    FireFly,
    TinyDragonFly,
    RockFlea,
    GrassHopper,
    RedSwarmer,
    Ant,
    Beetle,
    WaterGlowworm,
    Wasp,
    Moth,
}

impl InsectType {
    pub const ALL: [Self; 11] = [
        Self::StandardFly,
        Self::FireFly,
        Self::TinyDragonFly,
        Self::RockFlea,
        Self::GrassHopper,
        Self::RedSwarmer,
        Self::Ant,
        Self::Beetle,
        Self::WaterGlowworm,
        Self::Wasp,
        Self::Moth,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::StandardFly => "StandardFly",
            Self::FireFly => "FireFly",
            Self::TinyDragonFly => "TinyDragonFly",
            Self::RockFlea => "RockFlea",
            Self::GrassHopper => "GrassHopper",
            Self::RedSwarmer => "RedSwarmer",
            Self::Ant => "Ant",
            Self::Beetle => "Beetle",
            Self::WaterGlowworm => "WaterGlowworm",
            Self::Wasp => "Wasp",
            Self::Moth => "Moth",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.value() == value)
    }

    /// `InsectCoordinator.RoomEffectToInsectType` for the ten insect effects.
    #[must_use]
    pub fn from_effect(effect: RoomEffectType) -> Option<Self> {
        Some(match effect {
            RoomEffectType::Flies => Self::StandardFly,
            RoomEffectType::FireFlies => Self::FireFly,
            RoomEffectType::TinyDragonFly => Self::TinyDragonFly,
            RoomEffectType::RockFlea => Self::RockFlea,
            RoomEffectType::RedSwarmer => Self::RedSwarmer,
            RoomEffectType::Ant => Self::Ant,
            RoomEffectType::Beetle => Self::Beetle,
            RoomEffectType::WaterGlowworm => Self::WaterGlowworm,
            RoomEffectType::Wasp => Self::Wasp,
            RoomEffectType::Moth => Self::Moth,
            _ => return None,
        })
    }

    /// `InsectCoordinator.SpeciesDensity`.
    #[must_use]
    pub fn species_density(self) -> f32 {
        match self {
            Self::FireFly => 0.8,
            Self::Ant => 6.0,
            Self::WaterGlowworm => 0.33,
            Self::Wasp => 0.15,
            _ => 1.0,
        }
    }
}

/// What an insect of a swarm reads of its `InsectGroup` (`mySwarm`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwarmArea {
    /// `placedObject.pos`.
    pub center: Vec2,
    /// `insectGroupData.Rad`.
    pub rad: f32,
}

/// A realized room's water as the insects read it: the surface once `Water` exists,
/// else the still level of the room file's water line (`Room.Loaded`'s `AddWater` has
/// made the surface in the game by the time anything reads it; the port makes it when
/// the room finishes loading).
#[derive(Debug, Clone, Copy, Default)]
pub struct InsectWater<'a> {
    pub surface: Option<&'a Water>,
    /// `20 * defaultWaterLevel + 10`.
    pub still: Option<f32>,
    /// `defaultWaterLevel` (the water's tile row).
    pub row: Option<i32>,
}

impl<'a> InsectWater<'a> {
    #[must_use]
    pub fn new(surface: Option<&'a Water>, room: &Room) -> Self {
        let row = room.water_level();
        Self {
            surface,
            still: row.map(|r| r as f32 * 20.0 + 10.0),
            row,
        }
    }

    /// `room.water`.
    #[must_use]
    pub fn present(&self) -> bool {
        self.row.is_some()
    }

    /// `Room.FloatWaterLevel(pos)`; `float.MinValue` without water.
    #[must_use]
    pub fn level(&self, pos: Vec2) -> f32 {
        match (self.surface, self.still) {
            (Some(w), _) => w.level_at(pos.x),
            (None, Some(l)) => l,
            (None, None) => f32::MIN,
        }
    }

    /// `Room.PointSubmerged(pos)`.
    #[must_use]
    pub fn submerged(&self, pos: Vec2) -> bool {
        self.present() && pos.y < self.level(pos)
    }
}

/// What an insect's update reads of its room and the world.
pub struct InsectCx<'a> {
    pub room: &'a Room,
    /// `aimap` once `readyForAI`.
    pub ai_map: Option<&'a AiMap>,
    pub water: InsectWater<'a>,
    /// `rainCycle.TimeUntilRain`.
    pub time_until_rain: i32,
    /// `rainCycle.CycleStartUp`.
    pub cycle_start_up: f32,
    /// `room.BeingViewed`.
    pub viewed: bool,
    /// `Room.Darkness(pos)`: camera 0's palette darkness while it views the room, else 0.
    pub darkness: f32,
    /// `room.world.region.name == "SI"`.
    pub region_si: bool,
    /// `room.abstractRoom.creatures`, realized ones with their bodies.
    pub creatures: &'a [InsectCreatureSlot],
    pub rng: &'a mut UnityRandom,
}

/// An entry of `room.abstractRoom.creatures`.
#[derive(Debug, Clone, PartialEq)]
pub struct InsectCreatureSlot {
    pub id: CreatureId,
    /// `realizedCreature`.
    pub realized: Option<InsectCreature>,
}

impl InsectCx<'_> {
    /// `room.readyForAI`.
    #[must_use]
    pub fn ready_for_ai(&self) -> bool {
        self.ai_map.is_some()
    }

    /// `Custom.RNV()`.
    pub fn rnv(&mut self) -> Vec2 {
        Vec2::random_unit(self.rng)
    }

    pub fn value(&mut self) -> f32 {
        self.rng.value()
    }

    /// `room.GetTile(pos).Solid`.
    #[must_use]
    pub fn solid_at(&self, pos: Vec2) -> bool {
        self.room.is_solid_at(pos)
    }

    #[must_use]
    pub fn solid_tile(&self, (x, y): (i32, i32)) -> bool {
        self.room.is_solid_tile(x, y)
    }

    /// `room.IsPositionInsideBoundries(tile)`.
    #[must_use]
    pub fn inside(&self, (x, y): (i32, i32)) -> bool {
        x >= 0 && x < self.room.width() && y >= 0 && y < self.room.height()
    }

    /// `room.aimap.getTerrainProximity(tile)` (the caller checked `readyForAI`).
    #[must_use]
    pub fn terrain_proximity(&self, (x, y): (i32, i32)) -> i32 {
        self.ai_map.map_or(0, |m| m.terrain_proximity(x, y))
    }

    /// The realized creature `id`, as the room lists it now.
    #[must_use]
    pub fn creature(&self, id: CreatureId) -> Option<&InsectCreature> {
        self.creatures
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.realized.as_ref())
    }
}

/// The fields `CosmeticInsect` (and `CosmeticSprite`) add to every kind.
#[derive(Debug, Clone, PartialEq)]
pub struct InsectBase {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub terrain_collision: bool,
    pub alive: bool,
    pub in_ground: f32,
    pub last_in_ground: f32,
    pub want_to_burrow: bool,
    pub burrow_pos: Option<Vec2>,
    last_wall_collide: bool,
    pub avoider: Option<CreatureAvoider>,
    get_away_from_rain_time: i32,
    emerge_after_rain_time: f32,
    last_non_solid_pos: Vec2,
    pub submerged: bool,
    /// `mySwarm`.
    pub swarm: Option<SwarmArea>,
    /// `slatedForDeletetion`.
    pub slated: bool,
}

impl InsectBase {
    /// The `CosmeticInsect` constructor: three draws, then, early in the cycle, the
    /// burrow below `pos` it starts in (no `room.terrain` in the base game).
    pub fn new(pos: Vec2, cx: &mut InsectCx) -> Self {
        let get_away_from_rain_time = cx.rng.range_i32(400, 1600);
        let emerge_after_rain_time = 0.1.lerp(0.6, cx.rng.clamped_random_variation(0.5, 0.5, 0.1));
        let mut b = Self {
            pos,
            last_pos: pos,
            vel: Vec2::ZERO,
            terrain_collision: true,
            alive: true,
            in_ground: 0.0,
            last_in_ground: 0.0,
            want_to_burrow: false,
            burrow_pos: None,
            last_wall_collide: false,
            avoider: None,
            get_away_from_rain_time,
            emerge_after_rain_time,
            last_non_solid_pos: pos,
            submerged: false,
            swarm: None,
            slated: false,
        };
        if cx.cycle_start_up < emerge_after_rain_time {
            let (x, mut y) = pos.tile();
            while y > 0 && !cx.solid_tile((x, y)) {
                y -= 1;
            }
            b.last_in_ground = 1.0;
            b.in_ground = 1.0;
            b.burrow_pos = Some(Vec2::middle_of_tile((x, y)) + Vec2::new(0.0, 9.0));
            b.want_to_burrow = true;
        }
        b
    }

    /// `OutOfBounds`: farther from its swarm's centre than the swarm's radius.
    #[must_use]
    pub fn out_of_bounds(&self) -> bool {
        self.swarm
            .is_some_and(|s| !self.pos.dist_less(s.center, s.rad))
    }

    /// `CosmeticInsect.Reset` (two draws).
    pub fn reset(&mut self, pos: Vec2, rng: &mut UnityRandom) {
        self.burrow_pos = None;
        self.pos = pos;
        self.last_pos = pos;
        self.last_non_solid_pos = pos;
        let dir = Vec2::random_unit(rng);
        self.vel = dir * rng.value();
    }

    /// The base `Act`: the creature avoider's update.
    pub fn act(&mut self, cx: &mut InsectCx) {
        let pos = self.pos;
        if let Some(a) = &mut self.avoider {
            a.update(pos, cx);
        }
    }

    /// `creatureAvoider.FleeSpeed` (0 without an avoider).
    #[must_use]
    pub fn flee_speed(&self) -> f32 {
        self.avoider
            .as_ref()
            .map_or(0.0, |a| a.flee_speed(self.pos))
    }

    /// `creatureAvoider.currentWorstCrit.DangerPos`, while there is one.
    #[must_use]
    pub fn worst_danger_pos(&self) -> Option<Vec2> {
        self.avoider
            .as_ref()
            .and_then(|a| a.current_worst.as_ref())
            .map(|c| c.danger_pos)
    }

    /// `Destroy()`.
    pub fn destroy(&mut self) {
        self.slated = true;
    }
}

/// What each kind overrides of `CosmeticInsect`'s virtual members.
pub(crate) trait InsectBehavior {
    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
    }

    fn reset(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.reset(pos, cx.rng);
    }

    fn wall_collision(
        &mut self,
        _b: &mut InsectBase,
        _dir: (i32, i32),
        _first: bool,
        _cx: &mut InsectCx,
    ) {
    }

    fn emerge_from_ground(&mut self, _b: &mut InsectBase, _pos: Vec2, _cx: &mut InsectCx) {}

    /// `CosmeticSprite.Update` then `CosmeticInsect.Update`: burrowing in and out, the
    /// point's terrain collision, the rain timers' check (one draw every tick), `Act`
    /// while alive, and destruction once the room is no longer viewed.
    fn base_update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.last_pos = b.pos;
        b.pos += b.vel;
        b.last_in_ground = b.in_ground;
        b.submerged = cx.water.submerged(b.pos);
        if let Some(burrow) = b.burrow_pos {
            b.pos = burrow;
            b.vel *= 0.0;
            if b.want_to_burrow {
                b.in_ground = (b.in_ground + 1.0 / 30.0).min(1.0);
                if b.in_ground == 1.0 {
                    b.destroy();
                }
            } else {
                b.in_ground = (b.in_ground - 1.0 / 30.0).max(0.0);
                if b.in_ground == 0.0 {
                    self.emerge_from_ground(b, burrow, cx);
                    b.burrow_pos = None;
                }
            }
        } else {
            if b.terrain_collision {
                let cd =
                    TerrainCollisionData::new(b.pos, b.last_pos, b.vel, 1.0, true).collide(cx.room);
                b.pos = cd.pos;
                b.vel = cd.vel;
                if cd.contact_point != (0, 0) {
                    let first = !b.last_wall_collide;
                    self.wall_collision(b, cd.contact_point, first, cx);
                    b.last_wall_collide = true;
                } else {
                    b.last_wall_collide = false;
                }
                if !b.alive && cd.contact_point.1 < 0 {
                    b.destroy();
                }
                if b.want_to_burrow && cd.contact_point.1 < 0 {
                    b.burrow_pos = Some(cd.pos + Vec2::new(0.0, -cd.rad - 2.0));
                }
            } else if b.want_to_burrow && cx.solid_at(b.pos) {
                b.burrow_pos = Some(b.pos + Vec2::new(0.0, -2.0));
            }
            if b.want_to_burrow && b.pos.y < -100.0 {
                b.destroy();
            }
            if cx.solid_at(b.pos) {
                let to = b.last_non_solid_pos;
                self.reset(b, to, cx);
            } else {
                b.last_non_solid_pos = b.pos;
            }
        }
        if cx.rng.value() < 0.025 {
            if !b.want_to_burrow && cx.time_until_rain < b.get_away_from_rain_time {
                b.want_to_burrow = true;
            } else if b.want_to_burrow && cx.cycle_start_up > b.emerge_after_rain_time {
                b.want_to_burrow = false;
            }
        }
        if b.alive {
            self.act(b, cx);
        }
        if !cx.viewed {
            b.destroy();
        }
    }
}

/// A cosmetic insect in a realized room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct CosmeticInsect {
    /// Its place in the room's update list (also its id).
    pub update_order: u64,
    pub base: InsectBase,
    pub body: InsectBody,
}

/// The kind-specific part of an insect.
#[derive(Debug, Clone, PartialEq)]
pub enum InsectBody {
    MiniFly(MiniFly),
    FireFly(FireFly),
    TinyDragonfly(TinyDragonfly),
    RockFlea(RockFlea),
    GrassHopper(GrassHopper),
    RedSwarmer(RedSwarmer),
    Ant(Ant),
    Beetle(Beetle),
    WaterGlowworm(WaterGlowworm),
    Wasp(Wasp),
    Moth(Moth),
}

impl CosmeticInsect {
    /// The kind's constructor after the base's, at `pos`.
    pub fn new(kind: InsectType, pos: Vec2, cx: &mut InsectCx) -> Self {
        let mut base = InsectBase::new(pos, cx);
        let body = match kind {
            InsectType::StandardFly => InsectBody::MiniFly(MiniFly::new(pos)),
            InsectType::FireFly => InsectBody::FireFly(FireFly::new(pos, cx)),
            InsectType::TinyDragonFly => {
                InsectBody::TinyDragonfly(TinyDragonfly::new(&mut base, pos, cx))
            }
            InsectType::RockFlea => InsectBody::RockFlea(RockFlea::new(&mut base, cx)),
            InsectType::GrassHopper => InsectBody::GrassHopper(GrassHopper::new(&mut base, cx)),
            InsectType::RedSwarmer => InsectBody::RedSwarmer(RedSwarmer::new(&mut base, pos, cx)),
            InsectType::Ant => InsectBody::Ant(Ant::new(pos, cx)),
            InsectType::Beetle => InsectBody::Beetle(Beetle::new(&mut base, pos, cx)),
            InsectType::WaterGlowworm => {
                InsectBody::WaterGlowworm(WaterGlowworm::new(&mut base, pos, cx))
            }
            InsectType::Wasp => InsectBody::Wasp(Wasp::new(cx)),
            InsectType::Moth => InsectBody::Moth(Moth::new(&mut base, pos, cx)),
        };
        Self {
            update_order: 0,
            base,
            body,
        }
    }

    #[must_use]
    pub fn kind(&self) -> InsectType {
        match self.body {
            InsectBody::MiniFly(_) => InsectType::StandardFly,
            InsectBody::FireFly(_) => InsectType::FireFly,
            InsectBody::TinyDragonfly(_) => InsectType::TinyDragonFly,
            InsectBody::RockFlea(_) => InsectType::RockFlea,
            InsectBody::GrassHopper(_) => InsectType::GrassHopper,
            InsectBody::RedSwarmer(_) => InsectType::RedSwarmer,
            InsectBody::Ant(_) => InsectType::Ant,
            InsectBody::Beetle(_) => InsectType::Beetle,
            InsectBody::WaterGlowworm(_) => InsectType::WaterGlowworm,
            InsectBody::Wasp(_) => InsectType::Wasp,
            InsectBody::Moth(_) => InsectType::Moth,
        }
    }

    /// Its `Update` (each kind wraps the base update in its own).
    pub fn update(&mut self, cx: &mut InsectCx) {
        if let Some(a) = &mut self.base.avoider {
            a.refresh(cx.creatures);
        }
        let b = &mut self.base;
        match &mut self.body {
            InsectBody::MiniFly(k) => k.update(b, cx),
            InsectBody::FireFly(k) => k.update(b, cx),
            InsectBody::TinyDragonfly(k) => k.update(b, cx),
            InsectBody::RockFlea(k) => k.update(b, cx),
            InsectBody::GrassHopper(k) => k.update(b, cx),
            InsectBody::RedSwarmer(k) => k.update(b, cx),
            InsectBody::Ant(k) => k.update(b, cx),
            InsectBody::Beetle(k) => k.update(b, cx),
            InsectBody::WaterGlowworm(k) => k.update(b, cx),
            InsectBody::Wasp(k) => k.update(b, cx),
            InsectBody::Moth(k) => k.update(b, cx),
        }
    }
}

/// Unity's `Vector3.RotateTowards` of two in-plane vectors, back to 2D.
pub(crate) trait RotateTowards {
    fn rotate_towards(self, target: Self, max_radians: f32, max_magnitude: f32) -> Self;
}

impl RotateTowards for Vec2 {
    fn rotate_towards(self, target: Vec2, max_radians: f32, max_magnitude: f32) -> Vec2 {
        const EPS: f32 = 1e-5;
        let (from_mag, to_mag) = (self.length(), target.length());
        let clamped_mag = {
            let delta = to_mag - from_mag;
            if delta > 0.0 {
                from_mag + delta.min(max_magnitude)
            } else {
                from_mag - (-delta).min(max_magnitude)
            }
        };
        if from_mag > EPS && to_mag > EPS {
            let (from, to) = (self * (1.0 / from_mag), target * (1.0 / to_mag));
            let dot = from.dot(to);
            if dot > 1.0 - EPS {
                return self.move_towards(target, max_magnitude);
            }
            if dot < -1.0 + EPS {
                // Turned about an in-plane axis: the vector leaves the plane.
                return from * (max_radians.cos() * clamped_mag);
            }
            let angle = dot.acos().min(max_radians);
            let cross = from.x * to.y - from.y * to.x;
            let a = if cross >= 0.0 { angle } else { -angle };
            let (s, c) = a.sin_cos();
            return Vec2::new(from.x * c - from.y * s, from.x * s + from.y * c) * clamped_mag;
        }
        self.move_towards(target, max_magnitude)
    }
}
