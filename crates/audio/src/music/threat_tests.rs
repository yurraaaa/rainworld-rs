//! `ThreatDetermination.Update` and `ThreatOfCreature` (73833-74032), with expected
//! values worked out from the decompiled formulas.

use rw_assets::CreatureType;
use rw_sim::creature::{CreatureDanger, CreatureId, EntityId, TrackedRep};
use rw_sim::{INPUT_HISTORY, Input, Vec2};

use super::{GameMusicView, ThreatDetermination};

fn creature(danger: f32, at: Vec2) -> CreatureDanger {
    CreatureDanger {
        creature: CreatureId(1),
        id: EntityId::new(-1, 1),
        kind: CreatureType::PinkLizard,
        dangerous_to_player: danger,
        dead: false,
        danger_pos: at,
        first_chunk_vel: Vec2::ZERO,
        player_rep: None,
        player_aggression: Some(1.0),
    }
}

fn view(room: i32, creatures: Vec<CreatureDanger>) -> GameMusicView {
    GameMusicView {
        room,
        game_over: false,
        bkg_drone_volume: 0.277,
        rain_mutes_drone: false,
        single_room_world: false,
        music_bank: "SU".to_owned(),
        player_pos: Vec2::ZERO,
        input: [Input::NONE; INPUT_HISTORY],
        creatures,
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

/// 1000 px away, untracked: `LerpMap(1000, 300, 2400, 1, 0)` = 2/3, no speed bonus,
/// `Lerp(1/3, 1, 0)` = 1/3: 0.45 * 2/9 = 0.1. Dead it is a third of the danger; no
/// danger is 0 however close.
#[test]
fn threat_of_a_creature_by_distance() {
    let far = creature(0.45, Vec2::new(1000.0, 0.0));
    assert!(close(
        ThreatDetermination::threat_of_creature(&far, Vec2::ZERO),
        0.1
    ));
    let dead = CreatureDanger { dead: true, ..far };
    assert!(close(
        ThreatDetermination::threat_of_creature(&dead, Vec2::ZERO),
        0.15
    ));
    let harmless = creature(0.0, Vec2::new(10.0, 0.0));
    assert_eq!(
        ThreatDetermination::threat_of_creature(&harmless, Vec2::ZERO),
        0.0
    );
    // Beyond 2400 px only a creature that sees the player counts (at 0.2).
    let seeing = CreatureDanger {
        danger_pos: Vec2::new(3000.0, 0.0),
        player_rep: Some(TrackedRep {
            visual_contact: true,
            estimated_chance_of_finding: 1.0,
        }),
        ..far
    };
    assert!(close(
        ThreatDetermination::threat_of_creature(&seeing, Vec2::ZERO),
        0.09
    ));
    let unseen = CreatureDanger {
        player_rep: None,
        ..seeing
    };
    assert_eq!(
        ThreatDetermination::threat_of_creature(&unseen, Vec2::ZERO),
        0.0
    );
}

/// 160 px away (`InverseLerp(300, 20, 160)` = 0.5) at speed 4.5 (`InverseLerp(2, 7)` =
/// 0.5): x1.25; sure of the player: x1; aggression 0.4: 0.8 * 1.25 * 0.4 = 0.4.
#[test]
fn a_close_fast_creature_counts_more() {
    let c = CreatureDanger {
        first_chunk_vel: Vec2::new(0.0, 4.5),
        player_rep: Some(TrackedRep {
            visual_contact: false,
            estimated_chance_of_finding: 1.0,
        }),
        player_aggression: Some(0.4),
        ..creature(0.8, Vec2::new(160.0, 0.0))
    };
    assert!(close(
        ThreatDetermination::threat_of_creature(&c, Vec2::ZERO),
        0.4
    ));
}

/// A threat of 1 with a still player (power `LerpMap(0, 5, 30, 1.2, 0.8)` = 1.2): the
/// current threat rises by 1/80 a tick. The first room counts as a change (decline
/// counters 120, then 119). Without a drone only the music-agnostic threat follows.
#[test]
fn the_threat_rises_toward_a_creature() {
    let c = CreatureDanger {
        player_rep: Some(TrackedRep {
            visual_contact: true,
            estimated_chance_of_finding: 1.0,
        }),
        ..creature(1.0, Vec2::new(100.0, 0.0))
    };
    let mut t = ThreatDetermination::default();
    t.update(&view(5, vec![c]), 0.0);
    assert_eq!((t.room, t.last_room, t.last_last_room), (5, -1, -1));
    assert_eq!(t.threat_decline_counter, 119);
    assert_eq!(t.threat, 1.0);
    assert!(close(t.current_threat, 1.0 / 80.0));
    assert!(close(t.current_music_agnostic_threat, 1.0 / 80.0));
    assert_eq!(t.creatures.len(), 1);
    assert_eq!(t.creatures[0].threat, 1.0);

    let mut silent = ThreatDetermination::default();
    let mut v = view(5, vec![c]);
    v.bkg_drone_volume = 0.0;
    silent.update(&v, 0.0);
    assert_eq!(silent.threat, 0.0);
    assert_eq!(silent.current_threat, 0.0);
    assert!(close(silent.current_music_agnostic_threat, 1.0 / 80.0));
}

/// A creature threat of 0.4 is raised to the power 1.2 (a still player), then, between
/// 0.25 and 0.75, drawn toward 0.5 by `InverseLerp(0.25, 0, |t - 0.5|)`.
#[test]
fn a_middle_threat_is_drawn_to_one_half() {
    let c = CreatureDanger {
        player_rep: Some(TrackedRep {
            visual_contact: true,
            estimated_chance_of_finding: 1.0,
        }),
        ..creature(0.4, Vec2::new(100.0, 0.0))
    };
    let mut t = ThreatDetermination::default();
    t.update(&view(5, vec![c]), 0.0);
    let raw = 0.4_f32.powf(1.2);
    let expected = raw + (0.5 - raw) * ((raw - 0.5).abs() - 0.25) / -0.25;
    assert!(close(t.threat, expected));
}

/// `DeathEvent`'s decline counter (400) makes the current threat fall by
/// `1 / Lerp(800, 4200, 0)` a tick, against `1 / Lerp(1600, 22000, 0)` without it.
#[test]
fn the_threat_declines_faster_after_a_death() {
    let mut t = ThreatDetermination {
        room: 5,
        current_threat: 0.5,
        current_music_agnostic_threat: 0.5,
        threat_decline_counter: 400,
        ..ThreatDetermination::default()
    };
    t.update(&view(5, Vec::new()), 0.0);
    assert_eq!(t.threat_decline_counter, 399);
    assert!(close(t.current_threat, 0.5 - 1.0 / 800.0));
    // The music-agnostic one had no death: the threat dropping more than 0.35 below
    // it sets its counter to 10 (after the tick's decrement), so it falls fast too.
    assert_eq!(t.music_agnostic_threat_decline_counter, 10);
    assert!(close(t.current_music_agnostic_threat, 0.5 - 1.0 / 800.0));

    let mut slow = ThreatDetermination {
        room: 5,
        current_threat: 0.3,
        ..ThreatDetermination::default()
    };
    slow.update(&view(5, Vec::new()), 0.0);
    assert_eq!(slow.threat_decline_counter, 0);
    assert!(close(slow.current_threat, 0.3 - 1.0 / 1600.0));
}

/// The newest input against each older one: a direction or pick-up 1, jump or throw 2.
#[test]
fn input_changes_weigh_jump_and_throw_double() {
    let mut input = [Input::NONE; INPUT_HISTORY];
    input[0] = Input {
        x: 1,
        jump: true,
        ..Input::NONE
    };
    assert_eq!(ThreatDetermination::input_changes(&input), 3 * 9);
    input[3] = input[0];
    assert_eq!(ThreatDetermination::input_changes(&input), 3 * 8);
}

/// In the music player: the tracker's threat follows its `ThreatDetermination`, a song
/// whose `maxThreatLevel` is below it is refused, a playing song forces the threat to
/// 0, and `DeathEvent` raises the decline counter to 400.
#[test]
fn the_player_reads_the_threat() {
    use crate::music::{MusicEvent, MusicPlayer, MusicProcess, MusicSettings, TimedSources};
    use rw_random::UnityRandom;

    let mut player =
        MusicPlayer::new(MusicSettings::new(1.8), Some(MusicProcess::StoryGame)).expect("music on");
    let mut sources = TimedSources::new(None);
    let mut rng = UnityRandom::new(7);
    let c = CreatureDanger {
        player_rep: Some(TrackedRep {
            visual_contact: true,
            estimated_chance_of_finding: 1.0,
        }),
        ..creature(1.0, Vec2::new(100.0, 0.0))
    };
    player.set_game_view(Some(view(5, vec![c])));
    player.update(&mut sources, &mut rng);
    let tracker = player.threat_tracker.as_ref().expect("tracker");
    assert!(close(tracker.current_threat, 1.0 / 80.0));

    let event = MusicEvent {
        song_name: "RW_1 - Urban Jungle".to_owned(),
        max_threat_level: 0.01,
        ..MusicEvent::default()
    };
    player.game_requests_song(&event, None);
    assert!(player.song.is_none());
    let event = MusicEvent {
        max_threat_level: 0.02,
        ..event
    };
    player.game_requests_song(&event, None);
    assert!(player.song.is_some());
    player.update(&mut sources, &mut rng);
    let tracker = player.threat_tracker.as_ref().expect("tracker");
    assert_eq!(tracker.current_threat, 0.0);
    assert_eq!(tracker.threat_determine.current_threat, 0.0);

    player.death_event();
    let d = &player
        .threat_tracker
        .as_ref()
        .expect("tracker")
        .threat_determine;
    assert_eq!(d.threat_decline_counter, 400);
}

/// A game over zeroes the current threats and changes nothing else.
#[test]
fn game_over_zeroes_the_threat() {
    let mut t = ThreatDetermination {
        current_threat: 0.7,
        current_music_agnostic_threat: 0.7,
        threat_decline_counter: 50,
        ..ThreatDetermination::default()
    };
    let mut v = view(5, Vec::new());
    v.game_over = true;
    t.update(&v, 0.0);
    assert_eq!(
        (t.current_threat, t.current_music_agnostic_threat),
        (0.0, 0.0)
    );
    assert_eq!((t.threat_decline_counter, t.room), (50, -1));
}
