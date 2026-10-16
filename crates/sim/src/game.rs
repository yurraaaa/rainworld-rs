//! `RainWorldGame` for one player: the world, the player's place in it, the shortcut
//! handler with its between-rooms lobby, the room realizer, and the order of a tick.
//!
//! Creatures: the world's population and its overseers' abstract AI
//! ([`crate::creature`]); the player is an `AbstractCreature` too. Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`, `shortcuts.md` section 5,
//! `creature-model.md`.

use std::collections::HashMap;
use std::sync::Arc;

#[path = "game_exit.rs"]
mod exit;
#[path = "game_flies.rs"]
mod flies;
#[path = "game_player.rs"]
mod hands;
#[path = "game_hits.rs"]
mod hits;
#[path = "game_instructions.rs"]
mod instructions;
#[path = "game_items.rs"]
mod items;
#[path = "game_map.rs"]
mod map;
#[path = "game_over.rs"]
mod over;

pub use items::Grabability;
pub use over::GameOver;
#[path = "game_shelter.rs"]
mod shelter_door;
#[path = "game_triggers.rs"]
mod triggers;
#[path = "game_weapons.rs"]
mod weapons;

use rw_assets::{CreatureType, NodeType, Room, Shortcut, ShortcutType};
use rw_random::UnityRandom;

use crate::camera::CameraView;
use crate::campaign::{SlugcatCampaign, SlugcatStats, StoryStart};
use crate::creature::fly::FlyView;
use crate::creature::overseer::hologram::{ControlPreset, OverseerHologram};
use crate::creature::overseer::{
    OverseerEffect, OverseerMode, OverseerTick, OverseerUpdate, OverseerView, PlayerView,
    StoryStats, TutorialMessage,
};
use crate::creature::population::PopulationContext;
use crate::creature::{
    CreatureId, CreatureTemplate, EntityId, IdIssuer, PlayerGuideState, RealizedCreature,
    WorldCoordinate,
};
use crate::item::{Entity, ObjectId};
use crate::room_script::{RoomScript, ScriptTick, StorySession};
use crate::shelter::{SessionEnd, ShelterDoor};
use crate::shortcut::{CreatureVessel, Transport};
use crate::world::{AbstractTick, FollowCreature, RainCycle, RegionState, RoomRealizer, World};
use crate::{
    EntranceColor, HudMapLink, Input, LobbyVessel, MapDiscovery, Player, ShortcutHandler, Sounds,
    Vec2, VesselOwner, VesselStep, Water, determine_safe_spawn_tile,
};

/// What the app attaches to the player (its graphics module), called where the game
/// calls it.
pub trait PlayerObserver {
    /// The player was constructed, before it is placed (`AddObject` builds the graphics
    /// module in a viewed room).
    fn created(&mut self, player: &Player, rng: &mut UnityRandom);
    /// Right after `Player.Update`: `PlayerGraphics.Update`.
    fn updated(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    );
    /// The player left a shortcut: the inner `Player.SpitOutOfShortCut`'s graphics reset
    /// and `NewRoom`'s.
    fn spat_out(&mut self, player: &Player, rng: &mut UnityRandom);
    /// The outer `Player.SpitOutOfShortCut`'s reset, after the objects carried out with it.
    fn spat_out_finished(&mut self, _player: &Player, _rng: &mut UnityRandom) {}
    /// Whether the player has a graphics module.
    fn has_player_graphics(&self) -> bool {
        false
    }
    /// `PlayerGraphics.LookAtObject(target)`, or `LookAtNothing` for `None`.
    fn player_look_at(&mut self, _target: Option<Entity>) {}
    /// `PlayerGraphics.BiteFly(hand)` up to its drips: returns `head.pos`, where they
    /// start (`None` without a graphics module).
    fn player_bite_fly(&mut self, _hand: usize, _fly: CreatureId) -> Option<Vec2> {
        None
    }
    /// What the graphics' hands did in their update.
    fn player_hands(&self) -> Option<PlayerHands> {
        None
    }
    /// `PlayerGraphics.swallowing = 20` (`GrabUpdate` swallowed an object).
    fn player_swallowing(&mut self) {}
    /// `PlayerGraphics.ThrowObject(hand, obj)`: the hand follows the thrown weapon for
    /// five updates.
    fn player_throw_object(&mut self, _hand: usize, _obj: Entity) {}
    /// `(graphicsModule as PlayerGraphics).head.vel += dv`.
    fn nudge_player_head(&mut self, _dv: Vec2) {}
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
    /// A realized fly is in the camera's room (`AddObject` or `NowViewed`): its
    /// `FlyGraphics` is built unless it has one.
    fn fly_added(&mut self, _view: &FlyView<'_>, _rng: &mut UnityRandom) {}
    /// `FlyGraphics.Reset`.
    fn fly_reset(&mut self, _view: &FlyView<'_>) {}
    /// Right after `Fly.Update`: `FlyGraphics.Update`. `cameras` is the room's camera
    /// count (the culling test needs more than two).
    #[allow(clippy::too_many_arguments)]
    fn fly_updated(
        &mut self,
        _view: &FlyView<'_>,
        _room: &Room,
        _cameras: usize,
        _camera_room: &Room,
        _camera: &CameraView,
        _rng: &mut UnityRandom,
    ) {
    }
    /// `MoveToFront` of the fly's sprites (it joined a chain).
    fn fly_to_front(&mut self, _id: EntityId) {}
    /// The fly's graphics module is dropped.
    fn fly_removed(&mut self, _id: EntityId) {}
}

/// What the game tells camera 0's HUD.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HudEvent {
    /// The guide's `TutorialText`.
    Message(TutorialMessage),
    /// `rainMeter.halfTimeBlink = 0`.
    HalfTimeBlinkReset,
    /// `foodMeter.RefuseFood()`.
    RefuseFood,
    /// `RoomSpecificTextMessage` showed its text:
    /// `deathPersistentSaveData.DangleFruitInWaterMessage = true`.
    DangleFruitInWaterMessage,
}

/// What `Player.GraphicsModuleUpdated` reads of the graphics' last update.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayerHands {
    /// Each hand's (pos, vel).
    pub hands: [(Vec2, Vec2); 2],
    /// `spearDir`.
    pub spear_dir: f32,
    /// A hand's `ChangeOverlap` of the weapon it holds.
    pub overlap: [Option<bool>; 2],
    /// What the hands added to the main chunk's velocity (the swallow's shake).
    pub body_push: Vec2,
}

/// What the player's graphics read of its room and hands.
pub struct PlayerSurroundings<'a> {
    pub world: &'a World,
    /// What the player holds in each hand.
    pub grasps: [Option<Entity>; 2],
    /// `objectInStomach != null`.
    pub stomach: bool,
    pub room: i32,
    /// The room's physical objects in its update list, oldest first, the player left out.
    pub objects: Vec<Entity>,
    /// `FoodInStomach < MaxFoodInStomach`.
    pub hungry: bool,
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
    /// `FlyingWeapon`: this thrown weapon is in its room.
    FlyingWeapon(ObjectId),
    /// `HitByWeapon` of a weapon thrown by a slugcat (or not).
    HitByWeapon { by_slugcat: bool },
    /// `OverseerCommunicationModule.Update` alone.
    #[cfg(test)]
    Communication,
}

/// An object in a room's update list the port runs besides the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoomObject {
    Overseer(CreatureId),
    Hologram {
        owner: CreatureId,
        id: u64,
    },
    Fly(CreatureId),
    /// A fly's hover loop emitter.
    Hover(CreatureId),
    /// A `WaterDrip` by id.
    Drip(u64),
    /// A realized rock or spear.
    Weapon(ObjectId),
    /// A `Spark` or `ExplosionSpikes` by id.
    Cosmetic(u64),
    /// A rock's or spear's loop emitter.
    WeaponLoop(ObjectId),
    /// A trigger checker or an object a trigger added, by id.
    Trigger(u64),
}

/// No graphics.
impl PlayerObserver for () {
    fn created(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn updated(
        &mut self,
        _: &Player,
        _: &Room,
        _: Option<&Water>,
        _: &PlayerSurroundings<'_>,
        _: &mut UnityRandom,
    ) {
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGameWorld {
    pub ids: IdIssuer,
    pub save_state_seed: i32,
    /// `saveState.respawnCreatures` after `GeneratePopulation`.
    pub respawn_creatures: Vec<i32>,
}

/// A running game.
pub struct RainWorldGame {
    campaign: &'static SlugcatCampaign,
    /// The save state's start values.
    story: StoryStart,
    /// `session.characterStats`: `new SlugcatStats(name, saveState.malnourished)`,
    /// replaced by `SetMalnourished`.
    character_stats: SlugcatStats,
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
    /// The last hologram id issued.
    hologram_ids: u64,
    /// Each realized creature's `Creature.lastCoord`; a new body starts at the default
    /// coordinate (room 0, tile (0, 0)).
    last_coords: HashMap<CreatureId, WorldCoordinate>,
    /// `ColorEntrance` calls on camera 0 since the last `take_entrance_colors`.
    entrance_colors: Vec<EntranceColor>,
    /// Whether the player had a graphics module at its last update.
    player_graphics: bool,
    /// `progression.mapDiscoveryTextures`.
    map_discovery: MapDiscovery,
    /// Camera 0's HUD map, as the app last reported it.
    hud_map: HudMapLink,
    /// Camera 0's `PaletteDarkness()`, as the app last reported it.
    camera_darkness: f32,
    /// Set when the cycle ends (the shelter closes, the player starves).
    session_end: Option<SessionEnd>,
    /// Food added this cycle (`saveState.totFood`'s share).
    session_food: i32,
    /// What the player ate this cycle (`SessionRecord.AddEat`), by entity ID.
    eaten: Vec<EntityId>,
    /// The realized shelters' doors.
    shelter_doors: Vec<ShelterDoor>,
    closed_shelter_regions: Vec<String>,
    /// Shelters the player was spat out into (`TempDiscoverShelter`), until taken.
    entered_shelters: Vec<String>,
    /// The player's `karmaFlowerGrowPos`.
    karma_flower_grow_pos: Option<WorldCoordinate>,
    /// The player's death called `GameOver`; taken by the app.
    game_over: Option<GameOver>,
    /// The realized rooms' trigger checkers and the objects their events added.
    trigger_objects: Vec<crate::trigger::TriggerObject>,
    /// What the game asked of the music player this tick; taken by the app.
    music_requests: Vec<crate::trigger::MusicRequest>,
    /// Camera 0's HUD as the checkers read it, set by the app.
    trigger_hud: crate::trigger::TriggerHud,
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
        rng: UnityRandom,
        sounds: Sounds,
        observer: &mut dyn PlayerObserver,
    ) -> Self {
        let story = StoryStart::new_game(campaign);
        Self::new_story(world, campaign, story, start, rng, sounds, observer)
    }

    /// [`Self::new`] from `story`: a new game's or a saved one's.
    pub fn new_story(
        world: World,
        campaign: &'static SlugcatCampaign,
        story: StoryStart,
        start: Start,
        mut rng: UnityRandom,
        sounds: Sounds,
        observer: &mut dyn PlayerObserver,
    ) -> Self {
        let mut world = world;
        let NewGameWorld {
            mut ids,
            save_state_seed,
            respawn_creatures,
        } = Self::story_world(&mut world, campaign, &story, &mut rng);
        let mut story = story;
        story.respawn_creatures = respawn_creatures;
        let session = StorySession {
            character: campaign.name,
            cycle_number: story.cycle_number,
            den_position: story.den.clone(),
        };
        let guide = story.guide.clone();
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
        // `LoadInitCondition`: the saved food, a pip at a time into the one stomach while
        // it is above 0, so a malnourished `Win`'s negative food starts at 0.
        player.state.food_in_stomach = u32::try_from(story.food).unwrap_or(0);
        let player_id = world.insert_creature(player);
        world.add_entity(player_id);
        let realizer = (!world.single_room()).then(RoomRealizer::new);
        let character_stats = if story.malnourished {
            campaign.stats.malnourished()
        } else {
            campaign.stats
        };
        let mut game = Self {
            campaign,
            story,
            character_stats,
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
            hologram_ids: 0,
            last_coords: HashMap::new(),
            entrance_colors: Vec::new(),
            player_graphics: false,
            map_discovery: MapDiscovery::default(),
            hud_map: HudMapLink::default(),
            camera_darkness: 0.0,
            session_end: None,
            session_food: 0,
            eaten: Vec::new(),
            shelter_doors: Vec::new(),
            closed_shelter_regions: Vec::new(),
            entered_shelters: Vec::new(),
            karma_flower_grow_pos: None,
            game_over: None,
            trigger_objects: Vec::new(),
            music_requests: Vec::new(),
            trigger_hud: crate::trigger::TriggerHud::default(),
        };
        if let Err(e) = game
            .world
            .activate_room(start.room, &mut game.ids, &mut game.rng)
        {
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
        Self::story_world(world, campaign, &StoryStart::new_game(campaign), rng)
    }

    /// [`Self::new_game_world`] from `story`, whose saved `NEXTID` and `SEED` replace
    /// the drawn values (`LoadGame`, before the world loads).
    pub fn story_world(
        world: &mut World,
        campaign: &'static SlugcatCampaign,
        story: &StoryStart,
        rng: &mut UnityRandom,
    ) -> NewGameWorld {
        let mut ids = IdIssuer {
            next_issued_id: rng.range_i32(1000, 10000),
        };
        // `new SaveState` draws its seed; a state still in memory is reused.
        let mut save_state_seed = if story.in_memory {
            0
        } else {
            rng.range_i32(0, 10000)
        };
        if let Some(id) = story.next_issued_id {
            ids.next_issued_id = id;
        }
        if let Some(seed) = story.seed {
            save_state_seed = seed;
        }
        let session = StorySession {
            character: campaign.name,
            cycle_number: story.cycle_number,
            den_position: story.den.clone(),
        };
        let guide = &story.guide;
        world.rain_cycle = RainCycle::new_story(campaign.name, rng);
        let mut respawn = story.respawn_creatures.clone();
        let fresh = story.region_state.is_none() && story.live_region_state.is_none();
        // `World.LoadWorld`: the region state still in memory, else read from its load
        // string, then adapted to.
        if !world.single_room() {
            if let Some(state) = &story.live_region_state {
                world.region_state = Some(state.clone());
            } else if let Some(saved) = &story.region_state {
                let state = RegionState::from_string(world.data(), saved, world.mods(), rng);
                world.region_state = Some(state);
            }
        }
        let mut population = PopulationContext {
            session: &session,
            ids: &mut ids,
            rng,
            guide_overseer_dead: story.guide_overseer_dead,
            guide,
            food_to_hibernate: campaign.stats.food_to_hibernate,
            respawn: &mut respawn,
        };
        if !world.single_room() {
            world.adapt_world_to_region_state(&mut population, story.food_replenish_bonus);
            world.generate_population(fresh, &mut population);
        }
        world.create_flies_world_ai(&mut population);
        NewGameWorld {
            ids,
            save_state_seed,
            respawn_creatures: respawn,
        }
    }

    /// The player's `AbstractCreature.pos`.
    fn pos(&self) -> WorldCoordinate {
        self.world.creature(self.player_id).pos
    }

    fn set_pos(&mut self, pos: WorldCoordinate) {
        self.world.place_creature(self.player_id, pos);
        self.move_carried(pos);
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
        player.set_stats(self.character_stats);
        player.set_object_in_stomach(self.stomach_from_save());
        self.world.player_mass = player.total_mass();
        self.karma_flower_grow_pos = None;
        self.player_wakes_in_shelter(&mut player);
        observer.created(&player, &mut self.rng);
        player.place(tile, &mut self.rng);
        self.player = Some(player);
        self.sync_player_realized();
        self.last_coords.remove(&self.player_id);
        self.player_seq = self.world.next_update_seq();
    }

    /// A fresh player in the camera's room, at the start tile in the start room, else
    /// on a safe spawn tile (the R key outside story sessions; not a game function).
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
        self.shortcuts.drop_player();
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
    /// step.
    pub fn tick(&mut self, input: Input, observer: &mut dyn PlayerObserver) {
        let follow = self.follow_creature();
        if let (Some(realizer), Some(follow)) = (&mut self.realizer, follow) {
            realizer.update(
                &mut self.world,
                follow,
                self.camera_room,
                &mut self.ids,
                &mut self.rng,
            );
        }
        self.update_abstract_room();
        self.enter_from_abstract_rooms();
        self.update_world_processes(observer);
        self.world.rain_cycle.update(&mut self.rng);
        self.rain_request_stop_song();
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
        let session = self.story_session();
        let mut cx = AbstractTick {
            rng: &mut self.rng,
            guide: &self.guide,
            player: Some(self.player_id),
            player_karma: self.story.karma,
            cycle: self.story.cycle_number,
            ids: &mut self.ids,
            session: &session,
        };
        self.world.update_abstract_room(room, time, &mut cx);
    }

    /// `CreatureEnterFromAbstractRoom` for the creatures `ChangeRooms` realized: each
    /// waits in the lobby, an overseer in a teleportation vessel, a fly in a shortcut
    /// vessel at its node's entrance (or a bat hive vessel).
    pub(super) fn enter_from_abstract_rooms(&mut self) {
        for (id, to) in std::mem::take(&mut self.world.entering) {
            let owner = if self.world.creature(id).kind() == CreatureType::Overseer {
                VesselOwner::Teleport(id)
            } else {
                match self.creature_enter_vessel(id, to) {
                    Some(owner) => owner,
                    None => continue,
                }
            };
            self.shortcuts.lobby.push(LobbyVessel {
                room: to.room,
                entrance_node: to.node,
                updates: 0,
                owner,
            });
        }
    }

    /// The player's `AbstractCreature.realizedCreature` is its body while it has one.
    /// A new or removed body lets go of what the old one held; a realized player has two
    /// hands (`grasps`).
    fn sync_player_realized(&mut self) {
        self.world.lose_all_grasps(self.player_id);
        let realized = self.player.is_some().then_some(RealizedCreature::Player);
        let c = self.world.creature_mut(self.player_id);
        c.grasps = if realized.is_some() {
            vec![None, None]
        } else {
            Vec::new()
        };
        c.realized = realized;
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
            self.add_shelter_door(index);
            self.add_trigger_checkers(index);
            self.world.realize_room_objects(index, &mut self.rng);
            for id in self.world.room_creatures(index).to_vec() {
                if id == self.player_id {
                    if self.tick == 0 && self.player.is_none() {
                        self.realize_player(observer);
                    }
                } else if self.world.creature(id).kind() == CreatureType::Fly {
                    if self.world.held_by_abstract_creature(id) {
                        continue;
                    }
                    self.realize_fly_in_room(index, id, observer);
                } else {
                    let _before = self.rng.state();
                    let _creature = self.world.creature(id).clone();
                    if self.world.realize_overseer(id, 0, &mut self.rng) {
                        self.run_overseer(id, OverseerCall::Place, observer);
                    }
                }
            }
            self.place_quantified(index, observer);
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

    /// The story session as the room scripts read it.
    #[must_use]
    pub fn story_session(&self) -> StorySession {
        StorySession {
            character: self.campaign.name,
            cycle_number: self.story.cycle_number,
            den_position: self.story.den.clone(),
        }
    }

    /// What the session keeps of its deaths (kills, respawns, community influences).
    #[must_use]
    pub fn session_deaths(&self) -> &crate::creature::SessionDeaths {
        &self.world.session
    }

    /// How the cycle ended, once it has; taken by the process that ends the game.
    pub fn take_session_end(&mut self) -> Option<SessionEnd> {
        self.session_end.take()
    }

    /// What the game started from.
    #[must_use]
    pub fn story(&self) -> &StoryStart {
        &self.story
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

    /// The camera's sounds, the shared random stream and the map discovery, for what
    /// camera 0 updates before the rooms (the HUD).
    pub fn camera_io(&mut self) -> (&mut Sounds, &mut UnityRandom, &mut MapDiscovery) {
        (&mut self.sounds, &mut self.rng, &mut self.map_discovery)
    }

    /// Camera 0's HUD map as the player's exit discovery reads it from the next tick on.
    pub fn set_hud_map(&mut self, link: HudMapLink) {
        self.hud_map = link;
    }

    /// Camera 0's palette darkness (`Room.Darkness` in the camera's room).
    pub fn set_camera_darkness(&mut self, darkness: f32) {
        self.camera_darkness = darkness;
    }

    #[must_use]
    pub fn map_discovery(&self) -> &MapDiscovery {
        &self.map_discovery
    }

    pub fn map_discovery_mut(&mut self) -> &mut MapDiscovery {
        &mut self.map_discovery
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

    /// `ShortcutHandler.Update`: the vessels in the tubes, newest first, then the lobby.
    /// A room exit leading nowhere returns from the whole update.
    fn update_shortcuts(&mut self, observer: &mut dyn PlayerObserver) {
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
    fn player_in_lobby(&self) -> bool {
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
                VesselOwner::Player => self.player_vessel_step(i, vessel, observer),
                VesselOwner::Shortcut(v) => self.creature_lobby_step(v, vessel),
                VesselOwner::BatHive(id) => self.bat_hive_lobby_step(id, vessel),
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
            if let Err(e) = self
                .world
                .activate_room(lobby.room, &mut self.ids, &mut self.rng)
            {
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
            self.shortcuts.transport.push(Transport::Player);
            if node.kind == NodeType::Exit {
                self.blink_arrival(lobby, 2.5, CreatureType::Slugcat);
            }
        }
        log::info!(
            "entered room {} after {} shortcut updates in the lobby",
            self.world
                .abstract_room(lobby.room)
                .map_or("?", |r| r.name.as_str()),
            lobby.updates
        );
        self.camera_changed_room(self.camera_room, lobby.room, observer);
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

    /// A vessel came out of the lobby into exit node `lobby.entrance_node`: the room
    /// blinks the entrance leading there by `fac` (2.5 for a slugcat, else 1.5), which
    /// camera 0, if it already shows the room, colours as `creature`.
    fn blink_arrival(&mut self, lobby: LobbyVessel, fac: f32, creature: CreatureType) {
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
                creature,
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
            .filter(|p| player_room == index && !p.in_shortcut());
        let chunks = match here {
            Some(p) => &mut p.body.chunks[..],
            None => &mut [][..],
        };
        self.world
            .update_water(index, chunks, &mut self.rng, sounds);
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        let room = realized.room.clone();
        realized.water_glitter.update();
        if let Some(blinking) = &mut realized.shortcuts_blinking {
            blinking.update(&room, &mut self.rng);
        }
        self.run_flies_room_ai(index, observer);
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
        objects.extend(self.room_fly_objects(index));
        objects.extend(self.room_drip_objects(index));
        objects.extend(self.room_weapon_objects(index));
        objects.extend(self.room_trigger_objects(index));
        objects.extend(self.room_cosmetic_objects(index));
        objects.sort_by_key(|&(seq, _)| std::cmp::Reverse(seq));
        let mut player_done = false;
        for (seq, object) in objects {
            if !player_done && self.player_seq > seq {
                self.update_room_player_now(index, input, observer);
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
                        self.world.kill_tag_tick(id);
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
                RoomObject::Fly(id) => self.run_fly(index, id, &room, observer),
                RoomObject::Hover(id) => self.run_hover(index, id),
                RoomObject::Drip(id) => self.run_drip(index, id, &room),
                RoomObject::Weapon(id) => self.run_weapon(index, id, observer),
                RoomObject::WeaponLoop(id) => self.run_weapon_loop(index, id),
                RoomObject::Trigger(id) => self.run_trigger_object(index, id),
                RoomObject::Cosmetic(id) => self.world.update_cosmetic(index, id, &mut self.rng),
            }
        }
        self.remove_slated_trigger_objects();
        if !player_done {
            self.update_room_player_now(index, input, observer);
        }
        self.update_room_scripts(index, observer);
        // The door joined the update list in `Room.Loaded`, before everything else.
        self.update_shelter_door(index);
        self.world.collide_weapons(index, &mut self.rng);
    }

    /// The player's turn in room `index`'s update list, reading the room as it is now
    /// (a spear updated before it may have added poles).
    fn update_room_player_now(
        &mut self,
        index: i32,
        input: Input,
        observer: &mut dyn PlayerObserver,
    ) {
        if let Some(room) = self.world.realized(index).map(|r| Arc::clone(&r.room)) {
            self.update_room_player(index, &room, input, observer);
        }
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
            food: self.food_in_stomach().cast_signed(),
            jump: [inputs[0].jump, inputs[1].jump],
            entering_shortcut: p.entering_shortcut(),
            in_shortcut: p.in_shortcut(),
            standing: p.is_standing(),
            animation: p.animation(),
            hips_contact_y: chunks[1].contact().y,
            input_x: inputs[0].x,
            input_y: inputs[0].y,
            pickup: inputs[0].pickup,
            throw: inputs[0].throw,
            eat_counter: p.grab_state().eat_counter,
        })
    }

    fn story_stats(&self) -> StoryStats {
        let stats = &self.campaign.stats;
        StoryStats {
            cycle: self.story.cycle_number,
            food_to_hibernate: stats.food_to_hibernate,
            max_food: stats.max_food,
            karma_cap: self.story.karma_cap,
            yellow: self.campaign.name == rw_assets::SlugcatName::Yellow,
            how_well_is_player_doing: -1.0,
            ever_met_moon: self.story.ever_met_moon,
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
        let edibles = self.room_edibles(room_index);
        let story = self.story_stats();
        let instructions = self.input_instruction_views();
        let forced_directions = self.forced_direction_views();
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
                edibles,
                player_karma: self.story.karma,
                camera: self.camera_view,
                story,
                rain,
                hud: self.script_tick.hud_with_control_preset,
                viewed: room_index == self.camera_room,
                effects: Vec::new(),
                instructions,
                forced_directions,
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
                OverseerCall::FlyingWeapon(w) => run.flying_weapon(w),
                OverseerCall::HitByWeapon { by_slugcat } => run.hit_by_weapon(by_slugcat),
                #[cfg(test)]
                OverseerCall::Communication => run.communication_update(),
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
        self.world.put_overseer(id, parts, pos);
        self.world.rain_cycle = rain;
        self.apply_overseer_effects(id, effects, observer);
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
                        owner: VesselOwner::Teleport(id),
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
                OverseerEffect::Cosmetic(kind) => {
                    let room = self.world.creature(id).pos.room;
                    self.world.add_cosmetic(room, kind);
                }
                OverseerEffect::Destroy => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(id).realized
                    {
                        body.in_room = false;
                        body.slated = true;
                    }
                }
                OverseerEffect::InstructionFollowed(instruction) => {
                    self.instruction_followed(instruction);
                }
                OverseerEffect::InstructionCompleted(instruction) => {
                    self.instruction_completed(instruction);
                }
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

    /// The player's slot in room `index`'s update list, when its body is there (alive
    /// or dead): `Player.Update` with the bat-fly catch and `GrabUpdate`, the drips its
    /// impacts threw, the grasps a stun or death let go, the carried flies pulled and
    /// sucked into a shortcut, then its graphics and `GraphicsModuleUpdated`.
    fn update_room_player(
        &mut self,
        index: i32,
        room: &Arc<Room>,
        input: Input,
        observer: &mut dyn PlayerObserver,
    ) {
        let player_room = self.pos().room;
        let updates = self
            .player
            .as_ref()
            .is_some_and(|p| player_room == index && !p.in_shortcut());
        if !updates || self.world.realized(index).is_none() {
            return;
        }
        let last = self.last_coords.get(&self.player_id).copied();
        let moved = last.unwrap_or(WorldCoordinate::new(0, 0, 0, 0))
            != self.world.creature(self.player_id).pos;
        self.creature_new_tile(self.player_id);
        self.player_graphics = observer.has_player_graphics();
        self.report_spawn_room(index);
        self.player_sleep_update(room, input);
        self.player_map_discovery(index, room, moved);
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        if let Some(p) = self.player.as_mut() {
            p.exhaustion_update(&mut self.rng, sounds);
        }
        self.update_karma_flower_grow_pos(index, room);
        let water = self.world.realized_mut(index).and_then(|r| r.water.take());
        let ceiling_drips = self
            .world
            .realized(index)
            .map_or(0.0, |r| r.settings.ceiling_drips);
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let was_stunned = player.is_stunned();
        player.set_ceiling_drips(ceiling_drips);
        if player.update_before_catch(room, water.as_ref(), input, &mut self.rng, sounds) {
            self.player_catch(index, observer);
            self.player_shelter_step(index, room);
            let sounds = if index == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            if let Some(player) = self.player.as_mut() {
                player.update_after_clamp(room, water.as_ref(), &mut self.rng, sounds);
            }
        }
        self.game_over_on_death(room);
        self.apply_player_releases();
        self.add_player_drips(index);
        self.pull_carried();
        if self.player.as_ref().is_some_and(Player::movement_ran) {
            self.grab_update(index, room, water.as_ref(), observer);
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.water = water;
        }
        let in_tube = self.player.as_ref().is_some_and(Player::in_shortcut);
        if in_tube {
            self.carried_sucked_in();
        }
        self.world.player_body = self.player_body();
        let around = PlayerSurroundings {
            world: &self.world,
            grasps: self.player_grasps(),
            stomach: self
                .player
                .as_ref()
                .is_some_and(|p| p.object_in_stomach().is_some()),
            room: index,
            objects: self.room_physical_objects(index),
            hungry: self.food_in_stomach() < self.max_food(),
        };
        let water = self.world.realized(index).and_then(|r| r.water.as_ref());
        if let Some(player) = self.player.as_ref() {
            observer.updated(player, room, water, &around, &mut self.rng);
        }
        let present = self
            .player
            .as_ref()
            .is_some_and(|p| !p.in_shortcut() && !p.is_destroyed());
        if present {
            self.graphics_module_updated(observer);
        }
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let tile = player.head().pos.tile();
        let mut pos = self.world.creature(self.player_id).pos;
        pos.tile = tile;
        self.world.place_creature(self.player_id, pos);
        let Some(player) = self.player.as_mut() else {
            return;
        };
        if let Some(entrance) = player.shortcut_vessel_tile() {
            self.shortcuts.transport.push(Transport::Player);
            self.sucked_in(room, entrance);
            return;
        }
        let Some(player) = self.player.as_mut() else {
            return;
        };
        if player.is_destroyed() {
            let sounds = if index == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            sounds.owner_removed();
            log::info!("player fell out of the room and was destroyed");
            self.player = None;
            self.sync_player_realized();
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
            && let Err(e) = self.world.activate_room(dest, &mut self.ids, &mut self.rng)
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
            .find(|v| v.owner == VesselOwner::Player)
            .copied()
    }

    /// `transportVessels` in order: `None` for the player's, else the creature's vessel.
    pub fn transport_vessels(&self) -> impl Iterator<Item = Option<CreatureVessel>> + '_ {
        self.shortcuts.transport.iter().map(|t| match t {
            Transport::Player => None,
            Transport::Creature(v) => Some(*v),
        })
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

    /// Gives the stream back after [`Self::take_rng`].
    pub fn set_rng(&mut self, rng: UnityRandom) {
        self.rng = rng;
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Food added this cycle.
    #[must_use]
    pub fn session_food(&self) -> i32 {
        self.session_food
    }

    /// What the player ate this cycle.
    #[must_use]
    pub fn eaten(&self) -> &[EntityId] {
        &self.eaten
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
        // The port's teleport lets go of what the hands held.
        if self.player.is_some() {
            self.release_all_grasps();
        }
        let player = self.player.get_or_insert_with(Player::new);
        player.clear_vessel();
        self.shortcuts.drop_player();
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
        self.sync_player_realized();
    }

    /// Kills the player (for tests and tools), as `Player.Die` from its room.
    pub fn kill_player(&mut self) {
        if let Some(p) = &mut self.player {
            p.die();
        }
        let room = self
            .world
            .realized(self.pos().room)
            .map(|r| Arc::clone(&r.room));
        if let Some(room) = room {
            self.game_over_on_death(&room);
        }
    }
}
