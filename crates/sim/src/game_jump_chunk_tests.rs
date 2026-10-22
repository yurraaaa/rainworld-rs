//! The slugcat jumping off a green lizard in one hall: the layer-1 collision's
//! `Player.Collide` notes the lizard chunk under the hips, a jump pressed within 5
//! ticks is `JumpOnChunk` (`Player.cs`), pushing that chunk down and stunning the
//! lizard 1 tick.

use crate::WorldCoordinate;
use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::Entity;
use crate::{HEAD, Input, Vec2};

const JUMP: Input = Input::new(0, 0, true);

#[test]
fn the_slugcat_resting_on_a_lizard_jumps_off_it() {
    // The player 30 px above the lizard's middle chunk; the lizard joins the room
    // after it, so it updates before the player (the update list runs newest first).
    let mut g = game(&HALL, (3, 1));
    g.set_player(Vec2::new(170.0, 59.6), Vec2::ZERO, &mut ());
    let l = g.seeing_lizard((8, 1), 1.0, 0);
    let at = WorldCoordinate::new(0, 8, 1, -1);
    g.dev_set_lizard_destination(l, at, Some(0.0));
    // Resting on the lizard after the ground's coyote time (`canJump`) ran out.
    let mut ticks = 0;
    while g
        .player()
        .is_some_and(|p| p.jump_chunk_counter() != 5 || p.can_jump() > 0)
    {
        g.tick(Input::NONE, &mut ());
        ticks += 1;
        assert!(ticks < 20, "never rested on the lizard");
    }
    let p = g.player().unwrap();
    let chunk = p.jump_chunk().expect("a jump chunk");
    assert_eq!(chunk.owner, Entity::Creature(l));
    // What `JumpOnChunk` reads: the green lizard's `TotalMass` (`bodyMass` 7.5) and a
    // chunk's third of it; heavier than the slugcat's 0.7, so `f = 1`.
    let body = g.jump_chunk_body(0).expect("in the room");
    assert!((body.total_mass - 7.5).abs() < 1e-5);
    assert!((body.mass - 2.5).abs() < 1e-5);
    // The press edge is buffered a tick; the next tick's dispatch jumps off.
    g.tick(JUMP, &mut ());
    g.tick(JUMP, &mut ());
    let p = g.player().unwrap();
    assert_eq!(p.jump_chunk_counter(), -5);
    assert_eq!(p.jump_chunk(), None);
    assert!(
        p.body.chunks[HEAD].vel.y > 3.0,
        "{}",
        p.body.chunks[HEAD].vel.y
    );
    // The lizard updated before the player this tick: `Stun(1)` is still on it.
    assert_eq!(g.world.lizard_ref(l).stun, 1);
}
