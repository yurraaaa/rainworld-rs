//! What `get_state` reports of the realized rooms' placed light beams and spot lights.
//!
//! Mirrors: - (port-only: MCP server)

use rw_assets::RoomEffectType;
use rw_graphics::{LightBeamSprites, LightBeamView};
use rw_sim::world::World;
use rw_sim::{FloatExt, LightBeam, SpotLight, Vec2};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One placed `LightBeam`.
#[derive(Debug, Clone, Serialize)]
pub struct LightBeamState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    /// `quad`: the position, then the position plus each handle.
    pub quad: [[f32; 2]; 4],
    /// The data's alpha, colour mixes and sun flag.
    pub alpha: f32,
    pub color_a: f32,
    pub color_b: f32,
    pub sun: bool,
    /// `InverseLerp(-0.2, 0, ShaderLight)^1.2`, the sun's factor now.
    pub sun_factor: f32,
    /// The alpha `DrawSprites` computes now (the vertex alpha).
    pub draw_alpha: f32,
    pub palette_lit_color: [f32; 4],
    pub environment_color: [f32; 4],
}

/// One placed `SpotLight`.
#[derive(Debug, Clone, Serialize)]
pub struct SpotLightState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub quad: [[f32; 2]; 4],
}

impl LightBeamState {
    /// Every realized room's beams.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        let shader_light = world.rain_cycle.shader_light();
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                let view = LightBeamView {
                    camera: 0,
                    shader_light,
                    darken_lights: r.settings.effect_amount(RoomEffectType::DarkenLights),
                    water_lights: false,
                };
                r.light_beams
                    .iter()
                    .map(move |b| Self::of(room.clone(), b, &view))
            })
            .collect()
    }

    fn of(room: String, b: &LightBeam, view: &LightBeamView) -> Self {
        Self {
            room,
            id: b.update_order,
            quad: b.quad.map(Vec2::to_array),
            alpha: b.data.alpha,
            color_a: b.data.color_a,
            color_b: b.data.color_b,
            sun: b.data.sun,
            sun_factor: view.shader_light.inverse_lerp(-0.2, 0.0).mathf_pow(1.2),
            draw_alpha: LightBeamSprites::frame_alpha(b, view),
            palette_lit_color: b.palette_lit_color,
            environment_color: b.environment_color,
        }
    }
}

impl SpotLightState {
    /// Every realized room's spot lights.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| r.spot_lights.iter().map(move |s| Self::of(room.clone(), s)))
            .collect()
    }

    fn of(room: String, s: &SpotLight) -> Self {
        Self {
            room,
            id: s.update_order,
            quad: s.quad.map(Vec2::to_array),
        }
    }
}
