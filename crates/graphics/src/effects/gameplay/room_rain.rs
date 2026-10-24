//! A `RoomRain`'s sprites: sprite 0, a screen-sized quad with the `DeathRain` shader in
//! `GrabShaders` (its `_MainTex`, the room's `RainMask_`, is read by the shader from the
//! level's mask texture), and `splashes` splash sprites in `Items`, of which the game never
//! adds the last to a container nor updates it (the user's decision, R3: ported as is). Each
//! drawn frame moves the splashes to random `splashTiles` on the cosmetic stream (D1) and
//! sets the shader's globals. Omitted DLC and Remix branches: D46.
//!
//! Mirrors: RoomRain.cs (InitiateSprites, DrawSprites, AddToContainer, SplashSize)

use rw_assets::DangerType;
use rw_random::UnityRandom;
use rw_sim::world::RoomRain;
use rw_sim::{FloatExt, Vec2, Water};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// The globals `DrawSprites` sets for the `DeathRain` shader.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RainShaderGlobals {
    /// `_rainDirection`.
    pub direction: f32,
    /// `_rainIntensity`.
    pub intensity: f32,
    /// `_rainEverywhere`.
    pub everywhere: f32,
    /// `_RainSpriteRect`.
    pub sprite_rect: [f32; 4],
}

/// What `DrawSprites` reads of the camera, the room and the global rain.
pub struct RainDrawView<'a> {
    /// `camPos`: the frame's camera position.
    pub cam_pos: Vec2,
    /// `CamPos(currentCameraPosition)`.
    pub camera_position: Vec2,
    /// `options.ScreenSize.x`.
    pub screen_width: f32,
    /// The room's `TileWidth` and `TileHeight`.
    pub tiles: (i32, i32),
    pub water: Option<&'a Water>,
    /// The palette's `fogColor`.
    pub fog_color: Color,
    /// `ArenaMode`: not a story session.
    pub arena: bool,
    /// `globalRain.lastRainDirection` and `rainDirection`.
    pub direction: (f32, f32),
    pub time_stacker: f32,
}

pub trait RoomRainSprites {
    /// `InitiateSprites`' count: the rain quad and `splashes` splashes.
    fn sprite_count(&self) -> usize;

    /// `DrawSprites`: the shader globals (set whatever the intensity) and the visible
    /// sprites that sit in a container.
    fn draw(&self, view: &RainDrawView<'_>, rng: &mut UnityRandom)
    -> (RainShaderGlobals, DrawList);
}

impl RainDrawView<'_> {
    /// `RoomCamera.IsVisibleAtCameraPosition`.
    fn is_visible(&self, pos: Vec2) -> bool {
        let cam = self.camera_position;
        pos.x > cam.x + 188.0
            && pos.x < cam.x + 188.0 + self.screen_width
            && pos.y > cam.y + 18.0
            && pos.y < cam.y + 18.0 + 768.0
    }

    /// `Room.FloatWaterLevel`.
    fn float_water_level(&self, pos: Vec2) -> f32 {
        self.water.map_or(f32::MIN, |w| w.level_at(pos.x))
    }
}

impl RoomRainSprites for RoomRain {
    fn sprite_count(&self) -> usize {
        1 + self.splashes.max(0) as usize
    }

    fn draw(
        &self,
        view: &RainDrawView<'_>,
        rng: &mut UnityRandom,
    ) -> (RainShaderGlobals, DrawList) {
        let (w, h) = (view.tiles.0 as f32, view.tiles.1 as f32);
        let everywhere = if self.danger_type == DangerType::Rain && !view.arena {
            self.rain_under_ceilings().max(0.1)
        } else {
            self.rain_under_ceilings()
        };
        let globals = RainShaderGlobals {
            direction: view.direction.0.lerp(view.direction.1, view.time_stacker),
            intensity: self.intensity.inverse_lerp(0.0, 0.5),
            everywhere,
            sprite_rect: [
                view.cam_pos.x / (20.0 * w),
                view.cam_pos.y / (20.0 * h),
                1366.0 / (20.0 * w),
                768.0 / (20.0 * h),
            ],
        };
        let mut items = Vec::new();
        if self.intensity <= 0.0 {
            return (globals, DrawList { items });
        }
        items.push(DrawItem::Sprite(Sprite {
            index: 0,
            element: "Futile_White",
            pos: view.cam_pos,
            rotation: 0.0,
            scale: Vec2::new(view.screen_width / 16.0, 768.0 / 16.0),
            anchor: Vec2::new(0.0, 0.0),
            color: Color::WHITE,
            shader: Shader::DeathRain,
            layer: Layer::GrabShaders,
        }));
        let size = self.splash_size();
        for j in 1..self.splashes {
            let near = j < self.splashes / 2;
            let mut at = None;
            if !self.splash_tiles.is_empty() {
                for _ in 0..5 {
                    let tile = self.splash_tiles
                        [rng.range_i32(0, self.splash_tiles.len() as i32) as usize];
                    let spot = Vec2::middle_of_tile(tile);
                    if spot.y > view.float_water_level(spot) && view.is_visible(spot) {
                        let x = spot.x + (-10.0).lerp(10.0, rng.value());
                        at = Some(Vec2::new(x, spot.y + 10.0));
                        break;
                    }
                }
            }
            let (rotation, scale_x) = if near {
                (
                    (-45.0).lerp(45.0, rng.value()),
                    (if rng.value() < 0.5 { -1.0 } else { 1.0 }) * size,
                )
            } else {
                (
                    (-25.0).lerp(25.0, rng.value()),
                    (-1.0).lerp(1.0, rng.value()) * size,
                )
            };
            let color = view
                .fog_color
                .lerp(Color::WHITE, rng.value() * self.intensity);
            if let Some(pos) = at {
                items.push(DrawItem::Sprite(Sprite {
                    index: j as usize,
                    element: if near { "90DegreeSplash" } else { "TallSplash" },
                    pos,
                    rotation,
                    scale: Vec2::new(scale_x, size),
                    anchor: Vec2::new(0.5, if near { 0.2 } else { 0.1 }),
                    color,
                    shader: Shader::Basic,
                    layer: Layer::Items,
                }));
            }
        }
        (globals, DrawList { items })
    }
}

#[cfg(test)]
#[path = "tests/room_rain.rs"]
mod tests;
