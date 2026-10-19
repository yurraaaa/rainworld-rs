//! The room sprites' atlases: `rainWorld` and `uiSprites` from the game's
//! `resources.assets`, and the overseer images' projection textures.
//!
//! Mirrors: - (port-only: atlas loading)

use rw_assets::{PngImage, StreamingAssets, UnityResources};
use rw_graphics::Atlas;
use rw_render::{Renderer, SpriteAtlas};

/// Atlas name holding the slugcat's elements (and `Futile_White`'s stand-in).
const ATLAS: &str = "rainWorld";

/// Further atlases, in `RainWorld.LoadResources`' order (the holograms' symbols and
/// keys are in `uiSprites`).
const MORE_ATLASES: [&str; 2] = ["uiSprites", "shelterGate"];

/// `HoloImage.LoadFile`'s textures: `Projections/<name>.png`, an atlas of one element.
const PROJECTIONS: [&str; 2] = ["RND_PROJ", "STR_PROJ"];

/// Frame data and one image per page of the sprite atlases.
pub struct SpriteAssets {
    atlas: SpriteAtlas,
    images: Vec<PngImage>,
}

impl SpriteAssets {
    /// Loads the TextAsset and Texture2D `Atlases/<name>` of each atlas, as Futile does,
    /// then the projections (the game loads them when the first image hologram is made;
    /// the port at startup). A missing projection only warns.
    ///
    /// # Errors
    /// A message naming what could not be read.
    pub fn load(resources: &UnityResources, assets: &StreamingAssets) -> Result<Self, String> {
        let atlas = Atlas::from_resources(resources, ATLAS).map_err(|e| e.to_string())?;
        let mut atlas = SpriteAtlas::new(atlas).ok_or_else(|| {
            format!(
                "atlas {ATLAS} has no {} element",
                SpriteAtlas::WHITE_ELEMENT
            )
        })?;
        let mut images = vec![Self::texture(resources, ATLAS)?];
        for name in MORE_ATLASES {
            let page = Atlas::from_resources(resources, name).map_err(|e| e.to_string())?;
            images.push(Self::texture(resources, name)?);
            atlas = atlas.with_page(page);
        }
        for name in PROJECTIONS {
            let path = assets.resolve_file_path(&format!("Projections/{name}.png"));
            match PngImage::load(&path) {
                Ok(image) => {
                    let (w, h) = (image.width() as u32, image.height() as u32);
                    atlas = atlas.with_page(Atlas::single(name, w, h));
                    images.push(image);
                }
                Err(e) => log::warn!("no projection {name} ({e})"),
            }
        }
        Ok(Self { atlas, images })
    }

    fn texture(resources: &UnityResources, name: &str) -> Result<PngImage, String> {
        resources
            .texture_image(&format!("Atlases/{name}"))
            .map_err(|e| e.to_string())
    }

    pub fn upload(&self, renderer: &mut impl Renderer) {
        let images: Vec<&PngImage> = self.images.iter().collect();
        renderer.set_atlas(&self.atlas, &images);
    }
}
