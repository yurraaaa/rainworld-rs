//! `get_state`'s `climbable_vines` and the player's vine fields, with a vine of the test's
//! own put into SU_A07 of the installed game (skipped without it): no base-game vine
//! object exists in the port yet (the coral neurons and stems come with the next steps).

use rw_sim::{ClimbableVine, Vec2, VineClimber, VineId, VineKind};

use super::Session;
use super::params::{InputParam, StepParams};

/// A straight vertical vine of heavy points, 40 px between them.
#[derive(Debug, Clone)]
struct TestVine {
    points: Vec<Vec2>,
}

impl ClimbableVine for TestVine {
    fn id(&self) -> VineId {
        VineId(7)
    }

    fn kind(&self) -> VineKind {
        VineKind::CoralStem
    }

    fn total_positions(&self) -> i32 {
        self.points.len() as i32
    }

    fn pos(&self, index: i32) -> Vec2 {
        self.points[index as usize]
    }

    fn rad(&self, _: i32) -> f32 {
        4.0
    }

    fn mass(&self, _: i32) -> f32 {
        1.0e6
    }

    fn push(&mut self, index: i32, movement: Vec2) {
        self.points[index as usize] += movement;
    }

    fn being_climbed_on(&mut self, _: &VineClimber) {}

    fn currently_climbable(&self) -> bool {
        true
    }

    fn clone_box(&self) -> Box<dyn ClimbableVine> {
        Box::new(self.clone())
    }
}

fn up() -> StepParams {
    StepParams {
        ticks: Some(2),
        input: Some(InputParam {
            y: 1,
            ..Default::default()
        }),
    }
}

/// A vine through the head, an up press: the player grabs it (animation `VineGrab`, the
/// vine's place and the cursor reported), and the room's vines list it.
#[test]
fn a_registered_vine_is_reported_and_grabbed() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.load("SU_A07", 1);
    s.step(&StepParams {
        ticks: Some(40),
        input: Some(InputParam::default()),
    })
    .expect("steps");
    let before = s.state().expect("state");
    assert!(before.climbable_vines.is_empty());
    let head = before.player.as_ref().expect("player").chunks[0].pos;
    let points = (0..5)
        .map(|i| Vec2::new(head[0], head[1] - 80.0 + 40.0 * i as f32))
        .collect();
    let game = s.scene_mut().expect("game").game_mut().sim_mut();
    let room = game.camera_room();
    game.world_mut()
        .realized_mut(room)
        .expect("realized")
        .register_climbable_vine(Box::new(TestVine { points }));

    let state = s.step(&up()).expect("steps");
    let vines = &state.climbable_vines;
    assert_eq!(vines.len(), 1);
    assert_eq!(
        (vines[0].vine_count, vines[0].vines[0].kind),
        (1, "CoralStem")
    );
    assert_eq!(vines[0].vines[0].points.len(), 5);
    assert!((vines[0].vines[0].total_length - 160.0).abs() < 1.0);
    let player = state.player.expect("player");
    assert_eq!(player.animation, "VineGrab");
    let on = player.vine_pos.expect("vine_pos");
    assert_eq!(on.vine, 7);
    assert!(on.float_pos > 0.0 && on.float_pos < 1.0, "{}", on.float_pos);
    assert_eq!(player.body_mode, "Default");
    assert!(player.vine_climb_cursor[1] > 0.0);
}
