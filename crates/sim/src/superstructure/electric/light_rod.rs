//! `SSLightRod`: a placed rod of light in the superstructure. Its `LightSource`s, one for
//! every 45 px of length (2 to 30), each ride along the rod at their own speed, fading in
//! and out with `sin(progression * pi)^0.5` and by how much of the rod's depth the
//! camera sees at the light (7 samples), scaled by the room's electric power. Every draw
//! is on the shared stream: three a light when the rod is made (`Reset`'s two, then the
//! starting progression) and two each time one wraps. The sprite is `rw-graphics`'.
//! Base game only: the More Slugcats colours of the rod's lights are a DLC branch (D36).
//!
//! Mirrors: SSLightRod.cs

use rw_assets::SsLightRodData;
use rw_random::UnityRandom;

use crate::math::float_ext::MathfSin;
use crate::{CameraImage, CameraView, FloatExt, LightSource, LightTie, Vec2};

/// `SSLightRod.color` outside the More Slugcats regions (and in an arena session).
pub const ROD_COLOR: [f32; 3] = [0.4, 1.0, 0.8];

/// What a light reads of camera 0: whether it sees a position and the depth under it.
pub struct RodCamera<'a> {
    pub view: CameraView,
    pub room_index: i32,
    pub room: &'a rw_assets::Room,
    pub image: Option<&'a CameraImage>,
}

impl RodCamera<'_> {
    /// `room.ViewedByAnyCamera(pos, margin)`.
    #[must_use]
    pub fn viewed(&self, pos: Vec2, margin: f32) -> bool {
        self.view.viewed(self.room_index, self.room, pos, margin)
    }

    /// `cameras[0].DepthAtCoordinate(coord)`; sky (1) when the camera has no picture.
    #[must_use]
    pub fn depth_at(&self, coord: Vec2) -> f32 {
        self.image.map_or(1.0, |i| i.depth_at_coordinate(coord))
    }
}

/// `SSLightRod.LightVessel`: one light riding the rod.
#[derive(Debug, Clone, PartialEq)]
pub struct LightVessel {
    /// Its `LightSource`'s id.
    pub light: u64,
    pub progression: f32,
    pub speed: f32,
    pub strength: f32,
    pub size: f32,
    pub visible: f32,
}

impl LightVessel {
    /// The constructor after the light exists: `Reset` (speed, size), then a random
    /// progression.
    fn new(light: u64, rng: &mut UnityRandom) -> Self {
        let mut v = Self {
            light,
            progression: 0.0,
            speed: 0.0,
            strength: 0.0,
            size: 0.0,
            visible: 0.0,
        };
        v.reset(rng);
        v.progression = rng.value();
        v
    }

    fn reset(&mut self, rng: &mut UnityRandom) {
        self.progression = 0.0;
        self.speed = 0.5f32.lerp(2.0, rng.value());
        self.size = rng.value();
    }
}

/// One `SSLightRod`.
#[derive(Debug, Clone, PartialEq)]
pub struct SsLightRod {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`.
    pub pos: Vec2,
    pub data: SsLightRodData,
    pub lights: Vec<LightVessel>,
}

impl SsLightRod {
    /// `UpdateLightAmount`'s count: `IntClamp((int)(length / 45), 2, 30)`.
    #[must_use]
    pub fn light_count(length: f32) -> usize {
        (length / 45.0).to_int_cs().clamp(2, 30) as usize
    }

    /// The constructor: a vessel for each of `lights` (the ids of the `LightSource`s the
    /// game made for them, from [`Self::light_source`]), in order.
    #[must_use]
    pub fn new(
        update_order: u64,
        pos: Vec2,
        data: SsLightRodData,
        lights: &[u64],
        rng: &mut UnityRandom,
    ) -> Self {
        Self {
            update_order,
            pos,
            data,
            lights: lights.iter().map(|&l| LightVessel::new(l, rng)).collect(),
        }
    }

    /// A vessel's `new LightSource(placedObject.pos, false, rod.color, rod)`.
    #[must_use]
    pub fn light_source(id: u64, room: i32, rod_order: u64, pos: Vec2) -> LightSource {
        LightSource::new(
            id,
            room,
            pos,
            false,
            ROD_COLOR,
            Some(LightTie::Fixture(rod_order)),
        )
    }

    /// `Update`: each vessel's, in order (`electric_power` is `Room.ElectricPower`). The
    /// rebuild when the data's length changes is the developer tools' (the data is fixed).
    pub fn update(
        &mut self,
        lights: &mut [LightSource],
        electric_power: f32,
        cam: &RodCamera<'_>,
        rng: &mut UnityRandom,
    ) {
        let (pos, data) = (self.pos, self.data);
        for v in &mut self.lights {
            Self::update_vessel(v, pos, data, electric_power, cam, lights, rng);
        }
    }

    fn update_vessel(
        v: &mut LightVessel,
        pos: Vec2,
        data: SsLightRodData,
        electric_power: f32,
        cam: &RodCamera<'_>,
        lights: &mut [LightSource],
        rng: &mut UnityRandom,
    ) {
        v.strength = (v.progression * std::f32::consts::PI)
            .mathf_sin()
            .mathf_pow(0.5)
            .inverse_lerp(0.1, 1.0);
        let dir = Vec2::from_degrees(data.rotation);
        let at = pos + dir * (v.progression * data.length);
        let mut b = 0.7;
        if cam.viewed(at, 100.0) {
            b = 0.0;
            for i in -3..4 {
                let coord = at + dir * (2.0 * i as f32);
                if cam.depth_at(coord) >= data.depth {
                    b += 1.0;
                }
            }
            b /= 7.0;
        }
        v.visible = v.visible.lerp(b, 0.2f32.lerp(0.05, data.brightness));
        v.strength *= v.visible;
        if let Some(l) = LightSource::find_mut(lights, v.light) {
            l.set_alpha = Some(1.0f32.lerp(0.5, v.size) * v.strength * electric_power);
            l.set_pos = Some(at);
            l.set_rad =
                Some(100.0f32.lerp(400.0, v.size) * v.visible * 0.2f32.lerp(1.5, data.brightness));
        }
        v.progression = (v.progression + v.speed / data.length).min(1.0);
        if v.progression >= 1.0 {
            v.reset(rng);
        }
    }
}

#[cfg(test)]
#[path = "tests/light_rod.rs"]
mod tests;
