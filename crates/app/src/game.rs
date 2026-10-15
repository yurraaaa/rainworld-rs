//! Simulation state owned by the app: the game ([`RainWorldGame`]: world, rooms, player,
//! shortcuts, random stream and sounds) and the player's graphics module, seen from the
//! camera's room.
//!
//! This is the only place the app touches the simulation.

use std::sync::Arc;

use rw_assets::unity::AssetBundle;
use rw_assets::world::AbstractRoomData;
use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{ActiveMods, Room, RoomSettings, SlugcatName, SoundTable, StreamingAssets};
use rw_audio::Follow as SoundFollow;
use rw_graphics::{
    Color, DrawList, HologramFrame, HologramGraphics, OverseerGraphics, PlayerGraphics,
};
use rw_hud::{HudSetup, OwnerTick, RainCycleView, StartCondition, Subregions};
use rw_random::UnityRandom;
use rw_render::{BodyStatus, ChunkInterpolation, Follow, PlayerLight, WaterSurface};
use rw_sim::campaign::{self, SlugcatCampaign};
use rw_sim::creature::overseer::OverseerView;
use rw_sim::creature::overseer::hologram::OverseerHologram;
use rw_sim::creature::{CreatureId, EntityId};
use rw_sim::room_script::{RoomScript, SuC04StartUp};
use rw_sim::world::World;
use rw_sim::{
    CameraMove, CameraView, HudEvent, Input, PhysicalObject, Player, PlayerObserver, RainWorldGame,
    SoundEvent, Sounds, Start, Vec2, Water,
};

/// How a game starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameStart {
    /// `StoryGameInitCondition.New` for a campaign: its start den, at the den's start
    /// position (`TryGetPlayerStartPos`, else (15, 25)).
    NewStory(SlugcatName),
    /// A given room, in its region's world (a single-room world for arena levels), as
    /// the Survivor: on `tile`, or on a safe spawn tile (the dev start, `--spawn`).
    Room {
        name: String,
        tile: Option<(i32, i32)>,
    },
}

/// The game and the player's graphics.
pub struct Game {
    sim: RainWorldGame,
    graphics: Graphics,
    /// `menuSetup.startGameCondition`: `New` for a new story, `Dev` for a room start.
    start_condition: StartCondition,
    /// The region's subregions, for the HUD; `None` in a world without a region.
    subregions: Option<Subregions>,
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
    /// The camera room palette's earth colour (`OverseerGraphics.ApplyPalette`).
    earth: Color,
    /// Holograms the sim cannot make yet, drawn in the camera's room after the real ones
    /// (screenshot tests).
    #[cfg(test)]
    extra_holograms: Vec<(OverseerHologram, HologramGraphics)>,
    /// The lens's and holograms' per-frame draws (the game's come from the global
    /// stream: D1).
    cosmetic: UnityRandom,
}

impl Graphics {
    /// Overseer `id`'s graphics module and its conversation partner's.
    fn overseer_pair(
        &mut self,
        id: EntityId,
        partner: Option<EntityId>,
    ) -> Option<(&mut OverseerGraphics, Option<&mut OverseerGraphics>)> {
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
                Some((a, Some(b)))
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
        rng: &mut UnityRandom,
    ) {
        if let Some(g) = &mut self.module {
            g.update(player, room, water, rng);
        }
    }

    fn spat_out(&mut self, player: &Player, rng: &mut UnityRandom) {
        if let Some(g) = &mut self.module {
            g.spat_out_of_shortcut(player, rng);
        }
    }

    fn overseer_added(&mut self, view: &OverseerView<'_>, rng: &mut UnityRandom) {
        if !self.overseers.iter().any(|g| g.id == view.id) {
            self.overseers.push(OverseerGraphics::new(view, rng));
        }
        self.set_hologram_base(view);
    }

    fn overseer_reset(&mut self, view: &OverseerView<'_>, rng: &mut UnityRandom) {
        if let Some(g) = self.overseers.iter_mut().find(|g| g.id == view.id) {
            g.reset(view.body, rng);
        }
    }

    fn overseer_emerge(&mut self, view: &OverseerView<'_>, rng: &mut UnityRandom) {
        if let Some(g) = self.overseers.iter_mut().find(|g| g.id == view.id) {
            g.emerge(view.body, rng);
        }
    }

    fn overseer_updated(
        &mut self,
        view: &OverseerView<'_>,
        room: &Room,
        camera: &CameraView,
        rng: &mut UnityRandom,
    ) {
        if let Some((g, partner)) = self.overseer_pair(view.id, view.partner) {
            g.update(view, room, camera, partner, rng);
            self.set_hologram_base(view);
        }
    }

    fn overseer_removed(&mut self, id: EntityId) {
        self.overseers.retain(|g| g.id != id);
        self.hologram_bases.retain(|(o, _)| *o != id);
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
    /// bits).
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
        Self::build(
            world,
            campaign,
            start,
            UnityRandom::new(seed as i32),
            Sounds::default(),
        )
    }

    /// The game `start` asks for, read from the installed game, drawing from `rng` (the
    /// stream the menus drew from before, D1) and playing sounds from `table`.
    ///
    /// # Errors
    /// When the world or the first room cannot be loaded.
    pub fn start(
        assets: &StreamingAssets,
        start: &GameStart,
        rng: UnityRandom,
        table: Arc<SoundTable>,
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
            GameStart::Room { name, tile } => (&SlugcatCampaign::SURVIVOR, name.clone(), *tile),
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
        let world = World::new(data, rooms, campaign.timeline, mods, true);
        let start_at = Start { room: first, tile };
        let mut game = Self::build(world, campaign, start_at, rng, Sounds::new(table));
        game.subregions = subregions;
        game.start_condition = match start {
            GameStart::NewStory(_) => StartCondition::New,
            GameStart::Room { .. } => StartCondition::Dev,
        };
        Ok(game)
    }

    fn build(
        world: World,
        campaign: &'static SlugcatCampaign,
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
            earth: Color::BLACK,
            #[cfg(test)]
            extra_holograms: Vec::new(),
            cosmetic: UnityRandom::new(0),
        };
        let sim = RainWorldGame::new(world, campaign, start, rng, sounds, &mut graphics);
        log::info!("spawned player");
        Self {
            sim,
            graphics,
            start_condition: StartCondition::Dev,
            subregions: None,
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
    #[must_use]
    pub fn hud_due(&self) -> bool {
        self.sim.player().is_some() && !self.sim.world().single_room()
    }

    /// What `InitSinglePlayerHud` and `FireUpSinglePlayerHUD` read: the slugcat's food
    /// meter, the new game's karma (no saves yet), cycle 0, and the world's rain cycle.
    #[must_use]
    pub fn hud_setup(&self, screen_size: [f32; 2]) -> HudSetup {
        let campaign = self.sim.campaign();
        let world = self.sim.world();
        HudSetup {
            screen_size,
            max_food: campaign.stats.max_food,
            food_to_hibernate: campaign.stats.food_to_hibernate,
            current_food: 0,
            karma: campaign.start_karma,
            karma_cap: campaign.start_karma_cap,
            karma_reinforced: false,
            cycle_number: self.sim.story_session().cycle_number,
            camera_room_shelter: world
                .abstract_room(self.sim.camera_room())
                .is_some_and(AbstractRoomData::shelter),
            rain: self.rain_view(),
            subregions: self.subregions.clone(),
            start: self.start_condition,
            swarm_rooms: world
                .data()
                .rooms
                .iter()
                .filter(|r| r.swarm_room_index > -1)
                .count(),
        }
    }

    /// What the HUD reads of the player this tick. Eating, shelters (doors,
    /// hibernation) and the rain cycle are not ported: no food, never in a shelter's
    /// sleep pose or ready to win, no rain danger.
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
            room: room.map(|r| r.name.clone()),
            // Broken shelters are not read: every shelter is intact.
            in_intact_shelter: shelter,
            force_sleep_pose: shelter && self.in_force_sleep_pose(player),
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

    /// The map button as the game-over prompt reads it (the default keyboard binding).
    #[must_use]
    pub fn map_button(&self) -> bool {
        self.sim.player().is_some_and(|p| p.input_history()[0].map)
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
        self.camera_room()
            .scripts
            .iter()
            .map(|s| match s {
                RoomScript::SuC04StartUp(s) => s,
            })
            .next()
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

    /// The camera move since the last call.
    pub fn take_camera_move(&mut self) -> Option<CameraMove> {
        self.sim.take_camera_move()
    }

    /// Replaces the player with a fresh one in the camera's room (continuing the random
    /// sequence).
    pub fn respawn(&mut self) {
        self.sim.respawn(&mut self.graphics);
    }

    /// One 40 Hz simulation tick ([`RainWorldGame::tick`]).
    pub fn tick(&mut self, input: Input) {
        let was_in_shortcut = self.sim.player().is_some_and(Player::in_shortcut);
        self.sim.tick(input, &mut self.graphics);
        let in_shortcut = self.sim.player().is_some_and(Player::in_shortcut);
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

    /// The slugcat's sprites at `alpha` between ticks, eyes in `face_color` (the room
    /// palette's `blackColor`); `None` once the player is destroyed or elsewhere.
    pub fn draw_list(&mut self, alpha: f32, face_color: Color) -> Option<DrawList> {
        let player = self
            .sim
            .player()
            .filter(|_| self.sim.player_in_camera_room())?;
        Some(
            self.graphics
                .module
                .as_mut()?
                .draw(player, alpha, face_color),
        )
    }

    /// Overseer `id`'s graphics module, while its room is viewed.
    #[cfg(test)]
    #[must_use]
    pub fn overseer_graphics(&self, id: EntityId) -> Option<&OverseerGraphics> {
        self.graphics.overseers.iter().find(|g| g.id == id)
    }

    /// The overseers' sprites in the camera's room at `alpha` between ticks, oldest
    /// first (the order their sprites joined the containers), in the room palette's
    /// earth colour.
    pub fn overseer_draw_lists(&mut self, alpha: f32, earth: Color) -> Vec<DrawList> {
        let world = self.sim.world();
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
                lists.push(g.draw(&view, earth, alpha, &mut graphics.cosmetic));
            }
        }
        lists
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
        #[cfg(test)]
        for (h, g) in &mut graphics.extra_holograms {
            let frame = HologramFrame {
                t: alpha,
                cam_pos,
                screen_size,
                head: h.pos,
            };
            lists.push(g.draw(h, &frame, &mut graphics.cosmetic));
        }
        lists
    }

    /// Draws `hologram` in the camera's room from now on, its parts' colours lerped
    /// toward `base` for `updates` updates.
    #[cfg(test)]
    pub fn show_extra_hologram(&mut self, hologram: OverseerHologram, base: Color, updates: usize) {
        let mut g = HologramGraphics::new(&hologram);
        for _ in 0..updates {
            g.update(&hologram, base);
        }
        self.graphics.extra_holograms.push((hologram, g));
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

    /// Whether the player is dead and frozen.
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

    /// The shortcut tile the player is travelling through in the camera's room, if any.
    #[must_use]
    pub fn vessel_tile(&self) -> Option<(i32, i32)> {
        self.shown_player().and_then(Player::shortcut_vessel_tile)
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
            in_shortcut_at: player
                .shortcut_vessel_tile()
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
                p.shortcut_vessel_tile()
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

    pub fn set_food_in_stomach(&mut self, food: i32) {
        self.sim.set_food_in_stomach(food);
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
    }

    #[test]
    fn player_starts_on_all_fours_and_falls() {
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        assert!(!game.player().is_standing());
        let before = game.head_y();
        for _ in 0..5 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.ticks(), 5);
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
    fn dead_player_freezes_until_respawn() {
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
        game.sim.kill_player();
        let frozen = game.body().expect("body").clone();
        for _ in 0..10 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.body(), Some(&frozen));
        game.respawn();
        assert!(!game.player_dead());
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
}
