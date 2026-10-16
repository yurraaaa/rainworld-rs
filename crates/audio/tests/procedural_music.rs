//! `ProceduralMusic` (414974-415435) and `MusicPlayer.NewRegion` (414683) on silent
//! timed sources: the bank parse, `Reset`'s draws, the re-scramble timing and the layer
//! volumes. Expected values are read off the decompiled code; the bank below has the
//! shape of the installed `su.txt` (8 stems, 5 layers).

use std::sync::Arc;

use rw_audio::music::{
    DayNight, HudSubregion, MusicPlayer, MusicProcess, MusicSettings, ProceduralFiles,
    ProceduralInstruction, ProceduralView, TimedSources,
};
use rw_random::UnityRandom;

/// A bank file and the stems the bundle has.
#[derive(Debug)]
struct Bank {
    text: &'static str,
    stems: Vec<&'static str>,
}

impl ProceduralFiles for Bank {
    fn instruction_text(&self, bank: &str) -> Option<String> {
        (bank == "SU").then(|| self.text.to_owned())
    }

    fn contains_stem(&self, stem: &str) -> bool {
        self.stems.iter().any(|s| s.eq_ignore_ascii_case(stem))
    }
}

const SU: &str = "\
A - KICK : NEUTRAL
A - SHAKER : NEUTRAL
A - PERC1 : NEUTRAL
A - ARPS : NEUTRAL
A - BASS : DANGER
A - HITS : NEUTRAL
A - NOISE : NEUTRAL
A - LEAD : HIGHKARMA


Layer : A - KICK, A - SHAKER
Layer : A - KICK, A - SHAKER, A - PERC1, A - NOISE
Layer : A - KICK, A - SHAKER, A - PERC1, A - NOISE, A - HITS, A - BASS
Layer : A - KICK, A - SHAKER, A - PERC1, A - ARPS, A - NOISE, A - HITS, A - BASS
Layer : A - LEAD, A - BASS";

impl Bank {
    fn su() -> Self {
        Self {
            text: SU,
            stems: vec![
                "A - KICK",
                "A - SHAKER",
                "A - PERC1",
                "A - ARPS",
                "A - BASS",
                "A - HITS",
                "A - NOISE",
                "A - LEAD",
            ],
        }
    }
}

struct Run {
    player: MusicPlayer,
    sources: TimedSources,
    rng: UnityRandom,
}

impl Run {
    /// A story game's player with its bundles loaded (two updates) and the view set.
    fn new(view: ProceduralView) -> Self {
        let player = MusicPlayer::new(MusicSettings::new(1.8), Some(MusicProcess::StoryGame))
            .expect("music on")
            .with_procedural_files(Arc::new(Bank::su()));
        let mut run = Self {
            player,
            sources: TimedSources::new(None),
            rng: UnityRandom::new(11),
        };
        run.player.set_procedural_view(Some(view));
        run.update();
        run.update();
        assert!(run.player.asset_bundles_loaded());
        run
    }

    fn view() -> ProceduralView {
        ProceduralView {
            sub_regions: Some(vec![String::new(), "Outskirts".to_owned()]),
            camera_room: true,
            camera_room_subregion: Some("Outskirts".to_owned()),
            hud: HudSubregion::Tracker {
                last_shown_region: 1,
            },
            night: false,
            mushroom_mode: 0.0,
        }
    }

    fn update(&mut self) {
        self.player.update(&mut self.sources, &mut self.rng);
        self.sources.advance();
    }

    /// Draws made by `f`, counted by replaying the stream.
    fn draws(&mut self, f: impl FnOnce(&mut Self)) -> usize {
        let before = self.rng.clone();
        f(self);
        let mut replay = before;
        let mut n = 0;
        while replay != self.rng {
            replay.next_u32();
            n += 1;
            assert!(n < 100, "the stream did not line up");
        }
        n
    }

    fn new_region(&mut self) -> usize {
        self.draws(|r| r.player.new_region("SU", &mut r.rng))
    }

    fn stems(&self) -> Vec<String> {
        let piece = self.player.procedural_music.as_ref().expect("a piece");
        piece
            .piece
            .sub_tracks
            .iter()
            .map(|t| t.track_name.clone())
            .collect()
    }
}

#[test]
fn the_bank_keeps_stems_in_the_bundle_and_layers_of_tracks_above() {
    let bank = Bank {
        text: "\
X - ONE : NEUTRAL, <PA>
X - TWO : NEURTAL <PA>
X - GONE : NEUTRAL
Layer : X - ONE, X - THREE,
X - THREE : WEIRD
Layer : {D}X - ONE, {N}X - TWO, {M}X - THREE, X - GONE
Layer : {Outskirts|Drainage}X - TWO, X - ONE , X -TWO
onelayergroup ",
        stems: vec!["x - one", "X - TWO", "X - THREE"],
    };
    let i = ProceduralInstruction::load("SU", &bank);
    let names: Vec<&str> = i.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["X - ONE", "X - TWO", "X - THREE"]);
    assert!(i.tracks[0].remain_in_panic_mode);
    assert_eq!(i.tracks[0].tags, ["NEUTRAL"]);
    // `<PA>` counts only as its own comma-separated tag.
    assert!(!i.tracks[1].remain_in_panic_mode);
    assert_eq!(i.tracks[1].tags, ["NEURTAL <PA>"]);
    // THREE is defined below the first layer; "X - ONE " and "X -TWO" match nothing.
    assert_eq!(i.layers, [vec![0], vec![0, 1, 2], vec![1]]);
    assert_eq!(i.tracks[0].day_night, DayNight::Day);
    assert_eq!(i.tracks[2].day_night, DayNight::Any);
    assert!(i.tracks[2].mushroom);
    // The last layer's prefix replaced TWO's night filter.
    assert_eq!(i.tracks[1].day_night, DayNight::Any);
    assert_eq!(
        i.tracks[1].sub_regions.as_deref(),
        Some(&["Outskirts".to_owned(), "Drainage".to_owned()][..])
    );
    assert!(i.use_one_layer_group);
}

#[test]
fn new_region_builds_the_piece_with_one_draw_per_layer() {
    let mut run = Run::new(Run::view());
    assert_eq!(run.new_region(), 5);
    let piece = run.player.procedural_music.as_ref().unwrap();
    assert_eq!(piece.name, "SU");
    assert_eq!(piece.piece.volume, 0.0);
    assert!(piece.piece.looped && piece.piece.play_when_ready);
    let stems = run.stems();
    assert_eq!(stems.len(), 5);
    // Each layer's pick is one of its tracks, never one an earlier layer took.
    for (i, stem) in stems.iter().enumerate() {
        assert!(!stems[..i].contains(stem), "{stems:?}");
    }
    assert_eq!(run.player.current_procedural_region.as_deref(), Some("SU"));
    assert_eq!(run.player.next_procedural, None);
}

#[test]
fn before_the_bundles_load_the_region_waits_in_next_procedural() {
    let player = MusicPlayer::new(MusicSettings::new(1.8), Some(MusicProcess::StoryGame))
        .unwrap()
        .with_procedural_files(Arc::new(Bank::su()));
    let mut run = Run {
        player,
        sources: TimedSources::new(None),
        rng: UnityRandom::new(3),
    };
    run.player.set_procedural_view(Some(Run::view()));
    assert_eq!(run.new_region(), 0);
    assert_eq!(run.player.next_procedural.as_deref(), Some("SU"));
    // Update 1 asks for the bundles; update 2 finds them loaded and then builds.
    assert_eq!(run.draws(Run::update), 0);
    assert_eq!(run.draws(Run::update), 5);
    assert_eq!(run.player.next_procedural, None);
}

#[test]
fn the_piece_rescrambles_200_silent_updates_after_it_was_built() {
    let mut run = Run::new(Run::view());
    run.new_region();
    let mut resets = Vec::new();
    for tick in 1..=700 {
        let n = run.draws(Run::update);
        if n > 0 {
            resets.push((tick, n));
        }
    }
    // 150 silent updates read the day (`lastValidDaynight` 0 -> 1), which sets the
    // counter back to 100: the reset follows 50 updates later. The subregion stays.
    assert_eq!(resets, [(200, 5)]);
    let piece = run.player.procedural_music.as_ref().unwrap();
    assert_eq!(piece.last_valid_daynight, DayNight::Day);
    assert_eq!(piece.last_valid_subregion.as_deref(), Some("Outskirts"));
    assert_eq!(piece.silent_counter, 650);
}

#[test]
fn nightfall_and_a_new_subregion_each_rescramble_50_updates_after_the_check() {
    let mut run = Run::new(Run::view());
    run.new_region();
    for _ in 0..300 {
        run.update();
    }
    // The silent counter is 250: the next check comes 50 updates on.
    let mut view = Run::view();
    view.night = true;
    run.player.set_procedural_view(Some(view.clone()));
    let mut resets = Vec::new();
    for tick in 1..=200 {
        if run.draws(Run::update) > 0 {
            resets.push(tick);
        }
    }
    assert_eq!(resets, [100]);
    // The tracker shows subregion 0 now: index 0 is never a new subregion.
    view.hud = HudSubregion::Tracker {
        last_shown_region: 0,
    };
    run.player.set_procedural_view(Some(view));
    let mut later = 0;
    for _ in 0..400 {
        later += run.draws(Run::update);
    }
    assert_eq!(later, 0);
}

#[test]
fn at_threat_0_the_layers_are_silent_and_the_piece_eases_to_the_drone_mix() {
    let mut run = Run::new(Run::view());
    run.new_region();
    let drone = 0.277;
    run.player
        .threat_tracker
        .as_mut()
        .unwrap()
        .recommended_drone_volume = drone;
    run.update();
    let piece = run.player.procedural_music.as_ref().unwrap();
    assert!(piece.piece.sub_tracks.iter().all(|t| t.volume == 0.0));
    // `LerpAndTick(0, 0.277, 0.0003, 1/420)`.
    let expected = drone * 0.0003 + 1.0 / 420.0;
    assert!(
        (piece.piece.volume - expected).abs() < 1e-7,
        "{}",
        piece.piece.volume
    );
    assert_eq!(piece.silent_counter, 1);
}

#[test]
fn each_layer_is_full_halfway_through_its_threat_band() {
    let mut run = Run::new(Run::view());
    run.new_region();
    run.player.threat_tracker.as_mut().unwrap().current_threat = 0.5;
    run.update();
    let piece = run.player.procedural_music.as_ref().unwrap();
    let volumes: Vec<f32> = piece.piece.sub_tracks.iter().map(|t| t.volume).collect();
    // t = 0.45 / 0.95; the third band [0.4, 0.6] is (t - 0.4) / 0.2 through, doubled.
    let third = ((0.45f32 / 0.95 - 0.4) / 0.2) / 0.5;
    assert_eq!(volumes[..2], [1.0, 1.0]);
    assert!((volumes[2] - third).abs() < 1e-5, "{volumes:?}");
    assert_eq!(volumes[3..], [0.0, 0.0]);
}

#[test]
fn a_new_bank_waits_for_the_old_piece_to_fade_out() {
    let mut run = Run::new(Run::view());
    run.new_region();
    run.player
        .threat_tracker
        .as_mut()
        .unwrap()
        .recommended_drone_volume = 0.3;
    for _ in 0..10 {
        run.update();
    }
    assert!(run.player.procedural_music.as_ref().unwrap().piece.volume > 0.0);
    assert_eq!(run.new_region(), 0);
    assert_eq!(run.player.next_procedural.as_deref(), Some("SU"));
    let mut faded = None;
    for tick in 1..=500 {
        let n = run.draws(Run::update);
        if run.player.procedural_music.is_none() && faded.is_none() {
            faded = Some(tick);
        }
        if n > 0 {
            // Built on the update after the old piece reached volume 0.
            assert_eq!(Some(tick - 1), faded);
            assert_eq!(n, 5);
            return;
        }
    }
    panic!("no new piece");
}

#[test]
fn outside_a_game_the_piece_stays_muted_and_draws_nothing() {
    let mut run = Run::new(Run::view());
    run.new_region();
    // The sleep screen: the tracker zeroes the drone, the piece reads no game.
    run.player.update_music_context(Some(MusicProcess::Other));
    run.player.set_procedural_view(None);
    let mut draws = 0;
    for _ in 0..1000 {
        draws += run.draws(Run::update);
    }
    assert_eq!(draws, 0);
    let piece = run.player.procedural_music.as_ref().unwrap();
    assert_eq!(piece.piece.volume, 0.0);
    assert!(!piece.re_scramble_on_next_silence);
}

#[test]
fn the_hunters_game_over_and_dreams_fade_every_song_and_the_layers() {
    let mut run = Run::new(Run::view());
    run.new_region();
    run.player.menu_requests_song("RW_8 - Sundown", 1.0, 0.0);
    run.player.reds_game_over();
    assert_eq!(run.player.song.as_ref().unwrap().fade_out_time, 20.0);
    assert_eq!(run.player.next_procedural.as_deref(), Some(""));
    run.update();
    // The piece was at volume 0: dropped, and no bank is built from "".
    assert!(run.player.procedural_music.is_none());
    run.update();
    assert!(run.player.procedural_music.is_none());

    let mut run = Run::new(Run::view());
    run.new_region();
    run.player.menu_requests_song("RW_8 - Sundown", 1.0, 0.0);
    run.player.dream_screen();
    assert_eq!(run.player.song.as_ref().unwrap().fade_out_time, 30.0);
    assert_eq!(run.player.next_procedural.as_deref(), Some(""));
}

#[test]
fn the_credits_ask_for_their_song_after_400_updates() {
    let mut run = Run::new(Run::view());
    let mut credits =
        rw_audio::music::CreditsMusic::new(Some(rw_audio::music::CreditsMusic::DEFAULT_SONG));
    for _ in 0..399 {
        credits.update(Some(&mut run.player));
    }
    assert!(run.player.song.is_none());
    credits.update(Some(&mut run.player));
    let song = run.player.song.as_ref().unwrap();
    assert_eq!(song.name, "RW_8 - Sundown");
    assert_eq!(song.priority, 1.4);
    // A running game's credits have none.
    let mut none = rw_audio::music::CreditsMusic::new(None);
    for _ in 0..500 {
        none.update(Some(&mut run.player));
    }
    assert_eq!(none.music_timer, 400);
}
