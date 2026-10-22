//! `ShelterDoor` against the decompiled constructor, `Reset` and `Update` (262186).

use std::collections::HashMap;
use std::sync::Arc;

use rw_assets::{ActiveMods, ClipNames, Room, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use super::{DoorStep, ShelterDoor};
use crate::tests::room;
use crate::{EmitterId, SoundEvent, SoundId, Sounds, Vec2};

/// A 12 x 10 shelter whose room exit is in the floor at x 5.
fn shelter() -> Room {
    let mut rows = vec!["############"];
    rows.extend(["#..........#"; 7]);
    rows.extend(["#####E######", "#####X######"]);
    room(&rows)
}

fn door(rain_timer: i32, rng: &mut UnityRandom) -> ShelterDoor {
    ShelterDoor::new(
        0,
        &shelter(),
        &RoomSettings::ANCESTOR,
        ActiveMods::NONE,
        rain_timer,
        true,
        rng,
    )
}

/// Every ported ID with one existing clip.
fn all_sounds() -> Sounds {
    let mut text = "Volume: 6\nVolume exponent: 1.8\n".to_owned();
    for id in SoundId::ALL {
        text.push_str(&format!("{} : clip\n", id.name()));
    }
    let names = ClipNames::new(["clip".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(&text, &names).unwrap()))
}

/// The one-shots played since the last call, counted by ID (the loops left out).
fn one_shots(sounds: &mut Sounds) -> HashMap<SoundId, usize> {
    let mut counts = HashMap::new();
    for e in sounds.take_events() {
        if let SoundEvent::Play(p) = e
            && !p.looped
        {
            *counts.entry(p.id).or_insert(0) += 1;
        }
    }
    counts
}

#[test]
fn the_floor_exit_sets_the_layout() {
    let d = door(1000, &mut UnityRandom::new(1));
    // The entrance (5, 1) opens up: pZero 60 px above its middle (110, 30).
    assert_eq!(d.dir, Vec2::new(0.0, 1.0));
    assert_eq!(d.p_zero, Vec2::new(110.0, 90.0));
    assert_eq!(d.close_tiles, [(5, 3), (5, 4), (5, 5), (5, 6)]);
    // Up to the ceiling (5, 8), left to the wall (1, 8), down to the floor (1, 2).
    assert_eq!(d.player_spawn_pos, (1, 2));
    assert!(d.kill_hostiles);
}

#[test]
fn reset_draws_nineteen_values() {
    let mut rng = UnityRandom::new(7);
    let d = door(1000, &mut rng);
    let mut expected = UnityRandom::new(7);
    for s in d.segment_pairs {
        assert_eq!(s[2], expected.value() * 0.5);
    }
    for p in d.pistons {
        assert_eq!(p[2], expected.value() * 0.8);
    }
    for c in d.covers {
        assert_eq!(c[2], expected.value() * 0.5);
    }
    for p in d.pumps {
        assert_eq!(p[2], expected.value() * 0.5);
    }
    // 5 + 2 + 4 + 8 draws, nothing else.
    assert_eq!(rng, expected);
}

#[test]
fn the_door_opens_with_the_cycle_timer() {
    let mut rng = UnityRandom::new(1);
    let mut sounds = Sounds::default();
    // `InverseLerp(initialWait + openUpTicks, initialWait, timer)` = InverseLerp(430, 80, t).
    let mut d = door(0, &mut rng);
    assert_eq!(d.closed_fac, 1.0);
    let step = d.update(80, &mut sounds, &mut rng, &mut Vec::new());
    assert_eq!(d.closed_fac, 1.0);
    // Shut since construction: `lastClosed` turns true with a Reset, the tiles solid.
    assert_eq!(step.close_tiles, Some(true));
    d.update(255, &mut sounds, &mut rng, &mut Vec::new());
    assert_eq!(d.closed_fac, 0.5);
    d.update(429, &mut sounds, &mut rng, &mut Vec::new());
    assert!(d.closed_fac > 0.0);
    assert!(d.ai_mapper.is_none());
    // From 430 the close speed (-1) takes it: open, the tiles air again, and a new
    // `AImapper` starts (opening, `closedFac` from above 0 to 0).
    let step = d.update(430, &mut sounds, &mut rng, &mut Vec::new());
    assert_eq!(d.closed_fac, 0.0);
    assert_eq!(step.close_tiles, Some(false));
    assert_eq!(d.open_time, 0);
    assert_eq!(d.ai_mapper, Some(crate::world::AiMapper::default()));
    d.ai_mapper = None;
    let step = d.update(431, &mut sounds, &mut rng, &mut Vec::new());
    assert_eq!(d.open_time, 1);
    assert!(step.stun_creatures);
    assert!(d.ai_mapper.is_none(), "only once");
}

#[test]
fn closing_takes_320_increments_and_marks_starving_and_sleep() {
    let mut rng = UnityRandom::new(1);
    let mut sounds = all_sounds();
    let mut d = door(1000, &mut rng);
    d.update(1000, &mut sounds, &mut rng, &mut Vec::new());
    one_shots(&mut sounds);
    assert!(!d.is_closing());
    d.close();
    assert_eq!(d.close_speed, 0.003_125);
    let mut marks = Vec::new();
    let mut closed_at = None;
    for t in 1..=400 {
        let step: DoorStep = d.update(1000, &mut sounds, &mut rng, &mut Vec::new());
        if t == 1 {
            assert_eq!(step.close_tiles, Some(true));
        }
        if step.starve_check {
            marks.push(("starve", t));
        }
        if step.sleep_when_still {
            marks.push(("sleep", t));
        }
        if step.closed {
            closed_at = Some(t);
            break;
        }
    }
    // f32 sums of 0.003125: past 0.4 at the 128th, past 0.87 at the 279th, 1 at the
    // 320th; each mark once.
    assert_eq!(marks, [("starve", 128), ("sleep", 279)]);
    assert_eq!(closed_at, Some(320));
    assert_eq!(d.closed_fac, 1.0);
    // Every part's sound once per part: the hatch past 0.04, the pistons out (below
    // 0.2 `PistonsClosed` lets go) and in hard (0.38..0.41), the segment pairs in and
    // together, the covers in and into place, a bolt each quarter of 0.53..0.61, the
    // gaskets sealing.
    let expected = HashMap::from([
        (SoundId::ShelterLittleHatchOpen, 1),
        (SoundId::ShelterPistonOut, 2),
        (SoundId::ShelterPistonInHard, 2),
        (SoundId::ShelterSegmentPairMoveIn, 5),
        (SoundId::ShelterSegmentPairCollide, 5),
        (SoundId::ShelterProtectiveCoverMoveIn, 4),
        (SoundId::ShelterProtectiveCoverClickIntoPlace, 4),
        (SoundId::ShelterBoltClose, 4),
        (SoundId::ShelterGasketsSeal, 1),
    ]);
    assert_eq!(one_shots(&mut sounds), expected);
}

/// The door's `room.ScreenMovement` calls while closing (all from `pZero`): a 0.1 shake
/// as each of the 5 segment pairs meets, a 3 px bump along `dir` for each of the 2
/// pistons going in hard, and a 0.1 shake every tick while `PumpsEnter > 0` and
/// `PumpsExit < 1` (`Closed` in 0.59..1: the 189th to the 319th increment of 0.003125).
#[test]
fn closing_moves_the_screen() {
    let mut rng = UnityRandom::new(1);
    let mut sounds = all_sounds();
    let mut d = door(1000, &mut rng);
    d.update(1000, &mut sounds, &mut rng, &mut Vec::new());
    d.close();
    let (mut shakes, mut bumps, mut pump_ticks) = (0, 0, Vec::new());
    for t in 1..=320 {
        let mut moves = Vec::new();
        d.update(1000, &mut sounds, &mut rng, &mut moves);
        let pumps = d.pumps_enter() > 0.0 && d.pumps_exit() < 1.0;
        if pumps {
            pump_ticks.push(t);
        }
        for m in &moves {
            assert_eq!(m.source, Some(Vec2::new(110.0, 90.0)));
            match (m.bump, m.shake) {
                (b, s) if b == Vec2::ZERO && s == 0.1 => shakes += 1,
                (b, s) if b == Vec2::new(0.0, 3.0) && s == 0.0 => bumps += 1,
                other => panic!("unexpected {other:?}"),
            }
        }
    }
    assert_eq!(pump_ticks.first(), Some(&189));
    assert_eq!(pump_ticks.last(), Some(&319));
    assert_eq!(shakes, 5 + 131);
    assert_eq!(bumps, 2);
}

#[test]
fn the_loops_play_while_the_door_moves() {
    let mut rng = UnityRandom::new(1);
    let mut sounds = all_sounds();
    let mut d = door(1000, &mut rng);
    d.close();
    for _ in 0..40 {
        d.update(1000, &mut sounds, &mut rng, &mut Vec::new());
    }
    // Past 0.1 (tick 33) the working loop's volume eases to 1 by 0.1 per tick.
    assert!(d.working_loop.volume > 0.0);
    assert_eq!(d.gasket_mover_loop.volume, 0.0);
    let started = sounds.take_events().iter().any(|e| {
        matches!(e, SoundEvent::Play(p)
            if p.looped && p.id == SoundId::ShelterWorkingBackgroundLoop)
    });
    assert!(started);
}

#[test]
fn shelter_range_and_hostiles() {
    use rw_assets::CreatureType;
    assert!(ShelterDoor::coord_inside_shelter_range((0, 0), false));
    assert!(ShelterDoor::coord_inside_shelter_range((20, 13), true));
    assert!(!ShelterDoor::coord_inside_shelter_range((20, 9), true));
    assert!(ShelterDoor::hostile_for_shelter(CreatureType::Spider));
    assert!(!ShelterDoor::hostile_for_shelter(CreatureType::Fly));
    assert!(!ShelterDoor::hostile_for_shelter(CreatureType::Overseer));
}

impl ShelterDoor {
    /// One turn in the room's update list: the loop emitters (which joined after the
    /// door) before the door; the `Stop`s it made.
    fn turn(&mut self, sounds: &mut Sounds, rng: &mut UnityRandom) -> Vec<EmitterId> {
        sounds.take_events();
        self.update_loop_emitters(sounds);
        self.update(1000, sounds, rng, &mut Vec::new());
        sounds
            .take_events()
            .into_iter()
            .filter_map(|e| match e {
                SoundEvent::Stop(id) => Some(id),
                _ => None,
            })
            .collect()
    }
}

/// `StaticSoundLoop.Update` at volume 0 sets `emitter.alive = false` and drops it. While
/// the door closes, each loop fades to 0 and is let go; its emitter, which updates
/// before the door, is destroyed at its next update and nothing stops before.
#[test]
fn a_silenced_static_loop_dies_at_its_next_update() {
    let mut rng = UnityRandom::new(1);
    let mut sounds = all_sounds();
    let mut d = door(1000, &mut rng);
    d.close();
    let (mut let_go, mut stopped) = (Vec::new(), Vec::new());
    for _ in 0..400 {
        let before = [d.working_loop.emitter(), d.gasket_mover_loop.emitter()];
        let stops = d.turn(&mut sounds, &mut rng);
        assert_eq!(stops, std::mem::take(&mut let_go));
        stopped.extend(stops);
        let after = [d.working_loop.emitter(), d.gasket_mover_loop.emitter()];
        let_go = before
            .into_iter()
            .zip(after)
            .filter_map(|(b, a)| b.filter(|_| a != b))
            .collect();
    }
    assert_eq!(d.closed_fac, 1.0);
    assert_eq!(d.working_loop.emitter(), None);
    assert!(
        stopped.len() >= 2,
        "both loops played and were let go: {stopped:?}"
    );
}
