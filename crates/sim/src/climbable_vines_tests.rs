//! The geometry of `ClimbableVinesSystem` on small hand-worked vines. Expected values are
//! worked from `ClimbableVinesSystem.cs`'s formulas and written next to each assertion.

use super::*;

/// A vine of fixed points whose `Push` moves a point and its velocity (the coral
/// neuron's, `segments[i,0] += m; segments[i,2] += m`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TestVine {
    pub id: VineId,
    pub kind: VineKind,
    pub points: Vec<Vec2>,
    pub vels: Vec<Vec2>,
    pub rads: Vec<f32>,
    pub masses: Vec<f32>,
    pub climbable: bool,
    pub climbed_on: i32,
}

impl TestVine {
    pub(crate) fn new(id: u64, points: &[(f32, f32)]) -> Self {
        let points: Vec<Vec2> = points.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
        Self {
            id: VineId(id),
            kind: VineKind::CoralNeuron,
            vels: vec![Vec2::ZERO; points.len()],
            rads: vec![2.0; points.len()],
            masses: vec![1.0; points.len()],
            points,
            climbable: true,
            climbed_on: 0,
        }
    }

    pub(crate) fn boxed(self) -> Box<dyn ClimbableVine> {
        Box::new(self)
    }
}

impl ClimbableVine for TestVine {
    fn id(&self) -> VineId {
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
        self.vels[index as usize] += movement;
    }

    fn being_climbed_on(&mut self, _: &VineClimber) {
        self.climbed_on += 1;
    }

    fn currently_climbable(&self) -> bool {
        self.climbable
    }

    fn clone_box(&self) -> Box<dyn ClimbableVine> {
        Box::new(self.clone())
    }
}

pub(crate) fn system_of(vines: Vec<TestVine>) -> ClimbableVinesSystem {
    let mut s = ClimbableVinesSystem::new();
    for v in vines {
        s.register(v.boxed());
    }
    s
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
    let s = system_of(vec![upright()]);
    let v = s.vines()[0].as_ref();
    assert_close(s.total_length(v), 300.0, "total");
    assert_close(s.float_at_segment(v, 0), 0.0, "at 0");
    assert_close(s.float_at_segment(v, 1), 100.0 / 300.0, "at 1");
    assert_close(s.float_at_segment(v, 2), 1.0, "at 2");
    assert_eq!(s.prev_seg_at_float(v, 0.2), 0);
    assert_eq!(s.prev_seg_at_float(v, 0.5), 1);
    assert_eq!(s.prev_seg_at_float(v, 1.0), 2);
    assert_eq!(s.prev_seg_at_float(v, f32::NAN), 2);
}

/// A vine whose points coincide has no length: the fractions are 0 / 0 (NaN), as in the
/// game.
#[test]
fn coincident_points_have_nan_fractions() {
    let s = system_of(vec![TestVine::new(1, &[(5.0, 5.0), (5.0, 5.0)])]);
    let v = s.vines()[0].as_ref();
    assert_eq!(s.total_length(v), 0.0);
    assert!(s.float_at_segment(v, 1).is_nan());
    assert_eq!(s.prev_seg_at_float(v, 0.0), 1);
}

/// Halfway (150 px) is 50 px into the second segment: `t` = 0.25 between points 1 and 2.
#[test]
fn place_on_the_vine() {
    let s = system_of(vec![upright()]);
    let at = VinePosition::new(VineId(1), 0.5);
    assert_vec(s.on_vine_pos(&at), 100.0, 150.0, "pos");
    assert_close(s.vine_rad(&at), 4.0 + 0.25 * 2.0, "rad");
    assert_vec(s.vine_dir(&at), 0.0, 1.0, "dir");
    // At the end the two points coincide: the position is the last point, the
    // direction that of the last segment.
    let end = VinePosition::new(VineId(1), 1.0);
    assert_vec(s.on_vine_pos(&end), 100.0, 300.0, "end pos");
    assert_close(s.vine_rad(&end), 6.0, "end rad");
    assert_vec(s.vine_dir(&end), 0.0, 1.0, "end dir");
    let start = VinePosition::new(VineId(1), 0.0);
    assert_vec(s.on_vine_pos(&start), 100.0, 0.0, "start pos");
}

/// `OverlappingSegment`: within the radii of the point nearest the segment's line (or its
/// end point past the end); the nearest segment wins, an unclimbable vine is skipped.
#[test]
fn overlap_finds_the_nearest_climbable_place() {
    let near = upright();
    let mut far = TestVine::new(2, &[(110.0, 0.0), (110.0, 300.0)]);
    let s = system_of(vec![near.clone(), far.clone()]);
    // 5 px from the second vine is not within 2 + 3; the first (4.5 + 3) takes it.
    let found = s.vine_overlap(Vec2::new(105.0, 150.0), 3.0).unwrap();
    assert_eq!(found.vine, VineId(1));
    assert_close(found.float_pos, 0.5, "float");
    // 3 px from the second, 7 from the first: the nearer wins.
    let found = s.vine_overlap(Vec2::new(107.0, 150.0), 3.0).unwrap();
    assert_eq!(found.vine, VineId(2));
    assert_close(found.float_pos, 0.5, "float");
    // Too far from both.
    assert!(s.vine_overlap(Vec2::new(120.0, 150.0), 3.0).is_none());
    // Past the top end only the end point's circle counts: 6 + 3 = 9 above (100, 300).
    assert!(s.vine_overlap(Vec2::new(100.0, 308.0), 3.0).is_some());
    assert!(s.vine_overlap(Vec2::new(100.0, 310.0), 3.0).is_none());
    far.climbable = false;
    let s = system_of(vec![near, far]);
    let found = s.vine_overlap(Vec2::new(107.0, 150.0), 3.0).unwrap();
    assert_eq!(found.vine, VineId(1));
}

/// `ClimbOnVineSpeed`: +1 at the start, -1 at the end; between, the goal along the vine
/// gives `InverseLerp(0.3, 0.5, |dot|)` with the dot's sign, nothing across it.
#[test]
fn climb_speed_follows_the_goal() {
    let s = system_of(vec![upright()]);
    let at = VinePosition::new(VineId(1), 0.5);
    assert_eq!(
        s.climb_on_vine_speed(&VinePosition::new(VineId(1), 0.0), Vec2::ZERO),
        1.0
    );
    assert_eq!(
        s.climb_on_vine_speed(&VinePosition::new(VineId(1), 1.0), Vec2::ZERO),
        -1.0
    );
    // (pos(num) - pos(num2)) points down: a goal above gives a dot of +1.
    assert_close(
        s.climb_on_vine_speed(&at, Vec2::new(100.0, 170.0)),
        1.0,
        "up",
    );
    assert_close(
        s.climb_on_vine_speed(&at, Vec2::new(100.0, 130.0)),
        -1.0,
        "down",
    );
    assert_close(
        s.climb_on_vine_speed(&at, Vec2::new(130.0, 150.0)),
        0.0,
        "across",
    );
    // A dot of 0.4 is halfway between the thresholds.
    let goal = Vec2::new(100.0, 150.0) + Vec2::new(0.916_515_1, 0.4);
    assert_close(s.climb_on_vine_speed(&at, goal), 0.5, "slanted");
}

/// `VineSwitch`: a goal across the vine moves to the crossing vine under the climber; a
/// goal along it stays.
#[test]
fn switch_to_a_crossing_vine() {
    let up = TestVine::new(1, &[(100.0, 0.0), (100.0, 150.0), (100.0, 300.0)]);
    let across = TestVine::new(2, &[(0.0, 150.0), (150.0, 150.0), (300.0, 150.0)]);
    let s = system_of(vec![up, across]);
    let at = VinePosition::new(VineId(1), 0.5);
    // Goal 30 px right: across the vine (dot 0). The climber's own vine is within 100 px of
    // itself; the horizontal vine's first segment is the candidate, 100 px of 300 along.
    let switched = s.vine_switch(&at, Vec2::new(130.0, 150.0), 9.0).unwrap();
    assert_eq!(switched.vine, VineId(2));
    assert_close(switched.float_pos, 100.0 / 300.0, "float");
    // Along the vine (dot 1 > 0.5): no switch.
    assert!(s.vine_switch(&at, Vec2::new(100.0, 170.0), 9.0).is_none());
    // Without the crossing vine, nothing to switch to.
    let alone = system_of(vec![TestVine::new(
        1,
        &[(100.0, 0.0), (100.0, 150.0), (100.0, 300.0)],
    )]);
    assert!(
        alone
            .vine_switch(&at, Vec2::new(130.0, 150.0), 9.0)
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
    let s = system_of(vec![u]);
    // 50 px along (100, 50), goal to the right, the circle (radius 100) reaching the right
    // arm at (200, 50), 250 px along. The arm's top and the crossbar are within 100 px.
    let at = VinePosition::new(VineId(1), 50.0 / 300.0);
    assert_vec(s.on_vine_pos(&at), 100.0, 50.0, "pos");
    let switched = s.vine_switch(&at, Vec2::new(130.0, 50.0), 100.0).unwrap();
    assert_eq!(switched.vine, VineId(1));
    assert_close(switched.float_pos, 250.0 / 300.0, "float");
}

/// `ConnectChunkToVine`: a chunk 30 px from the vine with a 10 px leash and equal shares
/// moves 10 px in and the vine 10 px out, at both points of the segment.
#[test]
fn chunk_is_pulled_to_the_vine_and_the_vine_to_the_chunk() {
    let mut s = system_of(vec![upright()]);
    let at = VinePosition::new(VineId(1), 0.5);
    // Vine mass there: Lerp(2, 6, 0.25) = 3. Chunk mass 3: share = 3 / 6 = 0.5.
    let mut chunk = BodyChunk::new(Vec2::new(130.0, 150.0), 9.0, 3.0);
    s.connect_chunk_to_vine(&mut chunk, &at, 10.0);
    assert_vec(chunk.pos, 120.0, 150.0, "chunk pos");
    assert_vec(chunk.vel, -10.0, 0.0, "chunk vel");
    let pushed = s.take_events();
    assert_eq!(pushed.len(), 2);
    for (event, index) in pushed.iter().zip([1, 2]) {
        assert_eq!(
            *event,
            VineEvent::Push {
                vine: VineId(1),
                index,
                movement: Vec2::new(10.0, 0.0),
            }
        );
    }
    // Inside the leash nothing moves.
    let mut near = BodyChunk::new(Vec2::new(105.0, 150.0), 9.0, 3.0);
    s.connect_chunk_to_vine(&mut near, &at, 10.0);
    assert_vec(near.pos, 105.0, 150.0, "near");
    assert!(s.take_events().is_empty());
}

/// `PushAtVine`: the push over the larger of 1 and the interpolated mass, shared by the
/// place's fraction between the two points.
#[test]
fn push_is_shared_by_fraction_and_mass() {
    let mut s = system_of(vec![upright()]);
    // Mass Lerp(2, 6, 0.25) = 3: (0, 12) / 3 = (0, 4); 0.75 to point 1, 0.25 to point 2.
    s.push_at_vine(&VinePosition::new(VineId(1), 0.5), Vec2::new(0.0, 12.0));
    let v = s.vines()[0].as_ref();
    assert_vec(v.pos(1), 100.0, 103.0, "point 1");
    assert_vec(v.pos(2), 100.0, 301.0, "point 2");
    // A light vine counts as mass 1.
    let mut light = upright();
    light.masses = vec![0.25; 3];
    let mut s = system_of(vec![light]);
    s.push_at_vine(&VinePosition::new(VineId(1), 0.5), Vec2::new(4.0, 0.0));
    let v = s.vines()[0].as_ref();
    assert_vec(v.pos(1), 103.0, 100.0, "light point 1");
    assert_vec(v.pos(2), 101.0, 300.0, "light point 2");
}

/// `BeingClimbedOn` reaches the vine and is logged for its owner.
#[test]
fn being_climbed_on_is_forwarded() {
    let mut s = system_of(vec![upright()]);
    let climber = VineClimber {
        main_chunk_pos: Vec2::new(1.0, 2.0),
    };
    s.vine_being_climbed_on(&VinePosition::new(VineId(1), 0.5), climber);
    assert_eq!(
        s.take_events(),
        [VineEvent::BeingClimbedOn {
            vine: VineId(1),
            climber
        }]
    );
}

/// `vines.Contains` and the registry: a vine registered is found, one removed is not.
#[test]
fn registry() {
    let mut s = system_of(vec![upright()]);
    assert!(s.contains(VineId(1)));
    assert!(!s.contains(VineId(2)));
    s.unregister(VineId(1));
    assert!(!s.contains(VineId(1)));
}
