//! The geometry of `ClimbableVinesSystem` on small hand-worked vines. Expected values are
//! worked from `ClimbableVinesSystem.cs`'s formulas and written next to each assertion.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use super::*;

/// A vine of fixed points whose `Push` moves a point (the coral neuron's,
/// `segments[i,0] += m; segments[i,2] += m`).
#[derive(Debug, Clone)]
pub(crate) struct TestVine {
    pub id: u64,
    pub kind: VineKind,
    pub points: Vec<Vec2>,
    pub rads: Vec<f32>,
    pub masses: Vec<f32>,
    pub climbable: bool,
    /// How often `BeingClimbedOn` was called.
    pub climbed_on: Arc<AtomicI32>,
    /// `BeingClimbedOn` draws one value (a pole mimic's sound would).
    pub draws: bool,
}

impl TestVine {
    pub(crate) fn new(id: u64, points: &[(f32, f32)]) -> Self {
        let points: Vec<Vec2> = points.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
        Self {
            id,
            kind: VineKind::CoralNeuron,
            rads: vec![2.0; points.len()],
            masses: vec![1.0; points.len()],
            points,
            climbable: true,
            climbed_on: Arc::new(AtomicI32::new(0)),
            draws: false,
        }
    }
}

impl ClimbableVine for TestVine {
    fn id(&self) -> u64 {
        self.id
    }

    fn kind(&self) -> VineKind {
        self.kind
    }

    fn total_positions(&self) -> i32 {
        self.points.len() as i32
    }

    fn pos(&self, index: i32) -> Vec2 {
        self.points[index as usize]
    }

    fn rad(&self, index: i32) -> f32 {
        self.rads[index as usize]
    }

    fn mass(&self, index: i32) -> f32 {
        self.masses[index as usize]
    }

    fn push(&mut self, index: i32, movement: Vec2) {
        self.points[index as usize] += movement;
    }

    fn being_climbed_on(&mut self, _: &VineClimber, rng: &mut UnityRandom, _: &mut Sounds) {
        self.climbed_on.fetch_add(1, Ordering::Relaxed);
        if self.draws {
            rng.value();
        }
    }

    fn currently_climbable(&self) -> bool {
        self.climbable
    }

    fn draws_when_climbed(&self) -> bool {
        self.draws
    }
}

/// A room's `climbableVines` and the owners of its vines (room 0).
#[derive(Debug, Default)]
pub(crate) struct Vines {
    pub system: ClimbableVinesSystem,
    pub host: PlacedVines,
}

impl Vines {
    pub(crate) fn of(vines: Vec<TestVine>) -> Self {
        let mut v = Self::default();
        for vine in vines {
            v.system.register(Self::vine_ref(vine.id));
            v.host.add(Box::new(vine));
        }
        v
    }

    pub(crate) fn vine_ref(id: u64) -> VineRef {
        VineRef::Placed { room: 0, id }
    }
}

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{what}: expected {expected}, got {actual}"
    );
}

fn assert_vec(actual: Vec2, x: f32, y: f32, what: &str) {
    assert_close(actual.x, x, what);
    assert_close(actual.y, y, what);
}

fn at(id: u64, float_pos: f32) -> VinePosition {
    VinePosition::new(Vines::vine_ref(id), float_pos)
}

/// Segments of 100 and 200 px going up at x = 100, radii 2, 4, 6, masses 1, 2, 6.
fn upright() -> TestVine {
    let mut v = TestVine::new(1, &[(100.0, 0.0), (100.0, 100.0), (100.0, 300.0)]);
    v.rads = vec![2.0, 4.0, 6.0];
    v.masses = vec![1.0, 2.0, 6.0];
    v
}

/// `FloatAtSegment` is the length before the point over the total; `PrevSegAtFloat` the
/// segment whose running length first passes the fraction, the last point when none does
/// (a fraction of 1 or NaN).
#[test]
fn arc_length_helpers() {
    let v = Vines::of(vec![upright()]);
    let (s, h, r) = (&v.system, &v.host, Vines::vine_ref(1));
    assert_close(s.total_length(h, r), 300.0, "total");
    assert_close(s.float_at_segment(h, r, 0), 0.0, "at 0");
    assert_close(s.float_at_segment(h, r, 1), 100.0 / 300.0, "at 1");
    assert_close(s.float_at_segment(h, r, 2), 1.0, "at 2");
    assert_eq!(s.prev_seg_at_float(h, r, 0.2), 0);
    assert_eq!(s.prev_seg_at_float(h, r, 0.5), 1);
    assert_eq!(s.prev_seg_at_float(h, r, 1.0), 2);
    assert_eq!(s.prev_seg_at_float(h, r, f32::NAN), 2);
}

/// A vine whose points coincide has no length: the fractions are 0 / 0 (NaN), as in the
/// game.
#[test]
fn coincident_points_have_nan_fractions() {
    let v = Vines::of(vec![TestVine::new(1, &[(5.0, 5.0), (5.0, 5.0)])]);
    let r = Vines::vine_ref(1);
    assert_eq!(v.system.total_length(&v.host, r), 0.0);
    assert!(v.system.float_at_segment(&v.host, r, 1).is_nan());
    assert_eq!(v.system.prev_seg_at_float(&v.host, r, 0.0), 1);
}

/// Halfway (150 px) is 50 px into the second segment: `t` = 0.25 between points 1 and 2.
#[test]
fn place_on_the_vine() {
    let v = Vines::of(vec![upright()]);
    let (s, h) = (&v.system, &v.host);
    assert_vec(s.on_vine_pos(h, &at(1, 0.5)), 100.0, 150.0, "pos");
    assert_close(s.vine_rad(h, &at(1, 0.5)), 4.0 + 0.25 * 2.0, "rad");
    assert_vec(s.vine_dir(h, &at(1, 0.5)), 0.0, 1.0, "dir");
    // At the end the two points coincide: the position is the last point, the
    // direction that of the last segment.
    assert_vec(s.on_vine_pos(h, &at(1, 1.0)), 100.0, 300.0, "end pos");
    assert_close(s.vine_rad(h, &at(1, 1.0)), 6.0, "end rad");
    assert_vec(s.vine_dir(h, &at(1, 1.0)), 0.0, 1.0, "end dir");
    assert_vec(s.on_vine_pos(h, &at(1, 0.0)), 100.0, 0.0, "start pos");
}

/// `OverlappingSegment`: within the radii of the point nearest the segment's line (or its
/// end point past the end); the nearest segment wins, an unclimbable vine is skipped.
#[test]
fn overlap_finds_the_nearest_climbable_place() {
    let near = upright();
    let mut far = TestVine::new(2, &[(110.0, 0.0), (110.0, 300.0)]);
    let v = Vines::of(vec![near.clone(), far.clone()]);
    let (s, h) = (&v.system, &v.host);
    // 5 px from the second vine is not within 2 + 3; the first (4.5 + 3) takes it.
    let found = s.vine_overlap(h, Vec2::new(105.0, 150.0), 3.0).unwrap();
    assert_eq!(found.vine, Vines::vine_ref(1));
    assert_close(found.float_pos, 0.5, "float");
    // 3 px from the second, 7 from the first: the nearer wins.
    let found = s.vine_overlap(h, Vec2::new(107.0, 150.0), 3.0).unwrap();
    assert_eq!(found.vine, Vines::vine_ref(2));
    assert_close(found.float_pos, 0.5, "float");
    // Too far from both.
    assert!(s.vine_overlap(h, Vec2::new(120.0, 150.0), 3.0).is_none());
    // Past the top end only the end point's circle counts: 6 + 3 = 9 above (100, 300).
    assert!(s.vine_overlap(h, Vec2::new(100.0, 308.0), 3.0).is_some());
    assert!(s.vine_overlap(h, Vec2::new(100.0, 310.0), 3.0).is_none());
    far.climbable = false;
    let v = Vines::of(vec![near, far]);
    let found = v
        .system
        .vine_overlap(&v.host, Vec2::new(107.0, 150.0), 3.0)
        .unwrap();
    assert_eq!(found.vine, Vines::vine_ref(1));
}

/// `ClimbOnVineSpeed`: +1 at the start, -1 at the end; between, the goal along the vine
/// gives `InverseLerp(0.3, 0.5, |dot|)` with the dot's sign, nothing across it.
#[test]
fn climb_speed_follows_the_goal() {
    let v = Vines::of(vec![upright()]);
    let (s, h) = (&v.system, &v.host);
    assert_eq!(s.climb_on_vine_speed(h, &at(1, 0.0), Vec2::ZERO), 1.0);
    assert_eq!(s.climb_on_vine_speed(h, &at(1, 1.0), Vec2::ZERO), -1.0);
    // (pos(num) - pos(num2)) points down: a goal above gives a dot of +1.
    let mid = at(1, 0.5);
    assert_close(
        s.climb_on_vine_speed(h, &mid, Vec2::new(100.0, 170.0)),
        1.0,
        "up",
    );
    assert_close(
        s.climb_on_vine_speed(h, &mid, Vec2::new(100.0, 130.0)),
        -1.0,
        "down",
    );
    assert_close(
        s.climb_on_vine_speed(h, &mid, Vec2::new(130.0, 150.0)),
        0.0,
        "across",
    );
    // A dot of 0.4 is halfway between the thresholds.
    let goal = Vec2::new(100.0, 150.0) + Vec2::new(0.916_515_1, 0.4);
    assert_close(s.climb_on_vine_speed(h, &mid, goal), 0.5, "slanted");
}

/// `VineSwitch`: a goal across the vine moves to the crossing vine under the climber; a
/// goal along it stays.
#[test]
fn switch_to_a_crossing_vine() {
    let up = TestVine::new(1, &[(100.0, 0.0), (100.0, 150.0), (100.0, 300.0)]);
    let across = TestVine::new(2, &[(0.0, 150.0), (150.0, 150.0), (300.0, 150.0)]);
    let v = Vines::of(vec![up, across]);
    let (s, h) = (&v.system, &v.host);
    // Goal 30 px right: across the vine (dot 0). The climber's own vine is within 100 px of
    // itself; the horizontal vine's first segment is the candidate, 100 px of 300 along.
    let switched = s
        .vine_switch(h, &at(1, 0.5), Vec2::new(130.0, 150.0), 9.0)
        .unwrap();
    assert_eq!(switched.vine, Vines::vine_ref(2));
    assert_close(switched.float_pos, 100.0 / 300.0, "float");
    // Along the vine (dot 1 > 0.5): no switch.
    assert!(
        s.vine_switch(h, &at(1, 0.5), Vec2::new(100.0, 170.0), 9.0)
            .is_none()
    );
    // Without the crossing vine, nothing to switch to.
    let alone = Vines::of(vec![TestVine::new(
        1,
        &[(100.0, 0.0), (100.0, 150.0), (100.0, 300.0)],
    )]);
    assert!(
        alone
            .system
            .vine_switch(&alone.host, &at(1, 0.5), Vec2::new(130.0, 150.0), 9.0)
            .is_none()
    );
}

/// A part of the same vine more than 100 px along it can be switched to.
#[test]
fn switch_to_a_far_part_of_the_same_vine() {
    // A U: up the left arm, across, down the right arm; 300 px in all.
    let u = TestVine::new(
        1,
        &[(100.0, 0.0), (100.0, 100.0), (200.0, 100.0), (200.0, 0.0)],
    );
    let v = Vines::of(vec![u]);
    // 50 px along (100, 50), goal to the right, the circle (radius 100) reaching the right
    // arm at (200, 50), 250 px along. The arm's top and the crossbar are within 100 px.
    let here = at(1, 50.0 / 300.0);
    assert_vec(v.system.on_vine_pos(&v.host, &here), 100.0, 50.0, "pos");
    let switched = v
        .system
        .vine_switch(&v.host, &here, Vec2::new(130.0, 50.0), 100.0)
        .unwrap();
    assert_eq!(switched.vine, Vines::vine_ref(1));
    assert_close(switched.float_pos, 250.0 / 300.0, "float");
}

/// `ConnectChunkToVine`: a chunk 30 px from the vine with a 10 px leash and equal shares
/// moves 10 px in and the vine 10 px out, at both points of the segment.
#[test]
fn chunk_is_pulled_to_the_vine_and_the_vine_to_the_chunk() {
    let mut v = Vines::of(vec![upright()]);
    let r = Vines::vine_ref(1);
    // Vine mass there: Lerp(2, 6, 0.25) = 3. Chunk mass 3: share = 3 / 6 = 0.5.
    let mut chunk = BodyChunk::new(Vec2::new(130.0, 150.0), 9.0, 3.0);
    v.system
        .connect_chunk_to_vine(&mut v.host, &mut chunk, &at(1, 0.5), 10.0);
    assert_vec(chunk.pos, 120.0, 150.0, "chunk pos");
    assert_vec(chunk.vel, -10.0, 0.0, "chunk vel");
    assert_vec(v.host.pos(r, 1), 110.0, 100.0, "point 1");
    assert_vec(v.host.pos(r, 2), 110.0, 300.0, "point 2");
    assert_vec(v.host.pos(r, 0), 100.0, 0.0, "point 0");
    // Inside the leash nothing moves.
    let mut near = BodyChunk::new(Vec2::new(105.0, 150.0), 9.0, 3.0);
    let before = v.host.pos(r, 1);
    v.system
        .connect_chunk_to_vine(&mut v.host, &mut near, &at(1, 0.5), 10.0);
    assert_vec(near.pos, 105.0, 150.0, "near");
    assert_eq!(v.host.pos(r, 1), before);
}

/// `PushAtVine`: the push over the larger of 1 and the interpolated mass, shared by the
/// place's fraction between the two points.
#[test]
fn push_is_shared_by_fraction_and_mass() {
    let mut v = Vines::of(vec![upright()]);
    let r = Vines::vine_ref(1);
    // Mass Lerp(2, 6, 0.25) = 3: (0, 12) / 3 = (0, 4); 0.75 to point 1, 0.25 to point 2.
    v.system
        .push_at_vine(&mut v.host, &at(1, 0.5), Vec2::new(0.0, 12.0));
    assert_vec(v.host.pos(r, 1), 100.0, 103.0, "point 1");
    assert_vec(v.host.pos(r, 2), 100.0, 301.0, "point 2");
    // A light vine counts as mass 1.
    let mut light = upright();
    light.masses = vec![0.25; 3];
    let mut v = Vines::of(vec![light]);
    v.system
        .push_at_vine(&mut v.host, &at(1, 0.5), Vec2::new(4.0, 0.0));
    assert_vec(v.host.pos(r, 1), 103.0, 100.0, "light point 1");
    assert_vec(v.host.pos(r, 2), 101.0, 300.0, "light point 2");
}

/// `BeingClimbedOn` reaches the vine at once, with the climber, the random stream and the
/// sounds.
#[test]
fn being_climbed_on_reaches_the_owner() {
    let mut vine = upright();
    vine.draws = true;
    let counter = Arc::clone(&vine.climbed_on);
    let mut v = Vines::of(vec![vine]);
    let mut rng = UnityRandom::new(4);
    let climber = VineClimber {
        main_chunk_pos: Vec2::new(1.0, 2.0),
    };
    v.system.vine_being_climbed_on(
        &mut v.host,
        &at(1, 0.5),
        &climber,
        &mut rng,
        &mut Sounds::default(),
    );
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    let mut expected = UnityRandom::new(4);
    expected.value();
    assert_eq!(rng, expected);
    assert!(v.host.draws_when_climbed(Vines::vine_ref(1)));
}

/// `vines.Contains` and the registry: a vine registered is found, one removed is not; the
/// same id in another room is another vine.
#[test]
fn registry() {
    let mut v = Vines::of(vec![upright()]);
    assert!(v.system.contains(Vines::vine_ref(1)));
    assert!(!v.system.contains(Vines::vine_ref(2)));
    assert!(!v.system.contains(VineRef::Placed { room: 1, id: 1 }));
    v.system.unregister(Vines::vine_ref(1));
    assert!(!v.system.contains(Vines::vine_ref(1)));
}
