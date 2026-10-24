# `effects/gameplay/` (effects that act on play)

| File | Holds (key types) | Ports |
|---|---|---|
| `anti_gravity_turn.rs` | zero gravity in the game: `Room.Loaded`'s `AntiGravity`, its turn (the room's `gravity`), `brokenAntiGrav.Update` from `RainCycle.Update` with the camera room's switch sound and screen movement, the ambience gravity the microphone fades by | `Room.Loaded`, `AntiGravity.Update`, `BrokenAntiGravity.Update` |
| `border_push_back_turn.rs` | a room's `RoomBorderPushBack` in the game: `Room.Loaded` making it, its turn in the update list pushing the player's chunks and what it holds | `Room.Loaded` (`BorderPushBack`), `RoomBorderPushBack.Update`, `PushAtCreature` |
| `electric_death_turn.rs` | a room's `ElectricDeath` in the game: `Room.Loaded` making it and `AIMapReady`'s wall tiles, its turn (loops, wall sparks, flashes, colour, the strikes over the room's `physicalObjects` with the creatures' electric `Violence`), its loop emitters' turns and its `SparkFlash`es' turns | `Room.Loaded` (`ElectricDeath`), `ElectricDeath.Update`, `AIMapReady`, `SparkFlash.Update` |
| `death_fall_turn.rs` | a room's `DeathFallGraphic` in the game: `Room.Loaded` making it (open bottom, not an unbroken shelter), its counter ticking in the update list whether or not a camera views the room | `Room.Loaded` (`DeathFallGraphic`), `DeathFallGraphic.Update` |
| `death_fall.rs` | `DeathFall`: `RoomRequiresDeathFall`, `defaultHeight`, focal-point height, `counter`, the sprite's alpha (the sprite is the app's) | `DeathFallGraphic` |
| `anti_gravity.rs` | `AntiGravity` (the room's gravity from `ZeroG`/`BrokenZeroG`), `BrokenAntiGravity` (the world's flickering switch: counter, 121-tick transition, lights) | `AntiGravity` |
| `electric_death.rs` | `ElectricDeath` (`Intensity` from the rain cycle, `LightFlash`es, the colour walk, the two sound loops, the wall-spark and strike rolls) and `SparkFlash` | `ElectricDeath`, `LightFlash`, `SparkFlash` |
| `border_push_back.rs` | `BorderPushBack`: margin, camera span, `pushPower` per player and the push it gives | `RoomBorderPushBack` |
