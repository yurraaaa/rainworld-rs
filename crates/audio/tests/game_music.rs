//! The music player in a story game: `GameRequestsSong`'s rules (414568) and the
//! `PlayerThreatTracker`'s room switches and `Random.Range(80, 400)` delays
//! (414790-414971), on silent timed sources. Expected values are read off the
//! decompiled code.

use rw_audio::music::{
    GameMusicView, MusicEvent, MusicPlayer, MusicProcess, MusicSettings, StorySongs, TimedSources,
};
use rw_random::UnityRandom;
use rw_save::SongPlayRecord;
use rw_sim::{INPUT_HISTORY, Input, Vec2};

struct Story {
    player: MusicPlayer,
    sources: TimedSources,
    rng: UnityRandom,
    records: Vec<SongPlayRecord>,
    cycle: i32,
}

impl Story {
    fn new() -> Self {
        let player = MusicPlayer::new(MusicSettings::new(1.8), Some(MusicProcess::StoryGame))
            .expect("music on");
        Self {
            player,
            sources: TimedSources::new(None),
            rng: UnityRandom::new(7),
            records: Vec::new(),
            cycle: 3,
        }
    }

    fn event(name: &str, prio: f32) -> MusicEvent {
        MusicEvent {
            song_name: name.to_owned(),
            prio,
            ..MusicEvent::default()
        }
    }

    fn request(&mut self, event: &MusicEvent) {
        let story = StorySongs {
            records: &mut self.records,
            cycle_number: self.cycle,
        };
        self.player.game_requests_song(event, Some(story));
    }

    fn song(&self) -> Option<&str> {
        self.player.song.as_ref().map(|s| s.name.as_str())
    }

    fn next(&self) -> Option<&str> {
        self.player.next_song.as_ref().map(|s| s.name.as_str())
    }

    fn view(room: i32) -> GameMusicView {
        GameMusicView {
            room,
            game_over: false,
            bkg_drone_volume: 0.276_956_5,
            rain_mutes_drone: false,
            single_room_world: false,
            music_bank: "SU".to_owned(),
            player_pos: Vec2::ZERO,
            input: [Input::NONE; INPUT_HISTORY],
            creatures: Vec::new(),
        }
    }

    fn tick_in(&mut self, room: i32) {
        self.player.set_game_view(Some(Self::view(room)));
        self.player.update(&mut self.sources, &mut self.rng);
        self.sources.advance();
    }
}

/// The first request plays at once with the event's fields and writes the record; a
/// request of the same song, or of one of no higher priority, is refused; a higher one
/// waits as the next song.
#[test]
fn requests_play_queue_and_refuse() {
    let mut s = Story::new();
    let mut proxima = Story::event("NA_01 - Proxima", 0.5);
    proxima.volume = 0.310_869_3;
    proxima.fade_in_time = 202.173_9;
    proxima.rooms_range = 6;
    s.request(&proxima);
    let song = s.player.song.as_ref().expect("song");
    assert_eq!(song.name, "NA_01 - Proxima");
    assert_eq!(
        (song.base_volume, song.fade_in_time),
        (0.310_869_3, 202.173_9)
    );
    assert_eq!((song.priority, song.fade_out_at_threat), (0.5, 1.0));
    assert_eq!(song.room_transitions, 6);
    assert!(song.play_when_ready && s.player.has_played_a_song_this_cycle);
    assert_eq!(
        s.records,
        [SongPlayRecord {
            song_name: "NA_01 - Proxima".into(),
            cycle_last_played: 3
        }]
    );

    s.request(&Story::event("RW_1 - Urban Jungle", 0.5));
    assert_eq!(s.next(), None);
    s.request(&Story::event("NA_40 - Unseen Lands", 1.0));
    assert_eq!(s.next(), Some("NA_40 - Unseen Lands"));
    assert_eq!(s.records.len(), 2);
    // A waiting song of at least the priority keeps its place, and no record is
    // written for the refused one.
    s.request(&Story::event("RW_2 - Other", 1.0));
    assert_eq!(s.next(), Some("NA_40 - Unseen Lands"));
    assert_eq!(s.records.len(), 2);
}

/// `rest` cycles must pass since the song last played in this save; a negative rest is
/// never again; `oneSongPerCycle` waits for `NewCycleEvent`.
#[test]
fn rest_and_one_song_per_cycle() {
    let mut s = Story::new();
    s.records.push(SongPlayRecord {
        song_name: "RW_1 - Urban Jungle".into(),
        cycle_last_played: 0,
    });
    s.request(&Story::event("RW_1 - Urban Jungle", 0.5));
    assert_eq!(s.song(), None);
    s.cycle = 5;
    s.request(&Story::event("RW_1 - Urban Jungle", 0.5));
    assert_eq!(s.song(), Some("RW_1 - Urban Jungle"));
    assert_eq!(s.records[0].cycle_last_played, 5);

    let mut never = Story::new();
    never.records.push(SongPlayRecord {
        song_name: "X".into(),
        cycle_last_played: -100,
    });
    let mut x = Story::event("X", 0.5);
    x.cycles_rest = -1;
    never.request(&x);
    assert_eq!(never.song(), None);

    let mut once = Story::new();
    once.request(&Story::event("A", 0.2));
    let mut b = Story::event("B", 0.9);
    b.one_song_per_cycle = true;
    once.request(&b);
    assert_eq!(once.next(), None);
    once.player.new_cycle_event();
    once.request(&b);
    assert_eq!(once.next(), Some("B"));
}

/// Refused above its threat level or with music off; outside a story save no record.
#[test]
fn threat_and_play_music() {
    let mut s = Story::new();
    s.player
        .threat_tracker
        .as_mut()
        .expect("story tracker")
        .current_threat = 0.5;
    let mut calm = Story::event("Calm", 0.5);
    calm.max_threat_level = 0.4;
    s.request(&calm);
    assert_eq!(s.song(), None);
    s.player.settings.play_music = false;
    s.request(&Story::event("Any", 0.5));
    assert_eq!(s.song(), None);
    s.player.settings.play_music = true;
    s.player.game_requests_song(&Story::event("Any", 0.5), None);
    assert_eq!(s.song(), Some("Any"));
    assert!(s.records.is_empty());
}

/// The first room counts as a switch and asks for the region's bank; 200 ticks later
/// the delay ends: the songs move a room nearer their end and one `Range(80, 400)` is
/// drawn. Going back to the room before last is no switch.
#[test]
fn room_switch_delay_draws_once() {
    let mut s = Story::new();
    let mut proxima = Story::event("NA_01 - Proxima", 0.5);
    proxima.rooms_range = 1;
    s.request(&proxima);
    s.tick_in(10);
    let t = s.player.threat_tracker.clone().expect("tracker");
    assert_eq!((t.room, t.room_switches, t.room_switch_delay), (10, 1, 199));
    assert_eq!(t.region.as_deref(), Some("SU"));
    assert_eq!(s.player.next_procedural.as_deref(), Some("SU"));
    assert_eq!(t.recommended_drone_volume, 0.276_956_5);
    let before = s.rng.clone();
    for _ in 0..198 {
        s.tick_in(10);
    }
    assert_eq!(s.rng, before);
    s.tick_in(10);
    let mut drawn = before.clone();
    let delay = drawn.range_i32(80, 400);
    assert_eq!(s.rng, drawn);
    let t = s.player.threat_tracker.clone().expect("tracker");
    assert_eq!((t.room_switches, t.room_switch_delay), (0, delay));
    // Requested before any switch: `rooms` 1 + 0; `PlayerToNewRoom` spends it and
    // fades the song over 400 ticks.
    let song = s.player.song.as_ref().expect("song");
    assert_eq!((song.room_transitions, song.fade_out_time), (0, 400.0));

    s.tick_in(11);
    s.tick_in(10);
    let t = s.player.threat_tracker.clone().expect("tracker");
    assert_eq!((t.last_last_room, t.last_room, t.room), (10, 11, 10));
    assert_eq!(t.room_switches, 1);
}

/// Outside the game the tracker zeroes its levels and forgets the bank.
#[test]
fn tracker_outside_the_game() {
    let mut s = Story::new();
    s.tick_in(4);
    s.player.update_music_context(Some(MusicProcess::Other));
    s.player.update(&mut s.sources, &mut s.rng);
    let t = s.player.threat_tracker.clone().expect("tracker");
    assert_eq!((t.recommended_drone_volume, t.region), (0.0, None));
    assert_eq!(t.room, 4);
}
