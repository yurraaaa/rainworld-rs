//! `Player.Collide`'s `jumpChunk` tail and `JumpOnChunk`. The slugcat's `TotalMass` is
//! 0.7 (`0.7 * bodyWeightFac`, two chunks of 0.35).

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};
use rw_random::UnityRandom;

use super::{JUMP, flat_room};
use crate::creature::CreatureId;
use crate::item::Entity;
use crate::player::{JumpChunk, JumpChunkBody, JumpChunkPush};
use crate::{BodyChunk, BodyMode, HEAD, HIPS, Input, Player, SoundId, Sounds, Vec2};

const OTHER: JumpChunk = JumpChunk {
    owner: Entity::Creature(CreatureId(7)),
    index: 1,
};

/// A chunk of radius 10 at `pos`.
fn other_chunk(pos: Vec2) -> BodyChunk {
    BodyChunk::new(pos, 10.0, 2.5)
}

/// A `sounds.txt` with one line, for `id`.
fn sounds_with(id: SoundId) -> Sounds {
    let text = format!("Volume: 6\nVolume exponent: 1.8\n{} : clip\n", id.name());
    let names = ClipNames::new(["clip".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(&text, &names).unwrap()))
}

impl Player {
    /// Airborne in `Default` mode, hips at `hips`, head 17 px above.
    fn default_mode_at(hips: Vec2) -> Self {
        let mut p = Self::new();
        p.body.chunks[HIPS].reset_at(hips);
        p.body.chunks[HEAD].reset_at(hips + Vec2::new(0.0, 17.0));
        p.body_mode = BodyMode::Default;
        p
    }

    /// Ready to jump off `OTHER` with the given owner and chunk masses.
    fn on_chunk(total_mass: f32, mass: f32) -> Self {
        let mut p = Self::default_mode_at(Vec2::new(100.0, 100.0));
        p.jump_chunk = Some(OTHER);
        p.jump_chunk_counter = 5;
        p.jump_chunk_body = Some(JumpChunkBody { total_mass, mass });
        p
    }
}

#[test]
fn hips_above_the_lower_half_of_a_chunk_mark_it_for_five_ticks() {
    // `bodyChunks[1].pos.y > other.pos.y - other.rad / 2`: 100 - 5 = 95.
    let mut p = Player::default_mode_at(Vec2::new(100.0, 95.01));
    p.collide_jump_chunk(HIPS, OTHER, &other_chunk(Vec2::new(100.0, 100.0)));
    assert_eq!(p.jump_chunk_counter(), 5);
    assert_eq!(p.jump_chunk(), Some(OTHER));
}

#[test]
fn the_jump_chunk_needs_the_hips_default_mode_height_and_no_negative_counter() {
    let chunk = other_chunk(Vec2::new(100.0, 100.0));
    // Exactly at the threshold: not above it.
    let mut p = Player::default_mode_at(Vec2::new(100.0, 95.0));
    p.collide_jump_chunk(HIPS, OTHER, &chunk);
    assert_eq!((p.jump_chunk_counter(), p.jump_chunk()), (0, None));
    // The head's contact (`myChunk == 0`).
    let mut p = Player::default_mode_at(Vec2::new(100.0, 120.0));
    p.collide_jump_chunk(HEAD, OTHER, &chunk);
    assert_eq!((p.jump_chunk_counter(), p.jump_chunk()), (0, None));
    // Another body mode.
    let mut p = Player::default_mode_at(Vec2::new(100.0, 120.0));
    p.body_mode = BodyMode::Crawl;
    p.collide_jump_chunk(HIPS, OTHER, &chunk);
    assert_eq!((p.jump_chunk_counter(), p.jump_chunk()), (0, None));
    // Just after a jump off a chunk (`jumpChunkCounter = -5`).
    let mut p = Player::default_mode_at(Vec2::new(100.0, 120.0));
    p.jump_chunk_counter = -5;
    p.collide_jump_chunk(HIPS, OTHER, &chunk);
    assert_eq!((p.jump_chunk_counter(), p.jump_chunk()), (-5, None));
}

#[test]
fn jump_on_a_heavier_creature_keeps_a_tenth_of_vel_y_and_adds_4_and_3() {
    // `f = Clamp(7.5 / 0.7, 0, 1)^0.7 = 1`; `vel.y *= 0.1`, head `+= 4`, hips `+= 3`;
    // the chunk's `vel.y -= 7 * 1 * 0.7 / 2.5 = 1.96`.
    let mut p = Player::on_chunk(7.5, 2.5);
    p.body.chunks[HEAD].vel.y = -2.0;
    p.body.chunks[HIPS].vel.y = -3.0;
    p.jump_on_chunk();
    assert!((p.body.chunks[HEAD].vel.y - 3.8).abs() < 1e-5);
    assert!((p.body.chunks[HIPS].vel.y - 2.7).abs() < 1e-5);
    assert_eq!(p.jump_boost, 7.0);
    assert_eq!(p.jump_chunk(), None);
    let push = p.take_jump_chunk_push().unwrap();
    assert_eq!(push.chunk, OTHER);
    assert!((push.vel_y - 1.96).abs() < 1e-5);
}

#[test]
fn jump_on_a_lighter_object_scales_by_its_mass_ratio_to_the_0_7() {
    // `f = (0.35 / 0.7)^0.7 = 0.615_572_2`: `vel.y *= 1 - 0.9 f = 0.445_985`,
    // head `+= 4 f = 2.462_289`, hips `+= 3 f = 1.846_717`;
    // the chunk's `vel.y -= 7 f 0.7 / 0.1 = 30.163_04`.
    let mut p = Player::on_chunk(0.35, 0.1);
    p.body.chunks[HEAD].vel.y = 2.0;
    p.body.chunks[HIPS].vel.y = 0.0;
    p.jump_on_chunk();
    assert!((p.body.chunks[HEAD].vel.y - (2.0 * 0.445_985 + 2.462_289)).abs() < 1e-4);
    assert!((p.body.chunks[HIPS].vel.y - 1.846_717).abs() < 1e-4);
    let JumpChunkPush { vel_y, .. } = p.take_jump_chunk_push().unwrap();
    assert!((vel_y - 30.163_04).abs() < 1e-3, "{vel_y}");
}

#[test]
fn jump_on_a_chunk_out_of_the_room_does_nothing_but_keeps_the_chunk() {
    // `jumpChunk.owner.room != room`: return before anything.
    let mut p = Player::on_chunk(7.5, 2.5);
    p.jump_chunk_body = None;
    p.body.chunks[HEAD].vel.y = -2.0;
    p.jump_on_chunk();
    assert_eq!(p.body.chunks[HEAD].vel.y, -2.0);
    assert_eq!(p.jump_chunk(), Some(OTHER));
    assert_eq!(p.take_jump_chunk_push(), None);
}

#[test]
fn jump_on_chunk_plays_jump_on_creature_which_has_no_line_and_draws_nothing() {
    // `room.PlaySound(Slugcat_Jump_On_Creature, bodyChunks[1])`. The game's
    // `sounds.txt` has no line for it: `ShouldSoundPlay` refuses it before any draw.
    let mut rng = UnityRandom::new(5);
    let mut p = Player::on_chunk(7.5, 2.5);
    let mut s = sounds_with(SoundId::SlugcatNormalJump);
    p.jump_on_chunk();
    p.flush_sounds(&mut rng, &mut s);
    assert_eq!(rng.value(), UnityRandom::new(5).value());
    // With a line it would make an ordinary one-shot's 7 draws on the hips.
    let mut rng = UnityRandom::new(5);
    let mut p = Player::on_chunk(7.5, 2.5);
    let mut s = sounds_with(SoundId::SlugcatJumpOnCreature);
    p.jump_on_chunk();
    p.flush_sounds(&mut rng, &mut s);
    let mut expected = UnityRandom::new(5);
    for _ in 0..7 {
        expected.value();
    }
    assert_eq!(rng.value(), expected.value());
}

#[test]
fn a_press_within_the_counter_jumps_off_the_chunk_after_canjump_and_wall_jump() {
    // Airborne (no `canJump`, no `canWallJump`): the press edge is buffered one tick,
    // then `jumpChunkCounter > 0 && wantToJump > 0` jumps (`jumpChunkCounter = -5`).
    // The counter ticks toward 0 in `Update` before the dispatch: 5 -> 4 -> 3.
    let r = flat_room();
    let mut p = Player::on_chunk(7.5, 2.5);
    p.step(&r, JUMP);
    assert_eq!(p.jump_chunk_counter(), 4);
    assert!(p.take_jump_chunk_push().is_none());
    p.step(&r, JUMP);
    assert_eq!(p.jump_chunk_counter(), -5);
    assert_eq!(p.jump_boost, 7.0);
    assert!(p.take_jump_chunk_push().is_some());
    // Then back toward 0 one a tick.
    p.step(&r, Input::NONE);
    assert_eq!(p.jump_chunk_counter(), -4);
}

#[test]
fn a_ground_jump_takes_precedence_over_the_jump_chunk() {
    // `canJump > 0` is the first branch: `Jump()` and the chunk is left alone.
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.jump_chunk = Some(OTHER);
    p.jump_chunk_counter = 5;
    p.jump_chunk_body = Some(JumpChunkBody {
        total_mass: 7.5,
        mass: 2.5,
    });
    p.step(&r, JUMP);
    p.step(&r, JUMP);
    assert_eq!(p.jump_boost, 8.0);
    assert_eq!(p.jump_chunk_counter(), 3);
    assert_eq!(p.jump_chunk(), Some(OTHER));
}
