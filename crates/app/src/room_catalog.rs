//! The rooms on disk a game can start in (`list_rooms`, the debug UI's room list):
//! `world/<region>-rooms`, `world/gates` and `levels`, each room with a first camera
//! image.

use std::path::PathBuf;

use rw_assets::StreamingAssets;

/// The room directories, by region (`gates` and `arena` for the gates and levels).
pub struct RoomCatalog {
    dirs: Vec<(String, PathBuf)>,
}

impl RoomCatalog {
    /// # Errors
    /// When the world directory cannot be read.
    pub fn read(assets: &StreamingAssets) -> Result<Self, String> {
        let world = assets.file("world");
        let mut dirs: Vec<(String, PathBuf)> = std::fs::read_dir(&world)
            .map_err(|e| format!("cannot read {}: {e}", world.display()))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let region = name.strip_suffix("-rooms")?.to_owned();
                Some((region, entry.path()))
            })
            .collect();
        dirs.sort();
        dirs.push(("gates".to_owned(), assets.file("world/gates")));
        dirs.push(("arena".to_owned(), assets.file("levels")));
        Ok(Self { dirs })
    }

    /// The regions' acronyms (lower case, as on disk), sorted.
    #[must_use]
    pub fn regions(&self) -> Vec<String> {
        self.dirs
            .iter()
            .map(|(r, _)| r.clone())
            .filter(|r| r != "gates" && r != "arena")
            .collect()
    }

    /// Room names (lower case) of `region` (every one when `None`) starting with
    /// `prefix` (lower case), sorted and deduplicated.
    #[must_use]
    pub fn rooms(&self, region: Option<&str>, prefix: &str) -> Vec<String> {
        let mut rooms: Vec<String> = self
            .dirs
            .iter()
            .filter(|(r, _)| region.is_none_or(|want| want == r))
            .filter_map(|(_, dir)| std::fs::read_dir(dir).ok())
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let file = entry.file_name().into_string().ok()?;
                let name = file.strip_suffix(".txt")?;
                let image = entry.path().with_file_name(format!("{name}_1.png"));
                (name.starts_with(prefix) && image.is_file()).then(|| name.to_owned())
            })
            .collect();
        rooms.sort();
        rooms.dedup();
        rooms
    }
}
