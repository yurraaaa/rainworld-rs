//! `AbstractCreatureAI` and the per-type abstract AI constructors, with their random
//! draws. The plain behaviour (lizards) is `creature/abstract_ai/world.rs`, the overseer's
//! `overseer/abstract_ai.rs`.
//!
//! Mirrors: AbstractCreatureAI.cs

pub mod kinds;
pub(crate) mod kinds_world;
pub(crate) mod path_finder;
pub(crate) mod world;

use rw_assets::{CreatureType, SlugcatName, Timeline};
use rw_random::UnityRandom;

use super::overseer::OverseerAbstractAi;
use super::{AbstractCreature, CreatureId, EntityId, IdIssuer, WorldCoordinate};
use crate::FloatExt;
use crate::creature::abstract_ai::kinds::{
    BigEelAbstractAi, CicadaAbstractAi, DeerAbstractAi, MirosBirdAbstractAi, NeedleWormAbstractAi,
    VultureAbstractAi,
};
use crate::room_script::StorySession;
use crate::world::World;

/// The kind of an object a creature carries (only its ID matters so far: each one
/// takes a `GetNewID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Spear { explosive: bool },
    ScavengerBomb,
    GraffitiBomb,
    Lantern,
    FirecrackerPlant,
    SporePlant,
    Rock,
    DangleFruit,
    DataPearl,
    KarmaFlower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarriedItem {
    pub kind: ItemKind,
    pub id: EntityId,
}

/// The `AbstractCreatureAI` subclass and its own fields.
#[derive(Debug, Clone, PartialEq)]
pub enum AiKind {
    Plain,
    Overseer(OverseerAbstractAi),
    Vulture(VultureAbstractAi),
    Scavenger { carry_rocks: i32, dont_migrate: i32 },
    NeedleWorm(NeedleWormAbstractAi),
    DropBug,
    Cicada(CicadaAbstractAi),
    BigEel(BigEelAbstractAi),
    Deer(DeerAbstractAi),
    MirosBird(MirosBirdAbstractAi),
}

/// `AbstractCreatureAI`.
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractCreatureAi {
    pub destination: WorldCoordinate,
    pub migration_destination: Option<WorldCoordinate>,
    pub last_room: i32,
    pub time_buffer: i32,
    /// `privDenPos` (the `denPosition` getter answers the off-screen den for creatures
    /// that hibernate off-screen).
    pub den_position: Option<WorldCoordinate>,
    /// `RealAI != null`: the realized AI was built and not dropped (an overseer's has no
    /// path finder, so it outlives abstraction).
    pub real_ai: bool,
    /// `path`: the next step last, the destination first.
    pub path: Vec<WorldCoordinate>,
    pub stranded_in_room: i32,
    pub follow_creature: Option<CreatureId>,
    pub freeze_destination: bool,
    pub kind: AiKind,
}

/// What an AI constructor reads and draws from.
pub(crate) struct AiContext<'a> {
    pub world: &'a World,
    pub session: &'a StorySession,
    pub ids: &'a mut IdIssuer,
    pub rng: &'a mut UnityRandom,
}

impl AbstractCreatureAi {
    fn base(parent: &AbstractCreature) -> Self {
        Self {
            destination: parent.pos,
            migration_destination: None,
            last_room: parent.pos.room,
            time_buffer: 0,
            den_position: None,
            real_ai: false,
            path: Vec::new(),
            stranded_in_room: -1,
            follow_creature: None,
            freeze_destination: false,
            kind: AiKind::Plain,
        }
    }

    /// `DoIwantToDropThisItemInDen`: a scavenger keeps everything, a drop bug only
    /// creatures (its item stays), every other AI drops all.
    #[must_use]
    pub fn wants_to_drop_in_den(&self, item_is_creature: bool) -> bool {
        match self.kind {
            AiKind::Scavenger { .. } => false,
            AiKind::DropBug => item_is_creature,
            _ => true,
        }
    }

    /// The constructor `AbstractCreature` picks by type, with its draws. Carried items
    /// and `ignoreCycle` are set on `parent`.
    pub(crate) fn new(parent: &mut AbstractCreature, cx: &mut AiContext<'_>) -> Self {
        let mut ai = Self::base(parent);
        let kind = parent.kind();
        ai.kind = match kind {
            CreatureType::Vulture | CreatureType::KingVulture => {
                let mut vulture = VultureAbstractAi {
                    check_rooms: Vec::new(),
                    time_in_room: 0,
                    dont_go_to_this_room: -1,
                };
                cx.world
                    .vulture_add_random_check_room(parent.pos, kind, &mut vulture, cx.rng);
                AiKind::Vulture(vulture)
            }
            CreatureType::CicadaA | CreatureType::CicadaB => {
                AiKind::Cicada(CicadaAbstractAi::default())
            }
            CreatureType::BigEel => AiKind::BigEel(BigEelAbstractAi {
                go_to: None,
                last_room: parent.pos.room,
                time_in_room: 0,
            }),
            CreatureType::Deer => AiKind::Deer(DeerAbstractAi::new(parent, cx.world)),
            CreatureType::MirosBird => AiKind::MirosBird(MirosBirdAbstractAi::new(cx.world)),
            CreatureType::Scavenger => Self::scavenger(parent, cx),
            CreatureType::Overseer => AiKind::Overseer(OverseerAbstractAi::new(parent, cx)),
            CreatureType::BigNeedleWorm | CreatureType::SmallNeedleWorm => {
                let migrate_active = cx.rng.value() < 0.5;
                let mother = if kind == CreatureType::SmallNeedleWorm {
                    cx.world
                        .find_needle_mother(parent.pos.room, parent.pos, None, cx.rng)
                } else {
                    None
                };
                AiKind::NeedleWorm(NeedleWormAbstractAi {
                    mother,
                    migrate_active,
                    migrate_active_counter: 0,
                    temp_suppress_follow_creature: false,
                })
            }
            CreatureType::DropBug => {
                Self::drop_bug_item(parent, cx);
                AiKind::DropBug
            }
            _ => AiKind::Plain,
        };
        ai
    }

    /// The `denPosition` getter.
    #[must_use]
    pub fn den_position(
        &self,
        parent: &AbstractCreature,
        offscreen_den: i32,
    ) -> Option<WorldCoordinate> {
        if parent.template.hibernate_off_screen {
            Some(WorldCoordinate::new(offscreen_den, -1, -1, 0))
        } else {
            self.den_position
        }
    }

    /// `WantToMigrate`.
    #[must_use]
    pub fn want_to_migrate(&self, pos: WorldCoordinate) -> bool {
        self.migration_destination
            .is_some_and(|m| m.room != pos.room)
    }

    /// The `MigrationDestination` getter of a creature at `pos`.
    #[must_use]
    pub fn migration_target(&self, pos: WorldCoordinate) -> WorldCoordinate {
        match self.migration_destination {
            Some(m) if m.room != pos.room => m,
            _ => self.destination,
        }
    }

    /// `DoIHaveAPathToCoordinate` of a creature at `pos`.
    #[must_use]
    pub fn has_path_to(&self, dest: WorldCoordinate, pos: WorldCoordinate) -> bool {
        self.path
            .first()
            .map_or(dest.same_room_and_node(pos), |&p| {
                dest.same_room_and_node(p)
            })
    }

    /// `SetDestinationNoPathing`.
    pub fn set_destination_no_pathing(&mut self, dest: WorldCoordinate, migrate: bool) {
        self.destination = dest;
        if migrate {
            self.migration_destination = Some(dest);
        }
    }

    /// `TimeInfluencedRandomRoll`: up to `time` draws.
    pub fn time_influenced_roll(stat: f32, time: i32, rng: &mut UnityRandom) -> bool {
        (0..time).any(|_| rng.value() < stat)
    }

    /// `DropBugAbstractAI`'s constructor: a quarter of drop bugs carry an item.
    fn drop_bug_item(parent: &mut AbstractCreature, cx: &mut AiContext<'_>) {
        let world = cx.world;
        if world.single_room() || cx.rng.value() >= 0.25 {
            return;
        }
        if world
            .abstract_room(parent.pos.room)
            .is_none_or(|r| r.shelter())
        {
            return;
        }
        let rng = &mut *cx.rng;
        let kind = if rng.value() < 0.5 {
            Some(ItemKind::DangleFruit)
        } else if rng.value() < 0.5 {
            let id = cx.ids.new_id(-1);
            let explosive = rng.value() < 0.5;
            parent.carried.push(CarriedItem {
                kind: ItemKind::Spear { explosive },
                id,
            });
            None
        } else if rng.value() < 0.5 {
            Some(ItemKind::ScavengerBomb)
        } else if rng.value() < 0.5 {
            Some(ItemKind::DataPearl)
        } else if rng.value() < 0.5 && cx.session.character != rw_assets::SlugcatName::Red {
            Some(ItemKind::KarmaFlower)
        } else {
            None
        };
        if let Some(kind) = kind {
            let id = cx.ids.new_id(-1);
            parent.carried.push(CarriedItem { kind, id });
        }
    }

    /// `ScavengerAbstractAI`'s constructor: rocks to carry (seeded), gear when it starts
    /// in the off-screen den, then `dontMigrate`.
    fn scavenger(parent: &mut AbstractCreature, cx: &mut AiContext<'_>) -> AiKind {
        let seeded = cx.rng.seeded_value(parent.id.random_seed());
        let p = parent.personality;
        let carry_rocks = ((seeded.mathf_pow(2.5.lerp(0.5, p.aggression * (1.0 - p.dominance)))
            * 4.5) as i32)
            .clamp(0, 4);
        if cx.world.single_room() || cx.world.offscreen_den_index() == parent.pos.room {
            ScavengerGear {
                parent,
                carry_rocks,
            }
            .init_gear_up(cx);
        }
        let dont_migrate = cx.rng.range_i32(400, 4800);
        AiKind::Scavenger {
            carry_rocks,
            dont_migrate,
        }
    }
}

/// `UnityRandom` extensions the constructors use.
pub(crate) trait SeededRandom {
    /// `RainWorldGame.SeededRandom`: the first value of `seed`'s stream, the global state
    /// restored.
    fn seeded_value(&mut self, seed: i32) -> f32;
}

impl SeededRandom for UnityRandom {
    fn seeded_value(&mut self, seed: i32) -> f32 {
        let saved = self.state();
        self.init_state(seed);
        let v = self.value();
        self.set_state(saved);
        v
    }
}

/// `ScavengerAbstractAI.InitGearUp` (story: not under a saved state, so its draws are
/// global).
struct ScavengerGear<'p> {
    parent: &'p mut AbstractCreature,
    carry_rocks: i32,
}

impl ScavengerGear<'_> {
    fn init_gear_up(&mut self, cx: &mut AiContext<'_>) {
        let mut slots = 3;
        let (cycle, num3) = match cx.session.character {
            SlugcatName::Yellow => ((cx.session.cycle_number as f32 * 0.75).floor() as i32, 0.5),
            SlugcatName::Red => (cx.session.cycle_number + 60, 1.5),
            _ => (cx.session.cycle_number, 1.0),
        };
        let dominance = self.parent.personality.dominance;
        let v = cx.rng.value();
        let spears = ((v.mathf_pow(1.5.lerp(0.5, dominance.mathf_pow(3.0 - num3))) * (3.5 + num3))
            as i32)
            .clamp(0, 4);
        for _ in 0..spears {
            let id = cx.ids.new_id(-1);
            let explosive = self.is_spear_explosive(cycle, cx);
            self.carry(ItemKind::Spear { explosive }, id);
            slots -= 1;
        }
        let world = cx.world;
        let region = if world.single_room() {
            ""
        } else {
            world.region_name().unwrap_or("")
        };
        if slots >= 0 && world.properties().params.locust_region && cx.rng.value() < 0.6 {
            let id = cx.ids.new_id(-1);
            self.carry(ItemKind::GraffitiBomb, id);
            slots -= 1;
        }
        if slots >= 0 && cx.rng.value() < 0.6 && !world.single_room() {
            let lantern = match region {
                "SB" => cx.rng.value() < 0.7,
                "SH" => true,
                _ => false,
            };
            if lantern {
                let id = cx.ids.new_id(-1);
                self.carry(ItemKind::Lantern, id);
                slots -= 1;
            }
        }
        let bomb_gate = (cycle as f32).inverse_lerp(10.0, 60.0);
        if slots >= 0 && cx.rng.value().mathf_pow(num3) < bomb_gate {
            let v = cx.rng.value();
            let bombs = ((v.mathf_pow(1.5.lerp(0.5, dominance.mathf_pow(2.0) * bomb_gate)) * 2.5)
                as i32)
                .clamp(0, 2);
            for _ in 0..bombs {
                let id = cx.ids.new_id(-1);
                self.carry(ItemKind::ScavengerBomb, id);
                slots -= 1;
                if slots < 0 {
                    break;
                }
            }
        }
        if slots >= 0 && cx.rng.value() < 0.08 {
            let id = cx.ids.new_id(-1);
            self.carry(ItemKind::FirecrackerPlant, id);
            slots -= 1;
        }
        if slots >= 0
            && cx.rng.value().mathf_pow(num3) < (cycle as f32).inverse_lerp(40.0, 110.0)
            && cx.rng.value() < 1.0 / 12.0.lerp(3.0, dominance)
        {
            let id = cx.ids.new_id(-1);
            self.carry(ItemKind::SporePlant, id);
            slots -= 1;
        }
        if slots >= 0 {
            let rocks = cx.rng.range_i32(0, self.carry_rocks + 1);
            for _ in 0..rocks {
                let id = cx.ids.new_id(-1);
                self.carry(ItemKind::Rock, id);
                slots -= 1;
                if slots < 0 {
                    break;
                }
            }
        }
    }

    fn carry(&mut self, kind: ItemKind, id: EntityId) {
        self.parent.carried.push(CarriedItem { kind, id });
    }

    /// `IsSpearExplosive`. A new game's scavenger opinion of the player keeps the
    /// community term at 0 (`LikeOfPlayer` > -0.5).
    fn is_spear_explosive(&self, cycle: i32, cx: &mut AiContext<'_>) -> bool {
        let p = self.parent.personality;
        let mut num = p.dominance * ((p.aggression + p.bravery) / 2.0) * 0.75;
        let world = cx.world;
        match world.region_name().filter(|_| !world.single_room()) {
            Some(region) => {
                if region == "SU" {
                    num = num.mathf_pow(2.0) * 0.5;
                } else if region == "HI" || region == "DS" {
                    num *= 0.5;
                }
                num *= (cycle as f32).inverse_lerp(10.0, 30.0);
                if world.timeline().at_or_before(Timeline::Red) {
                    num = num.lerp(1.0, 0.25 * cx.rng.value());
                }
            }
            None => num = num.lerp(1.0, 0.15),
        }
        cx.rng.value() < num
    }
}
