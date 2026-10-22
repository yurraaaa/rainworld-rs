//! The jump off another object's chunk (`jumpChunk`): `Player.Collide`'s tail notes a
//! chunk the hips rest on, and `MovementUpdate` jumps off it within 5 ticks. The game
//! lends the chunk's masses for the update and applies the push and the stun after.
//!
//! Behaviour: `slugcat-movement.md` section 16. No adrenaline (D3): the jump's
//! `Lerp(1, 1.35, Adrenaline)` is 1. The Snail branch comes with snails.
//!
//! Mirrors: Player.cs (Collide, JumpOnChunk)

use super::{BodyMode, HEAD, HIPS, Player};
use crate::item::Entity;
use crate::{BodyChunk, FloatExt, SoundId};

/// A chunk of another object (`jumpChunk`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpChunk {
    pub owner: Entity,
    pub index: usize,
}

/// What `JumpOnChunk` reads of the jump chunk, lent for the update; `None` when its
/// owner is not in the player's room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpChunkBody {
    /// The owner's `TotalMass`.
    pub total_mass: f32,
    /// The chunk's `mass`.
    pub mass: f32,
}

/// `JumpOnChunk`'s effect on the other object: the chunk's `vel.y` drop, and
/// `Stun(1)` when the owner is a creature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpChunkPush {
    pub chunk: JumpChunk,
    pub vel_y: f32,
}

impl Player {
    /// The tail of `Player.Collide(other, myChunk, otherChunk)`: in `Default` mode, the
    /// hips touching a chunk above its lower half mark it for 5 ticks (not while the
    /// counter is negative after a jump off a chunk).
    pub(crate) fn collide_jump_chunk(
        &mut self,
        my_chunk: usize,
        other: JumpChunk,
        chunk: &BodyChunk,
    ) {
        if self.jump_chunk_counter >= 0
            && self.body_mode == BodyMode::Default
            && my_chunk == HIPS
            && self.body.chunks[HIPS].pos.y > chunk.pos.y - chunk.rad / 2.0
        {
            self.jump_chunk_counter = 5;
            self.jump_chunk = Some(other);
        }
    }

    /// Lends the jump chunk's masses for the next update.
    pub(crate) fn lend_jump_chunk_body(&mut self, body: Option<JumpChunkBody>) {
        self.jump_chunk_body = body;
    }

    #[must_use]
    pub fn jump_chunk(&self) -> Option<JumpChunk> {
        self.jump_chunk
    }

    /// `canJump`.
    #[must_use]
    pub fn can_jump(&self) -> i32 {
        self.can_jump
    }

    #[must_use]
    pub fn jump_chunk_counter(&self) -> i32 {
        self.jump_chunk_counter
    }

    /// The push `JumpOnChunk` made in the last update, for the game to apply.
    pub(crate) fn take_jump_chunk_push(&mut self) -> Option<JumpChunkPush> {
        self.jump_chunk_push.take()
    }

    /// `jumpChunkCounter` toward 0, next to `canWallJump`'s.
    pub(super) fn count_jump_chunk(&mut self) {
        self.jump_chunk_counter -= self.jump_chunk_counter.signum();
    }

    /// `JumpOnChunk`: a hop off the chunk, weaker the lighter its owner is next to the
    /// player, pushing the chunk down. `Slugcat_Jump_On_Creature` has no `sounds.txt`
    /// line, so it makes no draws.
    pub(super) fn jump_on_chunk(&mut self) {
        let (Some(chunk), Some(body)) = (self.jump_chunk, self.jump_chunk_body) else {
            return;
        };
        self.play_sound(SoundId::SlugcatJumpOnCreature, HIPS, 1.0, 1.0);
        let total_mass = self.total_mass();
        let f = (body.total_mass / total_mass)
            .clamp(0.0, 1.0)
            .mathf_pow(0.7);
        self.body.chunks[HEAD].vel.y *= 1.0 - f * 0.9;
        self.body.chunks[HIPS].vel.y *= 1.0 - f * 0.9;
        self.body.chunks[HEAD].vel.y += 4.0 * f;
        self.body.chunks[HIPS].vel.y += 3.0 * f;
        self.jump_boost = 7.0;
        self.jump_chunk_push = Some(JumpChunkPush {
            chunk,
            vel_y: 7.0 * f * total_mass / body.mass,
        });
        self.jump_chunk = None;
    }
}
