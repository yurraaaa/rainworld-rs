//! The game's save data, in its own format: the slot files' container
//! ([`user_data`]), `PlayerProgression` and `SaveState`, a survived cycle's save
//! ([`hibernation`]), the options ([`options`]), where the port keeps its saves, and the importer of the game's
//! slots. Format notes: `rainworld-re/notes/saves.md`.

pub mod checksum;
pub mod communities;
pub mod coordinate;
pub mod cs_float;
pub mod death_persistent;
pub mod dirs;
pub mod dreams;
pub mod hibernation;
pub mod import;
pub mod miner;
pub mod misc;
pub mod misc_world;
pub mod options;
pub mod progression;
pub mod save_state;
pub mod session;
pub mod songs;
pub mod user_data;
pub mod win_state;

pub use checksum::ChecksumSalt;
pub use communities::{CommunityId, CreatureCommunities};
pub use coordinate::SavedCoordinate;
pub use death_persistent::Tutorial;
pub use dirs::{Platform, SaveSlot};
pub use dreams::{DreamId, DreamsState};
pub use hibernation::WinOutcome;
pub use import::{ImportReport, import_slot};
pub use miner::{MinerResult, MinerTarget, SaveStateMiner};
pub use misc::{MiscProgression, WatchedScreen};
pub use misc_world::{MiscWorldSaveData, PlayerGuideState};
pub use options::{Language, Options};
pub use progression::{CampaignStart, PlayerProgression, SaveError};
pub use save_state::SaveState;
pub use session::{EatenKind, HibernationStats, HibernationWorld, KillRecord, PlayerSessionRecord};
pub use songs::SongPlayRecord;
