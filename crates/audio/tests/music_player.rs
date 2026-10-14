//! `MusicPlayer` and `Song` tick by tick against the decompiled `Music.MusicPlayer.Update`,
//! `Song.Update` and `MusicPiece.SubTrack.Update` (rainworld-re/notes/music.md 4.1), on
//! silent timed sources.

use rw_audio::music::{
    ClipLength, MusicPlayer, MusicProcess, MusicSettings, MusicSources, StopMusicEvent,
    StopMusicType, TimedSources,
};

const EXPONENT: f32 = 1.8;

struct Run {
    player: MusicPlayer,
    sources: TimedSources,
}

impl Run {
    fn new(clips: &[(&str, u32)]) -> Self {
        let mut sources = TimedSources::new(None);
        for &(name, ticks) in clips {
            // 44.1 kHz: 1102.5 frames per tick.
            let frames = ticks * 44_100 / 40;
            sources = sources.with_length(
                name,
                ClipLength {
                    channels: 2,
                    sample_rate: 44_100,
                    frames,
                },
            );
        }
        let player = MusicPlayer::new(MusicSettings::new(EXPONENT), Some(MusicProcess::Other))
            .expect("music on");
        Self { player, sources }
    }

    /// One tick: the player's update, then a tick of playback.
    fn tick(&mut self) {
        self.player.update(&mut self.sources);
        self.sources.advance();
    }

    fn source_volume(&self, sub_track: usize) -> Option<f32> {
        let song = self.player.song.as_ref()?;
        let id = song.sub_tracks[sub_track].id();
        self.sources.track(id).map(|t| t.volume)
    }

    fn playing(&self, sub_track: usize) -> bool {
        self.player
            .song
            .as_ref()
            .is_some_and(|s| self.sources.is_playing(s.sub_tracks[sub_track].id()))
    }
}

/// `MenuRequestsSong("RW_65 - Garden", 100, 50)` (the options menu's song) on a clip of
/// 120 ticks, then a fade out over 20 (`SlugcatSelectMenu.StartGame`'s `FadeOut(20f)`).
///
/// Decompiled order per update: `song.Update()` (each `SubTrack.Update`: volume from
/// the piece's previous `volume`; load on the first update after the bundles are
/// loaded, ready on the next; then `StartPlaying`; then
/// `volume = mainSongMix * baseVolume`), then
/// `mainSongMix = Min(1, mainSongMix + 1 / (1 + fadeInTime))`, or while fading
/// `Max(0, mainSongMix - 1 / (1 + fadeOutTime))` and destroy at 0.
#[test]
fn menu_song_fade_in_and_out_ladder() {
    let mut run = Run::new(&[("RW_65 - Garden", 120)]);
    run.player.menu_requests_song("RW_65 - Garden", 100.0, 50.0);
    // Expected state, advanced with the decompiled formulas.
    let mut mix = 0.0f32;
    let mut piece_volume = 0.3f32;
    let mut started = false;
    for tick in 1..=60 {
        run.tick();
        // Bundles: requested on update 1, loaded on 2; clip asked on 2, ready and
        // started on 3.
        if tick == 3 {
            started = true;
        }
        let expected_source =
            started.then(|| MusicSettings::source_volume(1.0 * piece_volume * 0.8, EXPONENT));
        // The source's volume is first set on the update after it started.
        if tick > 3 {
            assert_eq!(run.source_volume(0), expected_source, "tick {tick}");
        }
        assert_eq!(run.playing(0), tick >= 3, "tick {tick}");
        piece_volume = mix * 0.3;
        mix = (mix + 1.0 / (1.0 + 50.0)).min(1.0);
        assert_eq!(run.player.main_song_mix, mix, "tick {tick}");
        assert_eq!(run.player.song.as_ref().unwrap().volume, piece_volume);
    }
    // 1/51 per update: 1 after 51 updates.
    assert_eq!(run.player.main_song_mix, 1.0);
    run.player.song.as_mut().unwrap().fade_out(20.0);
    let mut ticks = 0;
    while run.player.song.is_some() {
        run.tick();
        ticks += 1;
        let expected = (mix - 1.0 / (1.0 + 20.0)).max(0.0);
        assert_eq!(run.player.main_song_mix, expected, "fade tick {ticks}");
        mix = expected;
    }
    // In f32, 1 - k * (1/21) is still above 0 after 21 steps; 0 on the 22nd.
    assert_eq!(ticks, 22);
    assert_eq!(run.player.main_song_mix, 0.0);
}

/// A song that ends by itself fades out over 10 (`Song.Update`: `FadeOut(10f)` once no
/// track plays and all are ready).
#[test]
fn finished_song_fades_over_ten() {
    let mut run = Run::new(&[("NA_11 - Digital Sundown", 30)]);
    run.player
        .menu_requests_song("NA_11 - Digital Sundown", 1.0, 0.0);
    let mut ended_at = None;
    for tick in 1..=200 {
        run.tick();
        let Some(song) = &run.player.song else {
            break;
        };
        if ended_at.is_none() && song.fading_out() {
            ended_at = Some(tick);
            assert_eq!(song.fade_out_time, 10.0);
        }
    }
    // Started on update 3, 30 ticks of audio: not playing after the advance of tick
    // 32, seen by update 33.
    assert_eq!(ended_at, Some(33));
    assert!(run.player.song.is_none());
}

/// A second song waits as `nextSong`; the current one is cut by 0.0125 per update
/// (`mainSongMix - 0.0125`, destroyed at 0), then the next starts at once (fade-in 0
/// sets the mix to 1).
#[test]
fn next_song_replaces_the_current_one() {
    let mut run = Run::new(&[("A", 4000), ("B", 4000)]);
    run.player.menu_requests_song("A", 1.0, 0.0);
    for _ in 0..10 {
        run.tick();
    }
    assert_eq!(run.player.main_song_mix, 1.0);
    run.player.menu_requests_song("B", 2.0, 0.0);
    assert_eq!(run.player.next_song.as_ref().unwrap().name, "B");
    // Lower priority than the playing song: refused.
    run.player.menu_requests_song("C", 0.5, 0.0);
    assert_eq!(run.player.next_song.as_ref().unwrap().name, "B");
    let mut mix = 1.0f32;
    let mut updates = 0;
    while run.player.song.as_ref().is_some_and(|s| s.name == "A") {
        run.tick();
        updates += 1;
        mix = (mix - 0.0125).max(0.0);
        assert_eq!(run.player.main_song_mix, mix, "update {updates}");
    }
    // In f32, 1 - k * 0.0125 is still above 0 after 80 steps; 0 on the 81st.
    assert_eq!(updates, 81);
    run.tick();
    let song = run.player.song.as_ref().unwrap();
    assert_eq!(song.name, "B");
    assert!(song.play_when_ready);
    assert_eq!(run.player.main_song_mix, 1.0);
}

/// `IntroRollMusic`: the rain starts when both clips are loaded; `StartMusic` (the
/// IntroRoll's 8.5 s mark) starts `RW_8 - Sundown`; `GoToMenu`'s `fadeOutRain` takes
/// the rain down by 0.02 per update; `SlugcatSelectMenu.StartGame`'s `FadeOut(20)`,
/// then `UpdateMusicContext(Game)`'s `FadeOut(2)` (the shorter wins).
#[test]
fn intro_roll_music_sequence() {
    let mut run = Run::new(&[("TitleRollRain", 6000), ("RW_8 - Sundown", 8000)]);
    run.player.request_intro_roll_music();
    let song = run.player.song.as_ref().unwrap();
    assert_eq!(
        (song.priority, song.fade_in_time, song.stop_at_death),
        (0.0, 10.0, true)
    );
    for tick in 1..=340 {
        run.tick();
        assert_eq!(run.playing(0), tick >= 3, "rain, tick {tick}");
        assert!(!run.playing(1), "music, tick {tick}");
    }
    run.player.song.as_mut().unwrap().start_music();
    run.tick();
    assert!(run.playing(1));
    // Track volume 1, piece volume mainSongMix (1) * baseVolume 0.3, music 0.8.
    run.tick();
    let full = MusicSettings::source_volume(0.3 * 0.8, EXPONENT);
    assert_eq!(run.source_volume(1), Some(full));
    assert_eq!(run.source_volume(0), Some(full));
    // GoToMenu.
    {
        let song = run.player.song.as_mut().unwrap();
        song.start_music();
        song.fade_out_rain();
    }
    run.player
        .update_music_context(Some(MusicProcess::MainMenu));
    let mut rain = 1.0f32;
    for tick in 1..=60 {
        run.tick();
        // The track volume set by update n reaches the source on update n + 1.
        let expected = MusicSettings::source_volume(rain * 0.3 * 0.8, EXPONENT);
        assert_eq!(run.source_volume(0), Some(expected), "rain fade {tick}");
        rain = (rain - 0.02).max(0.0);
    }
    assert_eq!(run.source_volume(0), Some(0.0));
    assert_eq!(run.source_volume(1), Some(full));
    run.player
        .update_music_context(Some(MusicProcess::SlugcatSelect));
    assert!(!run.player.song.as_ref().unwrap().fading_out());
    run.player.song.as_mut().unwrap().fade_out(20.0);
    run.player
        .update_music_context(Some(MusicProcess::StoryGame));
    assert_eq!(run.player.song.as_ref().unwrap().fade_out_time, 2.0);
    let mut updates = 0;
    while run.player.song.is_some() {
        run.tick();
        updates += 1;
    }
    // 1 - k/3 reaches 0 on the third update.
    assert_eq!(updates, 3);
}

/// `GameRequestsSongStop`: prio at least the song's, and the type's name rule.
#[test]
fn stop_music_event_rules() {
    let mut run = Run::new(&[("A", 4000)]);
    run.player.menu_requests_song("A", 0.5, 0.0);
    run.tick();
    let stop = StopMusicEvent {
        stop_type: StopMusicType::AllButSpecific,
        song_name: "A".into(),
        prio: 1.0,
        fade_out_time: 82.0,
    };
    run.player.game_requests_song_stop(&stop);
    assert!(!run.player.song.as_ref().unwrap().fading_out());
    run.player.game_requests_song_stop(&StopMusicEvent {
        prio: 0.4,
        stop_type: StopMusicType::AllSongs,
        ..stop.clone()
    });
    assert!(!run.player.song.as_ref().unwrap().fading_out());
    run.player.game_requests_song_stop(&StopMusicEvent {
        stop_type: StopMusicType::SpecificSong,
        ..stop
    });
    assert_eq!(run.player.song.as_ref().unwrap().fade_out_time, 82.0);
}

/// No player without `playMusic` or with the music volume at 0 (`ProcessManager.Update`).
#[test]
fn no_player_when_music_is_off() {
    let mut settings = MusicSettings::new(EXPONENT);
    settings.music_volume = 0.0;
    assert!(MusicPlayer::new(settings, None).is_none());
    settings.music_volume = 0.8;
    settings.play_music = false;
    assert!(MusicPlayer::new(settings, None).is_none());
}
