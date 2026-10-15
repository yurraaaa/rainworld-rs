//! `SlideShowMenuScene`: a slideshow's scene, its camera path along `camera.txt`
//! (keyed to the slide's display time, the third column its focus), the depth branch of
//! `BuildScene` for the intro scenes and `ApplySceneSpecificAlphas`. No random draws.
//! Behaviour: `rainworld-re/notes/slideshow.md` ("Scenes").

use rw_assets::StreamingAssets;
use rw_graphics::Vec2Bezier;
use rw_sim::{FloatExt, Vec2, Vec3};

use super::Menu;
use super::build::FlatLayers;
use super::depth::{DepthScene, MenuShader};
use super::object::{Kind, ObjectId, SceneId};

/// `SlideShowMenuScene.cameraMovementPoints`: `x, y` the camera (screen pixels; the
/// scene divides by 300), `z` the focus, -1 where it is interpolated.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraPath {
    pub points: Vec<Vec3>,
}

impl CameraPath {
    /// The constructor's read: `<folder>/camera.txt`, or one point at zero when the
    /// scene has no folder or no such file.
    #[must_use]
    pub fn read(assets: &StreamingAssets, folder: &str) -> Self {
        let path = assets.resolve_file_path(&format!("{folder}/camera.txt"));
        let text = if folder.is_empty() {
            None
        } else {
            std::fs::read_to_string(path).ok()
        };
        let path = text.map(|t| Self::parse(&t)).unwrap_or_default();
        if path.points.is_empty() {
            Self {
                points: vec![Vec3::default()],
            }
        } else {
            path
        }
    }

    /// The lines `x, y, z` (`ValidateSpacedDelimiter`, split on `", "`, invariant
    /// floats), up to the first that does not parse (where the game would throw).
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut points = Vec::new();
        for line in text.lines() {
            let line = if line.ends_with(',') {
                format!("{line} ")
            } else {
                line.to_owned()
            };
            let v: Vec<Option<f32>> = line
                .split(", ")
                .take(3)
                .map(|p| p.trim().parse::<f32>().ok())
                .collect();
            let [Some(x), Some(y), Some(z)] = v[..] else {
                log::warn!("camera: unreadable line {line:?}");
                break;
            };
            points.push(Vec3::new(x, y, z));
        }
        Self { points }
    }

    /// The segment `f` (0..1 over the path) falls in, the next point's index and the
    /// segment position `f * (n - 1)`.
    fn segment(&self, f: f32) -> (usize, usize, f32) {
        let last = self.points.len().saturating_sub(1);
        let at = f * last as f32;
        let i = (at.floor() as i32).clamp(0, last as i32) as usize;
        (i, (i + 1).min(last), at)
    }

    /// `CameraPosition(f)`: the cubic through the points, each tangent along its
    /// neighbours.
    #[must_use]
    pub fn camera_position(&self, f: f32) -> Vec2 {
        let (i, j, at) = self.segment(f);
        if i == j {
            return self.points[i].xy();
        }
        let (a, b) = (self.points[i].xy(), self.points[j].xy());
        let d = a.distance(b);
        let ca = a - self.direction_of_segment(i) * d / 3.0;
        let cb = b + self.direction_of_segment(j) * d / 3.0;
        a.bezier(ca, b, cb, at.inverse_lerp(i as f32, j as f32))
    }

    /// `DirectionOfSegment`: zero at the ends.
    fn direction_of_segment(&self, i: usize) -> Vec2 {
        if i == 0 || i + 1 >= self.points.len() {
            return Vec2::ZERO;
        }
        let p = &self.points;
        ((p[i - 1].xy() - p[i].xy()) + (p[i].xy() - p[i + 1].xy())).normalized()
    }

    /// `DepthPosition(f)`: the focus, linear between the points around `f` that are not
    /// -1.
    #[must_use]
    pub fn depth_position(&self, f: f32) -> f32 {
        let (mut i, mut j, at) = self.segment(f);
        let last = self.points.len() - 1;
        while i > 0 && self.points[i].z == -1.0 {
            i -= 1;
        }
        while j < last && self.points[j].z == -1.0 {
            j += 1;
        }
        self.points[i]
            .z
            .lerp(self.points[j].z, at.inverse_lerp(i as f32, j as f32))
    }
}

impl SceneId {
    /// `sceneFolder` of a slideshow scene (`""` for `Empty` and the title).
    #[must_use]
    pub fn slide_show_folder(self) -> &'static str {
        match self.slide_show_layers() {
            FlatLayers::Scene { folder, .. } => folder,
            FlatLayers::None | FlatLayers::Title => "",
        }
    }

    /// The depth branch of `BuildScene` for a `SlideShowMenuScene` (where Intro 1, 3, 4
    /// and 5 have other depths than as an `InteractiveMenuScene`, which only Remix and
    /// Expedition menus build); `None` for `Empty` and the title, which have none.
    #[must_use]
    pub fn slide_show_depth_scene(self) -> Option<DepthScene> {
        let scene = DepthScene::new(self.slide_show_folder());
        let at = Vec2::new(71.0, 49.0);
        let zero = Vec2::ZERO;
        use MenuShader::{Basic, LightEdges, Lighten, Normal, Overlay, SoftLight};
        Some(match self {
            Self::Intro1Tree => scene
                .layer("11 - Bkg", zero, 16.0, Normal)
                .layer("10 - TerrainC", zero, 11.0, Normal)
                .layer("9 - TerrainB", zero, 8.0, Normal)
                .layer("8 - TerrainA", zero, 4.5, Normal)
                .layer("7 - FoliageB", zero, 4.0, LightEdges)
                .layer("6 - Trunk", zero, 3.0, LightEdges)
                .layer("5 - LightC", zero, 2.7, Lighten)
                .layer("4 - FoliageA", zero, 2.6, LightEdges)
                .layer("3 - LightB", zero, 1.9, Lighten)
                .layer("2 - LightA", zero, 1.8, Lighten)
                .layer("1 - Pipes", zero, 0.4, Normal),
            Self::Intro2Branch => scene
                .layer("6 - Bkg", zero, 8.0, Normal)
                .layer("5 - Trunk", zero, 4.0, Normal)
                .layer("4 - FoliageB", zero, 2.5, Normal)
                .layer("3 - Slugcats", zero, 1.5, Normal)
                .layer("2 - Light", zero, 1.4, Lighten)
                .layer("1 - FoliageA", zero, 0.1, Normal),
            Self::Intro3InTree => scene
                .layer("12 - Bkg", Vec2::new(71.0, 49.0), 15.0, Normal)
                .layer("11 - TerrainD", Vec2::new(543.0, 51.0), 8.0, Normal)
                .layer("10 - RainB", Vec2::new(475.0, 126.0), 6.0, Normal)
                .layer("9 - RainA", Vec2::new(593.0, 19.0), 4.8, Normal)
                .layer("RainEffect", Vec2::new(593.0, 19.0), 4.0, Normal)
                .alpha(0.75)
                .layer("RainMist", Vec2::new(726.0, 272.0), 5.4, Normal)
                .alpha(0.75)
                .layer("8 - TerrainC", Vec2::new(726.0, 72.0), 5.1, Normal)
                .layer("7 - TerrainB", Vec2::new(-126.0, -100.0), 2.5, Normal)
                .layer("6 - FruitsB", Vec2::new(480.0, 441.0), 2.0, Normal)
                .layer("5 - SlugcatB", Vec2::new(547.0, 4.0), 3.0, Normal)
                .layer("4 - TerrainA", Vec2::new(-358.0, -89.0), 1.0, Normal)
                .layer("3 - SlugcatsA", Vec2::new(283.0, -83.0), 1.5, Normal)
                .layer("2 - FruitsA", Vec2::new(-103.0, 423.0), 1.3, Normal)
                .layer("1 - Roots", Vec2::new(-213.0, -85.0), 1.1, Normal),
            Self::Intro4Walking => scene
                .layer("7 - RocksD", at, 15.0, Normal)
                .layer("6 - RocksC", at, 12.0, Normal)
                .layer("5 - RocksB", at, 9.0, Normal)
                .layer("4 - RocksA", at, 5.0, Normal)
                .layer("3 - Pipe", at, 2.0, Normal)
                .layer("2 - Slugcats", Vec2::new(543.0, 51.0), 1.9, Normal)
                .layer("1 - Foreground", Vec2::new(475.0, 126.0), 0.9, Normal),
            Self::Intro5Hunting => scene
                .layer("13 - Bkg", at, 7.4, Normal)
                .layer("12 - Light", at, 7.0, Lighten)
                .alpha(0.3)
                .layer("11 - Waterfall", at, 5.0, LightEdges)
                .layer("10 - Pillar", at, 3.0, LightEdges)
                .layer("9 - Terrain", at, 1.8, LightEdges)
                .layer("8 - SlugcatC", at, 2.5, LightEdges)
                .layer("7 - SlugcatD", at, 2.1, LightEdges)
                .layer("6 - SlugcatB", at, 2.2, LightEdges)
                .layer("5 - Specks", at, 2.0, LightEdges)
                .alpha(0.7)
                .layer("4 - CeilingB", at, 1.9, LightEdges)
                .layer("3 - CeilingA", at, 0.9, LightEdges)
                .layer("2 - SlugcatA", at, 0.6, LightEdges)
                .layer("1 - Grass", at, 0.2, LightEdges),
            Self::Intro67RainDrop => scene
                .layer("Sharp Ground", at, 4.0, Normal)
                .layer("Blurred Ground", at, 4.0, Normal)
                .layer("Sharp Slugcats", at, 3.5, Normal)
                .layer("Blurred Slugcats", at, 3.5, Normal)
                .layer("RainDrops", at, 3.2, Normal),
            Self::Intro8Climbing => scene
                .layer("9 - PipesB", at, 6.5, Normal)
                .layer("8 - PipesA", at, 5.5, Normal)
                .layer("7 - WaterB", at, 3.5, Normal)
                .layer("6 - RockFormation", at, 4.0, Normal)
                .layer("5 - RockBloom", at, 3.5, Lighten)
                .layer("4 - ClimbingSlugcat", at, 3.0, Normal)
                .layer("3 - SlugcatsAndRock", at, 2.0, Normal)
                .layer("2 - WaterB", at, 1.5, Normal)
                .layer("1 - RainDistortion", at, 2.0, Normal)
                .layer("0 - RainFilter", at, 1.9, Lighten)
                .alpha(0.25)
                .layer("-1 - Dark", at, 7.0, Normal)
                .layer("-2 - Bloom", at, 2.5, Lighten)
                .alpha(0.25),
            Self::Intro9RainyClimb => scene
                .layer("13 - Background", at, 100.0, Normal)
                .layer("12 - Flash", at, 15.0, Basic)
                .layer("11 - BkgAndSlugcat", at, 6.8, Normal)
                .layer("10 - Terrain", at, 6.5, Normal)
                .layer("9 - SkyOverlay", at, 6.0, Overlay)
                .alpha(0.5)
                .layer("8 - BkgWater", at, 5.0, Normal)
                .layer("7 - WaterFallB", at, 3.6, LightEdges)
                .layer("6 - WaterFoamA", at, 2.85, LightEdges)
                .layer("5 - WaterFallA", at, 2.5, LightEdges)
                .layer("4 - BlurredSlugcats", at, 4.6, Lighten)
                .alpha(0.5)
                .layer("3 - SlugcatsClimbing", at, 4.6, Normal)
                .layer("2 - ForegroundWavesB", at, 0.8, LightEdges)
                .layer("0 - RainOverlay", at, 2.0, SoftLight)
                .layer("1 - ForegroundWaves", at, 0.1, Normal)
                .layer("WhiteFade", zero, 200.0, Basic),
            Self::Intro10Fall => scene
                .layer("13 - Background", at, 100.0, Normal)
                .layer("11 - BkgAndSlugcat", at, 6.8, Normal)
                .layer("10 - Terrain", at, 6.5, Normal)
                .layer("9 - SidePipes", at, 5.0, Normal)
                .layer("3 - DarkSlugcatsClimbing", at, 4.6, Normal)
                .layer("8.5 - Darken", at, 100.0, Basic)
                .alpha(0.89)
                .layer("8 - DarkBkgWater", at, 5.0, Normal)
                .layer("7 - DarkWaterFallB", at, 3.6, Normal)
                .layer("6 - DarkWaterFoamA", at, 2.85, Normal)
                .layer("5 - DarkWaterFallA", at, 2.5, Normal)
                .layer("2 - DarkForegroundWavesB", at, 0.8, Normal)
                .layer("1 - DarkForegroundWaves", at, 0.1, Normal)
                .layer("0 - Lightning", at, 14.0, Basic)
                .layer("-1 - SkySoftLight", at, 3.0, SoftLight)
                .layer("-2 - SkyOverlay", at, 3.0, Overlay)
                .layer("-3 - LightOutline", at, 3.0, Lighten)
                .layer("-4 - FallingSlugcat", at, 4.4, Normal)
                .layer("0 - RainOverlay", at, 2.0, SoftLight)
                .alpha(0.75)
                .layer("WhiteFade", zero, 200.0, Basic),
            Self::Intro105Separation => scene
                .layer("6 - SlugcatPup", at, 3.0, Normal)
                .layer("5 - DripsD", at, 2.7, Normal)
                .layer("4 - DripsC", at, 2.5, Normal)
                .layer("3 - SlugcatMother", at, 1.6, Normal)
                .layer("2 - DripsB", at, 1.3, Normal)
                .layer("1 - DripsA", at, 1.1, Normal),
            Self::Intro11Drowning => scene
                .layer("11 - DrowningBkg", at, 10.0, Normal)
                .layer("10 - FloraC", at, 8.0, Normal)
                .layer("9 - FloraB", at, 7.0, Normal)
                .layer("8 - FloraA", at, 5.0, Normal)
                .layer("7 - BubblesD", at, 4.5, LightEdges)
                .layer("6 - BubblesC", at, 3.0, LightEdges)
                .layer("5 - Slugcat", at, 2.6, Normal)
                .layer("4 - BubblesB", at, 2.5, Overlay)
                .layer("3 - BubblesA", at, 2.1, LightEdges)
                .layer("2 - LightB", at, 4.0, LightEdges)
                .alpha(0.2)
                .layer("1 - LightA", at, 4.0, Lighten)
                .alpha(0.2),
            Self::Intro12Waking => scene
                .layer("3 - Ground", at, 5.0, Normal)
                .layer("2 - SlugcatWaking", at, 5.1, Normal)
                .layer("1 - Dust", at, 4.6, LightEdges),
            Self::Intro13Alone => scene
                .layer("12 - AloneBkg", at, 8.0, Normal)
                .layer("11 - BkgRain", at, 7.0, Normal)
                .layer("10 - Archways", at, 6.0, Normal)
                .layer("9 - SlugcatBloom", at, 2.25, Lighten)
                .layer("8 - SlugcatOnRock", at, 2.2, Normal)
                .layer("7 - Dust", at, 2.2, LightEdges)
                .layer("6 - Bloom", at, 2.0, Lighten)
                .layer("5 - Wires", at, 1.6, LightEdges)
                .layer("4 - ForegroundRocks", at, 0.9, Normal)
                .layer("2 - ChainB", at, 0.5, Normal)
                .layer("1 - ChainA", at, 0.6, Normal),
            Self::YellowIntroA => scene
                .layer("YellowA - 8", zero, 12.0, Normal)
                .layer("YellowA - 7", zero, 4.5, Normal)
                .layer("YellowA - 6", zero, 3.4, Normal)
                .layer("YellowA - 5", zero, 3.3, LightEdges)
                .layer("YellowA - 4", zero, 1.9, LightEdges)
                .layer("YellowA - 3", zero, 1.8, Normal)
                .layer("YellowA - 1", zero, 0.8, LightEdges)
                .layer("YellowA - 2", zero, 0.5, Normal)
                .layer("YellowA - Pixel", zero, 100.0, Basic)
                .scale(Vec2::new(18.0, 9.0))
                .alpha(0.0)
                .layer("YellowA - 0", zero, 1.9, Normal)
                .alpha(0.0),
            Self::YellowIntroB => scene
                .layer("YellowB - 5", zero, 5.0, Normal)
                .layer("YellowB - 4", zero, 50.0, Normal)
                .layer("YellowB - 3", zero, 2.6, Normal)
                .layer("YellowB - 1", zero, 1.5, LightEdges)
                .layer("YellowB - 2", zero, 0.75, Normal),
            _ => return None,
        })
    }
}

/// An alpha `ApplySceneSpecificAlphas` writes: `setAlpha` (taken by the layer's next
/// update) or `alpha` itself.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SceneAlpha {
    Set(ObjectId, f32),
    Direct(ObjectId, f32),
}

impl Menu {
    /// The rest of `new SlideShowMenuScene` after `BuildScene`: the camera path, the
    /// camera at its first point (not divided by 300, unlike in `Update`: the scene's
    /// first frames see that until its first update), the focus, the alphas at display
    /// time 0, then `lastAlpha = alpha` for the depth layers.
    pub(super) fn init_slide_show_scene(&mut self, assets: &StreamingAssets, scene: ObjectId) {
        let Some(s) = self.scene_mut(scene) else {
            return;
        };
        let path = CameraPath::read(assets, s.id.slide_show_folder());
        s.cam_pos = path.camera_position(0.0);
        s.last_cam_pos = s.cam_pos;
        s.focus = path.depth_position(0.0);
        s.last_focus = s.focus;
        s.camera_path = path;
        self.apply_slide_show_alphas(scene);
        let Kind::Scene(s) = &self.objects[scene.0].kind else {
            return;
        };
        for id in s.depth_illustrations.clone() {
            if let Some(i) = self.illustration_mut(id) {
                i.last_alpha = i.alpha;
            }
        }
    }

    /// `SlideShowMenuScene.Update` after its sub-objects: the display time is the
    /// slideshow's time in the scene; the camera (divided by 300) and focus follow the
    /// path; then the scene's alphas.
    pub fn update_slide_show_scene(&mut self, scene: ObjectId, display_time: f32) {
        let Some(s) = self.scene_mut(scene) else {
            return;
        };
        s.display_time = display_time;
        s.last_focus = s.focus;
        s.last_cam_pos = s.cam_pos;
        s.cam_pos = s.camera_path.camera_position(display_time) / 300.0;
        s.focus = s.camera_path.depth_position(display_time);
        self.apply_slide_show_alphas(scene);
    }

    /// `ApplySceneSpecificAlphas` (the intro scenes' branches).
    fn apply_slide_show_alphas(&mut self, scene: ObjectId) {
        for a in self.slide_show_alphas(scene) {
            match a {
                SceneAlpha::Set(id, v) => {
                    if let Some(i) = self.illustration_mut(id) {
                        i.set_alpha = Some(v);
                    }
                }
                SceneAlpha::Direct(id, v) => {
                    if let Some(i) = self.illustration_mut(id) {
                        i.alpha = v;
                    }
                }
            }
        }
    }

    fn slide_show_alphas(&self, scene: ObjectId) -> Vec<SceneAlpha> {
        let Kind::Scene(s) = &self.objects[scene.0].kind else {
            return Vec::new();
        };
        let t = s.display_time;
        let (depth, flats) = (&s.depth_illustrations, &s.flat_illustrations);
        let n = depth.len();
        let flat = s.flat_mode;
        let mut out = Vec::new();
        match s.id {
            SceneId::Intro67RainDrop => {
                let num = t.inverse_lerp(0.52, 0.58);
                if flat {
                    out.extend(flats.get(1).map(|&id| SceneAlpha::Set(id, num)));
                } else if n >= 5 {
                    let (off, on) = ((1.0 - num).mathf_pow(0.5), num.mathf_pow(0.5));
                    for (i, v) in [off, on, off, on, on].into_iter().enumerate() {
                        out.push(SceneAlpha::Set(depth[i], v));
                    }
                }
            }
            SceneId::Intro9RainyClimb if !flat => {
                out.extend(
                    depth
                        .last()
                        .map(|&id| SceneAlpha::Set(id, t.inverse_lerp(0.96, 0.99))),
                );
            }
            SceneId::Intro10Fall if !flat => {
                out.extend(
                    depth
                        .last()
                        .map(|&id| SceneAlpha::Set(id, t.inverse_lerp(0.04, 0.01))),
                );
            }
            // Not checked for flat mode: the title is a flat illustration in both.
            SceneId::Intro14Title => {
                out.extend(
                    flats
                        .last()
                        .map(|&id| SceneAlpha::Set(id, t.inverse_lerp(1.0, 0.7))),
                );
            }
            SceneId::YellowIntroA => {
                let num = t.inverse_lerp(0.51, 0.55);
                if flat {
                    out.extend(flats.get(1).map(|&id| SceneAlpha::Set(id, num)));
                } else if n >= 2 {
                    out.push(SceneAlpha::Direct(depth[n - 2], num.mathf_pow(1.5) * 0.65));
                    out.push(SceneAlpha::Direct(depth[n - 1], num.mathf_pow(0.5)));
                }
            }
            SceneId::YellowIntroB if !flat && n >= 3 => {
                // `Custom.LerpMap(t, a, b, from, to, exponent)`.
                let first = 1.0.lerp(0.2, t.inverse_lerp(0.1, 0.9).mathf_pow(0.7));
                let second = 0.5.lerp(1.0, t.inverse_lerp(0.1, 0.5).mathf_pow(1.2));
                out.push(SceneAlpha::Direct(depth[1], first));
                out.push(SceneAlpha::Direct(depth[2], second));
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(points: &[(f32, f32, f32)]) -> CameraPath {
        CameraPath {
            points: points.iter().map(|&(x, y, z)| Vec3::new(x, y, z)).collect(),
        }
    }

    #[test]
    fn camera_lines_parse_as_the_game() {
        let p = CameraPath::parse("-416, 170, 0.1415929\r\n-421, 104, -1\nbad\n1, 2, 3");
        assert_eq!(
            p.points,
            [
                Vec3::new(-416.0, 170.0, 0.141_592_9),
                Vec3::new(-421.0, 104.0, -1.0)
            ]
        );
    }

    #[test]
    fn camera_follows_the_bezier_through_the_points() {
        // Three points: segment 0 runs from (0, 0) (no tangent at the ends) to
        // (100, 0), whose tangent is `normalized((0,0)-(100,0) + (100,0)-(100,100))`
        // = (-1, -1)/sqrt 2; `cB = (100, 0) + dir * 100 / 3`.
        let p = path(&[(0.0, 0.0, 0.0), (100.0, 0.0, 0.0), (100.0, 100.0, 0.0)]);
        assert_eq!(p.camera_position(0.0), Vec2::ZERO);
        assert_eq!(p.camera_position(1.0), Vec2::new(100.0, 100.0));
        let d = -100.0 / 3.0 / 2.0_f32.sqrt();
        let want = Vec2::ZERO.bezier(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0 + d, d),
            0.5,
        );
        let got = p.camera_position(0.25);
        assert!((got - want).length() < 1e-4, "{got:?} vs {want:?}");
        // A single point (no `camera.txt`) stays put.
        assert_eq!(
            path(&[(3.0, 4.0, 0.0)]).camera_position(0.7),
            Vec2::new(3.0, 4.0)
        );
    }

    #[test]
    fn focus_skips_interpolated_points() {
        // Intro 1's first points: 0.1415929, -1, 0.3539823 over 8 segments.
        let p = path(&[
            (0.0, 0.0, 0.2),
            (0.0, 0.0, -1.0),
            (0.0, 0.0, -1.0),
            (0.0, 0.0, 0.8),
            (0.0, 0.0, -1.0),
        ]);
        // f = 0.375: position 1.5 lies between points 0 and 3.
        assert!((p.depth_position(0.375) - 0.2.lerp(0.8, 0.5)).abs() < 1e-6);
        assert_eq!(p.depth_position(0.0), 0.2);
        // Past the last real point the end (-1) is the far side, not skipped.
        assert!((p.depth_position(0.875) - 0.8.lerp(-1.0, 0.5)).abs() < 1e-6);
    }

    #[test]
    fn tables_follow_build_scene() {
        let counts = [
            (SceneId::Intro1Tree, 11),
            (SceneId::Intro2Branch, 6),
            (SceneId::Intro3InTree, 14),
            (SceneId::Intro4Walking, 7),
            (SceneId::Intro5Hunting, 13),
            (SceneId::Intro67RainDrop, 5),
            (SceneId::Intro8Climbing, 12),
            (SceneId::Intro9RainyClimb, 15),
            (SceneId::Intro10Fall, 19),
            (SceneId::Intro105Separation, 6),
            (SceneId::Intro11Drowning, 11),
            (SceneId::Intro12Waking, 3),
            (SceneId::Intro13Alone, 11),
            (SceneId::YellowIntroA, 10),
            (SceneId::YellowIntroB, 5),
        ];
        for (id, n) in counts {
            let s = id.slide_show_depth_scene().expect("a table");
            assert_eq!(s.layers.len(), n, "{id:?}");
            assert_eq!(s.folder, id.slide_show_folder());
        }
        assert!(SceneId::Empty.slide_show_depth_scene().is_none());
        assert!(SceneId::Intro14Title.slide_show_depth_scene().is_none());
        // The slideshow's depths, not the interactive scene's.
        let intro1 = SceneId::Intro1Tree.slide_show_depth_scene().unwrap();
        assert_eq!(intro1.layers[10].depth, 0.4);
        let intro3 = SceneId::Intro3InTree.slide_show_depth_scene().unwrap();
        let depths: Vec<f32> = intro3.layers.iter().map(|l| l.depth).collect();
        assert_eq!(
            depths,
            [
                15.0, 8.0, 6.0, 4.8, 4.0, 5.4, 5.1, 2.5, 2.0, 3.0, 1.0, 1.5, 1.3, 1.1
            ]
        );
        assert_eq!(intro3.layers[4].set_alpha, Some(0.75));
        let intro4 = SceneId::Intro4Walking.slide_show_depth_scene().unwrap();
        assert_eq!(intro4.layers[6].depth, 0.9);
        let intro5 = SceneId::Intro5Hunting.slide_show_depth_scene().unwrap();
        assert_eq!(
            intro5.layers[10..]
                .iter()
                .map(|l| l.depth)
                .collect::<Vec<_>>(),
            [0.9, 0.6, 0.2]
        );
        let yellow = SceneId::YellowIntroA.slide_show_depth_scene().unwrap();
        assert_eq!(yellow.layers[8].scale, Some(Vec2::new(18.0, 9.0)));
        assert_eq!(yellow.layers[8].shader, MenuShader::Basic);
        assert_eq!(yellow.layers[9].set_alpha, Some(0.0));
        let fall = SceneId::Intro10Fall.slide_show_depth_scene().unwrap();
        assert_eq!(fall.layers[18].file, "WhiteFade");
        assert_eq!(fall.layers[17].set_alpha, Some(0.75));
    }
}
