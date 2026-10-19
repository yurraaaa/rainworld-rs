//! Entering, riding and leaving shortcuts: the tail of `Player.MovementUpdate`, the
//! entering branch of `Creature.Update`, `SuckedIntoShortCut`, the vessel step of
//! `ShortcutHandler.Update` and `Player.SpitOutOfShortCut`.
//!
//! Behaviour: `rainworld-re/notes/shortcuts.md` sections 2-5, Remix off. What happens at
//! a room exit is the world's business ([`crate::RainWorldGame`]).
//!
//! Mirrors: Player.cs (MovementUpdate, SpitOutOfShortCut), Creature.cs (Update, SuckedIntoShortCut),
//!   ShortcutHandler.cs (Update)

use rw_assets::{Room, Shortcut, ShortcutType, Terrain, tile_centre};
use rw_random::UnityRandom;

use super::{HEAD, HIPS, Player};
use crate::shortcut::ShortcutVessel;
use crate::{SoundId, Sounds, Vec2};

/// `shortcutDelay` after leaving a shortcut.
const SPIT_OUT_DELAY: i32 = 20;
/// Pull-in speed, px per tick.
const PULL_IN_SPEED: f32 = 4.5;
/// Pull-in velocity damping per tick.
const PULL_IN_DAMPING: f32 = 0.06;
/// Chunks closer than this (or `short_cut_rad_add`, if larger) snap to the target.
const PULL_IN_RADIUS: f32 = 10.0;
/// `shortCutRadAdd` while not entering.
pub(super) const RAD_ADD_RESET: f32 = 0.9;

impl Player {
    /// Ticks before a shortcut can be entered again (`shortcutDelay`).
    #[must_use]
    pub fn shortcut_delay(&self) -> i32 {
        self.shortcut_delay
    }

    /// The entrance the body is being pulled into (`enteringShortCut`).
    #[must_use]
    pub fn entering_shortcut(&self) -> Option<(i32, i32)> {
        self.entering_shortcut
    }

    /// Whether the player is inside a tube (`inShortcut`): out of the room, no body.
    #[must_use]
    pub fn in_shortcut(&self) -> bool {
        self.vessel.is_some()
    }

    /// The tube tile the player's vessel is on, if inside a shortcut.
    #[must_use]
    pub fn shortcut_vessel_tile(&self) -> Option<(i32, i32)> {
        self.vessel.map(|v| v.pos)
    }

    /// The vessel's `lastPositions[0]`, if inside a shortcut.
    #[must_use]
    pub fn shortcut_vessel_last_tile(&self) -> Option<(i32, i32)> {
        self.vessel.map(|v| v.last_pos)
    }

    /// The shortcut block at the end of `MovementUpdate`: a chunk in an entrance tile
    /// with input exactly into the hole starts entering, head first. Dead ends, dens and
    /// NPC transports can't be entered.
    pub(super) fn enter_shortcut_by_input(&mut self, room: &Room) {
        if self.shortcut_delay >= 1 {
            return;
        }
        let input = (self.input_x(), self.input_y());
        for i in [HEAD, HIPS] {
            let pos = self.body.chunks[i].pos;
            if self.entering_shortcut.is_some()
                || room.tile_at(pos.to_array()).terrain != Terrain::ShortcutEntrance
            {
                continue;
            }
            let tile = pos.tile();
            let enterable = matches!(
                room.shortcut_map().type_at(tile),
                ShortcutType::Normal | ShortcutType::RoomExit | ShortcutType::RegionTransportation
            );
            let hole = room.shortcut_hole_direction(tile.0, tile.1);
            if enterable && input == (-hole.0, -hole.1) {
                self.entering_shortcut = Some(tile);
            }
        }
    }

    /// The entering branch of `Creature.Update`, run instead of physics: damp and pull
    /// every chunk 4.5 px per tick toward a point 5 px into the tube from the entrance
    /// middle, snapping those within the radius; once none moved, go in. A stun of 10
    /// or more cancels the entry.
    pub(super) fn pull_into_shortcut(&mut self, room: &Room, entrance: (i32, i32)) {
        self.short_cut_rad_add += 0.2;
        let hole = room.shortcut_hole_direction(entrance.0, entrance.1);
        let target = Vec2::from(tile_centre(entrance.0, entrance.1))
            + Vec2::new(hole.0 as f32, hole.1 as f32) * -5.0;
        let radius = PULL_IN_RADIUS.max(self.short_cut_rad_add);
        self.set_shortcut_pull(target, radius);
        let mut moving = 0;
        for c in &mut self.body.chunks {
            c.vel *= PULL_IN_DAMPING;
            if c.pos.distance(target) < radius {
                c.pos = target;
                c.last_pos = target;
            } else {
                c.last_pos = c.pos;
                c.pos += c.pos.direction_to(target) * PULL_IN_SPEED;
                moving += 1;
            }
        }
        if moving == 0 {
            self.sucked_into_shortcut(room, entrance);
        }
        if self.is_stunned() {
            self.entering_shortcut = None;
        }
    }

    /// `Creature.SuckedIntoShortCut` and `ShortcutHandler.SuckInCreature`: a vessel
    /// starts at the entrance and the body leaves the room.
    fn sucked_into_shortcut(&mut self, room: &Room, entrance: (i32, i32)) {
        if room.tile(entrance.0, entrance.1).terrain != Terrain::ShortcutEntrance {
            return;
        }
        // `shortcutData` gives the placeholder starting at the tile itself when the
        // entrance is not indexed, so the vessel always starts on `entrance`.
        // `Player_Enter_Shortcut` has no sounds.txt line: it draws nothing.
        self.play_sound(SoundId::PlayerEnterShortcut, HEAD, 1.0, 1.0);
        self.vessel = Some(ShortcutVessel::new(entrance));
        self.entering_shortcut = None;
    }

    /// One vessel step of `ShortcutHandler.Update` in `room`, the room the vessel is in.
    /// Each step ticks (`Player_Tick_Along_In_Shortcut`); reaching an entrance spits the
    /// player out there with `Player_Exit_Shortcut` (no sounds.txt line). Reaching a
    /// node tile is reported for the world to handle.
    pub(crate) fn shortcut_vessel_update(
        &mut self,
        room: &Room,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> VesselStep {
        let Some(vessel) = &mut self.vessel else {
            return VesselStep::Waited;
        };
        let from = vessel.pos;
        if !vessel.step(room) {
            return VesselStep::Waited;
        }
        let pos = vessel.pos;
        sounds.play_disembodied(SoundId::PlayerTickAlongInShortcut, 0.0, 1.0, 1.0, rng);
        let tile = room.tile(pos.0, pos.1);
        if tile.terrain == Terrain::ShortcutEntrance {
            let middle = Vec2::from(tile_centre(pos.0, pos.1));
            sounds.play_at(SoundId::PlayerExitShortcut, middle, 1.0, 1.0, rng);
            self.spit_out_of_shortcut(room, pos, rng);
            return VesselStep::SpatOut;
        }
        if pos != from && !matches!(tile.shortcut, Shortcut::None | Shortcut::Path) {
            let node = room.shortcut_map().nodes().iter().position(|&n| n == pos);
            return VesselStep::Node {
                node,
                kind: tile.shortcut,
                from,
            };
        }
        VesselStep::Moved
    }

    /// The faulty room exit: the vessel steps back to `from` (`PushNewLastPos(pos)`,
    /// `pos = from`).
    pub(crate) fn bounce_vessel(&mut self, from: (i32, i32)) {
        if let Some(v) = &mut self.vessel {
            v.last_pos = v.pos;
            v.pos = from;
        }
    }

    /// The dev teleport's `SuckInCreature`: a vessel on the head's tile, the body out of
    /// the room (the lobby then carries it to another room's node).
    pub(crate) fn dev_enter_vessel(&mut self) {
        self.vessel = Some(ShortcutVessel::new(self.head().pos.tile()));
        self.entering_shortcut = None;
    }

    /// `SetAllPositions`: the vessel restarts on `tile` (another room's node).
    pub(crate) fn set_vessel_at(&mut self, tile: (i32, i32)) {
        if let Some(v) = &mut self.vessel {
            *v = ShortcutVessel::new(tile);
        }
    }

    /// The vessel is dropped (`transportVessels.RemoveAt` with nowhere to go, or the
    /// player put back into a room).
    pub(crate) fn clear_vessel(&mut self) {
        self.vessel = None;
        self.entering_shortcut = None;
    }

    /// `Player.SpitOutOfShortCut` into `room`: head 2.5 px out of the entrance middle
    /// toward the room, hips 2.5 px into the tube, both moving `2 * hole`;
    /// `shortcutDelay = 20`. Animation, body mode and standing are kept. The base
    /// `Creature.SpitOutOfShortCut` first puts each chunk at the middle plus `Custom.RNV()`
    /// (one draw each), which the player's positions overwrite.
    fn spit_out_of_shortcut(&mut self, room: &Room, tile: (i32, i32), rng: &mut UnityRandom) {
        for _ in &self.body.chunks {
            Vec2::random_unit(rng);
        }
        self.entering_shortcut = None;
        self.force_sleep_counter = 0;
        self.still_in_start_shelter = false;
        self.shortcut_delay = SPIT_OUT_DELAY;
        self.vessel = None;
        let (hx, hy) = room.shortcut_hole_direction(tile.0, tile.1);
        let hole = Vec2::new(hx as f32, hy as f32);
        let middle = Vec2::from(tile_centre(tile.0, tile.1));
        for (i, c) in self.body.chunks.iter_mut().enumerate() {
            c.reset_at(middle - hole * ((-0.5 + i as f32) * 5.0));
            c.vel = hole * 2.0;
        }
    }
}

/// What one vessel step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VesselStep {
    /// No vessel, or it sat out a wait.
    Waited,
    Moved,
    /// Left the tube into the room.
    SpatOut,
    /// Reached a node tile (`shortCut > 1`): `node` is its index in the room's node list
    /// (`None` for an NPC transport tile), `from` the tile before.
    Node {
        node: Option<usize>,
        kind: Shortcut,
        from: (i32, i32),
    },
}
