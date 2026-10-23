//! One realized overseer at work: `Overseer.Update`, `OverseerAI.Update` with its
//! tutorial module, placing it in a room (`PlaceInRoom`, `HardSetTile`) and its zips
//! in and out. The overseer's parts are taken out of the world while it runs
//! ([`OverseerUpdate`]); what it does to the rest of the game comes back as
//! [`OverseerEffect`]s.
//!
//! Mirrors: Overseer.cs, OverseerAI.cs, OverseerTutorialBehavior.cs

use rw_assets::{AbstractObjectType, CreatureType, Room};
use rw_random::UnityRandom;

use super::abstract_ai::{AbstractStep, GuideContext, Parent, TUTORIAL_ROOMS};
use super::hologram::{ControlPreset, HologramKind, HologramMessage, PointerKind};
use super::{
    CommunicationModule, OverseerAbstractAi, OverseerAi, OverseerBody, OverseerMode, PlayerConcern,
    PlayerGuideState, TutorialBehavior, TutorialMessage,
};
use crate::camera::CameraView;
use crate::creature::{AbstractCreatureAi, CreatureId, EntityId, WorldCoordinate};
use crate::item::{Entity, ObjectId, WeaponMode};
use crate::room_ext::RoomExt;
use crate::trigger::{ForcedDirectionView, InputInstructionView, InstructionViewKind};
use crate::world::{AiMap, EIGHT_DIRECTIONS, RainCycle, World};
use crate::{
    CosmeticKind, FloatExt, MyceliaCx, MyceliumEnv, MyceliumPool, OverseerEffectSprite,
    PlayerObserver, SoundId, Sounds, Spark, Vec2,
};

/// `Custom.eightDirectionsAndZero`'s first eight (the zero offset first).
const EIGHT_AND_ZERO: [(i32, i32); 8] = [
    (0, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
];

/// `scaredDistance`.
const SCARED_DISTANCE: f32 = 130.0;

/// `OverseerGraphics.MainColor`'s choice (base game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverseerColor {
    /// The player guide and Five Pebbles' (owner 1 or 3): (1, 0.8, 0.3).
    Yellow,
    /// Owner 2 (Shoreline's): (0, 1, 0).
    Green,
    /// Everyone else: (114, 230, 196) / 255.
    Wild,
}

impl OverseerColor {
    /// `MainColor`'s RGB.
    #[must_use]
    pub fn rgb(self) -> [f32; 3] {
        match self {
            Self::Yellow => [1.0, 0.8, 0.3],
            Self::Green => [0.0, 1.0, 0.0],
            Self::Wild => [114.0 / 255.0, 230.0 / 255.0, 196.0 / 255.0],
        }
    }
}

/// What the graphics module reads of a realized overseer.
#[derive(Debug, Clone, Copy)]
pub struct OverseerView<'a> {
    pub id: EntityId,
    pub body: &'a OverseerBody,
    /// `AI.CosmeticLookAt`.
    pub cosmetic_look_at: Vec2,
    /// `AI.bringUpLens`.
    pub bring_up_lens: f32,
    pub color: OverseerColor,
    /// The conversation partner's ID, when it has one.
    pub partner: Option<EntityId>,
    /// `neuronSystem`.
    pub neuron_system: Option<i32>,
}

impl<'a> OverseerView<'a> {
    #[must_use]
    pub fn new(
        id: EntityId,
        body: &'a OverseerBody,
        ai: &OverseerAi,
        color: OverseerColor,
        partner: Option<EntityId>,
    ) -> Self {
        Self {
            id,
            body,
            cosmetic_look_at: ai.cosmetic_look_at(body.chunk().pos),
            bring_up_lens: ai.bring_up_lens,
            color,
            partner,
            neuron_system: body.neuron_system,
        }
    }
}

/// The player as a realized overseer reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerView {
    /// `realizedCreature.room` is the overseer's room (not in a shortcut there).
    pub in_room: bool,
    /// `mainBodyChunk` (`DangerPos`) now and last tick.
    pub pos: Vec2,
    pub last_pos: Vec2,
    /// `bodyChunks[1]`.
    pub hips: Vec2,
    pub dead: bool,
    /// `Consious`: alive and not stunned.
    pub conscious: bool,
    pub food: i32,
    /// `input[0].jmp`, `input[1].jmp`.
    pub jump: [bool; 2],
    pub entering_shortcut: Option<(i32, i32)>,
    /// `room == null`: travelling a shortcut.
    pub in_shortcut: bool,
    pub standing: bool,
    pub animation: crate::Animation,
    /// `bodyChunks[1].ContactPoint.y`.
    pub hips_contact_y: i8,
    /// `input[0].x`, `input[0].y`.
    pub input_x: i8,
    pub input_y: i8,
    /// `input[0].pckp`, `input[0].thrw`.
    pub pickup: bool,
    pub throw: bool,
    pub eat_counter: i32,
}

/// An `IPlayerEdible` in the room's `physicalObjects` (a realized fly in the room), in
/// list order, as `OverseerTutorialBehavior` reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdibleView {
    /// `firstChunk.pos`.
    pub pos: Vec2,
    /// Nothing holds it and it is not burrowing (`foodArrows` point at these).
    pub free: bool,
    /// `grabbedBy[0].grabber` is the player.
    pub held_by_player: bool,
}

/// The story numbers the tutorial reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StoryStats {
    pub cycle: i32,
    pub food_to_hibernate: i32,
    pub max_food: i32,
    /// `deathPersistentSaveData.karmaCap`.
    pub karma_cap: i32,
    /// Monk (`SlugcatStats.Name.Yellow`).
    pub yellow: bool,
    /// `deathPersistentSaveData.howWellIsPlayerDoing`.
    pub how_well_is_player_doing: f32,
    /// `miscWorldSaveData.EverMetMoon`.
    pub ever_met_moon: bool,
}

/// What an overseer does to the rest of the game.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OverseerEffect {
    /// `TutorialText` (only with a HUD).
    Message(TutorialMessage),
    /// `hud.rainMeter.halfTimeBlink = 0`.
    HalfTimeBlinkReset,
    /// `CreatureTeleportOutOfRoom(this, pos, destination)`.
    TeleportOut(WorldCoordinate),
    /// Another overseer is pulled into a conversation (`conversationPartner = me`,
    /// `SwitchModes(Conversing)` from Watching, then its drawn `conversationDelay`).
    StartConversation {
        other: CreatureId,
        with: CreatureId,
        delay: i32,
    },
    /// `SwitchModes(Watching)` of a conversing partner, with its drawn delay.
    EndConversation { other: CreatureId, delay: i32 },
    /// The tutorial kills the living green lizards of a room (`LizardState.Die`).
    KillGreenLizards(i32),
    /// The super-jump lesson holds the crouch charge down: `player.superLaunchJump = 0`.
    ResetSuperLaunchJump,
    /// `Destroy()`: the dead overseer has died away and leaves the room.
    Destroy,
    /// `room.AddObject` of a cosmetic sprite (a dying overseer's effects and sparks).
    Cosmetic(CosmeticKind),
    /// `markInstructionAsFollowed = false` on input instruction object `id`.
    InstructionFollowed(u64),
    /// `completed = true` on input instruction object `id`.
    InstructionCompleted(u64),
}

/// What a realized overseer reads and writes of the game for one call.
pub(crate) struct OverseerTick<'a> {
    pub world: &'a World,
    pub room_index: i32,
    pub room: &'a Room,
    pub ai_map: &'a AiMap,
    pub rng: &'a mut UnityRandom,
    pub sounds: &'a mut Sounds,
    pub observer: &'a mut dyn PlayerObserver,
    pub guide: &'a mut PlayerGuideState,
    /// `options.controls[0].GetActivePreset()`, which instruction picture is built.
    pub control_preset: ControlPreset,
    /// The next hologram id.
    pub hologram_ids: &'a mut u64,
    /// `Players[0]`.
    pub player_id: CreatureId,
    /// The realized player, if any.
    pub player: Option<PlayerView>,
    /// The edible objects in the overseer's room.
    pub edibles: Vec<EdibleView>,
    pub player_karma: i32,
    pub camera: CameraView,
    pub story: StoryStats,
    /// `world.rainCycle`, written back afterwards.
    pub rain: RainCycle,
    /// Camera 0 has a HUD.
    pub hud: bool,
    /// Whether the camera shows this room (`BeingViewed`).
    pub viewed: bool,
    pub effects: Vec<OverseerEffect>,
    /// The rooms' input instruction triggers (`communication.inputInstruction`).
    pub instructions: Vec<InputInstructionView>,
    /// The rooms' `ReliableIggyDirection`s (`communication.forcedDirectionToGive`).
    pub forced_directions: Vec<ForcedDirectionView>,
    /// The world's mycelia, lent to the graphics module's strands.
    pub pool: &'a mut MyceliumPool,
    /// `room.camerasChangedTick`.
    pub cameras_changed_tick: i32,
    /// The room's `CoralNeuronSystem`'s `Frozen`, if it has one.
    pub coral_frozen: Option<bool>,
}

impl<'a> OverseerTick<'a> {
    /// What a strand reads of the room.
    pub(crate) fn mycelium_env(&self) -> MyceliumEnv<'a> {
        MyceliumEnv {
            room: self.room,
            room_index: self.room_index,
            ai_map: Some(self.ai_map),
            camera: self.camera,
            cameras_changed_tick: self.cameras_changed_tick,
            frozen: self.coral_frozen,
        }
    }
}

/// What `RealizedCreatureInterest`, `HoverScoreOfTile` and the mode checks read of a
/// realized creature.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RealizedInfo {
    kind: CreatureType,
    small: bool,
    body_size: f32,
    dead: bool,
    in_room: bool,
    /// `DangerPos` (the main chunk).
    pos: Vec2,
    last_pos: Vec2,
}

/// A realized overseer's parts while it runs.
pub(crate) struct OverseerUpdate<'a, 'b> {
    pub me: CreatureId,
    pub id: EntityId,
    /// `abstractCreature.pos`, written back afterwards.
    pub pos: WorldCoordinate,
    pub ignore_cycle: bool,
    pub body: &'a mut OverseerBody,
    pub ai: &'a mut OverseerAi,
    pub abs: &'a mut OverseerAbstractAi,
    pub base: &'a mut AbstractCreatureAi,
    pub cx: &'a mut OverseerTick<'b>,
}

impl OverseerUpdate<'_, '_> {
    fn room_size(&self) -> (i32, i32) {
        (self.cx.room.width(), self.cx.room.height())
    }

    fn mid(tile: (i32, i32)) -> Vec2 {
        Vec2::middle_of_tile(tile)
    }

    /// `IntVector2.FloatDist`.
    fn float_dist(a: (i32, i32), b: (i32, i32)) -> f32 {
        Vec2::new(a.0 as f32, a.1 as f32).distance(Vec2::new(b.0 as f32, b.1 as f32))
    }

    pub fn color(&self) -> OverseerColor {
        self.abs.color()
    }

    fn partner_id(&self) -> Option<EntityId> {
        self.body
            .conversation_partner
            .map(|p| self.cx.world.creature(p).id)
    }

    /// The graphics module's `Reset`.
    fn graphics_reset(&mut self) {
        let (color, partner) = (self.color(), self.partner_id());
        let view = OverseerView::new(self.id, self.body, self.ai, color, partner);
        let cx = &mut *self.cx;
        let env = cx.mycelium_env();
        let mut mx = MyceliaCx {
            pool: &mut *cx.pool,
            env,
        };
        cx.observer.overseer_reset(&view, &mut mx, cx.rng);
    }

    /// The realized creature `id` in the game, as far as the overseer reads it.
    fn realized(&self, id: CreatureId) -> Option<RealizedInfo> {
        let c = self.cx.world.creature(id);
        let template = c.template;
        let base = RealizedInfo {
            kind: template.kind,
            small: template.small_creature,
            body_size: template.body_size,
            dead: !c.state.alive,
            in_room: false,
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
        };
        if id == self.me {
            let chunk = self.body.chunk();
            return Some(RealizedInfo {
                in_room: true,
                pos: chunk.pos,
                last_pos: chunk.last_pos,
                ..base
            });
        }
        if id == self.cx.player_id {
            let p = self.cx.player?;
            return Some(RealizedInfo {
                dead: p.dead,
                in_room: p.in_room,
                pos: p.pos,
                last_pos: p.last_pos,
                ..base
            });
        }
        if let Some(f) = c.fly() {
            return Some(RealizedInfo {
                dead: f.dead,
                in_room: f.room == Some(self.cx.room_index),
                pos: f.pos(),
                last_pos: f.chunk().last_pos,
                ..base
            });
        }
        if let Some(l) = c.lizard() {
            return Some(RealizedInfo {
                dead: l.dead,
                in_room: l.room == Some(self.cx.room_index),
                pos: l.head().pos,
                last_pos: l.head().last_pos,
                ..base
            });
        }
        let body = c.overseer_body()?;
        Some(RealizedInfo {
            in_room: body.in_room && c.pos.room == self.cx.room_index,
            pos: body.chunk().pos,
            last_pos: body.chunk().last_pos,
            ..base
        })
    }

    /// Another realized overseer's body (self excluded).
    fn other_overseer(&self, id: CreatureId) -> Option<&OverseerBody> {
        if id == self.me {
            return None;
        }
        self.cx.world.creature(id).overseer_body()
    }

    // ---- Overseer ----

    /// `Overseer.Update` (and the `Creature`/`PhysicalObject` parts before it).
    pub fn update(&mut self) {
        if self.body.stun > 0 {
            self.body.stun -= 1;
        }
        self.body.drop_slated_holograms();
        let water = self
            .cx
            .world
            .realized(self.cx.room_index)
            .and_then(|r| r.water.as_ref());
        self.body.body.update_with(self.cx.room, water, &mut ());
        self.pos.tile = self.body.chunk().pos.tile();
        self.body.can_be_hit_by_weapons = self.body.extended > 0.2;
        self.body.update_appendage();
        self.body.update_appendage();
        let b = &mut *self.body;
        b.last_root_pos = b.root_pos;
        b.last_root_dir = b.root_dir;
        b.last_extended = b.extended;
        b.in_shortcut = b.extended <= 0.2;
        b.last_dying = b.dying;
        if b.dead {
            self.dying_update();
        }
        let b = &mut *self.body;
        if b.conversation_delay > 0 {
            b.conversation_delay -= 1;
        }
        b.stationary_counter += 1;
        let hover_mid = Self::mid(b.hover_tile);
        let (rx, ry) = (b.root_tile.0 as f32 * 20.0, b.root_tile.1 as f32 * 20.0);
        let restricted = Vec2::new(
            hover_mid.x.clamp(rx, rx + 20.0),
            hover_mid.y.clamp(ry, ry + 20.0),
        );
        b.root_pos = b.root_pos.lerp(restricted, 0.2);
        let num = if b.mode == OverseerMode::Conversing {
            100.0
        } else {
            50.0
        } * b.size;
        let size = b.size;
        let pos = b.chunk().pos;
        if !hover_mid.dist_less(pos, num) {
            let v = hover_mid.direction_to(pos) * (num - hover_mid.distance(pos));
            let c = b.chunk_mut();
            c.vel += v * 0.15;
            c.pos += v * 0.15;
        }
        let pos = b.chunk().pos;
        let pull = hover_mid.distance(pos).inverse_lerp(0.0, num) * 0.8 * size;
        b.chunk_mut().vel += pos.direction_to(hover_mid) * pull;
        let pos = b.chunk().pos;
        if !b.root_pos.dist_less(pos, num) {
            let v = b.root_pos.direction_to(pos) * (num - b.root_pos.distance(pos));
            let c = b.chunk_mut();
            c.vel += v * 0.15;
            c.pos += v * 0.15;
        }
        let mut open = Vec2::ZERO;
        for &(dx, dy) in &EIGHT_DIRECTIONS {
            if !self
                .cx
                .room
                .is_solid_tile(b.root_tile.0 + dx, b.root_tile.1 + dy)
            {
                open += Vec2::new(dx as f32, dy as f32).normalized();
            }
        }
        let pos = b.chunk().pos;
        let toward = b.root_pos.direction_to((hover_mid + pos) / 2.0);
        b.root_dir = if open.length() > 0.0 {
            b.root_dir.slerp((toward + open * 5.0).normalized(), 0.2)
        } else {
            b.root_dir.slerp(toward, 0.2)
        };
        self.pull_toward_attention();
        self.ai_update();
        if self.body.hologram.is_some() && self.body.mode == OverseerMode::Watching {
            self.switch_modes(OverseerMode::Projecting);
        } else if self.body.hologram.is_none() && self.body.mode == OverseerMode::Projecting {
            self.switch_modes(OverseerMode::Watching);
        }
        self.mode_update();
        if matches!(
            self.body.mode,
            OverseerMode::Zipping | OverseerMode::Emerging
        ) {
            let progs = &mut self.body.zip_progs;
            for i in (1..progs.len()).rev() {
                progs[i] = progs[i - 1];
            }
            progs[0] = (progs[0] + self.body.zip_prog_speed).min(1.0);
        }
    }

    /// `Update`'s dead branch: the body shakes (four draws), the first tick plays
    /// `Overseer_Death`, it stays extended and dies away over six ticks, then is
    /// destroyed. Not ported: the `ShockWave` a viewed death shows and the
    /// `OverseerCarcass` it leaves (docs/divergences/ D14).
    fn dying_update(&mut self) {
        let dying = self.body.dying;
        let dir = Vec2::random_unit(self.cx.rng);
        let push = dir * (self.cx.rng.value() * 4.0 * dying);
        self.body.chunk_mut().vel += push;
        let dir = Vec2::random_unit(self.cx.rng);
        let push = dir * (self.cx.rng.value() * 4.0 * dying);
        self.body.chunk_mut().pos += push;
        if self.body.last_dying == 0.0 {
            let pos = self.body.chunk().pos;
            self.cx.sounds.play_on_object(
                SoundId::OverseerDeath,
                pos,
                false,
                1.0,
                1.0,
                self.cx.rng,
            );
        }
        let b = &mut *self.body;
        b.extended = 1.0;
        b.dying = (b.dying + 1.0 / 6.0).min(1.0);
        if b.dying >= 1.0 {
            self.cx.effects.push(OverseerEffect::Destroy);
        }
    }

    /// `OverseerGraphics.Update`'s dying part, which no draw of that update comes
    /// before: in view (400 px margin), ten `OverseerEffect`s and three sparks of its
    /// colour along the body (the game places them on the drawn body, the port between
    /// root and eye).
    fn dying_graphics(&mut self) {
        let pos = self.body.chunk().pos;
        if self.body.dying <= 0.0
            || !self
                .cx
                .camera
                .viewed(self.cx.room_index, self.cx.room, pos, 400.0)
        {
            return;
        }
        let [r, g, b] = self.color().rgb();
        let dying = self.body.dying;
        for _ in 0..10 {
            let f = self.cx.rng.value();
            let at = self.segment_pos(f);
            let dir = Vec2::random_unit(self.cx.rng);
            let vel = dir * (self.cx.rng.value() * 0.1);
            let rad = 80.0.lerp(10.0, dying);
            let fac = 1.0.lerp(0.1, dying);
            let e = OverseerEffectSprite::new(at, vel, [r, g, b], rad, fac, self.cx.rng);
            self.cx
                .effects
                .push(OverseerEffect::Cosmetic(CosmeticKind::OverseerEffect(e)));
        }
        for _ in 0..3 {
            let f = self.cx.rng.value();
            let at = self.segment_pos(f);
            let dir = Vec2::random_unit(self.cx.rng);
            let vel = self.body.chunk().vel * 0.5 + dir * (14.0 * self.cx.rng.value());
            let spark = Spark::new(at, vel, [r, g, b, 1.0], 14, 21, self.cx.rng);
            self.cx
                .effects
                .push(OverseerEffect::Cosmetic(CosmeticKind::Spark(spark)));
        }
    }

    /// A point along the body from the eye (0) to the root (1).
    fn segment_pos(&self, f: f32) -> Vec2 {
        self.body.chunk().pos.lerp(self.body.root_pos, f)
    }

    /// `Update`, then the graphics module's (`Room.Update` runs each object's right
    /// after it).
    pub fn update_with_graphics(&mut self) {
        self.update();
        self.dying_graphics();
        let (color, partner) = (self.color(), self.partner_id());
        let view = OverseerView::new(self.id, self.body, self.ai, color, partner);
        let cx = &mut *self.cx;
        let env = cx.mycelium_env();
        let mut mx = MyceliaCx {
            pool: &mut *cx.pool,
            env,
        };
        cx.observer.overseer_updated(&view, &mut mx, cx.rng);
    }

    /// The conversation pull, or the turn toward `AI.lookAt`.
    fn pull_toward_attention(&mut self) {
        let pos = self.body.chunk().pos;
        let partner = self
            .body
            .conversation_partner
            .filter(|_| self.body.mode == OverseerMode::Conversing);
        if let Some(partner) = partner {
            let other = if partner == self.me {
                pos
            } else {
                self.other_overseer(partner).map_or(pos, |b| b.chunk().pos)
            };
            let mut mid = (pos + other) / 2.0;
            mid += mid.direction_to(pos) * 10.0;
            let c = self.body.chunk_mut();
            c.vel *= 0.6;
            c.vel += (mid - pos) * 0.1;
        } else {
            let look = self.ai.look_at;
            let f = pos.distance(look).lerp_map(30.0, 150.0, -0.8, 0.8) * self.body.size;
            self.body.chunk_mut().vel += pos.direction_to(look) * f;
        }
    }

    /// The mode machine after the AI.
    fn mode_update(&mut self) {
        match self.body.mode {
            OverseerMode::Withdrawing => {
                self.body.extended = (self.body.extended - 0.25).max(0.0);
                if self.body.extended <= 0.0 {
                    let next = self.body.after_withdraw_mode;
                    self.switch_modes(next);
                }
            }
            OverseerMode::Zipping => {
                if self.body.zip_progs[1] <= 0.5 && self.body.zip_progs[0] > 0.5 {
                    self.body.zip_mesh_direction = true;
                }
                if self.body.zip_progs[0] >= 1.0 {
                    if self.body.leave_room_on_zip_completion {
                        self.body.destroy_hologram();
                        self.body.leave_room_on_zip_completion = false;
                        self.cx
                            .effects
                            .push(OverseerEffect::TeleportOut(self.base.destination));
                    } else {
                        self.switch_modes(OverseerMode::Emerging);
                    }
                }
                self.body.stationary_counter = 0;
            }
            OverseerMode::Emerging => {
                self.body.extended = (self.body.extended + 0.25).min(1.0);
                let hover = Self::mid(self.body.hover_tile);
                let target = hover + hover.direction_to(self.ai.look_at) * 30.0;
                let pos = self.body.chunk().pos;
                self.body.chunk_mut().vel += (target - pos) * 0.05;
                if self.body.extended >= 1.0 {
                    self.switch_modes(OverseerMode::Watching);
                }
            }
            OverseerMode::Conversing if self.body.conversation_delay == 0 => {
                if let Some(partner) = self.body.conversation_partner {
                    self.end_partner_conversation(partner);
                }
                self.switch_modes(OverseerMode::Watching);
            }
            _ => {}
        }
    }

    /// `conversationPartner.SwitchModes(Watching)`: the partner (conversing) draws its
    /// new delay.
    fn end_partner_conversation(&mut self, partner: CreatureId) {
        if partner == self.me {
            self.switch_modes(OverseerMode::Watching);
            return;
        }
        let conversing = self
            .other_overseer(partner)
            .is_some_and(|b| b.mode == OverseerMode::Conversing);
        let delay = if conversing {
            self.cx.rng.range_i32(50, 120)
        } else {
            -1
        };
        self.cx.effects.push(OverseerEffect::EndConversation {
            other: partner,
            delay,
        });
    }

    /// `SwitchModes`.
    pub fn switch_modes(&mut self, mode: OverseerMode) {
        if self.body.dead {
            return;
        }
        let b = &mut *self.body;
        if b.mode == OverseerMode::Conversing {
            b.conversation_delay = self.cx.rng.range_i32(50, 120);
            b.last_conversation_partner = b.conversation_partner;
            b.conversation_partner = None;
        }
        b.mode = mode;
        if mode == OverseerMode::Withdrawing {
            b.zip_progs = [0.0; 5];
        }
        match mode {
            OverseerMode::Withdrawing => {
                if b.after_withdraw_mode == OverseerMode::SittingInWall {
                    let solid: Vec<(i32, i32)> = EIGHT_AND_ZERO
                        .iter()
                        .map(|d| (b.root_tile.0 + d.0, b.root_tile.1 + d.1))
                        .filter(|&(x, y)| self.cx.room.is_solid_tile(x, y))
                        .collect();
                    b.zip_path = vec![b.root_tile];
                    if !solid.is_empty() {
                        let len = i32::try_from(solid.len()).unwrap_or(i32::MAX);
                        let pick = self.cx.rng.range_i32(0, len);
                        b.zip_path.push(solid[usize::try_from(pick).unwrap_or(0)]);
                        b.zip_path.push(b.root_tile);
                    }
                    b.zip_path_count = b.zip_path.len();
                }
                let pos = b.chunk().pos;
                self.play_sound(SoundId::OverseerWithdraw, pos);
                self.body.zip_mesh_direction = false;
            }
            OverseerMode::Zipping => {
                b.zip_prog_speed = 3.0 / b.zip_path_count as f32;
                let start = b.zip_path[0];
                let size = self.room_size();
                self.ai.reset_zip_matrix(start, size);
            }
            OverseerMode::Emerging => {
                let pos = b.chunk().pos;
                self.play_sound(SoundId::OverseerEmerge, pos);
                let b = &mut *self.body;
                b.root_tile = b.zip_path[0];
                b.hover_tile = b.next_hover_tile;
                b.root_pos = Self::mid(b.root_tile);
                b.last_root_pos = b.root_pos;
                b.root_dir = Vec2::ZERO;
                b.last_root_dir = Vec2::ZERO;
                let root = b.root_pos;
                b.chunk_mut().hard_set_position(root);
                let (color, partner) = (self.color(), self.partner_id());
                let view = OverseerView::new(self.id, self.body, self.ai, color, partner);
                let cx = &mut *self.cx;
                let env = cx.mycelium_env();
                let mut mx = MyceliaCx {
                    pool: &mut *cx.pool,
                    env,
                };
                cx.observer.overseer_emerge(&view, &mut mx, cx.rng);
            }
            OverseerMode::SittingInWall => b.zip_prog_speed = 0.0,
            _ => {}
        }
    }

    /// `OverseerAI.FlyingWeapon`: a thrown weapon within 600 px in sight. Watching and
    /// still for a while, with the weapon heading at it, it may withdraw into the wall
    /// (one draw); then the guide, when the weapon was the player's and passed closest
    /// to it, likes the player less (a spear 0.1, a rock 0.05) and stays away longer.
    /// It looks at the nearest such weapon.
    pub fn flying_weapon(&mut self, weapon: ObjectId) {
        let Some(w) = self.cx.world.object(weapon).realized.as_deref() else {
            return;
        };
        let c = w.chunk();
        let (pos, vel) = (c.pos, c.vel);
        let me = self.body.chunk().pos;
        if !me.dist_less(pos, 600.0) || !self.cx.room.visual_contact(me, pos) {
            return;
        }
        let spear = w.spear().is_some();
        let thrown_by = w.thrown_by;
        let closest = w.thrown_closest_to_creature;
        let stationary = (self.body.stationary_counter as f32).inverse_lerp(10.0, 40.0);
        if self.cx.rng.value() < stationary * 0.75
            && self.body.mode == OverseerMode::Watching
            && me.dist_less(pos + vel.normalized() * 100.0, 100.0)
        {
            self.body.after_withdraw_mode = OverseerMode::SittingInWall;
            self.switch_modes(OverseerMode::Withdrawing);
            if self.abs.is_player_guide
                && let Some(by) = thrown_by
                && closest.is_none_or(|c| c == self.me)
                && self.cx.world.creature(by).kind() == CreatureType::Slugcat
            {
                self.cx
                    .guide
                    .influence_like(if spear { -0.1 } else { -0.05 });
                self.cx.guide.increase_like_on_save = false;
                if self.abs.go_to_player {
                    self.abs.player_guide_counter /= if spear { 5 } else { 2 };
                }
            }
        }
        let nearer = self.ai.look_at_flying_weapon.is_none_or(|old| {
            self.cx
                .world
                .object(old)
                .realized
                .as_deref()
                .is_none_or(|o| me.distance(pos) < me.distance(o.chunk().pos))
        });
        if nearer {
            self.ai.look_at_flying_weapon = Some(weapon);
        }
    }

    /// `Overseer.HitByWeapon`: the guide hit by the player's weapon likes the player
    /// 0.1 less, stays away longer, and below -0.5 goes away (one draw).
    pub fn hit_by_weapon(&mut self, by_slugcat: bool) {
        if !self.abs.is_player_guide || !by_slugcat {
            return;
        }
        self.cx.guide.influence_like(-0.1);
        self.cx.guide.increase_like_on_save = false;
        if self.abs.go_to_player {
            self.abs.player_guide_counter /= 4;
        }
        if self.cx.guide.likes_player < -0.5 {
            let time = self.cx.rng.range_i32(200, 1500);
            self.abs.player_guide_go_away(time);
        }
    }

    /// `room.PlaySound(id, mainBodyChunk)`: the overseer's movement sounds are
    /// commented out of `sounds.txt`, so they never play or draw.
    fn play_sound(&mut self, id: SoundId, pos: Vec2) {
        self.cx.sounds.play_at(id, pos, 1.0, 1.0, self.cx.rng);
    }

    /// `HardSetTile`.
    pub fn hard_set_tile(&mut self, hover: (i32, i32), root: (i32, i32)) {
        let b = &mut *self.body;
        b.hover_tile = hover;
        b.next_hover_tile = hover;
        b.chunk_mut().hard_set_position(Self::mid(hover));
        b.root_tile = root;
        b.root_pos = Self::mid(root);
        b.last_root_pos = b.root_pos;
        b.zip_path = vec![root];
        b.zip_path_count = 1;
        let size = self.room_size();
        self.ai.reset_zip_matrix(root, size);
        self.graphics_reset();
    }

    /// `Creature.PlaceInRoom` then `Overseer.PlaceInRoom`: added to the room (graphics
    /// made when viewed), the chunk on the abstract tile with a random offset, `NewRoom`
    /// (the AI's, then a graphics reset), then a random open tile next to a wall.
    pub fn place_in_room(&mut self) {
        self.body.in_room = true;
        let (color, partner) = (self.color(), self.partner_id());
        let view = OverseerView::new(self.id, self.body, self.ai, color, partner);
        let cx = &mut *self.cx;
        if cx.viewed {
            let env = cx.mycelium_env();
            let mut mx = MyceliaCx {
                pool: &mut *cx.pool,
                env,
            };
            cx.observer.overseer_added(&view, &mut mx, cx.rng);
        }
        let start = Self::mid(self.pos.tile) + Vec2::random_unit(self.cx.rng);
        self.body.chunk_mut().reset_at(start);
        self.ai_new_room();
        self.graphics_reset();
        self.update_coral_neuron_system();
        let (w, h) = self.room_size();
        let room = self.cx.room;
        for _ in 0..100 {
            let t = (self.cx.rng.range_i32(0, w), self.cx.rng.range_i32(0, h));
            if room.is_solid_tile(t.0, t.1) {
                continue;
            }
            if let Some(d) = EIGHT_AND_ZERO
                .iter()
                .find(|d| room.is_solid_tile(t.0 + d.0, t.1 + d.1))
            {
                self.hard_set_tile(t, (t.0 + d.0, t.1 + d.1));
                return;
            }
        }
        let tile = self.pos.tile;
        if self.pos.tile_defined()
            && let Some(d) = EIGHT_AND_ZERO
                .iter()
                .find(|d| room.is_solid_tile(tile.0 + d.0, tile.1 + d.1))
        {
            self.hard_set_tile(tile, (tile.0 + d.0, tile.1 + d.1));
            return;
        }
        let t = (self.cx.rng.range_i32(0, w), self.cx.rng.range_i32(0, h));
        self.hard_set_tile(t, t);
        self.graphics_reset();
    }

    /// `Overseer.UpdateCoralNeuronSystem` (from `NewRoom`): the system of the room it is
    /// in now, and its graphics module's strands moved to that system's registry.
    fn update_coral_neuron_system(&mut self) {
        let (color, partner) = (self.color(), self.partner_id());
        let view = OverseerView::new(self.id, self.body, self.ai, color, partner);
        let cx = &mut *self.cx;
        let env = cx.mycelium_env();
        let new = env.system();
        let mut mx = MyceliaCx {
            pool: &mut *cx.pool,
            env,
        };
        cx.observer.overseer_new_room(&view, &mut mx, new);
        self.body.neuron_system = new;
    }

    /// `ArtificialIntelligence.NewRoom` and `OverseerAI.NewRoom`.
    fn ai_new_room(&mut self) {
        let room = self.cx.room_index;
        if self.ai.last_room != room {
            self.ai.last_room = room;
            self.ai.time_in_room = 0;
            let dest = self.base.destination;
            if dest.room == room && !dest.tile_defined() {
                // `SetDestination` of a random tile: no path finder takes it.
                let (w, h) = self.room_size();
                self.cx.rng.range_i32(0, w);
                self.cx.rng.range_i32(0, h);
            }
        }
        self.ai.last_look_at_air_positions.clear();
        self.ai.avoid_positions.clear();
        self.ai.slow_look_at = self.ai.look_at;
    }

    /// `TeleportingIntoRoom`: placed, then zipping in from the room it came from.
    pub fn teleport_into_room(&mut self) {
        self.place_in_room();
        let from = self.abs.last_room;
        self.zip_into_room(from);
    }

    /// `FindZipPath`.
    fn find_zip_path(&mut self, new_root: (i32, i32), new_hover: (i32, i32)) {
        if new_root == self.body.root_tile {
            return;
        }
        let size = self.room_size();
        self.body.next_hover_tile = new_hover;
        let mut cur = new_root;
        self.body.zip_path = vec![new_root];
        self.body.zip_path_count = 1;
        for _ in 0..200 {
            let mut best_cost = f32::MAX;
            let mut best = cur;
            for &(dx, dy) in &EIGHT_DIRECTIONS {
                let t = (cur.0 + dx, cur.1 + dy);
                if t.0 < 0 || t.1 < 0 || t.0 >= size.0 || t.1 >= size.1 {
                    continue;
                }
                let cell = self.ai.zip_cell(t, size);
                if cell >= 0.0 && cell < best_cost {
                    best_cost = cell;
                    best = t;
                }
            }
            if cur == best {
                break;
            }
            cur = best;
            self.body.zip_path.push(cur);
            self.body.zip_path_count = self.body.zip_path.len();
            if cur == self.body.root_tile {
                return;
            }
        }
        log::warn!("OVERSEER FAILED PATHING");
        let root = self.body.root_tile;
        self.body.ray_trace_zip_path(new_root, root);
    }

    /// `ZipOutOfRoom`.
    pub fn zip_out_of_room(&mut self, dest: WorldCoordinate) {
        let root = self.body.root_tile;
        self.inter_room_zip_path(root, dest.room, false);
        self.body.leave_room_on_zip_completion = true;
        self.body.after_withdraw_mode = OverseerMode::Zipping;
        self.switch_modes(OverseerMode::Withdrawing);
    }

    /// `ZipIntoRoom`.
    fn zip_into_room(&mut self, from: WorldCoordinate) {
        let root = self.body.root_tile;
        self.inter_room_zip_path(root, from.room, true);
        self.body.zip_progs = [0.0; 5];
        self.body.extended = 0.0;
        self.body.last_extended = 0.0;
        self.switch_modes(OverseerMode::Zipping);
    }

    /// `InterRoomZipPath`: a path through walls toward a random point of the other
    /// room on the region map.
    fn inter_room_zip_path(&mut self, tile: (i32, i32), other: i32, into: bool) {
        let world = self.cx.world;
        if other == self.cx.room_index {
            return;
        }
        let Some(other_room) = world.abstract_room(other) else {
            log::warn!("inter-room zip path toward missing room {other}");
            return;
        };
        let x = other_room.size.0 as f32 * self.cx.rng.value() * 20.0;
        let y = other_room.size.1 as f32 * self.cx.rng.value() * 20.0;
        let goal = world.room_to_world_pos(Vec2::new(x, y), other);
        let mut cur = tile;
        self.body.zip_path = vec![tile];
        self.body.zip_path_count = 1;
        for _ in 0..50 {
            let mut best_cost = f32::MAX;
            let mut best = cur;
            for &(dx, dy) in &EIGHT_DIRECTIONS {
                let t = (cur.0 + dx, cur.1 + dy);
                if self.body.zip_path[..self.body.zip_path_count].contains(&t) {
                    continue;
                }
                let mut cost = if self.cx.room.is_solid_tile(t.0, t.1) {
                    0.0
                } else {
                    100_000.0
                };
                cost += world
                    .room_to_world_pos(Self::mid(t), self.cx.room_index)
                    .distance(goal);
                cost += self.cx.rng.value();
                if cost < best_cost {
                    best_cost = cost;
                    best = t;
                }
            }
            if cur == best || best_cost == f32::MAX {
                break;
            }
            cur = best;
            self.body.zip_path.push(cur);
            self.body.zip_path_count = self.body.zip_path.len();
        }
        if !into {
            self.body.zip_path.reverse();
        }
    }

    // ---- OverseerAI ----

    /// `OverseerAI.Update`.
    fn ai_update(&mut self) {
        self.ai.time_in_room += 1;
        self.communication_update();
        if self.ai.tutorial.is_some() {
            self.tutorial_update();
        }
        let ai = &mut *self.ai;
        ai.slow_look_at = ai
            .slow_look_at
            .move_towards(ai.look_at, 60.0)
            .lerp(ai.look_at, 0.02);
        if self.cx.rng.value() < 0.0125 {
            let v = self.cx.rng.value().mathf_pow(3.0) * 2.0;
            let sign = if self.cx.rng.value() < 0.5 { -1.0 } else { 1.0 };
            self.ai.casual_interest_bonus = v * sign;
        }
        let player_pos = self.cx.player.map(|p| p.pos);
        let me = self.body.chunk().pos;
        match self.body.current_hologram().map(|h| {
            let point_at = h.pointed_creature_pos(self.cx.world).unwrap_or(me);
            h.look_at(player_pos, me, point_at)
        }) {
            Some(look) => self.ai.look_at = look,
            None => self.choose_look_at(),
        }
        if self.cx.rng.value() < 0.025 {
            let dir = Vec2::random_unit(self.cx.rng);
            self.ai.look_at_add = dir * self.cx.rng.value();
        }
        self.update_zip_matrix();
        self.update_temp_hover_position();
        match self.body.mode {
            OverseerMode::Watching | OverseerMode::Projecting => {
                self.watch_creatures();
            }
            OverseerMode::SittingInWall => {
                let disturbed = self
                    .cx
                    .world
                    .room_creatures(self.cx.room_index)
                    .iter()
                    .filter_map(|&c| self.realized(c))
                    .any(|c| {
                        c.kind != CreatureType::Overseer
                            && !c.small
                            && !c.dead
                            && self.body.root_pos.dist_less(c.pos, 200.0)
                    });
                if !disturbed {
                    self.switch_modes(OverseerMode::Emerging);
                }
            }
            OverseerMode::Conversing => self.keep_conversing(),
            _ => {}
        }
        self.abstract_behavior();
        let alive_player = self.cx.world.creature(self.cx.player_id).state.alive;
        if self.abs.is_player_guide
            && self.cx.story.cycle == 0
            && self.ai.tutorial.is_none()
            && alive_player
            && self.cx.world.creature(self.cx.player_id).pos.room == self.cx.room_index
            && self.cx.world.region_name() == Some("SU")
        {
            let name = self
                .cx
                .world
                .abstract_room(self.cx.room_index)
                .map_or("", |r| r.name.as_str());
            if TUTORIAL_ROOMS.contains(&name) {
                log::info!("the player guide starts its tutorial in {name}");
                self.ai.tutorial = Some(TutorialBehavior::default());
            }
        }
        self.abstract_behavior();
    }

    /// The `lookAt` choice: a casually interesting creature, the target, or a point in
    /// the air worth looking at.
    fn choose_look_at(&mut self) {
        let room = self.cx.room_index;
        let casual = self.ai.casual_interest_creature.filter(|&c| {
            self.realized(c).is_some()
                && self.cx.world.creature(c).pos.room == room
                && self.ai.tutorial.is_none()
        });
        let target = self
            .abs
            .target_creature
            .filter(|&t| self.realized(t).is_some_and(|i| i.in_room));
        let casual = casual.filter(|&c| {
            let mine = self.realized_interest(Some(c)) + self.ai.casual_interest_bonus;
            match target {
                Some(t) => mine > self.realized_interest(Some(t)),
                None => mine > 0.0,
            }
        });
        if let Some(w) = self.ai.look_at_flying_weapon {
            let weapon = self.cx.world.object(w).realized.as_deref();
            if let Some(weapon) = weapon {
                self.ai.look_at = weapon.chunk().pos;
            }
            if weapon.is_none_or(|w| w.mode != WeaponMode::Thrown) {
                self.ai.look_at_flying_weapon = None;
            }
            return;
        }
        if let Some(c) = casual.or(target) {
            if let Some(info) = self.realized(c) {
                self.ai.look_at = info.pos;
                self.lens_update(c, info);
            }
            return;
        }
        self.ai.target_stationary = (self.ai.target_stationary - 1.0 / 120.0).max(0.0);
        let test = if self.cx.rng.value() < 0.1 {
            let dir = Vec2::random_unit(self.cx.rng);
            self.body.chunk().pos + dir * (self.cx.rng.value() * 600.0)
        } else {
            let dir = Vec2::random_unit(self.cx.rng);
            self.ai.look_at + dir * (self.cx.rng.value().mathf_pow(3.0) * 600.0)
        };
        if self.look_at_air_pos_score(test) > self.look_at_air_pos_score(self.ai.look_at) {
            self.ai.look_at = test;
            self.ai.look_at_same_air_pos_counter = self.cx.rng.range_i32(30, 130);
        } else {
            self.ai.look_at_same_air_pos_counter -= 1;
            if self.ai.look_at_same_air_pos_counter < 1 {
                let look = self.ai.look_at;
                let list = &mut self.ai.last_look_at_air_positions;
                list.insert(0, look);
                list.truncate(10);
                self.ai.look_at_same_air_pos_counter = self.cx.rng.range_i32(30, 130);
            }
        }
    }

    /// `LookAtAirPosScore`.
    fn look_at_air_pos_score(&self, pos: Vec2) -> f32 {
        if self.cx.room.is_solid_at(pos) {
            return f32::MIN;
        }
        let (x, y) = pos.tile();
        let mut score = self.cx.ai_map.visibility(x, y) as f32;
        for &p in &self.ai.last_look_at_air_positions {
            score -= pos.distance(p).inverse_lerp(300.0, 10.0) * 400.0;
        }
        let (w, h) = self.room_size();
        if x < 0 || y < 0 || x >= w || y > h {
            score -= 1000.0;
        }
        score
    }

    /// `RealizedCreatureInterest` (`float.MinValue` for none or another room).
    fn realized_interest(&self, id: Option<CreatureId>) -> f32 {
        let Some(id) = id else {
            return f32::MIN;
        };
        let Some(info) = self.realized(id) else {
            return f32::MIN;
        };
        if self.cx.world.creature(id).pos.room != self.cx.room_index {
            return f32::MIN;
        }
        let mut num = self.abs.interest(
            self.cx.world,
            self.cx.rng,
            self.cx.player_karma,
            self.me,
            Some(id),
        );
        num += 0.05;
        let pos = self.body.chunk().pos;
        if info.pos.dist_less(pos, 900.0) && self.cx.room.visual_contact(pos, info.pos) {
            num *= info
                .last_pos
                .distance(info.pos)
                .lerp_map(2.0, 14.0, 1.0, 3.0);
        }
        num * self
            .body
            .root_pos
            .distance(info.pos)
            .lerp_map_pow(20.0, 600.0, 4.0, 1.0, 0.2)
    }

    /// `LensUpdate`.
    fn lens_update(&mut self, id: CreatureId, info: RealizedInfo) {
        let ai = &mut *self.ai;
        ai.target_stationary = (ai.target_stationary
            + info
                .last_pos
                .distance(info.pos)
                .lerp_map(1.0, 12.0, 1.0 / 180.0, -0.0125))
        .clamp(0.0, 1.0);
        if !info.pos.dist_less(ai.last_target_pos, 200.0) {
            ai.last_target_pos = info.pos;
            ai.target_stationary = (ai.target_stationary - 0.25).max(0.0);
        } else {
            ai.last_target_pos = ai.last_target_pos.move_towards(info.pos, 2.0);
            let away = ai
                .last_target_pos
                .distance(info.pos)
                .inverse_lerp(0.0, 200.0);
            ai.target_stationary = (ai.target_stationary - away * 0.1).max(0.0);
        }
        let interest = self.realized_interest(Some(id));
        let ai = &mut *self.ai;
        let counter = self.body.stationary_counter as f32;
        let mut num = (ai.target_stationary.mathf_pow(2.0.lerp(0.1, interest))
            + counter.inverse_lerp(40.0, 400.0.lerp(70.0, interest))
            + ai.bring_up_lens.lerp(0.5, 0.2)
            + interest.lerp(ai.random_bring_up_lens_bonus, 0.5))
            / 4.0;
        if self.body.mode == OverseerMode::Zipping
            && self.base.destination.room == self.cx.room_index
        {
            ai.bring_up_lens -= 0.025;
        } else {
            if self.body.mode == OverseerMode::SittingInWall {
                num *= 0.49;
            }
            ai.bring_up_lens = (ai.bring_up_lens + (-1.0).lerp(1.0, num) / 40.0).clamp(0.0, 1.0);
        }
        if self.cx.rng.value() < 1.0 / 160.0 {
            self.ai.random_bring_up_lens_bonus = self.cx.rng.value();
        }
    }

    /// `UpdateZipMatrix`: a few steps of the cost spread from the root.
    fn update_zip_matrix(&mut self) {
        let size = self.room_size();
        if self.ai.zip_matrix.is_none() {
            let root = self.body.root_tile;
            self.ai.reset_zip_matrix(root, size);
        }
        if self.ai.add_to_pathing_list.is_empty() {
            return;
        }
        // `RequesAccesibilityUpdates` answers the ideal count below its budget of 500.
        let ideal = if self.body.mode == OverseerMode::SittingInWall {
            16
        } else {
            8
        };
        for _ in 0..=ideal * 2 {
            if self.ai.add_to_pathing_list.is_empty() {
                continue;
            }
            let tile = self.ai.add_to_pathing_list.remove(0);
            let here = self.ai.zip_cell(tile, size);
            for &(dx, dy) in &EIGHT_DIRECTIONS {
                let t = (tile.0 + dx, tile.1 + dy);
                if self.ai.zip_cell(t, size) != f32::MAX {
                    continue;
                }
                let cost = self.zip_step_cost(t) * Vec2::new(dx as f32, dy as f32).length();
                let total = here + cost * (1.0 + self.cx.rng.value());
                if let Some(m) = &mut self.ai.zip_matrix {
                    m.set(t, total, size);
                }
                self.ai.insert_by_cost(t, total, size);
            }
        }
    }

    /// The zip cost of entering tile `t` (before the diagonal factor).
    fn zip_step_cost(&self, (x, y): (i32, i32)) -> f32 {
        let room = self.cx.room;
        let tile = room.tile(x, y);
        if !tile.is_solid() {
            if tile.any_pole() {
                5.0
            } else if tile.wall_behind() {
                if tile.shortcut == rw_assets::Shortcut::None {
                    100.0
                } else {
                    2.0
                }
            } else {
                1000.0
            }
        } else if tile.shortcut != rw_assets::Shortcut::None {
            0.001
        } else {
            let surrounded = EIGHT_DIRECTIONS[..4]
                .iter()
                .all(|(dx, dy)| room.is_solid_tile(x + dx, y + dy));
            if surrounded { 1.5 } else { 1.0 }
        }
    }

    /// `UpdateTempHoverPosition`: try a random spot; move the hover there, or zip to a
    /// new root.
    fn update_temp_hover_position(&mut self) {
        let size = self.room_size();
        let dir = Vec2::random_unit(self.cx.rng);
        let candidate =
            (Self::mid(self.body.hover_tile) + dir * (self.cx.rng.value() * 400.0)).tile();
        let root = self.find_root_tile(candidate);
        if let Some(r) = root {
            let cell = self.ai.zip_cell(r, size);
            if (0.0..f32::MAX).contains(&cell)
                && self.hover_score(candidate) < self.hover_score(self.ai.temp_hover_tile)
            {
                self.ai.temp_hover_tile = candidate;
            }
        }
        let margin = if self.body.mode == OverseerMode::SittingInWall {
            10.0
        } else if let Some(h) = self.body.current_hologram() {
            if h.overseer_sit_still {
                300.0
            } else if self.ai.add_to_pathing_list.is_empty() {
                30.0
            } else {
                200.0
            }
        } else {
            200.0
        };
        let temp = self.ai.temp_hover_tile;
        let (root_tile, hover, size_f) =
            (self.body.root_tile, self.body.hover_tile, self.body.size);
        if Self::float_dist(temp, root_tile) <= 6.0 * size_f
            && self.hover_score(temp) < self.hover_score(hover)
        {
            self.body.hover_tile = temp;
            return;
        }
        let b = &*self.body;
        let allowed = (b.extended > 0.5 || b.mode == OverseerMode::SittingInWall)
            && b.mode != OverseerMode::Withdrawing
            && Self::float_dist(temp, hover) > 3.0 * size_f
            && self.hover_score(temp) + margin < self.hover_score(hover);
        if !allowed {
            return;
        }
        let Some(root) = self.find_root_tile(temp) else {
            return;
        };
        let cell = self.ai.zip_cell(root, size);
        if cell < 0.0 || cell >= f32::MAX {
            return;
        }
        let hover = self.body.hover_tile;
        self.ai.avoid_positions.push(hover);
        if self.ai.avoid_positions.len() > 10 {
            self.ai.avoid_positions.remove(0);
        }
        self.find_zip_path(root, temp);
        if self.body.mode == OverseerMode::SittingInWall {
            self.switch_modes(OverseerMode::Zipping);
            return;
        }
        self.body.after_withdraw_mode = OverseerMode::Zipping;
        self.switch_modes(OverseerMode::Withdrawing);
    }

    /// `DoIWantToTalkToThisOverSeer`.
    fn wants_to_talk(&self, other: CreatureId, other_body: &OverseerBody) -> bool {
        self.body.conversation_delay == 0
            && other_body.conversation_delay == 0
            && self.body.last_conversation_partner != Some(other)
    }

    /// `HoverScoreOfTile` (lower is better).
    fn hover_score(&mut self, t: (i32, i32)) -> f32 {
        let (w, h) = self.room_size();
        if t.0 < 0 || t.1 < 0 || t.0 >= w || t.1 >= h {
            return f32::MAX;
        }
        let room = self.cx.room;
        if room.is_solid_tile(t.0, t.1) {
            return f32::MAX;
        }
        let map = self.cx.ai_map;
        if map.terrain_proximity(t.0, t.1) > (6.0 * self.body.size) as i32 {
            return f32::MAX;
        }
        let mid = Self::mid(t);
        let mut num = 0.0;
        if let Some(h) = self.body.current_hologram() {
            let display = Self::mid(h.display_tile);
            num += (100.0 - mid.distance(display)).abs() * 2.0;
            if mid.dist_less(display, 500.0) {
                if room.visual_contact_tiles(t, h.display_tile) {
                    num -= 500.0;
                }
            } else {
                num += 1000.0;
            }
            let point_at = h
                .pointed_creature_pos(self.cx.world)
                .unwrap_or(self.body.chunk().pos);
            num = h.influence_hover_score(
                mid,
                num,
                self.cx.player.map(|p| p.pos),
                point_at,
                self.cx.rng,
            );
        } else {
            let look = self.ai.look_at;
            num += (300.0 - mid.distance(look)).abs();
            if mid.dist_less(look, 1000.0) && room.visual_contact_tiles(t, look.tile()) {
                num -= 100.0;
            }
            for &a in &self.ai.avoid_positions {
                let d = Self::float_dist(a, t);
                if d < 15.0 {
                    num += d.lerp_map(10.0, 15.0, 50.0, 0.0);
                }
            }
        }
        for &c in self.cx.world.room_creatures(self.cx.room_index) {
            let Some(info) = self.realized(c).filter(|i| i.in_room) else {
                continue;
            };
            if info.kind != CreatureType::Overseer {
                if !info.small && !info.dead && mid.dist_less(info.pos, SCARED_DISTANCE + 10.0) {
                    return f32::MAX;
                }
                let reach = (info.body_size * 600.0).clamp(60.0, 800.0);
                num += mid
                    .distance(info.pos)
                    .lerp_map(40.0, reach, info.body_size * 100.0, 0.0);
            } else if let Some(other) = self.other_overseer(c)
                && !self.wants_to_talk(c, other)
            {
                num += Self::float_dist(other.hover_tile, t).lerp_map(0.0, 3.0, 250.0, 0.0);
                num += Self::float_dist(other.next_hover_tile, t).lerp_map(0.0, 3.0, 250.0, 0.0);
            }
        }
        if map.narrow_space(t.0, t.1) {
            num += 200.0;
        }
        num -= map.terrain_proximity(t.0, t.1) as f32 * 10.0;
        if room.water_level().is_some_and(|level| t.1 <= level) {
            num += 10000.0;
        }
        match &self.ai.tutorial {
            Some(tutorial) => tutorial.influence_hover_score(t, num),
            None => num,
        }
    }

    /// `FindRootTileForHoverPos`: down the terrain proximity to a Solid tile no other
    /// overseer roots in.
    fn find_root_tile(&self, test: (i32, i32)) -> Option<(i32, i32)> {
        let map = self.cx.ai_map;
        let mut cur = test;
        for _ in 0..15 {
            let mut best_prox = i32::MAX;
            let mut best = cur;
            for &(dx, dy) in &EIGHT_DIRECTIONS {
                let t = (cur.0 + dx, cur.1 + dy);
                let prox = map.terrain_proximity(t.0, t.1);
                if prox < best_prox {
                    best = t;
                    best_prox = prox;
                }
            }
            if cur == best {
                return None;
            }
            cur = best;
            if !self.cx.room.is_solid_tile(cur.0, cur.1) {
                continue;
            }
            let taken = self
                .cx
                .world
                .room_creatures(self.cx.room_index)
                .iter()
                .filter_map(|&c| self.other_overseer(c))
                .any(|b| b.root_tile == cur);
            return (!taken).then_some(cur);
        }
        None
    }

    /// The watching branch: one random creature of the room may scare the overseer
    /// into the wall, catch its casual interest, or (an overseer) start a conversation.
    fn watch_creatures(&mut self) {
        let list = self.cx.world.room_creatures(self.cx.room_index);
        if list.is_empty() {
            return;
        }
        let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
        let pick = list[usize::try_from(self.cx.rng.range_i32(0, len)).unwrap_or(0)];
        let Some(info) = self.realized(pick) else {
            return;
        };
        if info.kind != CreatureType::Overseer {
            if !info.small && !info.dead && self.body.root_pos.dist_less(info.pos, SCARED_DISTANCE)
            {
                self.ai.casual_interest_creature = Some(pick);
                self.body.after_withdraw_mode = OverseerMode::SittingInWall;
                self.switch_modes(OverseerMode::Withdrawing);
            } else if self.abs.target_creature != Some(pick)
                && (self.ai.casual_interest_creature.is_none()
                    || self.realized_interest(Some(pick))
                        > self.realized_interest(self.ai.casual_interest_creature) + 0.1
                    || self.abs.target_creature == self.ai.casual_interest_creature)
                && self.cx.room.visual_contact(self.body.chunk().pos, info.pos)
            {
                self.ai.casual_interest_creature = Some(pick);
            }
            return;
        }
        if self.body.mode == OverseerMode::Projecting || self.body.conversation_delay != 0 {
            return;
        }
        self.maybe_converse(pick);
    }

    /// The conversation check with overseer `other` (possibly itself: the game lets an
    /// overseer pick itself).
    fn maybe_converse(&mut self, other: CreatureId) {
        let (other_root, other_size, other_mode, other_partner, other_delay) = if other == self.me {
            let b = &*self.body;
            (
                b.root_pos,
                b.size,
                b.mode,
                b.conversation_partner,
                b.conversation_delay,
            )
        } else {
            let Some(b) = self.other_overseer(other) else {
                return;
            };
            (
                b.root_pos,
                b.size,
                b.mode,
                b.conversation_partner,
                b.conversation_delay,
            )
        };
        let reach = 70.0 * self.body.size + 70.0 + other_size;
        if !(self.body.root_pos.dist_less(other_root, reach)
            && other_mode == OverseerMode::Watching
            && other_partner.is_none()
            && other_delay == 0
            && self.body.last_conversation_partner != Some(other))
        {
            return;
        }
        self.body.conversation_partner = Some(other);
        if other == self.me {
            self.switch_modes(OverseerMode::Conversing);
            self.switch_modes(OverseerMode::Conversing);
            self.body.conversation_delay = self.cx.rng.range_i32(30, 190);
            self.body.conversation_delay = self.cx.rng.range_i32(30, 190);
            return;
        }
        self.switch_modes(OverseerMode::Conversing);
        self.body.conversation_delay = self.cx.rng.range_i32(30, 190);
        let delay = self.cx.rng.range_i32(30, 190);
        self.cx.effects.push(OverseerEffect::StartConversation {
            other,
            with: self.me,
            delay,
        });
    }

    /// The conversing branch.
    fn keep_conversing(&mut self) {
        let partner = self.body.conversation_partner;
        let ok = partner.is_some_and(|p| {
            if p == self.me {
                return self.body.mode == OverseerMode::Conversing
                    && self.body.conversation_partner == Some(self.me);
            }
            self.other_overseer(p).is_some_and(|b| {
                b.in_room
                    && self.cx.world.creature(p).pos.room == self.cx.room_index
                    && b.mode == OverseerMode::Conversing
                    && b.conversation_partner == Some(self.me)
            })
        });
        match partner.filter(|_| ok) {
            Some(p) => {
                self.ai.look_at = if p == self.me {
                    self.body.chunk().pos
                } else {
                    self.other_overseer(p)
                        .map_or(self.ai.look_at, |b| b.chunk().pos)
                };
            }
            None => self.switch_modes(OverseerMode::Watching),
        }
    }

    /// `creature.abstractAI.AbstractBehavior(1)` while realized.
    fn abstract_behavior(&mut self) {
        let parent = Parent {
            id: self.me,
            pos: self.pos,
            realized: true,
            ignore_cycle: self.ignore_cycle,
            tutorial: self.ai.tutorial.is_some(),
        };
        let mut gcx = GuideContext {
            world: self.cx.world,
            rain: &self.cx.rain,
            rng: &mut *self.cx.rng,
            guide: self.cx.guide,
            player: Some(self.cx.player_id),
            player_karma: self.cx.player_karma,
        };
        let step = self.abs.abstract_behavior(self.base, parent, 1, &mut gcx);
        if let AbstractStep::ZipOut(dest) = step
            && matches!(
                self.body.mode,
                OverseerMode::Watching | OverseerMode::Projecting
            )
        {
            self.zip_out_of_room(dest);
        }
    }

    // ---- OverseerCommunicationModule ----

    /// `LikeOfPlayer`.
    fn like_of_player(&self) -> f32 {
        if self.abs.is_player_guide {
            self.cx.guide.likes_player.inverse_lerp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// `OverseerCommunicationModule.Update`: the player in the guide's room, no
    /// tutorial, and the guide likes the player (or has anger to show).
    pub(crate) fn communication_update(&mut self) {
        let Some(mut comm) = self.ai.communication.take() else {
            return;
        };
        let guide = &*self.cx.guide;
        let anger = guide.angry_with_player() && !guide.displayed_anger();
        let mut hologram = None;
        if self.cx.player.is_some_and(|p| p.in_room)
            && self.ai.tutorial.is_none()
            && (anger || self.like_of_player() >= 0.5)
        {
            if let Some(light) = comm.holo_light {
                let live = self
                    .cx
                    .world
                    .realized(self.cx.room_index)
                    .is_some_and(|r| r.holo_lights.iter().any(|l| l.id == light && !l.slated));
                if !live {
                    comm.holo_light = None;
                }
            }
            hologram = self.reevaluate_concern(&mut comm);
        }
        self.ai.communication = Some(comm);
        if let Some((message, weight)) = hologram {
            self.try_add_hologram(message, weight);
        }
    }

    /// `ReevaluateConcern`: returns the hologram to try.
    fn reevaluate_concern(
        &mut self,
        comm: &mut CommunicationModule,
    ) -> Option<(HologramMessage, f32)> {
        let player = self.cx.player?;
        let room = self.cx.room_index;
        if room != comm.player_room {
            comm.last_player_room = comm.player_room;
            comm.player_room = room;
            let guide = &*self.cx.guide;
            comm.general_hand_holding = 0.5 * guide.hand_holding.mathf_pow(0.2)
                + 0.5
                    * self
                        .cx
                        .story
                        .how_well_is_player_doing
                        .inverse_lerp(0.5, -0.5);
            comm.progression_show_tendency = self.progression_show_tendency();
            comm.showed_image_time = 0;
        }
        comm.current_concern = PlayerConcern::None;
        let dislike = self.like_of_player().inverse_lerp(1.0, 0.5).mathf_pow(6.0);
        comm.current_concern_weight = 1.0.lerp(dislike, comm.general_hand_holding);
        // `grabbedBy` is always empty.
        let open = !player.dead;
        if player.conscious && open {
            let shelter = self.player_shelter_need(comm, player.food);
            if shelter > comm.current_concern_weight {
                comm.current_concern_weight = shelter;
                comm.current_concern = PlayerConcern::Shelter;
            }
            let bats = self.player_bats_need(comm, player.food);
            if bats > comm.current_concern_weight {
                comm.current_concern_weight = bats;
                comm.current_concern = PlayerConcern::Bats;
            }
            if comm.progression_show_tendency > 0.0 {
                let progression = self.player_progression_need(comm, player.food);
                if progression > comm.current_concern_weight {
                    comm.current_concern_weight = progression;
                    comm.current_concern = PlayerConcern::Progression;
                }
            }
        }
        if let Some(id) = comm.input_instruction {
            match self.instruction(id) {
                Some(i) if !i.trigger.completed && !i.slated && i.room == room => {
                    if self
                        .cx
                        .camera
                        .viewed(room, self.cx.room, i.trigger.instruction_pos, 20.0)
                        && 0.7 > comm.current_concern_weight
                    {
                        comm.current_concern_weight = 0.7;
                        comm.current_concern = PlayerConcern::InputInstruction;
                    }
                }
                _ => comm.input_instruction = None,
            }
        }
        self.creature_concern(comm);
        if player.food < self.cx.story.food_to_hibernate {
            self.food_concern(comm, player);
        } else {
            comm.most_delicious_food_in_room = None;
            comm.most_delicous_food_delicious = 0.0;
        }
        let guide = &*self.cx.guide;
        if guide.angry_with_player() && !guide.displayed_anger() {
            comm.current_concern_weight = 1.0;
            comm.current_concern = PlayerConcern::Anger;
        }
        if self.gate_scene_room() {
            // `regionGate` is not ported (null): no gate scene.
            comm.current_concern = PlayerConcern::None;
        } else if comm.image_to_show.is_some() {
            if room == comm.show_image_room && self.want_to_show_image() {
                if comm.current_concern_weight < 0.99 {
                    comm.current_concern = PlayerConcern::ShowPlacedScene;
                    comm.current_concern_weight = 0.99;
                }
            } else {
                comm.image_to_show = None;
            }
        } else if let Some(id) = comm.forced_direction_to_give {
            if self.forced_direction(id).is_some_and(|d| d.room == room) {
                if comm.current_concern_weight < 0.98 {
                    comm.current_concern = PlayerConcern::ForcedDirection;
                    comm.current_concern_weight = 0.98;
                }
            } else {
                comm.forced_direction_to_give = None;
            }
        }
        if comm.current_concern == PlayerConcern::None || !open {
            if let Some(h) = self.body.current_hologram_mut() {
                h.still_relevant = false;
            }
            return None;
        }
        self.abs.player_guide_counter = 100;
        self.abs.go_to_player = true;
        let weight = comm.current_concern_weight;
        let message = match comm.current_concern {
            PlayerConcern::Bats => {
                if self.cx.world.guide_active_swarm_room(room) {
                    self.cx.guide.has_been_to_a_swarm_room_this_cycle = true;
                    return None;
                }
                HologramMessage::Bats
            }
            PlayerConcern::Shelter => HologramMessage::Shelter,
            PlayerConcern::DangerousCreature => HologramMessage::DangerousCreature,
            PlayerConcern::Progression => HologramMessage::ProgressionDirection,
            PlayerConcern::ShowGateScene | PlayerConcern::ShowPlacedScene => {
                HologramMessage::GateScene
            }
            PlayerConcern::FoodItemInRoom => HologramMessage::FoodObject,
            PlayerConcern::Anger => HologramMessage::Angry,
            PlayerConcern::ForcedDirection => HologramMessage::ForcedDirection,
            PlayerConcern::InputInstruction => {
                let kind = comm
                    .input_instruction
                    .and_then(|id| self.instruction(id))
                    .map(|i| i.kind);
                match kind {
                    Some(InstructionViewKind::SuperJump { .. }) => {
                        HologramMessage::InWorldSuperJump
                    }
                    Some(InstructionViewKind::Pickup { .. }) => HologramMessage::PickupObject,
                    Some(InstructionViewKind::ScavengerTrade { .. }) => {
                        HologramMessage::ScavengerTrade
                    }
                    None => return None,
                }
            }
            PlayerConcern::None => return None,
        };
        Some((message, weight))
    }

    /// The input instruction object `id` as this tick sees it.
    pub(crate) fn instruction(&self, id: u64) -> Option<InputInstructionView> {
        self.cx.instructions.iter().find(|i| i.id == id).copied()
    }

    /// The `ReliableIggyDirection` object `id`, while it is in a room.
    pub(crate) fn forced_direction(&self, id: u64) -> Option<ForcedDirectionView> {
        self.cx
            .forced_directions
            .iter()
            .find(|d| d.id == id)
            .copied()
    }

    /// `ReevaluateConcern`'s creature part: a random creature of the room (a `Range`)
    /// may become the most dangerous one, and a realized one is a chance (a `value`) to
    /// count as warned of when its type is known.
    fn creature_concern(&mut self, comm: &mut CommunicationModule) {
        let room = self.cx.room_index;
        let list = self.cx.world.room_creatures(room);
        let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
        let pick = list[usize::try_from(self.cx.rng.range_i32(0, len)).unwrap_or(0)];
        comm.most_dangerous_creature_danger =
            self.creature_danger_score(comm, comm.most_dangerous_creature_in_room);
        let danger = self.creature_danger_score(comm, Some(pick));
        if danger > comm.most_dangerous_creature_danger * 1.1 {
            comm.most_dangerous_creature_in_room = Some(pick);
            comm.most_dangerous_creature_danger = danger;
        }
        if comm.most_dangerous_creature_danger == 0.0 {
            comm.most_dangerous_creature_in_room = None;
        } else if comm.most_dangerous_creature_danger > comm.current_concern_weight {
            comm.current_concern_weight = comm.most_dangerous_creature_danger;
            comm.current_concern = PlayerConcern::DangerousCreature;
        }
        let Some(info) = self.realized(pick) else {
            return;
        };
        if self.cx.rng.value() < 0.025
            && self.creature_any_potential_danger_at_all(pick)
            && self.cx.camera.viewed(room, self.cx.room, info.pos, -10.0)
            && self.cx.guide.knows_creature_type(info.kind)
        {
            let id = self.cx.world.creature(pick).id;
            comm.player_has_now_been_warned_of_creature(id, info.kind, false, self.cx.guide);
        }
    }

    /// `CreatureAnyPotentialDangerAtAll`: a realized, living, not small creature in the
    /// player's room.
    fn creature_any_potential_danger_at_all(&self, id: CreatureId) -> bool {
        let c = self.cx.world.creature(id);
        if c.template.small_creature || c.pos.room != self.cx.room_index {
            return false;
        }
        let slated = id != self.me
            && matches!(&c.realized, Some(crate::creature::RealizedCreature::Overseer(b)) if b.slated);
        self.realized(id).is_some_and(|r| !r.dead) && !slated
    }

    /// `CreatureDangerScore` (231884). A creature whose type the player knows and that a
    /// camera shows scores 0; else its template's `dangerousToPlayer` (a scavenger's 1)
    /// times its AI's aggression to the player, fading from 20 to 1100 px from the
    /// player, 0 once talked about, raised to a power of the guide's hand-holding. A
    /// mimicking pole mimic is never seen (pole mimics are not realized yet).
    pub(crate) fn creature_danger_score(
        &self,
        comm: &CommunicationModule,
        creature: Option<CreatureId>,
    ) -> f32 {
        let Some(id) = creature.filter(|&c| self.creature_any_potential_danger_at_all(c)) else {
            return 0.0;
        };
        let (Some(info), Some(player)) = (self.realized(id), self.cx.player) else {
            return 0.0;
        };
        let viewed = self
            .cx
            .camera
            .viewed(self.cx.room_index, self.cx.room, info.pos, 60.0);
        if self.cx.guide.knows_creature_type(info.kind) && viewed {
            return 0.0;
        }
        let c = self.cx.world.creature(id);
        let mut num = if info.kind == CreatureType::Scavenger {
            1.0
        } else {
            c.template.dangerous_to_player
        };
        if num == 0.0 {
            return 0.0;
        }
        if let Some(aggression) = self
            .cx
            .world
            .current_player_aggression(id, self.cx.player_id)
        {
            num *= aggression;
        }
        num *= info.pos.distance(player.pos).inverse_lerp(1100.0, 20.0);
        if num == 0.0 || comm.objects_already_talked_about.contains(&c.id) {
            return 0.0;
        }
        num.mathf_pow(1.25.lerp(0.25, self.cx.guide.hand_holding))
    }

    /// `ReevaluateConcern`'s food part while the player is hungry: a random entity of
    /// the room (creatures and objects, a `Range`) may become the most delicious food,
    /// and food the player holds is learnt about.
    fn food_concern(&mut self, comm: &mut CommunicationModule, player: PlayerView) {
        let pick = self
            .cx
            .world
            .random_room_entity(self.cx.room_index, self.cx.rng);
        comm.most_delicous_food_delicious =
            self.food_delicious_score(comm, comm.most_delicious_food_in_room, player);
        let score = self.food_delicious_score(comm, pick, player);
        if score > comm.most_delicous_food_delicious * 1.1 {
            comm.most_delicious_food_in_room = pick;
            comm.most_delicous_food_delicious = score;
        }
        if comm.most_delicous_food_delicious == 0.0 {
            comm.most_delicious_food_in_room = None;
        } else if comm.most_delicous_food_delicious > comm.current_concern_weight {
            comm.current_concern_weight = comm.most_delicous_food_delicious;
            comm.current_concern = PlayerConcern::FoodItemInRoom;
        }
        if let Some(Entity::Object(o)) = comm.most_delicious_food_in_room {
            let object = self.cx.world.object(o);
            let held_by_player = self
                .cx
                .world
                .grabbed_by(Entity::Object(o))
                .first()
                .is_some_and(|g| g.grabber == self.cx.player_id);
            if !self.cx.guide.knows_item_type(object.kind)
                && object.realized.is_some()
                && held_by_player
            {
                log::info!("player has learnt about {:?}", object.kind);
                self.cx.guide.learn_item_type(object.kind);
            }
        }
    }

    /// `FoodDelicousScore`: dangle fruit, jellyfish, seed cobs and bubble fruit within
    /// 1100 px, less once the player knows the kind. A creature's type is `Creature`,
    /// which scores 0; only weapons are ever realized in the port, so every entity
    /// scores 0 (the seed cob's `dead`/`opened` are read with its realization).
    fn food_delicious_score(
        &self,
        comm: &CommunicationModule,
        food: Option<Entity>,
        player: PlayerView,
    ) -> f32 {
        let Some(Entity::Object(o)) = food else {
            return 0.0;
        };
        let object = self.cx.world.object(o);
        let Some(realized) = object.realized.as_deref() else {
            return 0.0;
        };
        if object.pos.room != self.cx.room_index
            || !matches!(
                object.kind,
                AbstractObjectType::DangleFruit
                    | AbstractObjectType::JellyFish
                    | AbstractObjectType::SeedCob
                    | AbstractObjectType::WaterNut
            )
        {
            return 0.0;
        }
        let at = realized.body.chunks[0].pos;
        let mut num = at.distance(player.pos).inverse_lerp(1100.0, 400.0);
        if num == 0.0 {
            return 0.0;
        }
        if self.cx.guide.knows_item_type(object.kind) {
            if num <= 0.2
                || !self
                    .cx
                    .camera
                    .viewed(self.cx.room_index, self.cx.room, at, 0.0)
            {
                return 0.0;
            }
            num = 0.3;
        }
        if comm.objects_already_talked_about.contains(&object.id) {
            return 0.0;
        }
        if food == comm.most_delicious_food_in_room
            && comm.current_concern == PlayerConcern::FoodItemInRoom
        {
            num *= 1.1;
        }
        num * self
            .general_player_food_need(comm, player.food)
            .lerp(0.6, 0.5)
    }

    /// `progressionShowTendency` on entering a room: how near the direction finder's
    /// destination the room's exits are.
    fn progression_show_tendency(&self) -> f32 {
        let world = self.cx.world;
        let Some(finder) = world.guide_direction_finder().filter(|f| f.done) else {
            return 0.0;
        };
        let _ = finder;
        if matches!(world.region_name(), Some("SL" | "SH")) {
            return 1.0;
        }
        let room = self.cx.room_index;
        let connections = world
            .abstract_room(room)
            .map_or(0, |r| i32::try_from(r.connections.len()).unwrap_or(0));
        let mut nearest = f32::MAX;
        for i in 0..connections {
            let d = world.guide_direction_distance(WorldCoordinate::new(room, -1, -1, i));
            if d < nearest {
                nearest = d;
            }
        }
        let mut tendency = nearest.inverse_lerp(400.0, 200.0);
        if self.cx.player_karma == self.cx.story.karma_cap {
            tendency = tendency.lerp(1.0, 0.5);
        }
        tendency
    }

    /// `AnyProgressionDirection` for the player in the guide's room.
    fn any_progression_direction(&self, comm: &CommunicationModule) -> bool {
        comm.any_progression_direction(
            self.cx.world,
            Some(self.cx.room_index),
            self.cx.player_karma,
        )
    }

    /// `WantToShowImage(room.abstractRoom.name)`.
    fn want_to_show_image(&self) -> bool {
        let gate_scene = self
            .body
            .current_hologram()
            .is_some_and(|h| h.message == HologramMessage::GateScene);
        if gate_scene {
            return true;
        }
        let name = self
            .cx
            .world
            .abstract_room(self.cx.room_index)
            .map_or("", |r| r.name.as_str());
        !self.cx.guide.has_image_been_shown_in_room(name)
    }

    /// `PlayerProgressionNeed`.
    fn player_progression_need(&self, comm: &CommunicationModule, food: i32) -> f32 {
        if !self.any_progression_direction(comm) || comm.first_few_cycles {
            return 0.0;
        }
        let rain = &self.cx.rain;
        let progression = (rain.timer as f32).inverse_lerp(0.0, rain.cycle_length as f32);
        let sin = f64::from(progression * std::f32::consts::PI).sin() as f32;
        (food as f32).lerp_map(0.0, 4.0, 0.6, 1.0)
            * sin.mathf_pow(0.5)
            * (0.2 + 0.8 * self.cx.guide.want_direction_hand_holding_this_cycle)
            * comm.progression_show_tendency
    }

    /// The gates where the guide shows Moon's scene to a player who never met her.
    fn gate_scene_room(&self) -> bool {
        let Some(data) = self.cx.world.abstract_room(self.cx.room_index) else {
            return false;
        };
        data.gate()
            && !self.cx.story.ever_met_moon
            && matches!(
                data.name.as_str(),
                "GATE_SU_HI" | "GATE_HI_GW" | "GATE_GW_SL" | "GATE_SH_SL"
            )
            && data.name.split('_').nth(1) == self.cx.world.region_name()
    }

    /// `PlayerShelterNeed` (`forceShelterNeed` is a Remix dev flag, always off).
    fn player_shelter_need(&self, comm: &CommunicationModule, food: i32) -> f32 {
        let stats = self.cx.story;
        if self.cx.room_index == comm.no_shelter_direction_from_room {
            return 0.0;
        }
        if comm.first_few_cycles {
            return if food < stats.food_to_hibernate {
                0.0
            } else {
                0.69
            };
        }
        let until_rain = self.cx.rain.time_until_rain() as f32;
        let need = if food < stats.food_to_hibernate || comm.has_already_showed_player_to_a_shelter
        {
            until_rain.inverse_lerp(4800.0, 2400.0) * 0.35
        } else {
            until_rain.inverse_lerp((30 * (6 + food) * 40) as f32, 2400.0)
        };
        let guide = &*self.cx.guide;
        need * (guide.hand_holding.mathf_pow(0.4) * guide.want_shelter_hand_holding_this_cycle)
    }

    /// `PlayerBatsNeed`.
    fn player_bats_need(&self, comm: &CommunicationModule, food: i32) -> f32 {
        if self.cx.room_index == comm.no_bat_direction_from_room
            || food >= self.cx.story.food_to_hibernate
        {
            return 0.0;
        }
        if comm.first_few_cycles {
            return 0.69;
        }
        let guide = &*self.cx.guide;
        let mut need = self.general_player_food_need(comm, food);
        if guide.has_been_to_a_swarm_room_this_cycle {
            need *= 0.2;
        }
        need * guide.want_food_hand_holding_this_cycle
    }

    /// `GeneralPlayerFoodNeed`.
    fn general_player_food_need(&self, comm: &CommunicationModule, food: i32) -> f32 {
        if comm.first_few_cycles {
            return if food < self.cx.story.food_to_hibernate {
                0.69
            } else {
                0.0
            };
        }
        let guide = &*self.cx.guide;
        let rain = &self.cx.rain;
        let timer = rain.timer as f32;
        let progression = timer.inverse_lerp(0.0, rain.cycle_length as f32);
        let sin = f64::from(progression * std::f32::consts::PI).sin() as f32;
        let swarm = if guide.has_been_to_a_swarm_room_this_cycle {
            0.5
        } else {
            1.0
        };
        timer.inverse_lerp(1600.0, 40.0 * 280.0.lerp(80.0, guide.hand_holding))
            * sin.mathf_pow((1 + food) as f32)
            * guide.hand_holding
            * (food as f32).lerp_map(0.0, (self.cx.story.food_to_hibernate - 1) as f32, 1.0, 0.5)
            * swarm
    }

    // ---- OverseerTutorialBehavior ----

    /// `OverseerTutorialBehavior.Update`.
    fn tutorial_update(&mut self) {
        let Some(mut tutorial) = self.ai.tutorial.take() else {
            return;
        };
        self.tutorial_step(&mut tutorial);
        self.ai.tutorial = Some(tutorial);
    }

    fn tutorial_step(&mut self, t: &mut TutorialBehavior) {
        let world = self.cx.world;
        let data = world.abstract_room(self.cx.room_index);
        let name = data.map_or("", |r| r.name.as_str());
        let gate = data.is_some_and(rw_assets::world::AbstractRoomData::gate);
        if t.encounter_counter > 0 && !gate && TUTORIAL_ROOMS.contains(&name) {
            self.ai.look_at = Vec2::new(2900.0, 200.0);
            self.ai.bring_up_lens = 0.0;
            self.ai.random_bring_up_lens_bonus = -100.0;
            if self.body.root_tile.0 < 120 {
                let b = &mut *self.body;
                b.root_tile.0 = 137;
                for y in (0..=b.root_tile.1).rev() {
                    if self.cx.room.is_solid_tile(137, y) {
                        b.root_tile.1 = y;
                        break;
                    }
                }
                // The game passes `rootPos` (a pixel position) to `MiddleOfTile`.
                b.root_pos = b.root_pos.tile_middle();
                b.hover_tile = b.root_tile;
                b.next_hover_tile = b.root_tile;
                let root = b.root_tile;
                let size = self.room_size();
                self.ai.reset_zip_matrix(root, size);
            }
        }
        let Some(player) = self.cx.player.filter(|p| p.in_room) else {
            return;
        };
        if t.encounter_counter == 1 {
            self.ai.look_at = player.pos;
            let away = player.pos + player.pos.direction_to(self.body.root_pos) * 1600.0;
            self.ai.temp_hover_tile = away.tile();
        }
        let food = player.food;
        let stats = self.cx.story;
        match name {
            "SU_C04" => {
                if !t.messages_displayed[0] && player.pos.x > 1450.0 {
                    self.tutorial_text("You are hungry, find food", 10, 160, true);
                    t.messages_displayed[0] = true;
                }
                if TutorialBehavior::in_box_zone(player.hips.x) {
                    t.player_has_approached_box = true;
                }
                if t.player_has_approached_box {
                    t.box_trouble += 1;
                }
                if !t.player_has_jumped_over_box
                    && t.box_trouble > 800
                    && TutorialBehavior::in_box_zone(player.hips.x)
                {
                    self.tutorial_hologram(HologramMessage::GetUpOnFirstBox, f32::MAX);
                }
                let pos = self.body.chunk().pos;
                if t.encounter_counter > 0
                    && (self
                        .cx
                        .camera
                        .viewed(self.cx.room_index, self.cx.room, pos, 0.0)
                        || t.box_trouble > 400)
                {
                    t.encounter_counter -= 1;
                }
            }
            "SU_A41" => {
                if !t.player_has_climbed_pole && player.pos.y < 250.0 && player.pos.x < 700.0 {
                    t.pole_trouble += 1;
                }
                if t.pole_trouble > 50 && player.pos.y > 300.0 {
                    t.player_has_climbed_pole = true;
                }
                let in_pole_zone = player.hips.x > 360.0 && player.hips.x < 660.0;
                if t.pole_trouble > 200 && !t.player_has_climbed_pole && in_pole_zone {
                    self.tutorial_hologram(HologramMessage::ClimbPole, f32::MAX);
                } else if food < stats.max_food - 1 {
                    self.tutorial_hologram(HologramMessage::TutorialFood, 0.5);
                } else {
                    self.tutorial_hologram(HologramMessage::Shelter, 0.5);
                }
                self.force_shelter_door(1);
            }
            "SU_A42" => {
                t.bat_trouble += 1;
                if self.eat_instruction_relevant(t, name, food) {
                    self.tutorial_hologram(HologramMessage::EatInstruction, 0.5);
                }
                if !t.messages_displayed[1] && t.bat_trouble > 30 {
                    self.tutorial_text("Catch and eat them", 10, 120, true);
                    t.messages_displayed[1] = true;
                }
            }
            "SU_A43" => {
                if food < stats.food_to_hibernate {
                    self.tutorial_hologram(HologramMessage::TutorialFood, 0.5);
                } else {
                    if !t.messages_displayed[2] {
                        self.tutorial_text("Leap the gap", 40, 120, true);
                        t.messages_displayed[2] = true;
                    }
                    if player.pos.x > 600.0 {
                        t.display_run_message = true;
                        t.bring_in_rain = true;
                        t.pause_rain = false;
                    }
                    if player.pos.y < 400.0 {
                        t.super_jump_trouble += 1;
                    }
                    if !t.player_has_made_super_jump
                        && t.super_jump_trouble > 40
                        && player.hips.x < 505.0
                    {
                        self.tutorial_hologram(HologramMessage::SuperJump, f32::MAX);
                    } else {
                        self.tutorial_hologram(HologramMessage::Shelter, 0.5);
                    }
                    if player.pos.x < 530.0 && player.pos.y < 400.0 {
                        t.player_has_made_super_jump = false;
                    }
                }
            }
            "SU_A22" => {
                t.pause_rain = false;
                if food >= stats.food_to_hibernate {
                    self.tutorial_hologram(HologramMessage::Shelter, 0.5);
                    if !t.messages_displayed[3] && t.display_run_message {
                        self.tutorial_text("Run!", 40, 60, true);
                        t.messages_displayed[3] = true;
                    }
                }
                if !t.messages_displayed[6] && player.entering_shortcut.is_some_and(|e| e.1 < 20) {
                    self.tutorial_text("This place is safe. Stay here to hibernate", 50, 160, true);
                    t.messages_displayed[6] = true;
                }
                self.force_shelter_door(2);
            }
            "SU_A44" => {
                t.fruit_trouble += 1;
                if player.jump[0] && !player.jump[1] {
                    t.fruit_trouble += 100;
                }
                if !t.messages_displayed[4] && t.fruit_trouble > 30 {
                    self.tutorial_text("Jump and grab them. They're delicious!", 10, 120, true);
                    t.messages_displayed[4] = true;
                }
                if self.eat_instruction_relevant(t, name, food) {
                    self.tutorial_hologram(HologramMessage::EatInstruction, 0.5);
                }
            }
            _ => {}
        }
        if !gate {
            self.food_messages(t, food);
        }
        t.last_food_in_stomach = food;
        self.rain_control(t);
        if let Some(a22) = world.room_index("SU_A22") {
            self.cx.effects.push(OverseerEffect::KillGreenLizards(a22));
        }
    }

    /// `EatInstructionRelevant` with the edible objects of the overseer's room.
    fn eat_instruction_relevant(&self, t: &TutorialBehavior, room: &str, food: i32) -> bool {
        let s = self.cx.story;
        t.eat_instruction_relevant(
            room,
            food,
            s.food_to_hibernate,
            s.max_food,
            self.cx.edibles.len(),
        )
    }

    /// The hibernation food lines when the stomach passes a mark.
    fn food_messages(&mut self, t: &TutorialBehavior, food: i32) {
        let s = self.cx.story;
        let last = t.last_food_in_stomach;
        if food >= s.food_to_hibernate && last < s.food_to_hibernate {
            let text = if s.yellow {
                "Three is enough to hibernate"
            } else {
                "Four is enough to hibernate"
            };
            self.tutorial_text(text, 10, 120, false);
        } else if food > s.food_to_hibernate && last <= s.food_to_hibernate {
            let text = if s.yellow {
                "Additional food (above three) is kept for later"
            } else {
                "Additional food (above four) is kept for later"
            };
            self.tutorial_text(text, 10, 120, false);
        } else if food == s.max_food && last < s.max_food {
            self.tutorial_text("You are full", 10, 120, false);
        }
    }

    /// `pauseRain` pins the cycle at 2000 ticks; `bringInRain` hurries it to 1000 ticks
    /// before the rain.
    fn rain_control(&mut self, t: &mut TutorialBehavior) {
        let rain = &mut self.cx.rain;
        if t.pause_rain {
            rain.timer = 2000;
            return;
        }
        if !t.bring_in_rain {
            return;
        }
        if rain.timer < rain.cycle_length - 2400 {
            rain.timer = (rain.cycle_length - 2400).min(rain.timer + 400);
        }
        if !t.messages_displayed[5] {
            self.tutorial_text("Rain is coming! Find shelter", 40, 200, true);
            t.messages_displayed[5] = true;
        }
        let rain = &mut self.cx.rain;
        if rain.timer < rain.cycle_length - 1000 {
            rain.timer = (rain.cycle_length - 1000).min(rain.timer + 10);
            if self.cx.hud {
                self.cx.effects.push(OverseerEffect::HalfTimeBlinkReset);
            }
        } else {
            t.bring_in_rain = false;
        }
    }

    /// `TutorialText`: shown only when camera 0 has a HUD.
    fn tutorial_text(&mut self, text: &'static str, wait: i32, time: i32, hide_hud: bool) {
        if !self.cx.hud {
            return;
        }
        log::info!("tutorial: {text}");
        self.cx
            .effects
            .push(OverseerEffect::Message(TutorialMessage {
                text,
                wait,
                time,
                hide_hud,
            }));
    }

    /// The tutorial's `FORCEPOINT`: a current shelter pointer shows this door.
    fn force_shelter_door(&mut self, door: i32) {
        if let Some(HologramKind::Door {
            pointer: PointerKind::Shelter { forcepoint, .. },
            ..
        }) = self.body.current_hologram_mut().map(|h| &mut h.kind)
        {
            *forcepoint = door;
        }
    }
}

impl OverseerBody {
    /// `Room.RayTraceTilesList(from, to, ref zipPath)`: the line's tiles written over
    /// the path's start; `zipPathCount` is their number.
    pub(crate) fn ray_trace_zip_path(&mut self, from: (i32, i32), to: (i32, i32)) {
        let (mut dx, mut dy) = ((to.0 - from.0).abs(), (to.1 - from.1).abs());
        let (mut x, mut y) = from;
        let mut n = 1 + dx + dy;
        let sx = if to.0 > from.0 { 1 } else { -1 };
        let sy = if to.1 > from.1 { 1 } else { -1 };
        let mut err = dx - dy;
        dx *= 2;
        dy *= 2;
        let mut i = 0;
        while n > 0 {
            if self.zip_path.len() <= i {
                self.zip_path.push((x, y));
            } else {
                self.zip_path[i] = (x, y);
            }
            i += 1;
            if err > 0 {
                x += sx;
                err -= dy;
            } else {
                y += sy;
                err += dx;
            }
            n -= 1;
        }
        self.zip_path_count = i;
    }
}
