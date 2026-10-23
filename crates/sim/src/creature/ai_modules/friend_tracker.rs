//! `FriendTracker`'s gift half: the taming difficulty and the gift offered to the AI
//! (`giftOfferedToMe`). Choosing, following and caring for a friend (taming proper)
//! are not ported; the world side is in `world/social_events.rs`.
//!
//! Mirrors: FriendTracker.cs (tamingDifficlty, giftOfferedToMe)

use crate::creature::CreatureId;
use crate::item::Entity;

/// `FriendTracker` as far as gifts go.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FriendTracker {
    /// `tamingDifficlty`: the breed's `tamingDifficulty * Lerp(0.9, 1.1, dominance)`.
    pub taming_difficulty: f32,
    pub gift_offered_to_me: Option<GiftOffer>,
}

/// The `OwnedItemOnGround` a `giftOfferedToMe` refers to: its serial in room `room`'s
/// recognizer (gone from the list means no longer `active`), its item and owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftOffer {
    pub room: i32,
    pub serial: u64,
    pub item: Entity,
    pub owner: CreatureId,
}

impl FriendTracker {
    #[must_use]
    pub fn new(taming_difficulty: f32) -> Self {
        Self {
            taming_difficulty,
            gift_offered_to_me: None,
        }
    }
}
