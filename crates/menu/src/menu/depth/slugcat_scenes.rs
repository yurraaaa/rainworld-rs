//! `BuildScene`'s depth branches for the slugcat select pages (544245-544336,
//! 544430-544499), Remix off: the slugcat layers are `LightEdges`.

use rw_sim::Vec2;

use super::{DepthScene, MenuShader};
use crate::menu::object::SceneId;

impl SceneId {
    /// The slugcat select scenes (the Hunter's dark until `redUnlocked`); `None` for
    /// any other.
    pub(super) fn slugcat_scene(self, red_unlocked: bool) -> Option<DepthScene> {
        use MenuShader::{LightEdges, Lighten, Normal, Overlay};
        let o = Vec2::ZERO;
        let scene = match self {
            Self::SlugcatWhite => DepthScene::new("Scenes/Slugcat - White")
                .layer("White Background - 5", o, 3.6, Normal)
                .layer("White Haze - 4", o, 3.2, Lighten)
                .alpha(0.3)
                .layer("White BkgPlants - 3", o, 3.1, Normal)
                .layer("White Vines - 1", o, 2.8, Normal)
                .glow()
                .layer("White Slugcat - 2", o, 2.7, LightEdges)
                .layer("White FgPlants - 0", o, 2.1, Normal)
                .idle(&[3.6, 2.8, 2.7, 2.6, 1.5]),
            Self::SlugcatYellow => DepthScene::new("Scenes/Slugcat - Yellow")
                .layer("Yellow Background - 5", o, 3.6, Normal)
                .layer("Yellow Specks - 4", o, 2.7, Overlay)
                .layer("Yellow Vines - 3", o, 2.9, Normal)
                .layer("Yellow BkgPlants - 2", o, 3.1, Normal)
                .glow()
                .layer("Yellow Slugcat - 1", o, 2.6, LightEdges)
                .layer("Yellow FgPlants - 0", o, 2.1, Normal)
                .idle(&[3.6, 2.8, 2.6, 2.5, 1.5]),
            Self::SlugcatRed => {
                let scene = DepthScene::new("Scenes/Slugcat - Red")
                    .layer("Red Background - 4", o, 3.2, Normal)
                    .layer("Red Spears - 3", o, 2.9, Normal)
                    .layer("Red BgPlants - 2", o, 2.7, Normal)
                    .glow();
                let scene = if red_unlocked {
                    scene.layer("Red Slugcat - 1", o, 2.3, LightEdges)
                } else {
                    scene.layer("Red Slugcat - 1 - Dark", o, 2.3, Normal)
                };
                scene
                    .layer("Red FgPlants - 0", o, 2.1, Normal)
                    .idle(&[3.0, 2.4, 2.3, 2.2, 1.5])
            }
            Self::GhostWhite => Self::ghost_scene("White"),
            Self::GhostYellow => Self::ghost_scene("Yellow"),
            Self::GhostRed => Self::ghost_scene("Red"),
            Self::SlugcatDeadRed => DepthScene::new("Scenes/Dead Red")
                .layer("Red Death - 5", o, 3.4, Normal)
                .layer("Red Death - 4", o, 2.7, Normal)
                .alpha(0.53)
                .layer("Red Death - 3", o, 2.5, Normal)
                .layer("Red Death - 2", o, 2.3, Normal)
                .layer("Red Death - 1", o, 1.6, Normal)
                .idle(&[3.0, 2.4, 2.3, 2.2, 1.5]),
            _ => return None,
        };
        Some(scene)
    }

    /// `Ghost_White`, `Ghost_Yellow` and `Ghost_Red`: the same three layers in the
    /// slugcat's folder.
    fn ghost_scene(name: &str) -> DepthScene {
        let o = Vec2::ZERO;
        DepthScene::new(&format!("Scenes/{name} Ghost Slugcat"))
            .layer(&format!("{name} Ghost Bkg"), o, 4.5, MenuShader::Normal)
            .layer(&format!("{name} Ghost A"), o, 2.85, MenuShader::Normal)
            .layer(&format!("{name} Ghost B"), o, 2.7, MenuShader::Overlay)
            .idle(&[3.1, 2.8])
    }
}

#[cfg(test)]
mod tests {
    use crate::menu::object::SceneId;

    #[test]
    fn white_glows_between_the_vines_and_the_slugcat() {
        let s = SceneId::SlugcatWhite.slugcat_scene(true).unwrap();
        assert_eq!(s.layers.len(), 6);
        assert_eq!(s.glow_at, Some(4));
        assert_eq!(s.layers[4].file, "White Slugcat - 2");
        assert_eq!(s.layers[1].set_alpha, Some(0.3));
    }

    #[test]
    fn the_locked_hunter_is_dark() {
        let s = SceneId::SlugcatRed.slugcat_scene(false).unwrap();
        assert_eq!(s.glow_at, Some(3));
        assert_eq!(s.layers[3].file, "Red Slugcat - 1 - Dark");
        assert_eq!(s.layers[3].shader, super::MenuShader::Normal);
        let s = SceneId::GhostYellow.slugcat_scene(false).unwrap();
        assert_eq!(s.folder, "Scenes/Yellow Ghost Slugcat");
        assert_eq!(
            (s.layers[2].shader, s.glow_at),
            (super::MenuShader::Overlay, None)
        );
    }
}
