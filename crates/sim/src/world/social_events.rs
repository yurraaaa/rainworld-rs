//! A room's `SocialEventRecognizer` events and the AIs that react to them:
//! `WeaponAttack`, `Killing`'s event, `SocialEvent`'s delivery to the room's living
//! creatures, and `LizardAI.SocialEvent` with `LizardPlayerRelationChange` (the lizard's
//! social memory of the player and its community's like of the player). Gifts
//! (`ItemOffering`, `OwnedItemOnGround`) belong to taming, which is not ported.
//! Decompile: `SocialEventRecognizer.cs`, `LizardAI.cs` 1765-1876.

use rw_assets::{AbstractObjectType, CreatureType};

use super::World;
use crate::creature::{CommunityInfluence, CreatureId, RelationshipType};
use crate::item::ObjectId;

/// `SocialEventRecognizer.EventID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialEventId {
    LethalAttackAttempt,
    LethalAttack,
    NonLethalAttackAttempt,
    NonLethalAttack,
    Theft,
    Killing,
    ItemOffering,
    ItemTransaction,
}

impl SocialEventId {
    const ALL: [Self; 8] = [
        Self::LethalAttackAttempt,
        Self::LethalAttack,
        Self::NonLethalAttackAttempt,
        Self::NonLethalAttack,
        Self::Theft,
        Self::Killing,
        Self::ItemOffering,
        Self::ItemTransaction,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::LethalAttackAttempt => "LethalAttackAttempt",
            Self::LethalAttack => "LethalAttack",
            Self::NonLethalAttackAttempt => "NonLethalAttackAttempt",
            Self::NonLethalAttack => "NonLethalAttack",
            Self::Theft => "Theft",
            Self::Killing => "Killing",
            Self::ItemOffering => "ItemOffering",
            Self::ItemTransaction => "ItemTransaction",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.value() == value)
    }

    /// `WeaponAttack`'s event: a spear's attack is lethal, a rock's not; an attempt
    /// unless it hit.
    #[must_use]
    pub fn weapon_attack(lethal: bool, hit: bool) -> Self {
        match (lethal, hit) {
            (true, true) => Self::LethalAttack,
            (true, false) => Self::LethalAttackAttempt,
            (false, true) => Self::NonLethalAttack,
            (false, false) => Self::NonLethalAttackAttempt,
        }
    }

    /// How much an attack on it (or what it fears) makes a lizard dislike the attacker.
    #[must_use]
    pub fn lizard_offence(self) -> f32 {
        match self {
            Self::NonLethalAttackAttempt => 0.05,
            Self::NonLethalAttack => 0.15,
            Self::LethalAttackAttempt => 0.35,
            Self::LethalAttack => 1.0,
            _ => 0.0,
        }
    }
}

impl World {
    /// `SocialEventRecognizer.WeaponAttack` in room `room`: weapon `weapon` thrown by
    /// `thrower` hit (or flew close by) `victim`. A slugcat's hit breaks the peaceful
    /// record.
    pub(crate) fn weapon_attack(
        &mut self,
        room: i32,
        weapon: ObjectId,
        thrower: CreatureId,
        victim: CreatureId,
        hit: bool,
    ) {
        let lethal = self.object(weapon).kind == AbstractObjectType::Spear;
        if self.creature(thrower).kind() == CreatureType::Slugcat && hit {
            self.break_peaceful(victim);
        }
        let id = SocialEventId::weapon_attack(lethal, hit);
        self.social_event(room, id, thrower, victim);
    }

    /// `SocialEventRecognizer.SocialEvent`: every living realized creature of room
    /// `room` whose AI reacts to social events hears of it (of the ported AIs, a
    /// lizard's).
    pub(crate) fn social_event(
        &mut self,
        room: i32,
        id: SocialEventId,
        subject: CreatureId,
        object: CreatureId,
    ) {
        for c in self.room_creatures(room).to_vec() {
            if self.creature(c).lizard().is_some() && !self.realized_dead(c) {
                self.lizard_social_event(c, id, subject, object);
            }
        }
    }

    /// `LizardAI.SocialEvent`: a slugcat it tracks attacking it, or attacking a creature
    /// it tracks (with one of the two seen in the last 40 ticks), changes how it and its
    /// community like the slugcat: by the attack's offence (a third on the dead), all of
    /// it when it is the victim, three quarters when the victim is of its pack. Fear for
    /// a victim it is afraid of reads the threat tracker (lizard step 7) and is not
    /// ported; a gift (`ItemOffering`) is taming's, also not ported.
    fn lizard_social_event(
        &mut self,
        me: CreatureId,
        id: SocialEventId,
        subject: CreatureId,
        object: CreatureId,
    ) {
        if self.creature(subject).kind() != CreatureType::Slugcat {
            return;
        }
        let see = self.lizard_ref(me).ai.tracker.see_around_corners;
        let t = &self.lizard_ref(me).ai.tracker;
        let Some(player_seen) = t
            .representation_for(subject)
            .map(|r| r.ticks_since_seen(see))
        else {
            return;
        };
        let victim_is_me = object == me;
        let mut victim_rel = None;
        if !victim_is_me {
            let Some(r) = t.representation_for(object) else {
                return;
            };
            if player_seen > 40 && r.ticks_since_seen(see) > 40 {
                return;
            }
            victim_rel = Some(
                r.current_relationship()
                    .unwrap_or_else(|| self.static_relationship(me, object)),
            );
        }
        if id == SocialEventId::ItemOffering {
            return;
        }
        let mut offence = id.lizard_offence();
        if self.realized_dead(object) {
            offence /= 3.0;
        }
        match victim_rel.map(|r| r.kind) {
            None => self.lizard_player_relation_change(me, -offence, subject),
            Some(RelationshipType::Pack) => {
                self.lizard_player_relation_change(me, -offence * 0.75, subject);
            }
            Some(_) => {}
        }
    }

    /// `LizardPlayerRelationChange`: the lizard's social memory of the player (temporary
    /// like by 1.5 times the change, like by 0.75 times, knowing by a quarter of its
    /// size) and its community's like of the player in this region (a twentieth,
    /// bleeding 0.25 across regions and 0.3 into all communities).
    pub(crate) fn lizard_player_relation_change(
        &mut self,
        me: CreatureId,
        change: f32,
        player: CreatureId,
    ) {
        let subject = self.creature(player).id;
        if let Some(m) = &mut self.creature_mut(me).state.social_memory {
            let r = m.get_or_initiate(subject);
            r.influence_temp_like(change * 1.5);
            r.influence_like(change * 0.75);
            r.influence_know(change.abs() * 0.25);
        }
        let community = self.creature(me).template.violence().community;
        self.influence_and_record(CommunityInfluence {
            community,
            influence: change * 0.05,
            inter_region_bleed: 0.25,
            inter_community_bleed: 0.3,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SocialEventRecognizer.cs` 128-149 and `LizardAI.cs` 1798-1818.
    #[test]
    fn weapon_attack_events() {
        assert_eq!(
            SocialEventId::weapon_attack(true, true),
            SocialEventId::LethalAttack
        );
        assert_eq!(
            SocialEventId::weapon_attack(true, false),
            SocialEventId::LethalAttackAttempt
        );
        assert_eq!(
            SocialEventId::weapon_attack(false, true),
            SocialEventId::NonLethalAttack
        );
        assert_eq!(
            SocialEventId::weapon_attack(false, false),
            SocialEventId::NonLethalAttackAttempt
        );
        assert_eq!(SocialEventId::NonLethalAttackAttempt.lizard_offence(), 0.05);
        assert_eq!(SocialEventId::NonLethalAttack.lizard_offence(), 0.15);
        assert_eq!(SocialEventId::LethalAttackAttempt.lizard_offence(), 0.35);
        assert_eq!(SocialEventId::LethalAttack.lizard_offence(), 1.0);
        assert_eq!(SocialEventId::Killing.lizard_offence(), 0.0);
        assert_eq!(
            SocialEventId::from_value("ItemTransaction"),
            Some(SocialEventId::ItemTransaction)
        );
    }
}
