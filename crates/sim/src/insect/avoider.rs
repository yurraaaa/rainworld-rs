//! `CosmeticInsect.CreatureAvoider`: every `updateRate` ticks an insect looks at one
//! creature of its room picked at random and keeps the scariest it can see.
//!
//! Mirrors: CosmeticInsect.cs (CreatureAvoider)

use super::{InsectCreatureSlot, InsectCx};
use crate::creature::CreatureId;
use crate::{FloatExt, RoomExt, Vec2};

/// A realized creature as the insects read it.
#[derive(Debug, Clone, PartialEq)]
pub struct InsectCreature {
    pub id: CreatureId,
    pub dead: bool,
    /// `realizedCreature.room` is the insect's room (not in a shortcut, not elsewhere).
    pub in_room: bool,
    /// `slatedForDeletetion`.
    pub slated: bool,
    /// `DangerPos`.
    pub danger_pos: Vec2,
    /// `TotalMass`.
    pub total_mass: f32,
    /// `mainBodyChunk.pos` and `.lastPos`.
    pub main_pos: Vec2,
    pub main_last_pos: Vec2,
    /// `bodyChunks`' positions and radii (the first is `firstChunk`).
    pub chunks: Vec<(Vec2, f32)>,
    /// `creatureTemplate.smallCreature`.
    pub small: bool,
    /// A `Player` that is `Malnourished`.
    pub malnourished_player: bool,
}

/// `CreatureAvoider`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureAvoider {
    /// `currentWorstCrit`, as last seen in the room's list (the game keeps the object).
    pub current_worst: Option<InsectCreature>,
    update_rate: i32,
    timer: i32,
    visual_range: f32,
    min_size: f32,
}

impl CreatureAvoider {
    /// The constructor (one draw).
    pub fn new(update_rate: i32, visual_range: f32, min_size: f32, cx: &mut InsectCx) -> Self {
        Self {
            current_worst: None,
            update_rate,
            timer: cx.rng.range_i32(0, update_rate),
            visual_range,
            min_size,
        }
    }

    /// `Reset`.
    pub fn reset(&mut self) {
        self.current_worst = None;
    }

    /// Reads the tracked creature's body as it is now, while the room lists it.
    pub(super) fn refresh(&mut self, creatures: &[InsectCreatureSlot]) {
        let Some(worst) = &mut self.current_worst else {
            return;
        };
        if let Some(now) = creatures
            .iter()
            .find(|c| c.id == worst.id)
            .and_then(|c| c.realized.as_ref())
        {
            worst.clone_from(now);
        } else {
            // Gone from the room or abstracted: its body is in no room now.
            worst.in_room = false;
        }
    }

    /// `FleeSpeed` for an insect at `pos`.
    #[must_use]
    pub fn flee_speed(&self, pos: Vec2) -> f32 {
        let Some(c) = &self.current_worst else {
            return 0.0;
        };
        let near = pos
            .distance(c.danger_pos)
            .inverse_lerp(self.visual_range, self.visual_range * 0.1);
        near.mathf_pow(c.total_mass.lerp_map(0.0, 20.0, 3.0, 0.2))
            * c.total_mass.lerp_map(0.0, 20.0, 0.5, 1.0)
    }

    /// `Update` for an insect at `pos` (one draw every `updateRate` ticks while the
    /// room lists a creature).
    pub fn update(&mut self, pos: Vec2, cx: &mut InsectCx) {
        self.timer -= 1;
        if self.timer < 1 {
            self.timer = self.update_rate;
            let picked = if cx.creatures.is_empty() {
                None
            } else {
                let i = cx.rng.range_i32(0, cx.creatures.len() as i32);
                cx.creatures[i as usize].realized.clone()
            };
            if let Some(creature) = picked {
                let mut worst = self.scary_score(self.current_worst.as_ref(), pos, cx);
                let score = self.scary_score(Some(&creature), pos, cx);
                if score > worst {
                    self.current_worst = Some(creature);
                    worst = score;
                }
                if worst == 0.0 {
                    self.current_worst = None;
                }
            }
        } else if let Some(c) = &self.current_worst
            && !pos.dist_less(c.danger_pos, self.visual_range)
        {
            self.current_worst = None;
        }
    }

    fn scary_score(&self, c: Option<&InsectCreature>, pos: Vec2, cx: &InsectCx) -> f32 {
        let Some(c) = c else {
            return 0.0;
        };
        if c.dead
            || !c.in_room
            || !pos.dist_less(c.danger_pos, self.visual_range)
            || c.total_mass < self.min_size
            || !cx.room.visual_contact(pos, c.danger_pos)
        {
            return 0.0;
        }
        c.total_mass.min(20.0)
            * 50.0.lerp(c.main_last_pos.distance(c.main_pos), 0.5)
            * pos
                .distance(c.danger_pos)
                .inverse_lerp(self.visual_range, self.visual_range * 0.1)
    }
}
