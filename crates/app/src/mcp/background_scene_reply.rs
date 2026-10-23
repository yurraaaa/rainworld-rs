//! What `get_state` reports of the realized rooms' background scenes
//! (`AboveCloudsView`, `RoofTopView`): the kind, the elements, where the camera is on
//! the scene and each `DistantLightning`'s state.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{BackgroundScene, DistantLightning, ElementKind, SceneKind, SceneParams, Vec2};
use serde::Serialize;

/// Where the camera is: its room and `RoomToWorldPos` of the shown camera's position and
/// of the window.
#[derive(Debug, Clone, Copy)]
pub struct CameraOnScene {
    pub room: i32,
    /// `room.cameraPositions[currentCameraPosition]`.
    pub camera_position: Vec2,
    /// `camPos`, the window's position in the room.
    pub window: Vec2,
}

/// One `DistantLightning`.
#[derive(Debug, Clone, Serialize)]
pub struct DistantLightningState {
    /// Its place in the update list.
    pub id: u64,
    pub asset: &'static str,
    /// `depth` (the first turn adds `minusDepthForLayering` back).
    pub depth: f32,
    pub wait: i32,
    pub thunder: i32,
    pub thunder_length: i32,
    pub tiny_thunder_wait: i32,
    pub tiny_thunder: i32,
    pub tiny_thunder_length: i32,
    pub random_level: f32,
    pub random_level_change: i32,
    pub power: f32,
    pub last_intensity: f32,
    pub intensity: f32,
}

/// One scene.
#[derive(Debug, Clone, Serialize)]
pub struct BackgroundSceneState {
    pub room: String,
    /// `AboveCloudsView` or `RoofTopView`.
    pub kind: &'static str,
    /// Its place in the update list.
    pub id: u64,
    pub elements: usize,
    /// The first turn has added the elements to the room.
    pub elements_added: bool,
    pub scene_origo: [f32; 2],
    pub convergence_point: [f32; 2],
    pub atmosphere_color: [f32; 3],
    /// `RoomToWorldPos((0, 0))`.
    pub room_origin: [f32; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_altitude: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_altitude: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_clouds: Option<bool>,
    /// The camera room's: `RoomToWorldPos` of the shown camera's position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_world: Option<[f32; 2]>,
    /// The camera room's: `RoomToWorldPos(camPos)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_world: Option<[f32; 2]>,
    /// The camera room's, of an `AboveCloudsView`: `InverseLerp(startAltitude,
    /// endAltitude, camera_world.y)`, which the clouds and lightning read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lightning: Vec<DistantLightningState>,
}

impl BackgroundSceneState {
    /// Every realized room's scenes.
    #[must_use]
    pub fn all(world: &World, camera: Option<CameraOnScene>) -> Vec<Self> {
        let mut states = Vec::new();
        for index in world.active_rooms() {
            let Some(r) = world.realized(index) else {
                continue;
            };
            let name = world
                .abstract_room(index)
                .map_or_else(|| format!("#{index}"), |a| a.name.clone());
            let here = camera.filter(|c| c.room == index);
            for scene in &r.background_scenes {
                states.push(Self::of(name.clone(), scene, here));
            }
        }
        states
    }

    fn of(room: String, scene: &BackgroundScene, camera: Option<CameraOnScene>) -> Self {
        let camera_world = camera.map(|c| scene.room_to_world_pos(c.camera_position));
        let params = match &scene.params {
            SceneParams::AboveClouds(p) => Some(p),
            SceneParams::RoofTop => None,
        };
        Self {
            room,
            kind: match scene.kind() {
                SceneKind::AboveCloudsView => "AboveCloudsView",
                SceneKind::RoofTopView => "RoofTopView",
            },
            id: scene.update_order,
            elements: scene.elements.len(),
            elements_added: scene.elements_added,
            scene_origo: scene.scene_origo.to_array(),
            convergence_point: scene.convergence_point.to_array(),
            atmosphere_color: scene.atmosphere_color,
            room_origin: scene.room_origin.to_array(),
            start_altitude: params.map(|p| p.start_altitude),
            end_altitude: params.map(|p| p.end_altitude),
            si_clouds: params.map(|p| p.si_clouds),
            camera_world: camera_world.map(Vec2::to_array),
            window_world: camera.map(|c| scene.room_to_world_pos(c.window).to_array()),
            altitude: camera_world.and_then(|w| scene.altitude_fraction(w.y)),
            lightning: scene
                .elements
                .iter()
                .filter_map(|e| match &e.kind {
                    ElementKind::DistantLightning(l) => Some(Self::lightning(e.update_order, e.depth, l)),
                    _ => None,
                })
                .collect(),
        }
    }

    fn lightning(id: u64, depth: f32, l: &DistantLightning) -> DistantLightningState {
        DistantLightningState {
            id,
            asset: l.asset_name,
            depth,
            wait: l.wait,
            thunder: l.thunder,
            thunder_length: l.thunder_length,
            tiny_thunder_wait: l.tiny_thunder_wait,
            tiny_thunder: l.tiny_thunder,
            tiny_thunder_length: l.tiny_thunder_length,
            random_level: l.random_level,
            random_level_change: l.random_level_change,
            power: l.power,
            last_intensity: l.last_intensity,
            intensity: l.intensity,
        }
    }
}
