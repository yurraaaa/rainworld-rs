//! The game's side of a room's `SocialEventRecognizer`: the slugcat putting an item on
//! the ground (`ReleaseObject`, `TossObject`, the back spear laid down) and
//! `SocialEventRecognizer.Update`, whose owned items read their owner (the player's
//! body is the game's). The recognizer and the lizards' reactions are in
//! `world/social_events.rs`.
//!
//! Mirrors: SocialEventRecognizer.cs (Update, OwnedItemOnGround.Update), Player.cs (ReleaseObject, TossObject, GrabUpdate)

use super::RainWorldGame;
use crate::Vec2;
use crate::creature::CreatureId;
use crate::item::Entity;
use crate::world::{OwnedItemOnGround, SocialEventId};

impl RainWorldGame {
    /// `room.socialEventRecognizer.CreaturePutItemOnGround(item, this)` of the player.
    pub(super) fn player_put_item_on_ground(&mut self, item: Entity) {
        let room = self.pos().room;
        self.world
            .creature_put_item_on_ground(room, item, self.player_id);
    }

    /// `SocialEventRecognizer.Update` of realized room `index`: from the newest, an
    /// inactive owned item is dropped, an active one updated.
    pub(super) fn social_event_recognizer_update(&mut self, index: i32) {
        let Some(n) = self
            .world
            .realized(index)
            .map(|r| r.social_events.owned_items_on_ground.len())
        else {
            return;
        };
        for i in (0..n).rev() {
            let Some(item) = self
                .world
                .realized(index)
                .and_then(|r| r.social_events.owned_items_on_ground.get(i).copied())
            else {
                continue;
            };
            if item.active {
                self.owned_item_update(index, i, item);
            } else if let Some(r) = self.world.realized_mut(index) {
                r.social_events.owned_items_on_ground.remove(i);
            }
        }
    }

    /// `OwnedItemOnGround.Update`: a slugcat's item is offered (`ItemOffered`) once its
    /// owner is 50 px from it; it lapses with age, when picked up (by the creature it
    /// was offered to: an `ItemTransaction`, and the gift that creature's friend
    /// tracker waits for is received) or when its owner leaves the room. An item with
    /// no body in the world (destroyed or abstract) is not offered (the game would
    /// measure its last position).
    fn owned_item_update(&mut self, room: i32, i: usize, mut o: OwnedItemOnGround) {
        o.age += 1;
        let item_pos = self.world.chunk_pos(o.item, 0);
        if self.world.creature(o.owner).kind() == rw_assets::CreatureType::Slugcat
            && !o.offered
            && let (Some(owner), Some(pos)) = (self.social_owner_danger_pos(o.owner), item_pos)
            && owner.distance(pos) >= 50.0
        {
            o.offered = true;
            o.offered_to = self.world.item_offered(room, o.owner, o.item, pos);
        }
        let living = match o.item {
            Entity::Creature(c) => !self.world.realized_dead(c),
            Entity::Object(_) => false,
        };
        if o.age > o.lifetime(living) {
            o.active = false;
        }
        self.put_owned_item(room, i, o);
        if let Some(grabber) = self.world.grabbed_by(o.item).first().map(|g| g.grabber) {
            if o.offered_to == Some(grabber) {
                self.world.social_event(
                    room,
                    SocialEventId::ItemTransaction,
                    o.owner,
                    grabber,
                    Some(o.item),
                );
                let waits = self
                    .world
                    .creature(grabber)
                    .lizard()
                    .and_then(|l| l.ai.friend.gift_offered_to_me)
                    .is_some_and(|g| g.room == room && g.serial == o.serial);
                if waits {
                    self.world.friend_gift_received(grabber);
                }
            }
            o.active = false;
        }
        if self.social_owner_room(o.owner) != room {
            o.active = false;
        }
        self.put_owned_item(room, i, o);
    }

    fn put_owned_item(&mut self, room: i32, i: usize, o: OwnedItemOnGround) {
        if let Some(slot) = self
            .world
            .realized_mut(room)
            .and_then(|r| r.social_events.owned_items_on_ground.get_mut(i))
        {
            *slot = o;
        }
    }

    /// An owner's `DangerPos` (the player's main chunk).
    fn social_owner_danger_pos(&self, owner: CreatureId) -> Option<Vec2> {
        if owner == self.player_id {
            return self.player.as_ref().map(|p| p.head().pos);
        }
        self.world
            .creature(owner)
            .realized
            .as_ref()
            .and_then(crate::creature::RealizedCreature::danger_pos)
    }

    /// `owner.abstractCreature.Room.index`.
    fn social_owner_room(&self, owner: CreatureId) -> i32 {
        if owner == self.player_id {
            self.pos().room
        } else {
            self.world.creature(owner).pos.room
        }
    }
}

#[cfg(test)]
#[path = "game_social_events_tests.rs"]
mod tests;
