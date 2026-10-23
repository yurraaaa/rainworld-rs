//! Shortcut travel: the game's `ShortcutHandler` tick divider and the vessel a creature
//! rides in while inside a tube; the rooms' entrance blinking (`Room.shortcutsBlinking`,
//! `Creature.NewTile`) that `ShortcutGraphics` draws.
//!
//! Behaviour: `rainworld-re/notes/shortcuts.md` sections 3 and 10. The map itself is
//! [`rw_assets::ShortcutMap`].
//!
//! Mirrors: ShortcutHandler.cs, Room.cs (shortcutsBlinking), Creature.cs (NewTile)

use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_assets::{Room, Shortcut, ShortcutType, Terrain};
use rw_random::UnityRandom;

use crate::creature::{CreatureId, ShortCutColor, WorldCoordinate};
use crate::world::World;
use crate::{FloatExt, Player, SoundId, Sounds, TileExt, Vec2, VesselStep};

/// `RainWorldGame.updateShortCut`, which runs `ShortcutHandler.Update` only on every
/// third game tick, and the handler's `transportVessels` and `betweenRoomsWaitingLobby`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShortcutHandler {
    update_counter: u8,
    /// `transportVessels`, oldest first (the player's vessel is the player's own).
    pub(crate) transport: Vec<Transport>,
    /// `betweenRoomsWaitingLobby`, oldest first.
    pub lobby: Vec<LobbyVessel>,
}

/// One of `transportVessels`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    /// The player's vessel ([`Player`] keeps it).
    Player,
    Creature(CreatureVessel),
}

/// A creature's `ShortCutVessel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatureVessel {
    pub creature: CreatureId,
    /// The room whose tubes it is in (`vessel.room`).
    pub room: i32,
    pub(crate) vessel: ShortcutVessel,
    pub entrance_node: i32,
}

impl CreatureVessel {
    /// The tube tile and `lastPositions[0]`.
    #[must_use]
    pub fn tiles(&self) -> ((i32, i32), (i32, i32)) {
        (self.vessel.pos, self.vessel.last_pos)
    }

    /// The tube tile and every `lastPositions` entry, newest first.
    #[must_use]
    pub fn positions(&self) -> ((i32, i32), Vec<(i32, i32)>) {
        (self.vessel.pos, self.vessel.last_positions())
    }

    /// Updates the vessel still sits out (`wait`).
    #[must_use]
    pub fn wait(&self) -> i32 {
        self.vessel.wait
    }
}

impl CreatureVessel {
    /// One `ShortcutHandler.Update` step of an NPC's vessel: waits, or moves a tile with
    /// `Small_NPC_Tick_Along_In_Shortcut` at the new tile; at an entrance it is spat out
    /// (the caller's `SpitOutCreature`), at another node it reports the node.
    pub(crate) fn step(
        &mut self,
        room: &Room,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        tick_sound: SoundId,
    ) -> VesselStep {
        let from = self.vessel.pos;
        if !self.vessel.step(room) {
            return VesselStep::Waited;
        }
        let pos = self.vessel.pos;
        let middle = Vec2::middle_of_tile(pos);
        sounds.play_at(tick_sound, middle, 1.0, 1.0, rng);
        let tile = room.tile(pos.0, pos.1);
        if tile.terrain == Terrain::ShortcutEntrance {
            return VesselStep::SpatOut;
        }
        if pos != self.vessel.last_pos && !matches!(tile.shortcut, Shortcut::None | Shortcut::Path)
        {
            let node = room.shortcut_map().nodes().iter().position(|&n| n == pos);
            return VesselStep::Node {
                node,
                kind: tile.shortcut,
                from,
            };
        }
        VesselStep::Moved
    }
}

/// Who rides a vessel in the lobby.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VesselOwner {
    /// The player's shortcut vessel.
    Player,
    /// A realized creature's teleportation vessel (an overseer's).
    Teleport(CreatureId),
    /// A creature's shortcut vessel.
    Shortcut(CreatureVessel),
    /// A creature entering at a bat hive (`BatHiveVessel`).
    BatHive(CreatureId),
}

/// A vessel in `betweenRoomsWaitingLobby`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LobbyVessel {
    /// Global index of the room it waits to enter.
    pub room: i32,
    /// `entranceNode`: that room's exit leading back (`ExitIndex`), -1 if none.
    pub entrance_node: i32,
    /// Shortcut updates spent waiting.
    pub updates: u32,
    pub owner: VesselOwner,
}

impl LobbyVessel {
    /// The creature riding it, unless it is the player's.
    #[must_use]
    pub fn creature(&self) -> Option<CreatureId> {
        match self.owner {
            VesselOwner::Player => None,
            VesselOwner::Teleport(id) | VesselOwner::BatHive(id) => Some(id),
            VesselOwner::Shortcut(v) => Some(v.creature),
        }
    }
}

impl ShortcutHandler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creature `id`'s vessel, in the tubes or (with `true`) in the lobby.
    #[must_use]
    pub fn creature_vessel(&self, id: CreatureId) -> Option<(CreatureVessel, bool)> {
        let moving = self.transport.iter().find_map(|t| match t {
            Transport::Creature(v) if v.creature == id => Some((*v, false)),
            _ => None,
        });
        moving.or_else(|| {
            self.lobby.iter().find_map(|l| match l.owner {
                VesselOwner::Shortcut(v) if v.creature == id => Some((
                    CreatureVessel {
                        room: l.room,
                        entrance_node: l.entrance_node,
                        ..v
                    },
                    true,
                )),
                _ => None,
            })
        })
    }

    /// The player's vessel leaves the tubes and the lobby (a respawn or a placement).
    pub(crate) fn drop_player(&mut self) {
        self.lobby.retain(|v| v.owner != VesselOwner::Player);
        self.transport.retain(|t| *t != Transport::Player);
    }

    /// Counts one game tick; true on every third, when the handler updates.
    pub fn divide(&mut self) -> bool {
        self.update_counter += 1;
        if self.update_counter > 2 {
            self.update_counter = 0;
            true
        } else {
            false
        }
    }

    /// One game tick in a single-room world, called before the room's objects update
    /// (as the game does): every third call moves `player`'s vessel one tile with its
    /// tick sound, spitting the player out when it reaches an entrance. A single-room
    /// world's exits all lead to -1, so at a room exit the vessel stalls and turns back.
    pub fn update(
        &mut self,
        room: &Room,
        player: &mut Player,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> VesselStep {
        if self.divide() {
            player.shortcut_vessel_update(room, rng, sounds)
        } else {
            VesselStep::Waited
        }
    }
}

/// `Room.shortcutsBlinking`, one row per shortcut (`room.shortcuts` order): `[0]` the
/// pulse's brightness, `[1]` its progress along the pipe (0 when none runs), `[2]` how
/// far the entrance symbol has turned to the symbol colour, `[3]` a flash counter
/// (negative while the entrance flashes, positive while its symbol flickers).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShortcutsBlinking {
    rows: Vec<[f32; 4]>,
}

impl ShortcutsBlinking {
    /// `Room.ShortCutsReady`: all zero.
    #[must_use]
    pub fn new(shortcuts: usize) -> Self {
        Self {
            rows: vec![[0.0; 4]; shortcuts],
        }
    }

    #[must_use]
    pub fn rows(&self) -> &[[f32; 4]] {
        &self.rows
    }

    /// `Room.BlinkShortCut`: `secondary` sends a pulse down its pipe and flashes its
    /// entrance by `5 * fac` ticks; `shortcut` flashes by `20 * fac`.
    pub fn blink(&mut self, shortcut: Option<usize>, secondary: Option<usize>, fac: f32) {
        if let Some(row) = secondary.and_then(|i| self.rows.get_mut(i)) {
            row[0] = 1.0;
            if row[1] == 0.0 {
                row[1] = 0.01;
            }
            row[3] = -5.0 * fac;
        }
        if let Some(row) = shortcut.and_then(|i| self.rows.get_mut(i)) {
            row[3] = -20.0 * fac;
        }
    }

    /// The `shortcutsBlinking` block of `Room.Update`: the pulses fade and advance, and
    /// an idle entrance starts a flicker with chance 0.01 a tick. Draws on the game's
    /// stream: one per idle row (two when it starts a flicker), two per flickering row.
    pub fn update(&mut self, room: &Room, rng: &mut UnityRandom) {
        for (row, data) in self.rows.iter_mut().zip(room.shortcut_map().shortcuts()) {
            let span = 20.0_f32.lerp(data.length as f32, 0.3);
            let fade = if row[1] > 0.0 { span } else { 10.0 };
            row[0] = (row[0] - 1.0 / fade).clamp(0.0, 1.0);
            if row[1] > 0.0 {
                row[1] += 1.0 / span;
                if row[1] > 1.0 {
                    row[1] = 0.0;
                }
                row[2] = 0.0;
            }
            if row[3] == 0.0 {
                row[2] = row[2].lerp(1.0, 0.04);
                if rng.value() < 0.01 {
                    row[3] = rng.value() * 20.0;
                }
            } else if row[3] < 0.0 {
                row[3] = (row[3] + 1.0).min(0.0);
            } else {
                row[3] = (row[3] - 1.0).max(0.0);
                let to = rng.value();
                let t = 0.0_f32.lerp(0.5, rng.value());
                row[2] = row[2].lerp(to, t);
            }
        }
    }
}

/// `ShortcutHandler.ShortCutVessel`: `lastPositions` holds `shortcutSegments - 1`
/// tiles (1 to 4 in the base game; one for the slugcat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShortcutVessel {
    pub(crate) pos: (i32, i32),
    /// `lastPositions[0]`.
    pub(crate) last_pos: (i32, i32),
    /// `lastPositions[1..]`.
    older: [(i32, i32); 3],
    /// `lastPositions.Length`.
    len: usize,
    /// Updates to sit out (an NPC transport's distance; the player's is 0 without Jolly
    /// co-op).
    pub(crate) wait: i32,
}

impl ShortcutVessel {
    /// A vessel with one last position (the slugcat's).
    pub(crate) fn new(pos: (i32, i32)) -> Self {
        Self::with_segments(pos, 2, 0)
    }

    /// The constructor for a creature of `shortcut_segments` (`IntClamp(segments - 1,
    /// 1, 100)` last positions, all at `pos`).
    pub(crate) fn with_segments(pos: (i32, i32), shortcut_segments: i32, wait: i32) -> Self {
        let len = usize::try_from((shortcut_segments - 1).clamp(1, 4)).unwrap_or(1);
        Self {
            pos,
            last_pos: pos,
            older: [pos; 3],
            len,
            wait,
        }
    }

    /// `lastPositions`, newest first.
    pub(crate) fn last_positions(&self) -> Vec<(i32, i32)> {
        let mut v = vec![self.last_pos];
        v.extend_from_slice(&self.older[..self.len - 1]);
        v
    }

    /// `PushNewLastPos`.
    fn push_new_last_pos(&mut self, pos: (i32, i32)) {
        if self.len > 1 {
            for i in (1..self.len - 1).rev() {
                self.older[i] = self.older[i - 1];
            }
            self.older[0] = self.last_pos;
        }
        self.last_pos = pos;
    }

    /// One `ShortcutHandler.Update` step: counts `wait` down, or moves one tile along
    /// the tube. Returns whether it moved.
    pub(crate) fn step(&mut self, room: &Room) -> bool {
        if self.wait > 0 {
            self.wait -= 1;
            return false;
        }
        let prev = self.pos;
        self.pos = room.next_shortcut_position(self.pos, self.last_pos);
        self.push_new_last_pos(prev);
        true
    }
}

/// `ShortcutGraphics.ColorEntrance` on camera 0, which shows `room`: the entrance of
/// shortcut `entrance` flashes in a creature's `ShortCutColor()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntranceColor {
    pub room: i32,
    pub entrance: usize,
    pub color: ShortCutColor,
}

impl World {
    /// `Creature.NewTile` for creature `id`, which moved from `last` to its current
    /// tile: every entrance up to three tiles straight out from it whose hole faces it,
    /// unless the creature moves away from it, blinks. A pipe within the room sends a
    /// pulse from that entrance and flashes the far one; a room exit flashes the
    /// entrance leading back in the room behind it when that room is viewed (camera 0
    /// in `camera_room`), which also takes the creature's colour.
    pub(crate) fn new_tile(
        &mut self,
        id: CreatureId,
        last: WorldCoordinate,
        camera_room: i32,
    ) -> Option<EntranceColor> {
        let coord = self.creature(id).pos;
        let template = self.creature(id).template;
        let room = self.realized(coord.room)?;
        room.shortcuts_blinking.as_ref()?;
        let room = room.room.clone();
        let map = room.shortcut_map();
        let mut color = None;
        for (dx, dy) in FOUR_DIRECTIONS {
            for j in 1..4 {
                let tile = (coord.tile.0 + dx * j, coord.tile.1 + dy * j);
                if room.tile(tile.0, tile.1).terrain != Terrain::ShortcutEntrance
                    || room.shortcut_hole_direction(tile.0, tile.1) != (-dx, -dy)
                    || last.tile.manhattan(tile) < coord.tile.manhattan(tile)
                {
                    continue;
                }
                // `room.shortcutData`: an empty dead end at `tile` where none starts.
                let (kind, start, dest, dest_node) = map
                    .data_at(tile)
                    .map_or((ShortcutType::DeadEnd, tile, tile, None), |d| {
                        (d.kind, d.start, d.dest, d.dest_node)
                    });
                let shortcut = map.index_of(dest);
                let mut secondary = None;
                if kind == ShortcutType::RoomExit {
                    let blink = template.body_size.lerp(1.5, 0.5);
                    if let Some(c) =
                        self.blink_exit_behind(coord.room, dest_node, blink, camera_room)
                    {
                        color = Some(EntranceColor {
                            color: self.creature(id).short_cut_color(),
                            ..c
                        });
                    }
                } else {
                    secondary = map.index_of(start);
                }
                if let Some(b) = self
                    .realized_mut(coord.room)
                    .and_then(|r| r.shortcuts_blinking.as_mut())
                {
                    b.blink(shortcut, secondary, 1.0);
                }
            }
        }
        color
    }

    /// The room-exit branch of `NewTile`: through exit node `dest_node` of room `from`,
    /// the room behind, if realized and viewed, blinks the entrance leading back by
    /// `fac`. Returns that entrance for `ColorEntrance` (the colour left unset).
    fn blink_exit_behind(
        &mut self,
        from: i32,
        dest_node: Option<usize>,
        fac: f32,
        camera_room: i32,
    ) -> Option<EntranceColor> {
        let to = *self.abstract_room(from)?.connections.get(dest_node?)?;
        if to <= -1 || to != camera_room {
            return None;
        }
        let exit = usize::try_from(self.abstract_room(to)?.exit_index(from)).ok()?;
        let behind = self.realized_mut(to)?;
        let start = behind.room.shortcut_map().leading_to_node(exit)?.start;
        let entrance = behind.room.shortcut_map().index_of(start);
        behind
            .shortcuts_blinking
            .as_mut()?
            .blink(entrance, None, fac);
        Some(EntranceColor {
            room: to,
            entrance: entrance?,
            color: ShortCutColor::Slugcat,
        })
    }
}

impl crate::creature::CreatureTemplate {
    /// `ShortcutHandler.NPCShortcutSound`: entering (0), ticking along (1) or leaving
    /// (2), small (small creatures and body size under 0.4), large (over 1.5) or medium.
    #[must_use]
    pub fn npc_shortcut_sound(&self, situation: u8) -> SoundId {
        let size = if self.small_creature || self.body_size < 0.4 {
            0
        } else if self.body_size > 1.5 {
            2
        } else {
            1
        };
        match (situation, size) {
            (0, 0) => SoundId::SmallNpcEnterShortcut,
            (0, 2) => SoundId::LargeNpcEnterShortcut,
            (0, _) => SoundId::MediumNpcEnterShortcut,
            (1, 0) => SoundId::SmallNpcTickAlongInShortcut,
            (1, 2) => SoundId::LargeNpcTickAlongInShortcut,
            (1, _) => SoundId::MediumNpcTickAlongInShortcut,
            (_, 0) => SoundId::SmallNpcExitShortcut,
            (_, 2) => SoundId::LargeNpcExitShortcut,
            _ => SoundId::MediumNpcExitShortcut,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pipe from (1, 3) (hole right) down, along and up to (4, 3) (hole left).
    fn pipe_room() -> Room {
        let rows = ["######", "#E..E#", "#x##x#", "#xxxx#", "######"];
        let (w, h) = (rows[0].len(), rows.len());
        let mut grid = String::new();
        for x in 0..w {
            for row in rows {
                grid.push_str(match row.as_bytes()[x] {
                    b'#' => "1",
                    b'E' => "4,3",
                    b'x' => "1,3",
                    _ => "0",
                });
                grid.push('|');
            }
        }
        let text = format!("T\n{w}*{h}|-1|0\n0*0|0|0\n0,0\n\n\n\n\n\n\n\n{grid}\n");
        Room::parse(&text).expect("test room parses")
    }

    /// `Room.BlinkShortCut` (61605-61620).
    #[test]
    fn blink_sends_a_pulse_and_flashes() {
        let mut b = ShortcutsBlinking::new(2);
        b.blink(Some(1), Some(0), 2.0);
        assert_eq!(b.rows(), [[1.0, 0.01, 0.0, -10.0], [0.0, 0.0, 0.0, -40.0]]);
        b.rows[0][1] = 0.5;
        b.blink(None, Some(0), 1.0);
        assert_eq!(
            b.rows()[0],
            [1.0, 0.5, 0.0, -5.0],
            "a running pulse keeps going"
        );
        b.blink(Some(5), None, 1.0);
    }

    /// The `shortcutsBlinking` block of `Room.Update` (63403-63434): a pulse fades and
    /// advances by `1 / Lerp(20, length, 0.3)`, a flash counts up to 0 without drawing,
    /// an idle entrance draws once (twice when it starts a flicker) and turns its symbol
    /// toward 1 by 0.04, a flicker draws twice and counts down.
    #[test]
    fn update_steps_rows_with_the_games_draws() {
        let room = pipe_room();
        let length = room.shortcut_map().shortcuts()[0].length as f32;
        let span = 20.0 + (length - 20.0) * 0.3;
        let mut b = ShortcutsBlinking::new(2);
        b.blink(None, Some(0), 1.0);
        b.rows[0][3] = 0.0;
        b.rows[1] = [0.0, 0.0, 0.5, 3.0];
        let mut rng = UnityRandom::new(7);
        let mut expect = rng.clone();
        b.update(&room, &mut rng);
        assert!(
            expect.value() >= 0.01,
            "the idle row starts no flicker on this seed"
        );
        let to = expect.value();
        let t = expect.value() * 0.5;
        assert_eq!(rng.state(), expect.state(), "three draws");
        let r0 = b.rows()[0];
        assert!((r0[0] - (1.0 - 1.0 / span)).abs() < 1e-6);
        assert!((r0[1] - (0.01 + 1.0 / span)).abs() < 1e-6);
        assert!((r0[2] - 0.04).abs() < 1e-6);
        assert_eq!(r0[3], 0.0);
        let r1 = b.rows()[1];
        assert_eq!(r1[3], 2.0);
        assert!((r1[2] - (0.5 + (to - 0.5) * t)).abs() < 1e-6);
        // A flash counts up to 0 with no draw.
        b.rows[0][3] = -2.0;
        b.rows[1][3] = -1.0;
        let before = rng.state();
        b.update(&room, &mut rng);
        assert_eq!(rng.state(), before);
        assert_eq!((b.rows()[0][3], b.rows()[1][3]), (-1.0, 0.0));
    }
}
