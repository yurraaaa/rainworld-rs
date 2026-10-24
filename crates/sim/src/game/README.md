# `game/`

| File | Holds (key types) | Ports |
|---|---|---|
| `mod.rs` | `RainWorldGame`: world, player, shortcut handler, realizer, order of a tick, a room's update list, sessions | `RainWorldGame` |
| `loading_turn.rs` | a room's loading on the game's side: `activate_room` with the end of `LoadFromDataString` and `Room.Loaded` on the realizing call, the loading step (`update_loading_rooms`; a preparer whose room was abstracted realizes nothing, D14), `ShortCutsReady` (the thread's shortcut map, objects, creatures without `AI`, the `ShortcutHelper`), `ReadyForAI` (the thread's AI map, creatures, quantified, the `AIMapReady` listeners in update-list order), `camera_room_ready` (dev starts step until it) | `Room.ShortCutsReady`, `Room.ReadyForAI`, `AllowedToExistInRoom` |
| `observer.rs` | `PlayerObserver`: the app's graphics-module hooks the game calls | - |
| `shortcut_turn.rs` | `ShortcutHandler.Update`: vessels in the tubes, the between-rooms lobby, room exits, arrival and camera 0 following | `ShortcutHandler.Update`, `RoomCamera.MoveCamera` |
| `shortcut_helper.rs` | `ShortcutHelper`, `ShortcutPusher`, `ObjectPusher`: the pushers at a room's entrances (wrong holes, valid neighbours, swell), the objects being pushed out, the round-robin's pick per layer | `ShortcutHelper` |
| `shortcut_helper_turn.rs` | the helper's turn in the update list: the player push, the `checkObj` round-robin over `physicalObjects`, `ObjectPusher.Update`, `PopsOutOfDeadShortcuts`, `CanBePulledIntoShortcut` | `ShortcutHelper`, `Room.ShortCutsReady` |
| `dev_turn.rs` | dev spawns/teleports through the game's own paths and the dev rain timer control and rain rewind (MCP, debug UI) | - (port dev tool, D18) |
| `exit_turn.rs` | what pause/`ExitGame` read | `RainWorldGame.ExitGame` |
| `instructions_turn.rs` | `Room.Loaded`'s placed room objects (guide triggers, placed lights, slime molds, dead token stalks, projected stars), input-instruction triggers | `SuperJumpInstruction`, `ReliableIggyDirection` |
| `map_turn.rs` | map discovery from the player's position | `Player.Update` discovery |
| `game_over_turn.rs` | `GameOver`, karma flower spot | `RainWorldGame.GameOver` |
| `shelter_turn.rs` | shelter door in the room, sleep/hibernate checks, `KillAllHostiles`, `DoorClosed` | `ShelterDoor` hookup |
| `trigger_turn.rs` | `ActiveTriggerChecker`s, `FireEvent`s | `Room.Loaded` triggers |
| `camera.rs` | what the sim reads of camera 0, visibility tests | `RoomCamera` |
| `campaign.rs` | per-slugcat campaign data: food, start den, karma | `SlugcatStats`, `SaveState` |
| `camera_image.rs` | `CameraImage`: camera 0's level image and palette (`PixelColorAtCoordinate`, effect colours) | `RoomCamera.PixelColorAtCoordinate` |
| `map_discovery.rs` | per-region discovery textures, base64 PNG form | `PlayerProgression` map textures |
| `room_script.rs` | room-specific scripts (`RoomScript`: SU_C04 start, SS_E08 gradient gravity), at their own place in the update list | `RoomSpecificScript` |
| `rain_turn.rs` | the rain in `RainWorldGame.Update`: camera 0's shake step with the rain's terms (`RainShake`), `GlobalRain.Update` after it, `RainHit` after the timers, `Room.RainDrownOutSounds` | `RainWorldGame.Update`, `RoomCamera.Update`, `RainCycle.Update`, `Room.RainDrownOutSounds` |
| `bullet_drip_turn.rs` | `RoomRain.Update`'s upkeep of the `BulletDrip` count (one added or the oldest destroyed per tick), each drip's turn in the update list, `Strike`'s trace against the layer-1 (player, lizards) chunks, the water entry, `Bullet_Drip_Strike` / `Small_Object_Into_Water_Fast` and the push and stun of what it hit | `BulletDrip` (`Update`, `Strike`), `RoomRain.Update`, `SharedPhysics.TraceProjectileAgainstBodyChunks` |
| `room_rain_turn.rs` | `Room.Loaded`'s `RoomRain` (and the constructor's `fWaterLevel`), its turn in the update list (intensity, `ThrowAroundObjects` over the room's physical objects with stun, `rainDeath` and the kill, the flood arm: the water level toward `FloodLevel`, `GeneralUpsetSurface`, `Room.AddWater`; the sound loops and their emitters), `Creature.rainDeath` reads | `Room.Loaded/AddWater`, `RoomRain.RoomRain/Update/ThrowAroundObjects` |
| `screen_shake.rs` | camera 0's shake: `CameraShake` (`screenShake`, `microShake`, the per-tick jump at the head of the tick, the rain's two terms from `RainShake`), `ScreenMovement` (camera side, `Room.ScreenMovement` with its ceiling drips), `TerrainImpact`'s bump, `ceilingTiles` | `RoomCamera.Update/ScreenMovement`, `Room.ScreenMovement` |
| `shelter.rs` | `ShelterDoor` (intact, base game) | `ShelterDoor` |
| `shortcut.rs` | `ShortcutHandler` divider, vessels, entrance blinking | `ShortcutHandler` |
| `spawn.rs` | safe spawn tile, placing an object | `DetermineSafeSpawnTile`, `PlaceInRoom` |
| `trigger.rs` | triggers in a realized room, event-made room objects | `ActiveTriggerChecker` |
