//! `RainWorldGame` for one player: the world, the player's place in it, the shortcut
//! handler with its between-rooms lobby, the room realizer, and the order of a tick.
//!
//! Creature-free: only the player exists (docs/DIVERGENCES.md D14). Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`, `shortcuts.md` section 5.

use rw_assets::{NodeType, Room, Shortcut, ShortcutType};
use rw_random::UnityRandom;

use crate::campaign::SlugcatCampaign;
use crate::world::{FollowCreature, RoomRealizer, World};
use crate::{Input, Player, ShortcutHandler, Sounds, VesselStep, Water, determine_safe_spawn_tile};

/// What the app attaches to the player (its graphics module), called where the game
/// calls it.
pub trait PlayerObserver {
    /// The player was constructed, before it is placed (`AddObject` builds the graphics
    /// module in a viewed room).
    fn created(&mut self, player: &Player, rng: &mut UnityRandom);
    /// Right after `Player.Update`.
    fn updated(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
    );
    /// The player left a shortcut (`SpitOutOfShortCut` resets the graphics).
    fn spat_out(&mut self, player: &Player, rng: &mut UnityRandom);
}

/// No graphics.
impl PlayerObserver for () {
    fn created(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn updated(&mut self, _: &Player, _: &Room, _: Option<&Water>, _: &mut UnityRandom) {}
    fn spat_out(&mut self, _: &Player, _: &mut UnityRandom) {}
}

/// The player's `AbstractCreature.pos`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoordinate {
    /// Global room index.
    pub room: i32,
    pub tile: (i32, i32),
    /// `abstractNode`, -1 when undefined.
    pub node: i32,
}

/// `RoomCamera.MoveCamera(room, camPos)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraMove {
    pub room: i32,
    pub camera: usize,
}

/// Where the player starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Start {
    /// Global index of the first room (`FIRSTROOM`).
    pub room: i32,
    /// Its tile; `None` picks a safe spawn tile (the port's dev start, D1).
    pub tile: Option<(i32, i32)>,
}

/// A running game.
pub struct RainWorldGame {
    campaign: &'static SlugcatCampaign,
    world: World,
    shortcuts: ShortcutHandler,
    /// `None` in a single-room world.
    realizer: Option<RoomRealizer>,
    player: Option<Player>,
    pos: WorldCoordinate,
    /// `cameras[0].room`.
    camera_room: i32,
    camera_move: Option<CameraMove>,
    rng: UnityRandom,
    sounds: Sounds,
    /// Sounds of rooms the camera is not in: the game's microphone ignores them, so
    /// they make no draws.
    silent: Sounds,
    /// `nextIssuedId`.
    next_issued_id: i32,
    /// The start tile, reused by respawns in the start room.
    start: Start,
    tick: u64,
}

impl RainWorldGame {
    /// The constructor's order for the parts ported: `nextIssuedId`, the first room
    /// activated and shown by camera 0, the realizer, then (the room loading at once)
    /// the player realized in it.
    pub fn new(
        world: World,
        campaign: &'static SlugcatCampaign,
        start: Start,
        mut rng: UnityRandom,
        sounds: Sounds,
        observer: &mut dyn PlayerObserver,
    ) -> Self {
        let next_issued_id = rng.range_i32(1000, 10000);
        let realizer = (!world.single_room()).then(RoomRealizer::new);
        let mut game = Self {
            campaign,
            world,
            shortcuts: ShortcutHandler::new(),
            realizer,
            player: None,
            pos: WorldCoordinate {
                room: start.room,
                tile: start.tile.unwrap_or(crate::campaign::DEFAULT_PLAYER_TILE),
                node: -1,
            },
            camera_room: start.room,
            camera_move: Some(CameraMove {
                room: start.room,
                camera: 0,
            }),
            rng,
            sounds,
            silent: Sounds::default(),
            next_issued_id,
            start,
            tick: 0,
        };
        if let Err(e) = game.world.activate_room(start.room) {
            log::error!("cannot realize the first room: {e}");
        }
        game.world.finish_loading();
        game.realize_player(observer);
        game
    }

    /// `AbstractCreature.RealizeInRoom` for the player at the start: an undefined node
    /// is drawn among the room's exits and dens (`NodesRelevantToCreature`), then the
    /// `Player` is built and placed.
    fn realize_player(&mut self, observer: &mut dyn PlayerObserver) {
        let Some(room) = self.world.realized(self.pos.room).map(|r| r.room.clone()) else {
            return;
        };
        let tile = match self.start.tile {
            Some(tile) => tile,
            None => determine_safe_spawn_tile(&room, &mut self.rng),
        };
        if self.pos.node < 0 {
            let relevant = self.world.abstract_room(self.pos.room).map_or(0, |r| {
                r.nodes
                    .iter()
                    .filter(|n| matches!(n.kind, NodeType::Exit | NodeType::Den))
                    .count()
            });
            let specific = self
                .rng
                .range_i32(0, i32::try_from(relevant).unwrap_or(i32::MAX));
            self.pos.node = self.common_node(specific);
        }
        self.pos.tile = tile;
        self.spawn_player(tile, observer);
    }

    /// `CreatureSpecificToCommonNodeIndex` for the slugcat (exit and den nodes).
    fn common_node(&self, specific: i32) -> i32 {
        self.world
            .abstract_room(self.pos.room)
            .and_then(|r| {
                r.nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| matches!(n.kind, NodeType::Exit | NodeType::Den))
                    .nth(usize::try_from(specific).ok()?)
                    .map(|(i, _)| i32::try_from(i).unwrap_or(-1))
            })
            .unwrap_or(-1)
    }

    /// The `Player` constructor, its graphics module, then `PlaceInRoom` on `tile`.
    fn spawn_player(&mut self, tile: (i32, i32), observer: &mut dyn PlayerObserver) {
        let mut player = Player::create(&mut self.rng);
        observer.created(&player, &mut self.rng);
        player.place(tile, &mut self.rng);
        self.player = Some(player);
    }

    /// A fresh player in the camera's room, at the start tile in the start room, else
    /// on a safe spawn tile (the R key; not a game function).
    pub fn respawn(&mut self, observer: &mut dyn PlayerObserver) {
        let Some(room) = self
            .world
            .realized(self.camera_room)
            .map(|r| r.room.clone())
        else {
            return;
        };
        let tile = match self.start.tile {
            Some(tile) if self.camera_room == self.start.room => tile,
            _ => determine_safe_spawn_tile(&room, &mut self.rng),
        };
        self.shortcuts.lobby = None;
        self.sounds.owner_removed();
        self.pos = WorldCoordinate {
            room: self.camera_room,
            tile,
            node: self.pos.node,
        };
        self.spawn_player(tile, observer);
        log::info!("respawned player");
    }

    /// One 40 Hz tick in `RainWorldGame.Update`'s order: the room realizer, the
    /// shortcut handler on every third tick, every active room (newest first: the entry
    /// delay, its water, and the player in it with its graphics), then the loading
    /// step. A dead player is frozen (docs/DIVERGENCES.md D4).
    pub fn tick(&mut self, input: Input, observer: &mut dyn PlayerObserver) {
        let follow = self.follow_creature();
        if let (Some(realizer), Some(follow)) = (&mut self.realizer, follow) {
            realizer.update(&mut self.world, follow, self.camera_room, &mut self.rng);
        }
        if self.shortcuts.divide() {
            self.update_shortcuts(observer);
        }
        for room in self.world.active_rooms().into_iter().rev() {
            self.update_room(room, input, observer);
        }
        self.world.finish_loading();
        self.tick += 1;
    }

    /// `cameras[0].followAbstractCreature` as the realizer reads it.
    fn follow_creature(&self) -> Option<FollowCreature> {
        let player = self.player.as_ref()?;
        Some(FollowCreature {
            room: self.pos.room,
            tile: self.pos.tile,
            in_room: !player.in_shortcut(),
            alive: !player.is_dead(),
        })
    }

    /// `ShortcutHandler.Update` for the player's vessel and the lobby.
    fn update_shortcuts(&mut self, observer: &mut dyn PlayerObserver) {
        let Some(player) = self.player.as_mut().filter(|p| !p.is_dead()) else {
            return;
        };
        if player.in_shortcut() && self.shortcuts.lobby.is_none() {
            let Some(room) = self.world.realized(self.pos.room).map(|r| r.room.clone()) else {
                log::warn!("vessel in an unrealized room; dropped");
                player.clear_vessel();
                return;
            };
            let sounds = if self.pos.room == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            match player.shortcut_vessel_update(&room, &mut self.rng, sounds) {
                VesselStep::SpatOut => observer.spat_out(player, &mut self.rng),
                VesselStep::Node { node, kind, from } => {
                    self.pos.node = node.map_or(-1, |n| i32::try_from(n).unwrap_or(-1));
                    if kind == Shortcut::RoomExit && !self.room_exit(node.unwrap_or(0), from) {
                        return;
                    }
                }
                VesselStep::Waited | VesselStep::Moved => {}
            }
        }
        self.update_lobby();
    }

    /// The room-exit case of `ShortcutHandler.Update`: the vessel joins the lobby for the
    /// connected room. Returns `false` where the game returns from the whole update (an
    /// exit leading nowhere: the vessel stalls and turns back).
    fn room_exit(&mut self, node: usize, from: (i32, i32)) -> bool {
        let connections = self
            .world
            .abstract_room(self.pos.room)
            .map(|r| r.connections.clone())
            .unwrap_or_default();
        let Some(player) = self.player.as_mut() else {
            return true;
        };
        if connections.is_empty() {
            log::warn!("room exit of a room without connections; the player is lost");
            self.player = None;
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
            .map_or(-1, |r| r.exit_index(self.pos.room));
        self.shortcuts.lobby = Some(crate::LobbyVessel {
            room: dest,
            entrance_node,
            updates: 0,
        });
        true
    }

    /// The `betweenRoomsWaitingLobby` loop: the destination is realized, then once it is
    /// `ReadyForPlayer` the player moves there, its vessel restarts at the entrance node
    /// and the camera moves to the node's camera.
    fn update_lobby(&mut self) {
        let Some(mut lobby) = self.shortcuts.lobby else {
            return;
        };
        lobby.updates += 1;
        self.shortcuts.lobby = Some(lobby);
        let Some(dest) = self.world.realized(lobby.room) else {
            if let Err(e) = self.world.activate_room(lobby.room) {
                log::warn!("cannot realize room {}: {e}", lobby.room);
            }
            return;
        };
        if !dest.ready_for_player() {
            return;
        }
        let node_tile = usize::try_from(lobby.entrance_node)
            .ok()
            .and_then(|n| dest.room.shortcut_map().nodes().get(n).copied());
        let node = self.world.abstract_room(lobby.room).and_then(|r| {
            r.nodes
                .get(usize::try_from(lobby.entrance_node).ok()?)
                .copied()
        });
        let Some(node) = node else {
            // The game indexes `nodes[-1]` here and throws every update.
            log::warn!(
                "room {} has no exit back; the vessel stays in the lobby",
                lobby.room
            );
            return;
        };
        self.pos = WorldCoordinate {
            room: lobby.room,
            tile: (-1, -1),
            node: lobby.entrance_node,
        };
        if matches!(
            node.kind,
            NodeType::Exit | NodeType::Den | NodeType::RegionTransportation
        ) && let (Some(player), Some(tile)) = (self.player.as_mut(), node_tile)
        {
            player.set_vessel_at(tile);
        }
        log::info!(
            "entered room {} after {} shortcut updates in the lobby",
            self.world
                .abstract_room(lobby.room)
                .map_or("?", |r| r.name.as_str()),
            lobby.updates
        );
        self.camera_room = lobby.room;
        self.camera_move = Some(CameraMove {
            room: lobby.room,
            camera: usize::try_from(node.viewed_by_camera).unwrap_or(0),
        });
        self.sounds.owner_removed();
        self.shortcuts.lobby = None;
    }

    /// `Room.Update` of active room `index`: the entry delay, the water (pushed by the
    /// player's body when it is here), then the player and its graphics.
    fn update_room(&mut self, index: i32, input: Input, observer: &mut dyn PlayerObserver) {
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        realized.update_wait();
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let here = self
            .player
            .as_mut()
            .filter(|p| self.pos.room == index && !p.in_shortcut() && !p.is_dead());
        let room = realized.room.clone();
        if let Some(water) = &mut realized.water {
            let chunks = match here {
                Some(p) => &mut p.body.chunks[..],
                None => &mut [][..],
            };
            water.update(&room, chunks.iter_mut(), &mut self.rng, sounds);
        }
        let Some(player) = self
            .player
            .as_mut()
            .filter(|p| self.pos.room == index && !p.in_shortcut() && !p.is_dead())
        else {
            return;
        };
        let was_stunned = player.is_stunned();
        player.update(&room, realized.water.as_ref(), input, &mut self.rng, sounds);
        observer.updated(player, &room, realized.water.as_ref(), &mut self.rng);
        self.pos.tile = player.head().pos.tile();
        if let Some(entrance) = player.shortcut_vessel_tile() {
            self.sucked_in(&room, entrance);
            return;
        }
        let Some(player) = self.player.as_mut() else {
            return;
        };
        if player.is_destroyed() {
            sounds.owner_removed();
            log::info!("player fell out of the room and was destroyed; R respawns");
            self.player = None;
        } else if player.is_dead() {
            log::info!("player died; R respawns");
        } else if player.is_stunned() && !was_stunned {
            log::info!("player stunned for {} ticks", player.stun_ticks());
        }
    }

    /// `ShortcutHandler.SuckInCreature` for the player going into a room exit: the
    /// connected room is activated at once (`connections[destNode]`, unless -1).
    fn sucked_in(&mut self, room: &Room, entrance: (i32, i32)) {
        let Some(data) = room.shortcut_map().data_at(entrance) else {
            return;
        };
        if data.kind != ShortcutType::RoomExit {
            return;
        }
        let dest = self
            .world
            .abstract_room(self.pos.room)
            .and_then(|r| r.connections.get(data.dest_node?).copied());
        if let Some(dest) = dest.filter(|&d| d > -1)
            && let Err(e) = self.world.activate_room(dest)
        {
            log::warn!("cannot realize room {dest}: {e}");
        }
    }

    /// The camera move since the last call (the first room, then each room entered).
    pub fn take_camera_move(&mut self) -> Option<CameraMove> {
        self.camera_move.take()
    }

    #[must_use]
    pub fn campaign(&self) -> &'static SlugcatCampaign {
        self.campaign
    }

    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    #[must_use]
    pub fn realizer(&self) -> Option<&RoomRealizer> {
        self.realizer.as_ref()
    }

    /// `cameras[0].room` (global index).
    #[must_use]
    pub fn camera_room(&self) -> i32 {
        self.camera_room
    }

    /// The player's abstract position.
    #[must_use]
    pub fn player_pos(&self) -> WorldCoordinate {
        self.pos
    }

    #[must_use]
    pub fn lobby(&self) -> Option<crate::LobbyVessel> {
        self.shortcuts.lobby
    }

    /// The player, unless destroyed.
    #[must_use]
    pub fn player(&self) -> Option<&Player> {
        self.player.as_ref()
    }

    /// Whether the player is in the camera's room (its body or its vessel).
    #[must_use]
    pub fn player_in_camera_room(&self) -> bool {
        self.player.is_some() && self.pos.room == self.camera_room
    }

    #[must_use]
    pub fn next_issued_id(&self) -> i32 {
        self.next_issued_id
    }

    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.tick
    }

    pub fn sounds_mut(&mut self) -> &mut Sounds {
        &mut self.sounds
    }

    #[must_use]
    pub fn rng(&self) -> &UnityRandom {
        &self.rng
    }

    /// Hands the random stream back (to the menus, when the game ends).
    pub fn take_rng(&mut self) -> UnityRandom {
        std::mem::replace(&mut self.rng, UnityRandom::new(0))
    }

    /// Puts the player (a new one if it was destroyed) into the camera's room out of any
    /// shortcut, its chunks 1 px above and below `pos` moving at `vel`. Movement state is
    /// kept; the observer is told of a new player.
    pub fn set_player(
        &mut self,
        pos: crate::Vec2,
        vel: crate::Vec2,
        observer: &mut dyn PlayerObserver,
    ) {
        let player = self.player.get_or_insert_with(Player::new);
        player.clear_vessel();
        self.shortcuts.lobby = None;
        // `place` resets the stuck-feet state; its random offsets are overwritten below.
        player.place(pos.tile(), &mut self.rng);
        for (chunk, dy) in player.body.chunks.iter_mut().zip([1.0, -1.0]) {
            chunk.reset_at(pos + crate::Vec2::new(0.0, dy));
            chunk.vel = vel;
        }
        self.pos = WorldCoordinate {
            room: self.camera_room,
            tile: pos.tile(),
            node: -1,
        };
        observer.created(player, &mut self.rng);
    }

    /// Kills the player (for tests and tools).
    pub fn kill_player(&mut self) {
        if let Some(p) = &mut self.player {
            p.die();
        }
    }
}
