//! The camera room's `BackgroundScene`s on screen: each element's sprites for the frame
//! (the lightning's brightness draws on the cosmetic stream), the images they need
//! uploaded, and the scene shaders' globals.
//!
//! Mirrors: BackgroundScene.cs (BackgroundSceneElement.DrawSprites), RoofTopView.cs (Floor.DrawSprites: `_WorldCamPos`)

use rw_graphics::{
    BackgroundSceneSprites, BackgroundSceneView, Color, DrawList, DustPuffSprites, SceneImageSizes,
};
use rw_render::SceneGlobals;
use rw_sim::{BackgroundScene, SceneKind};

use super::{Drawable, Game};

impl Game {
    /// The sprites of every element of the camera room's scenes that has joined the room,
    /// for `view`.
    pub fn background_scene_draw_lists(
        &mut self,
        view: &BackgroundSceneView,
        sizes: &SceneImageSizes,
    ) -> Vec<(Drawable, DrawList)> {
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let rng = &mut self.graphics.cosmetic;
        let mut lists = Vec::new();
        for scene in &r.background_scenes {
            for e in scene.elements.iter().filter(|e| e.update_order != 0) {
                lists.push((
                    Drawable::SceneElement(e.update_order),
                    BackgroundSceneSprites::draw(scene, e, view, sizes, rng),
                ));
            }
        }
        lists
    }

    /// The camera room's `DustPuff`s at `alpha` between ticks, in the palette's texel
    /// (9, 5) (`ApplyPalette`).
    #[must_use]
    pub fn dust_puff_draw_lists(
        &self,
        alpha: f32,
        palette: &rw_assets::PaletteTexture,
    ) -> Vec<(Drawable, DrawList)> {
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let [red, green, blue, a] = palette.texel(9, 5);
        let color = Color::from([red, green, blue, a].map(|c| f32::from(c) / 255.0));
        r.dust_puffs
            .iter()
            .map(|p| (Drawable::DustPuff(p.update_order), p.draw(alpha, color)))
            .collect()
    }

    /// The images of the camera room's scenes' elements.
    #[must_use]
    pub fn camera_scene_images(&self) -> Vec<&'static str> {
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let mut names: Vec<&'static str> = Vec::new();
        for name in r
            .background_scenes
            .iter()
            .flat_map(BackgroundScene::image_names)
        {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    /// The scene shaders' globals for the camera room: the last scene's `atmosphereColor`
    /// (the scenes' constructors set `_AboveCloudsAtmosphereColor` and `_MultiplyColor`;
    /// a roof top's floor sets `_WorldCamPos` to `camPos - sceneOrigo` in every draw).
    /// `None` when the room has no scene (the globals keep what the last one left).
    #[must_use]
    pub fn background_scene_globals(&self, cam_pos: rw_sim::Vec2) -> Option<SceneGlobals> {
        let r = self.sim.world().realized(self.sim.camera_room())?;
        let scene = r.background_scenes.last()?;
        let world_cam = if scene.kind() == SceneKind::RoofTopView {
            cam_pos - scene.scene_origo
        } else {
            rw_sim::Vec2::ZERO
        };
        Some(SceneGlobals {
            atmosphere: scene.atmosphere_color,
            multiply: [1.0; 3],
            world_cam_pos: [world_cam.x, world_cam.y],
        })
    }
}
