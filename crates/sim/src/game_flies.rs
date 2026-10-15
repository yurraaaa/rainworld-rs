//! The game's parts for the bat flies: `FliesWorldAI.Update` among the world processes,
//! `ReadyForAI`'s placement and realization, the realized flies' slots in a room's update
//! list, `FliesRoomAI`, their shortcut vessels and lobby turns, the camera's view of
//! them, and the per-call records a reference model is compared against.

use std::sync::Arc;

use rw_assets::{CreatureType, NodeType, Room, Shortcut};

use super::{PlayerObserver, RainWorldGame, RoomObject};
use crate::camera::CameraView;
use crate::creature::fly::{FliesRoomAi, FlockBehavior, FlyRoomTick, FlyRun, FlyTick, PlayerBody};
use crate::creature::{AbstractCreature, CreatureId, WorldCoordinate};
use crate::player::GrabState;
use crate::shortcut::{CreatureVessel, ShortcutVessel, Transport};
use crate::world::{FliesTick, PlacedQuantified};
use crate::{
    Animation, BodyChunk, BodyMode, INPUT_HISTORY, Input, LobbyVessel, SoundId, TerrainImpact,
    Vec2, VesselOwner, VesselStep, WaterDrip, WaterGlitter,
};

/// Which flies' call a record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FliesCall {
    /// `FliesWorldAI.Update`.
    Update,
    /// `Room.ReadyForAI`'s quantified placement in the room.
    Ready(i32),
    /// `AbstractRoom.Abstractize` of the room (no draws).
    Abstractize(i32),
    /// `ReadyForAI`'s `RealizeInRoom` of an abstract fly.
    Realize { room: i32, id: CreatureId },
    /// `FliesRoomAI.Update` of the room.
    RoomsAi(i32),
    /// The fly's slot in its room's update list (with its graphics update).
    Fly(CreatureId),
    /// The fly's hover loop emitter in the update list.
    Hover(CreatureId),
    /// One shortcut handler step of the fly's vessel.
    Vessel(CreatureId),
    /// The fly's vessel's turn in the lobby.
    Lobby(CreatureId),
    /// `NowViewed` building the fly's graphics.
    NowViewed { room: i32, id: CreatureId },
    /// `NoLongerViewed` of the room.
    NoLongerViewed(i32),
    /// The bat-fly catch in `Player.Update`.
    Catch,
    /// `Player.GrabUpdate`.
    GrabUpdate,
    /// `PlayerObjectLooker.Update`.
    Looker,
    /// The grasp branch of `SlugcatHand.Update` of hand `i`.
    Hand(usize),
    /// `Player.GraphicsModuleUpdated` with something held.
    ModuleUpdated,
    /// A grasp let go by `Stun` or `Die`.
    PlayerRelease,
    /// A `WaterDrip`'s update.
    Drip(u64),
    /// The drips of one of the player's terrain impacts.
    ImpactDrips,
    /// A carried fly pulled toward the shortcut the player enters.
    CarriedPull(CreatureId),
    /// A carried fly leaving the room with the player.
    CarriedSuckedIn(CreatureId),
    /// A carried fly's abstract move with the player.
    CarriedMove(CreatureId),
    /// A carried fly spat out of the shortcut with the player.
    CarriedSpitOut(CreatureId),
}

/// Why a grasp was let go outside `GrabUpdate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    Stun,
    Die,
    /// The port's `set_player` (not a game function).
    Teleport,
}

/// What a call read besides the common fields.
#[derive(Debug, Clone, PartialEq)]
pub enum CallExtra {
    None,
    Catch {
        single_room_world: bool,
    },
    /// `head_pos`: the graphics' head, where `BiteFly`'s drips start.
    GrabUpdate {
        head_pos: Option<Vec2>,
    },
    ModuleUpdated {
        actually_viewed: bool,
        hands: Option<[(Vec2, Vec2); 2]>,
    },
    PlayerRelease {
        grasp: usize,
        reason: ReleaseReason,
    },
    Drip {
        room: i32,
        glitter: WaterGlitter,
        float_water_level: f32,
        before: Box<WaterDrip>,
    },
    ImpactDrips {
        room: i32,
        impact: TerrainImpact,
        pos: Vec2,
        rad: f32,
        mass: f32,
        ceiling_drips: f32,
        /// The random state after the drips (the record's `rng_after` is later).
        rng_after: [u32; 4],
    },
    CarriedPull {
        target: Vec2,
        radius: f32,
    },
    CarriedMove {
        to: WorldCoordinate,
    },
    CarriedSpitOut {
        pos: (i32, i32),
        room: i32,
    },
}

/// A call inside the player's graphics, recorded by them: the random state around it
/// and what it read and left, as JSON text.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsCallRecord {
    pub call: FliesCall,
    pub rng: [[u32; 4]; 2],
    pub input: String,
    pub output: String,
}

/// The slugcat as the records see it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSnapshot {
    pub id: CreatureId,
    /// `None` in a shortcut.
    pub room: Option<i32>,
    pub chunks: [BodyChunk; 2],
    pub submersion: f32,
    pub input: [Input; INPUT_HISTORY],
    pub conscious: bool,
    pub dead: bool,
    pub stun: i32,
    pub animation: Animation,
    pub body_mode: BodyMode,
    pub standing: bool,
    pub flip_direction: i32,
    pub roll_direction: i32,
    pub roll_counter: i32,
    pub food: i32,
    pub quarter_food: i32,
    pub max_food: i32,
    pub food_to_hibernate: i32,
    pub grab: GrabState,
    pub grasps: [Option<CreatureId>; 2],
    pub graphics: bool,
    pub hud: bool,
}

/// A realized fly not deleted: its abstract creature (with the `Fly`) and its vessel (in
/// the lobby with `true`).
pub type FlyRecord = (CreatureId, AbstractCreature, Option<(CreatureVessel, bool)>);

/// A realized room's creature list: (id, type, realized).
pub type RoomCreatures = (i32, Vec<(CreatureId, CreatureType, bool)>);

/// The realized flies after a call.
#[derive(Debug, Clone)]
pub struct FliesState {
    pub flies: Vec<FlyRecord>,
    pub flocks: Vec<FlockBehavior>,
    /// The realized rooms' `FliesRoomAI`s.
    pub rooms_ai: Vec<(i32, FliesRoomAi)>,
    /// The player after the call.
    pub player: Option<PlayerSnapshot>,
    /// The realized rooms' drips.
    pub drips: Vec<WaterDrip>,
}

/// What a call saw of the creatures around the flies.
#[derive(Debug, Clone)]
pub struct FliesWorldView {
    pub creatures: Vec<RoomCreatures>,
    pub player: Option<PlayerBody>,
    /// The player's abstract room.
    pub player_room: i32,
}

/// One flies' call with the random stream around it and the world it saw.
#[derive(Debug, Clone)]
pub struct FliesCallRecord {
    pub tick: u64,
    pub call: FliesCall,
    pub rng_before: [u32; 4],
    pub rng_after: [u32; 4],
    pub rain_timer: i32,
    pub cycle_length: i32,
    /// The realized rooms and whether each is fully loaded, before the call.
    pub realized: Vec<(i32, bool)>,
    pub camera: CameraView,
    pub world: FliesWorldView,
    pub placed: Vec<PlacedQuantified>,
    /// The flies the call created, in order.
    pub new_ids: Vec<CreatureId>,
    /// After the call: every room's stored fly counts that are not 0 (room, node, count).
    pub counts: Vec<(i32, i32, i32)>,
    /// After the call: the rooms holding fly `AbstractCreature`s, and how many.
    pub creatures: Vec<(i32, usize)>,
    /// `nextIssuedId` after the call.
    pub next_id: i32,
    pub state: Option<FliesState>,
    /// The player before the call.
    pub player: Option<PlayerSnapshot>,
    /// `game.evenUpdate`.
    pub eu: bool,
    pub extra: CallExtra,
    /// The drips the call added, in order.
    pub new_drip_ids: Vec<u64>,
    pub graphics: Option<GraphicsCallRecord>,
}

impl RainWorldGame {
    /// The world processes in `worldProcesses` order: `OverseersWorldAI` (made by the
    /// population's first overseer) before `FliesWorldAI` (made with the world after
    /// it). The overseers' update draws nothing, so their order does not matter.
    pub(super) fn update_world_processes(&mut self, observer: &mut dyn PlayerObserver) {
        let region = self.world.region_name().unwrap_or_default().to_owned();
        let story = crate::world::StoryProgress {
            woke_up_in_region: &region,
            ever_met_moon: false,
            karma: self.campaign.start_karma,
        };
        self.world.update_world_processes(Some(story));
        let before = self.flies_record_start();
        let session = self.story_session();
        let mut cx = FliesTick {
            rng: &mut self.rng,
            ids: &mut self.ids,
            session: &session,
            observer: &mut *observer,
            camera_room: self.camera_room,
            new_flies: Vec::new(),
        };
        self.world.update_flies_world_ai(&mut cx);
        let new = cx.new_flies;
        // `ChangeRooms` puts the flies it realized into the lobby at once.
        self.enter_from_abstract_rooms();
        self.flies_record_end_new(before, FliesCall::Update, Vec::new(), new, observer);
    }

    /// `ReadyForAI`'s `PlaceQuantifiedCreaturesInRoom` for room `index`.
    pub(super) fn place_quantified(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let before = self.flies_record_start();
        let session = self.story_session();
        let mut cx = FliesTick {
            rng: &mut self.rng,
            ids: &mut self.ids,
            session: &session,
            observer: &mut *observer,
            camera_room: self.camera_room,
            new_flies: Vec::new(),
        };
        let placed = self.world.place_quantified_creatures(index, &mut cx);
        if !placed.is_empty() {
            log::info!("placed {} flies in room {index}", placed.len());
        }
        let new = placed.iter().map(|p| p.creature).collect();
        self.flies_record_end_new(before, FliesCall::Ready(index), placed, new, observer);
    }

    /// Records the abstraction of the rooms that left the active list.
    pub(super) fn record_abstractized(
        &mut self,
        active_before: &[i32],
        observer: &mut dyn PlayerObserver,
    ) {
        if self.flies_calls.is_none() {
            return;
        }
        let now = self.world.active_rooms();
        for &room in active_before.iter().filter(|r| !now.contains(r)) {
            let before = self.flies_record_start();
            self.flies_record_end(before, FliesCall::Abstractize(room), Vec::new(), observer);
        }
    }

    pub(super) fn flies_record_start(&self) -> Option<FliesCallRecord> {
        self.flies_calls.as_ref()?;
        let rain = &self.world.rain_cycle;
        let active = self.world.active_rooms();
        let creatures = active
            .iter()
            .map(|&r| {
                let list = self
                    .world
                    .room_creatures(r)
                    .iter()
                    .map(|&c| {
                        let a = self.world.creature(c);
                        (c, a.kind(), a.is_realized())
                    })
                    .collect();
                (r, list)
            })
            .collect();
        Some(FliesCallRecord {
            tick: self.tick,
            call: FliesCall::Update,
            rng_before: self.rng.state(),
            rng_after: [0; 4],
            rain_timer: rain.timer,
            cycle_length: rain.cycle_length,
            realized: active
                .iter()
                .map(|&r| (r, self.world.realized(r).is_some_and(|x| x.fully_loaded())))
                .collect(),
            camera: self.camera_view,
            world: FliesWorldView {
                creatures,
                player: self.player_body(),
                player_room: self.pos().room,
            },
            placed: Vec::new(),
            new_ids: Vec::new(),
            counts: Vec::new(),
            creatures: Vec::new(),
            next_id: 0,
            state: None,
            player: self.player_snapshot(),
            eu: self.even_update(),
            extra: CallExtra::None,
            new_drip_ids: Vec::new(),
            graphics: None,
        })
    }

    /// The player as the records see it.
    pub(super) fn player_snapshot(&self) -> Option<PlayerSnapshot> {
        let p = self.player.as_ref()?;
        let chunks = [p.head().clone(), p.hips().clone()];
        let room = (!p.in_shortcut() && !p.is_destroyed()).then(|| self.pos().room);
        let water = room.and_then(|r| self.world.realized(r)?.water.as_ref());
        let stats = &self.campaign.stats;
        Some(PlayerSnapshot {
            id: self.player_id,
            room,
            chunks,
            submersion: p.submersion(water),
            input: *p.input_history(),
            conscious: p.is_conscious(),
            dead: p.is_dead(),
            stun: p.stun_ticks(),
            animation: p.animation(),
            body_mode: p.body_mode(),
            standing: p.is_standing(),
            flip_direction: p.flip_direction(),
            roll_direction: p.roll_direction(),
            roll_counter: p.roll_counter(),
            food: self.food_in_stomach(),
            quarter_food: self.quarter_food_points(),
            max_food: stats.max_food,
            food_to_hibernate: stats.food_to_hibernate,
            grab: *p.grab_state(),
            grasps: self.player_grasps(),
            graphics: self.player_graphics,
            hud: self.script_tick.hud_with_control_preset,
        })
    }

    /// Ends a record of a call with what it read besides the common fields.
    pub(super) fn flies_record_end_extra(
        &mut self,
        record: Option<FliesCallRecord>,
        call: FliesCall,
        extra: CallExtra,
        observer: &mut dyn PlayerObserver,
    ) {
        let record = record.map(|mut r| {
            r.extra = extra;
            r
        });
        self.flies_record_end(record, call, Vec::new(), observer);
    }

    pub(super) fn flies_record_end(
        &mut self,
        record: Option<FliesCallRecord>,
        call: FliesCall,
        placed: Vec<PlacedQuantified>,
        observer: &mut dyn PlayerObserver,
    ) {
        self.flies_record_end_new(record, call, placed, Vec::new(), observer);
    }

    fn flies_record_end_new(
        &mut self,
        record: Option<FliesCallRecord>,
        call: FliesCall,
        placed: Vec<PlacedQuantified>,
        new_ids: Vec<CreatureId>,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(mut record) = record else { return };
        record.call = call;
        record.rng_after = self.rng.state();
        record.placed = placed;
        record.new_ids = new_ids;
        let world = &self.world;
        for i in 0..world.number_of_rooms() {
            let room = world.first_room_index() + i32::try_from(i).unwrap_or(0);
            for q in world.room_quantified(room) {
                if q.kind == CreatureType::Fly && q.count != 0 {
                    record.counts.push((room, q.node, q.count));
                }
            }
            let flies = world
                .room_creatures(room)
                .iter()
                .filter(|&&c| world.creature(c).kind() == CreatureType::Fly)
                .count();
            if flies > 0 {
                record.creatures.push((room, flies));
            }
        }
        record.counts.sort_unstable();
        record.next_id = self.ids.next_issued_id;
        record.new_drip_ids = self.world.take_new_drips();
        record.state = Some(self.flies_state());
        if let Some(calls) = &mut self.flies_calls {
            calls.push(record);
        }
        observer.flies_recorded();
    }

    fn flies_state(&self) -> FliesState {
        let flies = self
            .world
            .creatures()
            .filter(|(_, c)| c.fly().is_some() && !c.slated_for_deletion)
            .map(|(id, c)| (id, c.clone(), self.shortcuts.creature_vessel(id)))
            .collect();
        let rooms_ai = self
            .world
            .active_rooms()
            .into_iter()
            .filter_map(|r| Some((r, self.world.realized(r)?.flies_room_ai.clone()?)))
            .collect();
        let drips = self
            .world
            .active_rooms()
            .into_iter()
            .filter_map(|r| self.world.realized(r))
            .flat_map(|r| r.drips.iter().cloned())
            .collect();
        FliesState {
            flies,
            flocks: self.world.flocks.clone(),
            rooms_ai,
            player: self.player_snapshot(),
            drips,
        }
    }

    /// Starts or stops recording the flies' calls.
    pub fn record_flies_calls(&mut self, on: bool) {
        self.flies_calls = on.then(Vec::new);
    }

    /// The flies' calls recorded since the last take.
    pub fn take_flies_calls(&mut self) -> Vec<FliesCallRecord> {
        self.flies_calls
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// The random state right before and after `FliesWorldAI`'s constructor in the world
    /// load.
    #[must_use]
    pub fn flies_ctor_rng(&self) -> [[u32; 4]; 2] {
        self.flies_ctor_rng
    }
}

impl RainWorldGame {
    /// `fliesRoomAi.Update` in `Room.Update`, after the shortcut blinking.
    pub(super) fn run_flies_room_ai(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let has = self
            .world
            .realized(index)
            .is_some_and(|r| r.flies_room_ai.is_some());
        if !has {
            return;
        }
        let before = self.flies_record_start();
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        self.world.update_flies_room_ai(index, &mut cx, sounds);
        self.flies_record_end(before, FliesCall::RoomsAi(index), Vec::new(), observer);
    }

    /// The realized flies and their hover emitters in room `index`'s update list.
    pub(super) fn room_fly_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let mut list = Vec::new();
        for (id, c) in self.world.creatures() {
            let Some(fly) = c.fly() else { continue };
            if fly.room == Some(index) {
                list.push((fly.update_order, RoomObject::Fly(id)));
            }
            if let Some(h) = fly.hover.filter(|h| h.room == index && !h.slated) {
                list.push((h.update_order, RoomObject::Hover(id)));
            }
        }
        list
    }

    /// The player's body as the flies read it.
    pub(super) fn player_body(&self) -> Option<PlayerBody> {
        let p = self.player.as_ref()?;
        let chunks = p.chunks();
        Some(PlayerBody {
            id: self.player_id,
            room: (!p.in_shortcut() && !p.is_destroyed()).then(|| self.pos().room),
            chunks: [chunks[0].pos, chunks[1].pos],
        })
    }

    /// Fly `id`'s slot in room `index`'s update list: `Fly.Update`, then its graphics
    /// module's if it is still here; a fly slated for deletion is cleaned out.
    pub(super) fn run_fly(
        &mut self,
        index: i32,
        id: CreatureId,
        room: &Arc<Room>,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(fly) = self.world.creature(id).fly() else {
            return;
        };
        if fly.room != Some(index) {
            return;
        }
        if fly.slated {
            self.clean_out_fly(id);
            return;
        }
        let Some(ai_map) = self.world.realized(index).and_then(|r| r.ai_map.clone()) else {
            return;
        };
        let before = self.flies_record_start();
        let water = self.world.realized_mut(index).and_then(|r| r.water.take());
        let camera_room = self
            .world
            .realized(self.camera_room)
            .map_or_else(|| Arc::clone(room), |r| Arc::clone(&r.room));
        let player = self.player_body();
        self.world.player_body = player;
        let eu = self.even_update();
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = FlyTick {
            room_index: index,
            room: Arc::clone(room),
            ai_map,
            water: water.as_ref(),
            rng: &mut self.rng,
            sounds,
            observer: &mut *observer,
            camera: self.camera_view,
            camera_room,
            player,
            shortcuts: &mut self.shortcuts,
            entrance_colors: &mut self.entrance_colors,
            tick: self.tick,
            eu,
        };
        let mut run = FlyRun {
            me: id,
            world: &mut self.world,
            cx: &mut cx,
        };
        run.update();
        let fly = run.world.fly_ref(id);
        if fly.room == Some(index) && !fly.slated && fly.graphics {
            run.graphics_update();
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.water = water;
        }
        if self.world.fly_ref(id).slated {
            self.clean_out_fly(id);
        }
        self.flies_record_end(before, FliesCall::Fly(id), Vec::new(), observer);
    }

    /// `CleanOutObjectNotInThisRoom` of a fly slated for deletion: its abstract creature
    /// is destroyed and leaves the room's lists.
    fn clean_out_fly(&mut self, id: CreatureId) {
        self.world.fly_at(id).room = None;
        self.world.creature_mut(id).slated_for_deletion = true;
        self.world.remove_entity(id);
    }

    /// The hover loop emitter of fly `id` in room `index`'s update list
    /// (`ChunkSoundEmitter.Update`: it follows the fly's chunk).
    pub(super) fn run_hover(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(fly) = self.world.creature(id).fly() else {
            return;
        };
        let Some(mut h) = fly.hover.filter(|h| h.room == index && !h.slated) else {
            return;
        };
        let before = self.flies_record_start();
        let (owner_room, pos) = (fly.room, fly.pos());
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.move_emitter(h.sound, pos);
        h.update(owner_room, self.camera_room, self.tick);
        if h.slated {
            sounds.stop_emitter(h.sound);
        }
        self.world.fly_at(id).hover = Some(h);
        self.flies_record_end(before, FliesCall::Hover(id), Vec::new(), observer);
    }

    /// `CreatureEnterFromAbstractRoom` of a fly moved into a realized room: a shortcut
    /// vessel at the entrance of its node (exits, dens, region transports) or a bat hive
    /// vessel; `None` for a node without either (the fly is left realized nowhere, as in
    /// the game; border exits are not ported).
    pub(super) fn creature_enter_vessel(
        &mut self,
        id: CreatureId,
        to: WorldCoordinate,
    ) -> Option<VesselOwner> {
        let kind = self
            .world
            .abstract_room(to.room)?
            .nodes
            .get(usize::try_from(to.node).ok()?)?
            .kind;
        match kind {
            NodeType::Exit | NodeType::Den | NodeType::RegionTransportation => {
                let node = usize::try_from(to.node).ok()?;
                let tile = self
                    .world
                    .realized(to.room)?
                    .room
                    .shortcut_map()
                    .leading_to_node(node)?
                    .dest;
                Some(VesselOwner::Shortcut(CreatureVessel {
                    creature: id,
                    room: to.room,
                    vessel: ShortcutVessel::new(tile),
                    entrance_node: to.node,
                }))
            }
            NodeType::BatHive => Some(VesselOwner::BatHive(id)),
            NodeType::SideExit | NodeType::SkyExit | NodeType::SeaExit => {
                log::warn!("a fly entering by a border exit is not ported");
                None
            }
            NodeType::GarbageHoles => None,
        }
    }

    /// One `ShortcutHandler.Update` turn of the fly vessel at `transport[i]`. Returns
    /// `false` where the game returns from the whole update.
    pub(super) fn creature_vessel_step(
        &mut self,
        i: usize,
        mut v: CreatureVessel,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        if self.world.creature(v.creature).fly().is_none() {
            log::warn!("a vessel's creature is no longer realized; vessel dropped");
            self.shortcuts.transport.remove(i);
            return true;
        }
        let Some(room) = self.world.realized(v.room).map(|r| Arc::clone(&r.room)) else {
            self.shortcuts.transport.remove(i);
            return true;
        };
        let before = self.flies_record_start();
        let sounds = if v.room == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let step = v.step(&room, &mut self.rng, sounds);
        self.shortcuts.transport[i] = Transport::Creature(v);
        let mut go_on = true;
        match step {
            VesselStep::Waited | VesselStep::Moved => {}
            VesselStep::SpatOut => {
                self.shortcuts.transport.remove(i);
                self.spit_out_fly(v, observer);
            }
            VesselStep::Node { node, kind, .. } => {
                let node = node.map_or(-1, |n| i32::try_from(n).unwrap_or(-1));
                self.world.creature_mut(v.creature).pos.node = node;
                go_on = self.creature_vessel_node(i, v, node, kind);
            }
        }
        self.flies_record_end(before, FliesCall::Vessel(v.creature), Vec::new(), observer);
        go_on
    }

    /// The node cases of the vessel step: a room exit sends the vessel to the lobby of
    /// the room behind (even a faulty exit drops it), a den is the fly's
    /// `OpportunityToEnterDen` (it rides on: a fly is never `InDen`), a region
    /// transport needs an `NPCTransportationDestination` (none).
    fn creature_vessel_node(
        &mut self,
        i: usize,
        mut v: CreatureVessel,
        node: i32,
        kind: Shortcut,
    ) -> bool {
        match kind {
            Shortcut::RoomExit => {
                let connections = self
                    .world
                    .abstract_room(v.room)
                    .map(|r| r.connections.clone())
                    .unwrap_or_default();
                if !connections.is_empty() {
                    match usize::try_from(node).ok().and_then(|n| connections.get(n)) {
                        // The game steps the vessel back, then drops it below anyway.
                        None => log::warn!("faulty room exit"),
                        Some(&dest) if dest <= -1 => return false,
                        Some(&dest) => {
                            let entrance_node = self
                                .world
                                .abstract_room(dest)
                                .map_or(-1, |r| r.exit_index(v.room));
                            v.room = dest;
                            v.entrance_node = entrance_node;
                            self.shortcuts.lobby.push(LobbyVessel {
                                room: dest,
                                entrance_node,
                                updates: 0,
                                owner: VesselOwner::Shortcut(v),
                            });
                        }
                    }
                }
                self.shortcuts.transport.remove(i);
            }
            Shortcut::CreatureDen => {
                self.world.fly_opportunity_to_enter_den(v.creature);
            }
            _ => {}
        }
        true
    }

    /// `SpitOutCreature` (`Small_NPC_Exit_Shortcut`) and `Fly.SpitOutOfShortCut` with
    /// every stuck object: each realized one's chunks at the entrance (a draw each),
    /// added to the room, spat out itself, and `NewRoom` when it comes from elsewhere;
    /// then the fly out of the hole at 15 px a tick.
    fn spit_out_fly(&mut self, v: CreatureVessel, observer: &mut dyn PlayerObserver) {
        let (room, pos) = (v.room, v.vessel.pos);
        let Some(map_room) = self.world.realized(room).map(|r| Arc::clone(&r.room)) else {
            return;
        };
        let middle = Vec2::middle_of_tile(pos);
        let sounds = if room == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.play_at(
            SoundId::SmallNpcExitShortcut,
            middle,
            1.0,
            1.0,
            &mut self.rng,
        );
        let hole = map_room.shortcut_hole_direction(pos.0, pos.1);
        let hole = Vec2::new(hole.0 as f32, hole.1 as f32);
        let me = v.creature;
        if self.world.fly_ref(me).entering_shortcut.is_some() {
            log::warn!("Double shortcut entry issue!");
            self.world.fly_at(me).entering_shortcut = None;
        }
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        for o in self.world.connected_objects(me) {
            if self.world.creature(o).fly().is_none() {
                continue;
            }
            for c in &mut self.world.fly_at(o).body.chunks {
                let p = middle + Vec2::random_unit(cx.rng);
                c.hard_set_position(p);
                c.vel = Vec2::ZERO;
            }
            let came_from = self.world.fly_ref(o).room;
            self.world.fly_add_object(o, room, &mut cx);
            self.world.fly_spat_out(o, middle, hole, &mut cx);
            if came_from != Some(room) {
                self.world.fly_new_room(o, room, &mut cx);
            }
        }
        self.world.fly_spat_out(me, middle, hole, &mut cx);
    }

    /// A fly vessel's turn in the lobby: abstracted toward an unrealized room; into a
    /// room ready for the player and for AI it moves there, restarts at the entrance
    /// node's tile in the tubes and the entrance blinks (1.5, the fly's colour on camera
    /// 0). Returns whether it left the lobby.
    pub(super) fn creature_lobby_step(
        &mut self,
        mut v: CreatureVessel,
        lobby: LobbyVessel,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        let id = v.creature;
        if self.world.creature(id).fly().is_none() {
            log::warn!("a lobby vessel's creature is no longer realized; vessel dropped");
            return true;
        }
        let before = self.flies_record_start();
        let coord = WorldCoordinate::new(lobby.room, -1, -1, lobby.entrance_node);
        let ready = self
            .world
            .realized(lobby.room)
            .map(|r| r.ready_for_player() && r.ready_for_ai());
        let done = match ready {
            None => {
                self.world.abstractize_fly(id, coord);
                true
            }
            Some(true) => {
                self.world.move_creature(id, coord);
                let kind = self.world.abstract_room(lobby.room).and_then(|a| {
                    a.nodes
                        .get(usize::try_from(lobby.entrance_node).ok()?)
                        .map(|n| n.kind)
                });
                let node_tile = self.world.realized(lobby.room).and_then(|r| {
                    let n = usize::try_from(lobby.entrance_node).ok()?;
                    r.room.shortcut_map().nodes().get(n).copied()
                });
                if matches!(
                    kind,
                    Some(NodeType::Exit | NodeType::Den | NodeType::RegionTransportation)
                ) && let Some(tile) = node_tile
                {
                    v.room = lobby.room;
                    v.vessel = ShortcutVessel::new(tile);
                    v.entrance_node = -1;
                    self.shortcuts.transport.push(Transport::Creature(v));
                    if kind == Some(NodeType::Exit) {
                        self.blink_arrival(lobby, 1.5, CreatureType::Fly);
                    }
                }
                true
            }
            Some(false) => false,
        };
        self.flies_record_end(before, FliesCall::Lobby(id), Vec::new(), observer);
        done
    }

    /// A bat hive vessel's lobby turn: once the room is ready the creature moves there
    /// and `PopOutOfBatHive` does nothing (the fly stays realized out of any room, as
    /// in the game).
    pub(super) fn bat_hive_lobby_step(&mut self, id: CreatureId, lobby: LobbyVessel) -> bool {
        let coord = WorldCoordinate::new(lobby.room, -1, -1, lobby.entrance_node);
        let ready = self
            .world
            .realized(lobby.room)
            .map(|r| r.ready_for_player() && r.ready_for_ai());
        match ready {
            None => {
                self.world.abstractize_fly(id, coord);
                true
            }
            Some(true) => {
                self.world.move_creature(id, coord);
                true
            }
            Some(false) => false,
        }
    }

    /// `ReadyForAI`'s `RealizeInRoom` of abstract fly `id` in room `index`.
    pub(super) fn realize_fly_in_room(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        let c = self.world.creature(id);
        if c.kind() != CreatureType::Fly || c.is_realized() || c.in_den {
            return;
        }
        let before = self.flies_record_start();
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        self.world.realize_fly_in_room(id, index, &mut cx);
        self.flies_record_end(
            before,
            FliesCall::Realize { room: index, id },
            Vec::new(),
            observer,
        );
    }

    /// The camera left room `from` for room `to`: `NoLongerViewed` drops the graphics
    /// of the flies in `from`, `NowViewed` builds them, with the overseers', in the
    /// order the objects were added.
    pub(super) fn camera_changed_room(
        &mut self,
        from: i32,
        to: i32,
        observer: &mut dyn PlayerObserver,
    ) {
        for &id in self.world.room_creatures(from) {
            if let Some(crate::creature::RealizedCreature::Overseer(_)) =
                self.world.creature(id).realized
            {
                observer.overseer_removed(self.world.creature(id).id);
            }
        }
        let leaving: Vec<CreatureId> = self
            .world
            .creatures()
            .filter(|(_, c)| c.fly().is_some_and(|f| f.room == Some(from) && f.graphics))
            .map(|(id, _)| id)
            .collect();
        if !leaving.is_empty() {
            let before = self.flies_record_start();
            for id in leaving {
                self.world.fly_at(id).graphics = false;
                observer.fly_removed(self.world.creature(id).id);
            }
            self.flies_record_end(
                before,
                FliesCall::NoLongerViewed(from),
                Vec::new(),
                observer,
            );
        }
        let mut objects: Vec<(u64, CreatureId, bool)> = self
            .world
            .room_overseers(to)
            .into_iter()
            .map(|(seq, id)| (seq, id, false))
            .collect();
        for (id, c) in self.world.creatures() {
            if let Some(f) = c.fly().filter(|f| f.room == Some(to)) {
                objects.push((f.update_order, id, true));
            }
        }
        objects.sort_by_key(|&(seq, _, _)| seq);
        for (_, id, is_fly) in objects {
            if !is_fly {
                if let Some(view) = self.world.overseer_view(id) {
                    observer.overseer_added(&view, &mut self.rng);
                }
                continue;
            }
            if self.world.fly_ref(id).graphics {
                continue;
            }
            let before = self.flies_record_start();
            self.world.fly_at(id).graphics = true;
            let view = self.world.fly_view(id, None);
            observer.fly_added(&view, &mut self.rng);
            self.flies_record_end(
                before,
                FliesCall::NowViewed { room: to, id },
                Vec::new(),
                observer,
            );
        }
    }
}
