//! `RainWorldGame` for one player: the world, the player's place in it, the shortcut
//! handler with its between-rooms lobby, the room realizer, and the order of a tick.
//!
//! Creatures: the world's population and its overseers' abstract AI
//! ([`crate::creature`]); the player is an `AbstractCreature` too. Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`, `shortcuts.md` section 5,
//! `creature-model.md`.

use std::collections::HashMap;

#[path = "game_flies.rs"]
mod flies;
pub use flies::{FliesCall, FliesCallRecord};

use rw_assets::{CreatureType, NodeType, Room, Shortcut, ShortcutType};
use rw_random::UnityRandom;

use crate::camera::CameraView;
use crate::campaign::SlugcatCampaign;
use crate::creature::overseer::hologram::{ControlPreset, OverseerHologram};
use crate::creature::overseer::{
    OverseerEffect, OverseerMode, OverseerTick, OverseerUpdate, OverseerView, PlayerView,
    StoryStats, TutorialMessage,
};
use crate::creature::population::PopulationContext;
use crate::creature::{
    AbstractCreature, CreatureId, CreatureTemplate, EntityId, IdIssuer, PlayerGuideState,
    RealizedCreature, WorldCoordinate,
};
use crate::room_script::{RoomScript, ScriptTick, StorySession};
use crate::world::{AbstractTick, FollowCreature, RainCycle, RoomRealizer, World};
use crate::{
    EntranceColor, Input, LobbyVessel, Player, ShortcutHandler, Sounds, Vec2, VesselStep, Water,
    determine_safe_spawn_tile,
};

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
    /// A realized overseer is in the camera's room (`AddObject` or `NowViewed`): its
    /// `OverseerGraphics` is built unless it has one.
    fn overseer_added(&mut self, _view: &OverseerView<'_>, _rng: &mut UnityRandom) {}
    /// `OverseerGraphics.Reset`.
    fn overseer_reset(&mut self, _view: &OverseerView<'_>, _rng: &mut UnityRandom) {}
    /// Right after an overseer hologram's `Update` (the parts lerp their colours).
    fn hologram_updated(&mut self, _owner: EntityId, _hologram: &OverseerHologram) {}
    /// `OverseerGraphics.Emerge`.
    fn overseer_emerge(&mut self, _view: &OverseerView<'_>, _rng: &mut UnityRandom) {}
    /// Right after `Overseer.Update`: the graphics module's update.
    fn overseer_updated(
        &mut self,
        _view: &OverseerView<'_>,
        _room: &Room,
        _camera: &CameraView,
        _rng: &mut UnityRandom,
    ) {
    }
    /// The overseer's graphics module is dropped (`NoLongerViewed`, or abstracted).
    fn overseer_removed(&mut self, _id: EntityId) {}
}

/// What the game tells camera 0's HUD.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HudEvent {
    /// The guide's `TutorialText`.
    Message(TutorialMessage),
    /// `rainMeter.halfTimeBlink = 0`.
    HalfTimeBlinkReset,
}

/// Which overseer call [`RainWorldGame::run_overseer`] makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverseerCall {
    /// `PlaceInRoom` after `ReadyForAI` realized it.
    Place,
    /// `TeleportingIntoRoom` from the lobby.
    TeleportIn,
    /// `Update` and its graphics update.
    Update,
    /// The `Update` of the overseer's hologram with this id.
    Hologram(u64),
}

/// An object in a room's update list the port runs besides the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoomObject {
    Overseer(CreatureId),
    Hologram { owner: CreatureId, id: u64 },
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
    /// The random state right before and after `FliesWorldAI`'s constructor.
    pub flies_rng: [[u32; 4]; 2],
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
    /// Camera 0's positions as the app's room camera last reported them.
    camera_view: CameraView,
    /// The player's place in its room's `updateList`.
    player_seq: u64,
    hud_events: Vec<HudEvent>,
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
    /// The realized overseer calls recorded since `record_overseer_calls(true)`.
    overseer_calls: Option<Vec<OverseerCallRecord>>,
    /// The last overseer realized into a room, always recorded.
    last_overseer_place: Option<OverseerCallRecord>,
    /// The last hologram id issued.
    hologram_ids: u64,
    /// The player's `playerState.foodInStomach` (nothing is eaten yet).
    food_in_stomach: i32,
    /// Each realized creature's `Creature.lastCoord`; a new body starts at the default
    /// coordinate (room 0, tile (0, 0)).
    last_coords: HashMap<CreatureId, WorldCoordinate>,
    /// `ColorEntrance` calls on camera 0 since the last `take_entrance_colors`.
    entrance_colors: Vec<EntranceColor>,
    /// The flies' calls recorded since `record_flies_calls(true)`.
    flies_calls: Option<Vec<FliesCallRecord>>,
    flies_ctor_rng: [[u32; 4]; 2],
}

/// One realized overseer call with what it read and the random stream around it (for
/// comparing against a reference model).
#[derive(Debug, Clone)]
pub struct OverseerCallRecord {
    pub tick: u64,
    pub creature: CreatureId,
    /// `Place` covers `RealizeInRoom` from `PlaceInRealizedRoom` on.
    pub call: OverseerCall,
    pub rng_before: [u32; 4],
    pub rng_after: [u32; 4],
    pub player: Option<PlayerView>,
    /// The player's `abstractCreature.pos`.
    pub player_pos: WorldCoordinate,
    pub player_id: CreatureId,
    /// The player's `bodyChunks[1].lastPos`.
    pub player_hips_last: Vec2,
    pub camera: CameraView,
    pub rain_timer_before: i32,
    pub rain_timer_after: i32,
    pub cycle_length: i32,
    pub hud: bool,
    pub viewed: bool,
    /// The room's `abstractRoom.creatures`.
    pub creatures: Vec<CreatureId>,
    /// For `place`, the abstract creature before it was realized.
    pub abstract_before: Option<AbstractCreature>,
    /// Its abstract position as the call began.
    pub pos_before: WorldCoordinate,
    /// The abstract creature (with its body) after a `Place` or `TeleportIn`.
    pub after: Option<AbstractCreature>,
    /// The overseer's holograms after the call (the deleted ones gone), and its current
    /// one's id.
    pub holograms: Vec<OverseerHologram>,
    pub current_hologram: Option<u64>,
    /// The tutorial texts the call queued.
    pub messages: Vec<TutorialMessage>,
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
            flies_rng,
        } = Self::new_game_world(&mut world, campaign, &mut rng);
        let session = StorySession {
            character: campaign.name,
            cycle_number: 0,
            den_position: campaign.start_den,
        };
        let guide = PlayerGuideState::default();
        let player_pos = WorldCoordinate::new(start.room, 15, 25, -1);
        let template = CreatureTemplate::of(rw_assets::CreatureType::Slugcat, world.mods());
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
            camera_view: CameraView {
                room: start.room,
                current: 0,
                next: 0,
            },
            player_seq: 0,
            hud_events: Vec::new(),
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
            overseer_calls: None,
            last_overseer_place: None,
            hologram_ids: 0,
            food_in_stomach: 0,
            last_coords: HashMap::new(),
            entrance_colors: Vec::new(),
            flies_calls: None,
            flies_ctor_rng: flies_rng,
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
        let flies_rng_before = population.rng.state();
        world.create_flies_world_ai(&mut population);
        let flies_rng = [flies_rng_before, rng.state()];
        NewGameWorld {
            ids,
            save_state_seed,
            flies_rng,
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
        self.last_coords.remove(&self.player_id);
        self.player_seq = self.world.next_update_seq();
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
        self.shortcuts.lobby.retain(|v| v.teleport.is_some());
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
        let active_before = self.world.active_rooms();
        if let (Some(realizer), Some(follow)) = (&mut self.realizer, follow) {
            realizer.update(&mut self.world, follow, self.camera_room, &mut self.rng);
        }
        self.record_abstractized(&active_before);
        self.update_abstract_room();
        self.update_world_processes();
        self.enter_from_abstract_rooms();
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
    /// room, which updates with `NumberOfRooms` ticks.
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
            cycle: 0,
        };
        self.world.update_abstract_room(room, time, &mut cx);
    }

    /// `CreatureEnterFromAbstractRoom` for the creatures `ChangeRooms` realized: each
    /// waits in the lobby in a teleportation vessel.
    fn enter_from_abstract_rooms(&mut self) {
        for (id, to) in std::mem::take(&mut self.world.entering) {
            self.shortcuts.lobby.push(LobbyVessel {
                room: to.room,
                entrance_node: to.node,
                updates: 0,
                teleport: Some(id),
            });
        }
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
            for id in self.world.room_creatures(index).to_vec() {
                if id == self.player_id {
                    if self.tick == 0 && self.player.is_none() {
                        self.realize_player(observer);
                    }
                } else {
                    let before = self.rng.state();
                    let creature = self.world.creature(id).clone();
                    if self.world.realize_overseer(id, 0, &mut self.rng) {
                        self.run_overseer(id, OverseerCall::Place, observer);
                        self.mark_overseer_place(before, creature);
                    }
                }
            }
            self.place_quantified(index);
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
        if self.update_player_vessel(observer) {
            self.update_lobby(observer);
        }
    }

    /// Whether the player's vessel waits in the lobby.
    fn player_in_lobby(&self) -> bool {
        self.shortcuts.lobby.iter().any(|v| v.teleport.is_none())
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
                observer.spat_out(player, &mut self.rng);
                self.player_seq = self.world.next_update_seq();
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
            teleport: None,
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
            let done = match vessel.teleport {
                Some(id) => self.teleport_vessel_step(id, vessel, observer),
                None => self.player_vessel_step(i, vessel, observer),
            };
            if done {
                self.shortcuts.lobby.remove(i);
            }
        }
    }

    /// An overseer's teleportation vessel: into its destination once that room is
    /// `ReadyForPlayer` and `readyForAI` (`TeleportingIntoRoom`), or abstracted there if
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
                observer.overseer_removed(self.world.creature(id).id);
                self.world.abstractize_to(id, coord);
                true
            }
            Some(r) if r.ready_for_player() && r.ready_for_ai() => {
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
    fn player_vessel_step(
        &mut self,
        index: usize,
        mut lobby: LobbyVessel,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        lobby.updates += 1;
        self.shortcuts.lobby[index] = lobby;
        let Some(dest) = self.world.realized(lobby.room) else {
            if let Err(e) = self.world.activate_room(lobby.room) {
                log::warn!("cannot realize room {}: {e}", lobby.room);
            }
            return false;
        };
        if !dest.ready_for_player() {
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
            if node.kind == NodeType::Exit {
                self.blink_arrival(lobby);
            }
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
        let camera = usize::try_from(node.viewed_by_camera).unwrap_or(0);
        self.camera_move = Some(CameraMove {
            room: lobby.room,
            camera,
        });
        self.camera_view = CameraView {
            room: lobby.room,
            current: camera,
            next: camera,
        };
        self.sounds.owner_removed();
        true
    }

    /// A slugcat's vessel came out of the lobby into exit node `lobby.entrance_node`:
    /// the room blinks the entrance leading there by 2.5, which camera 0, if it already
    /// shows the room, colours as the slugcat.
    fn blink_arrival(&mut self, lobby: LobbyVessel) {
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
            b.blink(Some(entrance), None, 2.5);
        }
        if self.camera_room == lobby.room {
            self.entrance_colors.push(EntranceColor {
                room: lobby.room,
                entrance,
                creature: CreatureType::Slugcat,
            });
        }
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
        if let Some(blinking) = &mut realized.shortcuts_blinking {
            blinking.update(&room, &mut self.rng);
        }
        let mut objects: Vec<(u64, RoomObject)> = self
            .world
            .room_overseers(index)
            .into_iter()
            .map(|(seq, id)| (seq, RoomObject::Overseer(id)))
            .collect();
        objects.extend(
            self.world
                .room_holograms(index)
                .into_iter()
                .map(|(seq, owner, id)| (seq, RoomObject::Hologram { owner, id })),
        );
        objects.sort_by_key(|&(seq, _)| std::cmp::Reverse(seq));
        let mut player_done = false;
        for (seq, object) in objects {
            if !player_done && self.player_seq > seq {
                self.update_room_player(index, &room, input, observer);
                player_done = true;
            }
            match object {
                RoomObject::Overseer(id) => {
                    let in_room = self
                        .world
                        .creature(id)
                        .overseer_body()
                        .is_some_and(|b| b.in_room)
                        && self.world.creature(id).pos.room == index;
                    if in_room {
                        self.creature_new_tile(id);
                        self.run_overseer(id, OverseerCall::Update, observer);
                    }
                }
                RoomObject::Hologram { owner, id } => {
                    let live = self
                        .world
                        .creature(owner)
                        .overseer_body()
                        .is_some_and(|b| b.holograms.iter().any(|h| h.id == id && !h.slated));
                    if live {
                        self.run_overseer(owner, OverseerCall::Hologram(id), observer);
                    }
                }
            }
        }
        if !player_done {
            self.update_room_player(index, &room, input, observer);
        }
        self.update_room_scripts(index, observer);
    }

    /// The player as a realized overseer in room `room` reads it.
    fn player_view(&self, room: i32) -> Option<PlayerView> {
        let p = self.player.as_ref()?;
        let inputs = p.input_history();
        let chunks = &p.body.chunks;
        Some(PlayerView {
            in_room: self.pos().room == room && !p.in_shortcut() && !p.is_destroyed(),
            pos: chunks[0].pos,
            last_pos: chunks[0].last_pos,
            hips: chunks[1].pos,
            dead: p.is_dead(),
            conscious: !p.is_dead() && !p.is_stunned(),
            food: self.food_in_stomach,
            jump: [inputs[0].jump, inputs[1].jump],
            entering_shortcut: p.entering_shortcut(),
            in_shortcut: p.in_shortcut(),
            standing: p.is_standing(),
            animation: p.animation(),
            hips_contact_y: chunks[1].contact().y,
            input_x: inputs[0].x,
            input_y: inputs[0].y,
        })
    }

    fn story_stats(&self) -> StoryStats {
        let stats = &self.campaign.stats;
        StoryStats {
            cycle: 0,
            food_to_hibernate: stats.food_to_hibernate,
            max_food: stats.max_food,
            karma_cap: self.campaign.start_karma_cap,
            yellow: self.campaign.name == rw_assets::SlugcatName::Yellow,
            how_well_is_player_doing: -1.0,
        }
    }

    /// One call into realized overseer `id` in its room, with what it reads of the game,
    /// then what it did to the rest of the game.
    fn run_overseer(
        &mut self,
        id: CreatureId,
        call: OverseerCall,
        observer: &mut dyn PlayerObserver,
    ) {
        let room_index = match call {
            OverseerCall::Hologram(hid) => self
                .world
                .creature(id)
                .overseer_body()
                .and_then(|b| b.holograms.iter().find(|h| h.id == hid))
                .map_or(-1, |h| h.room),
            _ => self.world.creature(id).pos.room,
        };
        let player = self.player_view(room_index);
        let story = self.story_stats();
        let wanted = self.overseer_calls.is_some() || call == OverseerCall::Place;
        let record = wanted.then(|| OverseerCallRecord {
            tick: self.tick,
            creature: id,
            call,
            rng_before: self.rng.state(),
            rng_after: [0; 4],
            player,
            player_pos: self.pos(),
            player_id: self.player_id,
            player_hips_last: self
                .player
                .as_ref()
                .map_or(Vec2::ZERO, |p| p.body.chunks[1].last_pos),
            camera: self.camera_view,
            rain_timer_before: self.world.rain_cycle.timer,
            rain_timer_after: 0,
            cycle_length: self.world.rain_cycle.cycle_length,
            hud: self.script_tick.hud_with_control_preset,
            viewed: room_index == self.camera_room,
            creatures: self.world.room_creatures(room_index).to_vec(),
            abstract_before: None,
            pos_before: self.world.creature(id).pos,
            after: None,
            holograms: Vec::new(),
            current_hologram: None,
            messages: Vec::new(),
        });
        let Some(mut parts) = self.world.take_overseer(id) else {
            return;
        };
        let mut pos = self.world.creature(id).pos;
        let entity = self.world.creature(id).id;
        let ignore_cycle = self.world.creature(id).ignore_cycle;
        let mut rain = self.world.rain_cycle;
        let mut effects = Vec::new();
        if matches!(call, OverseerCall::Place | OverseerCall::TeleportIn) {
            parts.body.update_order = self.world.next_update_seq();
        }
        if let Some(realized) = self.world.realized(room_index)
            && let Some(ai_map) = realized.ai_map.as_ref()
        {
            let sounds = if room_index == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            let mut cx = OverseerTick {
                world: &self.world,
                room_index,
                room: &realized.room,
                ai_map,
                rng: &mut self.rng,
                sounds,
                observer,
                guide: &mut self.guide,
                control_preset: ControlPreset::Keyboard,
                hologram_ids: &mut self.hologram_ids,
                player_id: self.player_id,
                player,
                player_karma: self.campaign.start_karma,
                camera: self.camera_view,
                story,
                rain,
                hud: self.script_tick.hud_with_control_preset,
                viewed: room_index == self.camera_room,
                effects: Vec::new(),
            };
            let mut run = OverseerUpdate {
                me: id,
                id: entity,
                pos,
                ignore_cycle,
                body: &mut parts.body,
                ai: &mut parts.ai,
                abs: &mut parts.abs,
                base: &mut parts.base,
                cx: &mut cx,
            };
            match call {
                OverseerCall::Place => {
                    self.last_coords.remove(&id);
                    run.place_in_room();
                }
                OverseerCall::TeleportIn => run.teleport_into_room(),
                OverseerCall::Update => run.update_with_graphics(),
                OverseerCall::Hologram(hid) => run.update_hologram(hid),
            }
            pos = run.pos;
            rain = cx.rain;
            effects = cx.effects;
        }
        for h in &mut parts.body.holograms {
            if h.update_order == 0 {
                h.update_order = self.world.next_update_seq();
            }
        }
        if let OverseerCall::Hologram(hid) = call
            && let Some(h) = parts.body.holograms.iter().find(|h| h.id == hid)
        {
            observer.hologram_updated(self.world.creature(id).id, h);
        }
        let current = parts.body.hologram;
        parts
            .body
            .holograms
            .retain(|h| !h.slated || Some(h.id) == current);
        let holograms = record
            .as_ref()
            .map(|_| parts.body.holograms.clone())
            .unwrap_or_default();
        self.world.put_overseer(id, parts, pos);
        self.world.rain_cycle = rain;
        if let Some(mut record) = record {
            record.rng_after = self.rng.state();
            record.rain_timer_after = rain.timer;
            if matches!(call, OverseerCall::Place | OverseerCall::TeleportIn) {
                record.after = Some(self.world.creature(id).clone());
            }
            record.holograms = holograms;
            record.messages = effects
                .iter()
                .filter_map(|e| match e {
                    OverseerEffect::Message(m) => Some(*m),
                    _ => None,
                })
                .collect();
            record.current_hologram = current;
            if call == OverseerCall::Place {
                self.last_overseer_place = Some(record.clone());
            }
            if let Some(calls) = &mut self.overseer_calls {
                calls.push(record);
            }
        }
        self.apply_overseer_effects(id, effects, observer);
    }

    /// The last call recorded was `Place`: it began at `rng_before` (before
    /// `PlaceInRealizedRoom`).
    fn mark_overseer_place(&mut self, rng_before: [u32; 4], creature: AbstractCreature) {
        let recorded = self.overseer_calls.as_mut().and_then(|c| c.last_mut());
        for last in [self.last_overseer_place.as_mut(), recorded]
            .into_iter()
            .flatten()
        {
            last.rng_before = rng_before;
            last.abstract_before = Some(creature.clone());
        }
    }

    /// The last realization of an overseer into a room (`RealizeInRoom` through
    /// `PlaceInRoom`), recorded whether or not calls are.
    #[must_use]
    pub fn last_overseer_place(&self) -> Option<&OverseerCallRecord> {
        self.last_overseer_place.as_ref()
    }

    /// Starts (or stops and clears) recording the realized overseer calls.
    pub fn record_overseer_calls(&mut self, on: bool) {
        self.overseer_calls = on.then(Vec::new);
    }

    /// The overseer calls recorded since the last call.
    pub fn take_overseer_calls(&mut self) -> Vec<OverseerCallRecord> {
        self.overseer_calls
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    fn apply_overseer_effects(
        &mut self,
        id: CreatureId,
        effects: Vec<OverseerEffect>,
        observer: &mut dyn PlayerObserver,
    ) {
        let _ = observer;
        for effect in effects {
            match effect {
                OverseerEffect::Message(m) => self.hud_events.push(HudEvent::Message(m)),
                OverseerEffect::HalfTimeBlinkReset => {
                    self.hud_events.push(HudEvent::HalfTimeBlinkReset);
                }
                OverseerEffect::TeleportOut(dest) => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(id).realized
                    {
                        body.in_room = false;
                    }
                    self.shortcuts.lobby.push(LobbyVessel {
                        room: dest.room,
                        entrance_node: dest.node,
                        updates: 0,
                        teleport: Some(id),
                    });
                }
                OverseerEffect::StartConversation { other, with, delay } => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(other).realized
                    {
                        body.conversation_partner = Some(with);
                        body.mode = OverseerMode::Conversing;
                        body.conversation_delay = delay;
                    }
                }
                OverseerEffect::EndConversation { other, delay } => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(other).realized
                    {
                        body.switch_to_watching(delay);
                    }
                }
                OverseerEffect::KillGreenLizards(room) => self.world.kill_green_lizards(room),
                OverseerEffect::ResetSuperLaunchJump => {
                    if let Some(p) = self.player.as_mut() {
                        p.reset_super_launch_jump();
                    }
                }
            }
        }
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
        let updates = self
            .player
            .as_ref()
            .is_some_and(|p| player_room == index && !p.in_shortcut() && !p.is_dead());
        if !updates || self.world.realized(index).is_none() {
            return;
        }
        self.creature_new_tile(self.player_id);
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let Some(player) = self.player.as_mut() else {
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

    /// The start of `Creature.Update`: `NewTile` when the creature's coordinate moved
    /// to another tile or room since its last update, then `lastCoord = pos`.
    fn creature_new_tile(&mut self, id: CreatureId) {
        let coord = self.world.creature(id).pos;
        let last = self
            .last_coords
            .insert(id, coord)
            .unwrap_or(WorldCoordinate::new(0, 0, 0, 0));
        if (coord.tile != last.tile || coord.room != last.room)
            && let Some(c) = self.world.new_tile(id, last, self.camera_room)
        {
            self.entrance_colors.push(c);
        }
    }

    /// `ShortcutGraphics.ColorEntrance` calls on camera 0 since the last call.
    pub fn take_entrance_colors(&mut self) -> Vec<EntranceColor> {
        std::mem::take(&mut self.entrance_colors)
    }

    /// What the overseers told camera 0's HUD since the last call.
    pub fn take_hud_events(&mut self) -> Vec<HudEvent> {
        std::mem::take(&mut self.hud_events)
    }

    /// Camera 0's shown and most likely next camera positions, from the app's room
    /// camera (`RoomCamera` updates before the rooms).
    pub fn set_camera_view(&mut self, current: usize, next: usize) {
        self.camera_view = CameraView {
            room: self.camera_room,
            current,
            next,
        };
    }

    #[must_use]
    pub fn camera_view(&self) -> CameraView {
        self.camera_view
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
    pub fn guide_state(&self) -> &PlayerGuideState {
        &self.guide
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

    /// The player's vessel in the lobby.
    #[must_use]
    pub fn lobby(&self) -> Option<LobbyVessel> {
        self.shortcuts
            .lobby
            .iter()
            .find(|v| v.teleport.is_none())
            .copied()
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
        self.shortcuts.lobby.retain(|v| v.teleport.is_some());
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
        self.player_seq = self.world.next_update_seq();
    }

    /// Sets the food in the player's stomach (for tests and tools: nothing is eaten yet).
    pub fn set_food_in_stomach(&mut self, food: i32) {
        self.food_in_stomach = food.clamp(0, self.campaign.stats.max_food);
    }

    #[must_use]
    pub fn food_in_stomach(&self) -> i32 {
        self.food_in_stomach
    }

    /// Kills the player (for tests and tools).
    pub fn kill_player(&mut self) {
        if let Some(p) = &mut self.player {
            p.die();
        }
    }
}
