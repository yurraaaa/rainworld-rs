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
| `game_carried.rs`, `game_carried_tests.rs` | what a creature's vessel carries: `FollowedByCamera`, stuck objects moving out of the lobby, the held player spat out with its grabber or killed in its den | `AbstractCreature.FollowedByCamera`, `Creature.SpitOutOfShortCut` |
| `game_dev.rs` | dev spawns/teleports through the game's own paths (MCP, debug UI) | - (port dev tool, D18) |
| `game_exit.rs` | what pause/`ExitGame` read | `RainWorldGame.ExitGame` |
| `game_flies.rs`, `game_graphics_modules_tests.rs` | bat flies in the tick: world process, `ReadyForAI`, vessels, lobby turns; the creatures' graphics modules at the camera's room change (`NoLongerViewed`, `NowViewed` by collision layer) | `FliesWorldAI` hookup, `Room.NowViewed`/`NoLongerViewed` modules |
| `game_insects.rs`, `game_insects_tests.rs` | cosmetic insects in the update list: `NowViewed`/`NoLongerViewed` at the camera's room change, creatures for the avoiders, the firefly's light | `Room.NowViewed` insect branch |
| `game_hits.rs` | thrown rock/spear vs creatures | `Weapon.Update` Thrown branch |
| `game_holo_light.rs` | Monk guide light in the update list | `HologramLight.TryCreate` |
| `game_light_beams.rs`, `game_light_beams_tests.rs` | placed `LightBeam`s and `SpotLight`s joining the update list, a beam's turn (environment colour), `ApplyPalette` | `Room.Loaded` (`LightBeam`, `SpotLight`), `LightBeam.Update` |
| `game_light_fixtures.rs`, `game_light_fixtures_tests.rs`, `game_waterfalls.rs` | placed `LightFixture`s (`Redlight`, `HolyFire`, `SlimeMoldLight`) and the tile parse's `WaterFall`s joining the update list, their turns in it (the `RedlightLight` made on the first update, the embers shed, a fall's drips, bubbles and push on bodies), `Room.ElectricPower` | `Room.Loaded` (`LightFixture`), `Room.LoadFromDataString` (waterfalls), `Redlight.Update`, `WaterFall.Update` |
| `game_lightning.rs`, `game_lightning_tests.rs` | a room's `Lightning` in the game: `Room.Loaded` making it, its turn and room calls (`MakeBackgroundNoise`, `ScreenMovement`, `PlaySound`), its loop emitters' turns, hearing dulled by the background noise | `Room.Loaded` lightning, `MakeBackgroundNoise` |
| `game_lights.rs`, `game_lights_tests.rs` | light sources in the update list: placed lights (`Room.Loaded`), the glowing player's light (`PlayerGlow`), camera 0's picture they read, `Darkness`, `LightSourceExposure`/`LightSourceColor`/`DarknessOfPoint` | `Room` light members, `LightSource.Update`, `PlayerGraphics.Update` light |
| `game_instructions.rs` | `Room.Loaded`'s placed room objects (guide triggers, placed lights, slime molds, dead token stalks), input-instruction triggers | `SuperJumpInstruction`, `ReliableIggyDirection` |
| `game_particles.rs`, `game_particles_tests.rs` | the depth-faded particles in the game: `Room.Loaded`'s `SkyDandelions`/`GreenSparks` objects, `NowViewed`'s seeding of sparks, specks and spores, each one's turn in the update list, a spark's `LightSource`, the chunks the stalks read | `Room.Loaded`, `Room.NowViewed`, `GreenSparks` |
| `game_decals.rs`, `game_decals_tests.rs` | custom decals and rainbows joining the update list (the rainbow's `SeededRandom` gate), the rainbows' turns | `Room.Loaded` (`CustomDecal`, `Rainbow`, `RainbowNoFade`) |
| `game_placed_cosmetics.rs`, `game_placed_cosmetics_tests.rs` | cosmetic slime molds and dead token stalks joining the update list, the stalks' turns | `Room.Loaded` (`CosmeticSlimeMold`, `CosmeticSlimeMold2`, `SlimeMold`, `DeadTokenStalk`) |
| `game_items.rs` | slugcat grabbing: `Grabability` (dead lizards: `Drag`), `CanIPickThisUp`, pick-up candidate | `Player` item rules |
| `game_eat_meat.rs`, `game_eat_meat_tests.rs` | eating a held dead creature: `CanEatMeat`, `GrabUpdate`'s meat block, `EatMeatUpdate` (bites, drips, food) | `Player.EatMeatUpdate` |
| `game_lizards.rs` | realized lizards in the update list, vessels, `SpitOutOfShortCut` | `Lizard.Update` |
| `game_lizard_act.rs` | lizard movement (`Act`, `FollowPath`, animations) | `Lizard.Act` |
| `game_lizard_ai.rs`, `game_lizard_ai_tests.rs` | lizard AI update: the flee heavy-AI gate, moods, `DetermineBehavior`, behaviours, `IdleSpotScore`, base AI module order | `LizardAI.Update`, `ArtificialIntelligence.Update` |
| `game_lizard_hunt.rs`, `game_lizard_hunt_tests.rs` | Hunt and Flee bodies, `AggressiveBehavior` (lounge decision, bites), fall risks, `BitCreature` | `LizardAI` |
| `game_lizard_fight.rs`, `game_lizard_fight_tests.rs` | Fighting and ActingOutMission bodies, the signals between lizards (`SendCommunication`, `RecieveCommunication`), a rival's anger on touch | `LizardAI`, `Lizard.Collide` |
| `game_heavy_ai.rs` | `HeavyAiThrottle`: `RequestHeavyAi`'s three-a-tick limit | `RainWorldGame.RequestHeavyAi` |
| `game_lizard_hits.rs` | lizard damage dealt/taken, bites (and the jaws' internal container), stun, spear stick | `Lizard.Violence`, `Bite` |
| `game_lizard_perception.rs` | lizard eyes, tracker representation, relationships | `LizardAI` (tracker parts) |
| `game_lizard_graphics.rs` | game side of `LizardGraphics` (bubbles, drips, head light) | `LizardGraphics` hookup |
| `game_lizard_voice.rs` | `LizardVoice` update and emitters | `LizardVoice` hookup |
| `game_map.rs` | map discovery from the player's position | `Player.Update` discovery |
| `game_noise.rs` | in-room noises heard at once | `Room.InGameNoise` |
| `game_over.rs` | `GameOver`, karma flower spot | `RainWorldGame.GameOver` |
| `game_player.rs`, `game_jump_chunk_tests.rs` | slugcat hands: bat-fly catch, eat, swallow, throw, hand switch; the heavy carry's rod and the body lent to the player's update; the jump chunk's masses lent and `JumpOnChunk`'s push and stun applied | `Player.GrabUpdate`, `Collide`, `JumpOnChunk` |
| `game_player_grabbed.rs`, `game_player_grabbed_tests.rs` | the player held: `creature_grab` (`CanBeGrabbed`, `Player.Grabbed`), the `dangerGrasp` block, `ThrowToGetFree`, `DangerGraspPickup`, `GameOver(dangerGrasp)`, the release under the room | `Player.Update`, `Creature.Grab` |
| `game_social_events.rs`, `game_social_events_tests.rs` | the slugcat putting items on the ground (`ReleaseObject`, `TossObject`, the back spear laid down) and the room's `SocialEventRecognizer.Update` (owned items offered, lapsing, a lizard's gift received) | `SocialEventRecognizer`, `OwnedItemOnGround` |
| `game_spear_on_back.rs`, `game_spear_on_back_tests.rs` | the Hunter's spear on the back: stash/retrieve by holding pick-up, `SpearToBack`, `SpearToHand`, `DropSpear` applied, carried on the back | `Player.SpearOnBack` |
| `game_shelter.rs` | shelter door in the room, sleep/hibernate checks, `KillAllHostiles`, `DoorClosed` | `ShelterDoor` hookup |
| `game_tracker.rs` | realized AI `Tracker`/`RelationshipTracker` running | `Tracker.cs`, `RelationshipTracker.cs` |
| `game_triggers.rs` | `ActiveTriggerChecker`s, `FireEvent`s | `Room.Loaded` triggers |
| `game_drips.rs`, `game_drips_tests.rs` | the ceiling drip in `Room.Update`, `Room.Loaded`'s `DrippingSound` and its turns (sound, emitters) in the update list | `Room.Update`, `Room.Loaded` |
| `game_weapons.rs` | rocks/spears in the update list, loops, sparks | - |
| `*_tests.rs`, `tests.rs` | tests for their `game_*`/`shelter` siblings; `tests.rs` is physics unit tests | - |
| `camera.rs` | what the sim reads of camera 0, visibility tests | `RoomCamera` |
| `campaign.rs` | per-slugcat campaign data: food, start den, karma | `SlugcatStats`, `SaveState` |
| `chunk.rs` | `BodyChunk` integration + tile collision | `BodyChunk.Update` |
| `object.rs` | `PhysicalObject`: chunks + distance constraints | `PhysicalObject` |
| `item.rs` | abstract objects (`AbstractObject`, `Entity`, `ObjectData`, `AbstractSpear`), save strings | `AbstractPhysicalObject` |
| `light_beam.rs`, `spot_light.rs` | `LightBeam` (quad, environment and palette colours), `SpotLight` (quad) | `LightBeam`, `SpotLight` |
| `lightning.rs`, `lightning_tests.rs` | `Lightning` (`Lightning`/`BkgOnlyLightning` effects): two `LightningSource`s (reset, flash curve, thunder queue, `Electricity_Loop`), the camera's reads (`CurrentLightLevel`, `CurrentBackgroundColor`, `CurrentFogColor`, `LightningFrame`) | `Lightning` |
| `light_source.rs` | `LightSource` (`Alpha`, `Lightness`, update) | `LightSource` |
| `light_fixture.rs`, `light_fixture_tests.rs` | `Redlight` (flicker, gravity-dependent power), `HolyFire` with `HolyFireSprite`, `SlimeMoldLight` (seeded discs) | `LightFixture`, `Redlight`, `HolyFire`, `SlimeMoldLight` |
| `waterfall.rs`, `waterfall_tests.rs`, `room_fixtures.rs` | `WaterFall` (the falls a room's tiles make, ends, drips, surface hit, bubbles, push) and a realized room's fixtures and falls | `WaterFall`, `Room.LoadFromDataString` |
| `camera_image.rs` | `CameraImage`: camera 0's level image and palette (`PixelColorAtCoordinate`, effect colours) | `RoomCamera.PixelColorAtCoordinate` |
| `map_discovery.rs` | per-region discovery textures, base64 PNG form | `PlayerProgression` map textures |
| `shared_physics.rs` | point terrain collision, exact tile ray trace | `SharedPhysics`, `Custom.RectCollision` |
| `noise.rs` | `InGameNoise`, terrain-impact noise, `NoiseSink` (an updating object's noises and screen movements), `RoomCall`, `ImpactCalls` | `Noise/InGameNoise.cs` |
| `room_ext.rs`, `color.rs`, `float_ext.rs`, `vec2.rs`, `vec3.rs` | position lookups on `Room`; `Custom` colour/scalar helpers; vectors | `RWCustom.Custom` |
| `room_script.rs` | room-specific scripts (`RoomScript`, SU_C04 start) | `RoomSpecificScript` |
| `screen_shake.rs`, `screen_shake_tests.rs`, `game_screen_shake_tests.rs` | camera 0's shake: `CameraShake` (`screenShake`, `microShake`, the per-tick jump at the head of the tick), `ScreenMovement` (camera side, `Room.ScreenMovement` with its ceiling drips), `TerrainImpact`'s bump, `ceilingTiles` | `RoomCamera.Update/ScreenMovement`, `Room.ScreenMovement` |
| `shelter.rs` | `ShelterDoor` (intact, base game) | `ShelterDoor` |
| `shortcut.rs` | `ShortcutHandler` divider, vessels, entrance blinking | `ShortcutHandler` |
| `sound.rs` | `Room.PlaySound` events + `VirtualMicrophone` draws (disembodied and rect loops too, `SoundRect`) | `VirtualMicrophone.PlaySound`, `PlayDisembodiedLoop`, `PlayRectSound` |
| `spawn.rs` | safe spawn tile, placing an object | `DetermineSafeSpawnTile`, `PlaceInRoom` |
| `trigger.rs` | triggers in a realized room, event-made room objects | `ActiveTriggerChecker` |
| `water.rs` | wavy surface sim, `FloatWaterLevel`, `Water.Update`'s bottom roll, entry splashes and bubbles (`WaterObject`), `BubbleEmitter`s, `WaterfallHitSurface` | `Water`, `DefaultSurface` |
| `bubble.rs`, `bubble_tests.rs` | `Bubble` (`BubbleMode`): air bubbles in a room's update list | `Bubble` |
| `splash.rs`, `splash_tests.rs` | `Splash`: water thrown up by bodies crossing the surface (update, sounds, terrain drips, `WaterfallHitSurface`) | `SplashWater.Splash`, `WaterParticle` |
| `game_water_tests.rs` | creatures' chunks in `Water.Update`, the water's splashes' and bubbles' first update, the drowning lizard's bubble | - |
| `water_drip.rs` | `WaterDrip` droplets; `Room.Update`'s ambient ceiling drip roll | `WaterDrip`, `Room.Update` |
| `water_sound.rs`, `water_sound_tests.rs` | `WaterSound`: the water's calm surface loop (`RectangularDynamicSoundLoop` on the band under the surface, `SilenceWater`) | `Water.waterSounds`, `RectangularDynamicSoundLoop` |
| `game_water_sound.rs`, `game_water_sound_tests.rs` | the surface loop at the head of `Water.Update` and its emitters in the update list | `Room.Update` |
| `dripping_sound.rs`, `dripping_sound_tests.rs` | `DrippingSound`: the cycle start's `Cycle_Start_Drips` loop (`DisembodiedDynamicSoundLoop`) | `DrippingSound` |
| `decal.rs`, `decal_tests.rs` | `CustomDecal`: grid division and the seeded grid mesh (positions, per-vertex colours) | `CustomDecal` |
| `rainbow.rs`, `rainbow_tests.rs` | `Rainbow`/`RainbowNoFade`: position, radius, fade over the cycle's start-up, spawn condition | `Rainbow`, `RainbowNoFade` |
| `slime_mold.rs` | `CosmeticSlimeMold`: the seeded dots on the walls; the plain `SlimeMold`'s `SeededRandom` gate | `SlimeMold.CosmeticSlimeMold` |
| `token_stalk.rs` | `TokenStalk` without a token: arm, head, rope, two shared draws a tick | `CollectToken.TokenStalk` |
| `sky_dandelion.rs` | `SkyDandelions` spawner, `SkyDandelion`, `DandelionStalk`, the `DandelionPatch` planting (seeded) | `SkyDandelions.cs` |
| `green_sparks.rs` | `GreenSparks` spawner, `GreenSpark`, the `NowViewed` seeding | `GreenSparks.cs` |
| `zero_g_speck.rs` | `ZeroGSpeck` (`BlinkSpeck`, `CorruptionSpore`) on the `GenericZeroGSpeck` base; reads `Room.gravity` | `GenericZeroGSpeck.cs`, `BlinkSpeck.cs`, `CorruptionSpore.cs` |
| `room_particles.rs` | `RoomParticles`: a realized room's dandelions, stalks, sparks, specks and their spawners | `Room.updateList` |
| `cosmetic.rs` | `Spark`, `ExplosionSpikes`, `OverseerEffectSprite` sprites, `Bubble`s and `Splash`es (`CosmeticKind`) in update lists | `CosmeticSprite`s |

## `player/` (slugcat; `Player.cs` split by topic)

`mod.rs` (`Player`, two-chunk body), `input.rs` (`Input`), `movement.rs`
(`MovementUpdate`), `body_mode.rs` (`UpdateBodyMode`), `animation.rs`
(`UpdateAnimation`), `jump.rs`, `jump_on_chunk.rs` (`jumpChunk`: `Collide`'s note of
the chunk under the hips, `JumpOnChunk`), `advanced.rs` (flip, roll, rocket jump, belly slide),
`poles.rs`, `ledge.rs`, `corridor.rs`, `swim.rs` (water, breath, `LungUpdate`),
`falls.rs` (impact, stun, die), `grabbed.rs` (held: `dangerGrasp` state and timers,
`cantBeGrabbedCounter`, the pacifying stun, `WALLSTUCK`), `exhaustion.rs` (aerobic level, malnourishment),
`grab.rs` (hand counters, `ThrowDirection`), `carry.rs` (`HeavyCarry`: run slowdown, drag through narrow spaces, pulled into a shortcut, jump power), `shortcut.rs` (enter/ride/leave tubes),
`sleep.rs` (`SleepUpdate`), `sounds.rs` (slide loop, plays), `drips.rs` (the skid's and
corridor shoots' `WaterDrip`s), `spear_on_back.rs` (the Hunter's
`SpearOnBack` state, `DropSpear`'s draws in `Stun` and `Die`). `tests.rs`, `tests/`: tests.

## `world/` (the loaded world and its abstract layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `World`: abstract rooms, realized set, arenas | `World`, `AbstractRoom` |
| `realized.rs`, `realizer.rs` | realized room data + ready states; room realizer | `Room` bits, `RoomRealizer` |
| `creatures.rs` | creature arena, room lists, attractions | `AbstractRoom` creature lists |
| `objects.rs` | object arena, room items on first realization | `Room.Loaded` objects |
| `weapons.rs` | realized rocks/spears per room, `ShortCutsReady` | - |
| `room_water.rs` | a room's `Water.Update` fed every physical object's chunks (`physicalObjects` order), its splashes and bubbles into the room | `Water.Update`, `Room.physicalObjects` |
| `dens.rs` | into/out of dens with stuck items | `MoveEntityToDen` |
| `abstract_ai.rs`, `abstract_path.rs` | plain abstract AI (roam, migrate, den); abstract pathfinders | `AbstractCreatureAI`, `AbstractSpacePathFinder` |
| `abstract_ai_kinds.rs` | vulture, cicada, big eel, deer, Miros bird, needle worm behaviours | `*AbstractAI` |
| `flies_world_ai.rs`, `swarm_rooms.rs`, `quantified.rs` | bat-fly world process, swarm map, quantified counts | `FliesWorldAI`, `SwarmRoomMapper` |
| `overseers_world_ai.rs` | shelter finder, guide direction | `OverseersWorldAI` |
| `lizards.rs` | lizard realize/abstractize, `RealizeInRoom` | `AbstractCreature.Realize` |
| `ai_map.rs`, `ai_mapper.rs`, `creature_ai_map.rs`, `movement.rs` | `AiMap`, `AiMapper`, baked per-creature maps, accessibility/connection/cost vocabulary | `AImap`, `AImapper` |
| `quick_path.rs`, `quick_connectivity.rs` | `QuickPathFinder`, `QuickConnectivity` (`Check`, `QuickPath`) | same |
| `hearing.rs` | noise delivery to creatures and `NoiseTracker`; `LizardAI.ReactToNoise` | `Room.InGameNoise`, `HeardNoise` |
| `ai_modules.rs` | prey/threat/aggression trackers' and den finder's updates, `SortCreatureIntoModule`, threat of a tile, threat travel cost, `NewArea` | `PreyTracker`, `ThreatTracker`, `AgressionTracker`, `DenFinder` |
| `threat_flight.rs` | the threat tracker's flight: `FleeTo`, `EvaluateFlightDestThreat`, `GenerateRandomPath`, `FindMostAttractiveExit` | `ThreatTracker` |
| `missions.rs`, `missions_tests.rs` | `LeaveRoom.Act`: the exit a submitted lizard leaves by and where it heads | `MissionTracker` |
| `tracking.rs`, `social_events.rs` | tracker answers (relationships); `SocialEventId`, the recognizer (`ownedItemsOnGround`, `ItemOffered`) and its events, lizards' reactions (fear relief, gifts: `FriendTracker.ItemOffered`, `GiftRecieved`) | `DynamicRelationship`, `SocialEventRecognizer` |
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
| `centipede.rs` | `AbstractCreature::centipede_generate_size` (size from the ID, for its icon) | `Centipede.GenerateSize` |
| `death.rs`, `grasp.rs`, `danger.rs` | kill tag/die, `Killing`, `AddCreatureToRespawn` and the lineage roll; `Grasp` + grip sticks; threat/danger reads | `Creature.Die`, `Creature.Grab` |
| `tracker.rs`, `relationship_tracker.rs`, `looker.rs`, `vision.rs` | AI modules | `Tracker`, `RelationshipTracker`, `CreatureLooker`, `VisualScore` |
| `noise_tracker.rs`, `stuck_tracker.rs` | `NoiseTracker`; `StuckTracker`, `DiscomfortTracker` | same |
| `friend_tracker.rs` | `FriendTracker`'s gift half: `tamingDifficlty`, `giftOfferedToMe` (no friend: taming is not ported) | `FriendTracker` |
| `prey_tracker.rs`, `threat_tracker.rs`, `agression_tracker.rs`, `rain_tracker.rs`, `den_finder.rs`, `utility_comparer.rs`, `mission_tracker.rs` | AI modules' state (their world reads are `world/ai_modules.rs`, `world/missions.rs`); `UtilityComparer` with `FloatTweener`; missions | same |

- `lizard/`: `mod.rs` (`Lizard`), `breed.rs` (`LizardBreedParams`, green only), `ai.rs`
  (`LizardAi` state, `LizardBehavior`, injury utility), `movement.rs` (`MovementAnimation`), `voice.rs` (`LizardVoice`),
  `bubble.rs` (`LizardBubble`), `graphics_link.rs` (state shared with `rw-graphics`, with the
  jaws' `internalContainerObjects`).
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

## `insect/` (cosmetic insects; shared-stream draws, so simulated here)

`mod.rs` (`InsectType`, `InsectBase`, `CosmeticInsect`, `InsectCx`), `coordinator.rs`
(`InsectCoordinator`, `InsectGroup` placed objects, spawning), `avoider.rs`
(`CreatureAvoider`), one file per kind: `mini_fly.rs`, `fire_fly.rs`, `tiny_dragonfly.rs`,
`rock_flea.rs`, `grass_hopper.rs`, `red_swarmer.rs`, `ant.rs`, `beetle.rs` (also the
sitting/landing helpers Wasp and Moth share), `water_glowworm.rs`, `wasp.rs`, `moth.rs`;
`tests.rs`.

## `item/`

`weapon.rs` (`Weapon`, `WeaponKind::{Rock, Spear}`), `spear_stick.rs` (`SpearStick`, `SpearStickKind`),
`on_back_stick.rs` (`OnBackStick`, the Hunter's `AbstractOnBackStick`),
`weapon_tests.rs`.

## Where is X?

| Question | Look in |
|---|---|
| Shortcuts / pipes, vessels, lobby | `shortcut.rs`, `player/shortcut.rs`, `game.rs` (lobby), `game_lizards.rs`, `game_flies.rs`, `game_carried.rs` (what a vessel carries) |
| Water, swimming, drowning | `water.rs`, `world/room_water.rs`, `bubble.rs`, `splash.rs`, `player/swim.rs`, `water_drip.rs` |
| The water's surface sound | `water_sound.rs`, `game_water_sound.rs` (audio mix: `rw-audio` `voice.rs`) |
| Ceiling drips, the cycle start's dripping sound | `water_drip.rs`, `dripping_sound.rs`, `game_drips.rs`, `screen_shake.rs` (shaken drips) |
| Camera shake, `ScreenMovement` | `screen_shake.rs` (sim side), `rw-render` `room_camera.rs`, app `scene.rs` |
| Abstract creature AI (roam, migrate) | `world/abstract_ai.rs`, `abstract_path.rs`, `creature/ai.rs` |
| Type-specific abstract AI | `world/abstract_ai_kinds.rs`, `creature/ai_kinds.rs`, `creature/overseer/abstract_ai.rs` |
| Den logic | `world/dens.rs`, `world/abstract_ai.rs` (den counters), `world/creatures.rs` |
| Population generation | `creature/population.rs` |
| Tracker / relationships | `creature/tracker.rs`, `relationship_tracker.rs`, `game_tracker.rs`, `world/tracking.rs` |
| Noise and hearing | `noise.rs`, `game_noise.rs`, `world/hearing.rs`, `creature/noise_tracker.rs` |
| Spear sticking, lodged spears | `item/spear_stick.rs`, `item/weapon.rs`, `game_lizard_hits.rs`, `game_hits.rs` |
| Thrown weapon hits | `game_hits.rs`, `item/weapon.rs` |
| Lizard movement / path following | `game_lizard_act.rs`, `creature/lizard/movement.rs`, `creature/path_finder/` |
| Lizard decisions (behaviour, utilities, idle spot, den) | `game_lizard_ai.rs`, `creature/lizard/ai.rs`, `world/ai_modules.rs`, `creature/utility_comparer.rs` |
| Lizard hunting, lounging, fleeing | `game_lizard_hunt.rs`, `world/threat_flight.rs`, `game_heavy_ai.rs` |
| Lizards fighting, signalling, leaving on a mission | `game_lizard_fight.rs`, `creature/mission_tracker.rs`, `world/missions.rs` |
| Room tile AI map | `world/ai_map.rs`, `ai_mapper.rs` |
| Shelters, sleep, hibernation | `shelter.rs`, `game_shelter.rs`, `player/sleep.rs` |
| Save-state writing (sim side) | `world/region_state.rs`, `region_save.rs`, `creature/state_save.rs`, `item.rs` (object strings), `creature/overseer/guide_state.rs`, `map_discovery.rs`; text format in `rw-save` |
| Death, game over, karma flower | `game_over.rs`, `creature/death.rs`, `player/falls.rs` |
| The player bitten and held by a lizard, struggling free | `game_player_grabbed.rs`, `player/grabbed.rs`, `game_lizard_hits.rs` (`Bite`, the melee throw's hit) |
| Random draws / sounds | `sound.rs`, `rw-random` crate; draws live beside each ported function |
| Camera visibility | `camera.rs` |
| Room triggers, guide instructions | `trigger.rs`, `game_triggers.rs`, `game_instructions.rs`, `room_script.rs` |
| Room realization / loading | `world/realizer.rs`, `world/realized.rs`, `world/mod.rs` |
| Rain cycle | `world/rain_cycle.rs` |
| Eating, picking up, throwing | `game_player.rs`, `game_items.rs`, `player/grab.rs`; creatures' meat: `game_eat_meat.rs`; carrying a heavy body: `player/carry.rs`, `game_player.rs` |
| The Hunter's spear on the back | `game_spear_on_back.rs`, `player/spear_on_back.rs`, `item/on_back_stick.rs` |
| Body physics / collision | `chunk.rs`, `object.rs`, `player/mod.rs` |
| Overseer / guide | `creature/overseer/`, `world/overseers_world_ai.rs`, `game_holo_light.rs` |
| Placed light beams, spot lights | `light_beam.rs`, `spot_light.rs`, `game_light_beams.rs`; sprites in `rw-graphics` `light_beam.rs`, `spot_light.rs`, `quad_grid.rs` |
| Light fixtures (`Redlight`, `HolyFire`, `SlimeMoldLight`), waterfalls | `light_fixture.rs`, `waterfall.rs`, `room_fixtures.rs`, `game_light_fixtures.rs`, `game_waterfalls.rs`; sprites in `rw-graphics` `light_fixture.rs`, `water_fall.rs` |
| Lights (placed, player glow, room light queries) | `light_source.rs`, `game_lights.rs`, `camera_image.rs`; sprites in `rw-graphics` `light_source.rs` |
| Bat flies | `creature/fly/`, `world/flies_world_ai.rs`, `game_flies.rs` |
| Cosmetic insects (Flies, RockFlea, ... effects, `InsectGroup`) | `insect/`, `game_insects.rs`; sprites in `rw-graphics` `insect_graphics.rs` |
| Dev spawns / teleport | `game_dev.rs`, `spawn.rs` |
| Custom decals, rainbows | `decal.rs`, `rainbow.rs`, `game_decals.rs`; sprites in `rw-graphics`, shaders in `rw-render` |
| Cosmetic slime molds, dead token stalks | `slime_mold.rs`, `token_stalk.rs`, `game_placed_cosmetics.rs`; sprites in `rw-graphics` |

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
(`notes/intro-overseer-survey.md` covers the guide and `room_script.rs`.)

Divergences: `../../docs/divergences/` (IDs are global, `grep -n '^## D'`). In `sim.md`:
D2 spawning, D3 movement, D4 falls, D5 shortcuts, D6 poles/ledges, D9 skid/roll/belly
slide, D10 corridors, D11 water, D14 worlds/rooms/realizer (creatures, lizards, flies,
overseers), D17 shelters, D19 the player held by a creature. Elsewhere: D1 `random.md`, D12 `audio.md`, D13 `menu.md`,
D16 `save.md`, D18 `app.md`.

Tests: `*_tests.rs` beside their module, `player/tests/`, and `tests/` (13 `installed_*.rs` integration tests that read the installed game, plus `data/`).
