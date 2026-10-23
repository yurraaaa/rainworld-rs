//! `RoofTopView`'s constructor: the sky, the floor, 16 strips of rubble, the city's
//! buildings and four smoke layers of a room such as `uw_h01`. The four `Smoke`
//! constructors draw their `randomOffset` on the shared stream (D1), in element order;
//! the rubble's `SeededRandom(i)` leaves the stream as it was.
//!
//! `RoofTopView.DustpuffSpawner` and the `DustPuff`s it makes: the player's chunks
//! landing on or running along a floor raise puffs of dust that drift up and fade. The
//! spawner is made by `RoofTopView`'s constructor (and by a `Dustpuffs` effect, which no
//! base room has). Draws on the shared stream (D1): one for each chunk on the floor
//! that is not landing (the 1-in-10 test), then, for the puff that test lets through,
//! one for its size and one for its life time; one for a landing's life time.
//! Sprites: `rw-graphics` `effects/sky/background_scene.rs`.
//!
//! Base game only: the Lantern-at-the-Edge city (`isLC`), the Dust Storm waves
//! (`room.dustStorm`) and the `_DM` images are omitted (D32).
//!
//! Mirrors: RoofTopView.cs (constructor, AtmosphereColorAtDepth, DustpuffSpawner, DustPuff), CosmeticSprite.cs (Update)

use rw_random::UnityRandom;

use crate::effects::sky::background_scene::{
    BackgroundScene, ElementKind, SceneElement, SceneParams, SkyTime,
};
use crate::{BodyChunk, FloatExt, Vec2};

impl BackgroundScene {
    /// `RoofTopView.floorLevel`.
    pub const ROOF_FLOOR_LEVEL: f32 = 26.0;

    /// `new RoofTopView(room, effect)` in a room of `size` tiles whose origin is at
    /// `room_origin` in the region's world; `rng` is the shared stream.
    #[must_use]
    pub fn roof_top_view(room_origin: Vec2, size: (i32, i32), rng: &mut UnityRandom) -> Self {
        let mut scene = Self::empty(SceneParams::RoofTop, room_origin);
        let floor = Self::ROOF_FLOOR_LEVEL;
        scene.scene_origo = scene.room_to_world_pos(Vec2::new(size.0 as f32, size.1 as f32) * 10.0);
        for (name, time) in [
            ("Rf_NightSky", SkyTime::Night),
            ("Rf_DuskSky", SkyTime::Dusk),
            ("Rf_Sky", SkyTime::Day),
        ] {
            scene.add_element(SceneElement::new(
                Vec2::new(683.0, 384.0),
                f32::MAX,
                ElementKind::Sky {
                    name,
                    time,
                    alpha: 1.0,
                },
            ));
        }
        scene.add_element(SceneElement::new(
            Vec2::new(0.0, floor),
            12.0,
            ElementKind::Floor {
                asset: "floor",
                from_depth: 1.0,
                to_depth: 12.0,
            },
        ));
        for i in 0..16 {
            let f = i as f32 / 15.0;
            scene.add_element(SceneElement::new(
                Vec2::new(0.0, floor),
                1.5_f32.lerp(8.0, f.mathf_pow(1.5)),
                ElementKind::Rubble {
                    asset: "Rf_Rubble",
                    random_value: UnityRandom::new(i).value(),
                },
            ));
        }
        scene.add_element(SceneElement::new(
            Vec2::new(-2676.0, 9.0),
            1.0,
            ElementKind::RoofDistantBuilding {
                asset: "Rf_HoleFix",
            },
        ));
        for (asset, x, depth) in [("city2", 1780.0, 11.5), ("city1", 880.0, 10.5)] {
            let pos = scene.at_neutral_x(x, depth, floor);
            scene.add_element(SceneElement::new(
                pos,
                depth,
                ElementKind::Building { asset, scale: 3.0 },
            ));
        }
        for (asset, x, y, depth) in [
            ("RF_CityA", 300.0, floor - 25.5, 8.5),
            ("RF_CityB", 515.0, floor - 13.0, 6.5),
            ("RF_CityC", 400.0, floor - 8.5, 5.0),
        ] {
            let pos = scene.at_neutral_x(x, depth, y);
            scene.add_element(SceneElement::new(
                pos,
                depth,
                ElementKind::RoofDistantBuilding { asset },
            ));
        }
        for (y, depth, flattened, shader_input_color, dust) in [
            (floor + 560.0, 7.0, 2.5, 0.8, false),
            (floor, 4.2, 0.2, 0.0, true),
            (floor + 28.0, 2.0, 0.5, 0.0, true),
            (floor + 14.0, 1.2, 0.75, 0.0, true),
        ] {
            scene.add_element(SceneElement::new(
                Vec2::new(0.0, y),
                depth,
                ElementKind::Smoke {
                    flattened,
                    alpha: 0.1,
                    shader_input_color,
                    random_offset: rng.value(),
                    dust,
                },
            ));
        }
        scene
    }

    /// `(PosFromDrawPosAtNeutralCamPos((x, 0), depth).x, y)`.
    fn at_neutral_x(&self, x: f32, depth: f32, y: f32) -> Vec2 {
        Vec2::new(
            self.pos_from_draw_pos_at_neutral_cam_pos(Vec2::new(x, 0.0), depth)
                .x,
            y,
        )
    }

    /// `AtmosphereColorAtDepth`.
    #[must_use]
    pub fn atmosphere_color_at_depth(depth: f32) -> f32 {
        (depth / 15.0).clamp(0.0, 1.0) * 0.9
    }
}

/// A `DustPuff` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct DustPuff {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub life: f32,
    pub last_life: f32,
    pub life_time: f32,
    pub size: f32,
    /// `motion` (`(0.5, 0.25)`; `room.snow` zeroes it, which no base room has).
    pub motion: Vec2,
    pub slated: bool,
}

impl DustPuff {
    /// `new DustPuff(pos, size)`: its life time draws once.
    pub fn new(pos: Vec2, size: f32, rng: &mut UnityRandom) -> Self {
        Self {
            update_order: 0,
            pos,
            last_pos: pos,
            life: 1.0,
            last_life: 1.0,
            life_time: 40.0_f32.lerp(120.0, rng.value()) * 0.5_f32.lerp(1.5, size),
            size,
            motion: Vec2::new(0.5, 0.25),
            slated: false,
        }
    }

    /// `Update`: `CosmeticSprite.Update` (no velocity), the drift (`motion`'s x up, its y
    /// sideways), the life; destroyed the tick after the life passed 0.
    pub fn update(&mut self) {
        self.last_pos = self.pos;
        self.pos.y += self.motion.x;
        self.pos.x += self.motion.y;
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        if self.last_life < 0.0 {
            self.slated = true;
        }
    }
}

/// `DustpuffSpawner`, a room object with no state of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct DustpuffSpawner {
    /// Its place in the room's update list.
    pub update_order: u64,
}

impl DustpuffSpawner {
    /// `Update` for one player's chunks (in `bodyChunks` order): the puffs made, in order,
    /// each with its constructor's draw made.
    pub fn update(chunks: &[BodyChunk], rng: &mut UnityRandom) -> Vec<DustPuff> {
        let mut puffs = Vec::new();
        for c in chunks.iter().filter(|c| c.contact().y < 0) {
            let at = c.pos + Vec2::new(0.0, -c.rad);
            let fall = c.last_pos.y - c.pos.y;
            if c.last_contact().y >= 0 && fall > 5.0 {
                puffs.push(DustPuff::new(at, fall.lerp_map(5.0, 10.0, 0.5, 1.0), rng));
            } else if rng.value() < 0.1 && (c.last_pos.x - c.pos.x).abs() > 3.0 {
                let size = 0.25 * rng.value();
                puffs.push(DustPuff::new(at, size, rng));
            }
        }
        puffs
    }
}

#[cfg(test)]
#[path = "tests/roof_top_view.rs"]
mod tests;
