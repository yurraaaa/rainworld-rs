# `world/` (the loaded world and its abstract layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `World`: abstract rooms, realized set, arenas | `World`, `AbstractRoom` |
| `realized.rs`, `realizer.rs` | realized room data + ready states (`loadingProgress`, `readyForNonAICreaturesToEnter`, the stages' room-side steps, the realization `serial`); room realizer (`currentlyLoadingRoom` as a realization, waited on after an abstract) | `Room` bits, `RoomRealizer` |
| `room_preparer.rs` | `RoomPreparer`: its worker thread (`PreparerJob`: `ShortcutMapper`, `AImapper` and heat maps) and the hand-off to the main thread under a `parking_lot` lock (status, requests, `threadFinished`, `done`), the realization `serial` it loads, outliving an abstracted room; `PreparerStage` carries each stage's map (D14) | `RoomPreparer` |
| `loading_pace.rs` | `LoadingPace`: headless ticks wait out a 40 Hz frame while a room loads (port-only, D14) | - |
| `creatures.rs` | creature arena, room lists, attractions | `AbstractRoom` creature lists |
| `objects.rs` | object arena, room items at the game's three places (file items, a story's `MultiplayerItem`s in the placed-object loop, the `firstTimeRealized` pass) | `Room.Loaded` objects |
| `dens.rs` | into/out of dens with stuck items | `MoveEntityToDen` |
| `quantified.rs` | quantified creatures placed in a room and counted back | `Room.PlaceQuantifiedCreaturesInRoom`, `AbstractRoom.CountQuantifiedCreatures` |
| `global_rain.rs` | `GlobalRain` (direction, `Intensity`, shakes, flood), `DeathRain`, `DeathRainMode` | `GlobalRain`, `GlobalRain.DeathRain` |
| `bullet_drip.rs` | `BulletDrip`: its fields, `Update` and `Strike`'s setup (column, `pos`, `skyPos`, `fallSpeed`, `delay`: the draws in order), the drawn tip and head | `BulletDrip` |
| `room_rain.rs` | `RoomRain` (`rainReach`, `splashTiles`, `intensity`, push properties, `FloodLevel` and the flood arm's test, the per-chunk steps of `ThrowAroundObjects`), `SmashView` (`CreatureSmashedInGround`), `World::creature_smashed_in_ground` | `RoomRain` |
| `room_rain_sound.rs` | `RainLoops`: the `RoomRain`'s seven `DisembodiedDynamicSoundLoop`s (which exist per `DangerType`, clip, volume group, the volume formulas of `Update`) | `RoomRain` (constructor, `Update`) |
| `rain_cycle.rs` | `RainCycle` (with `GlobalRain` as `global_rain`, `RainHit`, `ShaderLight`/`ScreenShake`/`MicroScreenShake`), `WaterLevelCycle` | `RainCycle` |
| `region_state.rs` | `RegionState`, its save string, and adapting the world to and from it | `RegionState`, `SaveState` |

- `ai_map/`: `mod.rs` (`AiMap`, `AiTile`), `ai_mapper.rs` (`AiMapper`), `creature_ai_map.rs`
  (baked per-creature maps), `movement.rs` (accessibility/connection/cost vocabulary),
  `quick_path.rs` (`QuickPathFinder`), `quick_connectivity.rs` (`QuickConnectivity`:
  `Check`, `QuickPath`). Ports `AImap`, `AImapper`, `CreatureSpecificAImap`,
  `QuickPathFinder`, `QuickConnectivity`.

The world side of creatures, items and water lives with them: `creature/abstract_ai/`,
`creature/ai_modules/*_world.rs`, `creature/lizard/world.rs`, `creature/fly/world_ai.rs`,
`creature/overseer/world_ai.rs`, `item/weapon_world.rs`, `water/world.rs`.
