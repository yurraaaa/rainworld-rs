//! What `get_state` reports of the realized rooms' `ElectricDeath`s and
//! `RoomBorderPushBack`s.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{BorderPushBack, ElectricDeath, LightFlash, Strike};
use serde::Serialize;

/// One `LightFlash`.
#[derive(Debug, Clone, Serialize)]
pub struct LightFlashState {
    /// Screen pixels.
    pub pos: [f32; 2],
    pub rad: f32,
    pub life: f32,
    pub life_time: f32,
}

/// One strike on a body chunk.
#[derive(Debug, Clone, Serialize)]
pub struct StrikeState {
    pub tick: u64,
    pub creature: bool,
    pub pos: [f32; 2],
    /// `num2`.
    pub power: f32,
    /// `Violence`'s damage (0 for an item).
    pub damage: f32,
    pub stun_bonus: f32,
}

/// One room's `ElectricDeath`.
#[derive(Debug, Clone, Serialize)]
pub struct ElectricDeathState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub amount: f32,
    /// `Intensity` now.
    pub intensity: f32,
    pub sin: f32,
    pub color: [f32; 3],
    pub get_to_color: [f32; 3],
    /// `flashes[1..]` (the first is never updated).
    pub flashes: Vec<LightFlashState>,
    /// The volumes of `soundLoop` (heavy) and `soundLoop2` (early sizzle), once made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heavy_loop_volume: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizzle_loop_volume: Option<f32>,
    /// The loops' emitters in the update list.
    pub loop_emitters: usize,
    /// `closeToWallTiles`'s length.
    pub wall_tiles: usize,
    /// The last strikes, oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_strikes: Vec<StrikeState>,
}

/// One room's `RoomBorderPushBack`.
#[derive(Debug, Clone, Serialize)]
pub struct BorderPushBackState {
    pub room: String,
    pub id: u64,
    pub margin: f32,
    pub leftmost_camera_pos: f32,
    pub rightmost_camera_pos: f32,
    /// The x a player's main chunk must stay between: `leftmost - margin` and
    /// `rightmost + 1400 + margin`.
    pub edges: [f32; 2],
    /// `pushPower` per player.
    pub push_power: Vec<f32>,
}

impl ElectricDeathState {
    /// Every realized room's `ElectricDeath`s.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        let mut out = Vec::new();
        for i in world.active_rooms() {
            let Some(r) = world.realized(i) else { continue };
            let room = RoomName::of(world, i);
            out.extend(
                r.electric_deaths
                    .iter()
                    .map(|d| Self::of(room.clone(), d, &world.rain_cycle)),
            );
        }
        out
    }

    fn of(room: String, d: &ElectricDeath, cycle: &rw_sim::world::RainCycle) -> Self {
        Self {
            room,
            id: d.update_order,
            amount: d.amount,
            intensity: d.intensity(cycle),
            sin: d.sin,
            color: [d.color.x, d.color.y, d.color.z],
            get_to_color: [d.get_to_color.x, d.get_to_color.y, d.get_to_color.z],
            flashes: d.flashes[1..].iter().map(LightFlashState::of).collect(),
            heavy_loop_volume: d.heavy_loop.as_ref().map(|l| l.volume()),
            sizzle_loop_volume: d.sizzle_loop.as_ref().map(|l| l.volume()),
            loop_emitters: d.emitters().count(),
            wall_tiles: d.close_to_wall_tiles.as_ref().map_or(0, Vec::len),
            recent_strikes: d.strikes.iter().map(StrikeState::of).collect(),
        }
    }
}

impl LightFlashState {
    fn of(f: &LightFlash) -> Self {
        Self {
            pos: f.pos.to_array(),
            rad: f.rad,
            life: f.life,
            life_time: f.life_time,
        }
    }
}

impl StrikeState {
    fn of(s: &Strike) -> Self {
        Self {
            tick: s.tick,
            creature: s.creature,
            pos: s.pos.to_array(),
            power: s.power,
            damage: s.damage,
            stun_bonus: s.stun_bonus,
        }
    }
}

impl BorderPushBackState {
    /// Every realized room's `RoomBorderPushBack`s.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        let mut out = Vec::new();
        for i in world.active_rooms() {
            let Some(r) = world.realized(i) else { continue };
            let room = RoomName::of(world, i);
            out.extend(
                r.border_push_backs
                    .iter()
                    .map(|b| Self::of(room.clone(), b)),
            );
        }
        out
    }

    fn of(room: String, b: &BorderPushBack) -> Self {
        Self {
            room,
            id: b.update_order,
            margin: b.margin,
            leftmost_camera_pos: b.leftmost_camera_pos,
            rightmost_camera_pos: b.rightmost_camera_pos,
            edges: b.bounds().to_array(),
            push_power: b.push_power.clone(),
        }
    }
}

/// A room's name for the replies.
struct RoomName;

impl RoomName {
    fn of(world: &World, index: i32) -> String {
        world
            .abstract_room(index)
            .map_or_else(|| format!("#{index}"), |r| r.name.clone())
    }
}
