//! Importing one of the game's save slots: the slot file is read from the game's save
//! directory (never written) and copied into the port's, after the port's previous
//! file for that slot is moved to `backup/<unix seconds>_import/`.
//!
//! Mirrors: -
//! (port-only importer of the game's slot files)

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::dirs::SaveSlot;
use crate::progression::SaveError;
use crate::user_data::{UserDataError, UserDataFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub source: PathBuf,
    pub destination: PathBuf,
    /// Where the port's previous file went, if there was one.
    pub backup: Option<PathBuf>,
    /// The `SAVE STATE` slugcat values in the imported slot, as stored.
    pub save_states: Vec<String>,
}

/// Imports `slot` from `game_dir` into `port_dir`.
///
/// # Errors
/// When the source is missing or not a save file, the directories are the same, or
/// a file cannot be written.
pub fn import_slot(
    game_dir: &Path,
    port_dir: &Path,
    slot: SaveSlot,
) -> Result<ImportReport, SaveError> {
    let name = slot.file_name();
    let source = game_dir.join(&name);
    let destination = port_dir.join(&name);
    let bytes = std::fs::read(&source)?;
    let file = UserDataFile::parse(&bytes)?;
    if let (Ok(a), Ok(b)) = (game_dir.canonicalize(), port_dir.canonicalize())
        && a == b
    {
        return Err(SaveError::Format(UserDataError(
            "the port's save directory is the game's".to_owned(),
        )));
    }
    let save_states = file
        .get("save")
        .map(|s| {
            s.get(32..)
                .unwrap_or("")
                .split("<progDivA>")
                .filter_map(|l| l.strip_prefix("SAVE STATE<progDivB>"))
                .filter_map(|l| l.split("<svA>").next()?.split("<svB>").nth(1))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    std::fs::create_dir_all(port_dir)?;
    let backup = if destination.is_file() {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut dir = port_dir.join("backup").join(format!("{secs}_import"));
        let mut n = 1;
        while dir.join(&name).exists() {
            dir = port_dir.join("backup").join(format!("{secs}_import_{n}"));
            n += 1;
        }
        std::fs::create_dir_all(&dir)?;
        let to = dir.join(&name);
        std::fs::rename(&destination, &to)?;
        Some(to)
    } else {
        None
    };
    let tmp = destination.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &destination)?;
    Ok(ImportReport {
        source,
        destination,
        backup,
        save_states,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot_bytes(text: &str) -> Vec<u8> {
        let mut file = UserDataFile::new();
        file.set("save", format!("{:032}{text}", 0));
        let mut bytes = file.to_bytes();
        bytes.extend([0; 64]);
        bytes
    }

    #[test]
    fn imports_and_backs_up() {
        let game = tempfile::tempdir().unwrap();
        let port = tempfile::tempdir().unwrap();
        let first = slot_bytes("SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA><progDivA>");
        std::fs::write(game.path().join("sav2"), &first).unwrap();
        let report = import_slot(game.path(), port.path(), SaveSlot(1)).unwrap();
        assert_eq!(report.save_states, ["White"]);
        assert_eq!(report.backup, None);
        assert_eq!(std::fs::read(port.path().join("sav2")).unwrap(), first);
        // Again with another file: the port's copy goes to a backup first.
        let second = slot_bytes("SAVE STATE<progDivB>SAV STATE NUMBER<svB>Red<svA><progDivA>");
        std::fs::write(game.path().join("sav2"), &second).unwrap();
        let report = import_slot(game.path(), port.path(), SaveSlot(1)).unwrap();
        let backup = report.backup.unwrap();
        assert_eq!(std::fs::read(backup).unwrap(), first);
        assert_eq!(std::fs::read(port.path().join("sav2")).unwrap(), second);
        // The source is untouched.
        assert_eq!(std::fs::read(game.path().join("sav2")).unwrap(), second);
    }

    #[test]
    fn refuses_bad_sources() {
        let game = tempfile::tempdir().unwrap();
        let port = tempfile::tempdir().unwrap();
        assert!(import_slot(game.path(), port.path(), SaveSlot(0)).is_err());
        std::fs::write(game.path().join("sav"), b"not a save").unwrap();
        assert!(import_slot(game.path(), port.path(), SaveSlot(0)).is_err());
        std::fs::write(game.path().join("sav"), slot_bytes("")).unwrap();
        assert!(import_slot(game.path(), game.path(), SaveSlot(0)).is_err());
        assert!(!port.path().join("sav").exists());
    }
}
