//! What `get_state` reports of the realized rooms' `DeepProcessing` quads, their
//! `DeepProcessingLight`s and the `GravityDisruptor`s (with their `DisruptorSpeck`s).
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{DeepProcessing, DeepProcessingLight, DisruptorSpeck, GravityDisruptor, LightSource};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One `DeepProcessing`.
#[derive(Debug, Clone, Serialize)]
pub struct DeepProcessingState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub quad: [[f32; 2]; 4],
    pub from_depth: f32,
    pub to_depth: f32,
    pub intensity: f32,
    pub power: f32,
}

/// One `DeepProcessingLight`.
#[derive(Debug, Clone, Serialize)]
pub struct DeepProcessingLightState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub random_seed: i32,
    pub light: u64,
    /// Its `LightSource`'s rad and alpha now.
    pub rad: f32,
    pub alpha: f32,
}

/// One `DisruptorSpeck`.
#[derive(Debug, Clone, Serialize)]
pub struct DisruptorSpeckState {
    pub id: u64,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub orbit_rad: f32,
    pub new_orbit_rad: f32,
}

/// One `GravityDisruptor`.
#[derive(Debug, Clone, Serialize)]
pub struct GravityDisruptorState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub depth: i32,
    pub power: f32,
    pub last_power: f32,
    pub debug_mode: i32,
    pub point_dir: [f32; 2],
    pub dir_fac: f32,
    /// The sixteen streaks' brightness now.
    pub streaks: Vec<f32>,
    /// How many zero-gravity specks of the room the swirl moves (`roomSpecks`).
    pub room_specks: usize,
    pub specks: Vec<DisruptorSpeckState>,
}

impl DeepProcessingState {
    /// Every realized room's quads.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .deep_processing
                    .iter()
                    .map(move |d| Self::of(room.clone(), d))
            })
            .collect()
    }

    fn of(room: String, d: &DeepProcessing) -> Self {
        Self {
            room,
            id: d.update_order,
            quad: d.quad.map(rw_sim::Vec2::to_array),
            from_depth: d.data.from_depth,
            to_depth: d.data.to_depth,
            intensity: d.data.intensity,
            power: d.power,
        }
    }
}

impl DeepProcessingLightState {
    /// Every realized room's lights.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .deep_processing_lights
                    .iter()
                    .map(move |l| Self::of(room.clone(), l, &r.light_sources))
            })
            .collect()
    }

    fn of(room: String, l: &DeepProcessingLight, lights: &[LightSource]) -> Self {
        let source = lights.iter().find(|s| s.id == l.light);
        Self {
            room,
            id: l.update_order,
            pos: l.pos.to_array(),
            random_seed: l.random_seed,
            light: l.light,
            rad: source.map_or(0.0, |s| s.rad),
            alpha: source.map_or(0.0, |s| s.alpha),
        }
    }
}

impl GravityDisruptorState {
    /// Every realized room's disruptors.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures.gravity_disruptors.iter().map(move |d| {
                    let specks = r
                        .fixtures
                        .disruptor_specks
                        .iter()
                        .filter(|s| s.disruptor == d.update_order);
                    Self::of(room.clone(), d, specks)
                })
            })
            .collect()
    }

    fn of<'a>(
        room: String,
        d: &GravityDisruptor,
        specks: impl Iterator<Item = &'a DisruptorSpeck>,
    ) -> Self {
        Self {
            room,
            id: d.update_order,
            pos: d.pos.to_array(),
            depth: d.depth,
            power: d.power,
            last_power: d.last_power,
            debug_mode: d.debug_mode,
            point_dir: d.point_dir.to_array(),
            dir_fac: d.dir_fac,
            streaks: d.lights.iter().map(|l| l[0]).collect(),
            room_specks: d.room_specks.len(),
            specks: specks
                .map(|s| DisruptorSpeckState {
                    id: s.update_order,
                    pos: s.pos.to_array(),
                    vel: s.vel.to_array(),
                    orbit_rad: s.my_orbit_rad,
                    new_orbit_rad: s.new_orbit_rad,
                })
                .collect(),
        }
    }
}
