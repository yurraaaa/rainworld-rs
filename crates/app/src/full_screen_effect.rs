//! The room camera's `fullScreenEffect`: one `Futile_White` sprite over the screen with
//! the room's screen-wide shader, chosen by `ApplyPalette` (the first match wins) and
//! given its strength every frame by `DrawUpdate`.
//!
//! Mirrors: RoomCamera.cs (ApplyPalette, SetUpFullScreenEffect, DrawUpdate)

use rw_assets::{RoomEffectType, RoomSettings};
use rw_graphics::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use rw_render::SpriteAtlas;
use rw_sim::{FloatExt, Vec2};

/// The effect `ApplyPalette` set up for a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullScreenEffect {
    /// `lightBloomAlphaEffect`: the room effect whose amount sets the strength.
    pub effect: RoomEffectType,
    pub shader: Shader,
    /// The container `SetUpFullScreenEffect` put the sprite in.
    pub layer: Layer,
}

impl FullScreenEffect {
    /// `ApplyPalette`'s chain for a room with `settings`: `SkyBloom`, `SkyAndLightBloom`,
    /// `LightBurn` or `Lightning` (the `LightBloom` shader, strength from `Lightning`
    /// when the room has it), `Fog`, `Bloom`, each when its amount is above 0. The
    /// echo's branch (`ghostMode > 0`, first in the chain) comes with echoes, and
    /// `VoidMelt`'s (`LevelMelt2`, last) with that effect: until then a room whose only
    /// match is `VoidMelt` gets no effect.
    #[must_use]
    pub fn select(settings: &RoomSettings) -> Option<Self> {
        let (effect, shader, layer) = if Self::on(settings, RoomEffectType::SkyBloom) {
            (RoomEffectType::SkyBloom, Shader::SkyBloom, Layer::Bloom)
        } else if Self::on(settings, RoomEffectType::SkyAndLightBloom) {
            (
                RoomEffectType::SkyAndLightBloom,
                Shader::LightAndSkyBloom,
                Layer::Bloom,
            )
        } else if Self::on(settings, RoomEffectType::LightBurn)
            || Self::on(settings, RoomEffectType::Lightning)
        {
            let effect = if Self::on(settings, RoomEffectType::Lightning) {
                RoomEffectType::Lightning
            } else {
                RoomEffectType::LightBurn
            };
            (effect, Shader::LightBloom, Layer::Bloom)
        } else if Self::on(settings, RoomEffectType::Fog) {
            (RoomEffectType::Fog, Shader::Fog, Layer::Foreground)
        } else if Self::on(settings, RoomEffectType::Bloom) {
            (RoomEffectType::Bloom, Shader::Bloom, Layer::Bloom)
        } else {
            return None;
        };
        Some(Self {
            effect,
            shader,
            layer,
        })
    }

    /// `GetEffectAmount(kind) > 0`.
    fn on(settings: &RoomSettings, kind: RoomEffectType) -> bool {
        settings.effect_amount(kind) > 0.0
    }

    /// `DrawUpdate`'s `fullScreenEffect.alpha`: the effect's amount re-read
    /// (`lightBloomAlpha`), faded with the room's light `num4` (`InverseLerp(-0.7, 0,
    /// num4)`) for the bloom shaders; `Lightning` uses `InverseLerp(0, 0.2, amount)` for
    /// the amount. `num4` is the rain cycle's `ShaderLight` or 1; the lightning step
    /// replaces it with the lightning's light level.
    #[must_use]
    pub fn alpha(&self, settings: &RoomSettings, num4: f32) -> f32 {
        let amount = settings.effect_amount(self.effect);
        let light = num4.inverse_lerp(-0.7, 0.0);
        if self.effect == RoomEffectType::Lightning {
            amount.inverse_lerp(0.0, 0.2) * light
        } else if amount > 0.0
            && matches!(
                self.effect,
                RoomEffectType::Bloom
                    | RoomEffectType::SkyBloom
                    | RoomEffectType::SkyAndLightBloom
                    | RoomEffectType::LightBurn
            )
        {
            amount * light
        } else {
            amount
        }
    }

    /// The sprite (`SetUpFullScreenEffect`): `Futile_White` scaled to `ScreenSize.x / 16`
    /// by 48, anchored at its bottom-left on the stage origin (room position `cam_pos`),
    /// white at `alpha`.
    #[must_use]
    pub fn draw_list(&self, screen_width: f32, cam_pos: Vec2, alpha: f32) -> DrawList {
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: SpriteAtlas::FUTILE_WHITE,
                pos: cam_pos,
                rotation: 0.0,
                scale: Vec2::new(screen_width / 16.0, 48.0),
                anchor: Vec2::ZERO,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: alpha.clamp(0.0, 1.0),
                },
                shader: self.shader,
                layer: self.layer,
            })],
        }
    }
}

#[cfg(test)]
#[path = "full_screen_effect_tests.rs"]
mod tests;
