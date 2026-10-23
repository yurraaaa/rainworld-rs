
use super::*;

#[test]
fn forced_directions_count_per_room() {
    let mut g = PlayerGuideState::default();
    assert_eq!(g.how_many_times_has_forced_direction_been_given(7), 0);
    g.increment_times_forced_direction_has_been_given(7);
    g.increment_times_forced_direction_has_been_given(3);
    g.increment_times_forced_direction_has_been_given(7);
    assert_eq!(g.forced_directions_given, [(7, 2), (3, 1)]);
    assert_eq!(g.how_many_times_has_forced_direction_been_given(7), 2);
}

#[test]
fn properties_read_the_integers() {
    let mut g = PlayerGuideState {
        integers: [1, 4, 1, 2, 3, 0, 2],
        ..PlayerGuideState::default()
    };
    assert!(g.player_has_visited_moon());
    assert_eq!((g.super_jumps_shown(), g.pickup_objects_shown()), (4, 1));
    assert!(g.scav_trade_instruction_completed() && g.angry_with_player());
    assert!(!g.displayed_anger());
    assert_eq!(g.guide_symbol(), 2);
    g.set_angry_with_player(false);
    g.set_displayed_anger(true);
    assert_eq!(g.integers, [1, 4, 1, 2, 0, 1, 2]);
    g.item_types = vec!["DangleFruit".to_owned()];
    assert!(g.knows_item_type(AbstractObjectType::DangleFruit));
    assert!(!g.knows_item_type(AbstractObjectType::SeedCob));
}
