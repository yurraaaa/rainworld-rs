//! `RainWorldGame` for one player: the world, the player's place in it, the shortcut
//! handler with its between-rooms lobby, the room realizer, and the order of a tick.
//!
//! Creatures: the world's population and its overseers' abstract AI
//! ([`crate::creature`]); the player is an `AbstractCreature` too. Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`, `shortcuts.md` section 5,
//! `creature-model/`.
//!
//! Mirrors: RainWorldGame.cs

pub(crate) mod camera;
pub(crate) mod camera_image;
pub mod campaign;
pub mod map_discovery;
pub mod room_script;
pub(crate) mod screen_shake;
pub mod shelter;
pub(crate) mod shortcut;
pub(crate) mod shortcut_helper;
pub(crate) mod spawn;
pub mod trigger;

#[cfg(test)]
mod tests;

use std::sync::Arc;

mod dev_turn;
mod exit_turn;
mod instructions_turn;
pub(crate) mod loading_turn;
pub use crate::effects::lights::turn::PlayerGlow;
mod game_over_turn;
mod map_turn;
mod observer;
mod rain_turn;
mod shortcut_helper_turn;
mod shortcut_turn;

pub use crate::creature::ai_modules::heavy_ai_turn::HeavyAiThrottle;
pub use crate::creature::lizard::ai_turn::UtilityReadings;
pub use crate::item::turn::Grabability;
pub use dev_turn::DevRainControl;
pub use game_over_turn::GameOver;
pub use observer::PlayerObserver;
mod shelter_turn;
mod trigger_turn;

use rustc_hash::FxHashMap;
use rw_assets::{CreatureType, NodeType, Room, ShortcutType};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, SlugcatStats, StoryStart};
use crate::creature::overseer::TutorialMessage;
use crate::creature::population::PopulationContext;
use crate::creature::{
    CreatureId, CreatureTemplate, EntityId, IdIssuer, PlayerGuideState, RealizedCreature,
    WorldCoordinate,
};
use crate::game::camera::CameraView;
use crate::game::shortcut::{CreatureVessel, Transport};
use crate::item::{Entity, ObjectId};
use crate::room_script::{ScriptTick, StorySession};
use crate::shelter::{SessionEnd, ShelterDoor};
use crate::world::{AbstractTick, FollowCreature, RainCycle, RegionState, RoomRealizer, World};
use crate::{
    CameraShake, EmitterPlace, EntranceColor, HudMapLink, Input, LobbyVessel, MapDiscovery, Player,
    ScreenMovement, ShortcutHandler, Sounds, Vec2, VesselOwner, determine_safe_spawn_tile,
};

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
    /// `drawPositions[0, 0]` and `drawPositions[1, 0]` (the spear on the back's anchors).
    pub draw_positions: [Vec2; 2],
    /// `head.pos`.
    pub head: Vec2,
}

/// A physical object of the room the player's looker may look at: an item or creature, or
/// a coral stem's segment (D41).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookTarget {
    Entity(Entity),
    Segment(crate::SegmentRef),
}

impl LookTarget {
    /// The creature or item, if it is one.
    #[must_use]
    pub fn entity(self) -> Option<Entity> {
        match self {
            Self::Entity(e) => Some(e),
            Self::Segment(_) => None,
        }
    }
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
    pub objects: Vec<LookTarget>,
    /// `FoodInStomach < MaxFoodInStomach`.
    pub hungry: bool,
    /// Hand 0's `grabbedChunk` (position, radius) when it is a `HeavyCarry`.
    pub heavy_carry: Option<(Vec2, f32)>,
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
pub(crate) enum RoomObject {
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
    /// The room's `DrippingSound`.
    DrippingSound,
    /// An emitter of the `DrippingSound`'s loop, by its place in the update list.
    DrippingSoundLoop(u64),
    /// The room's `Lightning`.
    Lightning,
    /// An emitter of the `Lightning`'s sound loops, by its place in the update list.
    LightningLoop(u64),
    /// An `ElectricDeath` by its place in the update list.
    ElectricDeath(u64),
    /// An emitter of an `ElectricDeath`'s sound loops, by its place in the update list.
    ElectricDeathLoop(u64),
    /// A `RoomBorderPushBack` by its place in the update list.
    BorderPushBack(u64),
    /// The room's `DeathFallGraphic`.
    DeathFall,
    /// An emitter of the water's surface loop, by its place in the update list.
    WaterSoundLoop(u64),
    /// A realized rock or spear.
    Weapon(ObjectId),
    /// A `Spark` or `ExplosionSpikes` by id.
    Cosmetic(u64),
    /// A rock's or spear's loop emitter, held or let go, by its place in the update list.
    WeaponLoop {
        owner: ObjectId,
        order: u64,
    },
    /// A `ChunkSoundEmitter` of a lizard's graphics' loop, by its place in the update list.
    LizardLoop {
        owner: CreatureId,
        order: u64,
    },
    /// A loop emitter whose item was swallowed, by its place in the update list.
    OrphanLoop(u64),
    /// An emitter of the player's slide loop, by its place in the update list.
    SlideLoop(u64),
    /// A `ChunkSoundEmitter` of a lizard's voice, by its place in the update list.
    LizardVoice {
        owner: CreatureId,
        order: u64,
    },
    /// A trigger checker or an object a trigger added, by id.
    Trigger(u64),
    /// A `HologramLight` by id.
    HoloLight(u64),
    /// A `LightSource` by id.
    LightSource(u64),
    /// A cosmetic insect by its place in the update list.
    Insect(u64),
    /// A dead `TokenStalk` by its place in the update list.
    TokenStalk(u64),
    /// A `BackgroundScene` by its place in the update list.
    BackgroundScene(u64),
    /// A scene's `DistantLightning` by its place in the update list.
    SceneElement(u64),
    /// A `DustpuffSpawner` by its place in the update list.
    DustpuffSpawner(u64),
    /// A `DustPuff` by its place in the update list.
    DustPuff(u64),
    /// A dandelion, spark, speck or one of their spawners.
    Particle(crate::effects::particles::turn::ParticleRef),
    /// A `Rainbow` by its place in the update list.
    Rainbow(u64),
    /// A `LightBeam` by its place in the update list.
    LightBeam(u64),
    /// A light fixture, its ember or a waterfall.
    Fixture(crate::superstructure::electric::room_fixtures::FixtureRef),
    /// A superstructure projector, its part, a scan line set, its line or a star matrix.
    Projection(crate::superstructure::projections::room_projections::ProjectionRef),
    /// The room's `CoralNeuronSystem` or a `WallMycelia`.
    Coral(crate::superstructure::coral::turn::CoralRef),
    /// A `CircuitBit`, `CircuitConnector`, `CircuitCrab` or `CoralCircuit`.
    Circuit(crate::superstructure::coral::circuit_turn::CircuitRef),
    /// The room's `ShortcutHelper`.
    ShortcutHelper,
    /// The room's `AntiGravity`.
    AntiGravity,
    /// The room's `RoomSpecificScript`s.
    Script,
    Lizard(CreatureId),
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
    pub(crate) campaign: &'static SlugcatCampaign,
    /// The save state's start values.
    pub(crate) story: StoryStart,
    /// `session.characterStats`: `new SlugcatStats(name, saveState.malnourished)`,
    /// replaced by `SetMalnourished`.
    pub(crate) character_stats: SlugcatStats,
    pub(crate) world: World,
    pub(crate) shortcuts: ShortcutHandler,
    /// `None` in a single-room world.
    realizer: Option<RoomRealizer>,
    pub(crate) player: Option<Player>,
    /// The player's `AbstractCreature` (`Players[0]`).
    pub(crate) player_id: CreatureId,
    /// `cameras[0].room`.
    pub(crate) camera_room: i32,
    /// See [`RainWorldGame::now_viewed_modules`].
    pub(crate) now_viewed_modules: Vec<CreatureId>,
    camera_move: Option<CameraMove>,
    /// Camera 0's positions as the app's room camera last reported them.
    pub(crate) camera_view: CameraView,
    /// Camera 0's shake and its window during the tick.
    pub(crate) camera_shake: CameraShake,
    /// The player's place in its room's `updateList`.
    pub(crate) player_seq: u64,
    pub(crate) hud_events: Vec<HudEvent>,
    pub(crate) rng: UnityRandom,
    pub(crate) sounds: Sounds,
    /// Sounds of rooms the camera is not in: the game's microphone ignores them, so
    /// they make no draws.
    pub(crate) silent: Sounds,
    /// `nextIssuedId`.
    pub(crate) ids: IdIssuer,
    /// `saveState.seed` of the new game's `SaveState`.
    save_state_seed: i32,
    /// `saveState.miscWorldSaveData.playerGuideState`.
    pub(crate) guide: PlayerGuideState,
    /// `updateAbstractRoom`: the abstract room updated last.
    update_abstract_room: usize,
    /// The start tile, reused by respawns in the start room.
    start: Start,
    pub(crate) tick: u64,
    /// What the room scripts read of the process manager.
    pub(crate) script_tick: ScriptTick,
    /// The last hologram id issued.
    pub(crate) hologram_ids: u64,
    /// The player's room when the guide's being brought was last logged.
    guide_brought_logged_room: Option<i32>,
    /// Each realized creature's `Creature.lastCoord`; a new body starts at the default
    /// coordinate (room 0, tile (0, 0)).
    pub(crate) last_coords: FxHashMap<CreatureId, WorldCoordinate>,
    /// `ColorEntrance` calls on camera 0 since the last `take_entrance_colors`.
    pub(crate) entrance_colors: Vec<EntranceColor>,
    /// Whether the player had a graphics module at its last update.
    pub(crate) player_graphics: bool,
    /// `progression.mapDiscoveryTextures`.
    map_discovery: MapDiscovery,
    /// `RequestHeavyAi`'s state (the game's statics).
    pub(crate) heavy_ai: HeavyAiThrottle,
    dev_rain: DevRainControl,
    /// Camera 0's HUD map, as the app last reported it.
    hud_map: HudMapLink,
    /// Camera 0's `PaletteDarkness()`, as the app last reported it.
    pub(crate) camera_darkness: f32,
    /// Camera 0's level image and palette, as the app last reported them.
    pub(crate) camera_image: Option<crate::CameraImage>,
    /// `Player.glowing` and its graphics' light.
    pub(crate) player_glow: crate::effects::lights::turn::PlayerGlow,
    /// The camera's room change the insects have yet to follow.
    pub(crate) insect_view: Option<crate::effects::particles::insect::turn::InsectViewChange>,
    /// Set when the cycle ends (the shelter closes, the player starves).
    session_end: Option<SessionEnd>,
    /// Food added this cycle (`saveState.totFood`'s share).
    pub(crate) session_food: i32,
    /// What the player ate this cycle (`SessionRecord.AddEat`), by entity ID.
    pub(crate) eaten: Vec<EntityId>,
    /// The realized shelters' doors.
    pub(crate) shelter_doors: Vec<ShelterDoor>,
    /// `Room.chunkGlue`: a spear's chunk moved onto another chunk after the update list.
    pub(crate) chunk_glue: Vec<(ObjectId, crate::creature::lizard::hits_turn::ChunkRef, Vec2)>,
    closed_shelter_regions: Vec<String>,
    /// Shelters the player was spat out into (`TempDiscoverShelter`), until taken.
    pub(crate) entered_shelters: Vec<String>,
    /// The player's `karmaFlowerGrowPos`.
    karma_flower_grow_pos: Option<WorldCoordinate>,
    /// The player's death called `GameOver`; taken by the app.
    pub(crate) game_over: Option<GameOver>,
    /// `GameOver` called `GoToRedsGameOver`; taken by the app.
    reds_game_over: bool,
    /// The realized rooms' trigger checkers and the objects their events added.
    trigger_objects: Vec<crate::trigger::TriggerObject>,
    /// What the game asked of the music player this tick; taken by the app.
    music_requests: Vec<crate::trigger::MusicRequest>,
    /// Camera 0's HUD as the checkers read it, set by the app.
    trigger_hud: crate::trigger::TriggerHud,
    /// The start room's player waits for a loading stage to realize it (the game's
    /// `realizedCreature == null` of the player made with the session).
    start_player_pending: bool,
}

impl RainWorldGame {
    /// The constructor's order for the parts ported: `nextIssuedId`, the new game's
    /// `SaveState` seed, the world (`World`'s rain cycle; in a region, the population and
    /// the overseers; `FliesWorldAI`), the player's `AbstractCreature`, the first room
    /// activated and shown by camera 0, the realizer; the room's creatures, the player
    /// among them, are realized in list order when its loading reaches `ReadyForAI`.
    pub fn new(
        world: World,
        campaign: &'static SlugcatCampaign,
        start: Start,
        rng: UnityRandom,
        sounds: Sounds,
    ) -> Self {
        let story = StoryStart::new_game(campaign);
        Self::new_story(world, campaign, story, start, rng, sounds)
    }

    /// [`Self::new`] from `story`: a new game's or a saved one's.
    pub fn new_story(
        world: World,
        campaign: &'static SlugcatCampaign,
        story: StoryStart,
        start: Start,
        mut rng: UnityRandom,
        sounds: Sounds,
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
        let screen_width = world.screen_size().x;
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
            now_viewed_modules: Vec::new(),
            camera_move: Some(CameraMove {
                room: start.room,
                camera: 0,
            }),
            camera_view: CameraView {
                room: start.room,
                current: 0,
                next: 0,
            },
            camera_shake: CameraShake::with_screen_width(screen_width),
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
            guide_brought_logged_room: None,
            last_coords: FxHashMap::default(),
            entrance_colors: Vec::new(),
            player_graphics: false,
            map_discovery: MapDiscovery::default(),
            heavy_ai: HeavyAiThrottle::default(),
            dev_rain: DevRainControl::default(),
            hud_map: HudMapLink::default(),
            camera_darkness: 0.0,
            camera_image: None,
            player_glow: crate::effects::lights::turn::PlayerGlow::default(),
            insect_view: Some(crate::effects::particles::insect::turn::InsectViewChange {
                from: None,
                to: start.room,
            }),
            session_end: None,
            session_food: 0,
            eaten: Vec::new(),
            shelter_doors: Vec::new(),
            chunk_glue: Vec::new(),
            closed_shelter_regions: Vec::new(),
            entered_shelters: Vec::new(),
            karma_flower_grow_pos: None,
            game_over: None,
            reds_game_over: false,
            trigger_objects: Vec::new(),
            music_requests: Vec::new(),
            trigger_hud: crate::trigger::TriggerHud::default(),
            start_player_pending: true,
        };
        if let Err(e) = game.activate_room(start.room) {
            log::error!("cannot realize the first room: {e}");
        }
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
        world.rain_cycle = RainCycle::new_story(campaign.name, story.cycle_number, rng);
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
    pub(crate) fn pos(&self) -> WorldCoordinate {
        self.world.creature(self.player_id).pos
    }

    pub(crate) fn set_pos(&mut self, pos: WorldCoordinate) {
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
        if self.campaign.name == rw_assets::SlugcatName::Red {
            player.give_spear_on_back();
        }
        player.set_object_in_stomach(self.stomach_from_save());
        self.world.player_mass = player.total_mass();
        self.karma_flower_grow_pos = None;
        self.player_wakes_in_shelter(&mut player);
        observer.created(&player, &mut self.rng);
        self.player_glow = crate::effects::lights::turn::PlayerGlow {
            glowing: self.is_story_session() && self.story.the_glow,
            light: None,
        };
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

    /// One 40 Hz tick in `RainWorldGame.Update`'s order: camera 0's shake (the end of
    /// `RoomCamera.Update`; the app ran its window's lerp before the tick), `GlobalRain.Update`,
    /// the room realizer, one
    /// abstract room (in turn, with the time of a whole round), the rain cycle (with `RainHit`), the
    /// path finders' budget, the shortcut handler on every third tick, every active room (newest first: the entry
    /// delay, its water, and the player in it with its graphics), then the loading
    /// step (`loadingRooms`' preparers).
    pub fn tick(&mut self, input: Input, observer: &mut dyn PlayerObserver) {
        // `RoomCamera.Update`: the room change (`ApplyPositionChange`) before the shake.
        self.apply_insect_view_change();
        self.update_camera_shake();
        self.update_global_rain();
        let follow = self.follow_creature();
        if let (Some(realizer), Some(follow)) = (&mut self.realizer, follow) {
            realizer.update(&mut self.world, follow, self.camera_room, &mut self.rng);
        }
        self.rooms_loaded();
        self.update_abstract_room();
        self.enter_from_abstract_rooms();
        self.update_world_processes(observer);
        self.dev_update_rain_timers();
        self.update_rain_hit();
        self.update_broken_anti_gravity();
        self.world.rain_cycle.water_cycle.update(&mut self.rng);
        self.rain_request_stop_song();
        self.world.path_finders.divider.update();
        if self.shortcuts.divide() {
            self.update_shortcuts(observer);
        }
        for room in self.world.active_rooms().into_iter().rev() {
            self.update_room(room, input, observer);
        }
        self.update_loading_rooms(observer);
        self.sync_player_state();
        self.heavy_ai.end_of_tick();
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
            self.world.player_dead = dead;
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

    /// Room `index`'s update list besides the player, newest first (`Room.Update` walks
    /// `updateList` from its end), each with its place in the list.
    pub(crate) fn room_update_list(&self, index: i32) -> Vec<(u64, RoomObject)> {
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
        objects.extend(self.room_lizard_objects(index));
        objects.extend(self.room_drip_objects(index));
        objects.extend(self.room_dripping_sound_objects(index));
        objects.extend(self.room_lightning_objects(index));
        objects.extend(self.room_electric_death_objects(index));
        objects.extend(self.room_border_push_back_objects(index));
        objects.extend(self.room_death_fall_objects(index));
        objects.extend(self.room_water_sound_objects(index));
        objects.extend(self.room_weapon_objects(index));
        objects.extend(
            self.world
                .room_orphan_loops(index)
                .into_iter()
                .map(|order| (order, RoomObject::OrphanLoop(order))),
        );
        objects.extend(self.room_trigger_objects(index));
        objects.extend(self.room_cosmetic_objects(index));
        objects.extend(self.room_holo_light_objects(index));
        objects.extend(self.room_light_source_objects(index));
        objects.extend(self.room_slide_loop_objects(index));
        objects.extend(self.room_insect_objects(index));
        objects.extend(self.room_token_stalk_objects(index));
        objects.extend(self.room_background_scene_objects(index));
        objects.extend(self.room_particle_objects(index));
        objects.extend(self.room_rainbow_objects(index));
        objects.extend(self.room_light_beam_objects(index));
        objects.extend(self.room_fixture_objects(index));
        objects.extend(self.room_projection_objects(index));
        objects.extend(self.room_anti_gravity_objects(index));
        objects.extend(self.room_script_objects(index));
        objects.extend(self.room_coral_objects(index));
        objects.extend(self.room_circuit_objects(index));
        objects.extend(self.room_shortcut_helper_objects(index));
        objects.sort_by_key(|&(seq, _)| std::cmp::Reverse(seq));
        objects
    }

    /// `Room.Update` of active room `index`: the entry delay, the water (pushed by the
    /// room's bodies, adding its bubbles), the social event recognizer, then the update
    /// list with the player.
    fn update_room(&mut self, index: i32, input: Input, observer: &mut dyn PlayerObserver) {
        let player_room = self.pos().room;
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        realized.update_wait();
        realized.updated = true;
        // `Water.Update` begins with its surface loop.
        self.run_water_sound(index);
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let player_seq = self.player_seq;
        let here = self
            .player
            .as_mut()
            .filter(|p| player_room == index && !p.in_shortcut())
            .map(|p| (player_seq, &mut p.body.chunks[..]));
        self.world.update_water(index, here, &mut self.rng, sounds);
        self.social_event_recognizer_update(index);
        self.ambient_ceiling_drip(index);
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        let room = realized.room.clone();
        realized.water_glitter.update();
        if let Some(blinking) = &mut realized.shortcuts_blinking {
            blinking.update(&room, &mut self.rng);
        }
        self.run_flies_room_ai(index, observer);
        let objects = self.room_update_list(index);
        let mut player_done = false;
        // What the insects read of the room's creatures, until anything else moves.
        let mut insect_creatures = None;
        for (seq, object) in objects {
            if !player_done && self.player_seq > seq {
                self.update_room_player_now(index, input, observer);
                player_done = true;
                insect_creatures = None;
            }
            if let RoomObject::Insect(order) = object {
                self.run_insect(index, order, &mut insect_creatures);
                continue;
            }
            insect_creatures = None;
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
                RoomObject::Lizard(id) => self.run_lizard(index, id, &room, observer),
                RoomObject::Hover(id) => self.run_hover(index, id),
                RoomObject::Drip(id) => self.run_drip(index, id, &room),
                RoomObject::DrippingSound => self.run_dripping_sound(index),
                RoomObject::DrippingSoundLoop(order) => {
                    self.run_dripping_sound_emitter(index, order);
                }
                RoomObject::Lightning => self.run_lightning(index),
                RoomObject::LightningLoop(order) => self.run_lightning_emitter(index, order),
                RoomObject::ElectricDeath(order) => self.run_electric_death(index, order),
                RoomObject::ElectricDeathLoop(order) => {
                    self.run_electric_death_emitter(index, order);
                }
                RoomObject::BorderPushBack(order) => self.run_border_push_back(index, order),
                RoomObject::DeathFall => self.run_death_fall(index),
                RoomObject::WaterSoundLoop(order) => self.run_water_sound_emitter(index, order),
                RoomObject::Weapon(id) => self.run_weapon(index, id, observer),
                RoomObject::WeaponLoop { owner, order } => {
                    self.run_weapon_loop(index, owner, order);
                }
                RoomObject::LizardLoop { owner, order } => {
                    self.run_lizard_loop(index, owner, order);
                }
                RoomObject::OrphanLoop(order) => self.run_orphan_loop(index, order),
                RoomObject::SlideLoop(order) => self.run_slide_loop(index, order),
                RoomObject::LizardVoice { owner, order } => {
                    self.run_lizard_voice_emitter(index, owner, order)
                }
                RoomObject::Trigger(id) => self.run_trigger_object(index, id),
                RoomObject::Cosmetic(id) => self.run_cosmetic(index, id),
                RoomObject::HoloLight(id) => self.run_holo_light(index, id, observer),
                RoomObject::LightSource(id) => self.run_light_source(index, id),
                RoomObject::TokenStalk(order) => self.run_token_stalk(index, order),
                RoomObject::BackgroundScene(order) => self.run_background_scene(index, order),
                RoomObject::SceneElement(order) => self.run_scene_element(index, order),
                RoomObject::DustpuffSpawner(order) => self.run_dustpuff_spawner(index, order),
                RoomObject::DustPuff(order) => self.run_dust_puff(index, order),
                RoomObject::Particle(p) => self.run_particle(index, p),
                RoomObject::Rainbow(order) => self.run_rainbow(index, order),
                RoomObject::LightBeam(order) => self.run_light_beam(index, order),
                RoomObject::Fixture(f) => self.run_fixture(index, f),
                RoomObject::Projection(p) => self.run_projection(index, p),
                RoomObject::AntiGravity => self.run_anti_gravity(index),
                RoomObject::ShortcutHelper => self.run_shortcut_helper(index),
                RoomObject::Script => self.update_room_scripts(index, observer),
                RoomObject::Coral(c) => self.run_coral(index, c),
                RoomObject::Circuit(c) => self.run_circuit_object(index, c),
                RoomObject::Insect(_) => {}
            }
        }
        self.remove_slated_trigger_objects();
        self.remove_slated_holo_lights(index);
        self.remove_slated_particles(index);
        self.remove_slated_fixtures(index);
        self.remove_slated_projections(index);
        if !player_done {
            self.update_room_player_now(index, input, observer);
        }
        // The door joined the update list in `Room.Loaded`, before everything else.
        self.update_shelter_door(index);
        self.apply_chunk_glue();
        self.collide_creatures(index);
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

    /// The player's slide loop emitters in room `index`'s update list.
    fn room_slide_loop_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.player.as_ref().map_or_else(Vec::new, |p| {
            p.slide_loop_emitters()
                .filter(|e| e.room == index)
                .map(|e| (e.update_order, RoomObject::SlideLoop(e.update_order)))
                .collect()
        })
    }

    /// The `ChunkSoundEmitter.Update` of the slide loop emitter that joined room `index`
    /// at `order`: it dies when the player left the room (`room` null in a shortcut) or
    /// nothing kept it alive since its last update; its sound is cut then.
    fn run_slide_loop(&mut self, index: i32, order: u64) {
        let player_room = self.pos().room;
        let (camera_room, tick) = (self.camera_room, self.tick);
        let Some(p) = self.player.as_mut() else {
            return;
        };
        let owner_room = (!p.in_shortcut() && !p.is_destroyed()).then_some(player_room);
        let Some(e) = p
            .slide_loop_emitter_mut(order)
            .filter(|e| e.room == index && !e.slated)
        else {
            return;
        };
        e.update(owner_room, camera_room, tick);
        if e.slated {
            let sounds = if index == camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            sounds.stop_emitter(e.sound);
        }
    }

    /// The room's scripts at their place in the update list: added by `Room.Loaded` after
    /// its effects and placed objects, before its creatures (so after the player and
    /// the creatures in a tick).
    fn update_room_scripts(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let here = self.pos().room == index;
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        let pixel_height = realized.room.pixel_height();
        let mut gravity = None;
        for script in &mut realized.scripts {
            let player = self
                .player
                .as_mut()
                .filter(|p| here && !p.in_shortcut() && !p.is_destroyed());
            let set = script.update(player, pixel_height, self.script_tick, observer);
            gravity = set.or(gravity);
        }
        if let Some(g) = gravity {
            realized.gravity = g;
        }
    }

    /// The room's scripts' place in its update list.
    fn room_script_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .filter(|r| !r.scripts.is_empty())
            .map(|r| (r.script_order, RoomObject::Script))
            .into_iter()
            .collect()
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
        self.monk_hologram_light(index);
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
        self.player_danger_grasp_update(index, room, input, observer);
        let water = self.world.realized_mut(index).and_then(|r| r.water.take());
        let ceiling_drips = self
            .world
            .realized(index)
            .map_or(0.0, |r| r.settings.ceiling_drips);
        let room_gravity = self.room_gravity(index);
        let ai_map = self
            .world
            .realized(index)
            .filter(|r| r.ready_for_ai())
            .and_then(|r| r.ai_map.clone());
        self.refresh_player_swim_bits();
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
        player.set_room_gravity(room_gravity);
        player.set_ai_map(ai_map);
        player.retain_released_slide_loops(|r| self.world.realized(r).is_some());
        player.set_emitter_place(EmitterPlace {
            room: index,
            camera_room: self.camera_room,
            tick: self.tick,
            update_seq: self.world.update_seq(),
        });
        let rest = player.update_before_catch(room, water.as_ref(), input, &mut self.rng, sounds);
        self.world.set_update_seq(player.emitter_place().update_seq);
        if rest {
            self.player_catch(index, observer);
            self.player_shelter_step(index, room);
            let body = self.player_hearing_body(index);
            let carry = self.heavy_carry_loan(index);
            let jump_chunk = self.jump_chunk_body(index);
            if let Some(player) = self.player.as_mut() {
                player.lend_heavy_carry(carry);
                player.lend_jump_chunk_body(jump_chunk);
            }
            let sounds = if index == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            if let Some(player) = self.player.as_mut()
                && let Some(body) = body
            {
                let mut hearing = crate::sound::noise_turn::PlayerHearing {
                    world: &mut self.world,
                    room: index,
                    water: water.as_ref(),
                    player: body,
                    camera: self.camera_view,
                    shake: &mut self.camera_shake,
                };
                player.update_after_clamp(
                    room,
                    water.as_ref(),
                    &mut self.rng,
                    sounds,
                    &mut hearing,
                );
            }
            self.return_heavy_carry();
            self.apply_jump_chunk_push();
            // `PhysicalObject.Update`'s `grabbedBy` pruning, when physics ran (not the
            // pull into a shortcut).
            if self
                .player
                .as_ref()
                .is_some_and(|p| p.before_pull().is_none())
            {
                self.world
                    .clean_grabbed_by(Entity::Creature(self.player_id));
                self.sync_player_pulled();
            }
        }
        self.apply_player_release_grabbed_by();
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
            heavy_carry: self.heavy_carry_chunk(),
        };
        let water = self.world.realized(index).and_then(|r| r.water.as_ref());
        if let Some(player) = self.player.as_ref() {
            observer.updated(player, room, water, &around, &mut self.rng);
            if observer.has_player_graphics() {
                self.player_glow_update(index);
            }
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
            && let Err(e) = self.activate_room(dest)
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

    /// The room camera cut to another camera position in the room (`MoveCamera`'s
    /// `ApplyPositionChange`, including a cut to the position it already shows).
    pub fn camera_moved(&mut self) {
        self.cameras_changed(self.camera_room);
    }

    #[must_use]
    pub fn camera_view(&self) -> CameraView {
        self.camera_view
    }

    /// Camera 0's shake state.
    #[must_use]
    pub fn camera_shake(&self) -> &CameraShake {
        &self.camera_shake
    }

    /// Hands in the app's camera window (`pos` after `Update`'s lerp and clamp,
    /// `ScreenSize.x`) before the tick.
    pub fn set_camera_pos(&mut self, pos: Vec2, screen_width: f32) {
        self.camera_shake.set_pos(pos, screen_width);
    }

    /// `rainWorld.screenSize` (the chosen resolution).
    #[must_use]
    pub fn screen_size(&self) -> Vec2 {
        self.world.screen_size()
    }

    /// What the tick's shake jump and `ScreenMovement` bumps moved the camera window by.
    pub fn take_camera_displacement(&mut self) -> Vec2 {
        self.camera_shake.take_displacement()
    }

    /// `room.ScreenMovement(...)` in room `index`.
    pub(crate) fn room_screen_movement(&mut self, index: i32, m: ScreenMovement) {
        self.world.screen_movement(
            index,
            m,
            &self.camera_view,
            &mut self.camera_shake,
            &mut self.rng,
        );
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

    /// The player's place in its room's update list (its last `AddObject`).
    #[must_use]
    pub fn player_seq(&self) -> u64 {
        self.player_seq
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

    /// Creature `id`'s shortcut vessel, in the tubes or (with `true`) in the lobby.
    #[must_use]
    pub fn creature_vessel(&self, id: CreatureId) -> Option<(CreatureVessel, bool)> {
        self.shortcuts.creature_vessel(id)
    }

    /// `OnScreenPositionOfInShortCutCreature` for the player: its own vessel's tile, or
    /// the tile of the first vessel in room `room`'s tubes whose creature has a stick
    /// with the player (its own `stuckObjects`, either end).
    #[must_use]
    pub fn player_vessel_tile(&self, room: i32) -> Option<(i32, i32)> {
        let player = self.player.as_ref()?;
        if let Some(tile) = player.shortcut_vessel_tile() {
            return Some(tile);
        }
        let me = Entity::Creature(self.player_id);
        self.transport_vessels()
            .flatten()
            .filter(|v| v.room == room)
            .find(|v| {
                self.world
                    .sticks_of(Entity::Creature(v.creature))
                    .iter()
                    .any(|s| Entity::Creature(s.a) == me || s.b == me)
            })
            .map(|v| v.tiles().0)
    }

    /// The player, unless destroyed.
    #[must_use]
    pub fn player(&self) -> Option<&Player> {
        self.player.as_ref()
    }

    /// Whether the player is in the camera's room (its body or its vessel).
    #[must_use]
    pub fn player_in_camera_room(&self) -> bool {
        self.player_in_room(self.camera_room)
    }

    /// Whether the player's position is in room `room`.
    #[must_use]
    pub fn player_in_room(&self, room: i32) -> bool {
        self.player.is_some() && self.pos().room == room
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

    /// Kills the player (for tests and tools, and `Violence` from outside its update),
    /// as `Player.Die` from its room: the hands let go at once (`Creature.Die`'s
    /// `LoseAllGrasps`).
    pub fn kill_player(&mut self) {
        if let Some(p) = &mut self.player {
            p.die(&mut self.rng);
        }
        self.apply_player_releases();
        let room = self
            .world
            .realized(self.pos().room)
            .map(|r| Arc::clone(&r.room));
        if let Some(room) = room {
            self.game_over_on_death(&room);
        }
    }
}
