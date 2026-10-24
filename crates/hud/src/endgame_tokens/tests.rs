//! `EndgameTokens`: which passages get a token, where, and when the
//! PASSAGE button is due, with expectations from the decompile.

use rw_menu::Translator;
use rw_save::win_state::{EndgameId, EndgameTracker, Progress};

use super::*;
use crate::MeterSource;

fn meters(stage: &mut StageTree, trackers: &[EndgameTracker]) -> Vec<EndgameMeter> {
    let translator = Translator::default();
    let source = MeterSource {
        trackers,
        font: None,
        translator: &translator,
    };
    let c = stage.add_container(StageTree::ROOT);
    trackers
        .iter()
        .map(|t| EndgameMeter::new(stage, [c, c], Vec2::ZERO, t, &source))
        .collect()
}

fn integer(id: EndgameId, progress: i32, last_shown: i32, consumed: bool) -> EndgameTracker {
    EndgameTracker {
        id,
        consumed,
        progress: Progress::Integer {
            progress,
            last_shown,
        },
    }
}

#[test]
fn a_token_for_each_fulfilled_unused_passage() {
    let mut stage = StageTree::new();
    let meters = meters(
        &mut stage,
        &[
            integer(EndgameId::Survivor, 5, 5, false),
            integer(EndgameId::Hunter, 12, 12, true),
            integer(EndgameId::Saint, 8, 6, false),
            integer(EndgameId::Monk, 12, 10, false),
        ],
    );
    let (tokens, add_now) = EndgameTokens::new(&mut stage, StageTree::ROOT, Vec2::ZERO, &meters);
    // The used Hunter and the unfulfilled Saint get none.
    let ids: Vec<usize> = tokens.tokens.iter().map(|t| t.meter).collect();
    assert_eq!(ids, [0, 3]);
    // The Survivor was fulfilled before: PASSAGE at once; the Monk now: the watch.
    assert!(add_now);
    assert!(tokens.force_show_token_add);
    assert!(!tokens.add_passage_button_when_token_becomes_visible);
    // Slots: the shown Survivor first, the Monk (not shown yet) after it.
    assert_eq!(tokens.tokens[0].pos, Vec2::new(20.0, 15.0));
    assert_eq!(tokens.tokens[1].pos, Vec2::new(60.0, 15.0));
}

#[test]
fn passage_waits_for_the_token_of_a_passage_fulfilled_now() {
    let mut stage = StageTree::new();
    let mut meters = meters(&mut stage, &[integer(EndgameId::Monk, 12, 10, false)]);
    let (mut tokens, add_now) =
        EndgameTokens::new(&mut stage, StageTree::ROOT, Vec2::ZERO, &meters);
    assert!(!add_now);
    assert!(!tokens.update(&meters));
    assert_eq!(tokens.tokens[0].fade, 0.0);
    meters[0].show_as_fulfilled = 0.95;
    assert!(tokens.update(&meters));
    // Asked once only.
    assert!(!tokens.update(&meters));
    // The last token fades towards its meter's `showAsFullfilled`.
    assert!(tokens.tokens[0].fade > 0.0);
    assert!(!tokens.all_animations_done());
    for _ in 0..60 {
        tokens.update(&meters);
    }
    assert!(tokens.all_animations_done());
}

#[test]
fn each_token_draws_once_a_frame() {
    let mut stage = StageTree::new();
    let meters = meters(
        &mut stage,
        &[
            integer(EndgameId::Survivor, 5, 5, false),
            integer(EndgameId::Monk, 12, 12, false),
        ],
    );
    let (tokens, _) = EndgameTokens::new(&mut stage, StageTree::ROOT, Vec2::ZERO, &meters);
    let mut cosmetic = UnityRandom::new(3);
    tokens.draw(&mut stage, Vec2::ZERO, 1.0, &meters, &mut cosmetic);
    let mut probe = UnityRandom::new(3);
    probe.value();
    probe.value();
    assert_eq!(probe.state(), cosmetic.state());
}
