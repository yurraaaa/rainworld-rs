//! `RainWorldGame` for one player: the world, the player's place in it, the shortcut
//! handler with its between-rooms lobby, the room realizer, and the order of a tick.
//!
//! Creatures: the world's population and its overseers' abstract AI
//! ([`crate::creature`]); the player is an `AbstractCreature` too. Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`, `shortcuts.md` section 5,
//! `creature-model.md`.

use rw_assets::{NodeType, Room, Shortcut, ShortcutType};
use rw_random::UnityRandom;

use crate::campaign::SlugcatCampaign;
use crate::creature::population::PopulationContext;
use crate::creature::{
    CreatureId, CreatureTemplate, EntityId, IdIssuer, PlayerGuideState, WorldCoordinate,
};
use crate::room_script::{RoomScript, ScriptTick, StorySession};
use crate::world::{AbstractTick, FollowCreature, RainCycle, RoomRealizer, World};
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
    /// A room script adds `dvx` to every tail segment's x velocity.
    fn nudge_tail_x(&mut self, _dvx: f32) {}
    /// A realized overseer is in the camera's room (`AddObject` or `NowViewed` builds its
    /// `OverseerGraphics`).
    fn overseer_viewed(&mut self, _id: EntityId, _size: f32, _rng: &mut UnityRandom) {}
    /// `OverseerGraphics.Reset` (`HardSetTile`).
    fn overseer_reset(&mut self, _id: EntityId, _rng: &mut UnityRandom) {}
    /// The camera left the overseer's room (`NoLongerViewed`).
    fn overseer_unviewed(&mut self, _id: EntityId) {}
}

/// No graphics.
impl PlayerObserver for () {
    fn created(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn updated(&mut self, _: &Player, _: &Room, _: Option<&Water>, _: &mut UnityRandom) {}
    fn spat_out(&mut self, _: &Player, _: &mut UnityRandom) {}
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

/// What [`RainWorldGame::new_game_world`] leaves for the rest of the constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGameWorld {
    pub ids: IdIssuer,
    pub save_state_seed: i32,
}

/// A running game.
pub struct RainWorldGame {
    campaign: &'static SlugcatCampaign,
    world: World,
    shortcuts: ShortcutHandler,
    /// `None` in a single-room world.
    realizer: Option<RoomRealizer>,
    player: Option<Player>,
    /// The player's `AbstractCreature` (`Players[0]`).
    player_id: CreatureId,
    /// `cameras[0].room`.
    camera_room: i32,
    camera_move: Option<CameraMove>,
    rng: UnityRandom,
    sounds: Sounds,
    /// Sounds of rooms the camera is not in: the game's microphone ignores them, so
    /// they make no draws.
    silent: Sounds,
    /// `nextIssuedId`.
    ids: IdIssuer,
    /// `saveState.seed` of the new game's `SaveState`.
    save_state_seed: i32,
    /// `saveState.miscWorldSaveData.playerGuideState`.
    guide: PlayerGuideState,
    /// `updateAbstractRoom`: the abstract room updated last.
    update_abstract_room: usize,
    /// The start tile, reused by respawns in the start room.
    start: Start,
    tick: u64,
    /// What the room scripts read of the process manager.
    script_tick: ScriptTick,
}

impl RainWorldGame {
    /// The constructor's order for the parts ported: `nextIssuedId`, the new game's
    /// `SaveState` seed, the world (`World`'s rain cycle; in a region, the population and
    /// the overseers; `FliesWorldAI`), the player's `AbstractCreature`, the first room
    /// activated and shown by camera 0, the realizer, then (the room loading at once) the
    /// room's creatures realized in list order, the player among them.
    pub fn new(
        world: World,
        campaign: &'static SlugcatCampaign,
        start: Start,
        mut rng: UnityRandom,
        sounds: Sounds,
        observer: &mut dyn PlayerObserver,
    ) -> Self {
        let mut world = world;
        let NewGameWorld {
            mut ids,
            save_state_seed,
        } = Self::new_game_world(&mut world, campaign, &mut rng);
        let session = StorySession {
            character: campaign.name,
            cycle_number: 0,
            den_position: campaign.start_den,
        };
        let guide = PlayerGuideState::default();
        let player_pos = WorldCoordinate::new(start.room, 15, 25, -1);
        let template = CreatureTemplate::of(rw_assets::CreatureType::SLUGCAT, world.mods());
        let mut player = world.new_creature(
            template,
            player_pos,
            EntityId::new(-1, 0),
            &session,
            &mut ids,
            &mut rng,
        );
        player.pos.tile = start.tile.unwrap_or(crate::campaign::DEFAULT_PLAYER_TILE);
        let player_id = world.insert_creature(player);
        world.add_entity(player_id);
        let realizer = (!world.single_room()).then(RoomRealizer::new);
        let mut game = Self {
            campaign,
            world,
            shortcuts: ShortcutHandler::new(),
            realizer,
            player: None,
            player_id,
            camera_room: start.room,
            camera_move: Some(CameraMove {
                room: start.room,
                camera: 0,
            }),
            rng,
            sounds,
            silent: Sounds::default(),
            ids,
            save_state_seed,
            guide,
            update_abstract_room: 0,
            start,
            tick: 0,
            script_tick: ScriptTick::default(),
        };
        if let Err(e) = game.world.activate_room(start.room) {
            log::error!("cannot realize the first room: {e}");
        }
        game.finish_loading(observer);
        game
    }

    /// The constructor's draws up to the world's: `nextIssuedId`, the new game's
    /// `SaveState` seed (`StoryGameSession`), then `OverWorld`'s world load: the `World`
    /// constructor's rain cycle and, in a region, `GeneratePopulation` and
    /// `FliesWorldAI`'s constructor.
    pub fn new_game_world(
        world: &mut World,
        campaign: &'static SlugcatCampaign,
        rng: &mut UnityRandom,
    ) -> NewGameWorld {
        let mut ids = IdIssuer {
            next_issued_id: rng.range_i32(1000, 10000),
        };
        let save_state_seed = rng.range_i32(0, 10000);
        let session = StorySession {
            character: campaign.name,
            cycle_number: 0,
            den_position: campaign.start_den,
        };
        let guide = PlayerGuideState::default();
        world.rain_cycle = RainCycle::new_story(campaign.name, rng);
        let mut population = PopulationContext {
            session: &session,
            ids: &mut ids,
            rng,
            guide_overseer_dead: false,
            guide: &guide,
            food_to_hibernate: campaign.stats.food_to_hibernate,
        };
        if !world.single_room() {
            world.generate_population(&mut population);
        }
        world.create_flies_world_ai(&mut population);
        NewGameWorld {
            ids,
            save_state_seed,
        }
    }

    /// The player's `AbstractCreature.pos`.
    fn pos(&self) -> WorldCoordinate {
        self.world.creature(self.player_id).pos
    }

    fn set_pos(&mut self, pos: WorldCoordinate) {
        self.world.place_creature(self.player_id, pos);
    }

    /// `AbstractCreature.RealizeInRoom` for the player at the start: an undefined node
    /// is drawn among the room's exits and dens (`NodesRelevantToCreature`), then the
    /// `Player` is built and placed.
    fn realize_player(&mut self, observer: &mut dyn PlayerObserver) {
        let mut pos = self.pos();
        let Some(room) = self.world.realized(pos.room).map(|r| r.room.clone()) else {
            return;
        };
        let tile = match self.start.tile {
            Some(tile) => tile,
            None => determine_safe_spawn_tile(&room, &mut self.rng),
        };
        if pos.node < 0 {
            let relevant = self.world.abstract_room(pos.room).map_or(0, |r| {
                r.nodes
                    .iter()
                    .filter(|n| matches!(n.kind, NodeType::Exit | NodeType::Den))
                    .count()
            });
            let specific = self
                .rng
                .range_i32(0, i32::try_from(relevant).unwrap_or(i32::MAX));
            pos.node = self.common_node(specific);
        }
        pos.tile = tile;
        self.set_pos(pos);
        self.spawn_player(tile, observer);
    }

    /// `CreatureSpecificToCommonNodeIndex` for the slugcat (exit and den nodes).
    fn common_node(&self, specific: i32) -> i32 {
        self.world
            .abstract_room(self.pos().room)
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
        let node = self.pos().node;
        self.set_pos(WorldCoordinate::new(self.camera_room, tile.0, tile.1, node));
        self.spawn_player(tile, observer);
        log::info!("respawned player");
    }

    /// One 40 Hz tick in `RainWorldGame.Update`'s order: the room realizer, one
    /// abstract room (in turn, with the time of a whole round), the rain cycle, the
    /// shortcut handler on every third tick, every active room (newest first: the entry
    /// delay, its water, and the player in it with its graphics), then the loading
    /// step. A dead player is frozen (docs/DIVERGENCES.md D4).
    pub fn tick(&mut self, input: Input, observer: &mut dyn PlayerObserver) {
        let follow = self.follow_creature();
        if let (Some(realizer), Some(follow)) = (&mut self.realizer, follow) {
            realizer.update(&mut self.world, follow, self.camera_room, &mut self.rng);
        }
        self.update_abstract_room();
        self.world.rain_cycle.update(&mut self.rng);
        if self.shortcuts.divide() {
            self.update_shortcuts(observer);
        }
        for room in self.world.active_rooms().into_iter().rev() {
            self.update_room(room, input, observer);
        }
        self.finish_loading(observer);
        self.sync_player_state();
        self.tick += 1;
    }

    /// The story branch of `RainWorldGame.Update`: `updateAbstractRoom` steps to the next
    /// room, which updates with `NumberOfRooms` ticks. (`FliesWorldAI.Update`, which
    /// follows, is not ported: docs/DIVERGENCES.md D1.)
    fn update_abstract_room(&mut self) {
        let count = self.world.number_of_rooms();
        self.update_abstract_room += 1;
        if self.update_abstract_room >= count {
            self.update_abstract_room = 0;
        }
        let room =
            self.world.first_room_index() + i32::try_from(self.update_abstract_room).unwrap_or(0);
        let time = i32::try_from(count).unwrap_or(i32::MAX);
        let mut cx = AbstractTick {
            rng: &mut self.rng,
            guide: &self.guide,
            player: Some(self.player_id),
            player_karma: self.campaign.start_karma,
        };
        self.world.update_abstract_room(room, time, &mut cx);
    }

    /// The player's `AbstractCreature.state.alive` follows its body.
    fn sync_player_state(&mut self) {
        if let Some(p) = &self.player {
            let dead = p.is_dead();
            self.world.creature_mut(self.player_id).state.alive = !dead;
        }
    }

    /// The loading step: for each room that finished loading, `ReadyForAI` realizes its
    /// creatures in list order (the player only at the start), and `Room.Loaded`'s
    /// `AddRoomSpecificScript` adds its scripts.
    fn finish_loading(&mut self, observer: &mut dyn PlayerObserver) {
        let session = self.story_session();
        for index in self.world.finish_loading() {
            let viewed = index == self.camera_room;
            for id in self.world.room_creatures(index).to_vec() {
                if id == self.player_id {
                    if self.tick == 0 && self.player.is_none() {
                        self.realize_player(observer);
                    }
                } else {
                    self.world
                        .realize_overseer(id, viewed, observer, &mut self.rng);
                }
            }
            let name = self
                .world
                .abstract_room(index)
                .map(|r| r.name.clone())
                .unwrap_or_default();
            let mods = self.world.mods();
            let Some(room) = self.world.realized_mut(index) else {
                continue;
            };
            if room.settings.room_specific_script {
                room.scripts =
                    RoomScript::for_room(&name, Some(&session), mods, room.first_time_realized());
            }
        }
    }

    /// The story session as the room scripts read it: a new game's first cycle in the
    /// campaign's start den (the port has no saves).
    #[must_use]
    pub fn story_session(&self) -> StorySession {
        StorySession {
            character: self.campaign.name,
            cycle_number: 0,
            den_position: self.campaign.start_den,
        }
    }

    /// What the room scripts see of the process manager from the next tick on
    /// (`FadeDelayInProgress`).
    pub fn set_fade_delay_in_progress(&mut self, in_progress: bool) {
        self.script_tick.fade_delay_in_progress = in_progress;
    }

    /// Whether camera 0 has a HUD, as the room scripts read it from the next tick on.
    pub fn set_hud_present(&mut self, present: bool) {
        self.script_tick.hud_with_control_preset = present;
    }

    /// The camera's sounds and the shared random stream, for what camera 0 updates
    /// before the rooms (the HUD).
    pub fn camera_io(&mut self) -> (&mut Sounds, &mut UnityRandom) {
        (&mut self.sounds, &mut self.rng)
    }

    /// `cameras[0].followAbstractCreature` as the realizer reads it.
    fn follow_creature(&self) -> Option<FollowCreature> {
        let player = self.player.as_ref()?;
        let pos = self.pos();
        Some(FollowCreature {
            room: pos.room,
            tile: pos.tile,
            in_room: !player.in_shortcut(),
            alive: !player.is_dead(),
        })
    }

    /// `ShortcutHandler.Update` for the player's vessel and the lobby.
    fn update_shortcuts(&mut self, observer: &mut dyn PlayerObserver) {
        let here = self.pos().room;
        let Some(player) = self.player.as_mut().filter(|p| !p.is_dead()) else {
            return;
        };
        if player.in_shortcut() && self.shortcuts.lobby.is_none() {
            let Some(room) = self.world.realized(here).map(|r| r.room.clone()) else {
                log::warn!("vessel in an unrealized room; dropped");
                player.clear_vessel();
                return;
            };
            let sounds = if here == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            match player.shortcut_vessel_update(&room, &mut self.rng, sounds) {
                VesselStep::SpatOut => observer.spat_out(player, &mut self.rng),
                VesselStep::Node { node, kind, from } => {
                    let mut pos = self.world.creature(self.player_id).pos;
                    pos.node = node.map_or(-1, |n| i32::try_from(n).unwrap_or(-1));
                    self.world.place_creature(self.player_id, pos);
                    if kind == Shortcut::RoomExit && !self.room_exit(node.unwrap_or(0), from) {
                        return;
                    }
                }
                VesselStep::Waited | VesselStep::Moved => {}
            }
        }
        self.update_lobby(observer);
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
        self.shortcuts.lobby = Some(crate::LobbyVessel {
            room: dest,
            entrance_node,
            updates: 0,
        });
        true
    }

    /// The `betweenRoomsWaitingLobby` loop: the destination is realized, then once it is
    /// `ReadyForPlayer` the player moves there, its vessel restarts at the entrance node
    /// and the camera moves to the node's camera (the rooms' `NoLongerViewed` and
    /// `NowViewed` for the overseers' graphics).
    fn update_lobby(&mut self, observer: &mut dyn PlayerObserver) {
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
        }
        log::info!(
            "entered room {} after {} shortcut updates in the lobby",
            self.world
                .abstract_room(lobby.room)
                .map_or("?", |r| r.name.as_str()),
            lobby.updates
        );
        self.world
            .camera_changed_room(self.camera_room, lobby.room, observer, &mut self.rng);
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
        let player_room = self.pos().room;
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
            .filter(|p| player_room == index && !p.in_shortcut() && !p.is_dead());
        let room = realized.room.clone();
        if let Some(water) = &mut realized.water {
            let chunks = match here {
                Some(p) => &mut p.body.chunks[..],
                None => &mut [][..],
            };
            water.update(&room, chunks.iter_mut(), &mut self.rng, sounds);
        }
        self.update_room_player(index, &room, input, observer);
        self.update_room_scripts(index, observer);
    }

    /// The room's scripts, after the player (`updateList` runs newest first, and the
    /// scripts are added when the room loads).
    fn update_room_scripts(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let here = self.pos().room == index;
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        for script in &mut realized.scripts {
            let player = self
                .player
                .as_mut()
                .filter(|p| here && !p.in_shortcut() && !p.is_destroyed());
            script.update(player, self.script_tick, observer);
        }
    }

    /// The player's update in room `index`, when its body is there and alive.
    fn update_room_player(
        &mut self,
        index: i32,
        room: &Room,
        input: Input,
        observer: &mut dyn PlayerObserver,
    ) {
        let player_room = self.pos().room;
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let Some(player) = self
            .player
            .as_mut()
            .filter(|p| player_room == index && !p.in_shortcut() && !p.is_dead())
        else {
            return;
        };
        let was_stunned = player.is_stunned();
        player.update(room, realized.water.as_ref(), input, &mut self.rng, sounds);
        observer.updated(player, room, realized.water.as_ref(), &mut self.rng);
        let tile = player.head().pos.tile();
        let mut pos = self.world.creature(self.player_id).pos;
        pos.tile = tile;
        self.world.place_creature(self.player_id, pos);
        let Some(player) = self.player.as_mut() else {
            return;
        };
        if let Some(entrance) = player.shortcut_vessel_tile() {
            self.sucked_in(room, entrance);
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
            .abstract_room(self.pos().room)
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
        self.pos()
    }

    /// The player's `AbstractCreature`.
    #[must_use]
    pub fn player_creature(&self) -> CreatureId {
        self.player_id
    }

    /// `saveState.seed`.
    #[must_use]
    pub fn save_state_seed(&self) -> i32 {
        self.save_state_seed
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
        self.player.is_some() && self.pos().room == self.camera_room
    }

    #[must_use]
    pub fn next_issued_id(&self) -> i32 {
        self.ids.next_issued_id
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
        let tile = pos.tile();
        self.world.place_creature(
            self.player_id,
            WorldCoordinate::new(self.camera_room, tile.0, tile.1, -1),
        );
        observer.created(player, &mut self.rng);
    }

    /// Kills the player (for tests and tools).
    pub fn kill_player(&mut self) {
        if let Some(p) = &mut self.player {
            p.die();
        }
    }
}
