//! The slugcat's sprite atlas (`rainWorld`) from the game's `resources.assets`.

use rw_assets::{PngImage, UnityResources};
use rw_graphics::Atlas;
use rw_render::{Renderer, SpriteAtlas};

/// Atlas name holding the slugcat's elements.
const ATLAS: &str = "rainWorld";

/// Frame data and image of the sprite atlas.
pub struct SpriteAssets {
    atlas: SpriteAtlas,
    image: PngImage,
}

impl SpriteAssets {
    /// Loads the TextAsset and Texture2D `Atlases/rainWorld`, as Futile does.
    ///
    /// # Errors
    /// A message naming what could not be read.
    pub fn load(resources: &UnityResources) -> Result<Self, String> {
        let atlas = Atlas::from_resources(resources, ATLAS).map_err(|e| e.to_string())?;
        let atlas = SpriteAtlas::new(atlas).ok_or_else(|| {
            format!(
                "atlas {ATLAS} has no {} element",
                SpriteAtlas::WHITE_ELEMENT
            )
        })?;
        let image = resources
            .texture_image(&format!("Atlases/{ATLAS}"))
            .map_err(|e| e.to_string())?;
        Ok(Self { atlas, image })
    }

    pub fn upload(&self, renderer: &mut impl Renderer) {
        renderer.set_atlas(&self.atlas, &self.image);
    }
}
