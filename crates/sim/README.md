# rw-sim

The gameplay simulation: a fixed 40 Hz tick over the world, rooms, the slugcat,
creatures (abstract and realized), items, water, shortcuts, shelters, noise and the
game's random draws. It is engine-agnostic: no I/O, no windowing, no rendering, no
cosmetic graphics (those are `rw-graphics`, `rw-render`), no save-file I/O
(`rw-save`), no HUD (`rw-hud`), no menus (`rw-menu`). Where the sim needs a graphics
module's state it keeps a small "link" struct (e.g. `creature/lizard/graphics_link.rs`).
Entry point: `RainWorldGame` in `game/mod.rs` (the app's `Game` wraps it). Draws come from the
shared `UnityRandom` stream (`rw-random`); every sound makes its draws too (`sound/mod.rs`).

Navigation aids: module headers (`grep -n '^//!' <file>`), decompile line numbers in
comments refer to `/home/yura/.local/share/ilspy/full/Assembly-CSharp.decompiled.cs`.
Notes/surveys are in `../../../rainworld-re/` (see the last section).

## Layout

`src/` is split by topic: `math/`, `physics/`, `game/`, `player/`, `creature/`, `item/`,
`sound/`, `water/`, `effects/{lights,sky,particles,gameplay}/`,
`superstructure/{projections,electric,grab,coral}/`, `vines/`, `world/`. The module tree
is the folder tree. `lib.rs` declares the topics and re-exports the crate's API at the
root (`rw_sim::Vec2`, ...); the public modules are `creature`, `item`, `world` and the
re-exported `campaign`, `color`, `insect`, `map_discovery`, `room_script`, `shelter`,
`trigger`. `world/mod.rs` and `creature/mod.rs` re-export their topics' types
(`rw_sim::world::FliesWorldAi`, `rw_sim::creature::Tracker`) wherever those now live.

A topic folder holds three kinds of file beside each other:

- **Types** (`x.rs`): the decompiled classes the topic ports.
- **Turn files** (`turn.rs`, `<part>_turn.rs`): the topic's `impl RainWorldGame` code
  (making its objects in `Room.Loaded`, their turns in the update list). `turn.rs` is
  the folder's own topic, `<part>_turn.rs` one part of it (`creature/lizard/act_turn.rs`).
- **World files** (`world.rs`, `<part>_world.rs`, or a type's own file such as
  `creature/fly/world_ai.rs`): the topic's `impl World` code (realizing, abstracting,
  the world's update of an AI module).

`RainWorldGame`'s and `World`'s fields and the helpers these files share are
`pub(crate)`.

**Tests**: unit tests of `x.rs` are `tests/x.rs` in the same folder, declared from `x.rs`
as `#[cfg(test)] #[path = "tests/x.rs"] mod tests;` (a child module, so private items
stay reachable); a folder's `mod.rs` tests are `tests/mod.rs` (`game/tests/mod.rs` has
one child module per topic of `game/mod.rs`'s own code). `src/tests/mod.rs` holds the
test rooms the unit tests share. `../tests/` holds only the `installed_*.rs`
integration tests.

## Files by folder

### `game/`

| File | Holds (key types) | Ports |
|---|---|---|
| `mod.rs` | `RainWorldGame`: world, player, shortcut handler, realizer, order of a tick, a room's update list, sessions | `RainWorldGame` |
| `loading_turn.rs` | a room's loading on the game's side: `activate_room` with the end of `LoadFromDataString` and `Room.Loaded` on the realizing call, the loading step (`update_loading_rooms`; a preparer whose room was abstracted realizes nothing, D14), `ShortCutsReady` (objects, creatures without `AI`, the `ShortcutHelper`), `ReadyForAI` (creatures, quantified, the `AIMapReady` listeners in update-list order), `camera_room_ready` (dev starts step until it) | `Room.ShortCutsReady`, `Room.ReadyForAI`, `AllowedToExistInRoom` |
| `observer.rs` | `PlayerObserver`: the app's graphics-module hooks the game calls | - |
| `shortcut_turn.rs` | `ShortcutHandler.Update`: vessels in the tubes, the between-rooms lobby, room exits, arrival and camera 0 following | `ShortcutHandler.Update`, `RoomCamera.MoveCamera` |
| `shortcut_helper.rs` | `ShortcutHelper`, `ShortcutPusher`, `ObjectPusher`: the pushers at a room's entrances (wrong holes, valid neighbours, swell), the objects being pushed out, the round-robin's pick per layer | `ShortcutHelper` |
| `shortcut_helper_turn.rs` | the helper's turn in the update list: the player push, the `checkObj` round-robin over `physicalObjects`, `ObjectPusher.Update`, `PopsOutOfDeadShortcuts`, `CanBePulledIntoShortcut` | `ShortcutHelper`, `Room.ShortCutsReady` |
| `dev_turn.rs` | dev spawns/teleports through the game's own paths and the dev rain timer control (MCP, debug UI) | - (port dev tool, D18) |
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
| `room_rain_turn.rs` | `Room.Loaded`'s `RoomRain`, its turn in the update list (intensity, `ThrowAroundObjects` over the room's physical objects with stun, `rainDeath` and the kill), `Creature.rainDeath` reads | `Room.Loaded`, `RoomRain.Update/ThrowAroundObjects` |
| `screen_shake.rs` | camera 0's shake: `CameraShake` (`screenShake`, `microShake`, the per-tick jump at the head of the tick, the rain's two terms from `RainShake`), `ScreenMovement` (camera side, `Room.ScreenMovement` with its ceiling drips), `TerrainImpact`'s bump, `ceilingTiles` | `RoomCamera.Update/ScreenMovement`, `Room.ScreenMovement` |
| `shelter.rs` | `ShelterDoor` (intact, base game) | `ShelterDoor` |
| `shortcut.rs` | `ShortcutHandler` divider, vessels, entrance blinking | `ShortcutHandler` |
| `spawn.rs` | safe spawn tile, placing an object | `DetermineSafeSpawnTile`, `PlaceInRoom` |
| `trigger.rs` | triggers in a realized room, event-made room objects | `ActiveTriggerChecker` |

### `math/`

| File | Holds (key types) | Ports |
|---|---|---|
| `color.rs`, `float_ext.rs`, `vec2.rs`, `vec3.rs` | `Custom` colour/scalar helpers; vectors | `RWCustom.Custom` |

### `physics/`

| File | Holds (key types) | Ports |
|---|---|---|
| `chunk.rs` | `BodyChunk` integration + tile collision, the `goThroughFloors` getter (`pulled_into_shortcut`, `grabs_disable_floors`) | `BodyChunk.Update` |
| `object.rs` | `PhysicalObject`: chunks + distance constraints | `PhysicalObject` |
| `shared_physics.rs` | point terrain collision, exact tile ray trace | `SharedPhysics`, `Custom.RectCollision` |
| `room_ext.rs` | position lookups on `Room` (`RoomExt`) | - |

### `player/` (slugcat; `Player.cs` split by topic)

`mod.rs` (`Player`, two-chunk body), `input.rs` (`Input`), `movement.rs`
(`MovementUpdate`), `body_mode.rs` (`UpdateBodyMode`), `animation.rs`
(`UpdateAnimation`), `jump.rs`, `jump_on_chunk.rs` (`jumpChunk`: `Collide`'s note of
the chunk under the hips, `JumpOnChunk`), `advanced.rs` (flip, roll, rocket jump, belly slide),
`poles.rs`, `ledge.rs`, `corridor.rs`, `swim.rs` (water, breath, `LungUpdate`),
`falls.rs` (impact, stun, die), `grabbed.rs` (held: `dangerGrasp` state and timers,
`cantBeGrabbedCounter`, the pacifying stun, `WALLSTUCK`), `exhaustion.rs` (aerobic level, malnourishment),
`grab.rs` (hand counters, `ThrowDirection`), `carry.rs` (`HeavyCarry`: run slowdown, drag through narrow spaces, pulled into a shortcut, jump power), `shortcut.rs` (enter/ride/leave tubes),
`circuit.rs` (`CollideWithCoralCircuitBit`, `circuitSwimResistance`, `curcuitJumpMeter`, `swimBits`), `zero_g.rs` (the `ZeroG` body mode, `ZeroGSwim` and `ZeroGPoleGrab`, the entry and exit checks), `vines.rs` (the vine grab in `MovementUpdate` and the `VineGrab` animation; the room's vines come through `NoiseSink::vines()`), `sleep.rs` (`SleepUpdate`), `sounds.rs` (slide loop, plays), `drips.rs` (the skid's and
corridor shoots' `WaterDrip`s), `shortcut_push.rs` (a wrong hole pushing the player out: `ShortcutHelper.Update`'s player part, `PushPlayerTowardsValidNeighbor`), `spear_on_back.rs` (the Hunter's
`SpearOnBack` state, `DropSpear`'s draws in `Stun` and `Die`).

| File | Holds (key types) | Ports |
|---|---|---|
| `carried_turn.rs` | what a creature's vessel carries: `FollowedByCamera`, stuck objects moving out of the lobby, the held player spat out with its grabber or killed in its den | `AbstractCreature.FollowedByCamera`, `Creature.SpitOutOfShortCut` |
| `eat_meat_turn.rs` | eating a held dead creature: `CanEatMeat`, `GrabUpdate`'s meat block, `EatMeatUpdate` (bites, drips, food) | `Player.EatMeatUpdate` |
| `turn.rs` | slugcat hands: bat-fly catch, eat, swallow, throw, hand switch; the heavy carry's rod and the body lent to the player's update; the jump chunk's masses lent and `JumpOnChunk`'s push and stun applied | `Player.GrabUpdate`, `Collide`, `JumpOnChunk` |
| `grabbed_turn.rs` | the player held: `creature_grab` (`CanBeGrabbed`, `Player.Grabbed`), the `dangerGrasp` block, `ThrowToGetFree`, `DangerGraspPickup`, `GameOver(dangerGrasp)`, the release under the room | `Player.Update`, `Creature.Grab` |
| `spear_on_back_turn.rs` | the Hunter's spear on the back: stash/retrieve by holding pick-up, `SpearToBack`, `SpearToHand`, `DropSpear` applied, carried on the back | `Player.SpearOnBack` |

### `creature/` (shared creature layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `AbstractCreature`, `EntityId`, `IdIssuer`, `WorldCoordinate`, `CreatureState`, `RealizedCreature` | `AbstractCreature` |
| `template.rs`, `template_abstract.rs`, `pathing.rs`, `relationships.rs`, `violence.rs` | `CreatureTemplate` data, resistances, relationship tables, damage data | `StaticWorld.InitStaticWorld`, `LizardBreeds` |
| `population.rs` | world creature generation | `WorldLoader.GeneratePopulation` |
| `state_save.rs` | creature state save string | `CreatureState` |
| `centipede.rs` | `AbstractCreature::centipede_generate_size` (size from the ID, for its icon) | `Centipede.GenerateSize` |
| `death.rs`, `grasp.rs`, `danger.rs` | kill tag/die, `Killing`, `AddCreatureToRespawn` and the lineage roll; `Grasp` + grip sticks; threat/danger reads | `Creature.Die`, `Creature.Grab` |

#### `creature/abstract_ai/` (the abstract AI)

| File | Holds | Ports |
|---|---|---|
| `mod.rs`, `kinds.rs` | abstract AI state + constructors | `AbstractCreatureAI` subclasses |
| `world.rs` | plain abstract AI in the world (roam, migrate, den) | `AbstractCreatureAI`, `AbstractCreature` |
| `kinds_world.rs` | vulture, cicada, big eel, deer, Miros bird, needle worm behaviours | `*AbstractAI` |
| `path_finder.rs` | abstract pathfinders | `AbstractSpacePathFinder`, `AbstractSpaceNodeFinder` |

#### `creature/ai_modules/` (a realized creature's `AIModule`s)

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

#### `creature/social/`

| File | Holds | Ports |
|---|---|---|
| `social_events.rs` | `SocialEventId`, the recognizer (`ownedItemsOnGround`, `ItemOffered`) and its events, lizards' reactions (fear relief, gifts: `FriendTracker.ItemOffered`, `GiftRecieved`) | `SocialEventRecognizer` |
| `social_events_turn.rs` | the slugcat putting items on the ground (`ReleaseObject`, `TossObject`, the back spear laid down) and the room's `SocialEventRecognizer.Update` (owned items offered, lapsing, a lizard's gift received) | `SocialEventRecognizer`, `OwnedItemOnGround` |
| `social_memory.rs` | `SocialMemory` | `SocialMemory` |

#### Creature kinds

- `lizard/`: `mod.rs` (`Lizard`), `breed.rs` (`LizardBreedParams`, green only), `ai.rs`
  (`LizardAi` state, `LizardBehavior`, injury utility), `movement.rs` (`MovementAnimation`), `voice.rs` (`LizardVoice`),
  `bubble.rs` (`LizardBubble`), `graphics_link.rs` (state shared with `rw-graphics`, with the
  jaws' `internalContainerObjects`), `world.rs` (lizard realize/abstractize, `RealizeInRoom`);
  turn files in the table below.
- `fly/`: `mod.rs` (`Fly`), `update.rs` (`Fly.Update`), `ai.rs` + `ai_update.rs`
  (`FlyAi`, flocking), `room_ai.rs` (`FliesRoomAi`, hives), `world_ai.rs` (`FliesWorldAI`:
  the bat-fly world process), `swarm_rooms.rs` (`SwarmRoomMapper`), `turn.rs` (bat flies in the tick: world process, `ReadyForAI`, vessels, lobby turns; the creatures' graphics modules, `modules_no_longer_viewed`/`modules_now_viewed`, run by the camera's deferred room change).
- `overseer/`: `abstract_ai.rs` (`OverseerAbstractAi`), `ai.rs` (`OverseerAi` state,
  zip matrix), `body.rs` (`OverseerBody`), `update.rs` (the whole realized update),
  `tutorial.rs` (`TutorialBehavior`, `TutorialMessage`), `guide_state.rs` (`PlayerGuideState`),
  `world_ai.rs` (`OverseersWorldAI`: shelter finder, guide direction),
  `holo_light.rs` (`HologramLight`), `holo_light_turn.rs` (Monk guide light in the update list), `turn.rs` (an overseer's turn in the game: what it reads of the player and story, the effects it hands back), `hologram/` (the projected pictures: `mod.rs`
  `OverseerHolograms`, `add.rs` `TryAddHologram`, `update.rs`, `parts.rs`, `instruction.rs`,
  `world_queries.rs`).
- `path_finder/`: `mod.rs` (`PathFinder`), `mapper.rs` (`AccessibilityMapper`, world path
  finders), `lizard.rs` (`PatherKind`, the lizard pather's cost functions).

| Lizard turn file | Holds |
|---|---|
| `lizard/turn.rs` | realized lizards in the update list, vessels, `SpitOutOfShortCut` |
| `lizard/act_turn.rs` | lizard movement (`Act`, `FollowPath`, animations) |
| `lizard/ai_turn.rs` | lizard AI update: the flee heavy-AI gate, moods, `DetermineBehavior`, behaviours, `IdleSpotScore`, base AI module order |
| `lizard/hunt_turn.rs` | Hunt and Flee bodies, `AggressiveBehavior` (lounge decision, bites), fall risks, `BitCreature` |
| `lizard/fight_turn.rs` | Fighting and ActingOutMission bodies, the signals between lizards (`SendCommunication`, `RecieveCommunication`), a rival's anger on touch |
| `lizard/hits_turn.rs` | lizard damage dealt/taken, bites (and the jaws' internal container), stun, spear stick |
| `lizard/perception_turn.rs` | lizard eyes, tracker representation, relationships |
| `lizard/graphics_turn.rs` | game side of `LizardGraphics` (bubbles, drips, head light) |
| `lizard/voice_turn.rs` | `LizardVoice` update and emitters |

### `item/`

`weapon.rs` (`Weapon`, `WeaponKind::{Rock, Spear}`), `spear_stick.rs` (`SpearStick`, `SpearStickKind`),
`on_back_stick.rs` (`OnBackStick`, the Hunter's `AbstractOnBackStick`),
`tests/`.

| File | Holds (key types) | Ports |
|---|---|---|
| `hits_turn.rs` | thrown rock/spear vs creatures | `Weapon.Update` Thrown branch |
| `turn.rs` | slugcat grabbing: `Grabability` (dead lizards: `Drag`), `CanIPickThisUp`, pick-up candidate | `Player` item rules |
| `weapon_turn.rs` | rocks/spears in the update list, loops, sparks | - |
| `weapon_world.rs` | the world's realized rocks/spears per room, `ShortCutsReady`, the collision pass, `Abstractize` | `Room.Update`, `AbstractRoom.Abstractize`, `Weapon` |
| `mod.rs` | abstract objects (`AbstractObject`, `Entity`, `ObjectData`, `AbstractSpear`), save strings | `AbstractPhysicalObject` |

### `sound/`

| File | Holds (key types) | Ports |
|---|---|---|
| `noise_turn.rs` | in-room noises heard at once | `Room.InGameNoise` |
| `noise.rs` | `InGameNoise`, terrain-impact noise, `NoiseSink` (an updating object's noises and screen movements), `RoomCall`, `ImpactCalls` | `Noise/InGameNoise.cs` |
| `mod.rs` | `Room.PlaySound` events + `VirtualMicrophone` draws (disembodied and rect loops too, `SoundRect`) | `VirtualMicrophone.PlaySound`, `PlayDisembodiedLoop`, `PlayRectSound` |
| `water_sound.rs` | `WaterSound`: the water's calm surface loop (`RectangularDynamicSoundLoop` on the band under the surface, `SilenceWater`) | `Water.waterSounds`, `RectangularDynamicSoundLoop` |
| `water_sound_turn.rs` | the surface loop at the head of `Water.Update` and its emitters in the update list | `Room.Update` |
| `dripping_sound.rs` | `DrippingSound`: the cycle start's `Cycle_Start_Drips` loop (`DisembodiedDynamicSoundLoop`) | `DrippingSound` |

### `water/`

| File | Holds (key types) | Ports |
|---|---|---|
| `drips_turn.rs` | the ceiling drip in `Room.Update`, `Room.Loaded`'s `DrippingSound` and its turns (sound, emitters) in the update list | `Room.Update`, `Room.Loaded` |
| `waterfall.rs` | `WaterFall` (the falls a room's tiles make, ends, drips, surface hit, bubbles, push) | `WaterFall`, `Room.LoadFromDataString` |
| `waterfall_turn.rs` | a fall's turn in the update list: its drips, bubbles and push on bodies (made in `effects/lights/light_fixture_turn.rs`) | `WaterFall.Update` |
| `mod.rs` | wavy surface sim, `FloatWaterLevel`, `Water.Update`'s bottom roll, entry splashes and bubbles (`WaterObject`), `BubbleEmitter`s, `WaterfallHitSurface` | `Water`, `DefaultSurface` |
| `bubble.rs` | `Bubble` (`BubbleMode`): air bubbles in a room's update list | `Bubble` |
| `splash.rs` | `Splash`: water thrown up by bodies crossing the surface (update, sounds, terrain drips, `WaterfallHitSurface`) | `SplashWater.Splash`, `WaterParticle` |
| `water_drip.rs` | `WaterDrip` droplets; `Room.Update`'s ambient ceiling drip roll | `WaterDrip`, `Room.Update` |
| `world.rs` | a room's `Water.Update` fed every physical object's chunks (`physicalObjects` order), its splashes and bubbles into the room | `Water.Update`, `Room.physicalObjects` |

### `effects/`

| File | Holds (key types) | Ports |
|---|---|---|
| `cosmetic.rs` | `Spark`, `ExplosionSpikes`, `OverseerEffectSprite` sprites, `Bubble`s, `Splash`es and `NeuronSpark`s (`CosmeticKind`) in update lists | `CosmeticSprite`s |

### `effects/lights/`

| File | Holds (key types) | Ports |
|---|---|---|
| `light_beam_turn.rs` | placed `LightBeam`s and `SpotLight`s joining the update list, a beam's turn (environment colour), `ApplyPalette` | `Room.Loaded` (`LightBeam`, `SpotLight`), `LightBeam.Update` |
| `light_fixture_turn.rs` | placed `LightFixture`s (`Redlight`, `HolyFire`, `SlimeMoldLight`) and the tile parse's `WaterFall`s joining the update list, their turns in it (the `RedlightLight` made on the first update, the embers shed, a fall's drips, bubbles and push on bodies), `Room.ElectricPower` | `Room.Loaded` (`LightFixture`), `Room.LoadFromDataString` (waterfalls), `Redlight.Update`, `WaterFall.Update` |
| `turn.rs` | light sources in the update list: placed lights (`Room.Loaded`), the glowing player's light (`PlayerGlow`), camera 0's picture they read, `Darkness`, `LightSourceExposure`/`LightSourceColor`/`DarknessOfPoint` | `Room` light members, `LightSource.Update`, `PlayerGraphics.Update` light |
| `light_beam.rs` | `LightBeam` (quad, environment and palette colours), `SpotLight` (quad) | `LightBeam`, `SpotLight` |
| `light_source.rs` | `LightSource` (`Alpha`, `Lightness`, update) | `LightSource` |
| `light_fixture.rs` | `Redlight` (flicker, gravity-dependent power), `HolyFire` with `HolyFireSprite`, `SlimeMoldLight` (seeded discs) | `LightFixture`, `Redlight`, `HolyFire`, `SlimeMoldLight` |

### `effects/sky/`

| File | Holds (key types) | Ports |
|---|---|---|
| `lightning_turn.rs` | a room's `Lightning` in the game: `Room.Loaded` making it, its turn and room calls (`MakeBackgroundNoise`, `ScreenMovement`, `PlaySound`), its loop emitters' turns, hearing dulled by the background noise | `Room.Loaded` lightning, `MakeBackgroundNoise` |
| `lightning.rs` | `Lightning` (`Lightning`/`BkgOnlyLightning` effects): two `LightningSource`s (reset, flash curve, thunder queue, `Electricity_Loop`), the camera's reads (`CurrentLightLevel`, `CurrentBackgroundColor`, `CurrentFogColor`, `LightningFrame`) | `Lightning` |
| `background_scene.rs` | `BackgroundScene`: a room's scene object and its elements (`ElementKind`: skies, fog, distant buildings, clouds, a roof's floor, buildings, rubble and smoke), `DrawPos`, `PosFromDrawPosAtNeutralCamPos`, the add order | `BackgroundScene`, `BackgroundSceneElement` and the element classes' fields |
| `above_clouds_view.rs` | `AboveCloudsView`'s constructor: the 50 (Underhang) or 30 and 38 (Sky Islands) elements, altitudes, the clouds' draws under `InitState(12)`; `DistantLightning`: the bolt's timers (up to 3 shared draws a tick) and its brightness draw (cosmetic stream) | `AboveCloudsView`, `AboveCloudsView.DistantLightning` |
| `roof_top_view.rs` | `RoofTopView`'s constructor: sky, floor, 16 rubble strips, city, 4 smoke layers (4 shared draws); `DustpuffSpawner` (the player's chunks landing or running) and `DustPuff` | `RoofTopView`, `RoofTopView.DustpuffSpawner`, `DustPuff` |
| `background_scene_turn.rs` | scenes in the game: `Room.Loaded` making them, the first turn adding the elements far to near, the lightning's turns, the spawner's and puffs' turns | `Room.Loaded` (`AboveCloudsView`, `RoofTopView`), `BackgroundScene.Update` |

### `effects/particles/`

| File | Holds (key types) | Ports |
|---|---|---|
| `turn.rs` | the depth-faded particles in the game: `Room.Loaded`'s `SkyDandelions`/`GreenSparks` objects, `NowViewed`'s seeding of sparks, specks and spores (and its making the `SuperStructureProjector` and `ProjectedScanLines`), each one's turn in the update list, a spark's `LightSource`, the chunks the stalks read | `Room.Loaded`, `Room.NowViewed`, `GreenSparks` |
| `decal_turn.rs` | custom decals and rainbows joining the update list (the rainbow's `SeededRandom` gate), the rainbows' turns | `Room.Loaded` (`CustomDecal`, `Rainbow`, `RainbowNoFade`) |
| `placed_cosmetics_turn.rs` | cosmetic slime molds and dead token stalks joining the update list, the stalks' turns | `Room.Loaded` (`CosmeticSlimeMold`, `CosmeticSlimeMold2`, `SlimeMold`, `DeadTokenStalk`) |
| `decal.rs` | `CustomDecal`: grid division and the seeded grid mesh (positions, per-vertex colours) | `CustomDecal` |
| `rainbow.rs` | `Rainbow`/`RainbowNoFade`: position, radius, fade over the cycle's start-up, spawn condition | `Rainbow`, `RainbowNoFade` |
| `slime_mold.rs` | `CosmeticSlimeMold`: the seeded dots on the walls; the plain `SlimeMold`'s `SeededRandom` gate | `SlimeMold.CosmeticSlimeMold` |
| `token_stalk.rs` | `TokenStalk` without a token: arm, head, rope, two shared draws a tick | `CollectToken.TokenStalk` |
| `sky_dandelion.rs` | `SkyDandelions` spawner, `SkyDandelion`, `DandelionStalk`, the `DandelionPatch` planting (seeded) | `SkyDandelions.cs` |
| `green_sparks.rs` | `GreenSparks` spawner, `GreenSpark`, the `NowViewed` seeding | `GreenSparks.cs` |
| `zero_g_speck.rs` | `ZeroGSpeck` (`BlinkSpeck`, `CorruptionSpore`) on the `GenericZeroGSpeck` base; reads `Room.gravity` | `GenericZeroGSpeck.cs`, `BlinkSpeck.cs`, `CorruptionSpore.cs` |
| `room_particles.rs` | `RoomParticles`: a realized room's dandelions, stalks, sparks, specks and their spawners | `Room.updateList` |

### `effects/particles/insect/` (cosmetic insects; shared-stream draws, so simulated here)

`mod.rs` (`InsectType`, `InsectBase`, `CosmeticInsect`, `InsectCx`), `coordinator.rs`
(`InsectCoordinator`, `InsectGroup` placed objects, spawning), `avoider.rs`
(`CreatureAvoider`), one file per kind: `mini_fly.rs`, `fire_fly.rs`, `tiny_dragonfly.rs`,
`rock_flea.rs`, `grass_hopper.rs`, `red_swarmer.rs`, `ant.rs`, `beetle.rs` (also the
sitting/landing helpers Wasp and Moth share), `water_glowworm.rs`, `wasp.rs`, `moth.rs`;
`turn.rs` (the camera's deferred room change `apply_view_change`: `NoLongerViewed`/`NowViewed` of modules, particles and insects; cosmetic insects in the update list, creatures for the avoiders, the firefly's light); `tests/`.

### `effects/gameplay/` (effects that act on play)

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

### `superstructure/projections/`

| File | Holds (key types) | Ports |
|---|---|---|
| `projector.rs` | `SuperStructureProjector` (the `SuperStructureProjector` effect, 4 SS rooms): scan lines, cursors on a 15 px grid, single glyphs that spread, glyph matrices; its parts are update-list objects kept in creation order | `SuperStructureProjector` |
| `projected_circle.rs` | `ProjectedCircle` (a spoked ring behind a coral neuron's free end: constructor links to the circles already in the room, `Update`'s radius, blink and turn, `UpdateDepth`; base-game branches, D40) | `ProjectedCircle`, `IOwnProjectedCircles` |
| `scan_lines.rs` | `ProjectedScanLines` (3 SS rooms) and its lines | `ProjectedScanLines` |
| `star_matrix.rs` | `StarMatrix` (the placed `ProjectedStars`, 4 SS rooms): stars, connections, rings, the three rotations (the z one never moves), the projection | `StarMatrix` |
| `room_projections.rs` | a realized room's projectors, scan lines, star matrices and projected circles in its update list | `Room.updateList` |
| `turn.rs` | the projections' turns in the update list, `Room.Loaded`'s `ProjectedStars`, the shader's `_gridOffset`; `Room.NowViewed` makes the projector and scan lines in `effects/particles/turn.rs` | `Room.Loaded`, `StarMatrix.Update`, `SuperStructureProjector.UpdateOffset` |

### `superstructure/electric/`

| File | Holds (key types) | Ports |
|---|---|---|
| `turn.rs` | the superstructure's electric objects in the game: `Room.Loaded` making `SSLightRod`s, `ZapCoil`s, `SuperStructureFuses` and `ZapCoilLight`s (and `AIMapReady` linking each light to its coil), their turns in the update list (a coil's loops, the zaps over the room's `physicalObjects` that flash, kick and kill, a flash's light made on its first update) | `Room.Loaded`, `ZapCoil.Update`, `ZapCoilLight.AIMapReady`, `SuperStructureFuses.Update` |
| `light_rod.rs` | `SSLightRod` and its `LightVessel`s (a light every 45 px riding the rod, faded by what the camera sees of the rod's depth, scaled by `ElectricPower`) | `SSLightRod` |
| `zap_coil.rs` | `ZapCoil` (rectangle, disruption, spells off, power switch, the two rectangular sound loops, `TriggerZap`), `ZapFlash` (a blue flash with its light) and `ZapCoilLight` (a blue light following its coil) | `ZapCoil`, `ZapCoilLight` |
| `room_fixtures.rs` | a realized room's fixtures, falls and superstructure electric and screen-grab objects in its update list | `Room.updateList` |
| `fuses.rs` | `SuperStructureFuses` (a grid of blinking cell lights: modes, malfunction spells, gravity fade; no update while culled) | `SuperStructureFuses` |

### `superstructure/grab/`

| File | Holds (key types) | Ports |
|---|---|---|
| `turn.rs` | the superstructure's screen-grab objects in the game: `Room.Loaded` making `DeepProcessing`s, `DeepProcessingLight`s and `GravityDisruptor`s (the twenty specks first), `AIMapReady` collecting the room's zero-gravity specks, their turns in the update list, the disruptor's swirl over the room's `physicalObjects` | `Room.Loaded`, `GravityDisruptor.Update`, `GravityDisruptor.AIMapReady` |
| `deep_processing.rs` | `DeepProcessing` (a placed quad whose `power` chases `Room.ElectricPower`; the shader is `rw-render`'s) and `DeepProcessingLight` (a blue light following the electric power) | `DeepProcessing`, `DeepProcessingLight` |
| `gravity_disruptor.rs` | `GravityDisruptor` (the swirl's numbers for body chunks and zero-gravity specks, the sixteen streaks, the power following `CurrentAntiGravity`) and its `DisruptorSpeck`s | `GravityDisruptor`, `GravityDisruptor.DisruptorSpeck` |

### `superstructure/coral/`

| File | Holds (key types) | Ports |
|---|---|---|
| `stem.rs` | `CoralStem` and its `StemSegment`s: the seeded constructor (path search, segments with their neurons and strands, the stem's `ProjectedCircle`), each segment a two-chunk `PhysicalObject` of layer 0 with its turn (the terrain impacts' room calls, then the strands, neuron pull, constraint to the previous segment, root, walls and wind), the climbable vine members, `SegmentRef`, `StemTick`, `StemCx` (D41) | `CoralStem`, `StemSegment`, `IClimbableVine`, `IOwnProjectedCircles` |
| `circuit.rs` | `CoralCircuit` and its `CircuitBit`s: the clusters of placed objects, the seeded constructor (tiles, a bit or two each, neighbours, connectors, crabs), a bit's turn (physics, repair and break state machine, sounds), the push on a body chunk, `Explosion`, `BitRef`, `PushChunk` (D42) | `CoralCircuit`, `CoralCircuit.CircuitBit` |
| `circuit_connector.rs` | `CircuitConnector`: a dot on a bit with five to nine strands of the shared pool (`ConnectorAnchor` is their owner) | `CircuitConnector` |
| `circuit_crab.rs` | `CircuitCrab` and its `CrabLeg`s: the cosmetic walker on the bits | `CircuitCrab`, `CrabLeg` |
| `circuit_turn.rs` | the circuits in the game: `AIMapReady` building one per cluster (after the wall mycelia, before the stems), their objects in the update list (`CircuitRef`), the circuit's push over `physicalObjects` of layer 1 and up with the player's collision and sounds, `World::circuit_bit` | `CoralNeuronSystem.AIMapReady` (circuits), `CoralCircuit.Update` |
| `turn.rs` | the superstructure's coral in the game: `Room.Loaded` making the room's `CoralNeuronSystem` (an `SSSwarmers` effect or any coral or wall mycelia object), `AIMapReady` building the owners from the placed objects (the neurons as the loop meets them, each with the `ProjectedCircle`s it adds, then the wall mycelia, the circuits, then the stems; swarmers are a later step's), their turns in the update list (neurons, stem segments with the terrain impacts' noises, circles' depth read at the first draw), the `NeuronSpark`s strands strike, an overseer's graphics strands joining the system | `Room.Loaded` (`CoralNeuronSystem`), `CoralNeuronSystem.AIMapReady`, `Overseer.UpdateCoralNeuronSystem` |
| `neuron.rs` | `CoralNeuron` (the seeded constructor: wall-snapped ends, segments, two strands a pair, bumps, the circles it adds; `Update`'s chain, wall push, wind and pings; the climbable vine members; `NeuronGeometry` is what its strands ask of it) | `CoralNeuron` |
| `system.rs` | `CoralNeuronSystem` (wind, `age`, `notViewedCounter`, `Frozen`, its `neurons` and `wallMycelia`; the registry of strands is the pool's) | `CoralNeuronSystem` |
| `mycelium.rs` | `Mycelium` (a strand: chain, `Reset`, `Phase1` with the wall push, `Update`'s culling and pairing) and the world's `MyceliumPool` of every owner's strands with each system's registry; shared by the overseers' hairs and the wall mycelia | `Mycelium`, `IOwnMycelia`, `JobData` |
| `wall_mycelia.rs` | `WallMycelia` (the seeded constructor's strands rooted on walls, each tick's drift and bend) | `WallMycelia` |
| `neuron_spark.rs` | `NeuronSpark` (the flash two linked mycelia strike, with its sound), a `CosmeticKind` | `NeuronSpark` |

### `vines/`

| File | Holds (key types) | Ports |
|---|---|---|
| `mod.rs` | `ClimbableVinesSystem` (a room's list of `VineRef`s and the geometry the player climbs them with: overlap, place, direction, radius, climb speed, switch, connect, push), `IClimbableVine` as the `VineHost` / `VineHostMut` traits the live owners are read and called through, `PlacedVines` (vines registered at run time: tests), `RoomVines` / `RoomVinesView` (the room's host built from split borrows: placed vines and the coral system's neurons and stems), `VinePosition`, `VineKind`; no corruption vine object exists yet (D39) | `ClimbableVinesSystem`, `IClimbableVine` |

### `world/` (the loaded world and its abstract layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `World`: abstract rooms, realized set, arenas | `World`, `AbstractRoom` |
| `realized.rs`, `realizer.rs` | realized room data + ready states (`loadingProgress`, `readyForNonAICreaturesToEnter`, the stages' room-side steps, the realization `serial`); room realizer (`currentlyLoadingRoom` as a realization, waited on after an abstract) | `Room` bits, `RoomRealizer` |
| `room_preparer.rs` | `RoomPreparer` (status, hand-off requests, `done`, the realization `serial` it loads, outliving an abstracted room), `LoadingSchedule` (`GAME_MINIMUM`, the default; `Immediate` for world-only tests), `PreparerStage` | `RoomPreparer` |
| `creatures.rs` | creature arena, room lists, attractions | `AbstractRoom` creature lists |
| `objects.rs` | object arena, room items at the game's three places (file items, a story's `MultiplayerItem`s in the placed-object loop, the `firstTimeRealized` pass) | `Room.Loaded` objects |
| `dens.rs` | into/out of dens with stuck items | `MoveEntityToDen` |
| `quantified.rs` | quantified creatures placed in a room and counted back | `Room.PlaceQuantifiedCreaturesInRoom`, `AbstractRoom.CountQuantifiedCreatures` |
| `global_rain.rs` | `GlobalRain` (direction, `Intensity`, shakes, flood), `DeathRain`, `DeathRainMode` | `GlobalRain`, `GlobalRain.DeathRain` |
| `room_rain.rs` | `RoomRain` (`rainReach`, `splashTiles`, `intensity`, push properties, the per-chunk steps of `ThrowAroundObjects`), `SmashView` (`CreatureSmashedInGround`), `World::creature_smashed_in_ground` | `RoomRain` |
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

## Where is X?

| Question | Look in |
|---|---|
| Shortcuts / pipes, vessels, lobby | `game/shortcut.rs`, `player/shortcut.rs`, `game/shortcut_turn.rs` (lobby), `game/shortcut_helper.rs` + `shortcut_helper_turn.rs` + `player/shortcut_push.rs` (`ShortcutHelper`: wrong holes, objects pushed out of entrances), `creature/lizard/turn.rs`, `creature/fly/turn.rs`, `player/carried_turn.rs` (what a vessel carries) |
| Water, swimming, drowning | `water/mod.rs`, `water/world.rs`, `water/bubble.rs`, `water/splash.rs`, `player/swim.rs`, `water/water_drip.rs` |
| The water's surface sound | `sound/water_sound.rs`, `sound/water_sound_turn.rs` (audio mix: `rw-audio` `voice.rs`) |
| Ceiling drips, the cycle start's dripping sound | `water/water_drip.rs`, `sound/dripping_sound.rs`, `water/drips_turn.rs`, `game/screen_shake.rs` (shaken drips) |
| `rainWorld.screenSize` (the chosen resolution) | `World::screen_size`/`set_screen_size` (`world/mod.rs`; the app sets it before the first room loads), read by `effects/gameplay/border_push_back.rs`, `effects/gameplay/electric_death.rs` (`LightFlash`), `effects/sky/background_scene_turn.rs` (convergence point), `CameraShake` |
| Camera shake, `ScreenMovement` | `game/screen_shake.rs` (sim side), `rw-render` `room_camera.rs`, app `scene.rs` |
| Abstract creature AI (roam, migrate) | `creature/abstract_ai/` (`world.rs`, `path_finder.rs`, `mod.rs`) |
| Type-specific abstract AI | `creature/abstract_ai/kinds_world.rs`, `kinds.rs`, `creature/overseer/abstract_ai.rs` |
| Den logic | `world/dens.rs`, `creature/abstract_ai/world.rs` (den counters), `world/creatures.rs` |
| Population generation | `creature/population.rs` |
| Tracker / relationships | `creature/ai_modules/` (`tracker.rs`, `relationship_tracker.rs`, `tracker_turn.rs`, `tracker_world.rs`) |
| Noise and hearing | `sound/noise.rs`, `sound/noise_turn.rs`, `creature/ai_modules/noise_tracker_world.rs`, `noise_tracker.rs` |
| Spear sticking, lodged spears | `item/spear_stick.rs`, `item/weapon.rs`, `creature/lizard/hits_turn.rs`, `item/hits_turn.rs` |
| Thrown weapon hits | `item/hits_turn.rs`, `item/weapon.rs` |
| Lizard movement / path following | `creature/lizard/act_turn.rs`, `creature/lizard/movement.rs`, `creature/path_finder/` |
| Lizard decisions (behaviour, utilities, idle spot, den) | `creature/lizard/ai_turn.rs`, `creature/lizard/ai.rs`, `creature/ai_modules/world.rs`, `creature/ai_modules/utility_comparer.rs` |
| Lizard hunting, lounging, fleeing | `creature/lizard/hunt_turn.rs`, `creature/ai_modules/threat_tracker_world.rs`, `creature/ai_modules/heavy_ai_turn.rs` |
| Lizards fighting, signalling, leaving on a mission | `creature/lizard/fight_turn.rs`, `creature/ai_modules/mission_tracker.rs`, `mission_tracker_world.rs` |
| Room tile AI map | `world/ai_map/` |
| Shelters, sleep, hibernation | `game/shelter.rs`, `game/shelter_turn.rs`, `player/sleep.rs` |
| Save-state writing (sim side) | `world/region_state.rs`, `creature/state_save.rs`, `item/mod.rs` (object strings), `creature/overseer/guide_state.rs`, `game/map_discovery.rs`; text format in `rw-save` |
| Death, game over, karma flower | `game/game_over_turn.rs`, `creature/death.rs`, `player/falls.rs` |
| The player bitten and held by a lizard, struggling free | `player/grabbed_turn.rs`, `player/grabbed.rs`, `creature/lizard/hits_turn.rs` (`Bite`, the melee throw's hit) |
| Random draws / sounds | `sound/mod.rs`, `rw-random` crate; draws live beside each ported function |
| Camera visibility | `game/camera.rs` |
| `ElectricDeath` (Underhang), `BorderPushBack` (Sunlit Lands) | `effects/gameplay/electric_death.rs`, `effects/gameplay/electric_death_turn.rs`; `effects/gameplay/border_push_back.rs`, `effects/gameplay/border_push_back_turn.rs`; sprites in `rw-graphics` `effects/gameplay/electric_death.rs`, shaders in `rw-render` |
| Room triggers, guide instructions | `game/trigger.rs`, `game/trigger_turn.rs`, `game/instructions_turn.rs`, `game/room_script.rs` |
| Room realization / loading | `world/realizer.rs`, `world/realized.rs`, `world/room_preparer.rs`, `world/mod.rs`, `game/loading_turn.rs` |
| Rain cycle | `world/rain_cycle.rs` |
| The room rain (push, stun, rain death) | `world/room_rain.rs`, `game/room_rain_turn.rs`; `Creature.rainDeath` in `creature/mod.rs` / `player/mod.rs`, the lung cap in `player/swim.rs`; D45 |
| The global rain, the death rain's modes, the rain's shake and sky look | `world/global_rain.rs`, `game/rain_turn.rs`, `game/screen_shake.rs`, `water/mod.rs` (`SurfaceRumble`), `rw-graphics` `player/graphics.rs` (`sky_look`); D44 |
| Eating, picking up, throwing | `player/turn.rs`, `item/turn.rs`, `player/grab.rs`; creatures' meat: `player/eat_meat_turn.rs`; carrying a heavy body: `player/carry.rs`, `player/turn.rs` |
| The Hunter's spear on the back | `player/spear_on_back_turn.rs`, `player/spear_on_back.rs`, `item/on_back_stick.rs` |
| Body physics / collision | `physics/chunk.rs`, `physics/object.rs`, `player/mod.rs` |
| Climbable vines (the player climbing, the vine geometry) | `vines/mod.rs`, `player/vines.rs`; hands: `rw-graphics` `player/hand.rs` |
| Zero gravity (room `gravity`, `ElectricPower`, the broken switch, the player floating) | `effects/gameplay/anti_gravity.rs`, `effects/gameplay/anti_gravity_turn.rs`, `player/zero_g.rs`, `PhysicsParams::room_gravity` in `physics/chunk.rs`; the SS song: `game/trigger_turn.rs` (`SsMusic`) and `rw-audio` `music/song.rs` |
| Overseer / guide | `creature/overseer/` (`turn.rs`: in the game), `creature/overseer/world_ai.rs`, `creature/overseer/holo_light_turn.rs` |
| Placed light beams, spot lights | `effects/lights/light_beam.rs`, `effects/lights/light_beam_turn.rs`; sprites in `rw-graphics` `effects/lights/light_beam.rs` (beams and spot lights), `effects/lights/quad_grid.rs` |
| Superstructure rods, zap coils (and their lights, flashes), fuses | `superstructure/electric/light_rod.rs`, `superstructure/electric/zap_coil.rs`, `superstructure/electric/fuses.rs`, `superstructure/electric/turn.rs`; sprites in `rw-graphics` `superstructure/electric.rs` |
| Coral (`CoralNeuronSystem`, `CoralNeuron`, `ProjectedCircle`, `WallMycelia`, `Mycelium`, `NeuronSpark`) | `superstructure/coral/system.rs`, `superstructure/coral/neuron.rs`, `superstructure/projections/projected_circle.rs`, `superstructure/coral/wall_mycelia.rs`, `superstructure/coral/mycelium.rs`, `superstructure/coral/neuron_spark.rs`, `superstructure/coral/turn.rs`, `superstructure/projections/turn.rs`; sprites in `rw-graphics` `superstructure/coral/neuron.rs`, `superstructure/projections/projected_circle.rs`, `superstructure/coral/mycelium.rs` (strands and wall mycelia), `effects/cosmetic.rs` (neuron sparks) |
| Coral circuits (`CoralCircuit`, `CircuitBit`, `CircuitConnector`, `CircuitCrab`) and the player swimming through them | `superstructure/coral/circuit.rs`, `superstructure/coral/circuit_connector.rs`, `superstructure/coral/circuit_crab.rs`, `superstructure/coral/circuit_turn.rs`, `player/circuit.rs`, `player/zero_g.rs`; sprites in `rw-graphics` `superstructure/coral/circuit.rs`, shaders in `rw-render` `sprite.wgsl` (`fs_coral_circuit`) |
| Superstructure screen-grab effects (`DeepProcessing`, `DeepProcessingLight`, `GravityDisruptor`) | `superstructure/grab/deep_processing.rs`, `superstructure/grab/gravity_disruptor.rs`, `superstructure/grab/turn.rs`; sprites in `rw-graphics` `superstructure/grab.rs`, shaders in `rw-render` `ss_grab.wgsl` |
| Light fixtures (`Redlight`, `HolyFire`, `SlimeMoldLight`), waterfalls | `effects/lights/light_fixture.rs`, `water/waterfall.rs`, `superstructure/electric/room_fixtures.rs`, `effects/lights/light_fixture_turn.rs`, `water/waterfall_turn.rs`; sprites in `rw-graphics` `effects/lights/light_fixture.rs`, `water/waterfall.rs` |
| Lights (placed, player glow, room light queries) | `effects/lights/light_source.rs`, `effects/lights/turn.rs`, `game/camera_image.rs`; sprites in `rw-graphics` `effects/lights/light_source.rs` |
| Bat flies | `creature/fly/`, `creature/fly/world_ai.rs`, `creature/fly/turn.rs` |
| Cosmetic insects (Flies, RockFlea, ... effects, `InsectGroup`) | `effects/particles/insect/`, `effects/particles/insect/turn.rs`; sprites in `rw-graphics` `effects/particles/insect.rs` |
| Dev spawns / teleport | `game/dev_turn.rs`, `game/spawn.rs` |
| Custom decals, rainbows | `effects/particles/decal.rs`, `effects/particles/rainbow.rs`, `effects/particles/decal_turn.rs`; sprites in `rw-graphics`, shaders in `rw-render` |
| Cosmetic slime molds, dead token stalks | `effects/particles/slime_mold.rs`, `effects/particles/token_stalk.rs`, `effects/particles/placed_cosmetics_turn.rs`; sprites in `rw-graphics` |
| Background scenes (`AboveCloudsView`, `RoofTopView`), distant lightning, dust puffs | `effects/sky/background_scene.rs`, `effects/sky/above_clouds_view.rs` (distant lightning), `effects/sky/roof_top_view.rs` (dust puffs), `effects/sky/background_scene_turn.rs`; sprites in `rw-graphics` `effects/sky/background_scene.rs`, shaders in `rw-render` `background_scene.wgsl` |

## Cross-links

Notes (`../../../rainworld-re/notes/`): `physics-core.md`, `slope-collision.md`,
`slugcat-movement.md`, `advanced-moves.md`, `falls.md`, `corridors.md`,
`vertical-poles.md`, `horizontal-beams.md`, `ledge-grab.md`, `shortcuts.md`, `water.md`,
`ai-map.md`, `creature-model/`, `items.md`, `shelter.md`, `saves.md`, `sound.md`,
`unity-random.md`, `overseer.md`, `lizard-ai.md`, `room-realizer.md`, `world-format.md`,
`world-loading.md`, `room-format.md`, `campaign-model.md`, `music.md`.
Surveys (`../../../rainworld-re/surveys/`): `lizards.md`, `bat-flies.md`, `rocks-spears.md`,
`spawning.md`, `violence-templates.md`, `items-room-edges.md`, `campaign.md`,
`sleep-death-screens.md`, `malnourishment.md`, `food-pip-clip.md`.
(`notes/intro-overseer-survey.md` covers the guide and `game/room_script.rs`.)

Divergences: `../../docs/divergences/` (IDs are global, `grep -n '^## D'`). In `sim.md`:
D2 spawning, D3 movement, D4 falls, D5 shortcuts, D6 poles/ledges, D9 skid/roll/belly
slide, D10 corridors, D11 water, D14 worlds/rooms/realizer (creatures, lizards, flies,
overseers), D17 shelters, D19 the player held by a creature; D33 (`sim-gameplay-effects.md`) `ElectricDeath` and `BorderPushBack`. Elsewhere: D1 `random.md`, D12 `audio.md`, D13 `menu.md`,
D16 `save.md`, D18 `app.md`.

Tests: `tests/` folders beside their modules (see Layout), and `../tests/` (18 `installed_*.rs` integration tests that read the installed game, plus `data/`).
