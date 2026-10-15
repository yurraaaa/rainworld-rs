//! `WorldLoader.GeneratePopulation(fresh: true)` for a new game: the `CREATURES`
//! spawners in file order, the player guide (`OverseerSpawnConditions`), the region's
//! overseers; then the `FliesWorldAI` constructor's bat flies. Every random draw is the
//! game's, in its order (`rainworld-re/notes/creature-model.md`).

use rw_assets::creatures::{CreatureSpawner, Lineage, SimpleSpawner};
use rw_assets::{CreatureType, NodeType, SlugcatName};
use rw_random::UnityRandom;

use super::{CreatureTemplate, EntityId, IdIssuer, PlayerGuideState, WorldCoordinate};
use crate::FloatExt;
use crate::room_script::StorySession;
use crate::world::{FliesTick, FliesWorldAi, SwarmBehavior, World};

/// What population generation reads of the session.
pub(crate) struct PopulationContext<'a> {
    pub session: &'a StorySession,
    pub ids: &'a mut IdIssuer,
    pub rng: &'a mut UnityRandom,
    /// `saveState.guideOverseerDead`.
    pub guide_overseer_dead: bool,
    pub guide: &'a PlayerGuideState,
    /// `foodToHibernate` of the slugcat (SU's third swarm room gets one more bat).
    pub food_to_hibernate: i32,
}

impl World {
    /// `GeneratePopulation(fresh: true)` (story, not a single-room world).
    pub(crate) fn generate_population(&mut self, cx: &mut PopulationContext<'_>) {
        let region = self.data.region.as_ref().map_or(0, |r| r.region_number);
        let spawners = self.data.spawners.clone();
        for (i, spawner) in spawners.iter().enumerate() {
            let spawner_id = region * 1000 + i32::try_from(i).unwrap_or(0);
            match spawner {
                CreatureSpawner::Simple(s) => self.spawn_simple(s, spawner_id, cx),
                CreatureSpawner::Lineage(l) => self.spawn_lineage(l, spawner_id, cx),
            }
        }
        self.spawn_player_guide(cx);
        self.spawn_region_overseers(cx);
    }

    /// Whether a spawner's den is a Den or GarbageHoles node of an existing room.
    fn valid_den(&self, room: i32, node: i32) -> bool {
        self.abstract_room(room).is_some_and(|r| {
            usize::try_from(node)
                .ok()
                .and_then(|n| r.nodes.get(n))
                .is_some_and(|n| matches!(n.kind, NodeType::Den | NodeType::GarbageHoles))
        })
    }

    fn spawn_simple(&mut self, s: &SimpleSpawner, spawner_id: i32, cx: &mut PopulationContext<'_>) {
        if s.amount <= 0 || !self.valid_den(s.den.room, s.den.node) {
            return;
        }
        let template = CreatureTemplate::of(s.creature, self.mods);
        if template.quantified {
            self.add_quantified(s.den.room, s.den.node, s.creature, s.amount, cx.rng);
            return;
        }
        for _ in 0..s.amount {
            let id = cx.ids.new_id(spawner_id);
            let den = WorldCoordinate::new(s.den.room, -1, -1, s.den.node);
            self.spawn_into_den(template, den, id, s.spawn_data.clone(), false, cx);
        }
    }

    /// A lineage at its first stage (a new game's counter 0): NONE rolls
    /// `ChanceToProgress` instead of spawning.
    fn spawn_lineage(&mut self, l: &Lineage, spawner_id: i32, cx: &mut PopulationContext<'_>) {
        match l.creatures.first().copied().flatten() {
            None => {
                if l.progression_chances.len() > 1 && cx.rng.value() < l.progression_chances[0] {
                    log::debug!("lineage {spawner_id} progressed");
                }
            }
            Some(kind) => {
                if !self.valid_den(l.den.room, l.den.node) {
                    return;
                }
                let template = CreatureTemplate::of(kind, self.mods);
                let id = cx.ids.new_id(spawner_id);
                let data = l.spawn_data.first().cloned().flatten();
                let den = WorldCoordinate::new(l.den.room, -1, -1, l.den.node);
                self.spawn_into_den(template, den, id, data, l.night_creature, cx);
            }
        }
    }

    fn spawn_into_den(
        &mut self,
        template: CreatureTemplate,
        pos: WorldCoordinate,
        id: EntityId,
        spawn_data: Option<String>,
        night: bool,
        cx: &mut PopulationContext<'_>,
    ) {
        let mut c = self.new_creature(template, pos, id, cx.session, cx.ids, cx.rng);
        c.spawn_data = spawn_data;
        c.night_creature = night;
        let cid = self.insert_creature(c);
        if template.kind == CreatureType::BigNeedleWorm {
            self.small_worms_find_mother(cid, cx.rng);
        }
        self.move_entity_to_den(cid);
    }

    /// A big needle worm's constructor asks the small ones out in its room (not in dens)
    /// to look for their mother again.
    fn small_worms_find_mother(&mut self, big: crate::creature::CreatureId, rng: &mut UnityRandom) {
        let room = self.creature(big).pos.room;
        let smalls: Vec<_> = self
            .room_creatures(room)
            .iter()
            .copied()
            .filter(|&c| self.creature(c).kind() == CreatureType::SmallNeedleWorm)
            .collect();
        for small in smalls {
            let c = self.creature(small);
            let den =
                c.ai.as_ref()
                    .and_then(|a| a.den_position(c, self.offscreen_den_index()));
            let mother = self.find_needle_mother(c.pos, den, rng);
            if let Some(ai) = &mut self.creature_mut(small).ai
                && let crate::creature::AiKind::NeedleWorm { mother: m, .. } = &mut ai.kind
            {
                *m = mother;
            }
        }
    }

    /// `OverseerSpawnConditions` and the guide: SU_C04 (137, 17) in SU, else the
    /// off-screen den; ID (-1, 5).
    fn spawn_player_guide(&mut self, cx: &mut PopulationContext<'_>) {
        if self.data.region.is_none() {
            return;
        }
        let chance = self
            .data
            .properties
            .params
            .player_guide_overseer_spawn_chance;
        let roll = cx.rng.value() < chance;
        if cx.session.character == SlugcatName::Red {
            return;
        }
        if cx.guide_overseer_dead || cx.guide.angry_with_player || !roll {
            return;
        }
        let den = self.offscreen_den_index();
        let pos = match self.region_name() {
            Some("SU") => match self.room_index("SU_C04") {
                Some(room) => WorldCoordinate::new(room, 137, 17, 0),
                None => WorldCoordinate::new(den, -1, -1, 0),
            },
            _ => WorldCoordinate::new(den, -1, -1, 0),
        };
        let template = CreatureTemplate::of(CreatureType::Overseer, self.mods);
        let c = self.new_creature(
            template,
            pos,
            EntityId::new(-1, 5),
            cx.session,
            cx.ids,
            cx.rng,
        );
        let id = self.insert_creature(c);
        if pos.room == den {
            self.add_to_dens(id);
        } else {
            self.add_entity(id);
        }
        if let Some(ai) = &mut self.creature_mut(id).ai
            && let crate::creature::AiKind::Overseer(o) = &mut ai.kind
        {
            o.set_as_player_guide(1);
        }
        self.set_player_guide(id);
    }

    /// The region's overseers: always in UW, else by chance from the cycle number
    /// (Hunter counts 17 more), `Range(overseersMin, overseersMax)` of them in the
    /// off-screen den.
    fn spawn_region_overseers(&mut self, cx: &mut PopulationContext<'_>) {
        let params = &self.data.properties.params;
        let (chance, min, max) = (
            params.overseers_spawn_chance,
            params.overseers_min,
            params.overseers_max,
        );
        let red = cx.session.character == SlugcatName::Red;
        let cycle = cx.session.cycle_number + if red { 17 } else { 0 };
        let spawn = self.region_name() == Some("UW")
            || cx.rng.value() < chance * (cycle as f32).inverse_lerp(2.0, 21.0);
        if !spawn {
            return;
        }
        let count = cx.rng.range_i32(min, max);
        let den = self.offscreen_den_index();
        let template = CreatureTemplate::of(CreatureType::Overseer, self.mods);
        for _ in 0..count {
            let id = cx.ids.new_id(-1);
            let c = self.new_creature(
                template,
                WorldCoordinate::new(den, -1, -1, 0),
                id,
                cx.session,
                cx.ids,
                cx.rng,
            );
            let cid = self.insert_creature(c);
            self.add_to_dens(cid);
        }
    }

    /// `new FliesWorldAI(world)`: a migration target per swarm room, every behaviour
    /// Spread; then (creatures spawning, not a single-room world) the bats to spawn per
    /// active swarm room (inactive ones go to the respawn count), SU's third swarm room
    /// `foodToHibernate + 1` in a story, and in a new game's first cycle every one added at
    /// once (`AddFlyToSwarmRoom`).
    pub(crate) fn create_flies_world_ai(&mut self, cx: &mut PopulationContext<'_>) {
        let swarm_rooms = self.data.swarm_rooms.clone();
        let n = i32::try_from(swarm_rooms.len()).unwrap_or(i32::MAX);
        let targets: Vec<i32> = swarm_rooms.iter().map(|_| cx.rng.range_i32(0, n)).collect();
        self.processes.flies = Some(FliesWorldAi {
            migration_targets: targets,
            swarm_room_behaviors: vec![SwarmBehavior::Spread; swarm_rooms.len()],
            flies_to_spawn: vec![0; swarm_rooms.len()],
            flies_to_respawn: 0,
            all_initial_flies_spawned: false,
        });
        // The loader's swarm map comes with the process; it draws nothing.
        self.map_swarm_rooms();
        if self.single_room() {
            return;
        }
        let params = &self.data.properties.params;
        let (active_bats, inactive_bats) = (
            params.bats_per_active_swarm_room,
            params.bats_per_inactive_swarm_room,
        );
        let mut to_spawn = vec![0; swarm_rooms.len()];
        let mut to_respawn = 0;
        for (k, spawn) in to_spawn.iter_mut().enumerate() {
            if self
                .region_state
                .as_ref()
                .is_none_or(|s| s.swarm_room_active(&self.data, k))
            {
                *spawn = active_bats;
            } else {
                to_respawn += inactive_bats;
            }
        }
        if self.region_name() == Some("SU")
            && self.story
            && let Some(third) = to_spawn.get_mut(2)
        {
            *third = cx.food_to_hibernate + 1;
        }
        if let Some(f) = &mut self.processes.flies {
            f.flies_to_spawn.clone_from(&to_spawn);
            f.flies_to_respawn = to_respawn;
        }
        // `cycleStartUp` is on: only a story's first cycle spawns them all now.
        if cx.session.cycle_number > 0 {
            return;
        }
        let mut no_graphics = ();
        let mut tick = FliesTick {
            rng: &mut *cx.rng,
            ids: &mut *cx.ids,
            session: cx.session,
            observer: &mut no_graphics,
            camera_room: -1,
        };
        for (k, &count) in to_spawn.iter().enumerate() {
            for _ in 0..count {
                self.add_fly_to_swarm_room(k, &mut tick);
                if let Some(f) = &mut self.processes.flies {
                    f.flies_to_spawn[k] -= 1;
                }
            }
        }
    }
}
