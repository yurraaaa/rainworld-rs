//! Simulation state owned by the app: the game ([`RainWorldGame`]: world, rooms, player,
//! shortcuts, random stream and sounds) and the player's graphics module, seen from the
//! camera's room.
//!
//! This is the only place the app touches the simulation.
//!
//! Mirrors: RainWorldGame.cs, RoomCamera.cs (MoveObjectToInternalContainer)

use std::collections::HashMap;
use std::sync::Arc;

use rw_assets::unity::AssetBundle;
use rw_assets::world::AbstractRoomData;
use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{
    ActiveMods, PaletteTexture, Room, RoomEffectType, RoomSettings, SlugcatName, SoundTable,
    StreamingAssets,
};
use rw_audio::Follow as SoundFollow;
#[path = "game_background_scene.rs"]
mod background_scene;

use rw_graphics::{
    Color, CosmeticSprites, DandelionSprites, DecalSprites, DrawList, DripPalette, FlyGraphics,
    GreenSparkSprites, HologramFrame, HologramGraphics, HologramLightSprites, InsectDraw,
    InsectSprites, Layer, LightBeamSprites, LightBeamView, LightSourceSprites, LightSourceView,
    LizardColors, LizardGraphics, LizardModule, LizardReading, OverseerGraphics, PlayerGraphics,
    RainbowSprites, ShelterDoorSprites, ShortcutPalette, SlimeMoldSprites, SpeckSprites,
    SpotLightSprites, StalkSprites, TokenStalkSprites, WaterDripSprites, WeaponSprites,
};
use rw_hud::{HudSetup, OwnerTick, RainCycleView, StartCondition, Subregions};
use rw_random::UnityRandom;
use rw_render::{BodyStatus, ChunkInterpolation, Follow, PlayerLight, SurfaceLight, WaterSurface};
use rw_sim::campaign::{self, SlugcatCampaign, StoryStart};
use rw_sim::creature::fly::FlyView;
use rw_sim::creature::lizard::{LizardGraphicsLink, LizardGraphicsTick, LizardView};
use rw_sim::creature::overseer::hologram::OverseerHologram;
use rw_sim::creature::overseer::{OverseerBody, OverseerView};
use rw_sim::creature::{CreatureId, EntityId};
use rw_sim::item::Entity;
use rw_sim::room_script::{RoomScript, SuC04StartUp};
use rw_sim::shelter::ShelterDoor;
use rw_sim::world::World;
use rw_sim::{
    CameraMove, CameraView, CosmeticKind, HudEvent, Input, MyceliaCx, OwnerKey, PhysicalObject,
    Player, PlayerHands, PlayerObserver, PlayerSurroundings, RainWorldGame, SoundEvent, Sounds,
    Start, SystemId, Vec2, Water,
};

use crate::jaws::{JawHolder, Jaws};
use crate::shortcuts::CameraShortcuts;
use crate::sprite_order::{Drawable, Joined, Move, Overlap, Present, SpriteOrder};

/// How a game starts.
#[derive(Debug, Clone, PartialEq)]
pub enum GameStart {
    /// `StoryGameInitCondition.New` for a campaign: its start den, at the den's start
    /// position (`TryGetPlayerStartPos`, else (15, 25)).
    NewStory(SlugcatName),
    /// `StoryGameInitCondition.Load`: a saved game of `slugcat`, in `story.den`, on the
    /// shelter's spawn tile (`ShelterDoor.playerSpawnPos`) or the den's start position.
    SavedStory {
        slugcat: SlugcatName,
        story: Box<StoryStart>,
    },
    /// A given room, in its region's world (a single-room world for arena levels), as
    /// `slugcat`: on `tile`, or on a safe spawn tile (the dev start, `--spawn`).
    Room {
        name: String,
        tile: Option<(i32, i32)>,
        slugcat: SlugcatName,
    },
}

/// What a body in the debug view is, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugBody {
    Lizard,
    Fly,
    Overseer,
    Item,
}

impl DebugBody {
    /// The fill colour.
    #[must_use]
    pub fn color(self) -> rw_render::Color {
        match self {
            Self::Lizard => rw_render::Color::rgb(0.3, 1.0, 0.35),
            Self::Fly => rw_render::Color::rgb(0.3, 0.6, 1.0),
            Self::Overseer => rw_render::Color::rgb(1.0, 0.85, 0.3),
            Self::Item => rw_render::Color::rgb(0.8, 0.8, 0.8),
        }
    }
}

/// The game and the player's graphics.
pub struct Game {
    sim: RainWorldGame,
    graphics: Graphics,
    /// The last tick took the player into a shortcut.
    entered_shortcut: bool,
    /// `menuSetup.startGameCondition`: `New` for a new story, `Dev` for a room start.
    start_condition: StartCondition,
    /// The region's subregions, for the HUD; `None` in a world without a region.
    subregions: Option<Subregions>,
    shortcuts: CameraShortcuts,
    sprite_order: SpriteOrder,
}

/// The player's graphics module, attached where the game attaches it, and the
/// overseers' in the camera's room.
struct Graphics {
    module: Option<PlayerGraphics>,
    /// The story character, for the colours.
    slugcat: SlugcatName,
    overseers: Vec<OverseerGraphics>,
    /// `hologram.color` of each overseer with a graphics module, as of its last update
    /// (its holograms update before it in a tick, so this is what they read).
    hologram_bases: Vec<(EntityId, Color)>,
    /// Every live hologram's sprites and colours, viewed or not.
    holograms: Vec<HologramGraphics>,
    /// The flies' graphics modules, in sprite order (`MoveToFront` moves one last).
    flies: Vec<FlyGraphics>,
    /// Flies whose sprites went on top of their container this tick (`Fly.Grabbed`,
    /// `BringSpritesToFront`), in order.
    flies_to_front: Vec<EntityId>,
    /// The lizards' graphics modules, in the order they were built.
    lizards: Vec<LizardGraphics>,
    /// Dropped lizard modules their bubbles, sparks and flashes still refer to.
    dropped_lizards: Vec<LizardGraphics>,
    /// The last module serial given out.
    lizard_serial: u64,
    /// The camera room palette's earth colour (`OverseerGraphics.ApplyPalette`).
    earth: Color,
    /// The lens's and holograms' per-frame draws (the game's come from the global
    /// stream: D1).
    cosmetic: UnityRandom,
    /// Each corruption spore's `Pebble`, drawn on the cosmetic stream when its sprite is
    /// made (by its place in the update list).
    spore_pebbles: HashMap<u64, usize>,
    /// The camera room's placed `LightBeam`s' drawing state (`lastAlpha`, `lastCamPos`,
    /// ...) by their place in the update list, and the room it was made for.
    beams: Vec<(u64, LightBeamSprites)>,
    beams_room: i32,
}

impl Graphics {
    /// Moves the lizard modules `dropped` picks out of the live ones; their bubbles,
    /// sparks and flashes keep them, as the game's keep their `LizardGraphics`.
    fn drop_lizards(&mut self, dropped: impl Fn(&LizardGraphics) -> bool) {
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.lizards)
            .into_iter()
            .partition(|g| dropped(g));
        self.lizards = kept;
        self.dropped_lizards.extend(gone);
    }

    /// Forgets the dropped lizard modules no cosmetic in `world` refers to.
    fn forget_dropped_lizards(&mut self, world: &World) {
        if self.dropped_lizards.is_empty() {
            return;
        }
        let used: Vec<u64> = world
            .active_rooms()
            .into_iter()
            .flat_map(|r| world.room_cosmetics(r))
            .filter_map(|c| match &c.kind {
                CosmeticKind::LizardBubble(b) => Some(b.module),
                CosmeticKind::Spark(s) => s.lizard.map(|(_, m)| m),
                CosmeticKind::Flash(f) => f.lizard.map(|(_, m)| m),
                CosmeticKind::Spikes(_)
                | CosmeticKind::OverseerEffect(_)
                | CosmeticKind::Bubble(_)
                | CosmeticKind::Splash(_)
                | CosmeticKind::SparkFlash(_)
                | CosmeticKind::NeuronSpark(_) => None,
            })
            .collect();
        self.dropped_lizards.retain(|g| used.contains(&g.module));
    }

    /// Overseer `id`'s graphics module and its conversation partner's.
    fn overseer_pair(
        &mut self,
        id: EntityId,
        partner: Option<EntityId>,
    ) -> Option<(&mut OverseerGraphics, Option<&OverseerGraphics>)> {
        let i = self.overseers.iter().position(|g| g.id == id)?;
        let j = partner.and_then(|p| self.overseers.iter().position(|g| g.id == p));
        match j {
            Some(j) if j != i => {
                let (lo, hi) = self.overseers.split_at_mut(i.max(j));
                let (a, b) = if i < j {
                    (&mut lo[i], &mut hi[0])
                } else {
                    (&mut hi[0], &mut lo[j])
                };
                Some((a, Some(&*b)))
            }
            _ => Some((&mut self.overseers[i], None)),
        }
    }

    /// Records overseer `view`'s `hologram.color`.
    fn set_hologram_base(&mut self, view: &OverseerView<'_>) {
        let color = OverseerGraphics::hologram_color(view, self.earth);
        match self
            .hologram_bases
            .iter_mut()
            .find(|(id, _)| *id == view.id)
        {
            Some(entry) => entry.1 = color,
            None => self.hologram_bases.push((view.id, color)),
        }
    }
}

impl PlayerObserver for Graphics {
    fn created(&mut self, player: &Player, rng: &mut UnityRandom) {
        self.module = Some(PlayerGraphics::new(player, self.slugcat, rng));
    }

    fn updated(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    ) {
        if let Some(g) = &mut self.module {
            g.update(player, room, water, around, rng);
        }
    }

    fn spat_out(&mut self, player: &Player, rng: &mut UnityRandom) {
        if let Some(g) = &mut self.module {
            g.spat_out_of_shortcut(player, rng);
        }
    }

    fn spat_out_finished(&mut self, player: &Player, rng: &mut UnityRandom) {
        if let Some(g) = &mut self.module {
            g.reset(player, rng);
        }
    }

    fn has_player_graphics(&self) -> bool {
        self.module.is_some()
    }

    fn player_look_at(&mut self, target: Option<Entity>) {
        if let Some(g) = &mut self.module {
            g.look_at(target);
        }
    }

    fn player_bite_fly(&mut self, hand: usize, fly: CreatureId) -> Option<Vec2> {
        self.module.as_mut().map(|g| g.bite_fly(hand, fly))
    }

    fn player_hands(&self) -> Option<PlayerHands> {
        self.module.as_ref().map(PlayerGraphics::hand_motion)
    }

    fn player_swallowing(&mut self) {
        if let Some(g) = &mut self.module {
            g.swallow();
        }
    }

    fn player_throw_object(&mut self, hand: usize, obj: Entity) {
        if let Some(g) = &mut self.module {
            g.throw_object(hand, obj);
        }
    }

    fn nudge_player_head(&mut self, dv: Vec2) {
        if let Some(g) = &mut self.module {
            g.nudge_head(dv);
        }
    }

    fn overseer_added(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        rng: &mut UnityRandom,
    ) -> bool {
        let built = !self.overseers.iter().any(|g| g.id == view.id);
        if built {
            self.overseers.push(OverseerGraphics::new(view, mx, rng));
        }
        self.set_hologram_base(view);
        built
    }

    fn overseer_reset(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        rng: &mut UnityRandom,
    ) {
        if let Some(g) = self.overseers.iter_mut().find(|g| g.id == view.id) {
            g.reset(view.body, mx.pool, rng);
        }
    }

    fn overseer_emerge(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        rng: &mut UnityRandom,
    ) {
        if let Some(g) = self.overseers.iter_mut().find(|g| g.id == view.id) {
            g.emerge(view.body, mx.pool, rng);
        }
    }

    fn overseer_updated(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        rng: &mut UnityRandom,
    ) {
        if let Some((g, partner)) = self.overseer_pair(view.id, view.partner) {
            g.update(view, mx, partner, rng);
            self.set_hologram_base(view);
        }
    }

    fn overseer_new_room(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        new: Option<SystemId>,
    ) {
        if let Some(g) = self.overseers.iter().find(|g| g.id == view.id) {
            g.new_room(mx.pool, view.neuron_system, new);
        }
    }

    fn overseer_eye(&self, id: EntityId, body: &OverseerBody) -> Option<Vec2> {
        self.overseers
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.eye(body, 1.0))
    }

    fn overseer_removed(&mut self, id: EntityId) -> Option<OwnerKey> {
        let owner = self.overseers.iter().find(|g| g.id == id).map(|g| g.owner);
        self.overseers.retain(|g| g.id != id);
        self.hologram_bases.retain(|(o, _)| *o != id);
        owner
    }

    /// The parts' colour lerps; `hologram.color` is white without a graphics module.
    fn hologram_updated(&mut self, owner: EntityId, hologram: &OverseerHologram) {
        let base = self
            .hologram_bases
            .iter()
            .find(|(id, _)| *id == owner)
            .map_or(Color::WHITE, |&(_, c)| c);
        let i = match self.holograms.iter().position(|g| g.id == hologram.id) {
            Some(i) => i,
            None => {
                self.holograms.push(HologramGraphics::new(hologram));
                self.holograms.len() - 1
            }
        };
        self.holograms[i].update(hologram, base);
    }

    /// The sim builds a module only for a fly without one, so one kept here belongs to an
    /// earlier body of the creature (abstracted since) and is replaced.
    fn fly_added(&mut self, view: &FlyView<'_>, rng: &mut UnityRandom) {
        self.flies.retain(|g| g.id != view.id);
        self.flies.push(FlyGraphics::new(view, rng));
    }

    fn fly_reset(&mut self, view: &FlyView<'_>) {
        if let Some(g) = self.flies.iter_mut().find(|g| g.id == view.id) {
            g.reset(view);
        }
    }

    fn fly_updated(
        &mut self,
        view: &FlyView<'_>,
        room: &Room,
        cameras: usize,
        camera_room: &Room,
        camera: &CameraView,
        rng: &mut UnityRandom,
    ) {
        if let Some(g) = self.flies.iter_mut().find(|g| g.id == view.id) {
            g.update(view, room, cameras, camera_room, camera, rng);
        }
    }

    fn fly_to_front(&mut self, id: EntityId) {
        if let Some(i) = self.flies.iter().position(|g| g.id == id) {
            let g = self.flies.remove(i);
            self.flies.push(g);
            self.flies_to_front.push(id);
        }
    }

    fn fly_removed(&mut self, id: EntityId) {
        self.flies.retain(|g| g.id != id);
    }

    fn builds_lizard_graphics(&self) -> bool {
        true
    }

    /// A link without parts is a new module (one kept here belongs to an earlier body
    /// of the creature and is replaced); with parts, the module only resets.
    fn lizard_added(
        &mut self,
        view: &LizardView<'_>,
        link: &mut LizardGraphicsLink,
        rng: &mut UnityRandom,
    ) {
        let i = match self.lizards.iter().position(|g| g.id == view.id) {
            Some(i) if !link.parts.is_empty() => i,
            found => {
                if let Some(i) = found {
                    let old = self.lizards.remove(i);
                    self.dropped_lizards.push(old);
                }
                self.lizard_serial += 1;
                link.module = self.lizard_serial;
                let mut g = LizardGraphics::new(view);
                g.module = self.lizard_serial;
                self.lizards.push(g);
                self.lizards.len() - 1
            }
        };
        let g = &mut self.lizards[i];
        g.reset(view, rng);
        g.write_link(link);
    }

    fn lizard_reset(
        &mut self,
        view: &LizardView<'_>,
        link: &mut LizardGraphicsLink,
        rng: &mut UnityRandom,
    ) {
        if let Some(g) = self.lizards.iter_mut().find(|g| g.id == view.id) {
            g.reset(view, rng);
            g.write_link(link);
        }
    }

    fn lizard_updated(
        &mut self,
        view: &LizardView<'_>,
        link: &mut LizardGraphicsLink,
        cx: &mut LizardGraphicsTick<'_>,
    ) {
        if let Some(g) = self.lizards.iter_mut().find(|g| g.id == view.id) {
            g.update(view, link, cx);
        }
    }

    fn lizard_sucked_into_shortcut(
        &mut self,
        id: EntityId,
        link: &mut LizardGraphicsLink,
        pos: Vec2,
    ) {
        if let Some(g) = self.lizards.iter_mut().find(|g| g.id == id) {
            g.sucked_into_shortcut(pos);
            g.write_link(link);
        }
    }

    fn lizard_removed(&mut self, id: EntityId) {
        self.drop_lizards(|g| g.id == id);
    }

    fn nudge_tail_x(&mut self, dvx: f32) {
        if let Some(g) = &mut self.module {
            for segment in &mut g.tail {
                segment.part.vel.x += dvx;
            }
        }
    }
}

impl Game {
    /// A single-room world of `room` (in memory), the player on `spawn_tile` or a safe
    /// spawn tile, drawing from `UnityRandom::new(seed)` (`Random.InitState`, low 32
    /// bits), stepped until the room is ready as dev starts are (three ticks).
    #[cfg(test)]
    #[must_use]
    pub fn new(
        room: Room,
        settings: &RoomSettings,
        seed: u64,
        spawn_tile: Option<(i32, i32)>,
    ) -> Self {
        let data = WorldData::from_room(&room);
        let rooms = Arc::new(rw_assets::world::FixedRoom(rw_assets::world::LoadedRoom {
            room,
            settings: settings.clone(),
        }));
        let campaign = &SlugcatCampaign::SURVIVOR;
        let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
        let start = Start {
            room: 0,
            tile: spawn_tile,
        };
        let mut game = Self::build(
            world,
            campaign,
            StoryStart::new_game(campaign),
            start,
            UnityRandom::new(seed as i32),
            Sounds::default(),
        );
        while !game.sim.camera_room_ready() {
            game.tick(Input::NONE);
        }
        game
    }

    /// The game `start` asks for, read from the installed game, drawing from `rng` (the
    /// stream the menus drew from before, D1) and playing sounds from `table`, with
    /// `screen_size` the chosen resolution (`options.ScreenSize`) the rooms read as they load.
    ///
    /// # Errors
    /// When the world or the first room cannot be loaded.
    pub fn start(
        assets: &StreamingAssets,
        start: &GameStart,
        rng: UnityRandom,
        table: Arc<SoundTable>,
        screen_size: [f32; 2],
    ) -> Result<Self, String> {
        let mods = ActiveMods::NONE;
        let (campaign, room, tile) = match start {
            GameStart::NewStory(name) => {
                let campaign = SlugcatCampaign::of(*name, mods)
                    .ok_or_else(|| format!("no campaign for {name}"))?;
                let den = campaign.start_den;
                let tile =
                    campaign::player_start_pos(den, mods).unwrap_or(campaign::DEFAULT_PLAYER_TILE);
                (campaign, den.to_owned(), Some(tile))
            }
            GameStart::SavedStory { slugcat, story } => {
                let campaign = SlugcatCampaign::of(*slugcat, mods)
                    .ok_or_else(|| format!("no campaign for {slugcat}"))?;
                (campaign, story.den.clone(), None)
            }
            GameStart::Room {
                name,
                tile,
                slugcat,
            } => {
                let campaign = SlugcatCampaign::of(*slugcat, mods)
                    .ok_or_else(|| format!("no campaign for {slugcat}"))?;
                (campaign, name.clone(), *tile)
            }
        };
        let filter = LineFilter {
            timeline: Some(campaign.timeline),
            mods,
            conditions: StoryConditions::new_story(),
        };
        let regions = Regions::load(assets).map_err(|e| format!("regions: {e}"))?;
        let request = WorldRequest::for_first_room(assets, &room, filter.clone());
        let region_world = WorldData::load(assets, &regions, &request)
            .map_err(|e| format!("world {}: {e}", request.name))
            .and_then(|data| match data.room_named(&room) {
                Some(r) => Ok((r.index, data)),
                None => Err(format!("{room} is not in world {}", data.name)),
            });
        // A room outside its region's world (unused rooms, `--room`): a single-room
        // world of it, as arena levels get.
        let (first, data) = match region_world {
            Ok(found) => found,
            Err(e) if !request.single_room => {
                log::warn!("{e}; loading {room} as a single-room world");
                let single = WorldRequest {
                    name: room.clone(),
                    single_room: true,
                    filter,
                };
                let data = WorldData::load(assets, &regions, &single)
                    .map_err(|e| format!("room {room}: {e}"))?;
                (data.first_room_index(), data)
            }
            Err(e) => return Err(e),
        };
        log::info!(
            "world {} ({} rooms), first room {room}, as {}",
            data.name,
            data.rooms.len(),
            campaign.name
        );
        let region = match &data.region {
            Some(r) => Some(r.name.to_lowercase()),
            None => assets.region_of_room(&room.to_lowercase()),
        };
        let rooms = Arc::new(GameRooms {
            assets: assets.clone(),
            region,
            timeline: campaign.timeline,
        });
        let subregions = data
            .region
            .as_ref()
            .map(|r| Subregions::load(assets, &r.name, campaign.timeline.value()));
        let (tile, story) = match start {
            GameStart::SavedStory { story, .. } => (
                Some(Self::den_tile(&data, &room, rooms.as_ref(), mods)),
                (**story).clone(),
            ),
            _ => (tile, StoryStart::new_game(campaign)),
        };
        let mut world = World::new(data, rooms, campaign.timeline, mods, true);
        // `StoryGameSession`'s constructor: the saved communities, else the defaults.
        let region_names: Vec<String> = regions.0.iter().map(|r| r.name.clone()).collect();
        let alignments =
            std::fs::read_to_string(assets.resolve_file_path("World/default alignments.txt"))
                .unwrap_or_default();
        world.communities = rw_save::CreatureCommunities::for_session(
            story.communities.as_deref(),
            &region_names,
            &alignments,
            campaign.name,
        );
        world.session_difficulty = story.difficulty;
        world.set_screen_size(rw_sim::Vec2::from(screen_size));
        let start_at = Start { room: first, tile };
        let mut game = Self::build(world, campaign, story, start_at, rng, Sounds::new(table));
        game.subregions = subregions;
        game.start_condition = match start {
            GameStart::NewStory(_) => StartCondition::New,
            GameStart::SavedStory { .. } => StartCondition::Load,
            GameStart::Room { .. } => StartCondition::Dev,
        };
        Ok(game)
    }

    /// Where a saved game's player starts in den `room`: a shelter's spawn tile
    /// (`RealizeInRoom` in a shelter), else `TryGetPlayerStartPos` or (15, 25).
    fn den_tile(
        data: &WorldData,
        room: &str,
        rooms: &dyn rw_assets::world::RoomSource,
        mods: ActiveMods,
    ) -> (i32, i32) {
        let fallback =
            campaign::player_start_pos(room, mods).unwrap_or(campaign::DEFAULT_PLAYER_TILE);
        let Some(abstract_room) = data.room_named(room).filter(|r| r.shelter()) else {
            return fallback;
        };
        let file = abstract_room
            .alt_file_name
            .clone()
            .unwrap_or_else(|| abstract_room.name.clone());
        match rooms.load_room(&file) {
            Ok(loaded) => ShelterDoor::player_spawn_pos(
                &loaded.room,
                &loaded.settings,
                abstract_room.ancient_shelter,
                mods,
            ),
            Err(e) => {
                log::warn!("shelter {room}: {e}");
                fallback
            }
        }
    }

    fn build(
        world: World,
        campaign: &'static SlugcatCampaign,
        story: StoryStart,
        start: Start,
        rng: UnityRandom,
        sounds: Sounds,
    ) -> Self {
        let mut graphics = Graphics {
            module: None,
            slugcat: campaign.name,
            overseers: Vec::new(),
            hologram_bases: Vec::new(),
            holograms: Vec::new(),
            flies: Vec::new(),
            flies_to_front: Vec::new(),
            lizards: Vec::new(),
            dropped_lizards: Vec::new(),
            lizard_serial: 0,
            earth: Color::BLACK,
            cosmetic: UnityRandom::new(0),
            spore_pebbles: HashMap::new(),
            beams: Vec::new(),
            beams_room: -1,
        };
        let sim =
            RainWorldGame::new_story(world, campaign, story, start, rng, sounds, &mut graphics);
        log::info!("spawned player");
        Self {
            sim,
            graphics,
            start_condition: StartCondition::Dev,
            subregions: None,
            shortcuts: CameraShortcuts::default(),
            sprite_order: SpriteOrder::default(),
            entered_shortcut: false,
        }
    }

    /// The simulation.
    #[must_use]
    pub fn sim(&self) -> &RainWorldGame {
        &self.sim
    }

    pub fn sim_mut(&mut self) -> &mut RainWorldGame {
        &mut self.sim
    }

    /// `RoomCamera.Update`'s condition for firing up the HUD: the followed player is
    /// realized in a world that is not a single room.
    /// The region's subregions (`world.region.subRegions`) and its rooms' subregion
    /// names; `None` without a region.
    #[must_use]
    pub fn subregions(&self) -> Option<&Subregions> {
        self.subregions.as_ref()
    }

    #[must_use]
    pub fn hud_due(&self) -> bool {
        self.sim.player().is_some() && !self.sim.world().single_room()
    }

    /// What `InitSinglePlayerHud` and `FireUpSinglePlayerHUD` read: the slugcat's food
    /// meter (its `slugcatStats`, malnourished in a malnourished cycle), the save state's
    /// karma and cycle, and the world's rain cycle.
    #[must_use]
    pub fn hud_setup(&self, screen_size: [f32; 2]) -> HudSetup {
        let stats = self.sim.character_stats();
        let world = self.sim.world();
        HudSetup {
            screen_size,
            max_food: stats.max_food,
            food_to_hibernate: stats.food_to_hibernate,
            current_food: self.sim.food_in_stomach().cast_signed(),
            karma: self.sim.story().karma,
            karma_cap: self.sim.story().karma_cap,
            karma_reinforced: self.sim.story().karma_reinforced,
            cycle_number: self.sim.story().cycle_number,
            camera_room_shelter: world
                .abstract_room(self.sim.camera_room())
                .is_some_and(AbstractRoomData::shelter),
            rain: self.rain_view(),
            subregions: self.subregions.clone(),
            start: self.start_condition,
            map: None,
            cycle_label: None,
        }
    }

    /// What the HUD reads of the player this tick. The rain cycle's danger is not
    /// ported.
    #[must_use]
    pub fn hud_owner(&self) -> OwnerTick {
        let Some(player) = self.sim.player() else {
            return OwnerTick::default();
        };
        let input = player.input_history()[0];
        let room = (!player.in_shortcut())
            .then(|| self.sim.world().abstract_room(self.sim.player_pos().room))
            .flatten();
        let shelter = room.is_some_and(AbstractRoomData::shelter);
        OwnerTick {
            reveal_map: input.map,
            current_food: self.sim.food_in_stomach().cast_signed(),
            quarter_food_points: self.sim.quarter_food_points(),
            room: room.map(|r| r.name.clone()),
            // Broken shelters are not read: every shelter is intact.
            in_intact_shelter: shelter,
            force_sleep_pose: shelter && self.in_force_sleep_pose(player),
            still_in_start_shelter: player.still_in_start_shelter(),
            ready_for_win: player.ready_for_win(),
            force_sleep_counter: player.force_sleep_counter(),
            map: self.map_owner(),
            alive: !player.is_dead(),
            held_by: self
                .sim
                .player_grabbed_by()
                .iter()
                .map(|g| g.serial)
                .collect(),
            ..OwnerTick::default()
        }
    }

    /// The input and tiles `FoodMeter.GameUpdate` checks for lowering `forceSleep` by
    /// 1/60 per tick (holding down on solid ground with nothing else pressed).
    fn in_force_sleep_pose(&self, player: &Player) -> bool {
        let input = player.input_history()[0];
        let room = self.room();
        input.y < 0
            && !input.jump
            && player.is_tile_solid(room, 1, 0, -1)
            && (input.x == 0
                || ((!player.is_tile_solid(room, 1, -1, -1)
                    || !player.is_tile_solid(room, 1, 1, -1))
                    && player.is_tile_solid(room, 1, i32::from(input.x), 0)))
    }

    /// A story session (a new or continued campaign), as opposed to the port's dev
    /// starts in a room (`load_room`, `--room`, arena levels), which have no game over
    /// and keep R.
    #[must_use]
    pub fn story_session(&self) -> bool {
        self.start_condition != StartCondition::Dev
    }

    /// Hands the random stream back (to the menus, when the game ends).
    pub fn take_rng(&mut self) -> UnityRandom {
        self.sim.take_rng()
    }

    /// `manager.FadeDelayInProgress` as the room scripts will read it.
    pub fn set_fade_delay_in_progress(&mut self, in_progress: bool) {
        self.sim.set_fade_delay_in_progress(in_progress);
    }

    /// The camera room's start-up script, if it has one.
    #[must_use]
    pub fn start_up_script(&self) -> Option<&SuC04StartUp> {
        self.camera_room().scripts.iter().find_map(|s| match s {
            RoomScript::SuC04StartUp(s) => Some(s),
            RoomScript::SsE08GradientGravity(_) => None,
        })
    }

    /// `sounds.txt` resolved against the `loadedsoundeffects` bundle, or an empty table
    /// (nothing plays, no sound draws) when they cannot be read.
    #[must_use]
    pub fn load_sound_table(assets: &StreamingAssets) -> Arc<SoundTable> {
        AssetBundle::open(&assets.asset_bundle("loadedsoundeffects"))
            .and_then(|bundle| SoundTable::load(assets, &bundle))
            .inspect_err(|e| log::warn!("no sound table ({e}); sounds are off"))
            .map(Arc::new)
            .unwrap_or_default()
    }

    /// The sound events since the last call.
    pub fn take_sound_events(&mut self) -> Vec<SoundEvent> {
        self.sim.sounds_mut().take_events()
    }

    /// The camera's room's settings.
    #[must_use]
    pub fn settings(&self) -> &RoomSettings {
        &self.camera_room().settings
    }

    fn camera_room(&self) -> &rw_sim::world::RealizedRoom {
        self.sim
            .world()
            .realized(self.sim.camera_room())
            .expect("the camera's room stays realized")
    }

    /// The stream cosmetic draws (holograms, drips) use.
    #[must_use]
    pub fn cosmetic_rng(&self) -> &UnityRandom {
        &self.graphics.cosmetic
    }

    pub fn cosmetic_rng_mut(&mut self) -> &mut UnityRandom {
        &mut self.graphics.cosmetic
    }

    pub fn set_cosmetic_rng(&mut self, rng: UnityRandom) {
        self.graphics.cosmetic = rng;
    }

    /// The camera move since the last call.
    pub fn take_camera_move(&mut self) -> Option<CameraMove> {
        self.sim.take_camera_move()
    }

    /// Replaces the player with a fresh one in the camera's room (continuing the random
    /// sequence); a development key, so not in story sessions. Returns whether it did.
    pub fn respawn(&mut self) -> bool {
        if self.story_session() {
            log::info!("no respawn in a story session");
            return false;
        }
        self.sim.respawn(&mut self.graphics);
        true
    }

    /// One 40 Hz simulation tick ([`RainWorldGame::tick`]).
    pub fn tick(&mut self, input: Input) {
        let was_in_shortcut = self.sim.player().is_some_and(Player::in_shortcut);
        self.shortcuts.update();
        self.sim.tick(input, &mut self.graphics);
        self.graphics.forget_dropped_lizards(self.sim.world());
        self.shortcuts.sync(&mut self.sim, self.graphics.slugcat);
        self.sync_sprite_order();
        let in_shortcut = self.sim.player().is_some_and(Player::in_shortcut);
        self.entered_shortcut = in_shortcut && !was_in_shortcut;
        if in_shortcut != was_in_shortcut {
            log::info!(
                "player {} a shortcut",
                if was_in_shortcut { "left" } else { "entered" }
            );
        }
    }

    /// The player while it is in the camera's room (body or vessel).
    fn shown_player(&self) -> Option<&Player> {
        self.sim
            .player()
            .filter(|_| self.sim.player_in_camera_room())
    }

    /// The slugcat's sprites at `alpha` between ticks, eyes in the room palette's
    /// `blackColor` (tinted toward its `fogColor` while malnourished); `None` once the
    /// player is destroyed or elsewhere.
    pub fn draw_list(
        &mut self,
        alpha: f32,
        black_color: Color,
        fog_color: Color,
    ) -> Option<DrawList> {
        let player = self
            .sim
            .player()
            .filter(|_| self.sim.player_in_camera_room())?;
        Some(
            self.graphics
                .module
                .as_mut()?
                .draw(player, alpha, black_color, fog_color),
        )
    }

    /// The overseers' sprites in the camera's room at `alpha` between ticks, oldest
    /// first (the order their sprites joined the containers), in the room palette's
    /// earth colour.
    pub fn overseer_draw_lists(&mut self, alpha: f32, earth: Color) -> Vec<(Drawable, DrawList)> {
        let world = self.sim.world();
        let pool = world.mycelia();
        let mut shown: Vec<(u64, CreatureId)> = world
            .room_creatures(self.sim.camera_room())
            .iter()
            .filter_map(|&c| {
                let body = world.creature(c).overseer_body()?;
                body.in_room.then_some((body.update_order, c))
            })
            .collect();
        shown.sort_by_key(|&(seq, _)| seq);
        let mut lists = Vec::new();
        for (_, c) in shown {
            let Some(view) = world.overseer_view(c) else {
                continue;
            };
            let graphics = &mut self.graphics;
            if let Some(g) = graphics.overseers.iter_mut().find(|g| g.id == view.id) {
                let list = g.draw(&view, earth, alpha, pool, &mut graphics.cosmetic);
                lists.push((Drawable::Entity(Entity::Creature(c)), list));
            }
        }
        lists
    }

    /// The shelter door's sprites in the camera's room at `alpha` between ticks.
    #[must_use]
    pub fn shelter_draw_list(&self, alpha: f32) -> Option<DrawList> {
        self.sim
            .shelter_door(self.sim.camera_room())
            .map(|d| d.draw(alpha))
    }

    /// The rocks' and spears' sprites in the camera's room at `alpha` between ticks, in
    /// the order they joined the room (`DrawSprites`, its vibrate and blink draws on the
    /// cosmetic stream), in the palette's black.
    pub fn weapon_draw_lists(&mut self, alpha: f32, black: Color) -> Vec<(Entity, DrawList)> {
        let world = self.sim.world();
        let room = self.sim.camera_room();
        let water = world.realized(room).and_then(|r| r.water.as_ref());
        let mut shown = world.room_weapons(room);
        shown.sort_unstable();
        let cosmetic = &mut self.graphics.cosmetic;
        shown
            .iter()
            .filter_map(|&(_, id)| {
                let o = world.object(id);
                o.realized
                    .as_deref()?
                    .draw(o.id, room, alpha, black, water, cosmetic)
                    .map(|l| (Entity::Object(id), l))
            })
            .collect()
    }

    /// The sparks, spikes and lizard bubbles and flashes in the camera's room at `alpha`
    /// between ticks, oldest first, in the palette's `black` (the flashes draw on the
    /// cosmetic stream).
    pub fn cosmetic_draw_lists(&mut self, alpha: f32, black: Color) -> Vec<(Drawable, DrawList)> {
        let world = self.sim.world();
        let Some(room) = world.realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let mut shown: Vec<_> = room.cosmetics.iter().filter(|c| !c.slated).collect();
        shown.sort_by_key(|c| c.update_order);
        let heads = LizardHeads {
            graphics: &self.graphics.lizards,
            dropped: &self.graphics.dropped_lizards,
            world,
            black,
        };
        let rng = &mut self.graphics.cosmetic;
        shown
            .iter()
            .map(|c| {
                let list = c.draw(&room.room, alpha, &heads, rng);
                (Drawable::Cosmetic(c.id), list)
            })
            .collect()
    }

    /// The cosmetic insects' sprites in the camera's room at `alpha` between ticks, in
    /// the camera's `palette` (`ApplyPalette`) seen from `cam_pos` on a `screen`-sized
    /// view (their wings' jitter draws on the cosmetic stream).
    pub fn insect_draw_lists(
        &mut self,
        alpha: f32,
        palette: &PaletteTexture,
        cam_pos: rw_sim::Vec2,
        screen: rw_sim::Vec2,
    ) -> Vec<(Drawable, DrawList)> {
        let world = self.sim.world();
        let Some(coord) = world
            .realized(self.sim.camera_room())
            .and_then(|r| r.insects.as_ref())
        else {
            return Vec::new();
        };
        let room_palette = palette.room_palette();
        let mut cx = InsectDraw {
            palette,
            darkness: room_palette.darkness,
            fog_amount: room_palette.fog_amount,
            cam_pos,
            screen,
            rng: &mut self.graphics.cosmetic,
        };
        coord
            .insects
            .iter()
            .filter(|i| !i.base.slated)
            .map(|i| (Drawable::Insect(i.update_order), i.draw(alpha, &mut cx)))
            .collect()
    }

    /// The camera room's cosmetic slime molds and dead token stalks at `alpha` between
    /// ticks in the camera's `palette` (the stalks' lamps draw on the cosmetic stream).
    pub fn placed_cosmetic_draw_lists(
        &mut self,
        alpha: f32,
        palette: &rw_assets::RoomPalette,
    ) -> Vec<(Drawable, DrawList)> {
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let mut lists: Vec<(Drawable, DrawList)> = r
            .slime_molds
            .iter()
            .map(|m| (Drawable::SlimeMold(m.update_order), m.draw(palette)))
            .collect();
        let black = Color::from(palette.black_color);
        let rng = &mut self.graphics.cosmetic;
        lists.extend(r.token_stalks.iter().map(|s| {
            (
                Drawable::TokenStalk(s.update_order),
                s.draw(black, alpha, rng),
            )
        }));
        lists
    }

    /// The camera room's `HolyFireSprite`s, `SlimeMoldLight`s and `WaterFall`s at `alpha`
    /// between ticks in the camera's `palette`.
    #[must_use]
    pub fn fixture_draw_lists(
        &self,
        alpha: f32,
        palette: &rw_assets::RoomPalette,
    ) -> Vec<(Drawable, DrawList)> {
        use rw_graphics::{HolyFireSpriteDraw, SlimeMoldLightSprites, WaterFallSprites};
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let f = &r.fixtures;
        let mut lists: Vec<(Drawable, DrawList)> = f
            .holy_sprites
            .iter()
            .filter(|s| !s.slated)
            .map(|s| (Drawable::Fixture(s.update_order), s.draw(alpha)))
            .collect();
        lists.extend(
            f.slime_lights
                .iter()
                .map(|s| (Drawable::Fixture(s.update_order), s.draw(palette))),
        );
        lists.extend(f.waterfalls.iter().map(|w| {
            (
                Drawable::Fixture(w.update_order),
                w.draw(r.water.as_ref(), palette, alpha),
            )
        }));
        lists
    }

    /// The camera room's `ElectricDeath`s at `alpha` between ticks and the frame's camera
    /// position `cam_pos` (their two draws a light on the cosmetic stream).
    pub fn electric_death_draw_lists(
        &mut self,
        alpha: f32,
        cam_pos: rw_sim::Vec2,
    ) -> Vec<(Drawable, DrawList)> {
        use rw_graphics::ElectricDeathSprites;
        let world = self.sim.world();
        let screen = world.screen_size();
        let Some(r) = world.realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let rng = &mut self.graphics.cosmetic;
        r.electric_deaths
            .iter()
            .map(|d| {
                let intensity = d.intensity(&world.rain_cycle);
                (
                    Drawable::ElectricDeath(d.update_order),
                    d.draw(intensity, cam_pos, screen, alpha, rng),
                )
            })
            .collect()
    }

    /// The camera room's `CoralNeuron`s, `StemSegment`s, `WallMycelia` and circuit bits,
    /// connectors and crabs at `alpha`
    /// between ticks (the segments' dots blink from the cosmetic stream, per frame).
    pub fn coral_draw_lists(
        &mut self,
        alpha: f32,
        cam_pos: rw_sim::Vec2,
    ) -> Vec<(Drawable, DrawList)> {
        use rw_graphics::{
            CircuitSprites, CoralNeuronSprites, StemFrame, StemSegmentSprites, WallMyceliaSprites,
        };
        let darkness = self.sim.darkness(self.sim.camera_room());
        let world = self.sim.world();
        let Some(r) = world.realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let Some(system) = r.coral.as_ref() else {
            return Vec::new();
        };
        let water_lights = r
            .settings
            .effect_amount(rw_assets::RoomEffectType::WaterLights)
            > 0.0;
        let mut lists: Vec<(Drawable, DrawList)> = system
            .neurons
            .iter()
            .map(|n| {
                (
                    Drawable::Coral(n.update_order),
                    n.draw(world.mycelia(), alpha, water_lights),
                )
            })
            .collect();
        let frame = StemFrame {
            t: alpha,
            darkness,
            water_lights,
            cam_pos,
        };
        let rng = &mut self.graphics.cosmetic;
        for stem in &system.stems {
            for (k, segment) in stem.segments.iter().enumerate() {
                lists.push((
                    Drawable::Coral(segment.update_order),
                    stem.draw_segment(k, world.mycelia(), &frame, rng),
                ));
            }
        }
        lists.extend(system.wall_mycelia.iter().map(|w| {
            (
                Drawable::Coral(w.update_order),
                w.draw(world.mycelia(), alpha),
            )
        }));
        for circuit in &system.circuits {
            lists.extend(circuit.bits.iter().enumerate().map(|(i, bit)| {
                (
                    Drawable::Coral(bit.update_order),
                    circuit.draw_bit(i, alpha),
                )
            }));
            for (k, connector) in circuit.connectors.iter().enumerate() {
                lists.push((
                    Drawable::Coral(connector.update_order),
                    circuit.draw_connector(k, world.mycelia(), alpha, rng),
                ));
            }
            lists.extend(circuit.crabs.iter().enumerate().map(|(k, crab)| {
                (
                    Drawable::Coral(crab.update_order),
                    circuit.draw_crab(k, alpha),
                )
            }));
        }
        lists
    }

    /// The camera room's superstructure projections at `alpha` between ticks: the
    /// projector's parts, the scan lines of `ProjectedScanLines`, the star matrices and the
    /// coral's `ProjectedCircle`s (after their `UpdateDepth`, which the draw runs).
    /// `cam_pos` is the frame's camera position, `camera_position` the room's current
    /// camera (`room.cameraPositions[currentCameraPosition]`).
    pub fn projection_draw_lists(
        &mut self,
        alpha: f32,
        cam_pos: rw_sim::Vec2,
        camera_position: rw_sim::Vec2,
    ) -> Vec<(Drawable, DrawList)> {
        use rw_graphics::{
            CircleSurroundings, ProjectedCircleSprites, ProjectedScanLinesSprites, ProjectionView,
            ProjectorSprites, StarMatrixSprites,
        };
        self.sim.update_circle_depths();
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let view = ProjectionView {
            cam_pos,
            camera_position,
        };
        let p = &r.projections;
        let mut lists = Vec::new();
        for projector in &p.projectors {
            for part in projector.parts.iter().filter(|part| !part.slated) {
                let list = projector.draw_part(part.update_order, alpha, &view);
                if !list.items.is_empty() {
                    lists.push((Drawable::Projection(part.update_order), list));
                }
            }
        }
        for lines in &p.scan_lines {
            for line in lines.lines.iter().filter(|l| !l.slated) {
                let list = lines.draw_line(line.update_order, alpha, cam_pos);
                if !list.items.is_empty() {
                    lists.push((Drawable::Projection(line.update_order), list));
                }
            }
        }
        for stars in &p.star_matrices {
            lists.push((Drawable::Projection(stars.update_order), stars.draw(alpha)));
        }
        let around = CircleSurroundings {
            neurons: r.coral.as_ref().map_or(&[], |c| c.neurons.as_slice()),
            stems: r.coral.as_ref().map_or(&[], |c| c.stems.as_slice()),
            circles: &p.circles,
            light_angle: rw_sim::Vec2::from(r.room.light_angle()),
        };
        let rng = &mut self.graphics.cosmetic;
        for circle in p.circles.iter().filter(|c| !c.slated) {
            lists.push((
                Drawable::Projection(circle.update_order),
                circle.draw(&around, alpha, rng),
            ));
        }
        lists
    }

    /// The `_gridOffset` the camera room's projector leaves set at `alpha`.
    #[must_use]
    pub fn projection_grid_offset(&self, alpha: f32) -> Option<[f32; 2]> {
        self.sim
            .projection_grid_offset(alpha)
            .map(rw_sim::Vec2::to_array)
    }

    /// The camera room's `SSLightRod`s, `ZapCoil`s, `ZapFlash`es and `SuperStructureFuses` at
    /// `alpha` between ticks; `camera` is `CamPos(currentCameraPosition)`, which the fuses'
    /// depth pushes from. A flash's seven cosmetic draws come first, and its draw sets its
    /// light (`HardSetRad`, `HardSetAlpha`, `color`), so this runs before the lights draw.
    pub fn ss_electric_draw_lists(
        &mut self,
        alpha: f32,
        camera: rw_sim::Vec2,
    ) -> Vec<(Drawable, DrawList)> {
        use rw_graphics::{FuseSprites, LightRodSprites, ZapCoilSprites, ZapFlashSprites};
        let index = self.sim.camera_room();
        let world = self.sim.world();
        let Some(r) = world.realized(index) else {
            return Vec::new();
        };
        let power = r.electric_power(&world.rain_cycle);
        let water_lights = r.settings.effect_amount(RoomEffectType::WaterLights) > 0.0;
        let f = &r.fixtures;
        let mut lists: Vec<(Drawable, DrawList)> = Vec::new();
        lists.extend(
            f.light_rods
                .iter()
                .map(|x| (Drawable::Fixture(x.update_order), x.draw(power))),
        );
        lists.extend(f.zap_coils.iter().map(|x| {
            (
                Drawable::Fixture(x.update_order),
                x.draw(alpha, water_lights),
            )
        }));
        lists.extend(
            f.fuses
                .iter()
                .map(|x| (Drawable::Fixture(x.update_order), x.draw(alpha, camera))),
        );
        let mut lights = Vec::new();
        for flash in f.zap_flashes.iter().filter(|x| !x.slated) {
            let (list, set) = flash.draw(alpha, &mut self.graphics.cosmetic);
            lists.push((Drawable::Fixture(flash.update_order), list));
            lights.extend(flash.light.zip(set));
        }
        if let Some(r) = self.sim.world_mut().realized_mut(index) {
            for (id, set) in lights {
                if let Some(l) = r.light_sources.iter_mut().find(|l| l.id == id) {
                    l.hard_set_rad(set.rad);
                    l.hard_set_alpha(set.alpha);
                    l.set_color(set.color);
                }
            }
        }
        lists
    }

    /// The camera room's `DeepProcessing` quads, `GravityDisruptor`s and their
    /// `DisruptorSpeck`s at `alpha` between ticks; `camera` is `CamPos(currentCameraPosition)`,
    /// which the disruptors' depth pushes from.
    pub fn ss_grab_draw_lists(
        &self,
        alpha: f32,
        camera: rw_sim::Vec2,
    ) -> Vec<(Drawable, DrawList)> {
        use rw_graphics::{DeepProcessingSprites, DisruptorSpeckSprites, GravityDisruptorSprites};
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let f = &r.fixtures;
        let mut lists: Vec<(Drawable, DrawList)> = Vec::new();
        lists.extend(
            f.deep_processing
                .iter()
                .map(|x| (Drawable::Fixture(x.update_order), x.draw())),
        );
        lists.extend(f.disruptor_specks.iter().filter_map(|x| {
            let power = f
                .gravity_disruptors
                .iter()
                .find(|d| d.update_order == x.disruptor)?
                .power;
            Some((Drawable::Fixture(x.update_order), x.draw(power, alpha)))
        }));
        lists.extend(
            f.gravity_disruptors
                .iter()
                .map(|x| (Drawable::Fixture(x.update_order), x.draw(alpha, camera))),
        );
        lists
    }

    /// The camera room's dandelions, stalks, green sparks, specks and spores at `alpha`
    /// between ticks in the camera's `palette` (a new spore's pebble draws on the
    /// cosmetic stream).
    pub fn particle_draw_lists(
        &mut self,
        alpha: f32,
        palette: &PaletteTexture,
    ) -> Vec<(Drawable, DrawList)> {
        let room = self.sim.camera_room();
        let Some(r) = self.sim.world().realized(room) else {
            return Vec::new();
        };
        let p = &r.particles;
        let colors = palette.room_palette();
        let (fog, sky) = (Color::from(colors.fog_color), Color::from(colors.sky_color));
        let [tr, tg, tb, ta] = palette.texel(5, 2).map(|c| f32::from(c) / 255.0);
        let root = Color {
            r: tr,
            g: tg,
            b: tb,
            a: ta,
        };
        let light = rw_sim::Vec2::from(r.room.light_angle());
        let mut lists = Vec::new();
        for d in &p.dandelions {
            lists.push((
                Drawable::Particle(d.update_order),
                d.draw(alpha, fog, light),
            ));
        }
        for s in &p.stalks {
            let head = s.dandelion.and_then(|o| {
                let i = p
                    .dandelions
                    .binary_search_by_key(&o, |d| d.update_order)
                    .ok()?;
                Some(p.dandelions[i].last_pos.lerp(p.dandelions[i].pos, alpha))
            });
            lists.push((
                Drawable::Particle(s.update_order),
                s.draw(alpha, head, root),
            ));
        }
        for s in &p.sparks {
            lists.push((Drawable::Particle(s.update_order), s.draw(alpha, sky)));
        }
        let rng = &mut self.graphics.cosmetic;
        let pebbles = &mut self.graphics.spore_pebbles;
        for s in &p.specks {
            let pebble = match pebbles.get(&s.update_order) {
                Some(&n) => Some(n),
                None => {
                    let n = s.pick_pebble(rng);
                    if let Some(n) = n {
                        pebbles.insert(s.update_order, n);
                    }
                    n
                }
            };
            lists.push((Drawable::Particle(s.update_order), s.draw(alpha, pebble)));
        }
        pebbles.retain(|order, _| {
            p.specks
                .binary_search_by_key(order, |s| s.update_order)
                .is_ok()
        });
        lists
    }

    /// The camera room's `CustomDecal`s and `Rainbow`s (the camera's `ghostMode` is 0).
    #[must_use]
    pub fn placed_visual_draw_lists(&self) -> Vec<(Drawable, DrawList)> {
        let Some(r) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let mut lists: Vec<(Drawable, DrawList)> = r
            .decals
            .iter()
            .map(|d| (Drawable::Decal(d.update_order), d.draw()))
            .collect();
        lists.extend(
            r.rainbows
                .iter()
                .map(|b| (Drawable::Rainbow(b.update_order), b.draw(0.0))),
        );
        lists
    }

    /// The camera room's placed light beams and spot lights at
    /// camera position `camera`. A beam's sprites are made again when the camera
    /// enters the room (`InitiateSprites`).
    pub fn placed_light_draw_lists(&mut self, camera: usize) -> Vec<(Drawable, DrawList)> {
        let index = self.sim.camera_room();
        let Some(r) = self.sim.world().realized(index) else {
            return Vec::new();
        };
        let view = LightBeamView {
            camera,
            shader_light: self.sim.world().rain_cycle.shader_light(),
            darken_lights: r.darken_lights_factor(),
            water_lights: r.settings.effect_amount(RoomEffectType::WaterLights) > 0.0,
        };
        let g = &mut self.graphics;
        let entered = g.beams_room != index;
        g.beams_room = index;
        g.beams
            .retain(|(id, _)| r.light_beams.iter().any(|b| b.update_order == *id));
        let mut lists = Vec::new();
        for b in &r.light_beams {
            let at = g.beams.iter().position(|(id, _)| *id == b.update_order);
            let (_, sprites) = match at {
                Some(at) => &mut g.beams[at],
                None => {
                    g.beams.push((b.update_order, LightBeamSprites::default()));
                    g.beams.last_mut().expect("just pushed")
                }
            };
            if entered {
                sprites.initiate();
            }
            lists.push((Drawable::LightBeam(b.update_order), sprites.draw(b, &view)));
        }
        lists.extend(
            r.spot_lights
                .iter()
                .map(|s| (Drawable::SpotLight(s.update_order), s.draw())),
        );
        lists
    }

    /// The image names of the camera room's decals.
    #[must_use]
    pub fn camera_room_decal_images(&self) -> Vec<&str> {
        self.sim
            .world()
            .realized(self.sim.camera_room())
            .map_or_else(Vec::new, |r| {
                r.decals
                    .iter()
                    .map(|d| d.data.image_name.as_str())
                    .collect()
            })
    }

    /// The lizards' sprites in the camera's room at `alpha` between ticks, in the
    /// order their modules were built, in the palette's `black` (their draws come from
    /// the cosmetic stream).
    pub fn lizard_draw_lists(&mut self, alpha: f32, black: Color) -> Vec<(Entity, DrawList)> {
        let world = self.sim.world();
        let camera_room = self.sim.camera_room();
        let realized: Vec<EntityId> = world
            .creatures()
            .filter(|(_, c)| c.lizard().is_some_and(|l| l.graphics.is_some()))
            .map(|(_, c)| c.id)
            .collect();
        self.graphics.drop_lizards(|g| !realized.contains(&g.id));
        let graphics = &mut self.graphics;
        let mut lists = Vec::new();
        for g in &mut graphics.lizards {
            let in_view = world
                .creature(g.creature)
                .lizard()
                .and_then(|l| l.graphics.as_ref())
                .is_some_and(|link| link.drawn_in.contains(&camera_room));
            if in_view {
                g.palette_black = Some(black);
            }
            let shown = world
                .creature(g.creature)
                .lizard()
                .is_some_and(|l| l.room == Some(camera_room) && !l.in_shortcut);
            if !shown {
                continue;
            }
            if let Some(view) = world.lizard_view(g.creature, None) {
                let list = g.draw(&view, black, alpha, &mut graphics.cosmetic);
                lists.push((Entity::Creature(g.creature), list));
            }
        }
        lists
    }

    /// Camera 0's record of its sprite containers after a tick ([`SpriteOrder`]): the
    /// drawables now in its room, the tick's jaw moves (placed by the update-list
    /// counter), then the flies' re-adds and the weapons' `ChangeOverlap`s.
    pub fn sync_sprite_order(&mut self) {
        let present = self.present_drawables();
        let moves: Vec<(u64, Move)> = self
            .sim
            .world_mut()
            .take_sprite_moves()
            .into_iter()
            .map(|m| {
                let obj = Drawable::Entity(m.obj);
                let mv = match m.into {
                    Some(holder) => Move::IntoJaws {
                        obj,
                        holder: Drawable::Entity(Entity::Creature(holder)),
                    },
                    None => Move::ToDefault(obj),
                };
                (m.seq, mv)
            })
            .collect();
        let world = self.sim.world();
        let late: Vec<Move> = std::mem::take(&mut self.graphics.flies_to_front)
            .into_iter()
            .filter_map(|id| {
                let (c, _) = world.creatures().find(|(_, c)| c.id == id)?;
                Some(Move::ToDefault(Drawable::Entity(Entity::Creature(c))))
            })
            .collect();
        let room = self.sim.camera_room();
        self.sprite_order.update(room, &present, &moves, &late);
    }

    /// A creature's graphics module in camera 0's room: one it kept from an earlier
    /// room (a fly, lizard or teleported overseer) joined `drawableObjects` with its
    /// `AddObject`, not `NowViewed`.
    fn creature_module(&self, d: Drawable, id: CreatureId, seq: u64, layer: u8) -> Present {
        let module = Present::module(d, seq, layer);
        if self.sprite_order.had_room() && !self.sim.now_viewed_modules().contains(&id) {
            Present {
                joined: Joined::AddObject,
                ..module
            }
        } else {
            module
        }
    }

    /// Every drawable in camera 0's room: what `AddObject` put in `drawableObjects`
    /// (the shelter door, rocks and spears, drips, cosmetics, lights, and the player once
    /// its graphics module outlived a room) and the creatures' graphics modules
    /// (`physicalObjects[0]`: flies, overseers; `[1]`: lizards, a first player).
    fn present_drawables(&self) -> Vec<Present> {
        let world = self.sim.world();
        let room = self.sim.camera_room();
        let mut present = Vec::new();
        if let Some(p) = self.shown_player()
            && !p.in_shortcut()
            && !p.is_destroyed()
            && self.graphics.module.is_some()
        {
            let joined = if self.sprite_order.had_room() {
                Joined::AddObject
            } else {
                Joined::NowViewed { layer: 1 }
            };
            let player = Drawable::Entity(Entity::Creature(self.sim.player_creature()));
            present.push(Present {
                joined,
                ..Present::added(player, self.sim.player_seq())
            });
        }
        if let Some(door) = self.sim.shelter_door(room) {
            present.push(Present::added(Drawable::ShelterDoor, door.update_order));
        }
        for (seq, id) in world.room_weapons(room) {
            let Some(w) = world.object(id).realized.as_deref() else {
                continue;
            };
            present.push(Present {
                overlap: Some(Overlap {
                    moves: w.overlap_moves,
                    in_front: w.in_front_of_objects != 0,
                    seq: w.overlap_seq,
                }),
                ..Present::added(Drawable::Entity(Entity::Object(id)), seq)
            });
        }
        for (id, c) in world.creatures() {
            let d = Drawable::Entity(Entity::Creature(id));
            if let Some(f) = c.fly()
                && f.room == Some(room)
                && f.graphics
            {
                present.push(self.creature_module(d, id, f.update_order, 0));
            } else if let Some(l) = c.lizard()
                && l.room == Some(room)
                && !l.in_shortcut
                && l.graphics.is_some()
            {
                present.push(self.creature_module(d, id, l.update_order, 1));
            } else if let Some(b) = c.overseer_body()
                && b.in_room
                && c.pos.room == room
                && self.graphics.overseers.iter().any(|g| g.id == c.id)
            {
                present.push(self.creature_module(d, id, b.update_order, 0));
            }
        }
        if let Some(r) = world.realized(room) {
            present.extend(
                r.drips
                    .iter()
                    .filter(|d| !d.slated)
                    .map(|d| Present::added(Drawable::Drip(d.id), d.update_order)),
            );
            present.extend(
                r.cosmetics
                    .iter()
                    .filter(|c| !c.slated)
                    .map(|c| Present::added(Drawable::Cosmetic(c.id), c.update_order)),
            );
            present.extend(
                r.holo_lights
                    .iter()
                    .filter(|l| !l.slated)
                    .map(|l| Present::added(Drawable::HoloLight(l.id), l.id)),
            );
            present.extend(
                r.light_sources
                    .iter()
                    .filter(|l| !l.slated)
                    .map(|l| Present::added(Drawable::LightSource(l.id), l.id)),
            );
            if let Some(c) = &r.insects {
                present.extend(
                    c.insects
                        .iter()
                        .filter(|i| !i.base.slated)
                        .map(|i| Present::added(Drawable::Insect(i.update_order), i.update_order)),
                );
            }
            present.extend(
                r.slime_molds
                    .iter()
                    .map(|m| Present::added(Drawable::SlimeMold(m.update_order), m.update_order)),
            );
            present.extend(
                r.token_stalks
                    .iter()
                    .map(|s| Present::added(Drawable::TokenStalk(s.update_order), s.update_order)),
            );
            present.extend(
                r.particles
                    .present_orders()
                    .map(|o| Present::added(Drawable::Particle(o), o)),
            );
            present.extend(
                r.decals
                    .iter()
                    .map(|d| Present::added(Drawable::Decal(d.update_order), d.update_order)),
            );
            present.extend(
                r.rainbows
                    .iter()
                    .map(|b| Present::added(Drawable::Rainbow(b.update_order), b.update_order)),
            );
            present.extend(
                r.light_beams
                    .iter()
                    .map(|b| Present::added(Drawable::LightBeam(b.update_order), b.update_order)),
            );
            present.extend(
                r.spot_lights
                    .iter()
                    .map(|s| Present::added(Drawable::SpotLight(s.update_order), s.update_order)),
            );
            present.extend(
                r.background_scenes
                    .iter()
                    .flat_map(|scene| &scene.elements)
                    .filter(|e| e.update_order != 0)
                    .map(|e| {
                        Present::added(Drawable::SceneElement(e.update_order), e.update_order)
                    }),
            );
            present.extend(
                r.dust_puffs
                    .iter()
                    .map(|p| Present::added(Drawable::DustPuff(p.update_order), p.update_order)),
            );
            present.extend(
                r.fixtures
                    .drawn_orders()
                    .map(|o| Present::added(Drawable::Fixture(o), o)),
            );
            present.extend(
                r.electric_deaths.iter().map(|d| {
                    Present::added(Drawable::ElectricDeath(d.update_order), d.update_order)
                }),
            );
            present.extend(
                r.projections
                    .drawn_orders()
                    .map(|o| Present::added(Drawable::Projection(o), o)),
            );
            present.extend(
                r.coral
                    .iter()
                    .flat_map(|s| &s.neurons)
                    .map(|n| Present::added(Drawable::Coral(n.update_order), n.update_order)),
            );
            present.extend(
                r.coral
                    .iter()
                    .flat_map(|s| &s.stems)
                    .flat_map(|stem| &stem.segments)
                    .map(|g| Present::added(Drawable::Coral(g.update_order), g.update_order)),
            );
            present.extend(
                r.coral
                    .iter()
                    .flat_map(|s| &s.wall_mycelia)
                    .map(|w| Present::added(Drawable::Coral(w.update_order), w.update_order)),
            );
            for circuit in r.coral.iter().flat_map(|s| &s.circuits) {
                let orders = circuit
                    .bits
                    .iter()
                    .map(|b| b.update_order)
                    .chain(circuit.connectors.iter().map(|x| x.update_order))
                    .chain(circuit.crabs.iter().map(|x| x.update_order));
                present.extend(orders.map(|o| Present::added(Drawable::Coral(o), o)));
            }
        }
        present
    }

    /// The frame's sprites in camera 0's containers: a weapon in the container its last
    /// re-add chose, what the lizards' jaws hold nested in them ([`Jaws`]), the sprites
    /// of a jaw container off the stage left out, the rest in [`SpriteOrder`]'s order.
    pub fn in_container_order(&self, mut lists: Vec<(Drawable, DrawList)>) -> Vec<DrawList> {
        let behind = self.sprite_order.behind_set();
        for (d, list) in &mut lists {
            if !matches!(d, Drawable::Entity(Entity::Object(_))) {
                continue;
            }
            let layer = if behind.contains(d) {
                Layer::Background
            } else {
                Layer::Items
            };
            for item in &mut list.items {
                if matches!(item.layer(), Layer::Items | Layer::Background) {
                    item.set_layer(layer);
                }
            }
        }
        let holders = lists.iter().map(|(d, _)| self.jaw_holder(*d)).collect();
        let player = Drawable::Entity(Entity::Creature(self.sim.player_creature()));
        Jaws::new(holders, player).contain(&mut lists);
        let mut ordered = Vec::with_capacity(lists.len());
        let shown = self.sprite_order.shown_set();
        let count = lists.len();
        let mut lists: HashMap<Drawable, DrawList> = lists.into_iter().collect();
        debug_assert_eq!(lists.len(), count, "a drawable has two draw lists");
        for d in self.sprite_order.order() {
            let Some(mut list) = lists.remove(&d) else {
                continue;
            };
            if !shown.contains(&d) {
                // Only what went into the jaw container left the stage with it: the
                // player's `Foreground` sprites stay (`PlayerGraphics.AddToContainer`).
                if d != player {
                    continue;
                }
                list.items.retain(|item| item.layer() != Layer::Midground);
            }
            ordered.push(list);
        }
        ordered
    }

    /// `ApplyPalette`: camera 0's full-screen effect sprite back on top of its container.
    pub fn raise_full_screen_effect(&mut self) {
        self.sprite_order.raise_full_screen_effect();
    }

    /// Camera 0's container record (for MCP replies).
    #[must_use]
    pub fn sprite_order(&self) -> &SpriteOrder {
        &self.sprite_order
    }

    /// A drawn lizard's jaw container and what the camera has in it.
    fn jaw_holder(&self, d: Drawable) -> Option<JawHolder> {
        let Drawable::Entity(Entity::Creature(lizard)) = d else {
            return None;
        };
        let container = self
            .graphics
            .lizards
            .iter()
            .find(|g| g.creature == lizard)
            .and_then(|g| g.internal_container)?;
        Some(JawHolder {
            container,
            held: self.sprite_order.held_by(d),
        })
    }

    /// The lizards' graphics modules (for MCP replies).
    #[must_use]
    pub fn lizard_graphics(&self) -> &[LizardGraphics] {
        &self.graphics.lizards
    }

    /// The guide's lights and their light sources in the camera's room at `alpha`
    /// between ticks, in update-list order (`HologramLight.DrawSprites` on the cosmetic
    /// stream, hard-setting its light source before that draws); `cam_pos` and
    /// `darkness` are the camera's position and palette darkness.
    pub fn holo_light_draw_lists(
        &mut self,
        alpha: f32,
        cam_pos: Vec2,
        darkness: f32,
    ) -> Vec<(Drawable, DrawList)> {
        let index = self.sim.camera_room();
        let Some(room) = self.sim.world().realized(index) else {
            return Vec::new();
        };
        let view = LightSourceView {
            cam_pos,
            darkness,
            darken_lights: room.darken_lights_factor(),
            water: room.room.water_level().is_some(),
            water_lights: room.settings.effect_amount(RoomEffectType::WaterLights) > 0.0,
            shader_light: self.sim.world().rain_cycle.shader_light(),
        };
        let mut order: Vec<_> = room
            .holo_lights
            .iter()
            .filter(|l| !l.slated)
            .map(|l| LightDrawable::Holo(l.id))
            .chain(
                room.light_sources
                    .iter()
                    .filter(|l| !l.slated)
                    .map(|l| LightDrawable::Source(l.id)),
            )
            .collect();
        order.sort_by_key(LightDrawable::id);
        let mut lists = Vec::with_capacity(order.len());
        for drawable in order {
            match drawable {
                LightDrawable::Holo(id) => {
                    if let Some(list) = self.holo_light_frame(index, id, alpha) {
                        lists.push((Drawable::HoloLight(id), list));
                    }
                }
                LightDrawable::Source(id) => {
                    let light = self
                        .sim
                        .world()
                        .realized(index)
                        .and_then(|r| r.light_sources.iter().find(|l| l.id == id));
                    if let Some(light) = light {
                        lists.push((Drawable::LightSource(id), light.draw(&view, alpha)));
                    }
                }
            }
        }
        lists
    }

    /// The camera room's light sources as `Water.GetSurfaceColor` reads them (`Alpha`).
    #[must_use]
    pub fn surface_lights(&self) -> Vec<SurfaceLight> {
        let shader_light = self.sim.world().rain_cycle.shader_light();
        self.sim
            .world()
            .realized(self.sim.camera_room())
            .map_or_else(Vec::new, |r| {
                r.light_sources
                    .iter()
                    .filter(|l| !l.slated)
                    .map(|l| SurfaceLight {
                        pos: l.pos.to_array(),
                        rad: l.rad,
                        color: l.color,
                        alpha: l.alpha_now(shader_light),
                    })
                    .collect()
            })
    }

    /// Guide light `id`'s sprites; hard-sets its light source.
    fn holo_light_frame(&mut self, index: i32, id: u64, alpha: f32) -> Option<DrawList> {
        let world = self.sim.world();
        let room = world.realized(index)?;
        let l = room.holo_lights.iter().find(|l| l.id == id)?;
        let c = world.creature(l.overseer);
        let eye = c
            .overseer_body()
            .filter(|b| b.in_room && c.pos.room == l.room)
            .map(|b| {
                self.graphics
                    .overseers
                    .iter()
                    .find(|g| g.id == c.id)
                    .map_or_else(
                        || b.chunk().last_pos.lerp(b.chunk().pos, alpha),
                        |g| g.eye(b, alpha),
                    )
            });
        let source = l.light_source;
        let (list, set) = l.draw(&room.room, eye, alpha, &mut self.graphics.cosmetic);
        let light = source.and_then(|s| {
            self.sim
                .world_mut()
                .realized_mut(index)?
                .light_sources
                .iter_mut()
                .find(|l| l.id == s)
        });
        if let Some(light) = light {
            light.hard_set(set);
        }
        Some(list)
    }

    /// The water drips in the camera's room at `alpha` between ticks, oldest first
    /// (`WaterDrip.DrawSprites` on the cosmetic stream), in the room `palette`.
    pub fn drip_draw_lists(
        &mut self,
        alpha: f32,
        palette: DripPalette,
    ) -> Vec<(Drawable, DrawList)> {
        let Some(room) = self.sim.world().realized(self.sim.camera_room()) else {
            return Vec::new();
        };
        let glitter = room.water_glitter;
        room.drips
            .iter()
            .filter(|d| !d.slated)
            .map(|d| {
                let list = d.draw(glitter, palette, alpha, &mut self.graphics.cosmetic);
                (Drawable::Drip(d.id), list)
            })
            .collect()
    }

    /// The flies' sprites in the camera's room at `alpha` between ticks, in sprite
    /// order, in the room palette's `black`. Drops the graphics of flies no longer
    /// realized.
    pub fn fly_draw_lists(&mut self, alpha: f32, black: Color) -> Vec<(Entity, DrawList)> {
        let world = self.sim.world();
        let camera_room = self.sim.camera_room();
        let shown: Vec<CreatureId> = world
            .creatures()
            .filter(|(_, c)| {
                c.fly()
                    .is_some_and(|f| f.room == Some(camera_room) && f.graphics)
            })
            .map(|(id, _)| id)
            .collect();
        let realized: Vec<EntityId> = world
            .creatures()
            .filter(|(_, c)| c.fly().is_some())
            .map(|(_, c)| c.id)
            .collect();
        self.graphics.flies.retain(|g| realized.contains(&g.id));
        let views: Vec<(CreatureId, FlyView<'_>)> = shown
            .iter()
            .filter_map(|&c| Some((c, world.realized_fly_view(c)?)))
            .collect();
        self.graphics
            .flies
            .iter()
            .filter_map(|g| {
                let (c, view) = views.iter().find(|(_, v)| v.id == g.id)?;
                Some((Entity::Creature(*c), g.draw(view, black, alpha)))
            })
            .collect()
    }

    /// Camera 0's shortcut sprites at this frame (`ShortcutGraphics.Draw`), its image at
    /// `camera` in the room.
    pub fn shortcut_draw_list(
        &mut self,
        palette: ShortcutPalette,
        camera: Vec2,
    ) -> Option<DrawList> {
        self.shortcuts.draw(
            &self.sim,
            self.graphics.slugcat,
            palette,
            camera,
            &mut self.graphics.cosmetic,
        )
    }

    /// Camera 0's shortcut graphics.
    #[cfg(test)]
    pub fn shortcut_graphics(&mut self) -> Option<&rw_graphics::ShortcutGraphics> {
        self.shortcuts.current(&self.sim)
    }

    /// The camera room palette's earth colour, which the overseers' hologram colour
    /// mixes in.
    pub fn set_palette_earth(&mut self, earth: Color) {
        self.graphics.earth = earth;
    }

    /// The holograms in the camera's room at `alpha` between ticks, in the order they
    /// joined the `Bloom` container (creation); `cam_pos` is the game's `camPos` and
    /// `screen_size` its screen. Drops the graphics of holograms that are gone.
    pub fn hologram_draw_lists(
        &mut self,
        alpha: f32,
        cam_pos: Vec2,
        screen_size: Vec2,
    ) -> Vec<DrawList> {
        let world = self.sim.world();
        let camera_room = self.sim.camera_room();
        let live: Vec<u64> = world
            .creatures()
            .filter_map(|(_, c)| c.overseer_body())
            .flat_map(|b| b.holograms.iter().map(|h| h.id))
            .collect();
        self.graphics.holograms.retain(|g| live.contains(&g.id));
        let mut shown: Vec<(&OverseerHologram, Vec2)> = Vec::new();
        for &c in world.room_creatures(camera_room) {
            let creature = world.creature(c);
            let Some(body) = creature.overseer_body() else {
                continue;
            };
            let graphics = self.graphics.overseers.iter().find(|g| g.id == creature.id);
            let head = match graphics {
                Some(g) if body.in_room => g.eye(body, alpha),
                _ => body.chunk().last_pos.lerp(body.chunk().pos, alpha),
            };
            shown.extend(
                body.holograms
                    .iter()
                    .filter(|h| h.room == camera_room)
                    .map(|h| (h, head)),
            );
        }
        shown.sort_by_key(|(h, _)| h.id);
        let graphics = &mut self.graphics;
        let mut lists = Vec::new();
        for (h, head) in shown {
            let Some(g) = graphics.holograms.iter_mut().find(|g| g.id == h.id) else {
                continue;
            };
            let frame = HologramFrame {
                t: alpha,
                cam_pos,
                screen_size,
                head,
            };
            lists.push(g.draw(h, &frame, &mut graphics.cosmetic));
        }
        lists
    }

    /// The rain cycle as the HUD's rain meter reads it.
    #[must_use]
    pub fn rain_view(&self) -> RainCycleView {
        let rain = &self.sim.world().rain_cycle;
        RainCycleView {
            cycle_length: rain.cycle_length,
            timer: rain.timer,
            disable_rain: false,
        }
    }

    /// What the overseers told the HUD since the last call.
    pub fn take_hud_events(&mut self) -> Vec<HudEvent> {
        self.sim.take_hud_events()
    }

    /// The water surface as drawn at `alpha` between ticks; `None` without water.
    #[must_use]
    pub fn water_surface(&self, alpha: f32) -> Option<WaterSurface> {
        let water = self.water()?;
        let front = water.draw_points(alpha).map(Vec2::to_array).collect();
        let back = water.back_draw_points(alpha).map(Vec2::to_array).collect();
        Some(WaterSurface::from_rows(front, back, water.still_level()))
    }

    /// `PreviousPoint(x)` on the water's front points this tick.
    #[must_use]
    pub fn water_previous_point(&self, x: f32) -> Option<usize> {
        self.water().map(|w| w.previous_point(x))
    }

    /// `_waterDepth` (`RoomCamera.MoveCamera`): 0 with the water in front of the
    /// terrain, 1/31 behind it, 2/31 in gates.
    #[must_use]
    pub fn water_depth(&self) -> f32 {
        let room = self.room();
        let gate = room.name().to_ascii_uppercase().starts_with("GATE_");
        let depth = if gate {
            2.0
        } else if room.water_in_front() {
            0.0
        } else {
            1.0
        };
        depth / 31.0
    }

    /// `Room.waterGlitterCycle` between ticks: it drops by 1/60 every room tick.
    #[must_use]
    pub fn water_glitter(&self, alpha: f32) -> f32 {
        let tick = self.sim.ticks();
        let now = -(tick as f32) / 60.0;
        let last = -(tick.saturating_sub(1) as f32) / 60.0;
        last + (now - last) * alpha
    }

    /// The followed player's light in the water: its main chunk at `alpha` and
    /// `outOfBreathFac`; `None` when dead, destroyed, elsewhere or in a shortcut.
    #[must_use]
    pub fn player_light(&self, alpha: f32) -> Option<PlayerLight> {
        let player = self.shown_player()?;
        if player.is_dead() || player.in_shortcut() {
            return None;
        }
        Some(PlayerLight {
            pos: player.body.chunks[0].interpolated_pos(alpha),
            out_of_breath: player.out_of_breath(),
        })
    }

    /// Whether the player is dead.
    #[must_use]
    pub fn player_dead(&self) -> bool {
        self.sim.player().is_some_and(Player::is_dead)
    }

    #[must_use]
    pub fn status(&self) -> BodyStatus {
        BodyStatus {
            stunned: self.sim.player().is_some_and(Player::is_stunned),
            dead: self.player_dead(),
        }
    }

    /// The camera's room.
    #[must_use]
    pub fn room(&self) -> &Room {
        &self.camera_room().room
    }

    /// The player's body while it is in the camera's room (not destroyed, not in a
    /// shortcut).
    #[must_use]
    pub fn body(&self) -> Option<&PhysicalObject> {
        self.shown_player()
            .filter(|p| !p.in_shortcut())
            .map(|p| &p.body)
    }

    /// The body whose chunk emitters the audio follows: [`Game::body`], and the tick the
    /// player went into a shortcut too, as its `ChunkSoundEmitter`s update before it
    /// (`Room.Update` walks the newest first) and are destroyed only at the next tick.
    #[must_use]
    pub fn sound_body(&self) -> Option<&PhysicalObject> {
        self.shown_player()
            .filter(|p| !p.in_shortcut() || self.entered_shortcut)
            .map(|p| &p.body)
    }

    /// The debug view's other bodies in the camera's room: realized lizards, flies and
    /// overseers, then rocks and spears, each with its status and kind.
    #[must_use]
    pub fn debug_bodies(&self) -> Vec<(&PhysicalObject, BodyStatus, DebugBody)> {
        use rw_sim::creature::RealizedCreature as R;
        let room = self.sim.camera_room();
        let world = self.sim.world();
        let mut list = Vec::new();
        for (_, c) in world.creatures() {
            let (body, stun, dead, kind) = match &c.realized {
                Some(R::Lizard(l)) if l.room == Some(room) => {
                    (&l.body, l.stunned(), l.dead, DebugBody::Lizard)
                }
                Some(R::Fly(f)) if f.room == Some(room) => {
                    (&f.body, f.stunned(), f.dead, DebugBody::Fly)
                }
                Some(R::Overseer(b)) if b.in_room && c.pos.room == room => {
                    (&b.body, b.stun >= 10, b.dead, DebugBody::Overseer)
                }
                _ => continue,
            };
            let status = BodyStatus {
                stunned: stun && !dead,
                dead,
            };
            list.push((body, status, kind));
        }
        for (_, o) in world.objects() {
            if let Some(w) = o.realized.as_deref().filter(|w| w.room == room) {
                list.push((&w.body, BodyStatus::ALIVE, DebugBody::Item));
            }
        }
        list
    }

    /// The shortcut tile the player is travelling through in the camera's room, if any.
    #[must_use]
    pub fn vessel_tile(&self) -> Option<(i32, i32)> {
        self.shown_player()?;
        self.sim.player_vessel_tile(self.sim.camera_room())
    }

    /// What the room camera follows: the player's first chunk and input, or the centre
    /// of its shortcut tile while it travels through one. `None` once destroyed.
    #[must_use]
    pub fn camera_follow(&self) -> Option<Follow> {
        let player = self.shown_player()?;
        let chunk = &player.body.chunks[0];
        let input = player.input_history()[0];
        Some(Follow {
            pos: chunk.pos.to_array(),
            vel: chunk.vel.to_array(),
            input: [input.x, input.y],
            in_shortcut_at: self
                .vessel_tile()
                .map(|(x, y)| rw_assets::tile_centre(x, y)),
        })
    }

    /// The camera's room's water, if any.
    #[must_use]
    pub fn water(&self) -> Option<&Water> {
        self.camera_room().water.as_ref()
    }

    /// What the microphone listens from: the player's main chunk, or its shortcut
    /// tile while it travels.
    #[must_use]
    pub fn sound_follow(&self) -> SoundFollow {
        match self.shown_player() {
            None => SoundFollow::Abstract,
            Some(p) if p.in_shortcut() => SoundFollow::Shortcut(
                self.vessel_tile()
                    .map(|(x, y)| Vec2::from(rw_assets::tile_centre(x, y))),
            ),
            Some(p) => SoundFollow::Body(p.body.chunks[0].pos),
        }
    }

    /// Ticks simulated since start.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.sim.ticks()
    }

    /// The player, unless destroyed.
    #[must_use]
    pub fn live_player(&self) -> Option<&Player> {
        self.sim.player()
    }

    /// Moves the player (a new one if it was destroyed) into the camera's room so its
    /// chunks sit 1 px above and below `pos`, moving at `vel`, out of any shortcut.
    /// Movement state is kept; the graphics module starts afresh.
    pub fn set_player(&mut self, pos: Vec2, vel: Vec2) {
        self.sim.set_player(pos, vel, &mut self.graphics);
    }

    pub fn set_food_in_stomach(&mut self, food: u32) {
        self.sim.set_food_in_stomach(food);
    }

    /// [`RainWorldGame::dev_spawn_creature`] with the graphics as the observer.
    ///
    /// # Errors
    /// For a type that is not a spawnable creature or an unknown room.
    pub fn dev_spawn_creature(
        &mut self,
        kind: rw_assets::CreatureType,
        room: i32,
        tile: (i32, i32),
        seed_id: Option<i32>,
    ) -> Result<CreatureId, String> {
        self.sim
            .dev_spawn_creature(kind, room, tile, seed_id, &mut self.graphics)
    }

    /// [`RainWorldGame::dev_spawn_object`].
    ///
    /// # Errors
    /// For a kind the port does not realize or an unknown room.
    pub fn dev_spawn_object(
        &mut self,
        kind: rw_assets::AbstractObjectType,
        room: i32,
        tile: (i32, i32),
        explosive: bool,
    ) -> Result<rw_sim::item::ObjectId, String> {
        self.sim.dev_spawn_object(kind, room, tile, explosive)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An all-Air 20 x 12 room.
    pub(crate) fn open_room() -> Room {
        let (w, h) = (20, 12);
        let cells = "0|".repeat(w * h);
        let text = format!(
            "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{cells}\n"
        );
        Room::parse(&text).expect("test room parses")
    }

    impl Game {
        fn player(&self) -> &Player {
            self.sim.player().expect("player present")
        }

        fn head_y(&self) -> f32 {
            self.body().expect("body in room").chunks[0].pos.y
        }

        /// The graphics observer, with the sim it watches.
        pub(crate) fn observer_and_sim(&mut self) -> (&mut dyn PlayerObserver, &RainWorldGame) {
            (&mut self.graphics, &self.sim)
        }

        /// The observer's `lizard_removed` (`RemoveGraphicsModule`) for body `id`.
        pub(crate) fn remove_lizard_module(&mut self, id: EntityId) {
            self.graphics.lizard_removed(id);
        }

        /// What a cosmetic given lizard `id`'s module `module` reads at `t`.
        pub(crate) fn lizard_module(
            &self,
            id: CreatureId,
            module: u64,
            t: f32,
        ) -> Option<LizardModule> {
            LizardHeads {
                graphics: &self.graphics.lizards,
                dropped: &self.graphics.dropped_lizards,
                world: self.sim.world(),
                black: Color::BLACK,
            }
            .lizard_module(id, module, t)
        }
    }

    /// A closed 11 x 5 room with a pipe entrance at (3, 1) and one at (5, 1).
    fn pipe_room() -> Room {
        let rows = [
            "###########",
            "#...#.....#",
            "#...#.....#",
            "#..ExE....#",
            "###########",
        ];
        let mut grid = String::new();
        for x in 0..rows[0].len() {
            for row in rows {
                grid.push_str(match row.as_bytes()[x] {
                    b'.' => "0|",
                    b'#' => "1|",
                    b'E' => "4,3|",
                    _ => "1,3|",
                });
            }
        }
        let text = format!(
            "Test\n11*5|-1|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{grid}\n"
        );
        Room::parse(&text).expect("test room parses")
    }

    /// The player's `ChunkSoundEmitter`s are destroyed one tick after `SuckedIntoShortCut`
    /// removes it from the room: the emitters update before the player in `Room.Update`'s
    /// backward walk, so the tick it goes in they still follow its chunks.
    #[test]
    fn the_audio_follows_the_body_one_tick_into_a_shortcut() {
        let mut game = Game::new(pipe_room(), &RoomSettings::ANCESTOR, 1, Some((2, 1)));
        for _ in 0..20 {
            game.tick(Input::NONE);
        }
        assert!(game.sound_body().is_some());
        let mut entered = false;
        for _ in 0..120 {
            game.tick(Input::new(1, 0, false));
            if game.sim.player().is_some_and(Player::in_shortcut) {
                entered = true;
                break;
            }
        }
        assert!(entered, "the player went into the pipe");
        assert!(game.body().is_none());
        assert!(game.sound_body().is_some(), "the tick it went in");
        game.tick(Input::NONE);
        assert!(game.sound_body().is_none(), "the next tick");
    }

    #[test]
    fn player_starts_on_all_fours_and_falls() {
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        assert!(!game.player().is_standing());
        let (before, start) = (game.head_y(), game.ticks());
        for _ in 0..5 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.ticks(), start + 5);
        assert!(game.head_y() < before, "gravity pulls it down");
    }

    #[test]
    fn falling_out_removes_the_player_until_respawn() {
        // The all-Air room has an open bottom: the player falls out and is destroyed.
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        for _ in 0..400 {
            if game.sim.player().is_none() {
                break;
            }
            game.tick(Input::NONE);
        }
        assert!(game.sim.player().is_none());
        assert!(game.body().is_none());
        assert!(!game.player_dead());
        game.tick(Input::NONE);
        game.respawn();
        assert!(game.body().is_some());
    }

    #[test]
    fn dead_player_keeps_falling_and_r_respawns_outside_a_story() {
        // The dead body keeps its physics (`Player.Update` runs dead); R is a dev key.
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        game.sim.kill_player();
        let y = game.head_y();
        for _ in 0..10 {
            game.tick(Input::NONE);
        }
        assert!(game.head_y() < y, "falls");
        assert!(game.player_dead());
        assert!(game.respawn());
        assert!(!game.player_dead());
        game.start_condition = StartCondition::New;
        assert!(!game.respawn(), "no R in a story session");
    }

    #[test]
    fn respawn_gives_a_fresh_player() {
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        for _ in 0..10 {
            game.tick(Input::new(1, 1, false));
        }
        game.respawn();
        assert!(!game.player().is_standing());
        assert_eq!(
            game.player().input_history(),
            &[Input::NONE; rw_sim::INPUT_HISTORY]
        );
        assert_eq!(game.body().expect("body").chunks[0].vel, rw_sim::Vec2::ZERO);
    }

    #[test]
    fn slated_light_source_is_not_drawn() {
        // `LightSource.DrawSprites` ends with `CleanSpritesAndRemove` when slated, in
        // the same call that set the sprites, so Futile never renders them.
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        let index = game.sim.camera_room();
        let pos = rw_sim::Vec2::new(200.0, 120.0);
        let mut light = rw_sim::LightSource::new(999, index, pos, false, [1.0, 0.8, 0.3], None);
        light.rad = 100.0;
        light.last_rad = 100.0;
        let room = game.sim.world_mut().realized_mut(index).expect("room");
        room.light_sources.push(light);
        let cam = rw_sim::Vec2::ZERO;
        assert_eq!(game.holo_light_draw_lists(1.0, cam, 0.0).len(), 1);
        let room = game.sim.world_mut().realized_mut(index).expect("room");
        room.light_sources[0].slated = true;
        assert!(game.holo_light_draw_lists(1.0, cam, 0.0).is_empty());
    }
}

/// A guide light or a light source, by update-list id.
#[derive(Debug, Clone, Copy)]
enum LightDrawable {
    Holo(u64),
    Source(u64),
}

impl LightDrawable {
    fn id(&self) -> u64 {
        match *self {
            Self::Holo(id) | Self::Source(id) => id,
        }
    }
}

/// The lizards' head colours, as their cosmetics read them when drawn.
struct LizardHeads<'a> {
    graphics: &'a [LizardGraphics],
    dropped: &'a [LizardGraphics],
    world: &'a World,
    black: Color,
}

impl LizardColors for LizardHeads<'_> {
    /// The module's own lizard while it is realized (the game's module reads the
    /// body it was made for), else as the module last read it.
    fn lizard_module(&self, id: CreatureId, module: u64, t: f32) -> Option<LizardModule> {
        let g = self
            .graphics
            .iter()
            .chain(self.dropped)
            .find(|g| g.module == module)?;
        let lizard = self
            .world
            .lizard_view(id, None)
            .map_or(g.lizard, |v| LizardReading::of(&v));
        Some(g.for_cosmetics(&lizard, self.black, t))
    }

    fn black(&self) -> Color {
        self.black
    }
}
