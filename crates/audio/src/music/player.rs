//! `Music.MusicPlayer`, the side process holding the current song and the next one,
//! and the mix between them. Behaviour: `rainworld-re/notes/music.md` section 4.1.
//!
//! Mirrors: Music/MusicPlayer.cs

use std::sync::Arc;

use rw_assets::{MusicEvent, StopMusicEvent, StopMusicType};
use rw_random::UnityRandom;
use rw_save::SongPlayRecord;

use super::procedural::{ProceduralFiles, ProceduralInput, ProceduralMusic, ProceduralView};
use super::song::{PieceInput, Song};
use super::sources::{MusicSources, TrackId, TrackIds};
use super::threat::{GameMusicView, ThreatTracker};

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
    pub(crate) fn is_game(self) -> bool {
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

/// A story game's save state as `GameRequestsSong` reads and writes it.
#[derive(Debug)]
pub struct StorySongs<'a> {
    /// `deathPersistentSaveData.songsPlayRecords`.
    pub records: &'a mut Vec<SongPlayRecord>,
    /// `saveState.cycleNumber`.
    pub cycle_number: i32,
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
    /// `nextProcedural`: the bank asked for while a piece plays (`""` for none).
    pub next_procedural: Option<String>,
    pub current_procedural_region: Option<String>,
    pub procedural_music: Option<ProceduralMusic>,
    asset_bundles_loaded: bool,
    requested_asset_bundles_load: bool,
    /// The main process, for the threat tracker.
    pub(crate) process: Option<MusicProcess>,
    /// The game as the threat tracker reads it, set by the owner before each update.
    pub(crate) game_view: Option<GameMusicView>,
    /// The game as the procedural piece reads it; `None` outside a game.
    procedural_view: Option<ProceduralView>,
    /// The procedural banks; without them every bank is empty.
    procedural_files: Option<Arc<dyn ProceduralFiles>>,
    track_ids: TrackIds,
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
            next_procedural: None,
            current_procedural_region: None,
            procedural_music: None,
            asset_bundles_loaded: false,
            requested_asset_bundles_load: false,
            process,
            game_view: None,
            procedural_view: None,
            procedural_files: None,
            track_ids: TrackIds::default(),
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
        self.track_ids.next_id()
    }

    /// Where `ProceduralMusic` reads its banks.
    #[must_use]
    pub fn with_procedural_files(mut self, files: Arc<dyn ProceduralFiles>) -> Self {
        self.procedural_files = Some(files);
        self
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
            tracker.threat_determine.current_threat = 0.0;
            tracker.threat_determine.current_music_agnostic_threat = 0.0;
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
            // The arena's `MultiplayerDJ` is not ported.
            if context != MusicContext::StoryMode {
                self.next_procedural = Some(String::new());
            }
        }
    }

    /// What the threat tracker reads of the game this tick: `None` without a realized
    /// player in a room.
    pub fn set_game_view(&mut self, view: Option<GameMusicView>) {
        self.game_view = view;
    }

    /// What the procedural piece reads of the game this tick: `None` outside a game.
    pub fn set_procedural_view(&mut self, view: Option<ProceduralView>) {
        self.procedural_view = view;
    }

    /// `NewRegion`: `region`'s threat layers are built at once (`Reset`'s draws) when
    /// no piece plays and the bundles are loaded, else once the playing piece has
    /// faded out.
    pub fn new_region(&mut self, region: &str, rng: &mut UnityRandom) {
        log::info!("NEW MUSIC REGION: {region}");
        self.next_procedural = None;
        if self.procedural_music.is_none() && self.asset_bundles_loaded {
            self.build_procedural(region, rng);
        } else {
            self.next_procedural = Some(region.to_owned());
        }
        self.current_procedural_region = Some(region.to_owned());
    }

    /// `new ProceduralMusic(this, bank)`.
    fn build_procedural(&mut self, bank: &str, rng: &mut UnityRandom) {
        self.procedural_music = Some(ProceduralMusic::new(
            bank,
            self.procedural_files.as_deref(),
            self.procedural_view.as_ref(),
            rng,
            &mut self.track_ids,
        ));
    }

    /// `ResumeProcedural`: the last region's bank again (after an echo's song).
    pub fn resume_procedural(&mut self) {
        self.next_procedural = self.current_procedural_region.clone();
    }

    /// The procedural piece's part of [`Self::update`]: while another bank (or none) is
    /// asked for, the piece fades out and is dropped at volume 0; with no piece, the
    /// asked-for bank is built.
    fn update_procedural(&mut self, sources: &mut dyn MusicSources, rng: &mut UnityRandom) {
        if let Some(mut piece) = self.procedural_music.take() {
            if self.next_procedural.is_some() {
                self.drone_goal_mix = 0.0;
            }
            let input = ProceduralInput {
                piece: PieceInput {
                    main_song_mix: self.main_song_mix,
                    ripple_dimension: self.ripple_dimension,
                    settings: &self.settings,
                    asset_bundles_loaded: self.asset_bundles_loaded,
                },
                threat: self.threat_tracker.as_ref().map(|t| t.current_threat),
                drone_goal_mix: self.drone_goal_mix,
                view: self.procedural_view.as_ref(),
            };
            piece.update(&input, sources, rng, &mut self.track_ids);
            if self.next_procedural.is_some() && piece.piece.volume == 0.0 {
                piece.stop_and_destroy(sources);
            } else {
                self.procedural_music = Some(piece);
            }
        } else if self.asset_bundles_loaded
            && let Some(bank) = self.next_procedural.clone()
            && !bank.is_empty()
        {
            self.build_procedural(&bank, rng);
            self.next_procedural = None;
        }
    }

    /// `GameRequestsSong(musicEvent)`: a room trigger's song, refused while a song of at
    /// least its priority (or the same song) plays, above its threat, in ghost or laugh
    /// mode, within its rest since it last played in this save (`story`; a negative
    /// rest is never again), or after another song this cycle when it plays once per
    /// cycle. The play record is written unless a waiting song outranks it.
    pub fn game_requests_song(&mut self, event: &MusicEvent, story: Option<StorySongs<'_>>) {
        log::info!("Game request song {}", event.song_name);
        let name = event.song_name.as_str();
        if !self.settings.play_music
            || self
                .song
                .as_ref()
                .is_some_and(|s| s.priority >= event.prio || s.name == name)
            || self.threat_tracker.as_ref().is_some_and(|t| {
                event.max_threat_level < t.current_threat
                    || t.ghost_mode > 0.0
                    || t.spinning_top_laugh_mode > 0.0
            })
        {
            return;
        }
        if let Some(s) = &story
            && s.records.iter().any(|r| {
                r.song_name == name
                    && (s.cycle_number - r.cycle_last_played < event.cycles_rest
                        || event.cycles_rest < 0)
            })
        {
            return;
        }
        if event.one_song_per_cycle && self.has_played_a_song_this_cycle {
            return;
        }
        self.has_played_a_song_this_cycle = true;
        log::info!("Play song {name}");
        let id = self.track_id();
        let mut song = Song::new(name, MusicContext::StoryMode, id);
        song.fade_out_at_threat = event.max_threat_level;
        song.piece.looped = event.looped;
        song.priority = event.prio;
        song.base_volume = event.volume;
        song.fade_in_time = event.fade_in_time;
        song.stop_at_death = event.stop_at_death;
        song.stop_at_gate = event.stop_at_gate;
        if event.rooms_range > -1
            && let Some(t) = &self.threat_tracker
        {
            song.room_transitions = event.rooms_range + t.room_switches;
        }
        if self.song.is_none() {
            song.piece.play_when_ready = true;
            self.song = Some(song);
        } else {
            if self
                .next_song
                .as_ref()
                .is_some_and(|n| n.priority >= event.prio || n.name == name)
            {
                return;
            }
            song.piece.play_when_ready = false;
            self.next_song = Some(song);
        }
        let Some(s) = story else {
            return;
        };
        match s.records.iter_mut().find(|r| r.song_name == name) {
            Some(r) => r.cycle_last_played = s.cycle_number,
            None => s.records.push(SongPlayRecord {
                song_name: name.to_owned(),
                cycle_last_played: s.cycle_number,
            }),
        }
    }

    /// Makes `song` the current song, or the next one when a song plays.
    fn start_or_queue(&mut self, mut song: Song) {
        if self.song.is_none() {
            song.piece.play_when_ready = true;
            self.song = Some(song);
        } else {
            song.piece.play_when_ready = false;
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
        self.next_procedural = Some(String::new());
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
            // An unregistered type stops like `AllSongs` (`StopSongIfItShouldStop`).
            let applies = match event.stop_type {
                Some(StopMusicType::SpecificSong) => named,
                Some(StopMusicType::AllButSpecific) => !named,
                Some(StopMusicType::AllSongs) | None => true,
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
        self.next_procedural = Some(String::new());
    }

    /// `RequestSSSong` (base game): Five Pebbles' song unless one is playing or waiting.
    pub fn request_ss_song(&mut self) {
        let playing = self.song.as_ref().is_some_and(Song::is_ss_song);
        let waiting = self.next_song.as_ref().is_some_and(Song::is_ss_song);
        if !playing && !waiting && self.settings.play_music {
            let id = self.track_id();
            self.start_or_queue(Song::ss_song(id));
        }
    }

    /// `SSMusicTrigger.Trigger` on a player with a song: asks for the song, or raises its
    /// volume to `amount`.
    pub fn ss_music_trigger(&mut self, amount: f32) {
        match self.song.as_mut().filter(|s| s.is_ss_song()) {
            Some(song) => song.ss_set_volume(amount),
            None => self.request_ss_song(),
        }
    }

    /// `RainRequestStopSong` (base game: no siren song to spare).
    pub fn rain_request_stop_song(&mut self) {
        if let Some(song) = &mut self.song {
            song.fade_out(400.0);
        }
        self.next_song = None;
    }

    /// `DeathEvent`.
    pub fn death_event(&mut self) {
        if let Some(song) = self.song.as_mut().filter(|s| s.stop_at_death) {
            song.fade_out(60.0);
        }
        if self.next_song.as_ref().is_some_and(|s| s.stop_at_death) {
            self.next_song = None;
        }
        if let Some(t) = &mut self.threat_tracker {
            let d = &mut t.threat_determine;
            d.threat_decline_counter = d.threat_decline_counter.max(400);
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

    /// `MusicPlayer.Update`: one 40 Hz tick; `rng` is the shared `UnityEngine.Random`.
    pub fn update(&mut self, sources: &mut dyn MusicSources, rng: &mut UnityRandom) {
        if let Some(song) = &mut self.song {
            song.piece.run_pending(sources);
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
                .as_ref()
                .map_or(0.0, |t| t.recommended_drone_volume);
        }
        if let Some(mut next) = self.next_song.take() {
            match &mut self.song {
                None => {
                    next.piece.play_when_ready = true;
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
        self.update_procedural(sources, rng);
        self.threat_tracker_update(rng);
    }
}
