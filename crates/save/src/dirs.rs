//! Where saves live: the port's own data directory, and the game's
//! `Application.persistentDataPath`, which the importer only reads. The platform is a
//! parameter so each resolution is testable anywhere (Android comes later).

use std::path::{Path, PathBuf};

/// The platforms with known paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    Other,
}

/// A save slot: `Options.saveSlot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SaveSlot(pub u8);

impl SaveSlot {
    pub const ALL: [Self; 3] = [Self(0), Self(1), Self(2)];

    /// `GetSaveFileName`: `sav`, `sav2`, `sav3`.
    #[must_use]
    pub fn file_name(self) -> String {
        match self.0 {
            0 => "sav".to_owned(),
            n => format!("sav{}", u32::from(n) + 1),
        }
    }

    /// The slot of a file name, if it is one.
    #[must_use]
    pub fn from_file_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.file_name() == name)
    }
}

impl Platform {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }

    /// The port's save directory under the platform data directory
    /// (`~/.local/share/rainworld-rs`, `%APPDATA%\rainworld-rs`).
    #[must_use]
    pub fn port_save_dir_in(data_dir: &Path) -> PathBuf {
        data_dir.join("rainworld-rs")
    }

    /// The port's save directory, if the platform has a data directory.
    #[must_use]
    pub fn port_save_dir() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|b| Self::port_save_dir_in(b.data_dir()))
    }

    /// The game's `persistentDataPath`: on Linux the Proton prefix next to the game's
    /// `steamapps/common/Rain World` (`game_dir`), on Windows under the user's
    /// `AppData\LocalLow` (`home`).
    #[must_use]
    pub fn game_save_dir(self, game_dir: &Path, home: Option<&Path>) -> Option<PathBuf> {
        match self {
            Self::Linux => {
                let steamapps = game_dir.parent()?.parent()?;
                Some(steamapps.join(
                    "compatdata/312520/pfx/drive_c/users/steamuser/AppData/LocalLow/Videocult/Rain World",
                ))
            }
            Self::Windows => Some(
                home?
                    .join("AppData")
                    .join("LocalLow")
                    .join("Videocult")
                    .join("Rain World"),
            ),
            Self::Other => None,
        }
    }

    /// [`Self::game_save_dir`] with this machine's home directory.
    #[must_use]
    pub fn game_save_dir_here(self, game_dir: &Path) -> Option<PathBuf> {
        let base = directories::BaseDirs::new();
        self.game_save_dir(game_dir, base.as_ref().map(directories::BaseDirs::home_dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_files() {
        let names: Vec<String> = SaveSlot::ALL.iter().map(|s| s.file_name()).collect();
        assert_eq!(names, ["sav", "sav2", "sav3"]);
        assert_eq!(SaveSlot::from_file_name("sav3"), Some(SaveSlot(2)));
        assert_eq!(SaveSlot::from_file_name("exp1"), None);
    }

    #[test]
    fn platform_paths() {
        let game = Path::new("/home/u/.local/share/Steam/steamapps/common/Rain World");
        assert_eq!(
            Platform::Linux.game_save_dir(game, None).unwrap(),
            Path::new(
                "/home/u/.local/share/Steam/steamapps/compatdata/312520/pfx/drive_c/users/\
                 steamuser/AppData/LocalLow/Videocult/Rain World"
            )
        );
        let home = Path::new("C:/Users/u");
        assert_eq!(
            Platform::Windows.game_save_dir(game, Some(home)).unwrap(),
            home.join("AppData/LocalLow/Videocult/Rain World")
        );
        assert_eq!(Platform::Windows.game_save_dir(game, None), None);
        assert_eq!(Platform::Other.game_save_dir(game, Some(home)), None);
        assert_eq!(
            Platform::port_save_dir_in(Path::new("/home/u/.local/share")),
            Path::new("/home/u/.local/share/rainworld-rs")
        );
    }
}
