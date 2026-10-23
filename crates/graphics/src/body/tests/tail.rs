//! `TailSegment`: the chain correction and a hanging tail.

use rw_sim::Vec2;

use crate::tests::{EPS, assert_close, assert_vec, room};
use crate::{TailAnchor, TailSegment};

#[test]
fn tail_chain_correction_matches_hand_derivation() {
    // Open air; anchor at x = 0, segments in a line at x = 10, 20, 30, 40, at rest.
    let r = room(&[".........."; 10]);
    let y = 100.0;
    let mut tail = [
        (6.0, 4.0, 1.0),
        (4.0, 7.0, 0.5),
        (2.5, 7.0, 0.5),
        (1.0, 7.0, 0.5),
    ]
    .map(|(rad, con, aff)| TailSegment::new(rad, con, aff));
    for (i, seg) in tail.iter_mut().enumerate() {
        seg.part.pos = Vec2::new(10.0 + 10.0 * i as f32, y);
        seg.part.last_pos = seg.part.pos;
    }
    let base = Vec2::new(0.0, y);
    for l in 0..4 {
        let (before, rest) = tail.split_at_mut(l);
        let anchor = match before.last_mut() {
            Some(prev) => TailAnchor::Segment(&mut prev.part),
            None => TailAnchor::Point(base),
        };
        rest[0].update(&r, anchor);
    }
    // Segment 0: 10 from the point, pulled fully to 4.
    // Segment 1: d = 20 - 4 = 16, correction 7 - 16 = -9 shared: s1 -> 15.5, s0 -> 8.5.
    // Segment 2: d = 30 - 15.5 = 14.5, -7.5 shared: s2 -> 26.25, s1 -> 19.25.
    // Segment 3: d = 40 - 26.25 = 13.75, -6.75 shared: s3 -> 36.625, s2 -> 29.625.
    let xs = [8.5, 19.25, 29.625, 36.625];
    for (seg, x) in tail.iter().zip(xs) {
        assert_vec(seg.part.pos, x, y, "segment");
    }
    // stretched = clamp((con / (d / 2) + 2) / 3, 0.2, 1): segment 1 had d = 16.
    assert_close(tail[1].stretched, (7.0 / 8.0 + 2.0) / 3.0, "stretched");
    assert_close(
        tail[1].stretched_rad(),
        4.0 * (7.0 / 8.0 + 2.0) / 3.0,
        "stretched rad",
    );
    // Velocity takes the same correction as the position: s3 moved -3.375.
    assert_close(tail[3].part.vel.x, -3.375, "vel");
}

#[test]
fn hanging_tail_settles_to_connection_lengths() {
    // A fixed anchor, the graphics loop's slack-tail damping (tail drag 0:
    // vel *= 0.75, vel.y -= 0.1): the chain hangs taut. The tip link is corrected
    // last each tick, so it ends at exactly its connection length; every earlier
    // link is then pulled by half of the later corrections (affectPrevious 0.5) and
    // stays slightly stretched, never slack.
    let r = room(&[".........."; 20]);
    let base = Vec2::new(100.0, 300.0);
    let mut tail = [
        (6.0, 4.0, 1.0),
        (4.0, 7.0, 0.5),
        (2.5, 7.0, 0.5),
        (1.0, 7.0, 0.5),
    ]
    .map(|(rad, con, aff)| TailSegment::new(rad, con, aff));
    for (i, seg) in tail.iter_mut().enumerate() {
        seg.part.pos = base + Vec2::new(3.0 * i as f32, 1.0);
        seg.part.last_pos = seg.part.pos;
    }
    for _ in 0..400 {
        for l in 0..4 {
            let (before, rest) = tail.split_at_mut(l);
            let anchor = match before.last_mut() {
                Some(prev) => TailAnchor::Segment(&mut prev.part),
                None => TailAnchor::Point(base),
            };
            rest[0].update(&r, anchor);
            rest[0].part.vel *= 0.75;
            rest[0].part.vel.y -= 0.1;
        }
    }
    let links = [
        (base, tail[0].part.pos, 4.0),
        (tail[0].part.pos, tail[1].part.pos, 7.0),
        (tail[1].part.pos, tail[2].part.pos, 7.0),
        (tail[2].part.pos, tail[3].part.pos, 7.0),
    ];
    for (i, (a, b, len)) in links.into_iter().enumerate() {
        let d = a.distance(b);
        assert!(d >= len - EPS && d < len + 0.5, "link {i}: {d} vs {len}");
        assert!(b.y < a.y, "link {i} hangs down");
    }
    assert!((tail[2].part.pos.distance(tail[3].part.pos) - 7.0).abs() < 1e-3);
}
