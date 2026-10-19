//! A Futile font's BMFont text config (`Atlases/font`, `Atlases/DisplayFont`,
//! `Atlases/ps4Glyphs` TextAssets), read the way `FFont.LoadAndParseConfigFile` reads it
//! (`rainworld-re/notes/menu.md`): lines split on `\n` (else `\r\n`, else `\r`), tokens
//! on spaces, empty ones dropped. Values stay in the file's pixels; `FFont`'s scaling
//! by the atlas element happens in `rw-menu`.
//!
//! The game reads `common` positionally: the line height is the first field and the
//! texture width the third (`common lineHeight= base= scaleW= ...`).
//!
//! Mirrors: FFont.cs (LoadAndParseConfigFile)

use std::fmt;

/// One `char` line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontChar {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub xoffset: i32,
    pub yoffset: i32,
    pub xadvance: i32,
    pub page: i32,
    /// `letter="a"`: the character between the quotes, when the value has three or
    /// more characters.
    pub letter: Option<String>,
}

/// One `kerning` line; `first` is -1 when the line names none (the game then leaves
/// its slot empty).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontKerning {
    pub first: i32,
    pub second: i32,
    pub amount: i32,
}

/// A parsed font config.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontConfig {
    /// `common`'s first field (`lineHeight`).
    pub line_height: i32,
    /// `common`'s third field (`scaleW`), the width the positions are in.
    pub config_width: i32,
    /// `chars count=`.
    pub chars_count: i32,
    /// In file order.
    pub chars: Vec<FontChar>,
    /// `kernings count=`, `None` without a `kernings` line.
    pub kernings_count: Option<i32>,
    /// In file order, including lines without a `first`.
    pub kernings: Vec<FontKerning>,
}

/// Why a config could not be read; the game throws on the same inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontConfigError(String);

impl fmt::Display for FontConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "font config: {}", self.0)
    }
}

impl std::error::Error for FontConfigError {}

impl FontConfig {
    /// Parses the config text.
    ///
    /// # Errors
    /// When the text has fewer than two lines ("Your font file is messed up") or a
    /// number the game reads does not parse.
    pub fn parse(text: &str) -> Result<Self, FontConfigError> {
        let lines = Self::lines(text)?;
        let mut config = Self::default();
        for line in lines {
            let tokens: Vec<&str> = line.split(' ').filter(|t| !t.is_empty()).collect();
            match tokens.first().copied() {
                Some("common") => {
                    config.config_width = Self::field(&tokens, 3)?;
                    config.line_height = Self::field(&tokens, 1)?;
                }
                Some("chars") => config.chars_count = Self::field(&tokens, 1)?,
                Some("char") => config.chars.push(Self::char_line(&tokens)?),
                Some("kernings") => config.kernings_count = Some(Self::field(&tokens, 1)?),
                Some("kerning") => config.kernings.push(Self::kerning_line(&tokens)?),
                _ => {}
            }
        }
        Ok(config)
    }

    /// The first separator that gives more than one non-empty line.
    fn lines(text: &str) -> Result<Vec<&str>, FontConfigError> {
        for separator in ["\n", "\r\n", "\r"] {
            let lines: Vec<&str> = text.split(separator).filter(|l| !l.is_empty()).collect();
            if lines.len() > 1 {
                return Ok(lines);
            }
        }
        Err(FontConfigError("fewer than two lines".to_owned()))
    }

    /// The value after `=` of token `index`.
    fn field(tokens: &[&str], index: usize) -> Result<i32, FontConfigError> {
        let token = tokens
            .get(index)
            .ok_or_else(|| FontConfigError(format!("missing field {index} in {tokens:?}")))?;
        let value = token
            .split('=')
            .nth(1)
            .ok_or_else(|| FontConfigError(format!("no value in {token:?}")))?;
        Self::number(value)
    }

    /// `int.Parse` with `NumberStyles.Any`: surrounding white space (the `\r` of CRLF
    /// files) is allowed.
    fn number(value: &str) -> Result<i32, FontConfigError> {
        value
            .trim()
            .parse()
            .map_err(|_| FontConfigError(format!("bad number {value:?}")))
    }

    fn char_line(tokens: &[&str]) -> Result<FontChar, FontConfigError> {
        let mut c = FontChar::default();
        for token in &tokens[1..] {
            let mut parts = token.split('=');
            let key = parts.next().unwrap_or_default();
            if key == "letter" {
                let value = parts.next().unwrap_or_default();
                if value.chars().count() >= 3 {
                    c.letter = value.chars().nth(1).map(String::from);
                }
                continue;
            }
            if key == "\r" {
                continue;
            }
            let value = parts
                .next()
                .ok_or_else(|| FontConfigError(format!("no value in {token:?}")))?;
            let n = Self::number(value)?;
            match key {
                "id" => c.id = n,
                "x" => c.x = n,
                "y" => c.y = n,
                "width" => c.width = n,
                "height" => c.height = n,
                "xoffset" => c.xoffset = n,
                "yoffset" => c.yoffset = n,
                "xadvance" => c.xadvance = n,
                "page" => c.page = n,
                _ => {}
            }
        }
        Ok(c)
    }

    fn kerning_line(tokens: &[&str]) -> Result<FontKerning, FontConfigError> {
        let mut k = FontKerning {
            first: -1,
            second: 0,
            amount: 0,
        };
        for token in &tokens[1..] {
            let parts: Vec<&str> = token.split('=').collect();
            if parts.len() < 2 {
                continue;
            }
            let n = Self::number(parts[1])?;
            match parts[0] {
                "first" => k.first = n,
                "second" => k.second = n,
                "amount" => k.amount = n,
                _ => {}
            }
        }
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "info face=\"x\" size=15\r\ncommon lineHeight=15 base=12 scaleW=256 scaleH=256\r\n\
chars count=2\r\nchar id=32   x=150   y=85    width=3     height=1     xoffset=-1    yoffset=14    xadvance=3     page=0  chnl=15\r\n\
char id=65 x=1 y=2 width=7 height=9 xoffset=0 yoffset=3 xadvance=8 page=0 letter=\"A\"\r\n\
kernings count=1\r\nkerning first=65  second=86 amount=-1  \r\n";

    #[test]
    fn crlf_sample() {
        let c = FontConfig::parse(SAMPLE).unwrap();
        assert_eq!(c.line_height, 15);
        assert_eq!(c.config_width, 256);
        assert_eq!(c.chars_count, 2);
        assert_eq!(c.chars.len(), 2);
        assert_eq!(c.chars[0].xoffset, -1);
        assert_eq!(c.chars[1].letter.as_deref(), Some("A"));
        assert_eq!(c.kernings_count, Some(1));
        assert_eq!(
            c.kernings,
            [FontKerning {
                first: 65,
                second: 86,
                amount: -1
            }]
        );
    }

    #[test]
    fn one_line_is_rejected() {
        assert!(FontConfig::parse("common lineHeight=1").is_err());
    }
}
