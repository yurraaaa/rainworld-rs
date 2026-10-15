//! The region map's files in `world/<region>/`: the map image `map_<region>.png` (three
//! layers stacked bottom to top) and the shortcut lines of `map_<region>.txt`
//! (`HUD.Map.LoadConnectionPositions`).

use std::fs;
use std::path::PathBuf;

use crate::StreamingAssets;

/// One `Connection: A,B,ax,ay,bx,by,dirA,dirB` line: a room exit pair drawn as a line on
/// the map, positions in tiles, directions indexing `Custom.fourDirections`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapConnectionLine {
    pub room_a: String,
    pub room_b: String,
    pub pos_a: (i32, i32),
    pub pos_b: (i32, i32),
    pub dir_a: i32,
    pub dir_b: i32,
}

impl MapConnectionLine {
    /// Parses one line; `None` unless it is a `Connection` with eight fields. Like the
    /// game, the `HR_LAYERS_OF_REALITY` lines are dropped.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let line = if line.ends_with(':') {
            format!("{line} ")
        } else {
            line.to_owned()
        };
        let parts: Vec<&str> = line.split(": ").collect();
        if parts.len() != 2 || parts[0] != "Connection" {
            return None;
        }
        let f: Vec<&str> = parts[1].split(',').collect();
        if f.len() != 8 || f[0] == "HR_LAYERS_OF_REALITY" || f[1] == "HR_LAYERS_OF_REALITY" {
            return None;
        }
        let n: Vec<i32> = f[2..]
            .iter()
            .map(|v| v.trim().parse::<i32>().ok())
            .collect::<Option<_>>()?;
        Some(Self {
            room_a: f[0].to_owned(),
            room_b: f[1].to_owned(),
            pos_a: (n[0], n[1]),
            pos_b: (n[2], n[3]),
            dir_a: n[4],
            dir_b: n[5],
        })
    }
}

/// Where a region's map files are.
pub struct RegionMapFiles;

impl RegionMapFiles {
    /// `world/<region>/map_<region>-<slugcat>.<ext>` if it exists, else
    /// `map_<region>.<ext>`; `None` if neither does.
    #[must_use]
    pub fn find(
        assets: &StreamingAssets,
        region: &str,
        slugcat: &str,
        ext: &str,
    ) -> Option<PathBuf> {
        [
            format!("world/{region}/map_{region}-{slugcat}.{ext}"),
            format!("world/{region}/map_{region}.{ext}"),
        ]
        .iter()
        .map(|p| assets.resolve_file_path(p))
        .find(|p| p.is_file())
    }

    /// The `Connection` lines of the region's map text file (empty without one).
    #[must_use]
    pub fn connections(
        assets: &StreamingAssets,
        region: &str,
        slugcat: &str,
    ) -> Vec<MapConnectionLine> {
        let Some(path) = Self::find(assets, region, slugcat, "txt") else {
            return Vec::new();
        };
        fs::read_to_string(path)
            .map(|t| t.lines().filter_map(MapConnectionLine::parse).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_connection_line() {
        let c = MapConnectionLine::parse("Connection: GATE_LF_SU,SU_A45,45,9,3,18,0,2").unwrap();
        assert_eq!(c.room_a, "GATE_LF_SU");
        assert_eq!(c.room_b, "SU_A45");
        assert_eq!(
            (c.pos_a, c.pos_b, c.dir_a, c.dir_b),
            ((45, 9), (3, 18), 0, 2)
        );
        assert!(MapConnectionLine::parse("SU_A33: 187.4><108.3><141.6><313.8><1><").is_none());
        assert!(MapConnectionLine::parse("Connection: A,B,1,2,3,4,5").is_none());
    }
}
