//! `EndgameMeter` (530838-531649): its kinds, fills, draws, sounds and geometry, with
//! expectations from the decompile.

use rw_menu::{MenuMic, MenuSoundEvent};
use rw_random::UnityRandom;
use rw_save::win_state::WinState;

use super::*;
use crate::FadeCircle;

struct Rig {
    stage: StageTree,
    containers: [NodeId; 2],
    mic: MenuMic,
    rng: UnityRandom,
    fade_circles: Vec<FadeCircle>,
}

impl Rig {
    fn new() -> Self {
        let text = "Volume: 6\nVolume exponent: 1.8\n\
            MENU_Endgame_Meter_Fullfilled : clip\n\
            MENU_Endgame_Notch_Meter_Slot_Fill : clip\n";
        let names = rw_assets::ClipNames::new(["clip".to_owned()], None);
        let table = Arc::new(rw_assets::SoundTable::parse(text, &names).unwrap());
        let mut stage = StageTree::new();
        let containers = [0, 1].map(|_| stage.add_container(StageTree::ROOT));
        Self {
            stage,
            containers,
            mic: MenuMic::new(table),
            rng: UnityRandom::new(7),
            fade_circles: Vec::new(),
        }
    }

    fn meter(&mut self, tracker: &EndgameTracker) -> EndgameMeter {
        let translator = Translator::default();
        let source = MeterSource {
            trackers: std::slice::from_ref(tracker),
            font: None,
            translator: &translator,
        };
        EndgameMeter::new(
            &mut self.stage,
            self.containers,
            Vec2::new(225.0, 0.0),
            tracker,
            &source,
        )
    }

    fn io(&mut self) -> LadderIo<'_> {
        LadderIo {
            stage: &mut self.stage,
            mic: &mut self.mic,
            mic_attached: true,
            rng: &mut self.rng,
            fade_circles: &mut self.fade_circles,
        }
    }

    /// The draws made so far on a stream seeded as this one.
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

    fn sounds(&mut self) -> Vec<MenuSoundId> {
        self.mic
            .take_events()
            .into_iter()
            .filter_map(|e| match e {
                MenuSoundEvent::Play(s) => Some(s.id),
                _ => None,
            })
            .collect()
    }
}

fn integer(id: EndgameId, progress: i32, last_shown: i32) -> EndgameTracker {
    EndgameTracker {
        id,
        consumed: false,
        progress: Progress::Integer {
            progress,
            last_shown,
        },
    }
}

#[test]
fn meter_kinds_follow_the_trackers() {
    let w = WinState::from_string(
        "Survivor<egA>0<egA>3<wsA>Hunter<egA>0<egA>6<wsA>\
         Traveller<egA>0<egA>1.0.0.0.0.0.0.0.0.0.0.0.<wsA>\
         DragonSlayer<egA>0<egA>1.1.0.0.0.0.<wsA>Scholar<egA>0<egA>2.7<wsA>\
         Chieftain<egA>0<egA>0.5<wsA>",
    );
    let mut rig = Rig::new();
    let kinds: Vec<Option<usize>> = w
        .trackers
        .iter()
        .map(|t| match rig.meter(t).meter.kind {
            MeterKind::Notch(n) => Some(n.notches.len()),
            MeterKind::Float(_) => None,
        })
        .collect();
    // The Survivor always notched (5 from showFrom 0); the bool arrays and the list
    // under 15 notched; the other integer and float trackers float.
    assert_eq!(kinds, [Some(5), None, Some(12), Some(6), Some(3), None]);
    let dragon = rig.meter(&w.trackers[3]);
    let MeterKind::Notch(n) = &dragon.meter.kind else {
        panic!()
    };
    // `lizardsOrder`'s standard colours, filled or not (Green first, Black last).
    let colors = n.custom_colors.as_ref().unwrap();
    assert_eq!(colors[0], StageColor::rgb(0.2, 1.0, 0.0));
    assert_eq!(colors[5], StageColor::rgb(0.1, 0.1, 0.1));
    let scholar = rig.meter(&w.trackers[4]);
    let MeterKind::Notch(n) = &scholar.meter.kind else {
        panic!()
    };
    // Pearls 2 (CC) and 7 (HI); the third slot stays `default(Color)`.
    let colors = n.custom_colors.as_ref().unwrap();
    assert_eq!(colors[0], PearlColor::Cc.notch_color());
    assert_eq!(colors[1], PearlColor::Hi.notch_color());
    assert_eq!(colors[2].a, 0.0);
}

#[test]
fn a_scholar_pearl_colour_is_saturated() {
    // CC: (0.9, 0.6, 0.1) 40% to (1, 1, 0) is (0.94, 0.76, 0.06); `Saturate(0.2)` moves
    // it a fifth of the way to (1, 0.795, 0).
    let c = PearlColor::Cc.notch_color();
    let expected = [0.952, 0.767_09, 0.048];
    for (got, want) in [c.r, c.g, c.b].into_iter().zip(expected) {
        assert!((got - want).abs() < 1e-4, "{c:?}");
    }
    assert_eq!(PearlColor::of_index(99), PearlColor::Misc);
    assert_eq!(PearlColor::of_index(19), PearlColor::RedStomach);
}

#[test]
fn float_fills_are_fractions_above_show_from() {
    let mut rig = Rig::new();
    // Hunter: showFrom 2, max 12; 4 -> 6 is 0.2 -> 0.4, a gain.
    let m = rig.meter(&integer(EndgameId::Hunter, 6, 4));
    let MeterKind::Float(f) = &m.meter.kind else {
        panic!()
    };
    assert!((f.last_fill - 0.2).abs() < 1e-6 && (f.fill - 0.4).abs() < 1e-6);
    assert_eq!(f.change_direction, 1);
    assert_eq!(
        m.change_sound(),
        Some(MenuSoundId::EndgameFloatMeterStartAnimationUp)
    );
    assert!(m.any_change_to_show() && m.show_positive_change);
    // Chieftain 0.5 -> 0.3: a loss; its first bar goes behind.
    let t = EndgameTracker {
        id: EndgameId::Chieftain,
        consumed: false,
        progress: Progress::Float {
            progress: 0.3,
            last_shown: 0.5,
            text: None,
        },
    };
    let m = rig.meter(&t);
    let MeterKind::Float(f) = &m.meter.kind else {
        panic!()
    };
    assert_eq!(f.change_direction, -1);
    assert!(!m.show_positive_change);
    // A list tracker shows no change without Remix.
    let scholar = WinState::from_string("Scholar<egA>0<egA>2<wsA>");
    assert!(!rig.meter(&scholar.trackers[0]).any_change_to_show());
}

#[test]
fn a_meter_draws_once_twice_flickering_and_thrice_when_dimmed() {
    let mut rig = Rig::new();
    let mut m = rig.meter(&integer(EndgameId::Hunter, 6, 4));
    // Not powered: no draws, no energy.
    m.update(&mut rig.io());
    assert_eq!((rig.draws(), m.energy), (0, 0.0));
    m.powered_on = true;
    m.update(&mut rig.io());
    assert_eq!(rig.draws(), 1);
    // Counter 3 -> 2: `InverseLerp(5, 19, 2)` is 0, so never dimmed.
    m.flicker_counter = 3;
    m.update(&mut rig.io());
    assert_eq!(rig.draws(), 3);
    // Counter 30 -> 29: always dimmed.
    m.flicker_counter = 30;
    m.update(&mut rig.io());
    assert_eq!(rig.draws(), 6);
}

#[test]
fn a_passage_fulfilled_now_lights_up_when_its_meter_is_full() {
    let mut rig = Rig::new();
    let mut m = rig.meter(&integer(EndgameId::Survivor, 5, 4));
    assert!(m.fulfilled_now);
    assert_eq!(m.show_as_fulfilled, 0.0);
    assert_eq!(rig.stage.sprite(m.symbol).element, "SurvivorB");
    m.powered_on = true;
    m.energy = 1.0;
    m.meter_animation = 0.01;
    // 0.01 + n / 62 reaches 1 on the 62nd tick.
    let mut full_at = None;
    for tick in 1..=90 {
        m.update(&mut rig.io());
        let sounds = rig.sounds();
        if !sounds.is_empty() {
            assert_eq!(full_at, None);
            full_at = Some(tick);
            // The fulfilment, then the fifth notch filling as the animation passes it.
            assert_eq!(
                sounds,
                [
                    MenuSoundId::EndgameMeterFullfilled,
                    MenuSoundId::EndgameNotchMeterSlotFill
                ]
            );
        }
    }
    assert_eq!(full_at, Some(62));
    assert_eq!(rig.stage.sprite(m.symbol).element, "SurvivorA");
    assert!(m.show_as_fulfilled >= 0.999);
    let MeterKind::Notch(n) = &m.meter.kind else {
        panic!()
    };
    // Only the newly filled notch popped.
    assert_eq!(n.pop[4][0], 1.0);
    assert!(n.pop[..4].iter().all(|p| p[0] == 0.0));
}

#[test]
fn a_shown_fulfilled_passage_starts_lit() {
    let mut rig = Rig::new();
    let m = rig.meter(&integer(EndgameId::Survivor, 5, 5));
    assert!(!m.fulfilled_now);
    assert_eq!(m.show_as_fulfilled, 1.0);
    assert_eq!(rig.stage.sprite(m.symbol).element, "SurvivorA");
}

#[test]
fn the_stick_runs_from_the_ladder_towards_the_symbol() {
    let mut rig = Rig::new();
    let mut m = rig.meter(&integer(EndgameId::Survivor, 3, 3));
    m.meter.extended = 1.0;
    m.meter.last_extended = 1.0;
    let frame = MeterFrame {
        ladder: Vec2::new(500.0, 450.0),
        t: 1.0,
        map_fade: None,
    };
    m.draw(&mut rig.stage, &frame);
    // The symbol at (725, 450); the tip 35 px towards the ladder, the start where the
    // line crosses x = 550.
    let stick = rig.stage.sprite(m.meter.stick);
    assert_eq!((stick.transform.x, stick.transform.y), (550.0, 450.0));
    assert!((stick.transform.scale_y - 140.0).abs() < 1e-3);
    assert!((stick.transform.rotation - 90.0).abs() < 1e-3);
    let MeterKind::Notch(n) = &m.meter.kind else {
        panic!()
    };
    // Notches at a fifth of the stick each, floored plus a half.
    let xs: Vec<f32> = n
        .notches
        .iter()
        .map(|s| rig.stage.sprite(s[0]).transform.x)
        .collect();
    assert_eq!(xs, [578.5, 606.5, 634.5, 662.5, 690.5]);
    // Three filled before and now: their rings show, the rest hidden.
    let shown: Vec<bool> = n
        .notches
        .iter()
        .map(|s| rig.stage.sprite(s[2]).transform.visible)
        .collect();
    assert_eq!(shown, [true, true, true, false, false]);
}
