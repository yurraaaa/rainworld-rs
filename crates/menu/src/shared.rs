//! What the menu processes share across processes (`RainWorld` and Futile's globals).

use rw_assets::{StreamingAssets, UnityResources};

use crate::atlas::{AtlasLoadError, AtlasManager};
use crate::input::UiInputState;
use crate::translator::Translator;

/// The game files, loaded atlases and fonts, the translator and the menu input.
pub struct MenuShared {
    pub assets: StreamingAssets,
    pub atlases: AtlasManager,
    pub translator: Translator,
    pub input: UiInputState,
    /// `rainWorld.screenSize`.
    pub screen_size: [f32; 2],
}

impl MenuShared {
    /// Loads the English fonts and strings (`RainWorld.Update` once options are ready).
    ///
    /// # Errors
    /// When the font atlas or a font config cannot be read.
    pub fn load(
        assets: StreamingAssets,
        resources: &UnityResources,
        screen_size: [f32; 2],
    ) -> Result<Self, AtlasLoadError> {
        let mut atlases = AtlasManager::new();
        atlases.load_english_fonts(&assets, resources)?;
        let translator = Translator::load(&assets).unwrap_or_else(|e| {
            log::warn!("no strings ({e}); text is shown untranslated");
            Translator::default()
        });
        Ok(Self {
            assets,
            atlases,
            translator,
            input: UiInputState::default(),
            screen_size,
        })
    }
}

/// An app context the menu processes can run in.
pub trait MenuContext {
    fn menu(&mut self) -> &mut MenuShared;
}
