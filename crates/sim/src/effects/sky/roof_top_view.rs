//! `RoofTopView`'s constructor: the sky, the floor, 16 strips of rubble, the city's
//! buildings and four smoke layers of a room such as `uw_h01`. The four `Smoke`
//! constructors draw their `randomOffset` on the shared stream (D1), in element order;
//! the rubble's `SeededRandom(i)` leaves the stream as it was. The dust puffs the
//! constructor's `DustpuffSpawner` makes are in [`crate::effects::sky::dust_puff`].
//!
//! Base game only: the Lantern-at-the-Edge city (`isLC`), the Dust Storm waves
//! (`room.dustStorm`) and the `_DM` images are omitted (D32).
//!
//! Mirrors: RoofTopView.cs (constructor, AtmosphereColorAtDepth)

use rw_random::UnityRandom;

use crate::effects::sky::background_scene::{
    BackgroundScene, ElementKind, SceneElement, SceneParams, SkyTime,
};
use crate::{FloatExt, Vec2};

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
