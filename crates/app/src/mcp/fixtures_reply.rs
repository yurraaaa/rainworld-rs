//! What `get_state` reports of the realized rooms' light fixtures (`Redlight`, `HolyFire`,
//! `SlimeMoldLight`) and waterfalls.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::{RealizedRoom, World};
use rw_sim::{LightSource, WaterFall};
use serde::Serialize;

/// One light of a fixture.
#[derive(Debug, Clone, Serialize)]
pub struct FixtureLight {
    pub id: u64,
    pub pos: [f32; 2],
    pub alpha: f32,
    pub rad: f32,
    pub color: [f32; 3],
    pub flat: bool,
    /// The `LightMask0` element (a `RedlightLight`).
    pub mask: bool,
}

/// One light fixture.
#[derive(Debug, Clone, Serialize)]
pub struct LightFixtureState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    /// `RedLight`, `HolyFire` or `SlimeMoldLight`.
    pub kind: &'static str,
    pub pos: [f32; 2],
    pub lights: Vec<FixtureLight>,
    /// A red light's flicker state `[flicker, flickerWait]`, `sin`, `powered` and
    /// `gravityDependent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flicker: Option<([i32; 2], f32, bool, bool)>,
    /// A holy fire's `getToPositions`, `getToRads` and the embers it shed that are alive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fire: Option<([[f32; 2]; 3], [f32; 3], usize)>,
    /// A slime mold light's discs: offset, size and alpha.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub discs: Vec<([f32; 2], f32, f32)>,
}

/// One `WaterFall`.
#[derive(Debug, Clone, Serialize)]
pub struct WaterFallState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub tile: [i32; 2],
    pub width: i32,
    pub flow: f32,
    pub set_flow: f32,
    pub visual_density: f32,
    /// `topPos[0]` and `bottomPos[0]`.
    pub top: f32,
    pub bottom: f32,
    pub strike_level: f32,
    pub flooded: bool,
    pub water_contact: bool,
    pub hit_terrain_tiles: usize,
    pub bubbles: usize,
    /// The bubbles not parked off screen.
    pub bubbles_active: usize,
}

impl FixtureLight {
    fn of(l: &LightSource) -> Self {
        Self {
            id: l.id,
            pos: l.pos.to_array(),
            alpha: l.alpha,
            rad: l.rad,
            color: l.color,
            flat: l.flat,
            mask: l.element == rw_sim::LightElement::LightMask0,
        }
    }
}

impl LightFixtureState {
    /// Every realized room's fixtures.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        let mut out = Vec::new();
        for i in world.active_rooms() {
            let Some(r) = world.realized(i) else { continue };
            let room = RoomName::of(world, i);
            out.extend(r.fixtures.redlights.iter().map(|f| {
                let mut lights = vec![r.light_sources.iter().find(|l| l.id == f.flat_light)];
                lights.push(
                    f.light
                        .and_then(|id| r.light_sources.iter().find(|l| l.id == id)),
                );
                Self {
                    room: room.clone(),
                    id: f.update_order,
                    kind: "RedLight",
                    pos: f.pos.to_array(),
                    lights: lights.into_iter().flatten().map(FixtureLight::of).collect(),
                    flicker: Some((
                        [f.flicker, f.flicker_wait],
                        f.sin,
                        f.powered,
                        f.gravity_dependent,
                    )),
                    fire: None,
                    discs: Vec::new(),
                }
            }));
            out.extend(r.fixtures.holy_fires.iter().map(|f| {
                Self {
                    room: room.clone(),
                    id: f.update_order,
                    kind: "HolyFire",
                    pos: f.pos.to_array(),
                    lights: f
                        .lights
                        .iter()
                        .chain([&f.flat_light])
                        .filter_map(|id| r.light_sources.iter().find(|l| l.id == *id))
                        .map(FixtureLight::of)
                        .collect(),
                    flicker: None,
                    fire: Some((
                        f.get_to_positions.map(|p| p.to_array()),
                        f.get_to_rads,
                        r.fixtures
                            .holy_sprites
                            .iter()
                            .filter(|s| !s.slated && s.fire == f.update_order)
                            .count(),
                    )),
                    discs: Vec::new(),
                }
            }));
            out.extend(r.fixtures.slime_lights.iter().map(|f| {
                Self {
                    room: room.clone(),
                    id: f.update_order,
                    kind: "SlimeMoldLight",
                    pos: f.pos.to_array(),
                    lights: Vec::new(),
                    flicker: None,
                    fire: None,
                    discs: (0..5)
                        .map(|k| {
                            (
                                f.positions[k].to_array(),
                                f.sizes_and_alphas[k][0],
                                f.sizes_and_alphas[k][1],
                            )
                        })
                        .collect(),
                }
            }));
        }
        out
    }
}

impl WaterFallState {
    /// Every realized room's waterfalls.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.fixtures
                    .waterfalls
                    .iter()
                    .map(move |w| Self::of(room.clone(), r, w))
            })
            .collect()
    }

    fn of(room: String, r: &RealizedRoom, w: &WaterFall) -> Self {
        let water = r.water.as_ref();
        Self {
            room,
            id: w.update_order,
            tile: [w.tile_pos.0, w.tile_pos.1],
            width: w.width,
            flow: w.flow,
            set_flow: w.set_flow,
            visual_density: w.visual_density,
            top: w.top_pos[0],
            bottom: w.bottom_pos[0],
            strike_level: WaterFall::strike_level(water),
            flooded: w.flooded(water),
            water_contact: w.water_contact(water),
            hit_terrain_tiles: w.hit_terrain_tiles.len(),
            bubbles: w.bubbles.len(),
            bubbles_active: w.bubbles.iter().filter(|b| b[0].x > -999.0).count(),
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
