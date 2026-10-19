# rw-sim

The gameplay simulation: a fixed 40 Hz tick over the world, rooms, the slugcat,
creatures (abstract and realized), items, water, shortcuts, shelters, noise and the
game's random draws. It is engine-agnostic: no I/O, no windowing, no rendering, no
cosmetic graphics (those are `rw-graphics`, `rw-render`), no save-file I/O
(`rw-save`), no HUD (`rw-hud`), no menus (`rw-menu`). Where the sim needs a graphics
module's state it keeps a small "link" struct (e.g. `creature/lizard/graphics_link.rs`).
Entry point: `RainWorldGame` in `game.rs` (the app's `Game` wraps it). Draws come from the
shared `UnityRandom` stream (`rw-random`); every sound makes its draws too (`sound.rs`).

Navigation aids: module headers (`grep -n '^//!' <file>`), decompile line numbers in
comments refer to `/home/yura/.local/share/ilspy/full/Assembly-CSharp.decompiled.cs`.
Notes/surveys are in `../../../rainworld-re/` (see the last section).

## Top-level files (`src/`)

| File | Holds (key types) | Ports |
|---|---|---|
| `lib.rs` | module list, re-exports | - |
| `game.rs` | `RainWorldGame`: world, player, shortcut handler + lobby, realizer, order of a tick, sessions | `RainWorldGame` |
| `game_*.rs` | `impl RainWorldGame` splits, below | - |
| `game_dev.rs` | dev spawns/teleports through the game's own paths (MCP, debug UI) | - (port dev tool, D18) |
| `game_exit.rs` | what pause/`ExitGame` read | `RainWorldGame.ExitGame` |
| `game_flies.rs` | bat flies in the tick: world process, `ReadyForAI`, vessels, lobby turns | `FliesWorldAI` hookup |
| `game_hits.rs` | thrown rock/spear vs creatures | `Weapon.Update` Thrown branch |
| `game_holo_light.rs` | Monk guide light in the update list | `HologramLight.TryCreate` |
| `game_instructions.rs` | guide's placed room objects, input-instruction triggers | `SuperJumpInstruction`, `ReliableIggyDirection` |
| `game_items.rs` | slugcat grabbing: `Grabability`, `CanIPickThisUp`, pick-up candidate | `Player` item rules |
| `game_lizards.rs` | realized lizards in the update list, vessels, `SpitOutOfShortCut` | `Lizard.Update` |
| `game_lizard_act.rs` | stand-in lizard AI + movement (`Act`, `FollowPath`) | `Lizard.Act`, `LizardAI.Update` |
| `game_lizard_hits.rs` | lizard damage dealt/taken, bites, stun, spear stick | `Lizard.Violence`, `Bite` |
| `game_lizard_perception.rs` | lizard eyes, tracker representation, relationships | `LizardAI` (tracker parts) |
| `game_lizard_graphics.rs` | game side of `LizardGraphics` (bubbles, drips, head light) | `LizardGraphics` hookup |
| `game_lizard_voice.rs` | `LizardVoice` update and emitters | `LizardVoice` hookup |
| `game_map.rs` | map discovery from the player's position | `Player.Update` discovery |
| `game_noise.rs` | in-room noises heard at once | `Room.InGameNoise` |
| `game_over.rs` | `GameOver`, karma flower spot | `RainWorldGame.GameOver` |
| `game_player.rs` | slugcat hands: bat-fly catch, eat, swallow, throw, hand switch | `Player.GrabUpdate`, `Collide` |
| `game_shelter.rs` | shelter door in the room, sleep/hibernate checks, `DoorClosed` | `ShelterDoor` hookup |
| `game_tracker.rs` | realized AI `Tracker`/`RelationshipTracker` running | `Tracker.cs`, `RelationshipTracker.cs` |
| `game_triggers.rs` | `ActiveTriggerChecker`s, `FireEvent`s | `Room.Loaded` triggers |
| `game_weapons.rs` | rocks/spears in the update list, loops, sparks | - |
| `*_tests.rs`, `tests.rs` | tests for their `game_*`/`shelter` siblings; `tests.rs` is physics unit tests | - |
| `camera.rs` | what the sim reads of camera 0, visibility tests | `RoomCamera` |
| `campaign.rs` | per-slugcat campaign data: food, start den, karma | `SlugcatStats`, `SaveState` |
| `chunk.rs` | `BodyChunk` integration + tile collision | `BodyChunk.Update` |
| `object.rs` | `PhysicalObject`: chunks + distance constraints | `PhysicalObject` |
| `item.rs` | abstract objects (`AbstractObject`, `Entity`, `ObjectData`, `AbstractSpear`), save strings | `AbstractPhysicalObject` |
| `light_source.rs` | `LightSource` in the update list | `LightSource` |
| `map_discovery.rs` | per-region discovery textures, base64 PNG form | `PlayerProgression` map textures |
| `noise.rs` | `InGameNoise`, terrain-impact noise, `NoiseSink` | `Noise/InGameNoise.cs` |
| `room_ext.rs`, `color.rs`, `float_ext.rs`, `vec2.rs`, `vec3.rs` | position lookups on `Room`; `Custom` colour/scalar helpers; vectors | `RWCustom.Custom` |
| `room_script.rs` | room-specific scripts (`RoomScript`, SU_C04 start) | `RoomSpecificScript` |
| `shelter.rs` | `ShelterDoor` (intact, base game) | `ShelterDoor` |
| `shortcut.rs` | `ShortcutHandler` divider, vessels, entrance blinking | `ShortcutHandler` |
| `sound.rs` | `Room.PlaySound` events + `VirtualMicrophone` draws | `VirtualMicrophone.PlaySound` |
| `spawn.rs` | safe spawn tile, placing an object | `DetermineSafeSpawnTile`, `PlaceInRoom` |
| `trigger.rs` | triggers in a realized room, event-made room objects | `ActiveTriggerChecker` |
| `water.rs` | wavy surface sim, `FloatWaterLevel` | `Water`, `DefaultSurface` |
| `water_drip.rs` | `WaterDrip` droplets | `WaterDrip` |
| `cosmetic.rs` | `Spark`, `ExplosionSpikes`, `OverseerEffectSprite` sprites in update lists | `CosmeticSprite`s |

## `player/` (slugcat; `Player.cs` split by topic)

`mod.rs` (`Player`, two-chunk body), `input.rs` (`Input`), `movement.rs`
(`MovementUpdate`), `body_mode.rs` (`UpdateBodyMode`), `animation.rs`
(`UpdateAnimation`), `jump.rs`, `advanced.rs` (flip, roll, rocket jump, belly slide),
`poles.rs`, `ledge.rs`, `corridor.rs`, `swim.rs` (water, breath, `LungUpdate`),
`falls.rs` (impact, stun, die), `exhaustion.rs` (aerobic level, malnourishment),
`grab.rs` (hand counters, `ThrowDirection`), `shortcut.rs` (enter/ride/leave tubes),
`sleep.rs` (`SleepUpdate`), `sounds.rs` (slide loop, plays). `tests.rs`, `tests/`: tests.

## `world/` (the loaded world and its abstract layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `World`: abstract rooms, realized set, arenas | `World`, `AbstractRoom` |
| `realized.rs`, `realizer.rs` | realized room data + ready states; room realizer | `Room` bits, `RoomRealizer` |
| `creatures.rs` | creature arena, room lists, attractions | `AbstractRoom` creature lists |
| `objects.rs` | object arena, room items on first realization | `Room.Loaded` objects |
| `weapons.rs` | realized rocks/spears per room, `ShortCutsReady` | - |
| `dens.rs` | into/out of dens with stuck items | `MoveEntityToDen` |
| `abstract_ai.rs`, `abstract_path.rs` | plain abstract AI (roam, migrate, den); abstract pathfinders | `AbstractCreatureAI`, `AbstractSpacePathFinder` |
| `abstract_ai_kinds.rs` | vulture, cicada, big eel, deer, Miros bird, needle worm behaviours | `*AbstractAI` |
| `flies_world_ai.rs`, `swarm_rooms.rs`, `quantified.rs` | bat-fly world process, swarm map, quantified counts | `FliesWorldAI`, `SwarmRoomMapper` |
| `overseers_world_ai.rs` | shelter finder, guide direction | `OverseersWorldAI` |
| `lizards.rs` | lizard realize/abstractize, `RealizeInRoom` | `AbstractCreature.Realize` |
| `ai_map.rs`, `ai_mapper.rs`, `creature_ai_map.rs`, `movement.rs` | `AiMap`, `AiMapper`, baked per-creature maps, accessibility/connection/cost vocabulary | `AImap`, `AImapper` |
| `quick_path.rs`, `quick_connectivity.rs` | `QuickPathFinder`, `QuickConnectivity` | same |
| `hearing.rs` | noise delivery to creatures and `NoiseTracker` | `Room.InGameNoise`, `HeardNoise` |
| `tracking.rs`, `social_events.rs` | tracker answers (relationships); `SocialEventId` and the recognizer's functions | `DynamicRelationship`, `SocialEventRecognizer` |
| `rain_cycle.rs` | `RainCycle`, `WaterLevelCycle` | `RainCycle` |
| `region_state.rs`, `region_save.rs` | `RegionState` and adapting world to/from it | `RegionState` |
| `*_tests.rs` | tests of siblings | - |

## `creature/` (shared creature layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `AbstractCreature`, `EntityId`, `IdIssuer`, `WorldCoordinate`, `CreatureState`, `RealizedCreature` | `AbstractCreature` |
| `template.rs`, `template_abstract.rs`, `pathing.rs`, `relationships.rs`, `violence.rs` | `CreatureTemplate` data, resistances, relationship tables, damage data | `StaticWorld.InitStaticWorld`, `LizardBreeds` |
| `ai.rs`, `ai_kinds.rs` | abstract AI state + constructors | `AbstractCreatureAI` subclasses |
| `population.rs` | world creature generation | `WorldLoader.GeneratePopulation` |
| `state_save.rs`, `social_memory.rs` | creature state save string; `SocialMemory` | `CreatureState`, `SocialMemory` |
| `death.rs`, `grasp.rs`, `danger.rs` | kill tag/die; `Grasp` + grip sticks; threat/danger reads | `Creature.Die`, `Creature.Grab` |
| `tracker.rs`, `relationship_tracker.rs`, `looker.rs`, `vision.rs` | AI modules | `Tracker`, `RelationshipTracker`, `CreatureLooker`, `VisualScore` |
| `noise_tracker.rs`, `stuck_tracker.rs` | `NoiseTracker`; `StuckTracker`, `DiscomfortTracker` | same |

- `lizard/`: `mod.rs` (`Lizard`), `breed.rs` (`LizardBreedParams`, green only), `ai.rs`
  (stand-in `LizardAi`), `movement.rs` (`MovementAnimation`), `voice.rs` (`LizardVoice`),
  `bubble.rs` (`LizardBubble`), `graphics_link.rs` (state shared with `rw-graphics`).
- `fly/`: `mod.rs` (`Fly`), `update.rs` (`Fly.Update`), `ai.rs` + `ai_update.rs`
  (`FlyAi`, flocking), `room_ai.rs` (`FliesRoomAi`, hives).
- `overseer/`: `abstract_ai.rs` (`OverseerAbstractAi`), `ai.rs` (`OverseerAi` state,
  zip matrix), `body.rs` (`OverseerBody`), `update.rs` (the whole realized update),
  `tutorial.rs` (`TutorialBehavior`, `TutorialMessage`), `guide_state.rs` (`PlayerGuideState`),
  `holo_light.rs` (`HologramLight`), `hologram/` (the projected pictures: `mod.rs`
  `OverseerHolograms`, `add.rs` `TryAddHologram`, `update.rs`, `parts.rs`, `instruction.rs`,
  `world_queries.rs`).
- `path_finder/`: `mod.rs` (`PathFinder`), `mapper.rs` (`AccessibilityMapper`, world path
  finders), `lizard.rs` (`PatherKind`, the lizard pather's cost functions).

## `item/`

`weapon.rs` (`Weapon`, `WeaponKind::{Rock, Spear}`), `spear_stick.rs` (`SpearStick`, `SpearStickKind`),
`weapon_tests.rs`.

## Where is X?

| Question | Look in |
|---|---|
| Shortcuts / pipes, vessels, lobby | `shortcut.rs`, `player/shortcut.rs`, `game.rs` (lobby), `game_lizards.rs`, `game_flies.rs` |
| Water, swimming, drowning | `water.rs`, `player/swim.rs`, `water_drip.rs` |
| Abstract creature AI (roam, migrate) | `world/abstract_ai.rs`, `abstract_path.rs`, `creature/ai.rs` |
| Type-specific abstract AI | `world/abstract_ai_kinds.rs`, `creature/ai_kinds.rs`, `creature/overseer/abstract_ai.rs` |
| Den logic | `world/dens.rs`, `world/abstract_ai.rs` (den counters), `world/creatures.rs` |
| Population generation | `creature/population.rs` |
| Tracker / relationships | `creature/tracker.rs`, `relationship_tracker.rs`, `game_tracker.rs`, `world/tracking.rs` |
| Noise and hearing | `noise.rs`, `game_noise.rs`, `world/hearing.rs`, `creature/noise_tracker.rs` |
| Spear sticking, lodged spears | `item/spear_stick.rs`, `item/weapon.rs`, `game_lizard_hits.rs`, `game_hits.rs` |
| Thrown weapon hits | `game_hits.rs`, `item/weapon.rs` |
| Lizard movement / path following | `game_lizard_act.rs`, `creature/lizard/movement.rs`, `creature/path_finder/` |
| Room tile AI map | `world/ai_map.rs`, `ai_mapper.rs` |
| Shelters, sleep, hibernation | `shelter.rs`, `game_shelter.rs`, `player/sleep.rs` |
| Save-state writing (sim side) | `world/region_state.rs`, `region_save.rs`, `creature/state_save.rs`, `item.rs` (object strings), `creature/overseer/guide_state.rs`, `map_discovery.rs`; text format in `rw-save` |
| Death, game over, karma flower | `game_over.rs`, `creature/death.rs`, `player/falls.rs` |
| Random draws / sounds | `sound.rs`, `rw-random` crate; draws live beside each ported function |
| Camera visibility | `camera.rs` |
| Room triggers, guide instructions | `trigger.rs`, `game_triggers.rs`, `game_instructions.rs`, `room_script.rs` |
| Room realization / loading | `world/realizer.rs`, `world/realized.rs`, `world/mod.rs` |
| Rain cycle | `world/rain_cycle.rs` |
| Eating, picking up, throwing | `game_player.rs`, `game_items.rs`, `player/grab.rs` |
| Body physics / collision | `chunk.rs`, `object.rs`, `player/mod.rs` |
| Overseer / guide | `creature/overseer/`, `world/overseers_world_ai.rs`, `game_holo_light.rs` |
| Bat flies | `creature/fly/`, `world/flies_world_ai.rs`, `game_flies.rs` |
| Dev spawns / teleport | `game_dev.rs`, `spawn.rs` |

## Cross-links

Notes (`../../../rainworld-re/notes/`): `physics-core.md`, `slope-collision.md`,
`slugcat-movement.md`, `advanced-moves.md`, `falls.md`, `corridors.md`,
`vertical-poles.md`, `horizontal-beams.md`, `ledge-grab.md`, `shortcuts.md`, `water.md`,
`ai-map.md`, `creature-model.md`, `items.md`, `shelter.md`, `saves.md`, `sound.md`,
`unity-random.md`, `overseer.md`, `room-realizer.md`, `world-format.md`,
`world-loading.md`, `room-format.md`, `campaign-model.md`, `music.md`.
Surveys (`../../../rainworld-re/surveys/`): `lizards.md`, `bat-flies.md`, `rocks-spears.md`,
`spawning.md`, `violence-templates.md`, `items-room-edges.md`, `campaign.md`,
`sleep-death-screens.md`, `malnourishment.md`, `food-pip-clip.md`.
(`notes/intro-overseer-survey.md` covers the guide and `room_script.rs`.)

Divergences: `../../docs/divergences/` (IDs are global, `grep -n '^## D'`). In `sim.md`:
D2 spawning, D3 movement, D4 falls, D5 shortcuts, D6 poles/ledges, D9 skid/roll/belly
slide, D10 corridors, D11 water, D14 worlds/rooms/realizer (creatures, lizards, flies,
overseers), D17 shelters. Elsewhere: D1 `random.md`, D12 `audio.md`, D13 `menu.md`,
D16 `save.md`, D18 `app.md`.

Tests: `*_tests.rs` beside their module, `player/tests/`, and `tests/` (12 `installed_*.rs` integration tests that read the installed game, plus `data/`).
