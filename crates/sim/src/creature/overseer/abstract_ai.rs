//! `OverseerAbstractAI` (base game: roaming between rooms by attraction, the player
//! guide's comings and goings, leaving for the off-screen den before the rain).
//! Behaviour: `rainworld-re/notes/creature-model/overseers-and-flies.md`.
//!
//! Mirrors: OverseerAbstractAI.cs

use rw_assets::{CreatureType, RoomAttraction, SlugcatName};
use rw_random::UnityRandom;

use super::{OverseerAi, PlayerGuideState};
use crate::FloatExt;
use crate::creature::abstract_ai::{AbstractCreatureAi, AiContext, SeededRandom};
use crate::creature::{AbstractCreature, CreatureId, WorldCoordinate};
use crate::world::World;

/// `OverseerAbstractAI`'s own fields.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerAbstractAi {
    /// Its own `lastRoom` (a `WorldCoordinate`, all zero at first, hiding the base's).
    pub last_room: WorldCoordinate,
    /// Rooms visited, newest first, at most 10.
    pub last_rooms: Vec<i32>,
    pub target_creature: Option<CreatureId>,
    pub last_target_creature: Option<CreatureId>,
    pub go_to_player: bool,
    pub player_guide_counter: i32,
    pub target_creature_counter: i32,
    pub target_creature_time: i32,
    pub freeze_standard_roaming: i32,
    pub owner_iterator: i32,
    pub is_player_guide: bool,
    /// `RealAI`: built with each realization and kept when the overseer is abstracted
    /// again (it has no path finder to drop).
    pub real_ai: Option<Box<OverseerAi>>,
}

/// The tutorial rooms of the base game (`DefineTutorialRooms` without Remix's gates).
pub const TUTORIAL_ROOMS: [&str; 6] = ["SU_C04", "SU_A41", "SU_A42", "SU_A43", "SU_A44", "SU_A22"];

/// What `AbstractBehavior` reads of its `parent` (passed in: the creature's realized
/// parts are out of the world while it updates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Parent {
    pub id: CreatureId,
    pub pos: WorldCoordinate,
    pub realized: bool,
    pub ignore_cycle: bool,
    /// `RealAI.tutorialBehavior != null`.
    pub tutorial: bool,
}

/// Where `AbstractBehavior` sends the overseer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbstractStep {
    Stay,
    /// `parent.Move(destination)` (abstract).
    Move(WorldCoordinate),
    /// Realized: `ZipOutOfRoom(destination)` when watching or projecting.
    ZipOut(WorldCoordinate),
}

/// What `AbstractBehavior` reads besides the world.
pub(crate) struct GuideContext<'a> {
    pub world: &'a World,
    /// `world.rainCycle` (a realized overseer's tutorial may have moved its timer).
    pub rain: &'a crate::world::RainCycle,
    pub rng: &'a mut UnityRandom,
    pub guide: &'a PlayerGuideState,
    /// `world.game.FirstAnyPlayer`.
    pub player: Option<CreatureId>,
    /// The player's `Karma` (the save state's).
    pub player_karma: i32,
}

impl OverseerAbstractAi {
    /// The constructor: `playerGuideCounter`, the owner draws, `ignoreCycle` in the
    /// timerless regions (the world's `OverseersWorldAI` is made by the world).
    pub(crate) fn new(parent: &mut AbstractCreature, cx: &mut AiContext<'_>) -> Self {
        let mut ai = Self {
            last_room: WorldCoordinate::new(0, 0, 0, 0),
            last_rooms: vec![parent.pos.room],
            target_creature: None,
            last_target_creature: None,
            go_to_player: false,
            player_guide_counter: cx.rng.range_i32(200, 500),
            target_creature_counter: 0,
            target_creature_time: 0,
            freeze_standard_roaming: 0,
            owner_iterator: 0,
            is_player_guide: false,
            real_ai: None,
        };
        let world = cx.world;
        if !world.single_room() {
            let region = world.region_name().unwrap_or("");
            if matches!(region, "SS" | "RM" | "DM") {
                parent.ignore_cycle = true;
            }
            if cx.session.character == SlugcatName::Red && region != "UW" {
                let chance = if region == "SL" { 0.2 } else { 0.0125 };
                if cx.rng.value() < chance {
                    ai.owner_iterator = 3;
                }
            }
            if region == "SB" {
                ai.owner_iterator = 2;
            }
        }
        ai
    }

    /// `SetAsPlayerGuide`.
    pub fn set_as_player_guide(&mut self, owner: i32) {
        self.is_player_guide = true;
        self.owner_iterator = owner;
    }

    /// `OverseerGraphics.MainColor`'s choice.
    #[must_use]
    pub fn color(&self) -> super::OverseerColor {
        use super::OverseerColor;
        match self.owner_iterator {
            1 => OverseerColor::Yellow,
            2 => OverseerColor::Green,
            _ if self.is_player_guide || self.owner_iterator == 3 => OverseerColor::Yellow,
            _ => OverseerColor::Wild,
        }
    }

    /// Whether the realized AI has a `tutorialBehavior`.
    #[must_use]
    pub fn has_tutorial(&self) -> bool {
        self.real_ai
            .as_ref()
            .is_some_and(|ai| ai.tutorial.is_some())
    }

    /// `AbstractBehavior(time)` of overseer `parent` (base game): where it goes.
    pub(crate) fn abstract_behavior(
        &mut self,
        base: &mut AbstractCreatureAi,
        parent: Parent,
        time: i32,
        cx: &mut GuideContext<'_>,
    ) -> AbstractStep {
        let world = cx.world;
        let me = parent.id;
        if world.single_room() && parent.pos.room == 0 {
            return AbstractStep::Stay;
        }
        if self.target_creature.is_some() {
            self.target_creature_time += 1;
        }
        if self.is_player_guide {
            self.player_guide_update(time, parent.tutorial, cx);
        }
        let player = cx.player;
        if player.is_some() && self.go_to_player {
            self.target_creature = player;
        } else {
            if self.is_player_guide
                && let Some(t) = self.target_creature
                && world.creature(t).kind() == CreatureType::Slugcat
            {
                if base.destination.room == world.creature(t).pos.room {
                    self.roam(base, parent, time, 1.0, cx);
                }
                self.last_target_creature = Some(t);
                self.target_creature = None;
            }
            let here = world.room_creatures(parent.pos.room);
            if here.len() > 1 && AbstractCreatureAi::time_influenced_roll(0.2, time, cx.rng) {
                let len = i32::try_from(here.len()).unwrap_or(i32::MAX);
                let pick = here[usize::try_from(cx.rng.range_i32(0, len)).unwrap_or(0)];
                let kind = world.creature(pick).kind();
                if kind != CreatureType::Overseer
                    && (kind != CreatureType::Slugcat || !self.is_player_guide)
                    && Some(pick) != self.last_target_creature
                    && self.interest(cx.world, cx.rng, cx.player_karma, me, Some(pick))
                        > self.interest(cx.world, cx.rng, cx.player_karma, me, self.target_creature)
                {
                    self.target_creature = Some(pick);
                    let interest = self.interest(cx.world, cx.rng, cx.player_karma, me, Some(pick));
                    self.target_creature_counter =
                        (100.0.lerp(2000.0, cx.rng.value()) * interest) as i32;
                }
            }
            // `targetCreature != null || HowInterestingIsCreature(targetCreature) <= 0`:
            // always true.
            self.target_creature_counter -= time;
            if self.target_creature_counter < 1 {
                self.last_target_creature = self.target_creature;
                self.target_creature = None;
            }
        }
        if parent.pos.room != self.last_room.room {
            self.last_rooms.insert(0, parent.pos.room);
            self.last_rooms.truncate(10);
            self.last_room = parent.pos;
        }
        if self.freeze_standard_roaming <= 0 {
            if cx.rain.time_until_rain() < 4800
                && !parent.ignore_cycle
                && (!self.is_player_guide || !parent.tutorial)
            {
                if AbstractCreatureAi::time_influenced_roll(1.0 / 120.0, time, cx.rng) {
                    let den = WorldCoordinate::new(world.offscreen_den_index(), -1, -1, 0);
                    base.set_destination_no_pathing(den, true);
                }
            } else if let Some(t) = self
                .target_creature
                .filter(|&t| self.room_allowed(me, world.creature(t).pos.room, world))
            {
                base.set_destination_no_pathing(world.creature(t).pos, true);
            } else {
                let attraction = world.attraction_value(parent.pos.room, CreatureType::Overseer);
                self.roam(base, parent, time, 1.0 / 20.0.lerp(2000.0, attraction), cx);
            }
        }
        let mut step = AbstractStep::Stay;
        if base.destination.room != parent.pos.room {
            let attraction = world
                .attraction_value(base.destination.room, CreatureType::Overseer)
                .mathf_pow(0.4);
            if AbstractCreatureAi::time_influenced_roll(
                1.0 / 200.0.lerp(10.0, attraction),
                time,
                cx.rng,
            ) {
                step = if parent.realized {
                    AbstractStep::ZipOut(base.destination)
                } else {
                    AbstractStep::Move(base.destination)
                };
            }
        }
        self.freeze_standard_roaming -= 1;
        step
    }

    /// `Roam`.
    fn roam(
        &self,
        base: &mut AbstractCreatureAi,
        parent: Parent,
        time: i32,
        chance: f32,
        cx: &mut GuideContext<'_>,
    ) {
        let world = cx.world;
        let me = parent.id;
        let pos = parent.pos;
        if base.destination.room != pos.room
            || !AbstractCreatureAi::time_influenced_roll(chance, time, cx.rng)
        {
            return;
        }
        let kind = CreatureType::Overseer;
        if pos.room == world.offscreen_den_index() {
            let first = world.first_room_index();
            let count = i32::try_from(world.number_of_rooms()).unwrap_or(i32::MAX);
            let mut best = f32::MIN;
            let mut pick = -1;
            for _ in 0..20 {
                let room = cx.rng.range_i32(first, first + count);
                if !self.room_allowed(me, room, world) {
                    continue;
                }
                let mut score =
                    world.size_dependent_attraction(room, kind) * 0.9.lerp(1.1, cx.rng.value());
                if world.attraction(room, kind) == Some(RoomAttraction::Avoid) {
                    score *= 0.1;
                }
                for &c in world.room_creatures(room) {
                    if world.creature(c).kind() == CreatureType::Overseer {
                        score *= 0.5;
                    }
                }
                if score > best {
                    best = score;
                    pick = room;
                }
            }
            if pick > -1 {
                log::debug!(
                    "overseer travelling to: {}",
                    world.abstract_room(pick).map_or("?", |r| r.name.as_str())
                );
                base.set_destination_no_pathing(WorldCoordinate::new(pick, -1, -1, 1), true);
            }
            return;
        }
        let Some(room) = world.abstract_room(pos.room) else {
            return;
        };
        let attraction = world.attraction_value(pos.room, kind);
        let mut best = f32::MIN;
        let mut pick = -1;
        for &conn in &room.connections {
            if conn <= -1 || !self.room_allowed(me, conn, world) {
                continue;
            }
            let mut score = cx.rng.value();
            let interest = world
                .room_creatures(pos.room)
                .iter()
                .map(|&c| self.interest(cx.world, cx.rng, cx.player_karma, me, Some(c)))
                .fold(0.0, f32::max);
            score += interest * cx.rng.value();
            score *= attraction;
            score += 20.0 * attraction;
            if let Some(m) = self.last_rooms.iter().position(|&r| r == conn) {
                score -= (20.0 - m as f32) * cx.rng.value();
            }
            if score > best {
                pick = conn;
                best = score;
            }
        }
        if pick > -1 {
            base.set_destination_no_pathing(WorldCoordinate::new(pick, -1, -1, 0), true);
        }
    }

    /// `RoomAllowed`.
    #[must_use]
    pub fn room_allowed(&self, _me: CreatureId, room: i32, world: &World) -> bool {
        let first = world.first_room_index();
        let count = i32::try_from(world.number_of_rooms()).unwrap_or(i32::MAX);
        if room < first || room >= first + count {
            return false;
        }
        let Some(data) = world.abstract_room(room) else {
            return false;
        };
        if world.attraction(room, CreatureType::Overseer) == Some(RoomAttraction::Forbidden)
            && (!self.is_player_guide || !data.gate())
        {
            return false;
        }
        !data
            .tags
            .iter()
            .any(|t| t == "SCAVOUTPOST" || t == "SCAVTRADER")
    }

    /// `HowInterestingIsCreature` (`rng` only for its seeded draw, state kept).
    pub(crate) fn interest(
        &self,
        world: &World,
        rng: &UnityRandom,
        player_karma: i32,
        me: CreatureId,
        test: Option<CreatureId>,
    ) -> f32 {
        let Some(test) = test else {
            return 0.0;
        };
        let crit = world.creature(test);
        if crit.template.small_creature {
            return 0.0;
        }
        let mut num = match crit.kind() {
            CreatureType::Slugcat => {
                let mut n = if crit.is_realized() {
                    (player_karma as f32).lerp_map_pow(0.0, 4.0, 0.15, 2.0, 1.2)
                } else {
                    0.15
                };
                if self.is_player_guide {
                    n = n.max(1.5);
                }
                n
            }
            CreatureType::PinkLizard
            | CreatureType::YellowLizard
            | CreatureType::BlackLizard
            | CreatureType::BlueLizard
            | CreatureType::Salamander => 0.5,
            CreatureType::GreenLizard => 0.2,
            CreatureType::WhiteLizard => 0.6,
            CreatureType::RedLizard => 0.65,
            CreatureType::Overseer | CreatureType::Scavenger => 1.0,
            CreatureType::Vulture | CreatureType::KingVulture | CreatureType::LanternMouse => 0.25,
            CreatureType::GarbageWorm => 0.65,
            CreatureType::CicadaA | CreatureType::CicadaB => 0.15,
            CreatureType::DaddyLongLegs | CreatureType::BrotherLongLegs => 0.2,
            CreatureType::BigEel => 0.55,
            _ => 0.1,
        };
        if !crit.state.alive {
            num /= 10.0;
        }
        num *= world.attraction_value(crit.pos.room, CreatureType::Overseer);
        let me_seed = world.creature(me).id.random_seed();
        let seed = me_seed.wrapping_add(crit.id.random_seed());
        // `SeededRandom` restores the global state; a scratch copy is equivalent.
        let mut scratch = rng.clone();
        num * 0.5.lerp(1.5, scratch.seeded_value(seed))
    }

    /// `PlayerGuideUpdate`.
    fn player_guide_update(&mut self, time: i32, tutorial: bool, cx: &mut GuideContext<'_>) {
        let Some(player) = cx.player else {
            return;
        };
        self.player_guide_counter -= time;
        if self.player_guide_counter >= 1 {
            self.tutorial_call(tutorial, cx);
            return;
        }
        let world = cx.world;
        self.go_to_player = !self.go_to_player;
        let mut num = cx.guide.likes_player.inverse_lerp(-0.5, 1.0);
        if num <= 0.01 {
            self.go_to_player = false;
        }
        if self.go_to_player {
            let room = world.creature(player).pos.room;
            let scavengers = world.room_creatures(room).iter().any(|&c| {
                let c = world.creature(c);
                c.kind() == CreatureType::Scavenger && c.state.alive
            });
            if scavengers {
                self.go_to_player = false;
                log::debug!("Player guide not coming because scavengers in room");
            }
        }
        num = num.mathf_pow(0.15) * 0.5.lerp(1.0, cx.guide.hand_holding);
        let v = cx.rng.value();
        self.player_guide_counter += if self.go_to_player {
            200.0.lerp(400.0.lerp(1200.0, num), v) as i32
        } else {
            800.0.lerp(4400.0.lerp(2200.0, num), v) as i32
        };
        log::debug!(
            "player guide {} for {} ticks",
            if self.go_to_player {
                "comes"
            } else {
                "goes away"
            },
            self.player_guide_counter
        );
        self.tutorial_call(tutorial, cx);
    }

    /// The end of `PlayerGuideUpdate`: a guide with a tutorial comes to a player in a
    /// tutorial room.
    fn tutorial_call(&mut self, tutorial: bool, cx: &GuideContext<'_>) {
        if self.go_to_player || !tutorial {
            return;
        }
        let Some(player) = cx.player else {
            return;
        };
        let room = cx.world.creature(player).pos.room;
        let name = cx.world.abstract_room(room).map_or("", |r| r.name.as_str());
        if TUTORIAL_ROOMS.contains(&name) {
            self.go_to_player = true;
            self.player_guide_counter = 1000;
        }
    }

    /// `PlayerGuideGoAway`.
    pub fn player_guide_go_away(&mut self, time: i32) {
        self.go_to_player = false;
        self.player_guide_counter = self.player_guide_counter.max(time);
    }
}
