//! What `get_state` reports of zero gravity: each realized room's `Room.gravity` and
//! `ElectricPower`, and the world's `BrokenAntiGravity` switch.
//!
//! Mirrors: - (port-only: MCP server)

use rw_assets::RoomEffectType;
use rw_sim::world::World;
use serde::Serialize;

/// A realized room's gravity and power.
#[derive(Debug, Clone, Serialize)]
pub struct RoomGravityState {
    pub room: String,
    /// `Room.gravity`.
    pub gravity: f32,
    /// `Room.ElectricPower`.
    pub electric_power: f32,
    /// The `ZeroG` and `BrokenZeroG` effect amounts.
    pub zero_g: f32,
    pub broken_zero_g: f32,
}

/// `rainCycle.brokenAntiGrav`.
#[derive(Debug, Clone, Serialize)]
pub struct BrokenAntiGravityState {
    pub on: bool,
    pub counter: i32,
    pub from: f32,
    pub to: f32,
    pub progress: f32,
    /// `lights` (`CurrentLightsOn`).
    pub lights: f32,
    /// `CurrentAntiGravity`.
    pub current_anti_gravity: f32,
}

impl RoomGravityState {
    /// Every realized room with a `ZeroG` or `BrokenZeroG` effect.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| {
                let r = world.realized(i)?;
                let zero_g = r.settings.effect_amount(RoomEffectType::ZeroG);
                let broken_zero_g = r.settings.effect_amount(RoomEffectType::BrokenZeroG);
                if zero_g <= 0.0 && broken_zero_g <= 0.0 {
                    return None;
                }
                let room = world
                    .abstract_room(i)
                    .map_or_else(|| format!("#{i}"), |a| a.name.clone());
                Some(Self {
                    room,
                    gravity: r.gravity,
                    electric_power: r.electric_power(&world.rain_cycle),
                    zero_g,
                    broken_zero_g,
                })
            })
            .collect()
    }
}

impl BrokenAntiGravityState {
    #[must_use]
    pub fn of(world: &World) -> Option<Self> {
        let b = world.rain_cycle.broken_anti_grav?;
        Some(Self {
            on: b.on,
            counter: b.counter,
            from: b.from,
            to: b.to,
            progress: b.progress,
            lights: b.lights,
            current_anti_gravity: b.current_anti_gravity(),
        })
    }
}
