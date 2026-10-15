//! `Custom.Md5Sum`: the MD5 of a save string and a salt, as 32 lower-case hex digits,
//! which `PlayerProgression` puts before the string it stores under `"save"`.
//!
//! The salt is part of a constant in the game's code (`Custom.encrptString`); the port
//! reads it from the installed `Assembly-CSharp.dll` instead of shipping it: the
//! constant is the assembly's only user string of 1000 or more base64 characters.

use std::path::Path;

use md5::{Digest, Md5};

/// `Custom.CheckSumSalt`.
#[derive(Clone, PartialEq, Eq)]
pub struct ChecksumSalt(String);

impl std::fmt::Debug for ChecksumSalt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChecksumSalt(..)")
    }
}

impl ChecksumSalt {
    /// `encrptString.Substring(64, 97)`.
    const START: usize = 64;
    const LEN: usize = 97;
    const MIN_RUN: usize = 1000;

    /// Reads the salt from `Assembly-CSharp.dll` (`RainWorld_Data/Managed`).
    ///
    /// # Errors
    /// When the file cannot be read or holds no such constant.
    pub fn from_assembly(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_assembly_bytes(&bytes).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("no checksum constant in {}", path.display()),
            )
        })
    }

    /// The salt from the first long enough UTF-16LE run of base64 characters.
    #[must_use]
    pub fn from_assembly_bytes(bytes: &[u8]) -> Option<Self> {
        // The user-string heap's strings start at any byte offset.
        for parity in 0..2 {
            let mut run = String::new();
            for pair in bytes.get(parity..)?.as_chunks::<2>().0 {
                if pair[1] == 0 && Self::base64_char(pair[0]) {
                    run.push(char::from(pair[0]));
                    continue;
                }
                if run.len() >= Self::MIN_RUN {
                    break;
                }
                run.clear();
            }
            if run.len() >= Self::MIN_RUN {
                return Some(Self(
                    run.get(Self::START..Self::START + Self::LEN)?.to_owned(),
                ));
            }
        }
        None
    }

    fn base64_char(b: u8) -> bool {
        b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')
    }

    /// A salt given directly (tests).
    #[must_use]
    pub fn new(salt: &str) -> Self {
        Self(salt.to_owned())
    }

    /// `Custom.Md5Sum(text)`.
    #[must_use]
    pub fn md5_sum(&self, text: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(text.as_bytes());
        hasher.update(self.0.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_of_text_and_salt() {
        // MD5("abc") with an empty salt.
        assert_eq!(
            ChecksumSalt::new("").md5_sum("abc"),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(
            ChecksumSalt::new("c").md5_sum("ab"),
            ChecksumSalt::new("").md5_sum("abc")
        );
    }

    #[test]
    fn salt_from_utf16_run() {
        let text: String = (0..1200)
            .map(|i| char::from(b'A' + (i % 26) as u8))
            .collect();
        let mut bytes = vec![0x12, 0x34, 0x56];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        bytes.extend([0x01, 0x02]);
        let salt = ChecksumSalt::from_assembly_bytes(&bytes).unwrap();
        assert_eq!(salt, ChecksumSalt::new(&text[64..161]));
    }
}
