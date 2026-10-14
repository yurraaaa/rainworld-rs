//! `text/text_<lang>/strings.txt`, the `key|value` table `InGameTranslator` loads in
//! `LoadShortStrings` (`rainworld-re/notes/menu.md`).
//!
//! As the game: the text is read as UTF-8 (a BOM is dropped); a leading `0` marks a
//! plain file (the `0` is removed); a leading `1` an encrypted one (`Custom.xorEncrypt`
//! with a key string embedded in the game's code), which is not read: no vanilla
//! `strings.txt` is encrypted;
//! lines are split on `\r\n` only; a line containing `///` is cut at its first `/` and
//! trimmed at the end; a line splitting on `|` into at least two parts with a non-empty
//! second part maps the first part to the second (later lines win), anything else is
//! skipped.

use std::collections::HashMap;
use std::path::Path;

use crate::{AssetError, StreamingAssets};

/// A loaded table and what the loader skipped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShortStrings {
    pub entries: HashMap<String, String>,
    /// Lines (after the `\r\n` split) that gave no entry.
    pub rejected: usize,
    /// Lines whose key was already present.
    pub replaced: usize,
    /// Encrypted files skipped.
    pub encrypted: usize,
}

impl ShortStrings {
    /// `text/text_<lang>/strings.txt` (lower case, as `ShortStringsFilePaths`).
    #[must_use]
    pub fn path(assets: &StreamingAssets, lang: &str) -> std::path::PathBuf {
        assets.file(&format!("text/text_{lang}/strings.txt"))
    }

    /// Reads one file into a new table.
    ///
    /// # Errors
    /// When the file cannot be read.
    pub fn load(path: &Path) -> Result<Self, AssetError> {
        let mut table = Self::default();
        table.load_more(path)?;
        Ok(table)
    }

    /// Reads one more file over this table (the current language over English).
    ///
    /// # Errors
    /// When the file cannot be read.
    pub fn load_more(&mut self, path: &Path) -> Result<(), AssetError> {
        let bytes = std::fs::read(path).map_err(|e| AssetError::io(path, e))?;
        let text = String::from_utf8_lossy(&bytes);
        self.add_text(&text);
        Ok(())
    }

    /// Adds the entries of a file's text.
    pub fn add_text(&mut self, text: &str) {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let text = match text.chars().next() {
            Some('1') => {
                self.encrypted += 1;
                return;
            }
            Some('0') => &text[1..],
            _ => text,
        };
        for line in text.split("\r\n") {
            let line = if line.contains("///") {
                line.split('/').next().unwrap_or_default().trim_end()
            } else {
                line
            };
            let mut parts = line.split('|');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) if !value.is_empty() => {
                    if self
                        .entries
                        .insert(key.to_owned(), value.to_owned())
                        .is_some()
                    {
                        self.replaced += 1;
                    }
                }
                _ => self.rejected += 1,
            }
        }
    }

    /// `Translate`: the value, or the key itself when missing (a distribution build).
    #[must_use]
    pub fn translate<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries.get(key).map_or(key, String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_file_rules() {
        let mut t = ShortStrings::default();
        t.add_text("\u{feff}0A|a\r\nB|\r\nC|c|x\r\nD|d /// note\r\nnone\r\nA|b\r\n");
        assert_eq!(t.translate("A"), "b");
        assert_eq!(t.translate("C"), "c");
        // Cut at the first '/', then trimmed.
        assert_eq!(t.translate("D"), "d");
        assert_eq!(t.translate("B"), "B");
        assert_eq!(t.translate("missing"), "missing");
        // "B|", "none" and the empty line after the last CRLF.
        assert_eq!(t.rejected, 3);
        assert_eq!(t.replaced, 1);
    }

    #[test]
    fn lf_only_file_is_one_line() {
        let mut t = ShortStrings::default();
        t.add_text("0A|a\nB|b\n");
        assert_eq!(t.entries.len(), 1);
        assert_eq!(t.translate("A"), "a\nB");
    }
}
