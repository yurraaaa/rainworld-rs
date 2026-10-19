//! `DeathPersistentSaveData.SongPlayRecord`s (`SONGSPLAYRECORDS`): when each trigger
//! song last played, `name<dpD>cycle` entries separated by `<dpC>`.
//!
//! Mirrors: DeathPersistentSaveData.cs (SongPlayRecord)

use crate::death_persistent::ParseError;

/// `SongPlayRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongPlayRecord {
    pub song_name: String,
    pub cycle_last_played: i32,
}

impl SongPlayRecord {
    /// `FromString`'s `SONGSPLAYRECORDS` case: appends each non-empty entry.
    ///
    /// # Errors
    /// On an entry without a cycle or with a malformed one, where the game throws.
    pub fn parse_into(value: &str, records: &mut Vec<Self>) -> Result<(), ParseError> {
        for entry in value.split("<dpC>").filter(|e| !e.is_empty()) {
            let mut parts = entry.split("<dpD>");
            let song_name = parts.next().unwrap_or("").to_owned();
            let cycle_last_played = ParseError::int("SONGSPLAYRECORDS", parts.next())?;
            records.push(Self {
                song_name,
                cycle_last_played,
            });
        }
        Ok(())
    }

    /// The `SONGSPLAYRECORDS` value `SaveToString` writes (`None` without records).
    #[must_use]
    pub fn save_string(records: &[Self]) -> Option<String> {
        (!records.is_empty()).then(|| {
            records
                .iter()
                .map(|r| format!("{}<dpD>{}", r.song_name, r.cycle_last_played))
                .collect::<Vec<_>>()
                .join("<dpC>")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut records = Vec::new();
        SongPlayRecord::parse_into(
            "NA_01 - Proxima<dpD>3<dpC>RW_1 - Urban Jungle<dpD>0",
            &mut records,
        )
        .unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].song_name, "NA_01 - Proxima");
        assert_eq!(records[1].cycle_last_played, 0);
        assert_eq!(
            SongPlayRecord::save_string(&records).as_deref(),
            Some("NA_01 - Proxima<dpD>3<dpC>RW_1 - Urban Jungle<dpD>0")
        );
        assert_eq!(SongPlayRecord::save_string(&[]), None);
        assert!(SongPlayRecord::parse_into("X<dpD>y", &mut records).is_err());
        assert!(SongPlayRecord::parse_into("X", &mut records).is_err());
    }
}
