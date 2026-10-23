//! A heavy object in hand 0 (`HeavyCarry(grasps[0].grabbed)`) as `MovementUpdate` and
//! `Jump` feel it: a slower run outside the walking modes, a dragged body helped
//! through narrow spaces, the player pulled into a shortcut by what it drags, and
//! stronger jumps. The game lends the held body's chunks for the player's update
//! ([`HeavyCarry`]) and takes them back after. Base game: no cicada lift.
//!
//! Mirrors: Player.cs (MovementUpdate, Jump)

use std::sync::Arc;

use rw_assets::{Room, ShortcutType, Terrain};

use super::{BodyMode, HEAD, Player};
use crate::world::AiMap;
use crate::{BodyChunk, Vec2};

/// The object hand 0 carries heavily, lent to the player for its update.
#[derive(Debug, Clone, PartialEq)]
pub struct HeavyCarry {
    pub total_mass: f32,
    /// `Grabability == Drag`.
    pub drag: bool,
    /// The object's `gravity`.
    pub gravity: f32,
    /// `grasps[0].grabbedChunk`'s index.
    pub grabbed_chunk: usize,
    /// The object's body chunks, moved by the update and written back after.
    pub chunks: Vec<BodyChunk>,
    /// The room's `aimap`.
    pub ai_map: Option<Arc<AiMap>>,
}

impl Player {
    /// Lends the object hand 0 carries heavily for the next update (`None`: none).
    pub(crate) fn lend_heavy_carry(&mut self, carry: Option<HeavyCarry>) {
        self.heavy_carry = carry;
    }

    /// The lent object back, with its chunks as the update left them.
    pub(crate) fn take_heavy_carry(&mut self) -> Option<HeavyCarry> {
        self.heavy_carry.take()
    }

    /// `Jump`'s `num` (no adrenaline): 1, plus `1.5 (mass - 0.2)` up to 1.3 for a heavy
    /// carry.
    pub(crate) fn jump_power(&self) -> f32 {
        self.heavy_carry.as_ref().map_or(1.0, |c| {
            1.0 + ((c.total_mass - 0.2).max(0.0) * 1.5).min(1.3)
        })
    }

    /// `MovementUpdate`'s heavy-carry block, between the beam grab and the run force:
    /// the run speed caps divided by `1 + (mass - 0.2)` (by 1 for a dragged body while
    /// in the default, corridor, standing or crawling mode); a dragged body pushed along
    /// the input through narrow spaces; a held body touching a Normal or RoomExit
    /// shortcut entrance the input points into pulls the player in.
    pub(super) fn heavy_carry_movement(&mut self, room: &Room) {
        let Some(mut carry) = self.heavy_carry.take() else {
            return;
        };
        let mut slow = 1.0 + (carry.total_mass - 0.2).max(0.0);
        let (x, y) = (self.input_x(), self.input_y());
        let input = Vec2::new(x as f32, y as f32).normalized();
        if carry.drag {
            if matches!(
                self.body_mode,
                BodyMode::Default | BodyMode::CorridorClimb | BodyMode::Stand | BodyMode::Crawl
            ) {
                slow = 1.0;
            }
            if let Some(map) = carry.ai_map.clone() {
                self.drag_through_narrow_space(&mut carry, &map, input);
            }
        }
        let axis = (x == 0 || y == 0) && (x != 0 || y != 0);
        if self.shortcut_delay < 1 && self.entering_shortcut.is_none() && axis {
            for c in &carry.chunks {
                let (tx, ty) = c.pos.tile();
                let tile = (tx + x, ty + y);
                if room.tile(tile.0, tile.1).terrain != Terrain::ShortcutEntrance
                    || room.shortcut_hole_direction(tile.0, tile.1) != (-x, -y)
                {
                    continue;
                }
                if matches!(
                    room.shortcut_map().type_at(tile),
                    ShortcutType::RoomExit | ShortcutType::Normal
                ) {
                    self.entering_shortcut = Some(tile);
                    log::info!("player pulled into shortcut by carried object");
                    break;
                }
            }
        }
        for cap in &mut self.dynamic_run_speed {
            *cap /= slow;
        }
        self.heavy_carry = Some(carry);
    }

    /// A dragged body in narrow spaces: the held chunk pushed along the input while the
    /// main chunk is in one; each chunk in one slowed, held up against 0.85 of its
    /// gravity and moved along the input, less the heavier the body.
    fn drag_through_narrow_space(&self, carry: &mut HeavyCarry, map: &AiMap, input: Vec2) {
        let fac = self.stats.corridor_climb_speed_fac;
        let mass = carry.total_mass;
        let (mx, my) = self.body.chunks[HEAD].pos.tile();
        if map.narrow_space(mx, my)
            && let Some(c) = carry.chunks.get_mut(carry.grabbed_chunk)
        {
            c.vel += input * (fac * 4.0) / mass.max(0.75);
        }
        let n = carry.chunks.len() as f32;
        let eg = self.effective_room_gravity();
        for c in &mut carry.chunks {
            let (cx, cy) = c.pos.tile();
            if !map.narrow_space(cx, cy) {
                continue;
            }
            c.vel *= 0.8;
            c.vel.y += eg * carry.gravity * 0.85;
            c.vel += input * (fac * 1.5) / (n * ((mass + 1.0) / 2.0).max(1.0));
            c.pos += input * (fac * 1.1) / (n * ((mass + 2.0) / 3.0).max(1.0));
        }
    }
}
