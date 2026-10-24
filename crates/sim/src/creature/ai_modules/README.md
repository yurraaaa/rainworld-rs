# `creature/ai_modules/` (a realized creature's `AIModule`s)

| File | Holds | Ports |
|---|---|---|
| `tracker.rs`, `relationship_tracker.rs`, `looker.rs`, `vision.rs` | AI modules | `Tracker`, `RelationshipTracker`, `CreatureLooker`, `VisualScore` |
| `noise_tracker.rs`, `stuck_tracker.rs` | `NoiseTracker`; `StuckTracker`, `DiscomfortTracker` | same |
| `friend_tracker.rs` | `FriendTracker`'s gift half: `tamingDifficlty`, `giftOfferedToMe` (no friend: taming is not ported) | `FriendTracker` |
| `prey_tracker.rs`, `threat_tracker.rs`, `agression_tracker.rs`, `rain_tracker.rs`, `den_finder.rs`, `utility_comparer.rs`, `mission_tracker.rs` | AI modules' state; `UtilityComparer` with `FloatTweener`; missions | same |
| `world.rs` | prey/threat/aggression trackers' and den finder's updates, `SortCreatureIntoModule`, threat of a tile, threat travel cost, `NewArea` | `PreyTracker`, `ThreatTracker`, `AgressionTracker`, `DenFinder` |
| `threat_tracker_world.rs` | the threat tracker's flight: `FleeTo`, `EvaluateFlightDestThreat`, `GenerateRandomPath`, `FindMostAttractiveExit` | `ThreatTracker` |
| `tracker_world.rs` | tracker answers (relationships) | `DynamicRelationship` |
| `mission_tracker_world.rs` | `LeaveRoom.Act`: the exit a submitted lizard leaves by and where it heads | `MissionTracker` |
| `noise_tracker_world.rs` | noise delivery to creatures and `NoiseTracker`; `LizardAI.ReactToNoise` | `Room.InGameNoise`, `HeardNoise` |
| `tracker_turn.rs` | realized AI `Tracker`/`RelationshipTracker` running | `Tracker.cs`, `RelationshipTracker.cs` |
| `heavy_ai_turn.rs` | `HeavyAiThrottle`: `RequestHeavyAi`'s three-a-tick limit | `RainWorldGame.RequestHeavyAi` |
