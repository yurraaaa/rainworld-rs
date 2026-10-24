//! `ShortcutHandler.Update` in the game: the vessels in the tubes, the between-rooms
//! lobby, room exits, the player's arrival in a room and camera 0 following it.
//!
//! Mirrors: ShortcutHandler.cs (Update), RainWorldGame.cs, RoomCamera.cs (MoveCamera)

use rw_assets::{NodeType, Shortcut};

use super::{CameraMove, OverseerCall, PlayerObserver, RainWorldGame};
use crate::creature::{CreatureId, WorldCoordinate};
use crate::effects::particles::insect::turn::InsectViewChange;
use crate::game::camera::CameraView;
use crate::game::shortcut::Transport;
use crate::{EntranceColor, LobbyVessel, Player, ShortCutColor, VesselOwner, VesselStep};

impl RainWorldGame {
    /// `ShortcutHandler.Update`: the vessels in the tubes, newest first, then the lobby.
    /// A room exit leading nowhere returns from the whole update.
    pub(crate) fn update_shortcuts(&mut self, observer: &mut dyn PlayerObserver) {
        let mut i = self.shortcuts.transport.len();
        while i > 0 {
            i -= 1;
            let Some(&t) = self.shortcuts.transport.get(i) else {
                continue;
            };
            let go_on = match t {
                Transport::Player => {
                    let go_on = self.update_player_vessel(observer);
                    if !self.player_in_tube() {
                        self.shortcuts.transport.remove(i);
                    }
                    go_on
                }
                Transport::Creature(v) => self.creature_vessel_step(i, v, observer),
            };
            if !go_on {
                return;
            }
        }
        self.update_lobby(observer);
    }

    /// Whether the player's vessel is in `transportVessels`.
    fn player_in_tube(&self) -> bool {
        self.player.as_ref().is_some_and(Player::in_shortcut) && !self.player_in_lobby()
    }

    /// Whether the player's vessel waits in the lobby.
    pub(crate) fn player_in_lobby(&self) -> bool {
        self.shortcuts
            .lobby
            .iter()
            .any(|v| v.owner == VesselOwner::Player)
    }

    /// The player's vessel in `transportVessels`. Returns `false` where the game returns
    /// from the whole update.
    fn update_player_vessel(&mut self, observer: &mut dyn PlayerObserver) -> bool {
        let here = self.pos().room;
        let in_lobby = self.player_in_lobby();
        let Some(player) = self.player.as_mut().filter(|p| !p.is_dead()) else {
            return true;
        };
        if !player.in_shortcut() || in_lobby {
            return true;
        }
        let Some(room) = self.world.realized(here).map(|r| r.room.clone()) else {
            log::warn!("vessel in an unrealized room; dropped");
            player.clear_vessel();
            return true;
        };
        let sounds = if here == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        match player.shortcut_vessel_update(&room, &mut self.rng, sounds) {
            VesselStep::SpatOut => {
                self.player_seq = self.world.next_update_seq();
                observer.spat_out(player, &mut self.rng);
                // `NewRoom`'s `ReportRoomVisited` (a no-op for a room already visited).
                self.report_room_visited(here);
                if let Some(r) = self.world.abstract_room(here).filter(|r| r.shelter()) {
                    self.entered_shelters.push(r.name.clone());
                }
                self.spit_out_carried(observer);
                if let Some(player) = self.player.as_ref() {
                    observer.spat_out_finished(player, &mut self.rng);
                }
            }
            VesselStep::Node { node, kind, from } => {
                let mut pos = self.world.creature(self.player_id).pos;
                pos.node = node.map_or(-1, |n| i32::try_from(n).unwrap_or(-1));
                self.world.place_creature(self.player_id, pos);
                if kind == Shortcut::RoomExit && !self.room_exit(node.unwrap_or(0), from) {
                    return false;
                }
            }
            VesselStep::Waited | VesselStep::Moved => {}
        }
        true
    }

    /// The room-exit case of `ShortcutHandler.Update`: the vessel joins the lobby for the
    /// connected room. Returns `false` where the game returns from the whole update (an
    /// exit leading nowhere: the vessel stalls and turns back).
    fn room_exit(&mut self, node: usize, from: (i32, i32)) -> bool {
        let here = self.pos().room;
        let connections = self
            .world
            .abstract_room(here)
            .map(|r| r.connections.clone())
            .unwrap_or_default();
        let Some(player) = self.player.as_mut() else {
            return true;
        };
        if connections.is_empty() {
            log::warn!("room exit of a room without connections; the player is lost");
            self.player = None;
            self.sync_player_realized();
            return true;
        }
        let Some(&dest) = connections.get(node) else {
            log::warn!("faulty room exit");
            player.bounce_vessel(from);
            return true;
        };
        if dest <= -1 {
            return false;
        }
        let entrance_node = self
            .world
            .abstract_room(dest)
            .map_or(-1, |r| r.exit_index(here));
        self.shortcuts.lobby.push(LobbyVessel {
            room: dest,
            entrance_node,
            updates: 0,
            owner: VesselOwner::Player,
        });
        true
    }

    /// The `betweenRoomsWaitingLobby` loop, newest vessel first.
    fn update_lobby(&mut self, observer: &mut dyn PlayerObserver) {
        let mut i = self.shortcuts.lobby.len();
        while i > 0 {
            i -= 1;
            let Some(vessel) = self.shortcuts.lobby.get(i).copied() else {
                continue;
            };
            let done = match vessel.owner {
                VesselOwner::Teleport(id) => self.teleport_vessel_step(id, vessel, observer),
                VesselOwner::Player => self.player_vessel_step(i, vessel),
                VesselOwner::Shortcut(v) => self.creature_lobby_step(v, vessel),
                VesselOwner::BatHive(id) => self.bat_hive_lobby_step(id, vessel),
            };
            if done {
                self.shortcuts.lobby.remove(i);
            }
        }
    }

    /// An overseer's teleportation vessel: into its destination once
    /// `VesselAllowedInRoom` (`TeleportingIntoRoom`), or abstracted there if
    /// the room is not realized. Returns whether it left the lobby.
    fn teleport_vessel_step(
        &mut self,
        id: CreatureId,
        vessel: LobbyVessel,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        let coord = WorldCoordinate::new(vessel.room, -1, -1, vessel.entrance_node);
        match self.world.realized(vessel.room) {
            None => {
                self.overseer_module_removed(id, observer);
                self.world.abstractize_to(id, coord);
                true
            }
            Some(_) if self.world.vessel_allowed_in_room(id, vessel.room) => {
                self.world.move_creature(id, coord);
                self.run_overseer(id, OverseerCall::TeleportIn, observer);
                true
            }
            Some(_) => false,
        }
    }

    /// The player's vessel: the destination is realized, then once it is
    /// `ReadyForPlayer` the player moves there, its vessel restarts at the entrance node
    /// and the camera moves to the node's camera (the rooms' `NoLongerViewed` and
    /// `NowViewed` for the overseers' graphics). Returns whether it left the lobby.
    fn player_vessel_step(&mut self, index: usize, mut lobby: LobbyVessel) -> bool {
        lobby.updates += 1;
        self.shortcuts.lobby[index] = lobby;
        let Some(dest) = self.world.realized(lobby.room) else {
            if let Err(e) = self.activate_room(lobby.room) {
                log::warn!("cannot realize room {}: {e}", lobby.room);
            }
            return false;
        };
        if !self
            .world
            .vessel_allowed_in_room(self.player_id, lobby.room)
        {
            if dest.ready_for_player() {
                log::info!("denied room entrance because of carried creature");
            }
            return false;
        }
        let node_tile = usize::try_from(lobby.entrance_node)
            .ok()
            .and_then(|n| dest.room.shortcut_map().nodes().get(n).copied());
        let node = self.world.abstract_room(lobby.room).and_then(|r| {
            r.nodes
                .get(usize::try_from(lobby.entrance_node).ok()?)
                .cloned()
        });
        let Some(node) = node else {
            // The game indexes `nodes[-1]` here and throws every update.
            log::warn!(
                "room {} has no exit back; the vessel stays in the lobby",
                lobby.room
            );
            return false;
        };
        self.set_pos(WorldCoordinate::new(
            lobby.room,
            -1,
            -1,
            lobby.entrance_node,
        ));
        if matches!(
            node.kind,
            NodeType::Exit | NodeType::Den | NodeType::RegionTransportation
        ) && let (Some(player), Some(tile)) = (self.player.as_mut(), node_tile)
        {
            player.set_vessel_at(tile);
            self.shortcuts.transport.push(Transport::Player);
            if node.kind == NodeType::Exit {
                self.blink_arrival(lobby, 2.5, ShortCutColor::Slugcat);
            }
        }
        log::info!(
            "entered room {} after {} shortcut updates in the lobby",
            self.world
                .abstract_room(lobby.room)
                .map_or("?", |r| r.name.as_str()),
            lobby.updates
        );
        self.camera_moves_to(lobby.room, node.viewed_by_camera);
        true
    }

    /// `cameras[0].MoveCamera(room, camera)` for a vessel camera 0 follows leaving the
    /// lobby: the camera's position (the rooms' `NoLongerViewed`/`NowViewed` wait for
    /// `ApplyPositionChange`, `apply_view_change`), and the
    /// sounds whose owners the old room held stop following them.
    pub(crate) fn camera_moves_to(&mut self, room: i32, viewed_by_camera: i32) {
        // A move before the last one's `ApplyPositionChange` replaces it (`loadingRoom` is
        // overwritten): the room it named never gets its `NowViewed`.
        let from = self.insect_view.map_or(Some(self.camera_room), |v| v.from);
        self.insect_view = Some(InsectViewChange { from, to: room });
        self.camera_room = room;
        let camera = usize::try_from(viewed_by_camera).unwrap_or(0);
        self.camera_move = Some(CameraMove { room, camera });
        self.camera_view = CameraView {
            room,
            current: camera,
            next: camera,
        };
        self.sounds.owner_removed();
    }

    /// A vessel came out of the lobby into exit node `lobby.entrance_node`: the room
    /// blinks the entrance leading there by `fac` (2.5 for a slugcat, else 1.5), which
    /// camera 0, if it already shows the room, colours in the creature's `color`.
    pub(crate) fn blink_arrival(&mut self, lobby: LobbyVessel, fac: f32, color: ShortCutColor) {
        let Some(node) = usize::try_from(lobby.entrance_node).ok() else {
            return;
        };
        let Some(room) = self.world.realized_mut(lobby.room) else {
            return;
        };
        let map = room.room.shortcut_map();
        let Some(entrance) = map
            .leading_to_node(node)
            .and_then(|s| map.index_of(s.start))
        else {
            return;
        };
        if let Some(b) = &mut room.shortcuts_blinking {
            b.blink(Some(entrance), None, fac);
        }
        if self.camera_room == lobby.room {
            self.entrance_colors.push(EntranceColor {
                room: lobby.room,
                entrance,
                color,
            });
        }
    }
}
