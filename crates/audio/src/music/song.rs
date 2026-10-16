//! `MusicPiece`, its `SubTrack`s, `Song` and the song subclasses the base-game menus
//! use (`MenuOrSlideShowSong`, `IntroRollMusic`). Behaviour: `rainworld-re/notes/music.md`
//! section 4.

use super::bank::MusicBundle;
use super::player::{MusicContext, MusicSettings};
use super::sources::{MusicSources, TrackId};

/// One clip of a piece, on its own `AudioSource`.
#[derive(Debug, Clone)]
pub struct SubTrack {
    pub index: usize,
    /// Clip name in the piece's bundle.
    pub track_name: String,
    /// Track volume, multiplied with the piece's.
    pub volume: f32,
    pub ready_to_play: bool,
    id: TrackId,
    request_play: bool,
    /// Whether the clip was asked for (`source.clip != null`).
    load_requested: bool,
    /// A `StartPlaying` asked for between updates (`IntroRollMusic.StartMusic`), run at
    /// the start of the next update.
    start_pending: bool,
}

impl SubTrack {
    pub(crate) fn new(index: usize, track_name: &str, id: TrackId) -> Self {
        Self {
            index,
            track_name: track_name.to_owned(),
            volume: 1.0,
            ready_to_play: false,
            id,
            request_play: false,
            load_requested: false,
            start_pending: false,
        }
    }

    #[must_use]
    pub fn id(&self) -> TrackId {
        self.id
    }
}

/// `MusicPiece`: the sub-tracks and what they share.
#[derive(Debug, Clone)]
pub struct MusicPiece {
    /// `IsProcedural`: which bundle the clips come from.
    pub bundle: MusicBundle,
    /// `Loop`: every source loops.
    pub looped: bool,
    /// The piece's factor in each source's volume.
    pub volume: f32,
    pub started_playing: bool,
    pub play_when_ready: bool,
    pub sub_tracks: Vec<SubTrack>,
    /// `IntroRollMusic.StartPlaying` starts only the first sub-track.
    starts_first_only: bool,
}

/// What a piece's update reads from the music player.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PieceInput<'a> {
    pub main_song_mix: f32,
    pub ripple_dimension: f32,
    pub settings: &'a MusicSettings,
    pub asset_bundles_loaded: bool,
}

impl MusicPiece {
    pub(crate) fn new(bundle: MusicBundle) -> Self {
        Self {
            bundle,
            looped: false,
            volume: 0.3,
            started_playing: false,
            play_when_ready: false,
            sub_tracks: Vec::new(),
            starts_first_only: false,
        }
    }

    /// Runs `StartPlaying` calls made since the last update.
    pub(crate) fn run_pending(&mut self, sources: &mut dyn MusicSources) {
        for i in 0..self.sub_tracks.len() {
            if std::mem::take(&mut self.sub_tracks[i].start_pending) {
                self.start_sub_track(i, sources);
            }
        }
    }

    /// `SubTrack.StartPlaying`: plays once this and every other track are ready, else
    /// waits for its update to retry.
    fn start_sub_track(&mut self, i: usize, sources: &mut dyn MusicSources) {
        let all_ready = self
            .sub_tracks
            .iter()
            .enumerate()
            .all(|(j, t)| j == i || t.ready_to_play);
        let track = &mut self.sub_tracks[i];
        if track.ready_to_play && all_ready {
            sources.play(track.id, self.looped);
        } else {
            track.request_play = true;
        }
    }

    /// `SubTrack.Update`: retries a requested start, sets the source volume, then loads
    /// the clip (one update) and marks it ready once loaded (a later update).
    fn update_sub_track(&mut self, i: usize, input: &PieceInput, sources: &mut dyn MusicSources) {
        let track = &mut self.sub_tracks[i];
        if track.request_play && track.ready_to_play {
            track.request_play = false;
            self.start_sub_track(i, sources);
        }
        let track = &mut self.sub_tracks[i];
        if self.started_playing {
            let s = input.settings;
            sources.set_volume(
                track.id,
                MusicSettings::source_volume(
                    track.volume * self.volume * s.music_volume,
                    s.volume_exponent,
                ),
            );
        }
        if track.ready_to_play || !input.asset_bundles_loaded {
            return;
        }
        if !track.load_requested {
            track.load_requested = true;
            sources.load(track.id, self.bundle, &track.track_name);
        } else if !sources.is_playing(track.id) && sources.loaded(track.id) {
            track.ready_to_play = true;
        }
    }

    /// `MusicPiece.StartPlaying` (`IntroRollMusic`'s starts only the rain).
    fn start_playing(&mut self, sources: &mut dyn MusicSources) {
        self.started_playing = true;
        let count = if self.starts_first_only {
            1
        } else {
            self.sub_tracks.len()
        };
        for i in 0..count {
            self.start_sub_track(i, sources);
        }
    }

    /// `MusicPiece.Update`: the sub-tracks, then the start once all are ready.
    pub(crate) fn update(&mut self, input: &PieceInput, sources: &mut dyn MusicSources) {
        for i in 0..self.sub_tracks.len() {
            self.update_sub_track(i, input, sources);
        }
        if !self.started_playing
            && self.play_when_ready
            && self.sub_tracks.iter().all(|t| t.ready_to_play)
        {
            self.start_playing(sources);
        }
    }

    /// `StopAndDestroy`.
    pub(crate) fn stop_and_destroy(&mut self, sources: &mut dyn MusicSources) {
        for track in &mut self.sub_tracks {
            track.request_play = false;
            sources.destroy(track.id);
        }
    }
}

/// `IntroRollMusic`: the rain clip `TitleRollRain` as track 0, `RW_8 - Sundown` as
/// track 1, started separately by [`Song::start_music`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntroRoll {
    pub rain_vol: f32,
    pub fade_out_rain: bool,
    pub music_track_started: bool,
}

/// Which `Song` subclass a song is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SongKind {
    /// `Song` itself (room triggers, arena songs).
    Plain,
    MenuOrSlideShow,
    IntroRoll(IntroRoll),
}

/// `Music.Song` with its `MusicPiece`.
#[derive(Debug, Clone)]
pub struct Song {
    pub name: String,
    pub context: MusicContext,
    pub kind: SongKind,
    pub priority: f32,
    pub fade_out_at_threat: f32,
    pub drone_volume: f32,
    /// Ticks; `mainSongMix` rises by `1 / (1 + fade_in_time)` per update.
    pub fade_in_time: f32,
    pub base_volume: f32,
    pub room_transitions: i32,
    /// Ticks, or negative when not fading out.
    pub fade_out_time: f32,
    pub stop_at_gate: bool,
    pub stop_at_death: bool,
    pub piece: MusicPiece,
}

impl Song {
    /// `new Song(musicPlayer, name, context)`: one sub-track playing the clip `name`.
    pub(crate) fn new(name: &str, context: MusicContext, id: TrackId) -> Self {
        let mut piece = MusicPiece::new(MusicBundle::Songs);
        piece.sub_tracks.push(SubTrack::new(0, name, id));
        Self {
            name: name.to_owned(),
            context,
            kind: SongKind::Plain,
            priority: 0.5,
            fade_out_at_threat: 1.0,
            drone_volume: 0.0,
            fade_in_time: 0.0,
            base_volume: 0.3,
            room_transitions: -1,
            fade_out_time: -1.0,
            stop_at_gate: false,
            stop_at_death: false,
            piece,
        }
    }

    /// `MenuOrSlideShowSong`.
    pub(crate) fn menu_or_slide_show(
        name: &str,
        priority: f32,
        fade_in_time: f32,
        id: TrackId,
    ) -> Self {
        Self {
            kind: SongKind::MenuOrSlideShow,
            priority,
            fade_in_time,
            ..Self::new(name, MusicContext::Menu, id)
        }
    }

    /// `IntroRollMusic`.
    pub(crate) fn intro_roll(rain: TrackId, music: TrackId) -> Self {
        let mut song = Self::new(Self::TITLE_ROLL_RAIN, MusicContext::Menu, rain);
        song.piece
            .sub_tracks
            .push(SubTrack::new(1, Self::TITLE_ROLL_MUSIC, music));
        for track in &mut song.piece.sub_tracks {
            track.volume = 1.0;
        }
        song.piece.starts_first_only = true;
        song.kind = SongKind::IntroRoll(IntroRoll {
            rain_vol: 1.0,
            fade_out_rain: false,
            music_track_started: false,
        });
        song.priority = 0.0;
        song.stop_at_gate = true;
        song.stop_at_death = true;
        song.fade_in_time = 10.0;
        song
    }

    pub const TITLE_ROLL_RAIN: &'static str = "TitleRollRain";
    pub const TITLE_ROLL_MUSIC: &'static str = "RW_8 - Sundown";

    #[must_use]
    pub fn is_intro_roll(&self) -> bool {
        matches!(self.kind, SongKind::IntroRoll(_))
    }

    #[must_use]
    pub fn fading_out(&self) -> bool {
        self.fade_out_time >= 0.0
    }

    /// `FadeOut(speed)`: a fade already under way keeps the shorter time.
    pub fn fade_out(&mut self, speed: f32) {
        self.fade_out_time = if self.fade_out_time < 0.0 {
            speed
        } else {
            self.fade_out_time.min(speed)
        };
    }

    /// `PlayerToNewRoom`.
    pub fn player_to_new_room(&mut self) {
        if self.room_transitions >= 0 {
            self.room_transitions -= 1;
            if self.room_transitions <= 0 {
                self.fade_out(400.0);
            }
        }
    }

    /// `IntroRollMusic.StartMusic`: starts `RW_8 - Sundown` once. Takes effect at the
    /// start of the next [`super::MusicPlayer::update`]; no-op for other songs.
    pub fn start_music(&mut self) {
        if let SongKind::IntroRoll(intro) = &mut self.kind
            && !intro.music_track_started
        {
            intro.music_track_started = true;
            self.piece.sub_tracks[1].start_pending = true;
        }
    }

    /// `IntroRollMusic.fadeOutRain = true`: the rain track fades by 0.02 per update.
    pub fn fade_out_rain(&mut self) {
        if let SongKind::IntroRoll(intro) = &mut self.kind {
            intro.fade_out_rain = true;
        }
    }

    /// One update: `MusicPiece.Update`, `Song.Update` and the subclass's.
    pub(crate) fn update(&mut self, input: &PieceInput, sources: &mut dyn MusicSources) {
        self.piece.update(input, sources);
        let piece = &self.piece;
        // A finished song (no track playing, all loaded) fades out.
        if piece.started_playing
            && piece
                .sub_tracks
                .iter()
                .all(|t| !sources.is_playing(t.id) && t.ready_to_play)
        {
            self.fade_out(10.0);
        }
        self.piece.volume = input.main_song_mix * self.base_volume;
        if input.ripple_dimension >= 0.02 {
            self.piece.volume *= 1.0 + (0.1 - 1.0) * input.ripple_dimension.clamp(0.0, 1.0);
        }
        if let SongKind::IntroRoll(intro) = &mut self.kind {
            if intro.fade_out_rain {
                intro.rain_vol = (intro.rain_vol - 0.02).max(0.0);
            }
            self.piece.sub_tracks[0].volume = intro.rain_vol;
        }
    }

    /// `StopAndDestroy`.
    pub(crate) fn stop_and_destroy(&mut self, sources: &mut dyn MusicSources) {
        self.piece.stop_and_destroy(sources);
    }
}
