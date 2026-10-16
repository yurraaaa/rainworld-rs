//! The game's music (`Music` namespace): the `MusicPlayer` side process, its songs,
//! the region's procedural threat layers and their sources. Behaviour:
//! `rainworld-re/notes/music.md`; what is not ported: docs/DIVERGENCES.md D12.
//!
//! The player is pure tick logic over [`MusicSources`]: [`MusicOutput`] plays it
//! through kira, [`TimedSources`] runs it silently and deterministically.

mod bank;
mod player;
mod procedural;
mod screens;
mod song;
mod sources;
mod stream;
mod threat;

pub use bank::{ClipLength, MusicBundle, SongBank};
pub use player::{MusicContext, MusicPlayer, MusicProcess, MusicSettings, StorySongs};
pub use procedural::{
    DayNight, HudSubregion, InstalledProcedural, ProceduralFiles, ProceduralInstruction,
    ProceduralMusic, ProceduralTrack, ProceduralView,
};
pub use rw_assets::{MusicEvent, StopMusicEvent, StopMusicType};
pub use screens::CreditsMusic;
pub use song::{IntroRoll, MusicPiece, Song, SongKind, SubTrack};
pub use sources::{
    MusicClock, MusicSources, SourcePlayback, TimedSources, TimedTrack, TrackId, TrackIds,
};
pub use stream::{KiraMusic, MusicOutput};
pub use threat::{CreatureThreat, GameMusicView, ThreatDetermination, ThreatTracker};
