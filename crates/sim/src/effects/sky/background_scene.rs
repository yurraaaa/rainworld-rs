//! `BackgroundScene`: a room's parallax backdrop (`AboveCloudsView`, `RoofTopView`) as
//! objects of the room. The scene is in the room's update list from `Room.Loaded`; its
//! first turn sorts its elements by depth, far to near, and adds them to the room (each
//! one a place in the update list and a sprite leaser of its own). The elements'
//! positions and sprites are the graphics crate's (`rw-graphics` `background_scene.rs`);
//! only [`DistantLightning`] has a turn of its own.
//!
//! The scenes are made by [`crate::effects::sky::above_clouds_view`] and [`crate::effects::sky::roof_top_view`];
//! omitted branches: D32.
//!
//! Mirrors: BackgroundScene.cs, AboveCloudsView.cs (Cloud, DistantBuilding, Fog fields), RoofTopView.cs (element fields)

use crate::{FloatExt, Vec2};

pub use crate::effects::sky::above_clouds_view::DistantLightning;

/// The default `rainWorld.screenSize` (`Options.resolution` 1); a world starts with it
/// until the app sets the chosen one.
pub const SCREEN_SIZE: Vec2 = Vec2::new(1366.0, 768.0);

/// Which scene a room has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneKind {
    AboveCloudsView,
    RoofTopView,
}

/// A sky illustration (`Simple2DBackgroundIllustration`) by time of day; the day
/// branches of the scenes (`DayNight`) only change their alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkyTime {
    Night,
    Dusk,
    Day,
}

/// `AboveCloudsView.Cloud`'s shared fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudData {
    /// `randomOffset`: a draw of the constructor's private stream.
    pub random_offset: f32,
    pub index: usize,
}

/// What a scene element is, with the fields its sprites read.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementKind {
    /// `Simple2DBackgroundIllustration` (depth `float.MaxValue`).
    Sky {
        name: &'static str,
        time: SkyTime,
        alpha: f32,
    },
    /// `AboveCloudsView.Fog`.
    Fog,
    /// `AboveCloudsView.DistantBuilding`.
    DistantBuilding {
        asset: &'static str,
        atmospheric_depth_add: f32,
        alpha: f32,
        scale: f32,
    },
    /// `AboveCloudsView.DistantLightning`.
    DistantLightning(DistantLightning),
    /// `AboveCloudsView.CloseCloud`; the game's draws mutate its `depth`; the port derives it per draw.
    CloseCloud { cloud: CloudData, cloud_depth: f32 },
    /// `AboveCloudsView.DistantCloud`.
    DistantCloud {
        cloud: CloudData,
        distant_cloud_depth: f32,
    },
    /// `AboveCloudsView.FlyingCloud`.
    FlyingCloud {
        cloud: CloudData,
        flattened: f32,
        alpha: f32,
        shader_input_color: f32,
    },
    /// `RoofTopView.Floor`.
    Floor {
        asset: &'static str,
        from_depth: f32,
        to_depth: f32,
    },
    /// `RoofTopView.Building`.
    Building { asset: &'static str, scale: f32 },
    /// `RoofTopView.Rubble`; `random_value` is `game.SeededRandom(index)`.
    Rubble {
        asset: &'static str,
        random_value: f32,
    },
    /// `RoofTopView.DistantBuilding`.
    RoofDistantBuilding { asset: &'static str },
    /// `RoofTopView.Smoke`.
    Smoke {
        flattened: f32,
        alpha: f32,
        shader_input_color: f32,
        random_offset: f32,
        /// `shaderType`: the `Dust` shader, else `CloudDistant`.
        dust: bool,
    },
}

/// A `BackgroundSceneElement`.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneElement {
    /// Its place in the room's update list; 0 until the scene's first turn adds it.
    pub update_order: u64,
    pub pos: Vec2,
    pub depth: f32,
    pub kind: ElementKind,
}

impl SceneElement {
    #[must_use]
    pub fn new(pos: Vec2, depth: f32, kind: ElementKind) -> Self {
        Self {
            update_order: 0,
            pos,
            depth,
            kind,
        }
    }

    /// The image the element's sprite is made from, if it has one.
    #[must_use]
    pub fn asset(&self) -> Option<&'static str> {
        match &self.kind {
            ElementKind::Sky { name, .. } => Some(name),
            ElementKind::DistantBuilding { asset, .. }
            | ElementKind::Floor { asset, .. }
            | ElementKind::Building { asset, .. }
            | ElementKind::Rubble { asset, .. }
            | ElementKind::RoofDistantBuilding { asset } => Some(asset),
            ElementKind::DistantLightning(l) => Some(l.asset_name),
            ElementKind::CloseCloud { cloud, .. } | ElementKind::DistantCloud { cloud, .. } => {
                Some(SceneImages::CLOUDS[cloud.index % 3])
            }
            ElementKind::FlyingCloud { .. } => Some(SceneImages::FLYING_CLOUDS),
            ElementKind::Smoke { .. } => Some(SceneImages::SMOKE),
            ElementKind::Fog => None,
        }
    }
}

/// The images the scenes load by name (`LoadGraphic`).
pub struct SceneImages;

impl SceneImages {
    pub const CLOUDS: [&'static str; 3] = ["clouds1", "clouds2", "clouds3"];
    pub const FLYING_CLOUDS: &'static str = "flyingClouds1";
    pub const SMOKE: &'static str = "smoke1";
}

/// What the scene knows of its kind.
#[derive(Debug, Clone, PartialEq)]
pub enum SceneParams {
    AboveClouds(AboveCloudsParams),
    RoofTop,
}

/// `AboveCloudsView`'s fields.
#[derive(Debug, Clone, PartialEq)]
pub struct AboveCloudsParams {
    pub start_altitude: f32,
    pub end_altitude: f32,
    pub clouds_start_depth: f32,
    pub clouds_end_depth: f32,
    pub distant_clouds_end_depth: f32,
    /// `SIClouds`.
    pub si_clouds: bool,
    /// `IsArenaSession`.
    pub arena: bool,
    /// `yShift` (More Slugcats' ending scroll; 0).
    pub y_shift: f32,
}

/// A `BackgroundScene` object of a room.
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundScene {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub params: SceneParams,
    pub elements: Vec<SceneElement>,
    pub scene_origo: Vec2,
    /// Zero until the first turn (`Update` sets it), which is what
    /// [`Self::pos_from_draw_pos_at_neutral_cam_pos`] sees in the constructors.
    pub convergence_point: Vec2,
    pub elements_added: bool,
    /// `RoomToWorldPos((0, 0))`: where the room's origin lies in the region's world.
    pub room_origin: Vec2,
    /// `atmosphereColor` (`_AboveCloudsAtmosphereColor`).
    pub atmosphere_color: [f32; 3],
}

impl BackgroundScene {
    /// `atmosphereColor`'s default, also `_AboveCloudsAtmosphereColor`'s.
    pub const ATMOSPHERE_COLOR: [f32; 3] = [41.0 / 255.0, 59.0 / 255.0, 81.0 / 255.0];

    #[must_use]
    pub fn kind(&self) -> SceneKind {
        match self.params {
            SceneParams::AboveClouds(_) => SceneKind::AboveCloudsView,
            SceneParams::RoofTop => SceneKind::RoofTopView,
        }
    }

    pub(crate) fn empty(params: SceneParams, room_origin: Vec2) -> Self {
        Self {
            update_order: 0,
            params,
            elements: Vec::new(),
            scene_origo: Vec2::ZERO,
            convergence_point: Vec2::ZERO,
            elements_added: false,
            room_origin,
            atmosphere_color: Self::ATMOSPHERE_COLOR,
        }
    }

    /// `RoomToWorldPos`.
    #[must_use]
    pub fn room_to_world_pos(&self, in_room: Vec2) -> Vec2 {
        self.room_origin + in_room
    }

    /// `DrawPos`: where an element at `pos` and `depth` lies on the screen when the
    /// camera is at `cam_pos`.
    #[must_use]
    pub fn draw_pos(&self, pos: Vec2, depth: f32, cam_pos: Vec2, h_displace: f32) -> Vec2 {
        let mut v = pos + self.scene_origo - self.room_to_world_pos(cam_pos);
        v.x += h_displace - 8.0;
        (v - self.convergence_point) / depth + self.convergence_point
    }

    /// `PosFromDrawPosAtNeutralCamPos`.
    #[must_use]
    pub fn pos_from_draw_pos_at_neutral_cam_pos(&self, input: Vec2, depth: f32) -> Vec2 {
        let v = self.scene_origo - self.convergence_point;
        (self.scene_origo + input - v - self.convergence_point) * depth + self.convergence_point
    }

    /// `AddElement`.
    pub(crate) fn add_element(&mut self, element: SceneElement) {
        self.elements.push(element);
    }

    /// The element indices in the order the first turn adds them to the room
    /// (`OrderByDescending(depth)`, stable).
    #[must_use]
    pub fn add_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.elements.len()).collect();
        order.sort_by(|&a, &b| self.elements[b].depth.total_cmp(&self.elements[a].depth));
        order
    }

    /// The indices of the elements that have joined the room, in the order they were
    /// added (the sprite leaser's order, which `DrawSprites` runs in).
    #[must_use]
    pub fn added_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.elements.len())
            .filter(|&i| self.elements[i].update_order != 0)
            .collect();
        order.sort_by_key(|&i| self.elements[i].update_order);
        order
    }

    /// The convergence point the first turn sets (`waterInverted` is More Slugcats').
    #[must_use]
    pub fn convergence_for(screen: Vec2) -> Vec2 {
        Vec2::new(screen.x / 2.0, screen.y * 2.0 / 3.0)
    }

    /// The altitude fraction `Mathf.InverseLerp(startAltitude, endAltitude, y)` of
    /// an `AboveCloudsView`.
    #[must_use]
    pub fn altitude_fraction(&self, y: f32) -> Option<f32> {
        match &self.params {
            SceneParams::AboveClouds(p) => Some(y.inverse_lerp(p.start_altitude, p.end_altitude)),
            SceneParams::RoofTop => None,
        }
    }

    /// The distinct images the scene's elements are made from, in element order.
    #[must_use]
    pub fn image_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        for name in self.elements.iter().filter_map(SceneElement::asset) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }
}

#[cfg(test)]
#[path = "tests/background_scene.rs"]
mod tests;
