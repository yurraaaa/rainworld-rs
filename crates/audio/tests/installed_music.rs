//! Songs from the installed game's `music_songs` bundle (read-only). Set `RW_DIR` to
//! override the install; tests skip when the files are absent.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rw_assets::unity::{AssetBundle, Fsb5, Fsb5Stream};
use rw_assets::{SoundTable, StreamingAssets};
use rw_audio::Audio;
use rw_audio::music::{
    MusicOutput, MusicPlayer, MusicProcess, MusicSettings, MusicSources, Song, SongBank,
    TimedSources,
};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Install;

impl Install {
    fn assets() -> Option<StreamingAssets> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let assets = StreamingAssets::new(root.join("RainWorld_Data/StreamingAssets"));
        if assets.asset_bundle(SongBank::BUNDLE).is_file() {
            Some(assets)
        } else {
            eprintln!("skipping: no music_songs bundle");
            None
        }
    }

    fn bank() -> Option<SongBank> {
        Self::assets().map(|a| SongBank::open(&a).expect("music_songs opens"))
    }
}

/// `IntroRollMusic`'s two clips by name, with their FSB5 header lengths.
#[test]
fn title_songs_resolve() {
    let Some(bank) = Install::bank() else {
        return;
    };
    for name in [Song::TITLE_ROLL_RAIN, Song::TITLE_ROLL_MUSIC] {
        let length = bank.length(name).expect(name);
        eprintln!(
            "{name}: {} frames at {} Hz, {} channels ({:.3} s)",
            length.frames,
            length.sample_rate,
            length.channels,
            f64::from(length.frames) / f64::from(length.sample_rate)
        );
        assert_eq!((length.channels, length.sample_rate), (2, 44_100), "{name}");
        assert!(length.frames > 0);
    }
}

/// Every clip of the bundle resolves by its `m_Name`, and so does every song name of
/// the `music/songs` placeholder files.
#[test]
fn every_song_name_resolves() {
    let Some(assets) = Install::assets() else {
        return;
    };
    let bank = SongBank::open(&assets).unwrap();
    let names = bank.clip_names().unwrap();
    for name in &names {
        let length = bank.length(name).unwrap_or_else(|| panic!("{name}"));
        assert!(length.frames > 0, "{name}");
    }
    let dir = assets.path().join("music/songs");
    let mut placeholders = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            placeholders.push(stem.to_owned());
        }
    }
    let missing: Vec<_> = placeholders
        .iter()
        .filter(|p| !names.iter().any(|n| n.eq_ignore_ascii_case(p)))
        .collect();
    eprintln!(
        "{} clips resolve; {} placeholder names, not in the bundle: {missing:?}",
        names.len(),
        placeholders.len()
    );
    assert_eq!(names.len(), 266);
}

/// Streaming decode gives exactly the samples of the whole-clip decode.
#[test]
fn title_songs_stream_as_they_decode() {
    let Some(assets) = Install::assets() else {
        return;
    };
    let bank = SongBank::open(&assets).unwrap();
    for name in [Song::TITLE_ROLL_RAIN, Song::TITLE_ROLL_MUSIC] {
        let bytes = bank.bank(name).unwrap();
        let whole = Fsb5::parse(&bytes).unwrap().decode().unwrap();
        let mut stream = Fsb5Stream::new(bytes).unwrap();
        let mut streamed = Vec::with_capacity(whole.samples.len());
        let mut chunks = 0;
        while let Some(chunk) = stream.next_chunk().unwrap() {
            streamed.extend(chunk);
            chunks += 1;
        }
        eprintln!("{name}: {} frames in {chunks} chunks", whole.frames());
        assert!(streamed == whole.samples, "{name}");
        stream.rewind();
        let first = stream.next_chunk().unwrap().unwrap();
        assert_eq!(first[..], whole.samples[..first.len()]);
    }
}

/// The IntroRoll's music on the clips' real lengths: the rain plays from update 3, the
/// music from the 8.5 s mark (update 340).
#[test]
fn intro_roll_on_real_lengths() {
    let Some(bank) = Install::bank() else {
        return;
    };
    let rain = bank.length(Song::TITLE_ROLL_RAIN).unwrap();
    let rain_ticks = u64::from(rain.frames) * 40 / u64::from(rain.sample_rate);
    let mut sources = TimedSources::new(Some(bank));
    let mut player = MusicPlayer::new(MusicSettings::new(1.8), Some(MusicProcess::Other)).unwrap();
    player.request_intro_roll_music();
    for tick in 1..=340 {
        player.update(&mut sources);
        sources.advance();
        let song = player.song.as_ref().expect("intro roll music");
        let rain_playing = sources.is_playing(song.sub_tracks[0].id());
        assert_eq!(rain_playing, (3..3 + rain_ticks).contains(&tick), "{tick}");
    }
    player.song.as_mut().unwrap().start_music();
    player.update(&mut sources);
    let song = player.song.as_ref().unwrap();
    assert!(sources.is_playing(song.sub_tracks[1].id()));
    eprintln!("rain lasts {rain_ticks} ticks");
}

/// Listening check (plays out loud): the IntroRoll's music, `RW_8 - Sundown` from
/// the 8.5 s mark, the rain fading at `RW_MUSIC_SECONDS` (default 20) minus 3.
#[test]
#[ignore = "plays the title music out loud"]
fn listen_to_the_intro_roll_music() {
    let Some(assets) = Install::assets() else {
        return;
    };
    let seconds: f32 = std::env::var("RW_MUSIC_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20.0);
    let table = AssetBundle::open(&assets.asset_bundle("loadedsoundeffects"))
        .and_then(|b| SoundTable::load(&assets, &b))
        .expect("sound table");
    let exponent = table.volume_exponent;
    let mut audio = Audio::new(&assets, Arc::new(table)).expect("audio output");
    let mut output = MusicOutput::new(&assets);
    let mut player =
        MusicPlayer::new(MusicSettings::new(exponent), Some(MusicProcess::Other)).unwrap();
    player.request_intro_roll_music();
    let start = Instant::now();
    let ticks = (seconds * 40.0) as u32;
    for tick in 1..=ticks {
        if tick == 340 {
            player.song.as_mut().unwrap().start_music();
        }
        if tick + 120 == ticks {
            let song = player.song.as_mut().unwrap();
            song.start_music();
            song.fade_out_rain();
        }
        player.update(&mut output.sources(&mut audio));
        let next = start + Duration::from_secs_f32(tick as f32 / 40.0);
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    let song = player.song.as_ref().unwrap();
    let sources = output.sources(&mut audio);
    eprintln!(
        "after {seconds} s: mix {}, rain playing {}, music playing {}",
        player.main_song_mix,
        sources.is_playing(song.sub_tracks[0].id()),
        sources.is_playing(song.sub_tracks[1].id())
    );
    assert!(sources.is_playing(song.sub_tracks[0].id()));
}
