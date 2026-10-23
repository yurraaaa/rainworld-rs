//! The sprites of a room's `BackgroundScene` elements (`AboveCloudsView`, `RoofTopView`),
//! all in the `Water` container: where `DrawPos` puts each one for the frame's camera,
//! its scale, colour and shader. Colours carry the shaders' inputs (a cloud's depth tint
//! and random offset, a building's haze), quantised to 8 bits as Unity stores a mesh's
//! `Color[]`. A [`rw_sim::DistantLightning`]'s brightness draws on the cosmetic stream
//! (3 draws when the first is under a third, else 1).
//!
//! Mirrors: BackgroundScene.cs (BackgroundSceneElement, Simple2DBackgroundIllustration, FullScreenSingleColor, AddToContainer), AboveCloudsView.cs (Cloud, CloseCloud, DistantCloud, FlyingCloud, DistantBuilding, Fog, DistantLightning: InitiateSprites, DrawSprites), RoofTopView.cs (Floor, Building, Rubble, DistantBuilding, Smoke: InitiateSprites, DrawSprites)

use rustc_hash::FxHashMap;
use rw_random::UnityRandom;
use rw_sim::{
    AboveCloudsParams, BackgroundScene, ElementKind, FloatExt, SceneElement, SceneParams, Vec2,
};

use crate::decal::Unorm8;
use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// The size in pixels of each scene image, by element name (`sourceSize` of the atlas
/// element `LoadGraphic` made).
#[derive(Debug, Clone, Default)]
pub struct SceneImageSizes(FxHashMap<&'static str, Vec2>);

impl SceneImageSizes {
    pub fn insert(&mut self, name: &'static str, size: Vec2) {
        self.0.insert(name, size);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Vec2> {
        self.0.get(name).copied()
    }
}

/// What a frame's scene sprites read of the camera and palette.
#[derive(Debug, Clone, Copy)]
pub struct BackgroundSceneView {
    /// `camPos` the room camera passes to `DrawSprites`.
    pub cam_pos: Vec2,
    /// `room.cameraPositions[currentCameraPosition]`.
    pub camera_position: Vec2,
    /// `rCam.hDisplace`.
    pub h_displace: f32,
    /// `rainWorld.screenSize`.
    pub screen: Vec2,
    /// `palette.skyColor`.
    pub sky_color: Color,
    /// The frame's time stacker.
    pub time_stacker: f32,
}

/// The sprites of one scene element.
pub struct BackgroundSceneSprites;

/// One sprite as `InitiateSprites` and `DrawSprites` leave it, in screen pixels.
struct Piece {
    element: &'static str,
    pos: Vec2,
    scale: Vec2,
    anchor_y: f32,
    color: Color,
    shader: Shader,
}

impl Piece {
    fn new(element: &'static str, shader: Shader) -> Self {
        Self {
            element,
            pos: Vec2::ZERO,
            scale: Vec2::new(1.0, 1.0),
            anchor_y: 0.5,
            color: Color::WHITE,
            shader,
        }
    }
}

impl BackgroundSceneSprites {
    /// The `Water` container.
    const LAYER: Layer = Layer::Water;

    /// The sprites `DrawSprites` of `element` leaves visible for `view`. `sizes` holds
    /// the images' sizes; the lightning's brightness draws from `rng`.
    #[must_use]
    pub fn draw(
        scene: &BackgroundScene,
        element: &SceneElement,
        view: &BackgroundSceneView,
        sizes: &SceneImageSizes,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let frame = Frame {
            scene,
            element,
            view,
            sizes,
        };
        let pieces = match &element.kind {
            ElementKind::Sky { name, alpha, .. } => vec![frame.sky(name, *alpha)],
            ElementKind::Fog => frame.fog(),
            ElementKind::DistantBuilding {
                asset,
                atmospheric_depth_add,
                alpha,
                scale,
            } => frame.distant_building(asset, *atmospheric_depth_add, *alpha, *scale),
            ElementKind::DistantLightning(l) => frame.distant_lightning(l, rng),
            ElementKind::CloseCloud { cloud, cloud_depth } => {
                frame.close_cloud(cloud.index, cloud.random_offset, *cloud_depth)
            }
            ElementKind::DistantCloud {
                cloud,
                distant_cloud_depth,
            } => frame.distant_cloud(cloud.index, cloud.random_offset, *distant_cloud_depth),
            ElementKind::FlyingCloud {
                cloud,
                flattened,
                alpha,
                shader_input_color,
            } => frame.flying_cloud(cloud.random_offset, *flattened, *alpha, *shader_input_color),
            ElementKind::Floor {
                asset,
                from_depth,
                to_depth,
            } => frame.floor(asset, *from_depth, *to_depth),
            ElementKind::Building { asset, scale } => frame.building(asset, *scale),
            ElementKind::Rubble {
                asset,
                random_value,
            } => frame.rubble(asset, *random_value),
            ElementKind::RoofDistantBuilding { asset } => frame.roof_distant_building(asset),
            ElementKind::Smoke {
                flattened,
                alpha,
                shader_input_color,
                random_offset,
                dust,
            } => frame.smoke(
                *flattened,
                *alpha,
                *shader_input_color,
                *random_offset,
                *dust,
            ),
        };
        DrawList {
            items: pieces
                .into_iter()
                .enumerate()
                .map(|(index, p)| {
                    DrawItem::Sprite(Sprite {
                        index,
                        element: p.element,
                        pos: p.pos + view.cam_pos,
                        rotation: 0.0,
                        scale: p.scale,
                        anchor: Vec2::new(0.5, p.anchor_y),
                        color: Color::from([p.color.r, p.color.g, p.color.b, p.color.a].unorm8()),
                        shader: p.shader,
                        layer: Self::LAYER,
                    })
                })
                .collect(),
        }
    }
}

/// The element and frame a draw reads.
struct Frame<'a> {
    scene: &'a BackgroundScene,
    element: &'a SceneElement,
    view: &'a BackgroundSceneView,
    sizes: &'a SceneImageSizes,
}

impl Frame<'_> {
    fn params(&self) -> Option<&AboveCloudsParams> {
        match &self.scene.params {
            SceneParams::AboveClouds(p) => Some(p),
            SceneParams::RoofTop => None,
        }
    }

    fn y_shift(&self) -> f32 {
        self.params().map_or(0.0, |p| p.y_shift)
    }

    /// `DrawPos(camPos, hDisplace)` of the element.
    fn draw_pos(&self, cam_pos: Vec2) -> Vec2 {
        self.scene.draw_pos(
            self.element.pos,
            self.element.depth,
            cam_pos,
            self.view.h_displace,
        )
    }

    /// `DrawPos(new Vector2(camPos.x, camPos.y + yShift), hDisplace)`.
    fn draw_pos_shifted(&self) -> Vec2 {
        self.draw_pos(self.view.cam_pos + Vec2::new(0.0, self.y_shift()))
    }

    fn size_of(&self, name: &str) -> Vec2 {
        self.sizes.get(name).unwrap_or(Vec2::new(1.0, 1.0))
    }

    /// `RoomToWorldPos(camPos).y + yShift`.
    fn world_y(&self) -> f32 {
        self.scene.room_to_world_pos(self.view.cam_pos).y + self.y_shift()
    }

    /// `InverseLerp(startAltitude, endAltitude, RoomToWorldPos(cameraPositions[current]).y)`
    /// (plus `yShift` where the element adds it).
    fn altitude(&self, shifted: bool) -> f32 {
        let Some(p) = self.params() else { return 0.0 };
        let mut y = self.scene.room_to_world_pos(self.view.camera_position).y;
        if shifted {
            y += p.y_shift;
        }
        y.inverse_lerp(p.start_altitude, p.end_altitude)
    }

    /// `Mathf.Sin`.
    fn sin(x: f32) -> f32 {
        use crate::math::Mathf;
        x.mathf_sin()
    }

    fn atmosphere(&self) -> Color {
        let [r, g, b] = self.scene.atmosphere_color;
        Color::rgb(r, g, b)
    }

    fn sky(&self, name: &'static str, alpha: f32) -> Piece {
        let mut p = Piece::new(name, Shader::Background);
        p.pos = self.element.pos;
        p.color = Color::WHITE.with_alpha(alpha);
        p
    }

    /// `AboveCloudsView.Fog.DrawSprites`.
    fn fog(&self) -> Vec<Piece> {
        let Some(p) = self.params() else {
            return Vec::new();
        };
        let alpha = if p.arena {
            0.0
        } else if p.si_clouds {
            self.world_y().inverse_lerp(16000.0, 9000.0) * 0.6
        } else {
            self.world_y().inverse_lerp(22000.0, 18000.0) * 0.6
        };
        let screen = self.view.screen;
        let mut fog = Piece::new("pixel", Shader::Background);
        fog.scale = screen + Vec2::new(20.0, 20.0);
        fog.pos = screen / 2.0;
        fog.color = self
            .view
            .sky_color
            .with_alpha(alpha * self.view.sky_color.a);
        vec![fog]
    }

    fn distant_building(
        &self,
        asset: &'static str,
        atmospheric_depth_add: f32,
        alpha: f32,
        scale: f32,
    ) -> Vec<Piece> {
        let mut p = Piece::new(asset, Shader::DistantBkgObject);
        p.anchor_y = 0.0;
        p.scale = Vec2::new(scale, scale);
        p.pos = self.draw_pos_shifted();
        let haze = (self.element.depth + atmospheric_depth_add)
            .inverse_lerp(0.0, 600.0)
            .mathf_pow(0.3)
            * 0.9;
        p.color = Color::rgb(haze, 0.0, 0.0).with_alpha(alpha);
        vec![p]
    }

    fn distant_lightning(&self, l: &rw_sim::DistantLightning, rng: &mut UnityRandom) -> Vec<Piece> {
        let mut p = Piece::new(l.asset_name, Shader::Background);
        p.anchor_y = 0.0;
        p.pos = self.draw_pos_shifted();
        let value = self.world_y();
        let mut alpha = l.light_intensity(self.view.time_stacker, rng) * 0.2;
        if !l.non_position_based_intensity {
            alpha *= value.inverse_lerp(27600.0, 31400.0);
        }
        p.color = Color::WHITE.with_alpha(alpha);
        vec![p]
    }

    /// The fill under a cloud: a `pixel` of the sky blended toward the atmosphere,
    /// 1400 wide, up to `top`.
    fn cloud_fill(&self, scale_y: f32, color: Color) -> Piece {
        let mut fill = Piece::new("pixel", Shader::Background);
        fill.anchor_y = 0.0;
        fill.scale = Vec2::new(1400.0, scale_y);
        fill.pos = Vec2::new(683.0, 0.0);
        fill.color = color;
        fill
    }

    fn cloud_image(index: usize) -> &'static str {
        rw_sim::SceneImages::CLOUDS[index % 3]
    }

    /// `CloseCloud.DrawSprites`.
    fn close_cloud(&self, index: usize, random_offset: f32, cloud_depth: f32) -> Vec<Piece> {
        let Some(p) = self.params() else {
            return Vec::new();
        };
        let num = self.altitude(false);
        let mut num2 = cloud_depth;
        if num > 0.5 {
            num2 = num2.lerp(1.0, num.inverse_lerp(0.5, 1.0) * 0.5);
        }
        let depth = p.clouds_start_depth.lerp(p.clouds_end_depth, num2);
        let num3 = 10.0_f32.lerp(2.0, num2);
        let cam = self.view.cam_pos + Vec2::new(0.0, p.y_shift);
        let mut y2 = self
            .scene
            .draw_pos(self.element.pos, depth, cam, self.view.h_displace)
            .y;
        y2 += cloud_depth
            .mathf_pow(0.75)
            .lerp(Self::sin(cloud_depth * std::f32::consts::PI), 0.5)
            * num.inverse_lerp(0.5, 0.0)
            * 600.0;
        y2 -= num.inverse_lerp(0.18, 0.1) * (1.0 - cloud_depth).mathf_pow(3.0) * 100.0;
        let mut num4 = 1.0_f32.lerp(0.75_f32.lerp(0.25, num), num2);
        if p.si_clouds {
            y2 += -100.0 + 200.0 * num2;
            num4 = 1.0;
        }
        let image = Self::cloud_image(index);
        let mut cloud = Piece::new(image, Shader::Cloud);
        cloud.anchor_y = 0.0;
        cloud.scale = Vec2::new(num3, num4 * num3);
        cloud.color = Color::rgb(num2 * 0.75, random_offset, num4.lerp(1.0, 0.5));
        cloud.pos = Vec2::new(683.0, (y2 - self.size_of(image).y * cloud.scale.y).floor());
        let fill = self.cloud_fill(
            cloud.pos.y + 1.0,
            self.view.sky_color.lerp(self.atmosphere(), num2 * 0.75),
        );
        vec![fill, cloud]
    }

    /// `DistantCloud.DrawSprites`: hidden near the ground (`InverseLerp` under 0.33).
    fn distant_cloud(&self, index: usize, random_offset: f32, d: f32) -> Vec<Piece> {
        if self.altitude(true) < 0.33 {
            return Vec::new();
        }
        let num = 2.0;
        let y = self.draw_pos_shifted().y;
        let mut num2 = 0.3_f32.lerp(0.01, d);
        if index == 8 {
            num2 *= 1.5;
        }
        let image = Self::cloud_image(index);
        let mut cloud = Piece::new(image, Shader::CloudDistant);
        cloud.anchor_y = 0.0;
        cloud.scale = Vec2::new(num, num2 * num);
        cloud.color = Color::rgb(0.75_f32.lerp(0.95, d), random_offset, num2.lerp(1.0, 0.5));
        cloud.pos = Vec2::new(683.0, (y - self.size_of(image).y * cloud.scale.y).floor());
        let fill = self.cloud_fill(
            cloud.pos.y + 1.0,
            self.view
                .sky_color
                .lerp(self.atmosphere(), 0.75_f32.lerp(0.95, d)),
        );
        vec![fill, cloud]
    }

    /// `FlyingCloud.DrawSprites`: hidden near the ground.
    fn flying_cloud(
        &self,
        random_offset: f32,
        flattened: f32,
        alpha: f32,
        shader_input_color: f32,
    ) -> Vec<Piece> {
        if self.altitude(false) < 0.33 {
            return Vec::new();
        }
        let mut cloud = Piece::new(rw_sim::SceneImages::FLYING_CLOUDS, Shader::CloudDistant);
        cloud.anchor_y = 1.0;
        cloud.scale = Vec2::new(2.0, flattened * 2.0);
        cloud.color = Color {
            r: shader_input_color,
            g: random_offset,
            b: flattened.lerp(1.0, 0.5),
            a: alpha,
        };
        cloud.pos = Vec2::new(683.0, self.draw_pos(self.view.cam_pos).y);
        vec![cloud]
    }

    /// `RoofTopView.Floor.DrawSprites` (`_WorldCamPos` is set by the caller).
    fn floor(&self, asset: &'static str, from_depth: f32, to_depth: f32) -> Vec<Piece> {
        let size = self.size_of(asset);
        let cam = self.view.cam_pos;
        let near = self
            .scene
            .draw_pos(self.element.pos, to_depth, cam, self.view.h_displace);
        let far = self
            .scene
            .draw_pos(self.element.pos, from_depth, cam, self.view.h_displace);
        let mut p = Piece::new(asset, Shader::BkgFloor);
        p.anchor_y = 1.0;
        p.scale = Vec2::new(1400.0 / size.x, (near.y - far.y) / size.y);
        p.pos = Vec2::new(self.view.screen.x / 2.0, near.y);
        p.color = Color::rgb(1.0 / from_depth, 1.0 / to_depth, 0.0);
        vec![p]
    }

    /// `RoofTopView.Building.DrawSprites`.
    fn building(&self, asset: &'static str, scale: f32) -> Vec<Piece> {
        let size = self.size_of(asset);
        let mut p = Piece::new(asset, Shader::House);
        p.anchor_y = 0.0;
        p.scale = Vec2::new(scale, scale);
        p.pos = self.draw_pos(self.view.cam_pos);
        p.color = Color::rgb(
            size.x * scale / 4000.0,
            size.y * scale / 1500.0,
            1.0 / self.element.depth,
        );
        vec![p]
    }

    /// `RoofTopView.Rubble.DrawSprites`.
    fn rubble(&self, asset: &'static str, random_value: f32) -> Vec<Piece> {
        let depth = self.element.depth;
        let mut p = Piece::new(asset, Shader::DistantBkgObjectRepeatHorizontal);
        p.anchor_y = 0.0;
        p.scale = Vec2::new(1.0, 1.0 / depth);
        p.pos = Vec2::new(self.view.screen.x / 2.0, self.draw_pos(self.view.cam_pos).y);
        p.color = Color {
            r: BackgroundScene::atmosphere_color_at_depth(depth),
            g: 1.0 / depth,
            b: 1.0,
            a: random_value,
        };
        vec![p]
    }

    /// `RoofTopView.DistantBuilding.DrawSprites`.
    fn roof_distant_building(&self, asset: &'static str) -> Vec<Piece> {
        let mut p = Piece::new(asset, Shader::DistantBkgObject);
        p.anchor_y = 0.0;
        p.pos = self.draw_pos(self.view.cam_pos);
        p.color = Color::rgb(
            BackgroundScene::atmosphere_color_at_depth(self.element.depth),
            0.0,
            0.0,
        );
        vec![p]
    }

    /// `RoofTopView.Smoke.DrawSprites`.
    fn smoke(
        &self,
        flattened: f32,
        alpha: f32,
        shader_input_color: f32,
        random_offset: f32,
        dust: bool,
    ) -> Vec<Piece> {
        let mut p = Piece::new(
            rw_sim::SceneImages::SMOKE,
            if dust {
                Shader::Dust
            } else {
                Shader::CloudDistant
            },
        );
        p.anchor_y = 0.0;
        p.scale = Vec2::new(2.0, flattened * 2.0);
        p.color = Color {
            r: shader_input_color,
            g: random_offset,
            b: flattened.lerp(1.0, 0.5),
            a: alpha,
        };
        p.pos = Vec2::new(683.0, self.draw_pos(self.view.cam_pos).y);
        vec![p]
    }
}

#[cfg(test)]
#[path = "background_scene_tests.rs"]
mod tests;
