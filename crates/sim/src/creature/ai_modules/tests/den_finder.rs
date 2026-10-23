use super::*;

/// A search the game takes three updates for finishes on the third.
#[test]
fn search_finishes_after_its_updates() {
    let mut f = DenFinder::new(false);
    f.search = Some(DenSearch {
        path: Some(vec![WorldCoordinate::new(3, 4, 5, 2)]),
        updates_left: 3,
    });
    assert_eq!(f.step_search(), None);
    assert_eq!(f.step_search(), None);
    let path = f.step_search().expect("finished");
    assert!(f.search.is_none());
    assert_eq!(
        f.finished(path.as_deref()),
        Some(WorldCoordinate::new(3, -1, -1, 2))
    );
    assert_eq!(f.status, NodeFinderStatus::Found);
    assert_eq!(f.finished(None), None);
    assert_eq!(f.status, NodeFinderStatus::NoAccessible);
}
