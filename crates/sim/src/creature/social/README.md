# `creature/social/`

| File | Holds | Ports |
|---|---|---|
| `social_events.rs` | `SocialEventId`, the recognizer (`ownedItemsOnGround`, `ItemOffered`) and its events, lizards' reactions (fear relief, gifts: `FriendTracker.ItemOffered`, `GiftRecieved`) | `SocialEventRecognizer` |
| `social_events_turn.rs` | the slugcat putting items on the ground (`ReleaseObject`, `TossObject`, the back spear laid down) and the room's `SocialEventRecognizer.Update` (owned items offered, lapsing, a lizard's gift received) | `SocialEventRecognizer`, `OwnedItemOnGround` |
| `social_memory.rs` | `SocialMemory` | `SocialMemory` |
