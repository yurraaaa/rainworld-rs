//! What `get_state` reports of the realized rooms' depth-faded particles: sky dandelions
//! with their stalks and spawners, green sparks with theirs, zero-gravity specks and
//! corruption spores.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{RoomParticles, SpeckKind};
use serde::Serialize;

/// A room's particles: counts per kind and the spawners' state.
#[derive(Debug, Clone, Serialize)]
pub struct RoomParticlesState {
    pub room: String,
    /// `Room.gravity`.
    pub gravity: f32,
    /// Dandelions alive, and how many of them sit in the play layer.
    pub dandelions: usize,
    pub dandelions_in_play_layer: usize,
    /// Dandelions still held by a stalk, and stalks that let go.
    pub stalks_attached: usize,
    pub stalks_loose: usize,
    /// `SkyDandelions`: `[amount, wait]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dandelion_spawners: Vec<[f32; 2]>,
    pub sparks: usize,
    pub sparks_in_play_layer: usize,
    /// Sparks with a `LightSource`.
    pub sparks_lit: usize,
    /// `GreenSparks`: its cap, the sparks it owns and its wind.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spark_spawners: Vec<SparkSpawnerState>,
    pub specks: usize,
    pub spores: usize,
    /// Specks and spores waiting to be placed again (`reset`).
    pub specks_reset: usize,
    /// The first speck or spore: position, velocity and `saved_cam_pos`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_speck: Option<([f32; 2], [f32; 2], i32)>,
}

/// One `GreenSparks` spawner.
#[derive(Debug, Clone, Serialize)]
pub struct SparkSpawnerState {
    pub tot_sparks: usize,
    pub owned: usize,
    pub wind: [f32; 2],
}

impl RoomParticlesState {
    /// Every realized room that has particles.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| {
                let r = world.realized(i)?;
                let p = &r.particles;
                let any = !(p.dandelions.is_empty()
                    && p.stalks.is_empty()
                    && p.dandelion_spawners.is_empty()
                    && p.sparks.is_empty()
                    && p.spark_spawners.is_empty()
                    && p.specks.is_empty());
                let room = world
                    .abstract_room(i)
                    .map_or_else(|| format!("#{i}"), |a| a.name.clone());
                any.then(|| Self::of(room, r.gravity, p))
            })
            .collect()
    }

    fn of(room: String, gravity: f32, p: &RoomParticles) -> Self {
        Self {
            room,
            gravity,
            dandelions: p.dandelions.iter().filter(|d| !d.slated).count(),
            dandelions_in_play_layer: p
                .dandelions
                .iter()
                .filter(|d| !d.slated && d.in_play_layer())
                .count(),
            stalks_attached: p.stalks.iter().filter(|s| s.dandelion.is_some()).count(),
            stalks_loose: p.stalks.iter().filter(|s| s.dandelion.is_none()).count(),
            dandelion_spawners: p
                .dandelion_spawners
                .iter()
                .map(|s| [s.amount, s.wait])
                .collect(),
            sparks: p.sparks.iter().filter(|s| !s.slated).count(),
            sparks_in_play_layer: p
                .sparks
                .iter()
                .filter(|s| !s.slated && s.in_play_layer())
                .count(),
            sparks_lit: p.sparks.iter().filter(|s| s.light.is_some()).count(),
            spark_spawners: p
                .spark_spawners
                .iter()
                .map(|s| SparkSpawnerState {
                    tot_sparks: s.tot_sparks,
                    owned: s.sparks.len(),
                    wind: s.wind.to_array(),
                })
                .collect(),
            specks: p
                .specks
                .iter()
                .filter(|s| matches!(s.kind, SpeckKind::Blink(_)))
                .count(),
            spores: p
                .specks
                .iter()
                .filter(|s| matches!(s.kind, SpeckKind::CorruptionSpore(_)))
                .count(),
            specks_reset: p.specks.iter().filter(|s| s.reset).count(),
            first_speck: p
                .specks
                .first()
                .map(|s| (s.pos.to_array(), s.vel.to_array(), s.saved_cam_pos)),
        }
    }
}
