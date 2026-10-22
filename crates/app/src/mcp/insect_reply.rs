//! What `get_state` reports of the realized rooms' cosmetic insects.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::insect::{CosmeticInsect, InsectBody, InsectCoordinator, InsectType};
use rw_sim::world::World;
use serde::Serialize;

/// A realized room's `InsectCoordinator`.
#[derive(Debug, Clone, Serialize)]
pub struct RoomInsects {
    pub room: String,
    /// The insect effects and their amounts, in the room's order.
    pub effects: Vec<(String, f32)>,
    /// The `InsectGroup`s: kind, centre, radius, density.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<InsectGroupState>,
    /// How many insects of each kind live now.
    pub counts: Vec<(String, usize)>,
    pub insects: Vec<InsectState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsectGroupState {
    /// `None` for a name the game registers no insect for.
    pub kind: Option<String>,
    pub center: [f32; 2],
    pub rad: f32,
    pub density: f32,
}

/// One insect.
#[derive(Debug, Clone, Serialize)]
pub struct InsectState {
    pub kind: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    /// `inGround` (1 buried).
    pub in_ground: f32,
    pub want_to_burrow: bool,
    pub in_swarm: bool,
    /// Sitting on a wall (fleas, hoppers, beetles, wasps, moths).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sit_pos: Option<[f32; 2]>,
    /// A firefly's `LightSource` id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<u64>,
}

impl RoomInsects {
    /// Every realized room with a coordinator.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| {
                let c = world.realized(i)?.insects.as_ref()?;
                let room = world
                    .abstract_room(i)
                    .map_or_else(|| format!("#{i}"), |r| r.name.clone());
                Some(Self::of(room, c))
            })
            .collect()
    }

    fn of(room: String, c: &InsectCoordinator) -> Self {
        let live: Vec<&CosmeticInsect> = c.insects.iter().filter(|i| !i.base.slated).collect();
        let counts = InsectType::ALL
            .into_iter()
            .map(|k| {
                (
                    k.value().to_owned(),
                    live.iter().filter(|i| i.kind() == k).count(),
                )
            })
            .filter(|&(_, n)| n > 0)
            .collect();
        Self {
            room,
            effects: c
                .effects
                .iter()
                .map(|(k, a)| (k.value().to_owned(), *a))
                .collect(),
            groups: c
                .groups
                .iter()
                .map(|g| InsectGroupState {
                    kind: g.kind.map(|k| k.value().to_owned()),
                    center: g.center.to_array(),
                    rad: g.rad,
                    density: g.density,
                })
                .collect(),
            counts,
            insects: live.into_iter().map(InsectState::of).collect(),
        }
    }
}

impl InsectState {
    fn of(i: &CosmeticInsect) -> Self {
        let sit = match &i.body {
            InsectBody::RockFlea(k) => k.sit_pos,
            InsectBody::GrassHopper(k) => k.sit_pos,
            InsectBody::Beetle(k) => k.sit_pos,
            InsectBody::Wasp(k) => k.sit_pos,
            InsectBody::Moth(k) => k.sit_pos,
            _ => None,
        };
        let light = match &i.body {
            InsectBody::FireFly(f) => f.light,
            _ => None,
        };
        Self {
            kind: i.kind().value().to_owned(),
            id: i.update_order,
            pos: i.base.pos.to_array(),
            vel: i.base.vel.to_array(),
            in_ground: i.base.in_ground,
            want_to_burrow: i.base.want_to_burrow,
            in_swarm: i.base.swarm.is_some(),
            sit_pos: sit.map(rw_sim::Vec2::to_array),
            light,
        }
    }
}
