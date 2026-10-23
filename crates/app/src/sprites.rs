//! The room sprites' atlases: `rainWorld` and `uiSprites` from the game's
//! `resources.assets`, the overseer images' projection textures and the rainbow gradient.
//!
//! Mirrors: - (port-only: atlas loading)

use rw_assets::{PngImage, StreamingAssets, UnityResources};
use rw_graphics::{Atlas, RAINBOW_ELEMENT};
use rw_render::{Renderer, SceneTextures, SpriteAtlas};

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
    /// The background scene shaders' global textures.
    scene: Option<SceneTextures>,
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
        // `Rainbow`'s constructor loads `Atlases/Rainbow` as the atlas "rainbow".
        match resources.texture_image("Atlases/Rainbow") {
            Ok(image) => {
                let (w, h) = (image.width() as u32, image.height() as u32);
                atlas = atlas.with_page(Atlas::single(RAINBOW_ELEMENT, w, h));
                images.push(image);
            }
            Err(e) => log::warn!("no rainbow texture ({e})"),
        }
        Ok(Self {
            atlas,
            images,
            scene: Self::scene_textures(resources),
        })
    }

    /// `RainWorld.SetGlobalTextures`: `_CloudsTex` (with its mips), `_ApartmentsTex` and
    /// `_CityPalette` from `resources.assets`; none, with a warning, when one cannot be
    /// read.
    fn scene_textures(resources: &UnityResources) -> Option<SceneTextures> {
        let read = || -> Result<SceneTextures, rw_assets::AssetError> {
            Ok(SceneTextures {
                clouds: resources.texture_mips("Illustrations/cloudsTexture")?.1,
                apartments: resources.texture_image("Illustrations/apartments")?,
                city_palette: resources.texture_image("Palettes/cityPalette")?,
            })
        };
        read()
            .inspect_err(|e| log::warn!("no scene textures ({e})"))
            .ok()
    }

    fn texture(resources: &UnityResources, name: &str) -> Result<PngImage, String> {
        resources
            .texture_image(&format!("Atlases/{name}"))
            .map_err(|e| e.to_string())
    }

    pub fn upload(&self, renderer: &mut impl Renderer) {
        let images: Vec<&PngImage> = self.images.iter().collect();
        renderer.set_atlas(&self.atlas, &images);
        if let Some(scene) = &self.scene {
            renderer.set_scene_textures(scene);
        }
    }
}
