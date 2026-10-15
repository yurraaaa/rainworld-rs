//! The game's save data, in its own format: the slot files' container
//! ([`user_data`]), `PlayerProgression` and the parts of `SaveState` the port uses,
//! where the port keeps its saves, and the importer of the game's slots.
//! Format notes: `rainworld-re/notes/saves.md`.

pub mod checksum;
pub mod death_persistent;
pub mod dirs;
pub mod import;
pub mod misc;
pub mod progression;
pub mod save_state;
pub mod user_data;
pub mod win_state;

pub use checksum::ChecksumSalt;
pub use dirs::{Platform, SaveSlot};
pub use import::{ImportReport, import_slot};
pub use progression::{CampaignStart, PlayerProgression, SaveError};
pub use save_state::SaveState;
