//! `InGameTranslator` for English: `Translate` looks the text up in
//! `text/text_eng/strings.txt` and returns it unchanged when missing (a distribution
//! build). Other languages and mods' string files are not loaded.
//!
//! Mirrors: InGameTranslator.cs

use rw_assets::{AssetError, ShortStrings, StreamingAssets};

#[derive(Debug, Clone, Default)]
pub struct Translator {
    strings: ShortStrings,
}

impl Translator {
    /// `LangShort(English)`.
    pub const ENGLISH: &'static str = "eng";

    /// `LoadShortStrings` for English. A missing file leaves the table empty, as the
    /// game.
    ///
    /// # Errors
    /// When the file exists but cannot be read.
    pub fn load(assets: &StreamingAssets) -> Result<Self, AssetError> {
        let path = ShortStrings::path(assets, Self::ENGLISH);
        if !path.is_file() {
            return Ok(Self::default());
        }
        Ok(Self {
            strings: ShortStrings::load(&path)?,
        })
    }

    #[must_use]
    pub fn translate<'a>(&'a self, text: &'a str) -> &'a str {
        self.strings.translate(text)
    }

    #[must_use]
    pub fn strings(&self) -> &ShortStrings {
        &self.strings
    }
}
