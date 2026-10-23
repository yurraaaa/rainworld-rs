//! What `get_state` reports of the realized rooms' `SSLightRod`s, `ZapCoil`s (with their
//! `ZapFlash`es and `ZapCoilLight`s) and `SuperStructureFuses`.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{LightSource, SsLightRod, SuperStructureFuses, ZapCoil, ZapCoilLight, ZapFlash};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One light riding a rod.
#[derive(Debug, Clone, Serialize)]
pub struct LightVesselState {
    pub light: u64,
    pub progression: f32,
    pub speed: f32,
    pub size: f32,
    pub strength: f32,
    pub visible: f32,
    /// Its `LightSource`'s rad and alpha now.
    pub rad: f32,
    pub alpha: f32,
}

/// One `SSLightRod`.
#[derive(Debug, Clone, Serialize)]
pub struct LightRodState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub rotation: f32,
    pub length: f32,
    pub depth: f32,
    pub brightness: f32,
    pub lights: Vec<LightVesselState>,
}

/// One `ZapCoil`.
#[derive(Debug, Clone, Serialize)]
pub struct ZapCoilState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    /// `IntRect(left, bottom, right, top)` in tiles.
    pub rect: [i32; 4],
    pub horizontal: bool,
    pub powered: bool,
    pub turned_on: f32,
    pub turned_off_counter: i32,
    pub disruption: f32,
    pub smooth_disruption: f32,
    pub zap_lit: f32,
    /// The two edges' flicker now.
    pub flicker: [f32; 2],
    /// The hum's and the sizzle's volumes.
    pub volumes: [f32; 2],
}

/// One `ZapFlash`.
#[derive(Debug, Clone, Serialize)]
pub struct ZapFlashState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub size: f32,
    pub life: f32,
    pub life_time: f32,
    pub light: Option<u64>,
}

/// One `ZapCoilLight`.
#[derive(Debug, Clone, Serialize)]
pub struct ZapCoilLightState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub random_seed: i32,
    /// The update-list place of its coil, once found.
    pub coil: Option<u64>,
    pub light: u64,
    /// Its `LightSource`'s rad and alpha now.
    pub rad: f32,
    pub alpha: f32,
}

/// One `SuperStructureFuses`.
#[derive(Debug, Clone, Serialize)]
pub struct FuseState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub rect: [i32; 4],
    pub size: f32,
    /// Cells across and up.
    pub cells: [usize; 2],
    pub depth: i32,
    pub broken: f32,
    pub culled: bool,
    pub gravity_dependent: bool,
    pub power: f32,
    pub power_flicker: f32,
    pub lightness: f32,
    pub mal_function: i32,
    pub smoothed_malfunction: f32,
    pub malfunction_rect: [i32; 4],
    pub debug_mode: i32,
    /// How many cells are in each mode 0 to 5.
    pub modes: [usize; 6],
    /// The cells' mean brightness now.
    pub mean_brightness: f32,
}

impl LightRodState {
    /// Every realized room's rods.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .light_rods
                    .iter()
                    .map(move |x| Self::of(room.clone(), x, &r.light_sources))
            })
            .collect()
    }

    fn of(room: String, rod: &SsLightRod, lights: &[LightSource]) -> Self {
        Self {
            room,
            id: rod.update_order,
            pos: rod.pos.to_array(),
            rotation: rod.data.rotation,
            length: rod.data.length,
            depth: rod.data.depth,
            brightness: rod.data.brightness,
            lights: rod
                .lights
                .iter()
                .map(|v| {
                    let l = lights.iter().find(|l| l.id == v.light);
                    LightVesselState {
                        light: v.light,
                        progression: v.progression,
                        speed: v.speed,
                        size: v.size,
                        strength: v.strength,
                        visible: v.visible,
                        rad: l.map_or(0.0, |l| l.rad),
                        alpha: l.map_or(0.0, |l| l.alpha),
                    }
                })
                .collect(),
        }
    }
}

impl ZapCoilState {
    /// Every realized room's coils.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .zap_coils
                    .iter()
                    .map(move |c| Self::of(room.clone(), c))
            })
            .collect()
    }

    fn of(room: String, c: &ZapCoil) -> Self {
        Self {
            room,
            id: c.update_order,
            rect: c.rect,
            horizontal: c.horizontal,
            powered: c.powered,
            turned_on: c.turned_on,
            turned_off_counter: c.turned_off_counter,
            disruption: c.disruption,
            smooth_disruption: c.smooth_disruption,
            zap_lit: c.zap_lit,
            flicker: [c.flicker[0][0], c.flicker[1][0]],
            volumes: [c.sound_loop.volume(), c.disrupted_loop.volume()],
        }
    }
}

impl ZapFlashState {
    /// Every realized room's flashes.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .zap_flashes
                    .iter()
                    .filter(|f| !f.slated)
                    .map(move |f| Self::of(room.clone(), f))
            })
            .collect()
    }

    fn of(room: String, f: &ZapFlash) -> Self {
        Self {
            room,
            id: f.update_order,
            pos: f.pos.to_array(),
            size: f.size,
            life: f.life,
            life_time: f.life_time,
            light: f.light,
        }
    }
}

impl ZapCoilLightState {
    /// Every realized room's coil lights.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .zap_coil_lights
                    .iter()
                    .map(move |l| Self::of(room.clone(), l, &r.light_sources))
            })
            .collect()
    }

    fn of(room: String, l: &ZapCoilLight, lights: &[LightSource]) -> Self {
        let source = lights.iter().find(|s| s.id == l.light);
        Self {
            room,
            id: l.update_order,
            pos: l.pos.to_array(),
            random_seed: l.random_seed,
            coil: l.coil,
            light: l.light,
            rad: source.map_or(0.0, |s| s.rad),
            alpha: source.map_or(0.0, |s| s.alpha),
        }
    }
}

impl FuseState {
    /// Every realized room's fuse grids.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .fuses
                    .iter()
                    .map(move |f| Self::of(room.clone(), f))
            })
            .collect()
    }

    fn of(room: String, f: &SuperStructureFuses) -> Self {
        let mut modes = [0; 6];
        let mut sum = 0.0;
        for j in 0..f.width {
            for k in 0..f.height {
                let cell = f.cell(j, k);
                modes[(cell[3] as usize).min(5)] += 1;
                sum += cell[0];
            }
        }
        let count = f.width * f.height;
        Self {
            room,
            id: f.update_order,
            rect: f.rect,
            size: f.size,
            cells: [f.width, f.height],
            depth: f.depth,
            broken: f.broken,
            culled: f.culled,
            gravity_dependent: f.gravity_dependent,
            power: f.power,
            power_flicker: f.power_flicker,
            lightness: f.lightness,
            mal_function: f.mal_function,
            smoothed_malfunction: f.smoothed_malfunction,
            malfunction_rect: f.malfunction_rect,
            debug_mode: f.debug_mode,
            modes,
            mean_brightness: if count == 0 { 0.0 } else { sum / count as f32 },
        }
    }
}
