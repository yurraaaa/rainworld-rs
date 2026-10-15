//! `KarmaLadder` (537050-537967) and `KarmaSymbol` (536601-536888): phases, sounds'
//! places and random draws, with expectations from the decompile.

use super::*;

/// What the ladder updates with.
struct Env {
    stage: StageTree,
    mic: MenuMic,
    rng: UnityRandom,
    fade_circles: Vec<FadeCircle>,
}

impl Env {
    fn io(&mut self) -> LadderIo<'_> {
        LadderIo {
            stage: &mut self.stage,
            mic: &mut self.mic,
            mic_attached: false,
            rng: &mut self.rng,
            fade_circles: &mut self.fade_circles,
        }
    }

    /// The draws made so far: values from a stream seeded as this one.
    fn draws(&self) -> usize {
        let mut probe = UnityRandom::new(7);
        for n in 0..10_000 {
            if probe.state() == self.rng.state() {
                return n;
            }
            probe.value();
        }
        panic!("not reached by values");
    }
}

struct Rig {
    ladder: KarmaLadder,
    env: Env,
}

impl Rig {
    fn new(karma: (i32, i32), reinforced: bool) -> Self {
        Self::with_win_state(karma, reinforced, None)
    }

    /// A ladder with the meters of `win_state` (a `WINSTATE` value).
    fn with_win_state(karma: (i32, i32), reinforced: bool, win_state: Option<&str>) -> Self {
        let mut stage = StageTree::new();
        let containers = [0, 1, 2].map(|_| stage.add_container(StageTree::ROOT));
        let trackers = win_state.map(|w| rw_save::win_state::WinState::from_string(w).trackers);
        let translator = rw_menu::Translator::default();
        let source = trackers.as_ref().map(|trackers| MeterSource {
            trackers,
            font: None,
            translator: &translator,
        });
        let ladder = KarmaLadder::new(
            &mut stage,
            containers,
            Vec2::new(350.0, 450.0),
            karma,
            reinforced,
            1366.0,
            source.as_ref(),
        );
        Self {
            ladder,
            env: Env {
                stage,
                mic: MenuMic::default(),
                rng: UnityRandom::new(7),
                fade_circles: Vec::new(),
            },
        }
    }

    fn tick(&mut self) {
        self.ladder.update(&mut self.env.io(), Vec2::ZERO, 0.0);
    }

    /// Ticks until the ladder rests with its animation done; the phases in the order
    /// entered, with the ticks spent in each.
    fn run(&mut self) -> Vec<(LadderPhase, i32)> {
        let mut phases = vec![(self.ladder.phase, 0)];
        for _ in 0..1000 {
            self.tick();
            let last = phases.last_mut().unwrap();
            if self.ladder.phase == last.0 {
                last.1 += 1;
            } else {
                phases.push((self.ladder.phase, 1));
            }
            if self.ladder.phase == LadderPhase::Resting
                && self.ladder.all_animation_done()
                && phases.len() > 1
            {
                return phases;
            }
        }
        panic!("the ladder never came to rest: {phases:?}");
    }
}

#[test]
fn phase_spelling_round_trips() {
    for p in LadderPhase::ALL {
        assert_eq!(LadderPhase::from_value(p.value()), Some(p));
    }
    assert_eq!(LadderPhase::from_value("Ghost"), None);
}

#[test]
fn a_symbol_per_karma_up_to_the_cap_at_rest() {
    let rig = Rig::new((2, 5), false);
    let l = &rig.ladder;
    assert_eq!(l.symbols.len(), 6);
    assert_eq!(l.symbols[5].display_karma, (5, 5));
    // `moveToKarma = scroll = displayKarma.x`, flat, circle 53, resting, not started.
    assert_eq!(
        (l.move_to_karma, l.scroll, l.flat, l.circle_rad),
        (2, 2.0, 1.0, 53.0)
    );
    assert_eq!(l.phase, LadderPhase::Resting);
    assert!(!l.all_animation_done());
    // The symbol sprites: `karma0`..`karma4`, then `karma5-5`.
    let names: Vec<&str> = l
        .symbols
        .iter()
        .map(|s| rig.env.stage.sprite(s.sprites()[0]).element.as_str())
        .collect();
    assert_eq!(
        names,
        ["karma0", "karma1", "karma2", "karma3", "karma4", "karma5-5"]
    );
}

#[test]
fn a_symbol_draws_once_twice_flickering_and_thrice_when_dimmed() {
    let mut rig = Rig::new((0, 4), false);
    let mut s = rig.ladder.symbols[0].clone();
    s.update(false, 0.0, 0.0, &mut rig.env.rng);
    assert_eq!(rig.env.draws(), 1);
    // Flicker 3 counts down to 2: `InverseLerp(5, 19, 2)` is 0, never below a value.
    s.flicker_counter = 3;
    s.update(false, 0.0, 0.0, &mut rig.env.rng);
    assert_eq!(rig.env.draws(), 3);
    assert_eq!(s.flicker_counter, 2);
    // Flicker 30 counts down to 29: always dimmed, by `Pow(value, LerpMap(29, 50, 2,
    // 3, 0.2))`.
    s.flicker_counter = 30;
    s.update(false, 0.0, 0.0, &mut rig.env.rng);
    assert_eq!(rig.env.draws(), 6);
}

#[test]
fn a_resting_ladder_draws_once_for_each_symbol() {
    let mut rig = Rig::new((1, 4), false);
    rig.tick();
    assert_eq!(rig.env.draws(), 5);
    // Energy rises toward 1 by `LerpAndTick(.., 0.1, 1/70)`: 0.1 + 1/70 in one tick.
    assert!((rig.ladder.symbols[0].energy - (0.1 + 1.0 / 70.0)).abs() < 1e-6);
}

#[test]
fn deselecting_draws_once_then_twice_when_the_glow_flickers() {
    let mut rig = Rig::new((1, 4), false);
    rig.ladder.go_to_karma(2, true);
    rig.ladder.phase = LadderPhase::Deselecting;
    // `ticksInPhase` 0: `InverseLerp(0, 30, 0)` is 0, never above a value.
    rig.ladder.ticks_in_phase = 0;
    let before = rig.env.draws();
    rig.ladder.deselecting(&mut rig.env.io());
    assert_eq!(rig.env.draws() - before, 1);
    // At 30 the glow always flickers, and the ladder starts moving away from its karma.
    rig.ladder.ticks_in_phase = 30;
    let before = rig.env.draws();
    rig.ladder.deselecting(&mut rig.env.io());
    assert_eq!(rig.env.draws() - before, 2);
    assert_eq!(rig.ladder.phase, LadderPhase::MovingA);
    assert_eq!(rig.ladder.flat, 0.0);
    // `Sign(displayKarma.x - moveToKarma) * 0.01`.
    assert_eq!(rig.ladder.scroll_vel, -0.01);
}

#[test]
fn the_bump_flickers_the_other_symbols() {
    let mut rig = Rig::new((3, 5), false);
    rig.ladder.phase = LadderPhase::Bump;
    rig.ladder.ticks_in_phase = 6;
    rig.ladder.bump(&mut rig.env.io(), Vec2::ZERO);
    // Five other symbols, two draws each (`Range(12, Range(40, 80))`), and one more
    // (`Range(20, 30)`) for symbols 2 and 4 next to the selection.
    assert_eq!(rig.env.draws(), 12);
    let s = &rig.ladder.symbols;
    assert_eq!(s[3].flicker_counter, 0);
    assert!((12..80).contains(&s[0].flicker_counter));
    assert!((32..110).contains(&s[2].flicker_counter));
}

#[test]
fn sleeping_climbs_a_notch_and_bumps() {
    let mut rig = Rig::new((2, 5), false);
    rig.ladder.go_to_karma(3, true);
    assert!(rig.ladder.karma_increasing && !rig.ladder.capped_movement);
    let phases: Vec<LadderPhase> = rig.run().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        phases,
        [
            LadderPhase::Resting,
            LadderPhase::Deselecting,
            LadderPhase::MovingA,
            LadderPhase::MovingB,
            LadderPhase::Settling,
            LadderPhase::Bump,
            LadderPhase::Resting,
        ]
    );
    let l = &rig.ladder;
    assert_eq!(l.display_karma, (3, 5));
    // Resting with `displayMetersOnRest`: the meters' counter from 85.
    assert!(l.movement_shown);
    assert_eq!(l.show_end_game_meters_counter, 85);
    // The bump's fade circle at the ladder (`DrawPos(1)`).
    assert_eq!(rig.env.fade_circles.len(), 1);
    assert_eq!(rig.env.fade_circles[0].circle.pos, Vec2::new(350.0, 450.0));
}

#[test]
fn a_phase_lasts_as_its_counter_says() {
    let mut rig = Rig::new((2, 5), false);
    rig.ladder.go_to_karma(3, true);
    let phases = rig.run();
    // `ticksInPhase` starts at -1 on a new phase: Deselecting ends the tick it reaches
    // 30 (31 ticks after the tick it began), Settling at 40, Bump at 45.
    let spent: Vec<(LadderPhase, i32)> = phases
        .into_iter()
        .filter(|(p, _)| {
            matches!(
                p,
                LadderPhase::Deselecting | LadderPhase::Settling | LadderPhase::Bump
            )
        })
        .collect();
    assert_eq!(
        spent,
        [
            (LadderPhase::Deselecting, 31),
            (LadderPhase::Settling, 41),
            (LadderPhase::Bump, 46)
        ]
    );
}

#[test]
fn sleeping_at_the_cap_bumps_against_it() {
    let mut rig = Rig::new((5, 5), false);
    rig.ladder.go_to_karma(6, true);
    assert!(rig.ladder.capped_movement);
    let phases: Vec<LadderPhase> = rig.run().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        phases,
        [
            LadderPhase::Resting,
            LadderPhase::Deselecting,
            LadderPhase::CappedMovement,
            LadderPhase::Settling,
            LadderPhase::CapBump,
            LadderPhase::Resting,
        ]
    );
    assert_eq!(rig.ladder.display_karma.0, 5);
}

#[test]
fn dying_drops_a_notch() {
    let mut rig = Rig::new((2, 5), false);
    rig.ladder.go_to_karma(1, true);
    let phases: Vec<LadderPhase> = rig.run().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        phases,
        [
            LadderPhase::Resting,
            LadderPhase::Deselecting,
            LadderPhase::MovingA,
            LadderPhase::MovingB,
            LadderPhase::Settling,
            LadderPhase::Resting,
        ]
    );
    assert_eq!(rig.ladder.display_karma.0, 1);
    assert!(rig.env.fade_circles.is_empty());
}

#[test]
fn dying_at_karma_zero_hits_the_lower_cap() {
    let mut rig = Rig::new((0, 4), false);
    rig.ladder.go_to_karma(-1, true);
    let phases: Vec<LadderPhase> = rig.run().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        phases,
        [
            LadderPhase::Resting,
            LadderPhase::Deselecting,
            LadderPhase::CappedMovement,
            LadderPhase::Settling,
            LadderPhase::Resting,
        ]
    );
    assert_eq!(
        (rig.ladder.display_karma.0, rig.ladder.move_to_karma),
        (0, 0)
    );
}

#[test]
fn reinforced_karma_saves_the_notch_and_dissipates() {
    let mut rig = Rig::new((2, 5), true);
    rig.ladder.go_to_karma(1, true);
    let phases: Vec<LadderPhase> = rig.run().into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        phases,
        [
            LadderPhase::Resting,
            LadderPhase::Deselecting,
            LadderPhase::ReinforceSave,
            LadderPhase::Settling,
            LadderPhase::Resting,
        ]
    );
    let l = &rig.ladder;
    assert_eq!(l.display_karma.0, 2);
    assert!(!l.reinforced);
    assert!(l.reinforcement_death >= 1.0);
    // `FadeCircle(31.5, 16, 0.82, 40, 8)` when the reinforcement dies.
    assert_eq!(rig.env.fade_circles.len(), 1);
    assert_eq!(rig.env.fade_circles[0].life_time, 40.0);
}

#[test]
fn going_nowhere_shows_the_meters_at_once() {
    let mut rig = Rig::new((2, 5), false);
    rig.ladder.go_to_karma(2, true);
    assert!(rig.ladder.movement_shown);
    assert_eq!(rig.ladder.show_end_game_meters_counter, 125);
    assert!(rig.ladder.all_animation_done());
}

#[test]
fn meters_sit_on_two_arcs() {
    // 3 meters: one at 90 degrees, two on the second arc at 60 and 120 plus 180.
    let p = KarmaLadder::endgame_meter_positions(3);
    let expected = [
        Vec2::from_degrees(90.0) * 225.0,
        Vec2::from_degrees(240.0) * 225.0,
        Vec2::from_degrees(300.0) * 225.0,
    ];
    assert_eq!(p, expected);
    assert!((p[0].x - 225.0).abs() < 1e-3 && p[0].y.abs() < 1e-3);
    assert_eq!(KarmaLadder::endgame_meter_positions(0), []);
}

/// Survivor 3 shows (showFrom 0), Hunter 2 does not (showFrom 2), Chieftain 0.05 does.
const WIN_STATE: &str =
    "Survivor<egA>0<egA>3<wsA>Hunter<egA>0<egA>2<wsA>Chieftain<egA>0<egA>0.05<wsA>";

#[test]
fn meters_for_the_trackers_with_progress_to_show() {
    let rig = Rig::with_win_state((2, 5), false, Some(WIN_STATE));
    let ids: Vec<&str> = rig
        .ladder
        .meters
        .iter()
        .map(|m| m.tracker.id.value())
        .collect();
    assert_eq!(ids, ["Survivor", "Chieftain"]);
    let at: Vec<Vec2> = rig.ladder.meters.iter().map(|m| m.pos).collect();
    assert_eq!(at, KarmaLadder::endgame_meter_positions(2));
}

#[test]
fn the_power_on_draws_two_for_each_meter() {
    let mut rig = Rig::with_win_state((2, 5), false, Some(WIN_STATE));
    rig.ladder.show_end_game_meters_counter = 81;
    rig.ladder.update_meter_counters(&mut rig.env.io());
    assert_eq!(rig.env.draws(), 4);
    assert!(rig.ladder.meters.iter().all(|m| m.powered_on));
    assert!(
        rig.ladder
            .meters
            .iter()
            .all(|m| (12..60).contains(&m.flicker_counter))
    );
}

#[test]
fn powered_meters_draw_before_the_symbols() {
    // Resting at 2 of 5: a draw per symbol; each powered meter, unflickering, adds one.
    let mut rig = Rig::with_win_state((2, 5), false, Some(WIN_STATE));
    rig.tick();
    assert_eq!(rig.env.draws(), 6);
    for m in &mut rig.ladder.meters {
        m.powered_on = true;
    }
    rig.tick();
    assert_eq!(rig.env.draws(), 6 + 2 + 6);
}

#[test]
fn going_nowhere_animates_each_meter_in_turn() {
    let mut rig = Rig::with_win_state((2, 5), false, Some(WIN_STATE));
    rig.ladder.go_to_karma(2, true);
    // Counter 125: power-on at 80; at 1 (tick 124) the first animation starts the same
    // tick; without a change to show the next follows a tick later (else 10 + 20 / 2).
    for _ in 0..123 {
        rig.tick();
    }
    assert_eq!(rig.ladder.next_meter_to_animate, 0);
    rig.tick();
    assert_eq!(rig.ladder.next_meter_to_animate, 1);
    assert_eq!(rig.ladder.meters[0].meter_animation, 0.01);
    rig.tick();
    assert_eq!(rig.ladder.next_meter_to_animate, 2);
    assert!(!rig.ladder.all_animation_done());
    for _ in 0..400 {
        rig.tick();
    }
    assert!(rig.ladder.all_animation_done());
}

#[test]
fn circles_collision_time_of_a_line_into_a_circle() {
    // From (0, -250) straight up into a circle of 53 at the origin: hit at 197 of 250.
    let t =
        Vec2::new(0.0, -250.0).circles_collision_time(Vec2::ZERO, Vec2::new(0.0, 250.0), 0.0, 53.0);
    assert!((t * 250.0 - 197.0).abs() < 0.1, "{t}");
    // Missing it: NaN.
    let miss = Vec2::new(100.0, -250.0).circles_collision_time(
        Vec2::ZERO,
        Vec2::new(0.0, 250.0),
        0.0,
        53.0,
    );
    assert!(miss.is_nan());
}
