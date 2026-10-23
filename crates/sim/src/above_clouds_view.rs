//! `AboveCloudsView`'s constructor: which elements a room's scene has, their depths and
//! places. The clouds' `randomOffset` draws run under `Random.InitState(12)` with the
//! shared state restored at the end, so the constructor draws nothing on the shared
//! stream (D1); the scene's per-tick draws are [`crate::DistantLightning`]'s.
//!
//! Base game only: `PinkSky` (Watcher), the More Slugcats regions (`OE`, `LM`, `SL`,
//! the `SI_A07` clouds, the ending scroll `yShift`) and the Saint timeline's buildings
//! are omitted (D31).
//!
//! Mirrors: AboveCloudsView.cs (constructor, CloudDepth, DistantCloudDepth)

use rw_random::UnityRandom;

use crate::FloatExt;
use crate::Vec2;
use crate::background_scene::{
    AboveCloudsParams, BackgroundScene, CloudData, DistantLightning, ElementKind, SceneElement,
    SceneParams, SkyTime,
};

/// What the constructor reads of the room and world.
#[derive(Debug, Clone, Copy)]
pub struct AboveCloudsContext<'a> {
    /// `world.region.name`.
    pub region: Option<&'a str>,
    /// `abstractRoom.name`.
    pub room_name: &'a str,
    /// `RoomToWorldPos((0, 0))`.
    pub room_origin: Vec2,
    /// `IsArenaSession`.
    pub arena: bool,
    /// `effect.amount`.
    pub effect_amount: f32,
    /// `regionParams.cloudsStart` and `cloudsEnd`.
    pub clouds: (Option<f32>, Option<f32>),
}

/// One building row of the constructor: image, draw offset, depth and atmospheric add.
struct Building {
    asset: &'static str,
    at: (f32, f32),
    depth: f32,
    add: f32,
    /// The lightning's image and its `minusDepthForLayering`, made after the building.
    light: Option<(&'static str, f32, f32)>,
}

impl Building {
    const fn new(asset: &'static str, at: (f32, f32), depth: f32, add: f32) -> Self {
        Self {
            asset,
            at,
            depth,
            add,
            light: None,
        }
    }

    /// With a `DistantLightning` of `asset`, `minus_depth_for_layering` and a `y`
    /// offset of the bolt's draw position.
    const fn lit(mut self, asset: &'static str, minus_depth: f32, y: f32) -> Self {
        self.light = Some((asset, minus_depth, y));
        self
    }
}

impl BackgroundScene {
    /// `new AboveCloudsView(room, effect)`.
    #[must_use]
    pub fn above_clouds_view(cx: &AboveCloudsContext<'_>) -> Self {
        let mut rng = UnityRandom::new(12);
        let mut si_clouds = cx.region == Some("SI") || Self::name_starts_with(cx.room_name, "SI_");
        let (mut start, mut end) = if si_clouds {
            (9000.0, 26400.0)
        } else {
            (20000.0, 31400.0)
        };
        start = cx.clouds.0.unwrap_or(start);
        end = cx.clouds.1.unwrap_or(end);
        let mut scene_origo = Vec2::new(2514.0, (start + end) / 2.0);
        if cx.arena {
            let y = 10000.0 - cx.effect_amount * 30000.0;
            scene_origo = Vec2::new(2514.0, y);
            start = y - 5500.0;
            end = y + 5500.0;
            si_clouds = false;
        }
        let params = AboveCloudsParams {
            start_altitude: start,
            end_altitude: end,
            clouds_start_depth: 5.0,
            clouds_end_depth: 40.0,
            distant_clouds_end_depth: 200.0,
            si_clouds,
            arena: cx.arena,
            y_shift: 0.0,
        };
        let mut scene = Self::empty(SceneParams::AboveClouds(params.clone()), cx.room_origin);
        scene.scene_origo = scene_origo;
        scene.add_element(SceneElement::new(Vec2::ZERO, 0.0, ElementKind::Fog));
        for (name, time) in [
            ("AtC_NightSky", SkyTime::Night),
            ("AtC_DuskSky", SkyTime::Dusk),
            ("AtC_Sky", SkyTime::Day),
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
        if si_clouds {
            scene.add_building(&Building::new("AtC_Spire1", (517.0, -148.0), 50.0, -30.0));
        }
        let shifted = !si_clouds
            || Self::name_eq(cx.room_name, "SI_A07")
            || Self::name_eq(cx.room_name, "SI_SAINTINTRO");
        let (rows, shift) = if shifted {
            (Self::wide_rows(), if si_clouds { 200.0 } else { 0.0 })
        } else {
            (Self::si_rows(), 0.0)
        };
        for row in &rows {
            scene.add_building_shifted(row, shift);
        }
        for i in 0..7 {
            let cloud_depth = i as f32 / 6.0;
            let depth = params
                .clouds_start_depth
                .lerp(params.clouds_end_depth, cloud_depth);
            let cloud = CloudData {
                random_offset: rng.value(),
                index: i,
            };
            scene.add_element(SceneElement::new(
                Vec2::ZERO,
                depth,
                ElementKind::CloseCloud { cloud, cloud_depth },
            ));
        }
        if !si_clouds {
            for j in 0..11 {
                let f = j as f32 / 10.0;
                let depth = params
                    .clouds_end_depth
                    .lerp(params.distant_clouds_end_depth, f.mathf_pow(1.5));
                let cloud = CloudData {
                    random_offset: rng.value(),
                    index: j,
                };
                scene.add_element(SceneElement::new(
                    Vec2::new(0.0, -40.0 * params.clouds_end_depth * (1.0 - f)),
                    depth,
                    ElementKind::DistantCloud {
                        cloud,
                        distant_cloud_depth: f,
                    },
                ));
            }
            for (y, depth, flattened, alpha, input) in [
                (75.0, 355.0, 0.35, 0.5, 0.9),
                (43.0, 920.0, 0.15, 0.3, 0.95),
            ] {
                let cloud = CloudData {
                    random_offset: rng.value(),
                    index: 0,
                };
                let pos = scene.pos_from_draw_pos_at_neutral_cam_pos(Vec2::new(0.0, y), depth);
                scene.add_element(SceneElement::new(
                    pos,
                    depth,
                    ElementKind::FlyingCloud {
                        cloud,
                        flattened,
                        alpha,
                        shader_input_color: input,
                    },
                ));
            }
        }
        scene
    }

    /// C# `string.StartsWith` on the room's name (ordinal, as the world file's names are
    /// upper case; the port keeps room names as read, so compared ignoring case).
    fn name_starts_with(name: &str, prefix: &str) -> bool {
        name.get(..prefix.len())
            .is_some_and(|n| n.eq_ignore_ascii_case(prefix))
    }

    fn name_eq(name: &str, other: &str) -> bool {
        name.eq_ignore_ascii_case(other)
    }

    fn add_building(&mut self, b: &Building) {
        self.add_building_shifted(b, 0.0);
    }

    /// A `DistantBuilding` (and its `DistantLightning`) at the draw position `b.at` with
    /// `shift` added to its `x`, placed by `PosFromDrawPosAtNeutralCamPos`.
    fn add_building_shifted(&mut self, b: &Building, shift: f32) {
        let pos =
            self.pos_from_draw_pos_at_neutral_cam_pos(Vec2::new(b.at.0 + shift, b.at.1), b.depth);
        self.add_element(SceneElement::new(
            pos,
            b.depth,
            ElementKind::DistantBuilding {
                asset: b.asset,
                atmospheric_depth_add: b.add,
                alpha: 1.0,
                scale: 1.0,
            },
        ));
        if let Some((asset, minus_depth, y)) = b.light {
            let pos =
                self.pos_from_draw_pos_at_neutral_cam_pos(Vec2::new(b.at.0 + shift, y), b.depth);
            self.add_element(SceneElement::new(
                pos,
                b.depth - minus_depth,
                ElementKind::DistantLightning(DistantLightning::new(asset, minus_depth)),
            ));
        }
    }

    /// The rows of the first branch (`!SIClouds` or `SI_A07`): 15 buildings, then a
    /// second row of 7 (with a third lightning), all at depth factor 1.
    fn wide_rows() -> Vec<Building> {
        vec![
            Building::new("AtC_Structure1", (-520.0, -85.0), 160.0, -20.0).lit(
                "AtC_Light1",
                55.0,
                -119.0,
            ),
            Building::new("AtC_Structure2", (88.0, -37.0), 350.0, 0.0).lit(
                "AtC_Light2",
                250.0,
                -53.0,
            ),
            Building::new("AtC_Structure3", (-316.0, -24.0), 600.0, -100.0).lit(
                "AtC_Light3",
                80.0,
                -32.0,
            ),
            Building::new("AtC_Structure4", (-648.0, -21.0), 700.0, -200.0),
            Building::new("AtC_Structure5", (156.0, -22.0), 800.0, -350.0),
            Building::new("AtC_Structure6", (-242.0, -20.0), 850.0, -350.0),
            Building::new("AtC_Spire1", (-587.0, -134.0), 80.0, -60.0),
            Building::new("AtC_Spire2", (-653.0, -57.0), 100.0, 10.0),
            Building::new("AtC_Spire3", (0.0, -46.0), 155.0, 0.0),
            Building::new("AtC_Spire4", (-224.0, -20.0), 190.0, 80.0),
            Building::new("AtC_Spire5", (-276.0, -24.0), 360.0, -100.0),
            Building::new("AtC_Spire6", (-39.0, -33.0), 280.0, 0.0),
            Building::new("AtC_Spire7", (155.0, -7.0), 370.0, -85.0),
            Building::new("AtC_Spire8", (-380.0, 3.0), 380.0, -50.0),
            Building::new("AtC_Spire9", (-207.0, -1.0), 395.0, -50.0),
            Building::new("AtC_Structure1", (670.0, -85.0), 120.0, -20.0).lit(
                "AtC_Light1",
                55.0,
                -119.0,
            ),
            Building::new("AtC_Spire2", (470.0, -57.0), 160.0, 10.0),
            Building::new("AtC_Spire5", (220.0, -24.0), 365.0, -100.0),
            Building::new("AtC_Spire6", (520.0, -33.0), 285.0, 0.0),
            Building::new("AtC_Spire7", (340.0, -7.0), 375.0, -85.0),
            Building::new("AtC_Spire8", (600.0, 3.0), 380.0, -50.0),
            Building::new("AtC_Spire9", (175.0, -1.0), 390.0, -50.0),
        ]
    }

    /// The rows of the other Sky Islands rooms: the first 15 buildings and 3 lightnings
    /// (the same places as [`Self::wide_rows`]).
    fn si_rows() -> Vec<Building> {
        Self::wide_rows().into_iter().take(15).collect()
    }
}
