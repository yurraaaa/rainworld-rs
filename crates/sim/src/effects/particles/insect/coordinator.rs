//! `InsectCoordinator`: the room object `Room.Loaded` adds for the insect effects
//! (`Flies`, `FireFlies`, `TinyDragonFly`, `RockFlea`, `RedSwarmer`, `Ant`, `Beetle`,
//! `WaterGlowworm`, `Wasp`, `Moth`) and the `InsectGroup` placed objects. Each time the
//! room becomes viewed it rolls the insects anew: per effect a number of random spots
//! scaled by the room's area, per group spots around the group's centre; each kept spot
//! makes one insect unless the rain is near. `NoLongerViewed` destroys them all.
//!
//! Mirrors: InsectCoordinator.cs, PlacedObject.cs (InsectGroupData), Room.cs (Loaded, NowViewed, NoLongerViewed: the insect branches)

use rw_assets::{
    ActiveMods, PlacedObject, PlacedObjectType, RoomEffectType, RoomSettings, Timeline,
};

use super::{CosmeticInsect, InsectCx, InsectType, SwarmArea};
use crate::physics::shared_physics::ExactRayTrace;
use crate::{FloatExt, Vec2};

/// `PlacedObject.InsectGroupData` with its object's position (`Swarm`).
#[derive(Debug, Clone, PartialEq)]
pub struct InsectGroup {
    /// `placedObject.pos`.
    pub center: Vec2,
    /// `Rad`: the length of `handlePos`.
    pub rad: f32,
    /// `insectType`; `None` for a name the game registers no insect for (it still
    /// makes the coordinator's draws, but no insect).
    pub kind: Option<InsectType>,
    pub density: f32,
}

impl InsectGroup {
    /// `InsectGroupData.FromString` (`hx~hy~px~py~Type~density`); the defaults
    /// (radius 100, `RockFlea`, 0.5) for fields missing.
    #[must_use]
    pub fn from_placed(o: &PlacedObject) -> Self {
        let parts: Vec<&str> = o.data.split('~').collect();
        let handle = match (Self::field(&parts, 0), Self::field(&parts, 1)) {
            (Some(x), Some(y)) => Vec2::new(x, y),
            _ => Vec2::new(0.0, 100.0),
        };
        let kind = match parts.get(4) {
            Some(name) => InsectType::from_value(name),
            None => Some(InsectType::RockFlea),
        };
        Self {
            center: Vec2::new(o.pos[0], o.pos[1]),
            rad: handle.length(),
            kind,
            density: Self::field(&parts, 5).unwrap_or(0.5),
        }
    }

    fn field(parts: &[&str], i: usize) -> Option<f32> {
        parts.get(i).and_then(|s| s.trim().parse().ok())
    }

    fn area(&self) -> SwarmArea {
        SwarmArea {
            center: self.center,
            rad: self.rad,
        }
    }
}

/// `InsectCoordinator` of a realized room.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InsectCoordinator {
    /// `effects`: the insect effects in the room's effect order, with their amounts.
    pub effects: Vec<(InsectType, f32)>,
    /// `swarms`.
    pub groups: Vec<InsectGroup>,
    /// `allInsects`, in the order they were made.
    pub insects: Vec<CosmeticInsect>,
}

impl InsectCoordinator {
    /// `Room.Loaded`'s insect branches: one coordinator when the room has an insect
    /// effect or an active `InsectGroup`, else `None`.
    #[must_use]
    pub fn for_room(settings: &RoomSettings, timeline: Timeline, mods: ActiveMods) -> Option<Self> {
        let effects: Vec<(InsectType, f32)> = settings
            .effects
            .iter()
            .filter_map(|e| {
                let kind = RoomEffectType::from_value(&e.kind, mods)?;
                Some((InsectType::from_effect(kind)?, e.amount))
            })
            .collect();
        let groups: Vec<InsectGroup> = settings
            .placed_objects_for(timeline, mods)
            .iter()
            .filter(|o| {
                o.active
                    && PlacedObjectType::from_value(&o.kind, mods)
                        == Some(PlacedObjectType::InsectGroup)
            })
            .map(InsectGroup::from_placed)
            .collect();
        (!effects.is_empty() || !groups.is_empty()).then_some(Self {
            effects,
            groups,
            insects: Vec::new(),
        })
    }

    /// `NowViewed`: the effects' insects, then the groups'. Returns how many insects it
    /// made (appended to [`Self::insects`]).
    pub fn now_viewed(&mut self, cx: &mut InsectCx) -> usize {
        let before = self.insects.len();
        let (w, h) = (cx.room.width(), cx.room.height());
        for &(kind, amount) in &self.effects.clone() {
            let n = ((w * h) as f32 * amount * kind.species_density() / 50.0).to_int_cs();
            for _ in 0..=n.max(-1) {
                let x = cx.value() * cx.room.pixel_width();
                let pos = Vec2::new(x, cx.value() * cx.room.pixel_height());
                if !cx.solid_at(pos) && Self::effect_spawn_chance(kind, pos, cx) {
                    self.create_insect(Some(kind), pos, None, cx);
                }
            }
        }
        for g in &self.groups.clone() {
            let around = std::f32::consts::PI * g.rad.lerp(150.0, 0.5) * 2.0;
            let density = g.kind.map_or(1.0, InsectType::species_density) * g.density;
            let n = 1 + (around * density / 50.0).to_int_cs();
            for _ in 0..=n.max(-1) {
                let dir = cx.rnv();
                let reach = g.rad * cx.value() + g.rad * 0.1 * cx.value().mathf_pow(3.0);
                let pos = g.center + dir * reach;
                if !cx.solid_at(pos) {
                    self.create_insect(g.kind, pos, Some(g.area()), cx);
                }
            }
        }
        self.insects.len() - before
    }

    /// `NoLongerViewed`: every insect destroyed and forgotten.
    pub fn no_longer_viewed(&mut self) {
        self.insects.clear();
    }

    /// `EffectSpawnChanceForInsect` at `effectAmount` 0.5 (one draw for the flying
    /// kinds; the beetle's narrow-space test draws none).
    fn effect_spawn_chance(kind: InsectType, pos: Vec2, cx: &mut InsectCx) -> bool {
        match kind {
            InsectType::StandardFly
            | InsectType::FireFly
            | InsectType::TinyDragonFly
            | InsectType::RedSwarmer
            | InsectType::Moth => {
                let proximity = match cx.ai_map {
                    Some(m) => {
                        let (x, y) = pos.tile();
                        m.terrain_proximity(x, y)
                    }
                    None => 5,
                };
                cx.value().mathf_pow(0.5) > proximity as f32 * 0.05
            }
            InsectType::Beetle => cx.ai_map.is_none_or(|m| {
                let (x, y) = pos.tile();
                !m.narrow_space(x, y)
            }),
            _ => true,
        }
    }

    /// `TileLegalForInsect`: out of water for the air kinds, in deep water for the
    /// glowworm, anywhere for the flea (and a name with no insect).
    fn tile_legal(kind: Option<InsectType>, pos: Vec2, cx: &InsectCx) -> bool {
        let (x, y) = pos.tile();
        // `GetTile` clamps to the room's edge tiles.
        let water = cx.room.water_int(
            x.clamp(0, cx.room.width() - 1),
            y.clamp(0, cx.room.height() - 1),
        );
        match kind {
            Some(InsectType::WaterGlowworm) => water == 1,
            Some(InsectType::RockFlea) | None => true,
            Some(_) => water == 0,
        }
    }

    /// `BringPosToGround`: just above the first Solid tile below `pos`, or below the room.
    fn bring_pos_to_ground(pos: Vec2, cx: &InsectCx) -> Vec2 {
        let (x, ty) = pos.tile();
        let mut y = ty - 1;
        while y >= 0 {
            if cx.solid_tile((x, y)) {
                return Vec2::new(pos.x, Vec2::middle_of_tile((x, y)).y + 10.0);
            }
            y -= 1;
        }
        Vec2::new(pos.x, -10.0)
    }

    /// Up to five tries at a random wall within `reach` (`ExactTerrainRayTracePos` along
    /// `RNV() * reach`, one draw a try).
    fn wall_within(pos: Vec2, reach: f32, cx: &mut InsectCx) -> Option<Vec2> {
        for _ in 0..5 {
            let to = pos + cx.rnv() * reach;
            if let Some(hit) = cx.room.exact_terrain_ray_trace_pos(pos, to) {
                return Some(hit);
            }
        }
        None
    }

    /// `!room.readyForAI || room.aimap.getTerrainProximity(pos) < limit`.
    fn near_terrain(pos: Vec2, limit: i32, cx: &InsectCx) -> bool {
        cx.ai_map.is_none_or(|m| {
            let (x, y) = pos.tile();
            m.terrain_proximity(x, y) < limit
        })
    }

    /// `CreateInsect`.
    fn create_insect(
        &mut self,
        kind: Option<InsectType>,
        mut pos: Vec2,
        swarm: Option<SwarmArea>,
        cx: &mut InsectCx,
    ) {
        if !Self::tile_legal(kind, pos, cx) || cx.time_until_rain < cx.rng.range_i32(1200, 1600) {
            return;
        }
        let Some(kind) = kind else {
            return;
        };
        match kind {
            InsectType::RockFlea => {
                if !cx.water.submerged(pos) {
                    pos = Self::bring_pos_to_ground(pos, cx);
                }
            }
            InsectType::GrassHopper => pos = Self::bring_pos_to_ground(pos, cx),
            InsectType::Beetle => {
                if cx.value() > 1.0 / 7.0 {
                    let wall = if Self::near_terrain(pos, 10, cx) {
                        Self::wall_within(pos, 200.0, cx)
                    } else {
                        None
                    };
                    pos = wall.unwrap_or_else(|| Self::bring_pos_to_ground(pos, cx));
                }
            }
            InsectType::Moth => {
                if Self::near_terrain(pos, 5, cx)
                    && let Some(wall) = Self::wall_within(pos, 100.0, cx)
                {
                    pos = wall;
                }
            }
            _ => {}
        }
        let mut insect = CosmeticInsect::new(kind, pos, cx);
        insect.base.swarm = swarm;
        self.insects.push(insect);
    }
}
