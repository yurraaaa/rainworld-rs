//! Overseers: `OverseerAbstractAI` (base game: roaming between rooms by attraction,
//! the player guide's comings and goings, leaving for the off-screen den before the
//! rain), `PlayerGuideState`, and the realized body as far as it is ported.
//!
//! The realized `Overseer` stops at its placement (`PlaceInRoom`, `HardSetTile`): it
//! does not update, so its `OverseerAI` (which also calls `AbstractBehavior` while
//! realized) and the tutorial behaviour never run, and `tutorialBehavior` stays null
//! (docs/DIVERGENCES.md D14). Behaviour: `rainworld-re/notes/creature-model.md`.

use rw_assets::{CreatureType, RoomAttraction, SlugcatName};
use rw_random::UnityRandom;

use super::ai::{AbstractCreatureAi, AiContext, SeededRandom};
use super::{AbstractCreature, CreatureId, WorldCoordinate};
use crate::FloatExt;
use crate::world::World;

/// `PlayerGuideState` (in `miscWorldSaveData`), the fields the abstract AI reads; a new
/// game's values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerGuideState {
    pub likes_player: f32,
    pub hand_holding: f32,
    pub angry_with_player: bool,
}

impl Default for PlayerGuideState {
    fn default() -> Self {
        Self {
            likes_player: 0.3,
            hand_holding: 0.8,
            angry_with_player: false,
        }
    }
}

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
}

/// What `AbstractBehavior` reads besides the world.
pub(crate) struct GuideContext<'a> {
    pub world: &'a World,
    pub rng: &'a mut UnityRandom,
    pub guide: &'a PlayerGuideState,
    /// `world.game.FirstAnyPlayer`.
    pub player: Option<CreatureId>,
    /// The player's `Karma` (the save state's).
    pub player_karma: i32,
}

/// The realized `Overseer`, as far as it is ported: its tiles after `PlaceInRoom`.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerBody {
    pub hover_tile: (i32, i32),
    pub root_tile: (i32, i32),
    /// `size` (0.6 for the guide, else 0.5..1 from the ID's seed).
    pub size: f32,
    /// `OverseerAI.randomBringUpLensBonus`.
    pub random_bring_up_lens_bonus: f32,
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
        };
        let world = cx.world;
        if !world.single_room() {
            let region = world.region_name().unwrap_or("");
            if matches!(region, "SS" | "RM" | "DM") {
                parent.ignore_cycle = true;
            }
            if cx.session.character == SlugcatName::RED && region != "UW" {
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

    /// `AbstractBehavior(time)` of overseer `me` (base game). Returns where it moves
    /// (`parent.Move(destination)`); the realized branch (`ZipOutOfRoom`) is not
    /// reachable, as only abstract overseers behave.
    pub(crate) fn abstract_behavior(
        &mut self,
        base: &mut AbstractCreatureAi,
        me: CreatureId,
        time: i32,
        cx: &mut GuideContext<'_>,
    ) -> Option<WorldCoordinate> {
        let world = cx.world;
        let parent = world.creature(me);
        if world.single_room() && parent.pos.room == 0 {
            return None;
        }
        if self.target_creature.is_some() {
            self.target_creature_time += 1;
        }
        if self.is_player_guide {
            self.player_guide_update(time, cx);
        }
        let player = cx.player;
        if player.is_some() && self.go_to_player {
            self.target_creature = player;
        } else {
            if self.is_player_guide
                && let Some(t) = self.target_creature
                && world.creature(t).kind() == CreatureType::SLUGCAT
            {
                if base.destination.room == world.creature(t).pos.room {
                    self.roam(base, me, time, 1.0, cx);
                }
                self.last_target_creature = Some(t);
                self.target_creature = None;
            }
            let here = world.room_creatures(parent.pos.room);
            if here.len() > 1 && AbstractCreatureAi::time_influenced_roll(0.2, time, cx.rng) {
                let len = i32::try_from(here.len()).unwrap_or(i32::MAX);
                let pick = here[usize::try_from(cx.rng.range_i32(0, len)).unwrap_or(0)];
                let kind = world.creature(pick).kind();
                if kind != CreatureType::OVERSEER
                    && (kind != CreatureType::SLUGCAT || !self.is_player_guide)
                    && Some(pick) != self.last_target_creature
                    && self.interest(me, Some(pick), cx)
                        > self.interest(me, self.target_creature, cx)
                {
                    self.target_creature = Some(pick);
                    let interest = self.interest(me, Some(pick), cx);
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
        let parent = world.creature(me);
        if parent.pos.room != self.last_room.room {
            self.last_rooms.insert(0, parent.pos.room);
            self.last_rooms.truncate(10);
            self.last_room = parent.pos;
        }
        if self.freeze_standard_roaming <= 0 {
            // `RealAI.tutorialBehavior` is never built (see the module comment).
            if world.rain_cycle.time_until_rain() < 4800 && !parent.ignore_cycle {
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
                let attraction = world.attraction_value(parent.pos.room, CreatureType::OVERSEER);
                self.roam(base, me, time, 1.0 / 20.0.lerp(2000.0, attraction), cx);
            }
        }
        let mut moved = None;
        if base.destination.room != parent.pos.room {
            let attraction = world
                .attraction_value(base.destination.room, CreatureType::OVERSEER)
                .mathf_pow(0.4);
            if AbstractCreatureAi::time_influenced_roll(
                1.0 / 200.0.lerp(10.0, attraction),
                time,
                cx.rng,
            ) && !parent.is_realized()
            {
                moved = Some(base.destination);
            }
        }
        self.freeze_standard_roaming -= 1;
        moved
    }

    /// `Roam`.
    fn roam(
        &self,
        base: &mut AbstractCreatureAi,
        me: CreatureId,
        time: i32,
        chance: f32,
        cx: &mut GuideContext<'_>,
    ) {
        let world = cx.world;
        let pos = world.creature(me).pos;
        if base.destination.room != pos.room
            || !AbstractCreatureAi::time_influenced_roll(chance, time, cx.rng)
        {
            return;
        }
        let kind = CreatureType::OVERSEER;
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
                    if world.creature(c).kind() == CreatureType::OVERSEER {
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
                .map(|&c| self.interest(me, Some(c), cx))
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
        if world.attraction(room, CreatureType::OVERSEER) == Some(RoomAttraction::Forbidden)
            && (!self.is_player_guide || !data.gate())
        {
            return false;
        }
        !data
            .tags
            .iter()
            .any(|t| t == "SCAVOUTPOST" || t == "SCAVTRADER")
    }

    /// `HowInterestingIsCreature`.
    fn interest(&self, me: CreatureId, test: Option<CreatureId>, cx: &GuideContext<'_>) -> f32 {
        let Some(test) = test else {
            return 0.0;
        };
        let world = cx.world;
        let crit = world.creature(test);
        if crit.template.small_creature {
            return 0.0;
        }
        let mut num = match crit.kind().value() {
            "Slugcat" => {
                let mut n = if crit.is_realized() {
                    (cx.player_karma as f32).lerp_map_pow(0.0, 4.0, 0.15, 2.0, 1.2)
                } else {
                    0.15
                };
                if self.is_player_guide {
                    n = n.max(1.5);
                }
                n
            }
            "PinkLizard" | "YellowLizard" | "BlackLizard" | "BlueLizard" | "Salamander" => 0.5,
            "GreenLizard" => 0.2,
            "WhiteLizard" => 0.6,
            "RedLizard" => 0.65,
            "Overseer" | "Scavenger" => 1.0,
            "Vulture" | "KingVulture" | "LanternMouse" => 0.25,
            "GarbageWorm" => 0.65,
            "CicadaA" | "CicadaB" => 0.15,
            "DaddyLongLegs" | "BrotherLongLegs" => 0.2,
            "BigEel" => 0.55,
            _ => 0.1,
        };
        if !crit.state.alive {
            num /= 10.0;
        }
        num *= world.attraction_value(crit.pos.room, CreatureType::OVERSEER);
        let me_seed = world.creature(me).id.random_seed();
        let seed = me_seed.wrapping_add(crit.id.random_seed());
        // `SeededRandom` restores the global state; a scratch copy is equivalent.
        let mut scratch = cx.rng.clone();
        num * 0.5.lerp(1.5, scratch.seeded_value(seed))
    }

    /// `PlayerGuideUpdate`.
    fn player_guide_update(&mut self, time: i32, cx: &mut GuideContext<'_>) {
        let Some(player) = cx.player else {
            return;
        };
        self.player_guide_counter -= time;
        if self.player_guide_counter >= 1 {
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
                c.kind() == CreatureType::SCAVENGER && c.state.alive
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
    }
}

impl OverseerBody {
    /// `Overseer`'s constructor (size from the ID's seed, saved state) and `OverseerAI`'s
    /// (`randomBringUpLensBonus`).
    pub(crate) fn new(parent: &AbstractCreature, rng: &mut UnityRandom) -> Self {
        let mut size = 0.5.lerp(1.0, rng.seeded_value(parent.id.random_seed()));
        if parent.overseer_ai().is_some_and(|o| o.is_player_guide) {
            size = 0.6;
        }
        Self {
            hover_tile: parent.pos.tile,
            root_tile: parent.pos.tile,
            size,
            random_bring_up_lens_bonus: rng.value(),
        }
    }
}
