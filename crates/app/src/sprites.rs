//! The room sprites' atlases: `rainWorld` and `uiSprites` from the game's
//! `resources.assets`, the overseer images' projection textures and the rainbow gradient.
//!
//! Mirrors: - (port-only: atlas loading)

use rw_assets::unity::{AnisotropicFiltering, Texture2D};
use rw_assets::{PngImage, StreamingAssets, UnityResources};
use rw_graphics::{Atlas, RAINBOW_ELEMENT};
use rw_render::{Renderer, SceneTextures, SpriteAtlas, TextureSampling};

/// Atlas name holding the slugcat's elements (and `Futile_White`'s stand-in).
const ATLAS: &str = "rainWorld";

/// Further atlases, in `RainWorld.LoadResources`' order (the holograms' symbols and
/// keys are in `uiSprites`).
const MORE_ATLASES: [&str; 2] = ["uiSprites", "shelterGate"];

/// The atlas `RainWorld.LoadResources` sets to point filtering after loading it.
const POINT_FILTERED: &str = "shelterGate";

/// `HoloImage.LoadFile`'s textures: `Projections/<name>.png`, an atlas of one element.
const PROJECTIONS: [&str; 2] = ["RND_PROJ", "STR_PROJ"];

/// Frame data and one image per page of the sprite atlases.
pub struct SpriteAssets {
    atlas: SpriteAtlas,
    images: Vec<PngImage>,
    /// Each page's filter and wrap: its Texture2D's import settings.
    sampling: Vec<TextureSampling>,
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
        let mut images = Vec::new();
        let mut sampling = Vec::new();
        Self::push_texture(resources, ATLAS, &mut images, &mut sampling)?;
        for name in MORE_ATLASES {
            let page = Atlas::from_resources(resources, name).map_err(|e| e.to_string())?;
            Self::push_texture(resources, name, &mut images, &mut sampling)?;
            if name == POINT_FILTERED
                && let Some(last) = sampling.last_mut()
            {
                last.filter = rw_render::TextureFilter::Point;
            }
            atlas = atlas.with_page(page);
        }
        for name in PROJECTIONS {
            let path = assets.resolve_file_path(&format!("Projections/{name}.png"));
            match PngImage::load(&path) {
                Ok(image) => {
                    let (w, h) = (image.width() as u32, image.height() as u32);
                    atlas = atlas.with_page(Atlas::single(name, w, h));
                    images.push(image);
                    // `OverseerImage.LoadFile`: clamped and crisp.
                    sampling.push(TextureSampling::POINT_CLAMP);
                }
                Err(e) => log::warn!("no projection {name} ({e})"),
            }
        }
        // `Rainbow`'s constructor loads `Atlases/Rainbow` as the atlas "rainbow".
        match resources.texture_with_mips("Atlases/Rainbow") {
            Ok((texture, image)) => {
                let (w, h) = (image.width() as u32, image.height() as u32);
                atlas = atlas.with_page(Atlas::single(RAINBOW_ELEMENT, w, h));
                images.push(image);
                sampling.push(Self::import_sampling(&texture));
            }
            Err(e) => log::warn!("no rainbow texture ({e})"),
        }
        // `RainWorld.LoadResources` loads `Atlases/LightMask0` as the atlas "LightMask0".
        match resources.texture_with_mips("Atlases/LightMask0") {
            Ok((texture, image)) => {
                let (w, h) = (image.width() as u32, image.height() as u32);
                atlas = atlas.with_page(Atlas::single("LightMask0", w, h));
                images.push(image);
                sampling.push(Self::import_sampling(&texture));
            }
            Err(e) => log::warn!("no LightMask0 texture ({e})"),
        }
        // `RainWorld.LoadResources` loads `Atlases/glyphs` as the atlas "glyphs" (the
        // `GlyphProjection` shader's `_MainTex`).
        match resources.texture_with_mips("Atlases/glyphs") {
            Ok((texture, image)) => {
                let (w, h) = (image.width() as u32, image.height() as u32);
                atlas = atlas.with_page(Atlas::single("glyphs", w, h));
                images.push(image);
                sampling.push(Self::import_sampling(&texture));
            }
            Err(e) => log::warn!("no glyphs texture ({e})"),
        }
        // `RainWorld.LoadResources` loads `Atlases/maze` as the atlas "maze" (the
        // `CoralCircuit` shaders' `_MainTex`: bilinear, repeat, with its stored mips, D42).
        match resources.texture_with_mips("Atlases/maze") {
            Ok((texture, image)) => {
                let (w, h) = (image.width() as u32, image.height() as u32);
                atlas = atlas.with_page(Atlas::single(rw_graphics::MAZE_ELEMENT, w, h));
                images.push(image);
                sampling.push(Self::import_sampling(&texture));
            }
            Err(e) => log::warn!("no maze texture ({e})"),
        }
        Ok(Self {
            atlas,
            images,
            sampling,
            scene: Self::scene_textures(resources),
        })
    }

    /// `RainWorld.SetGlobalTextures`: `_CloudsTex` (with its mips), `_ApartmentsTex` and
    /// `_CityPalette` from `resources.assets`; none, with a warning, when one cannot be
    /// read.
    fn scene_textures(resources: &UnityResources) -> Option<SceneTextures> {
        Self::read_scene_textures(resources)
            .inspect_err(|e| log::warn!("no scene textures ({e})"))
            .ok()
    }

    fn read_scene_textures(
        resources: &UnityResources,
    ) -> Result<SceneTextures, rw_assets::AssetError> {
        let (clouds_texture, clouds) = resources.texture_mips("Illustrations/cloudsTexture")?;
        Ok(SceneTextures {
            clouds,
            clouds_aniso: clouds_texture.effective_aniso(AnisotropicFiltering::GAME),
            apartments: resources.texture_image("Illustrations/apartments")?,
            city_palette: resources.texture_image("Palettes/cityPalette")?,
        })
    }

    /// The sampling Unity gives `texture`: its own filter and wrap mode.
    fn import_sampling(texture: &Texture2D) -> TextureSampling {
        TextureSampling::from_import(texture.filter_mode, [texture.wrap[0], texture.wrap[1]])
    }

    fn push_texture(
        resources: &UnityResources,
        name: &str,
        images: &mut Vec<PngImage>,
        sampling: &mut Vec<TextureSampling>,
    ) -> Result<(), String> {
        let (texture, image) = resources
            .texture_with_settings(&format!("Atlases/{name}"))
            .map_err(|e| e.to_string())?;
        images.push(image);
        sampling.push(Self::import_sampling(&texture));
        Ok(())
    }

    pub fn upload(&self, renderer: &mut impl Renderer) {
        let images: Vec<&PngImage> = self.images.iter().collect();
        renderer.set_atlas(&self.atlas, &images, &self.sampling);
        if let Some(scene) = &self.scene {
            renderer.set_scene_textures(scene);
        }
    }
}
