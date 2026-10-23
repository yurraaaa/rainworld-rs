//! A room's `SocialEventRecognizer` and the AIs that react to it: `WeaponAttack`,
//! `Killing`'s event, `SocialEvent`'s delivery to the room's living creatures, the items
//! creatures put on the ground (`OwnedItemOnGround`, `ItemOffered`; their update reads
//! the player and lives in `world/social_events_turn.rs`), and `LizardAI.SocialEvent` with
//! `LizardPlayerRelationChange` (the lizard's social memory of the player and its
//! community's like of the player) and the gift half of taming (`FriendTracker.
//! ItemOffered`, `GiftRecieved`). Decompile: `SocialEventRecognizer.cs`,
//! `LizardAI.cs` 1765-1906, `FriendTracker.cs`.
//!
//! Mirrors: SocialEventRecognizer.cs, LizardAI.cs (SocialEvent, LizardPlayerRelationChange, GiftRecieved), FriendTracker.cs (ItemOffered, GiftRecieved)

use rw_assets::{AbstractObjectType, CreatureType};

use crate::FloatExt;
use crate::Vec2;
use crate::creature::{CommunityInfluence, CreatureId, GiftOffer, RelationshipType};
use crate::item::Entity;
use crate::item::ObjectId;
use crate::world::World;

/// `SocialEventRecognizer` of a realized room: the items creatures put on the ground.
/// `stolenProperty` is left out: only scavengers steal (`Theft`), and they are not
/// realized.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SocialEventRecognizer {
    pub owned_items_on_ground: Vec<OwnedItemOnGround>,
}

/// `SocialEventRecognizer.OwnedItemOnGround`; `serial` stands for the reference a
/// `giftOfferedToMe` keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnedItemOnGround {
    pub serial: u64,
    pub item: Entity,
    pub owner: CreatureId,
    pub offered_to: Option<CreatureId>,
    pub active: bool,
    pub age: i32,
    pub offered: bool,
}

impl OwnedItemOnGround {
    /// The age past which it lapses: 10 ticks, 30 once offered to someone, times 20
    /// for a living creature, else 40.
    #[must_use]
    pub fn lifetime(&self, living_creature: bool) -> i32 {
        let base = if self.offered_to.is_some() { 30 } else { 10 };
        base * if living_creature { 20 } else { 40 }
    }
}

impl SocialEventRecognizer {
    /// `CreaturePutItemOnGround`: a new owned item unless the item is listed already.
    pub fn put_item_on_ground(&mut self, serial: u64, item: Entity, owner: CreatureId) -> bool {
        if self.owned_items_on_ground.iter().any(|o| o.item == item) {
            return false;
        }
        self.owned_items_on_ground.push(OwnedItemOnGround {
            serial,
            item,
            owner,
            offered_to: None,
            active: true,
            age: 0,
            offered: false,
        });
        true
    }

    /// `ItemOwnership(item)`.
    #[must_use]
    pub fn item_ownership(&self, item: Entity) -> Option<&OwnedItemOnGround> {
        self.owned_items_on_ground.iter().find(|o| o.item == item)
    }

    /// `ItemOfferedTo(item)`.
    #[must_use]
    pub fn item_offered_to(&self, item: Entity) -> Option<CreatureId> {
        self.item_ownership(item).and_then(|o| o.offered_to)
    }
}

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
        self.social_event(room, id, thrower, victim, Some(Entity::Object(weapon)));
    }

    /// `CreaturePutItemOnGround` in realized room `room`: `owner` put `item` down (laid,
    /// tossed or dropped from the back).
    pub(crate) fn creature_put_item_on_ground(
        &mut self,
        room: i32,
        item: Entity,
        owner: CreatureId,
    ) {
        let serial = self.owned_item_serial + 1;
        let Some(r) = self.realized_mut(room) else {
            return;
        };
        if r.social_events.put_item_on_ground(serial, item, owner) {
            self.owned_item_serial = serial;
        }
    }

    /// `ItemOffered(gifter, item, null)` in room `room` (`item_pos` its `firstChunk`):
    /// unless the slugcat's item is stolen (nothing is), the closest living realized
    /// creature within 500 px of the item, other than the gifter, whose AI reacts to
    /// social events and has seen the gifter in the last 120 ticks (of the ported AIs,
    /// a lizard's) has the item offered to it (`ItemOffering`).
    pub(crate) fn item_offered(
        &mut self,
        room: i32,
        gifter: CreatureId,
        item: Entity,
        item_pos: Vec2,
    ) -> Option<CreatureId> {
        let mut dst = 500.0;
        let mut offered_to = None;
        for &c in self.room_creatures(room) {
            if c == gifter || self.realized_dead(c) {
                continue;
            }
            let Some(l) = self.creature(c).lizard() else {
                continue;
            };
            let danger = l.head().pos;
            if item_pos.distance(danger) >= dst {
                continue;
            }
            let t = &l.ai.tracker;
            if t.representation_for(gifter)
                .is_some_and(|r| r.ticks_since_seen(t.see_around_corners) < 120)
            {
                dst = item_pos.distance(danger);
                offered_to = Some(c);
            }
        }
        if let Some(to) = offered_to {
            self.social_event(room, SocialEventId::ItemOffering, gifter, to, Some(item));
        }
        offered_to
    }

    /// Whether the `OwnedItemOnGround` a gift refers to is still `active`.
    #[must_use]
    pub fn gift_active(&self, gift: &GiftOffer) -> bool {
        self.realized(gift.room).is_some_and(|r| {
            r.social_events
                .owned_items_on_ground
                .iter()
                .any(|o| o.serial == gift.serial && o.active)
        })
    }

    /// `FriendTracker.ItemOffered`: a creature lizard `me` eats, offered in a realized
    /// room, becomes the gift offered to it (that room's `ItemOwnership` of it).
    fn friend_item_offered(&mut self, me: CreatureId, item: Option<Entity>) {
        let Some(Entity::Creature(c)) = item else {
            return;
        };
        let Some(room) = self.physical_room(c) else {
            return;
        };
        if self.dynamic_relationship(me, c).kind != RelationshipType::Eats {
            return;
        }
        let gift = self.realized(room).and_then(|r| {
            r.social_events
                .item_ownership(Entity::Creature(c))
                .map(|o| GiftOffer {
                    room,
                    serial: o.serial,
                    item: o.item,
                    owner: o.owner,
                })
        });
        self.lizard_at(me).ai.friend.gift_offered_to_me = gift;
    }

    /// `FriendTracker.Update`'s gift part: a gift no longer active is forgotten; one the
    /// lizard (in a room) holds is received.
    pub(crate) fn friend_tracker_update(&mut self, me: CreatureId) {
        let Some(gift) = self.lizard_ref(me).ai.friend.gift_offered_to_me else {
            return;
        };
        if !self.gift_active(&gift) {
            self.lizard_at(me).ai.friend.gift_offered_to_me = None;
            return;
        }
        if self.lizard_ref(me).room.is_none() {
            return;
        }
        let holds = self
            .creature(me)
            .grasps
            .iter()
            .flatten()
            .any(|g| g.grabbed == gift.item);
        if holds {
            self.friend_gift_received(me);
        }
    }

    /// `FriendTracker.GiftRecieved`: an active gift is received (`LizardAI.
    /// GiftRecieved`) and forgotten.
    pub(crate) fn friend_gift_received(&mut self, me: CreatureId) {
        let Some(gift) = self.lizard_ref(me).ai.friend.gift_offered_to_me else {
            return;
        };
        if !self.gift_active(&gift) {
            return;
        }
        self.lizard_gift_received(me, gift);
        self.lizard_at(me).ai.friend.gift_offered_to_me = None;
    }

    /// `LizardAI.GiftRecieved`: unless it dislikes the giver beyond -0.9, the lizard
    /// likes it more (0.6, a dead creature 1.2) and for now more still (0.9, dead 1.7),
    /// both over its taming difficulty; a slugcat's gift raises its community's like of
    /// the player (0.1, bleeding 0.2 across regions and 0.1 into all communities). Rot
    /// (Watcher), the Challenge taming multiplier (More Slugcats) and the Remix love
    /// sound are not ported.
    fn lizard_gift_received(&mut self, me: CreatureId, gift: GiftOffer) {
        log::info!("Lizard recieved gift!");
        let dead = match gift.item {
            Entity::Creature(c) => self.realized_dead(c),
            Entity::Object(_) => false,
        };
        let taming = self.lizard_ref(me).ai.friend.taming_difficulty;
        let owner = self.creature(gift.owner).id;
        if let Some(m) = &mut self.creature_mut(me).state.social_memory {
            let r = m.get_or_initiate(owner);
            if r.like > -0.9 {
                r.influence_like(if dead { 1.2 } else { 0.6 } / taming);
                r.influence_temp_like(if dead { 1.7 } else { 0.9 } / taming);
            }
        }
        if self.creature(gift.owner).kind() == CreatureType::Slugcat {
            let community = self.creature(me).template.violence().community;
            self.influence_and_record(CommunityInfluence {
                community,
                influence: 0.1,
                inter_region_bleed: 0.2,
                inter_community_bleed: 0.1,
            });
        }
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
        item: Option<Entity>,
    ) {
        for c in self.room_creatures(room).to_vec() {
            if self.creature(c).lizard().is_some() && !self.realized_dead(c) {
                self.lizard_social_event(c, id, subject, object, item);
            }
        }
    }

    /// `LizardAI.SocialEvent`: a slugcat it tracks attacking it, or attacking a creature
    /// it tracks (with one of the two seen in the last 40 ticks), changes how it and its
    /// community like the slugcat: by the attack's offence (a third on the dead), all of
    /// it when it is the victim, three quarters when the victim is of its pack; an attack
    /// on a creature it is afraid of makes it like the slugcat more. A gift offered to
    /// it goes to its friend tracker (`ItemOffered`). A friend's attacks (halved, the
    /// attempts ignored) are taming's: no lizard has a friend.
    fn lizard_social_event(
        &mut self,
        me: CreatureId,
        id: SocialEventId,
        subject: CreatureId,
        object: CreatureId,
        item: Option<Entity>,
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
            if victim_is_me {
                self.friend_item_offered(me, item);
            }
            return;
        }
        let mut offence = id.lizard_offence();
        if self.realized_dead(object) {
            offence /= 3.0;
        }
        match victim_rel.map(|r| r.kind) {
            None => self.lizard_player_relation_change(me, -offence, subject),
            Some(RelationshipType::Afraid) => {
                let (change, fac) = self.lizard_fear_relief(me, id, object, offence);
                self.lizard_player_relation_change(me, change.mathf_pow(0.5) * fac, subject);
            }
            Some(RelationshipType::Pack) => {
                self.lizard_player_relation_change(me, -offence * 0.75, subject);
            }
            Some(_) => {}
        }
    }

    /// `LizardAI.SocialEvent`'s fear branch: the offence and its factor for an attack
    /// on a creature lizard `me` is afraid of: 0.1, plus `0.7 * LerpMap(distance to its
    /// DangerPos, 120, 320, 1, 0.1)` while it is a threat creature; 2 while it holds
    /// the lizard (a hit then counting fully).
    pub(crate) fn lizard_fear_relief(
        &self,
        me: CreatureId,
        id: SocialEventId,
        object: CreatureId,
        offence: f32,
    ) -> (f32, f32) {
        let l = self.lizard_ref(me);
        let mut fac = 0.1;
        let threat = l.ai.threat.creatures.iter().any(|c| {
            l.ai.tracker
                .rep(c.rep)
                .is_some_and(|r| r.creature == object)
        });
        if threat
            && let Some(danger) = self
                .creature(object)
                .realized
                .as_ref()
                .and_then(crate::creature::RealizedCreature::danger_pos)
        {
            fac += 0.7
                * l.head()
                    .pos
                    .distance(danger)
                    .lerp_map(120.0, 320.0, 1.0, 0.1);
        }
        let holds_me = self
            .creature(object)
            .grasps
            .iter()
            .flatten()
            .any(|g| g.grabbed == Entity::Creature(me));
        let mut offence = offence;
        if holds_me {
            if matches!(
                id,
                SocialEventId::NonLethalAttack | SocialEventId::LethalAttack
            ) {
                offence = 1.0;
            }
            fac = 2.0;
        }
        (offence, fac)
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
#[path = "tests/social_events.rs"]
mod tests;
