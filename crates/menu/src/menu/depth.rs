//! Depth menu scenes: `MenuDepthIllustration` (a layer whose texture holds its colour
//! over a depth map, moved by the scene's camera and blurred by its focus), the
//! `BuildScene` tables of the scenes the port shows in depth, `positions.txt`, and the
//! scene's shader globals. Behaviour: `rainworld-re/notes/menu.md` ("Depth scenes").

use rw_assets::StreamingAssets;
use rw_render::{MenuSceneGlobals, StageShader};
use rw_sim::{FloatExt, Vec2};

use super::Menu;
use super::object::{Kind, ObjectId, SceneData, SceneId};
use crate::atlas::AtlasManager;
use crate::shared::MenuShared;

/// `MenuDepthIllustration.MenuShader`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuShader {
    Normal,
    Lighten,
    LightEdges,
    Rain,
    Overlay,
    Basic,
    SoftLight,
    Multiply,
}

impl MenuShader {
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::Lighten,
        Self::LightEdges,
        Self::Rain,
        Self::Overlay,
        Self::Basic,
        Self::SoftLight,
        Self::Multiply,
    ];

    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Lighten => "Lighten",
            Self::LightEdges => "LightEdges",
            Self::Rain => "Rain",
            Self::Overlay => "Overlay",
            Self::Basic => "Basic",
            Self::SoftLight => "SoftLight",
            Self::Multiply => "Multiply",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.value() == value)
    }

    /// The stage shader the layer's sprite is drawn with. `SceneRain` and
    /// `SceneMultiply` are used by no base-game scene and are not ported: such a layer
    /// draws with `Basic`.
    #[must_use]
    pub fn stage_shader(self) -> StageShader {
        match self {
            Self::Normal => StageShader::SceneBlur,
            Self::Lighten => StageShader::SceneLighten,
            Self::LightEdges => StageShader::SceneBlurLightEdges,
            Self::Overlay => StageShader::SceneOverlay,
            Self::SoftLight => StageShader::SceneSoftLight,
            Self::Basic | Self::Rain | Self::Multiply => StageShader::Basic,
        }
    }

    /// `Basic` and `Rain` show the whole image; the others its upper (colour) half.
    #[must_use]
    pub fn whole_image(self) -> bool {
        matches!(self, Self::Basic | Self::Rain)
    }
}

/// One `new MenuDepthIllustration(...)` of a `BuildScene` table.
#[derive(Debug, Clone, PartialEq)]
pub struct DepthLayer {
    /// The image's folder; empty for `Illustrations`.
    pub folder: String,
    pub file: String,
    /// The default position (`positions.txt` overrides it).
    pub pos: Vec2,
    pub depth: f32,
    pub shader: MenuShader,
    /// The `setAlpha` `BuildScene` gives the layer.
    pub set_alpha: Option<f32>,
}

/// A scene's depth branch of `BuildScene`: its folder (`sceneFolder`, where
/// `positions.txt` is read), its layers in order and its idle focus depths.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DepthScene {
    pub folder: String,
    pub layers: Vec<DepthLayer>,
    pub idle_depths: Vec<f32>,
}

impl DepthScene {
    #[must_use]
    pub fn new(folder: &str) -> Self {
        Self {
            folder: folder.to_owned(),
            ..Self::default()
        }
    }

    /// A layer from the scene's folder.
    #[must_use]
    pub fn layer(self, file: &str, pos: Vec2, depth: f32, shader: MenuShader) -> Self {
        let folder = self.folder.clone();
        self.layer_in(&folder, file, pos, depth, shader)
    }

    /// A layer from another folder (`""`: `Illustrations`).
    #[must_use]
    pub fn layer_in(
        mut self,
        folder: &str,
        file: &str,
        pos: Vec2,
        depth: f32,
        shader: MenuShader,
    ) -> Self {
        self.layers.push(DepthLayer {
            folder: folder.to_owned(),
            file: file.to_owned(),
            pos,
            depth,
            shader,
            set_alpha: None,
        });
        self
    }

    /// `depthIllustrations[^1].setAlpha = alpha`.
    #[must_use]
    pub fn alpha(mut self, alpha: f32) -> Self {
        if let Some(last) = self.layers.last_mut() {
            last.set_alpha = Some(alpha);
        }
        self
    }

    /// `idleDepths.Add` for each.
    #[must_use]
    pub fn idle(mut self, depths: &[f32]) -> Self {
        self.idle_depths.extend_from_slice(depths);
        self
    }

    /// `RefreshPositions`' file: `positions_ims.txt` for an interactive scene when it
    /// exists, else `positions.txt`; its lines in order up to the first that does not
    /// parse (where the game would throw). Empty when there is no file.
    #[must_use]
    pub fn read_positions(assets: &StreamingAssets, folder: &str, interactive: bool) -> Vec<Vec2> {
        let ims = assets.resolve_file_path(&format!("{folder}/positions_ims.txt"));
        let path = if interactive && ims.is_file() {
            ims
        } else {
            assets.resolve_file_path(&format!("{folder}/positions.txt"))
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse_positions(&text),
            Err(_) => Vec::new(),
        }
    }

    /// The lines `x, y` (`ValidateSpacedDelimiter`, split on `", "`, invariant floats).
    #[must_use]
    pub fn parse_positions(text: &str) -> Vec<Vec2> {
        let mut out = Vec::new();
        for line in text.lines() {
            let line = if line.ends_with(',') {
                format!("{line} ")
            } else {
                line.to_owned()
            };
            let mut parts = line.split(", ");
            let x = parts.next().and_then(|p| p.trim().parse::<f32>().ok());
            let y = parts.next().and_then(|p| p.trim().parse::<f32>().ok());
            let (Some(x), Some(y)) = (x, y) else {
                log::warn!("positions: unreadable line {line:?}");
                break;
            };
            out.push(Vec2::new(x, y));
        }
        out
    }
}

impl SceneId {
    /// The depth branch of `MenuScene.BuildScene`, for the scenes the port shows in
    /// depth; `None` for the others, which stay flat (D13).
    #[must_use]
    pub fn depth_scene(self, _shared: &MenuShared) -> Option<DepthScene> {
        match self {
            // `MainMenu_Downpour` without Downpour installed (every DLC is treated as
            // not installed) builds the main-menu scene.
            Self::MainMenu | Self::MainMenuDownpour => Some(Self::main_menu_scene()),
            Self::LandscapeSu => Some(Self::landscape_su_scene()),
            _ => None,
        }
    }

    fn main_menu_scene() -> DepthScene {
        let at = Vec2::new(23.0, 17.0);
        DepthScene::new("Scenes/Main Menu")
            .layer("MainFarBackground", at, 14.0, MenuShader::Normal)
            .layer("MainArchs", at, 8.5, MenuShader::Normal)
            .layer("MainWaterfall", at, 8.0, MenuShader::Normal)
            .layer_in(
                "",
                "MainVines3",
                Vec2::new(-131.0, 675.0),
                7.5,
                MenuShader::Normal,
            )
            .layer_in(
                "",
                "MainVines2",
                Vec2::new(-13.0, 488.0),
                4.0,
                MenuShader::Normal,
            )
            .layer("MainTerrain1", at, 3.5, MenuShader::Normal)
            .layer("MainMGLizard2", at, 3.45, MenuShader::Normal)
            .layer("MainMGLizard1", at, 3.4, MenuShader::Normal)
            .layer("MainDarken", at, 3.5, MenuShader::Normal)
            .alpha(0.66)
            .layer("MainFog", at, 4.0, MenuShader::Normal)
            .alpha(0.21)
            .layer("MainVignette", at, 3.0, MenuShader::Normal)
            .alpha(0.55)
            .layer(
                "MainFGLizard3",
                Vec2::new(85.0, 91.0),
                3.3,
                MenuShader::Normal,
            )
            .layer(
                "MainFGLizard2",
                Vec2::new(143.0, 81.0),
                2.5,
                MenuShader::Normal,
            )
            .layer(
                "MainFGLizard1",
                Vec2::new(96.0, 39.0),
                2.35,
                MenuShader::Normal,
            )
            .layer(
                "MainVines1",
                Vec2::new(-66.0, -68.0),
                1.5,
                MenuShader::Normal,
            )
    }

    fn landscape_su_scene() -> DepthScene {
        DepthScene::new("Scenes/Landscape - SU")
            .layer(
                "SU_Landscape - 3",
                Vec2::new(85.0, 91.0),
                6.0,
                MenuShader::Normal,
            )
            .layer(
                "SU_Landscape - 2",
                Vec2::new(143.0, 81.0),
                2.5,
                MenuShader::Normal,
            )
            .layer(
                "SU_Landscape - 1",
                Vec2::new(96.0, 39.0),
                1.25,
                MenuShader::Normal,
            )
    }
}

impl SceneData {
    /// `CamPos(timeStacker)`: the camera, interpolated, times `cameraRange`.
    #[must_use]
    pub fn cam_pos_at(&self, t: f32) -> Vec2 {
        self.last_cam_pos.lerp(self.cam_pos, t) * self.camera_range
    }

    /// The shader globals `MenuScene.GrafUpdate` sets.
    #[must_use]
    pub fn shader_globals(&self, t: f32) -> MenuSceneGlobals {
        let focus = self.last_focus.lerp(self.focus, t);
        let cam = self.cam_pos_at(t);
        MenuSceneGlobals {
            blur_depth: 1.0 + focus * 9.0,
            blur_range: self
                .blur_max
                .lerp(self.blur_min, focus.max(0.0).mathf_pow(0.75))
                .clamp(0.0, 1.0),
            cam_pos: [cam.x.inverse_lerp(-1.0, 1.0), cam.y.inverse_lerp(-1.0, 1.0)],
        }
    }
}

impl Menu {
    /// `new MenuDepthIllustration(menu, owner, folder, file, pos, depth, shader)`: a
    /// bottom-left anchored, bilinear illustration whose sprite shows the colour half at
    /// half height (the whole image for `Basic` and `Rain`).
    #[allow(clippy::too_many_arguments)]
    pub fn add_depth_illustration(
        &mut self,
        shared: &mut MenuShared,
        owner: ObjectId,
        folder: &str,
        file: &str,
        pos: Vec2,
        depth: f32,
        shader: MenuShader,
    ) -> ObjectId {
        let id = self.add_illustration(shared, owner, folder, file, pos, false, false);
        let whole = shader.whole_image();
        if !whole {
            self.objects[id.0].size.y /= 2.0;
        }
        if let Some(i) = self.illustration_mut(id) {
            i.depth = Some(super::DepthData {
                depth,
                shader,
                offset: Vec2::ZERO,
            });
            let node = i.sprite;
            let sprite = self.stage.sprite_mut(node);
            sprite.shader = shader.stage_shader();
            if !whole {
                sprite.transform.scale_y = 0.5;
            }
        }
        id
    }

    /// The depth branch of `BuildScene` and `RefreshPositions`: the layers, their
    /// `positions.txt` places (plus `offset`), then the layers become sub-objects.
    pub(super) fn build_depth_scene(
        &mut self,
        shared: &mut MenuShared,
        scene: ObjectId,
        table: &DepthScene,
        interactive: bool,
    ) {
        let mut layers = Vec::with_capacity(table.layers.len());
        for l in &table.layers {
            let id = self.add_depth_illustration(
                shared, scene, &l.folder, &l.file, l.pos, l.depth, l.shader,
            );
            if let (Some(a), Some(i)) = (l.set_alpha, self.illustration_mut(id)) {
                i.set_alpha = Some(a);
            }
            layers.push(id);
        }
        let positions = DepthScene::read_positions(&shared.assets, &table.folder, interactive);
        for (&id, &p) in layers.iter().zip(&positions) {
            let offset = self
                .illustration_mut(id)
                .and_then(|i| i.depth)
                .map_or(Vec2::ZERO, |d| d.offset);
            let o = &mut self.objects[id.0];
            o.pos = p + offset;
            o.last_pos = o.pos;
        }
        if let Some(s) = self.scene_mut(scene) {
            s.flat_mode = false;
            s.depth_illustrations.clone_from(&layers);
            s.idle_depths.clone_from(&table.idle_depths);
        }
        for id in layers {
            self.add_sub(scene, id);
        }
    }

    /// `MenuDepthIllustration.DepthAtPosition(ps, devtool: false)`: the depth of the
    /// layer's pixel under `ps` (screen pixels), or -1 outside it or where its colour's
    /// alpha is under 0.25.
    #[must_use]
    pub fn depth_at_position(&self, atlases: &AtlasManager, id: ObjectId, ps: Vec2) -> f32 {
        let o = &self.objects[id.0];
        let Kind::Illustration(i) = &o.kind else {
            return -1.0;
        };
        let (Some(d), Some(owner)) = (i.depth, o.owner) else {
            return -1.0;
        };
        let image = atlases
            .element(&i.file_name)
            .and_then(|e| atlases.texture_image(e.texture));
        let (Some(image), Kind::Scene(s)) = (image, &self.objects[owner.0].kind) else {
            return -1.0;
        };
        let ps = ps - (o.pos - s.cam_pos_at(1.0) * 80.0 / d.depth);
        if ps.x < 0.0 || ps.y < 0.0 || ps.x > o.size.x || ps.y > o.size.y {
            return -1.0;
        }
        let x = ps.x as i32;
        if d.shader.whole_image() {
            if Self::texel(image, x, ps.y as i32)[3] < 0.25 {
                return -1.0;
            }
            return 1.0 + d.depth;
        }
        if Self::texel(image, x, (o.size.y + ps.y) as i32)[3] < 0.25 {
            return -1.0;
        }
        (1.0 - Self::texel(image, x, ps.y as i32)[0]) * 2.0 + d.depth
    }

    /// `Texture2D.GetPixel` of a clamped texture: `y` from the bottom, out-of-range
    /// coordinates clamped to the edge.
    fn texel(image: &rw_assets::PngImage, x: i32, y: i32) -> [f32; 4] {
        let x = x.clamp(0, image.width() as i32 - 1) as usize;
        let y = y.clamp(0, image.height() as i32 - 1) as usize;
        image.unity_pixel(x, y).map(|c| f32::from(c) / 255.0)
    }

    /// The nearest depth under `ps` over the scene's layers (`f32::MAX` if none).
    #[must_use]
    pub fn min_depth_at(&self, atlases: &AtlasManager, scene: ObjectId, ps: Vec2) -> f32 {
        let Kind::Scene(s) = &self.objects[scene.0].kind else {
            return f32::MAX;
        };
        s.depth_illustrations
            .iter()
            .map(|&id| self.depth_at_position(atlases, id, ps))
            .filter(|&d| d > -1.0)
            .fold(f32::MAX, f32::min)
    }

    /// `MenuDepthIllustration.GrafUpdate`: the place less the camera over the depth
    /// (`pos - CamPos * 80 / depth`); unless `Basic`, the colour carries the size and
    /// depth for the shader.
    pub(super) fn graf_depth_illustration(&mut self, id: ObjectId, t: f32) {
        let o = &self.objects[id.0];
        let (Kind::Illustration(i), Some(owner)) = (&o.kind, o.owner) else {
            return;
        };
        let (Some(d), Kind::Scene(s)) = (i.depth, &self.objects[owner.0].kind) else {
            return;
        };
        let pos = o.last_pos.lerp(o.pos, t) - s.cam_pos_at(t) * 80.0 / d.depth;
        let color = crate::stage::StageColor::rgb(
            o.size.x.inverse_lerp(1.0, 1400.0),
            o.size.y.inverse_lerp(1.0, 800.0),
            d.depth.inverse_lerp(0.0, 10.0),
        );
        let node = i.sprite;
        let sprite = self.stage.sprite_mut(node);
        sprite.transform.x = pos.x;
        sprite.transform.y = pos.y;
        if d.shader != MenuShader::Basic {
            sprite.color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_values_round_trip() {
        for s in MenuShader::ALL {
            assert_eq!(MenuShader::from_value(s.value()), Some(s));
        }
        assert_eq!(MenuShader::Normal.stage_shader(), StageShader::SceneBlur);
        assert!(MenuShader::Basic.whole_image() && !MenuShader::Lighten.whole_image());
    }

    #[test]
    fn positions_parse_as_the_game() {
        // A trailing comma gets a space (`ValidateSpacedDelimiter`) and still parses;
        // a line without ", " is where the game throws.
        let p = DepthScene::parse_positions("-183, 24\r\n-134, -68\n1.5, 2,\nbad\n9, 9");
        assert_eq!(
            p,
            [
                Vec2::new(-183.0, 24.0),
                Vec2::new(-134.0, -68.0),
                Vec2::new(1.5, 2.0)
            ]
        );
    }

    #[test]
    fn main_menu_table_follows_build_scene() {
        let s = SceneId::main_menu_scene();
        assert_eq!(s.layers.len(), 15);
        assert_eq!(s.layers[3].folder, "");
        assert_eq!(s.layers[3].file, "MainVines3");
        assert_eq!(s.layers[8].set_alpha, Some(0.66));
        assert_eq!(s.layers[14].depth, 1.5);
        assert!(s.idle_depths.is_empty());
        assert_eq!(SceneId::landscape_su_scene().layers[2].depth, 1.25);
    }

    #[test]
    fn globals_follow_graf_update() {
        let mut s = SceneData::new(SceneId::MainMenu);
        s.focus = 1.0;
        s.last_focus = 0.0;
        s.cam_pos = Vec2::new(0.5, -1.0);
        let g = s.shader_globals(0.5);
        // `1 + 0.5 * 9`; `Lerp(0.8, 0.25, 0.5^0.75)`; `InverseLerp(-1, 1, 0.25)`.
        assert!((g.blur_depth - 5.5).abs() < 1e-6);
        assert!((g.blur_range - (0.8 - 0.55 * 0.5_f32.powf(0.75))).abs() < 1e-6);
        assert!((g.cam_pos[0] - 0.625).abs() < 1e-6);
        assert!((g.cam_pos[1] - 0.25).abs() < 1e-6);
    }
}
