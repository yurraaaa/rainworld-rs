//! The game's music (`Music` namespace): the `MusicPlayer` side process, its songs and
//! their sources. Behaviour: `rainworld-re/notes/music.md`; what is not ported:
//! docs/DIVERGENCES.md D12.
//!
//! The player is pure tick logic over [`MusicSources`]: [`MusicOutput`] plays it
//! through kira, [`TimedSources`] runs it silently and deterministically.

mod bank;
mod player;
mod song;
mod sources;
mod stream;

pub use bank::{ClipLength, SongBank};
pub use player::{
    MusicContext, MusicPlayer, MusicProcess, MusicSettings, StopMusicEvent, StopMusicType,
    ThreatTracker,
};
pub use song::{IntroRoll, Song, SongKind, SubTrack};
pub use sources::{MusicClock, MusicSources, SourcePlayback, TimedSources, TimedTrack, TrackId};
pub use stream::{KiraMusic, MusicOutput};
