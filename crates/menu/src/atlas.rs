//! `FAtlasManager`: atlases (a texture each) and their elements by name, and the
//! loaded fonts. Atlases come from the game's Unity resources (`Atlases/<name>`), from
//! loose PNGs (menu illustrations, one element each) or are made in code
//! (`Futile_White`).
//!
//! Textures are kept here with their sampling; a [`TextureSync`] uploads them to a
//! renderer and frees the ones unloaded since.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use rw_assets::{AssetError, PngImage, StreamingAssets, UnityResources};
use rw_graphics::Atlas;
use rw_render::{Renderer, StageTextureId, TextureFilter, TextureSampling};

use crate::font::Font;
use crate::geometry::Rect;

/// One `FAtlasElement`.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    /// The atlas's texture.
    pub texture: StageTextureId,
    /// The atlas texture's size in pixels.
    pub texture_size: [f32; 2],
    /// Texture coordinates, `y` from the bottom.
    pub uv_rect: Rect,
    /// The untrimmed image size.
    pub source_size: [f32; 2],
    pub source_pixel_size: [f32; 2],
    /// Where the trimmed frame sits in the untrimmed image, `y` from its top.
    pub source_rect: Rect,
    pub trimmed: bool,
}

/// One loaded atlas.
#[derive(Debug, Clone)]
struct AtlasEntry {
    name: String,
    texture: StageTextureId,
    elements: Vec<String>,
}

/// A texture kept for upload.
#[derive(Debug, Clone)]
struct TextureEntry {
    image: Arc<PngImage>,
    sampling: TextureSampling,
}

/// Futile's atlas manager.
#[derive(Debug, Default)]
pub struct AtlasManager {
    atlases: Vec<AtlasEntry>,
    elements: HashMap<String, Element>,
    textures: BTreeMap<StageTextureId, TextureEntry>,
    fonts: HashMap<String, Arc<Font>>,
    next_texture: u32,
}

/// Why an atlas or font could not be loaded.
#[derive(Debug)]
pub enum AtlasLoadError {
    Asset(AssetError),
    Atlas(rw_graphics::AtlasError),
    Font(rw_assets::FontConfigError),
    /// Futile throws on a second element of the same name.
    DuplicateElement(String),
    MissingElement(String),
    MissingFont(String),
}

impl std::fmt::Display for AtlasLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asset(e) => write!(f, "{e}"),
            Self::Atlas(e) => write!(f, "{e}"),
            Self::Font(e) => write!(f, "{e}"),
            Self::DuplicateElement(n) => write!(f, "duplicate element name {n:?}"),
            Self::MissingElement(n) => write!(f, "no element named {n:?}"),
            Self::MissingFont(n) => write!(f, "no font named {n:?}"),
        }
    }
}

impl std::error::Error for AtlasLoadError {}

impl From<AssetError> for AtlasLoadError {
    fn from(e: AssetError) -> Self {
        Self::Asset(e)
    }
}

impl AtlasManager {
    /// The element `Futile.CreateDefaultAtlases` makes: a 16x16 opaque white texture,
    /// bilinear and clamped.
    pub const WHITE: &'static str = "Futile_White";

    /// A manager holding `Futile_White`, as after `Futile.Init`.
    #[must_use]
    pub fn new() -> Self {
        let mut manager = Self::default();
        let white = PngImage::from_pixels(16, 16, vec![[255; 4]; 256]);
        let sampling = TextureSampling {
            filter: TextureFilter::Bilinear,
            clamp: true,
        };
        manager
            .add_single_image(Self::WHITE, white, sampling)
            .expect("the first atlas has no duplicate");
        manager
    }

    #[must_use]
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements.get(name)
    }

    #[must_use]
    pub fn contains_atlas(&self, name: &str) -> bool {
        self.atlases.iter().any(|a| a.name == name)
    }

    #[must_use]
    pub fn font(&self, name: &str) -> Option<&Arc<Font>> {
        self.fonts.get(name)
    }

    /// The image of an atlas texture.
    #[must_use]
    pub fn texture_image(&self, id: StageTextureId) -> Option<&Arc<PngImage>> {
        self.textures.get(&id).map(|t| &t.image)
    }

    fn new_texture(&mut self, image: PngImage, sampling: TextureSampling) -> StageTextureId {
        let id = StageTextureId(self.next_texture);
        self.next_texture += 1;
        self.textures.insert(
            id,
            TextureEntry {
                image: Arc::new(image),
                sampling,
            },
        );
        id
    }

    /// `FAtlas.CreateAtlasFromSingleImage`: one element, named as the atlas, covering
    /// the whole texture.
    fn add_single_image(
        &mut self,
        name: &str,
        image: PngImage,
        sampling: TextureSampling,
    ) -> Result<(), AtlasLoadError> {
        let size = [image.width() as f32, image.height() as f32];
        let texture = self.new_texture(image, sampling);
        let element = Element {
            name: name.to_owned(),
            texture,
            texture_size: size,
            uv_rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            source_size: size,
            source_pixel_size: size,
            source_rect: Rect::new(0.0, 0.0, size[0], size[1]),
            trimmed: false,
        };
        self.add_atlas(name, texture, vec![element])
    }

    /// `FAtlasManager.AddAtlas`: registers the elements, refusing duplicate names.
    fn add_atlas(
        &mut self,
        name: &str,
        texture: StageTextureId,
        elements: Vec<Element>,
    ) -> Result<(), AtlasLoadError> {
        if let Some(dup) = elements
            .iter()
            .find(|e| self.elements.contains_key(&e.name))
        {
            self.textures.remove(&texture);
            return Err(AtlasLoadError::DuplicateElement(dup.name.clone()));
        }
        let names = elements.iter().map(|e| e.name.clone()).collect();
        for element in elements {
            self.elements.insert(element.name.clone(), element);
        }
        self.atlases.push(AtlasEntry {
            name: name.to_owned(),
            texture,
            elements: names,
        });
        Ok(())
    }

    /// `FAtlasManager.LoadAtlas(path)`: the texture is `<path>.png` resolved under
    /// `StreamingAssets` when that file exists (point filter, repeat), else the
    /// Texture2D resource `path` (its own filter and wrap); the frame data is
    /// `<path>.txt` likewise, else the TextAsset `path`. Loading a loaded atlas does
    /// nothing.
    ///
    /// # Errors
    /// When the texture or data cannot be read, or an element name is taken.
    pub fn load_atlas(
        &mut self,
        assets: &StreamingAssets,
        resources: &UnityResources,
        path: &str,
    ) -> Result<(), AtlasLoadError> {
        if self.contains_atlas(path) {
            return Ok(());
        }
        let png = assets.resolve_file_path(&format!("{path}.png"));
        let (image, sampling) = if png.is_file() {
            let sampling = TextureSampling {
                filter: TextureFilter::Point,
                clamp: false,
            };
            (PngImage::load(&png)?, sampling)
        } else {
            let texture: rw_assets::unity::Texture2D = resources.load(path)?;
            let sampling = TextureSampling {
                filter: if texture.filter_mode == 0 {
                    TextureFilter::Point
                } else {
                    TextureFilter::Bilinear
                },
                clamp: texture.wrap[0] == 1,
            };
            (resources.texture_image(path)?, sampling)
        };
        let data = assets.resolve_file_path(&format!("{path}.txt"));
        let atlas = if data.is_file() {
            Atlas::load(&data)
        } else {
            Atlas::from_resources(resources, path.strip_prefix("Atlases/").unwrap_or(path))
        }
        .map_err(AtlasLoadError::Atlas)?;
        let size = [image.width() as f32, image.height() as f32];
        let texture = self.new_texture(image, sampling);
        let mut elements: Vec<Element> = atlas
            .frames
            .iter()
            .map(|(name, frame)| Element {
                name: name.clone(),
                texture,
                texture_size: size,
                uv_rect: Rect::new(
                    frame.frame.x as f32 / size[0],
                    (size[1] - frame.frame.y as f32 - frame.frame.h as f32) / size[1],
                    frame.frame.w as f32 / size[0],
                    frame.frame.h as f32 / size[1],
                ),
                source_size: [frame.source_size.w as f32, frame.source_size.h as f32],
                source_pixel_size: [frame.source_size.w as f32, frame.source_size.h as f32],
                source_rect: Rect::new(
                    frame.sprite_source_size.x as f32,
                    frame.sprite_source_size.y as f32,
                    frame.sprite_source_size.w as f32,
                    frame.sprite_source_size.h as f32,
                ),
                trimmed: frame.trimmed,
            })
            .collect();
        elements.sort_by(|a, b| a.name.cmp(&b.name));
        self.add_atlas(path, texture, elements)
    }

    /// A Texture2D resource kept as a texture with no atlas (`Resources.Load(path) as
    /// Texture2D`, e.g. a shader's global texture), with its own filter and wrap.
    ///
    /// # Errors
    /// When the resource cannot be read.
    pub fn load_resource_texture(
        &mut self,
        resources: &UnityResources,
        path: &str,
    ) -> Result<StageTextureId, AtlasLoadError> {
        let texture: rw_assets::unity::Texture2D = resources.load(path)?;
        let sampling = TextureSampling {
            filter: if texture.filter_mode == 0 {
                TextureFilter::Point
            } else {
                TextureFilter::Bilinear
            },
            clamp: texture.wrap[0] == 1,
        };
        let image = resources.texture_image(path)?;
        Ok(self.new_texture(image, sampling))
    }

    /// `MenuIllustration.LoadFile(folder)`: reuses the atlas named `file` if loaded,
    /// else reads `<folder>/<file>.png` (resolved as the game does) into a one-element
    /// atlas named `file`, clamped, point filtered when `crisp_pixels` else bilinear.
    /// Returns the texture's size.
    ///
    /// # Errors
    /// When the PNG cannot be read. (The game logs a warning and goes on with a 1x1
    /// texture; the caller decides.)
    pub fn load_illustration(
        &mut self,
        assets: &StreamingAssets,
        folder: &str,
        file: &str,
        crisp_pixels: bool,
    ) -> Result<[f32; 2], AtlasLoadError> {
        if let Some(element) = self.element(file).filter(|_| self.contains_atlas(file)) {
            return Ok(element.texture_size);
        }
        let path = assets.resolve_file_path(&format!("{folder}/{file}.png"));
        let image = PngImage::load(&path)?;
        let size = [image.width() as f32, image.height() as f32];
        let sampling = TextureSampling {
            filter: if crisp_pixels {
                TextureFilter::Point
            } else {
                TextureFilter::Bilinear
            },
            clamp: true,
        };
        self.add_single_image(file, image, sampling)?;
        Ok(size)
    }

    /// `FAtlasManager.UnloadAtlas`: drops the atlas, its elements and its texture.
    pub fn unload_atlas(&mut self, name: &str) {
        while let Some(i) = self.atlases.iter().position(|a| a.name == name) {
            let atlas = self.atlases.remove(i);
            for element in &atlas.elements {
                self.elements.remove(element);
            }
            self.textures.remove(&atlas.texture);
        }
    }

    /// `FAtlasManager.LoadFont`: a font from the TextAsset `config_path` placed on
    /// element `element`.
    ///
    /// # Errors
    /// When the element or the config is missing or malformed.
    pub fn load_font(
        &mut self,
        resources: &UnityResources,
        name: &str,
        element: &str,
        config_path: &str,
        offset: [f32; 2],
        font_scale: f32,
    ) -> Result<(), AtlasLoadError> {
        let font = self.read_font(resources, name, element, config_path, offset, font_scale)?;
        self.fonts.insert(name.to_owned(), Arc::new(font));
        Ok(())
    }

    /// `FAtlasManager.CombineFonts`: adds the characters of another font config that
    /// the font `base` lacks.
    ///
    /// # Errors
    /// When the base font, the element or the config is missing or malformed.
    #[allow(clippy::too_many_arguments)]
    pub fn combine_fonts(
        &mut self,
        resources: &UnityResources,
        base: &str,
        add_name: &str,
        add_element: &str,
        add_config_path: &str,
        add_offset_y: f32,
        add_font_scale: f32,
    ) -> Result<(), AtlasLoadError> {
        if !self.fonts.contains_key(base) {
            return Err(AtlasLoadError::MissingFont(base.to_owned()));
        }
        let add = self.read_font(
            resources,
            add_name,
            add_element,
            add_config_path,
            [0.0, 0.0],
            add_font_scale,
        )?;
        let font = self
            .fonts
            .get_mut(base)
            .ok_or_else(|| AtlasLoadError::MissingFont(base.to_owned()))?;
        Arc::make_mut(font).combine_with(&add, add_offset_y);
        Ok(())
    }

    fn read_font(
        &self,
        resources: &UnityResources,
        name: &str,
        element: &str,
        config_path: &str,
        offset: [f32; 2],
        font_scale: f32,
    ) -> Result<Font, AtlasLoadError> {
        let element = self
            .element(element)
            .ok_or_else(|| AtlasLoadError::MissingElement(element.to_owned()))?;
        let text = resources.text(config_path)?;
        let config = rw_assets::FontConfig::parse(&String::from_utf8_lossy(&text))
            .map_err(AtlasLoadError::Font)?;
        Ok(Font::new(name, element, &config, offset, font_scale))
    }

    /// `InGameTranslator.LoadFonts` for English: the `Atlases/fontAtlas` atlas, the
    /// font `font` (with the PlayStation button glyphs of `ps4Glyphs` at half scale) and
    /// `DisplayFont`.
    ///
    /// # Errors
    /// When an atlas or font cannot be read.
    pub fn load_english_fonts(
        &mut self,
        assets: &StreamingAssets,
        resources: &UnityResources,
    ) -> Result<(), AtlasLoadError> {
        self.load_atlas(assets, resources, "Atlases/fontAtlas")?;
        if self.font(Font::FONT).is_none() {
            self.load_font(
                resources,
                Font::FONT,
                "font",
                "Atlases/font",
                [0.0, 0.0],
                1.0,
            )?;
            self.combine_fonts(
                resources,
                Font::FONT,
                "ps4Glyphs",
                "ps4Glyphs",
                "Atlases/ps4Glyphs",
                0.0,
                0.5,
            )?;
        }
        if self.font(Font::DISPLAY_FONT).is_none() {
            self.load_font(
                resources,
                Font::DISPLAY_FONT,
                "DisplayFont",
                "Atlases/DisplayFont",
                [0.0, 0.0],
                1.0,
            )?;
        }
        Ok(())
    }
}

/// What of an [`AtlasManager`]'s textures one renderer holds.
#[derive(Debug, Default)]
pub struct TextureSync {
    uploaded: HashSet<StageTextureId>,
}

impl TextureSync {
    /// Uploads the textures `atlases` has that `renderer` lacks and frees the ones it
    /// no longer has.
    pub fn sync(&mut self, atlases: &AtlasManager, renderer: &mut impl Renderer) {
        let stale: Vec<StageTextureId> = self
            .uploaded
            .iter()
            .filter(|id| !atlases.textures.contains_key(id))
            .copied()
            .collect();
        for id in stale {
            renderer.remove_stage_texture(id);
            self.uploaded.remove(&id);
        }
        for (id, texture) in &atlases.textures {
            if self.uploaded.insert(*id) {
                renderer.set_stage_texture(*id, &texture.image, texture.sampling);
            }
        }
    }
}
