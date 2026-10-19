//! A creature's kill tag and death, the parts every creature shares: `SetKillTag` and
//! the tag's countdown in `Creature.Update` (77079), `Creature.Die` (77465) with the
//! room's `SocialEventRecognizer.Killing` (20500: the player's `AddKill` and the
//! communities' like of the player), and `AbstractCreature.Die` (301418) with
//! `SaveState.AddCreatureToRespawn` (48297: a lineage's `ChanceToProgress`). What the
//! session keeps of it is [`SessionDeaths`]. Behaviour: `rainworld-re/notes/items.md`
//! section 12.
//!
//! Mirrors: Creature.cs (SetKillTag, Update, Die), SocialEventRecognizer.cs (Killing), AbstractCreature.cs (Die),
//!   SaveState.cs (AddCreatureToRespawn)

use rw_assets::CreatureType;
use rw_assets::creatures::CreatureSpawner;
use rw_random::UnityRandom;

use super::{Community, CreatureId, EntityId, RealizedCreature};
use crate::world::{SocialEventId, World};

/// `PlayerSessionRecord.KillRecord`: the kill's `IconSymbolData` (its type and
/// `intData`), the victim's ID and `Template.IsLizard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKill {
    pub kind: CreatureType,
    /// `CreatureSymbol.SymbolDataFromCreature`'s `intData` (0 for the base-game types
    /// the port realizes).
    pub int_data: i32,
    pub id: EntityId,
    pub lizard: bool,
}

/// One `CreatureCommunities.InfluenceLikeOfPlayer` call of the session (in its
/// region, `world.RegionNumber`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommunityInfluence {
    pub community: Community,
    pub influence: f32,
    pub inter_region_bleed: f32,
    pub inter_community_bleed: f32,
}

/// What a story session keeps of its deaths until the cycle ends: the first player's
/// `PlayerSessionRecord` kills and `peaceful`, what `AddCreatureToRespawn` added to the
/// save state's respawn lists, and the community influences (all applied to the save
/// at `Win`).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionDeaths {
    /// Spawners added to `saveState.respawnCreatures` this cycle.
    pub respawn_creatures: Vec<i32>,
    /// Spawners added to `saveState.waitRespawnCreatures` this cycle.
    pub wait_respawn_creatures: Vec<i32>,
    pub kills: Vec<SessionKill>,
    pub peaceful: bool,
    pub influences: Vec<CommunityInfluence>,
}

impl Default for SessionDeaths {
    fn default() -> Self {
        Self {
            respawn_creatures: Vec::new(),
            wait_respawn_creatures: Vec::new(),
            kills: Vec::new(),
            peaceful: true,
            influences: Vec::new(),
        }
    }
}

impl World {
    /// `Creature.SetKillTag(killer)`: a living creature (with health above 0) is tagged
    /// for at least 200 ticks.
    pub(crate) fn set_kill_tag(&mut self, victim: CreatureId, killer: CreatureId) {
        let c = self.creature_mut(victim);
        if !c.state.alive || (c.state.has_health() && c.state.health <= 0.0) {
            return;
        }
        c.kill_tag_counter = c.kill_tag_counter.max(200);
        c.kill_tag = Some(killer);
    }

    /// `Creature.Update`'s kill tag countdown: the tag is forgotten when it runs out
    /// (it stays on a dead creature).
    pub(crate) fn kill_tag_tick(&mut self, id: CreatureId) {
        let c = self.creature_mut(id);
        if c.kill_tag_counter > 0
            && c.state.alive
            && (!c.state.has_health() || c.state.health > 0.0)
        {
            c.kill_tag_counter -= 1;
            if c.kill_tag_counter < 1 {
                c.kill_tag = None;
            }
        }
    }

    /// `Creature.Stun(st)` of a realized fly or overseer: the stun is at least `st`.
    pub(crate) fn stun_creature(&mut self, id: CreatureId, st: i32) {
        match &mut self.creature_mut(id).realized {
            Some(RealizedCreature::Fly(f)) => f.stun = f.stun.max(st),
            Some(RealizedCreature::Overseer(b)) => b.stun = b.stun.max(st),
            Some(RealizedCreature::Lizard(l)) => l.stun = l.stun.max(st),
            _ => {}
        }
    }

    /// Whether the realized creature `id` is dead (`Creature.dead`).
    #[must_use]
    pub fn realized_dead(&self, id: CreatureId) -> bool {
        let c = self.creature(id);
        match &c.realized {
            Some(RealizedCreature::Fly(f)) => f.dead,
            Some(RealizedCreature::Overseer(b)) => b.dead,
            Some(RealizedCreature::Lizard(l)) => l.dead,
            Some(RealizedCreature::Player) | None => !c.state.alive,
        }
    }

    /// `Creature.Die` after the type's own part (`Fly.Die`'s surface friction,
    /// `Overseer.Die`'s guide): a first death by a realized killer is a `Killing` in the
    /// room; then `dead`, every grasp let go, and `AbstractCreature.Die`.
    pub(crate) fn creature_die(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        if !self.realized_dead(id)
            && let Some(killer) = self.creature(id).kill_tag
            && self.creature(killer).is_realized()
        {
            self.killing(killer, id);
        }
        match &mut self.creature_mut(id).realized {
            Some(RealizedCreature::Fly(f)) => f.dead = true,
            Some(RealizedCreature::Overseer(b)) => b.dead = true,
            Some(RealizedCreature::Lizard(l)) => l.dead = true,
            _ => {}
        }
        self.lose_all_grasps(id);
        self.abstract_die(id, rng);
    }

    /// `SocialEventRecognizer.Killing`: the social event in the victim's room (the victim
    /// is not dead yet), then a slugcat killer lowers the victim's community's like of
    /// the player (`-0.05 * communityInfluence` in this region, bleeding 0.25 to the
    /// region-wide cell) and adds the kill to the session record.
    fn killing(&mut self, killer: CreatureId, victim: CreatureId) {
        let room = self.creature(victim).pos.room;
        self.social_event(room, SocialEventId::Killing, killer, victim);
        if self.creature(killer).kind() != CreatureType::Slugcat {
            return;
        }
        let v = self.creature(victim).template.violence();
        if v.community != Community::None {
            self.influence_and_record(CommunityInfluence {
                community: v.community,
                influence: -0.05 * v.community_influence,
                inter_region_bleed: 0.25,
                inter_community_bleed: 0.0,
            });
        }
        if self.story {
            self.add_kill(victim);
        }
    }

    /// `CreatureCommunities.InfluenceLikeOfPlayer` on the session's communities, kept to
    /// be applied to the saved ones at `Win` (one of 0 changes nothing and is not kept).
    pub(crate) fn influence_and_record(&mut self, i: CommunityInfluence) {
        self.influence_like_of_player(i);
        if i.influence != 0.0 {
            self.session.influences.push(i);
        }
    }

    /// `CreatureCommunities.InfluenceLikeOfPlayer` in the world's region, on the
    /// session's communities.
    fn influence_like_of_player(&mut self, i: CommunityInfluence) {
        let (Some(community), Some(region)) = (
            rw_save::CommunityId::from_value(i.community.value()),
            self.region_name().map(str::to_owned),
        ) else {
            return;
        };
        let i = rw_save::CommunityInfluence {
            community,
            influence: i.influence,
            inter_region_bleed: i.inter_region_bleed,
            inter_community_bleed: i.inter_community_bleed,
        };
        self.communities.influence_like_of_player(i, &region);
    }

    /// `PlayerSessionRecord.BreakPeaceful`: a victim that counts as a kill ends the
    /// peaceful streak.
    pub(crate) fn break_peaceful(&mut self, victim: CreatureId) {
        if self.creature(victim).template.violence().counts_as_a_kill > 0 {
            self.session.peaceful = false;
        }
    }

    /// `PlayerSessionRecord.AddKill`: `BreakPeaceful`, then the victim once by ID.
    fn add_kill(&mut self, victim: CreatureId) {
        self.break_peaceful(victim);
        let c = self.creature(victim);
        if self.session.kills.iter().any(|k| k.id == c.id) {
            return;
        }
        log::info!("player kill added: {} {:?}", c.kind().value(), c.id);
        self.session.kills.push(SessionKill {
            kind: c.kind(),
            int_data: 0,
            id: c.id,
            lizard: c.template.is_lizard(),
        });
    }

    /// `AbstractCreature.Die`: a living creature from a spawner goes on the respawn
    /// lists (story only), then the state dies (`abstractAI.Die` does nothing in the
    /// base game).
    pub(crate) fn abstract_die(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        if c.state.alive && c.id.spawner >= 0 && self.story {
            self.add_creature_to_respawn(id, rng);
        }
        self.creature_mut(id).state.alive = false;
    }

    /// `SaveState.AddCreatureToRespawn`: killed by the slugcat, a lineage may progress
    /// (`ChanceToProgress`, one draw when below its last stage) and the spawner waits
    /// for a later cycle; otherwise it respawns next cycle.
    fn add_creature_to_respawn(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        let spawner = c.id.spawner;
        log::info!("CRITTER TO RESPAWN : {} {:?}", c.kind().value(), c.id);
        let by_player = c.is_realized()
            && c.kill_tag
                .is_some_and(|k| self.creature(k).kind() == CreatureType::Slugcat);
        if by_player {
            if let Some(CreatureSpawner::Lineage(l)) = self.spawner(c.id).cloned() {
                let den = self.lineage_den_string(&l);
                self.lineage_chance_to_progress(&l, &den, rng);
            }
            self.session.wait_respawn_creatures.push(spawner);
        } else {
            self.session.respawn_creatures.push(spawner);
        }
    }

    /// `World.GetSpawner(ID)`: the region's spawner `ID.spawner % 1000`.
    #[must_use]
    pub fn spawner(&self, id: EntityId) -> Option<&CreatureSpawner> {
        if id.spawner < 0 || self.single_room() {
            return None;
        }
        let index = id.spawner % 1000;
        let region = (id.spawner - index) / 1000;
        let number = self.data.region.as_ref().map(|r| r.region_number)?;
        if region != number {
            return None;
        }
        self.data.spawners.get(usize::try_from(index).ok()?)
    }
}
