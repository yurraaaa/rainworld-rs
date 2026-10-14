//! `Music.MusicPlayer`, the side process holding the current song and the next one,
//! and the mix between them. Behaviour: `rainworld-re/notes/music.md` section 4.1.

use super::song::{PieceInput, Song};
use super::sources::{MusicSources, TrackId};

/// `MusicPlayer.MusicContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicContext {
    Menu,
    StoryMode,
    Arena,
}

/// The current main process as `UpdateMusicContext` tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicProcess {
    /// `ProcessID.Game` in a story session.
    StoryGame,
    /// `ProcessID.Game` in an arena session.
    ArenaGame,
    SlugcatSelect,
    RegionsOverviewScreen,
    MultiplayerMenu,
    MainMenu,
    /// Any other process (`IntroRoll`, `Initialization`, `OptionsMenu`, ...).
    Other,
}

impl MusicProcess {
    fn is_game(self) -> bool {
        matches!(self, Self::StoryGame | Self::ArenaGame)
    }
}

/// What the music player reads from the options, setup and sound loader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicSettings {
    /// `Options.musicVolume` (default 0.8).
    pub music_volume: f32,
    /// `RainWorldGame.SetupValues.playMusic` (`setup.txt` "play music", default on).
    pub play_music: bool,
    /// `SoundLoader.volumeExponent` (`sounds.txt` line 1, 1.8).
    pub volume_exponent: f32,
}

impl MusicSettings {
    pub const DEFAULT_MUSIC_VOLUME: f32 = 0.8;

    /// The defaults with the sound table's exponent.
    #[must_use]
    pub fn new(volume_exponent: f32) -> Self {
        Self {
            music_volume: Self::DEFAULT_MUSIC_VOLUME,
            play_music: true,
            volume_exponent,
        }
    }

    /// `SubTrack.Update`: `clamp01(pow(level, exponent))`, with `Mathf.Pow`'s double
    /// precision.
    #[must_use]
    pub fn source_volume(level: f32, exponent: f32) -> f32 {
        (f64::from(level).powf(f64::from(exponent)) as f32).clamp(0.0, 1.0)
    }
}

/// `StopMusicEvent.Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopMusicType {
    AllSongs,
    SpecificSong,
    AllButSpecific,
}

/// `StopMusicEvent` (room trigger `StopMusicEvent`).
#[derive(Debug, Clone, PartialEq)]
pub struct StopMusicEvent {
    pub stop_type: StopMusicType,
    pub song_name: String,
    pub prio: f32,
    pub fade_out_time: f32,
}

/// The part of `PlayerThreatTracker` that the player reads outside the game; the
/// in-game tracker (threat, ghosts, room switches) is not ported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatTracker {
    pub recommended_drone_volume: f32,
    pub current_threat: f32,
}

impl ThreatTracker {
    fn new() -> Self {
        Self {
            recommended_drone_volume: 0.3,
            current_threat: 0.0,
        }
    }

    /// `PlayerThreatTracker.Update` outside `ProcessID.Game`; in the game it keeps its
    /// values (not ported).
    fn update(&mut self, process: Option<MusicProcess>) {
        if !process.is_some_and(MusicProcess::is_game) {
            self.recommended_drone_volume = 0.0;
            self.current_threat = 0.0;
        }
    }
}

/// `Music.MusicPlayer`. Its owner calls [`Self::update`] once per 40 Hz tick, after
/// the main process's update (side processes run after it in `ProcessManager.Update`).
#[derive(Debug, Clone)]
pub struct MusicPlayer {
    pub song: Option<Song>,
    pub next_song: Option<Song>,
    pub main_song_mix: f32,
    pub drone_goal_mix: f32,
    pub ripple_dimension: f32,
    pub music_context: MusicContext,
    pub threat_tracker: Option<ThreatTracker>,
    pub has_played_a_song_this_cycle: bool,
    pub settings: MusicSettings,
    asset_bundles_loaded: bool,
    requested_asset_bundles_load: bool,
    /// The main process, for the threat tracker.
    process: Option<MusicProcess>,
    next_track: u64,
}

impl MusicPlayer {
    /// `ProcessManager.Update` creates the player once the options are read, if
    /// `setup.playMusic` and `musicVolume > 0`; `None` otherwise. `process` is the
    /// current main process (`None` before the first).
    #[must_use]
    pub fn new(settings: MusicSettings, process: Option<MusicProcess>) -> Option<Self> {
        if !settings.play_music || settings.music_volume <= 0.0 {
            return None;
        }
        let mut player = Self {
            song: None,
            next_song: None,
            main_song_mix: 0.0,
            drone_goal_mix: 0.0,
            ripple_dimension: 0.0,
            music_context: MusicContext::Menu,
            threat_tracker: None,
            has_played_a_song_this_cycle: false,
            settings,
            asset_bundles_loaded: false,
            requested_asset_bundles_load: false,
            process,
            next_track: 0,
        };
        player.update_music_context(process);
        Some(player)
    }

    /// `assetBundlesLoaded`: `IntroRoll.GoToMenu` waits for it.
    #[must_use]
    pub fn asset_bundles_loaded(&self) -> bool {
        self.asset_bundles_loaded
    }

    fn track_id(&mut self) -> TrackId {
        self.next_track += 1;
        TrackId(self.next_track)
    }

    /// `UpdateMusicContext`, called on creation and after every main process switch.
    pub fn update_music_context(&mut self, process: Option<MusicProcess>) {
        self.process = process;
        match process {
            None => self.music_context = MusicContext::Menu,
            Some(p @ (MusicProcess::StoryGame | MusicProcess::ArenaGame)) => {
                self.music_context = if p == MusicProcess::StoryGame {
                    MusicContext::StoryMode
                } else {
                    MusicContext::Arena
                };
                if let Some(song) = self
                    .song
                    .as_mut()
                    .filter(|s| s.context == MusicContext::Menu)
                {
                    song.fade_out(2.0);
                }
            }
            Some(MusicProcess::SlugcatSelect | MusicProcess::RegionsOverviewScreen) => {
                self.music_context = MusicContext::StoryMode;
            }
            Some(MusicProcess::MultiplayerMenu) => self.music_context = MusicContext::Arena,
            Some(MusicProcess::MainMenu) => self.music_context = MusicContext::Menu,
            Some(MusicProcess::Other) => {}
        }
        if !process.is_some_and(MusicProcess::is_game) {
            self.ripple_dimension = 0.0;
        }
        if self.music_context == MusicContext::StoryMode {
            self.threat_tracker.get_or_insert_with(ThreatTracker::new);
        } else {
            self.threat_tracker = None;
        }
        if let Some(tracker) = &mut self.threat_tracker
            && !process.is_some_and(MusicProcess::is_game)
        {
            tracker.current_threat = 0.0;
        }
        if self.music_context != MusicContext::Menu {
            let context = self.music_context;
            if let Some(song) = self
                .song
                .as_mut()
                .filter(|s| s.context != MusicContext::Menu && s.context != context)
            {
                song.fade_out(2.0);
            }
            if self
                .next_song
                .as_ref()
                .is_some_and(|s| s.context != MusicContext::Menu && s.context != context)
            {
                self.next_song = None;
            }
        }
    }

    /// Makes `song` the current song, or the next one when a song plays.
    fn start_or_queue(&mut self, mut song: Song) {
        if self.song.is_none() {
            song.play_when_ready = true;
            self.song = Some(song);
        } else {
            song.play_when_ready = false;
            self.next_song = Some(song);
        }
    }

    /// `RequestIntroRollMusic`.
    pub fn request_intro_roll_music(&mut self) {
        if self.song.as_ref().is_some_and(Song::is_intro_roll)
            || self.next_song.as_ref().is_some_and(Song::is_intro_roll)
            || !self.settings.play_music
        {
            return;
        }
        let (rain, music) = (self.track_id(), self.track_id());
        self.start_or_queue(Song::intro_roll(rain, music));
    }

    /// `MenuRequestsSong`: refused while a song (or a waiting one) of at least this
    /// priority or of the same name plays.
    pub fn menu_requests_song(&mut self, name: &str, priority: f32, fade_in_time: f32) {
        if self
            .song
            .as_ref()
            .is_some_and(|s| s.priority >= priority || s.name == name)
        {
            return;
        }
        let id = self.track_id();
        let song = Song::menu_or_slide_show(name, priority, fade_in_time, id);
        // With a song playing, a waiting song of at least this priority or name stays.
        if self.song.is_none()
            || !self
                .next_song
                .as_ref()
                .is_some_and(|s| s.priority >= priority || s.name == name)
        {
            self.start_or_queue(song);
        }
    }

    /// `GameRequestsSongStop`.
    pub fn game_requests_song_stop(&mut self, event: &StopMusicEvent) {
        for song in [&mut self.song, &mut self.next_song].into_iter().flatten() {
            let named = song.name == event.song_name;
            let applies = match event.stop_type {
                StopMusicType::AllSongs => true,
                StopMusicType::SpecificSong => named,
                StopMusicType::AllButSpecific => !named,
            };
            if event.prio >= song.priority && applies {
                song.fade_out(event.fade_out_time);
            }
        }
    }

    /// `FadeOutAllSongs`.
    pub fn fade_out_all_songs(&mut self, fade_out_time: f32) {
        if let Some(song) = &mut self.song {
            song.fade_out(fade_out_time);
        }
        self.next_song = None;
    }

    /// `RainRequestStopSong` (base game: no siren song to spare).
    pub fn rain_request_stop_song(&mut self) {
        if let Some(song) = &mut self.song {
            song.fade_out(400.0);
        }
        self.next_song = None;
    }

    /// `DeathEvent` (the threat tracker's decline counter is not ported).
    pub fn death_event(&mut self) {
        if let Some(song) = self.song.as_mut().filter(|s| s.stop_at_death) {
            song.fade_out(60.0);
        }
        if self.next_song.as_ref().is_some_and(|s| s.stop_at_death) {
            self.next_song = None;
        }
    }

    /// `GateEvent`.
    pub fn gate_event(&mut self) {
        if let Some(song) = self.song.as_mut().filter(|s| s.stop_at_gate) {
            song.fade_out(120.0);
        }
        if self.next_song.as_ref().is_some_and(|s| s.stop_at_gate) {
            self.next_song = None;
        }
    }

    /// `NewCycleEvent`.
    pub fn new_cycle_event(&mut self) {
        self.has_played_a_song_this_cycle = false;
    }

    /// `MusicPlayer.Update`: one 40 Hz tick.
    pub fn update(&mut self, sources: &mut dyn MusicSources) {
        if let Some(song) = &mut self.song {
            song.run_pending(sources);
        }
        if !self.asset_bundles_loaded {
            if !self.requested_asset_bundles_load {
                self.requested_asset_bundles_load = true;
            } else if sources.bundles_loaded() {
                self.asset_bundles_loaded = true;
            }
        }
        let input = PieceInput {
            main_song_mix: self.main_song_mix,
            ripple_dimension: self.ripple_dimension,
            settings: &self.settings,
            asset_bundles_loaded: self.asset_bundles_loaded,
        };
        if let Some(song) = &mut self.song {
            song.update(&input, sources);
            if let Some(tracker) = &self.threat_tracker {
                if tracker.current_threat > song.fade_out_at_threat {
                    song.fade_out(200.0);
                }
                // Mathf.Lerp(1 - mainSongMix, 1, droneVolume)
                let from = 1.0 - self.main_song_mix;
                self.drone_goal_mix = tracker.recommended_drone_volume
                    * (from + (1.0 - from) * song.drone_volume.clamp(0.0, 1.0));
            } else {
                self.drone_goal_mix = 0.0;
            }
            if song.fading_out() {
                self.main_song_mix =
                    (self.main_song_mix - 1.0 / (1.0 + song.fade_out_time)).max(0.0);
                if self.main_song_mix <= 0.0 {
                    song.stop_and_destroy(sources);
                    self.song = None;
                }
            } else if self.next_song.as_ref().is_none_or(Song::fading_out) {
                self.main_song_mix =
                    (self.main_song_mix + 1.0 / (1.0 + song.fade_in_time)).min(1.0);
            }
        } else {
            self.main_song_mix = (self.main_song_mix - 0.025).max(0.0);
            self.drone_goal_mix = self
                .threat_tracker
                .map_or(0.0, |t| t.recommended_drone_volume);
        }
        if let Some(mut next) = self.next_song.take() {
            match &mut self.song {
                None => {
                    next.play_when_ready = true;
                    self.main_song_mix = if next.fade_in_time == 0.0 { 1.0 } else { 0.0 };
                    self.song = Some(next);
                }
                Some(song) => {
                    self.main_song_mix = (self.main_song_mix - 0.0125).max(0.0);
                    if self.main_song_mix == 0.0 {
                        song.stop_and_destroy(sources);
                        self.song = None;
                    }
                    self.next_song = Some(next);
                }
            }
        }
        if let Some(tracker) = &mut self.threat_tracker {
            tracker.update(self.process);
        }
    }
}
