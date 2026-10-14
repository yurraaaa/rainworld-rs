//! The slugcat's sprite atlas (`rainWorld`) from the asset cache.

use rw_assets::{AssetCache, PngImage};
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
    /// Loads `rainWorld.json` and `rainWorld.png` from `cache`.
    ///
    /// # Errors
    /// A message naming what is missing and how to extract it.
    pub fn load(cache: &AssetCache) -> Result<Self, String> {
        let json = cache.atlas_json(ATLAS);
        if !json.is_file() {
            return Err(format!(
                "no sprite atlas at {}; {}",
                json.display(),
                cache.fill_hint()
            ));
        }
        let atlas = Atlas::from_cache(cache, ATLAS).map_err(|e| e.to_string())?;
        let atlas = SpriteAtlas::new(atlas).ok_or_else(|| {
            format!(
                "atlas {ATLAS} has no {} element",
                SpriteAtlas::WHITE_ELEMENT
            )
        })?;
        let image = PngImage::load(&cache.atlas_png(ATLAS)).map_err(|e| e.to_string())?;
        Ok(Self { atlas, image })
    }

    pub fn upload(&self, renderer: &mut impl Renderer) {
        renderer.set_atlas(&self.atlas, &self.image);
    }
}
