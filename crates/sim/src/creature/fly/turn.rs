//! The game's parts for the bat flies: `FliesWorldAI.Update` among the world processes,
//! `ReadyForAI`'s placement and realization, the realized flies' slots in a room's update
//! list, `FliesRoomAI`, their shortcut vessels and lobby turns (shared with the
//! lizards'), the camera's view of them and of the other creatures' graphics modules.
//!
//! Mirrors: FliesWorldAI.cs (Update), FliesRoomAI.cs, Room.cs (ReadyForAI, NowViewed, NoLongerViewed), RoomCamera.cs (ChangeRoom), PhysicalObject.cs (InitiateGraphicsModule, RemoveGraphicsModule)

use std::sync::Arc;

use rw_assets::{CreatureType, NodeType, Room, Shortcut};

use crate::creature::fly::{FlyRoomTick, FlyRun, FlyTick, PlayerBody};
use crate::creature::{CreatureId, WorldCoordinate};
use crate::game::shortcut::{CreatureVessel, ShortcutVessel, Transport};
use crate::game::{PlayerObserver, RainWorldGame, RoomObject};
use crate::item::Entity;
use crate::world::{FliesTick, LizardRoomCx};
use crate::{LobbyVessel, SoundId, Vec2, VesselOwner, VesselStep};

impl RainWorldGame {
    /// The world processes in `worldProcesses` order: `OverseersWorldAI` (made by the
    /// population's first overseer) before `FliesWorldAI` (made with the world after
    /// it). The overseers' update draws nothing, so their order does not matter.
    pub(crate) fn update_world_processes(&mut self, observer: &mut dyn PlayerObserver) {
        let region = self.world.region_name().unwrap_or_default().to_owned();
        let story = crate::world::StoryProgress {
            woke_up_in_region: &region,
            ever_met_moon: false,
            karma: self.story.karma,
        };
        self.world.update_world_processes(Some(story));
        let session = self.story_session();
        let mut cx = FliesTick {
            rng: &mut self.rng,
            ids: &mut self.ids,
            session: &session,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        self.world.update_flies_world_ai(&mut cx);
        // `ChangeRooms` puts the flies it realized into the lobby at once.
        self.enter_from_abstract_rooms();
    }

    /// `ReadyForAI`'s `PlaceQuantifiedCreaturesInRoom` for room `index`.
    pub(crate) fn place_quantified(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let session = self.story_session();
        let mut cx = FliesTick {
            rng: &mut self.rng,
            ids: &mut self.ids,
            session: &session,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        let placed = self.world.place_quantified_creatures(index, &mut cx);
        if placed > 0 {
            log::info!("placed {placed} flies in room {index}");
        }
    }
}

impl RainWorldGame {
    /// `fliesRoomAi.Update` in `Room.Update`, after the shortcut blinking.
    pub(crate) fn run_flies_room_ai(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let has = self
            .world
            .realized(index)
            .is_some_and(|r| r.flies_room_ai.is_some());
        if !has {
            return;
        }
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
    }

    /// The realized flies and their hover emitters in room `index`'s update list.
    pub(crate) fn room_fly_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
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
    pub(crate) fn player_body(&self) -> Option<PlayerBody> {
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
    pub(crate) fn run_fly(
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
        let water = self.world.realized_mut(index).and_then(|r| r.water.take());
        let camera_room = self
            .world
            .realized(self.camera_room)
            .map_or_else(|| Arc::clone(room), |r| Arc::clone(&r.room));
        let player = self.player_body();
        self.world.player_body = player;
        let room_gravity = self.room_gravity(index);
        self.world.fly_at(id).body.params.room_gravity = room_gravity;
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
            camera_shake: &mut self.camera_shake,
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
    pub(crate) fn run_hover(&mut self, index: i32, id: CreatureId) {
        let Some(fly) = self.world.creature(id).fly() else {
            return;
        };
        let Some(mut h) = fly.hover.filter(|h| h.room == index && !h.slated) else {
            return;
        };
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
    }

    /// `CreatureEnterFromAbstractRoom` of a fly moved into a realized room: a shortcut
    /// vessel at the entrance of its node (exits, dens, region transports) or a bat hive
    /// vessel; `None` for a node without either (the fly is left realized nowhere, as in
    /// the game; border exits are not ported).
    pub(crate) fn creature_enter_vessel(
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
                    vessel: ShortcutVessel::with_segments(
                        tile,
                        self.world.creature(id).template.shortcut_segments(),
                        0,
                    ),
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

    /// One `ShortcutHandler.Update` turn of the fly or lizard vessel at `transport[i]`
    /// (the tick sound by its template's size). Returns `false` where the game returns
    /// from the whole update.
    pub(crate) fn creature_vessel_step(
        &mut self,
        i: usize,
        mut v: CreatureVessel,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        let c = self.world.creature(v.creature);
        if c.fly().is_none() && c.lizard().is_none() {
            log::warn!("a vessel's creature is no longer realized; vessel dropped");
            self.shortcuts.transport.remove(i);
            return true;
        }
        let Some(room) = self.world.realized(v.room).map(|r| Arc::clone(&r.room)) else {
            self.shortcuts.transport.remove(i);
            return true;
        };
        let tick = self
            .world
            .creature(v.creature)
            .template
            .npc_shortcut_sound(1);
        let (sounds, rng) = self.room_sounds(v.room);
        let step = v.step(&room, rng, sounds, tick);
        self.shortcuts.transport[i] = Transport::Creature(v);
        let mut go_on = true;
        match step {
            VesselStep::Waited | VesselStep::Moved => {}
            VesselStep::SpatOut => {
                self.shortcuts.transport.remove(i);
                if self.world.creature(v.creature).lizard().is_some() {
                    self.spit_out_lizard(v, observer);
                } else {
                    self.spit_out_fly(v, observer);
                }
            }
            VesselStep::Node { node, kind, .. } => {
                let node = node.map_or(-1, |n| i32::try_from(n).unwrap_or(-1));
                self.world.creature_mut(v.creature).pos.node = node;
                go_on = self.creature_vessel_node(i, v, node, kind);
            }
        }
        go_on
    }

    /// The node cases of the vessel step: a room exit sends the vessel to the lobby of
    /// the room behind (even a faulty exit drops it), a den is the creature's
    /// `OpportunityToEnterDen` (a creature that went in leaves the tubes; a fly is never
    /// `InDen`), a region transport sends it to the lobby of its
    /// `NPCTransportationDestination`'s room when it has one with a node.
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
                self.world
                    .opportunity_to_enter_den(v.creature, &mut self.rng);
                self.player_killed_in_den(v.room);
                if self.world.creature(v.creature).in_den {
                    self.shortcuts.transport.remove(i);
                }
            }
            Shortcut::RegionTransportation => {
                let dest = self
                    .world
                    .creature(v.creature)
                    .lizard()
                    .and_then(|l| l.npc_transportation_destination);
                if let Some(dest) = dest
                    && dest.room > -1
                    && dest.node_defined()
                {
                    v.entrance_node = dest.node;
                    v.room = dest.room;
                    self.shortcuts.lobby.push(LobbyVessel {
                        room: dest.room,
                        entrance_node: dest.node,
                        updates: 0,
                        owner: VesselOwner::Shortcut(v),
                    });
                    self.shortcuts.transport.remove(i);
                }
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
        for o in self.world.connected_objects(Entity::Creature(me)) {
            match o {
                Entity::Creature(o) if self.world.creature(o).fly().is_some() => {
                    self.stuck_fly_spat_out(o, room, middle, hole, observer);
                }
                Entity::Creature(_) => {}
                Entity::Object(o) => self.world.spit_out_object(o, room, middle, &mut self.rng),
            }
        }
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        self.world.fly_spat_out(me, middle, hole, &mut cx);
    }

    /// A fly or lizard vessel's turn in the lobby. Toward an unrealized room it is
    /// abstracted there, unless camera 0 follows it or what it carries (the held player:
    /// `FollowedByCamera`), which realizes the room and waits (an off-screen den is
    /// never realized). Into a room `VesselAllowedInRoom` it moves there with
    /// everything stuck to it, restarts at the entrance node's tile in the tubes, the
    /// entrance blinks (1.5, the creature's colour on camera 0), and a followed vessel
    /// takes the camera to the node's camera. Returns whether it left the lobby.
    pub(crate) fn creature_lobby_step(
        &mut self,
        mut v: CreatureVessel,
        lobby: LobbyVessel,
    ) -> bool {
        let id = v.creature;
        let lizard = self.world.creature(id).lizard().is_some();
        if self.world.creature(id).fly().is_none() && !lizard {
            log::warn!("a lobby vessel's creature is no longer realized; vessel dropped");
            return true;
        }
        let coord = WorldCoordinate::new(lobby.room, -1, -1, lobby.entrance_node);
        let followed = self.followed_by_camera(id);
        let ready = self
            .world
            .realized(lobby.room)
            .map(|_| self.world.vessel_allowed_in_room(id, lobby.room));
        match ready {
            None if followed && lobby.room != self.world.offscreen_den_index() => {
                if let Err(e) = self.activate_room(lobby.room) {
                    log::warn!("cannot realize room {}: {e}", lobby.room);
                }
                false
            }
            None => {
                if lizard {
                    self.abstractize_lizard_to(id, coord);
                } else {
                    self.world.abstractize_fly(id, coord, &mut self.rng);
                }
                true
            }
            Some(true) => {
                self.world.move_creature(id, coord);
                self.move_stuck_objects(id, coord);
                let node = self.world.abstract_room(lobby.room).and_then(|a| {
                    a.nodes
                        .get(usize::try_from(lobby.entrance_node).ok()?)
                        .cloned()
                });
                let kind = node.as_ref().map(|n| n.kind);
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
                    let segments = self.world.creature(id).template.shortcut_segments();
                    v.vessel = ShortcutVessel::with_segments(tile, segments, 0);
                    v.entrance_node = -1;
                    self.shortcuts.transport.push(Transport::Creature(v));
                    if kind == Some(NodeType::Exit) {
                        let color = self.world.creature(id).short_cut_color();
                        self.blink_arrival(lobby, 1.5, color);
                    }
                }
                if followed && let Some(node) = node {
                    self.camera_moves_to(lobby.room, node.viewed_by_camera);
                }
                true
            }
            Some(false) => false,
        }
    }

    /// A bat hive vessel's lobby turn: once the room is ready the creature moves there
    /// and `PopOutOfBatHive` does nothing (the fly stays realized out of any room, as
    /// in the game).
    pub(crate) fn bat_hive_lobby_step(&mut self, id: CreatureId, lobby: LobbyVessel) -> bool {
        let coord = WorldCoordinate::new(lobby.room, -1, -1, lobby.entrance_node);
        let ready = self
            .world
            .realized(lobby.room)
            .map(|_| self.world.vessel_allowed_in_room(id, lobby.room));
        match ready {
            None => {
                self.world.abstractize_fly(id, coord, &mut self.rng);
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
    pub(crate) fn realize_fly_in_room(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        let c = self.world.creature(id);
        if c.kind() != CreatureType::Fly || c.is_realized() || c.in_den {
            return;
        }
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        self.world.realize_fly_in_room(id, index, &mut cx);
    }

    /// `Room.NoLongerViewed` of room `from`, which camera 0 left: it drops the graphics
    /// modules in `from`'s `drawableObjects` (those of the creatures still in it: one
    /// that left took its module out with `CleanOutObjectNotInThisRoom` and keeps it).
    /// An overseer in a teleport lobby keeps its module; a destroyed one's is dropped
    /// with it.
    pub(crate) fn modules_no_longer_viewed(
        &mut self,
        from: i32,
        observer: &mut dyn PlayerObserver,
    ) {
        let overseers: Vec<CreatureId> = self
            .world
            .room_creatures(from)
            .iter()
            .copied()
            .filter(|&id| {
                matches!(
                    &self.world.creature(id).realized,
                    Some(crate::creature::RealizedCreature::Overseer(b)) if b.in_room || b.slated
                )
            })
            .collect();
        for id in overseers {
            self.overseer_module_removed(id, observer);
        }
        let leaving: Vec<CreatureId> = self
            .world
            .creatures()
            .filter(|(_, c)| c.fly().is_some_and(|f| f.room == Some(from) && f.graphics))
            .map(|(id, _)| id)
            .collect();
        for id in leaving {
            self.world.fly_at(id).graphics = false;
            observer.fly_removed(self.world.creature(id).id);
        }
        let lizards_leaving: Vec<CreatureId> = self
            .world
            .creatures()
            .filter(|(_, c)| {
                c.lizard()
                    .and_then(|l| l.graphics.as_ref())
                    .is_some_and(|g| g.drawn_in.contains(&from))
            })
            .map(|(id, _)| id)
            .collect();
        for id in lizards_leaving {
            let l = self.world.lizard_at(id);
            if let Some(mut g) = l.graphics.take() {
                l.dropped_loop_emitters.extend(g.sound_loop.take_emitters());
            }
            observer.lizard_removed(self.world.creature(id).id);
        }
    }

    /// `Room.NowViewed`'s `InitiateGraphicsModule` over room `to`'s `physicalObjects` layer
    /// by layer, each in joining order: flies and overseers (layer 0), then lizards (layer
    /// 1, whose `Reset` runs even on a module they kept).
    pub(crate) fn modules_now_viewed(&mut self, to: i32, observer: &mut dyn PlayerObserver) {
        self.now_viewed_modules.clear();
        for (_, id, kind) in self.now_viewed_objects(to) {
            match kind {
                Viewed::Overseer => {
                    if self.overseer_module_added(id, to, observer) {
                        self.now_viewed_modules.push(id);
                    }
                }
                Viewed::Fly => {
                    if self.world.fly_ref(id).graphics {
                        continue;
                    }
                    self.world.fly_at(id).graphics = true;
                    let view = self.world.fly_view(id, None);
                    observer.fly_added(&view, &mut self.rng);
                    self.now_viewed_modules.push(id);
                }
                Viewed::Lizard => {
                    let kept = self
                        .world
                        .creature(id)
                        .lizard()
                        .and_then(|l| l.graphics.as_ref())
                        .is_some_and(|g| g.drawn_in.contains(&to));
                    let mut cx = LizardRoomCx {
                        rng: &mut self.rng,
                        observer: &mut *observer,
                        camera_room: self.camera_room,
                    };
                    self.world.lizard_initiate_graphics(id, &mut cx);
                    if !kept {
                        self.now_viewed_modules.push(id);
                    }
                }
            }
        }
    }

    /// Room `to`'s creatures with a graphics module of their own, as `NowViewed` visits
    /// `physicalObjects`: by collision layer, then joining order.
    fn now_viewed_objects(&self, to: i32) -> Vec<(u64, CreatureId, Viewed)> {
        let mut objects: Vec<(u64, CreatureId, Viewed)> = self
            .world
            .room_overseers(to)
            .into_iter()
            .map(|(seq, id)| (seq, id, Viewed::Overseer))
            .collect();
        for (id, c) in self.world.creatures() {
            if let Some(f) = c.fly().filter(|f| f.room == Some(to)) {
                objects.push((f.update_order, id, Viewed::Fly));
            }
        }
        objects.extend(
            self.world
                .room_lizards(to)
                .into_iter()
                .map(|(seq, id)| (seq, id, Viewed::Lizard)),
        );
        objects.sort_by_key(|&(seq, _, kind)| (kind.layer(), seq));
        objects
    }

    /// The creatures whose graphics modules the camera's last room change put in its
    /// room's `drawableObjects` (`NowViewed`); the others there joined with `AddObject`.
    #[must_use]
    pub fn now_viewed_modules(&self) -> &[CreatureId] {
        &self.now_viewed_modules
    }
}

/// A creature whose graphics module `Room.NowViewed` initiates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Viewed {
    Overseer,
    Fly,
    Lizard,
}

impl Viewed {
    /// Its `collisionLayer` (`Overseer`, `Fly`: 0; `Lizard`: 1).
    fn layer(self) -> u8 {
        match self {
            Self::Overseer | Self::Fly => 0,
            Self::Lizard => 1,
        }
    }
}
