//! `Panics::guard`: a panic ends the guarded call, not the caller.

use super::Panics;

#[test]
fn guard_returns_none_after_a_panic_and_the_value_otherwise() {
    let empty: Vec<i32> = Vec::new();
    assert_eq!(Panics::guard(|| empty[0]), None);
    assert_eq!(Panics::guard(|| 3), Some(3));
}
