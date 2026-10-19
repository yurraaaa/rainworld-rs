//! What the menu screens and the game's endings ask of the music player. The sleep,
//! death and starve screens ask nothing (their `KarmaLadderScreen` plays only menu
//! sound loops); the starve's `DeathEvent` comes from `GoToStarveScreen`.
//!
//! Mirrors: Music/MusicPlayer.cs (GameRequestsSong), Menu/DreamScreen.cs, Menu/KarmaLadderScreen.cs

use super::player::MusicPlayer;
use super::song::SongKind;

impl MusicPlayer {
    /// The `DreamScreen` constructor: `FadeOutAllSongs(30)`.
    pub fn dream_screen(&mut self) {
        self.fade_out_all_songs(30.0);
    }

    /// The `CustomEndGameScreen` (passage) constructor.
    pub fn custom_end_game_screen(&mut self) {
        self.menu_requests_song("Passages", 1.2, 10.0);
    }

    /// `RainWorldGame.GoToRedsGameOver`'s music: `FadeOutAllSongs(20)`.
    pub fn reds_game_over(&mut self) {
        self.fade_out_all_songs(20.0);
    }
}

/// `EndCredits`' song: `manager.desiredCreditsSong`, asked for 400 updates in unless the
/// intro roll's music plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditsMusic {
    /// `""` for none.
    pub desired_credits_song: String,
    pub music_timer: i32,
}

impl CreditsMusic {
    /// `ProcessManager.desiredCreditsSong`'s value outside More Slugcats and Watcher.
    pub const DEFAULT_SONG: &'static str = "RW_8 - Sundown";

    /// The credits after an ending (`game == null`) play `desired`; the credits a
    /// running game opens play none.
    #[must_use]
    pub fn new(desired: Option<&str>) -> Self {
        Self {
            desired_credits_song: desired.unwrap_or("").to_owned(),
            music_timer: 400,
        }
    }

    /// One `EndCredits.Update`.
    pub fn update(&mut self, music: Option<&mut MusicPlayer>) {
        let Some(music) = music else {
            return;
        };
        if self.desired_credits_song.is_empty()
            || self.music_timer <= 0
            || music
                .song
                .as_ref()
                .is_some_and(|s| matches!(s.kind, SongKind::IntroRoll(_)))
        {
            return;
        }
        self.music_timer -= 1;
        if self.music_timer == 0 {
            music.menu_requests_song(&self.desired_credits_song, 1.4, 0.0);
        }
    }
}
