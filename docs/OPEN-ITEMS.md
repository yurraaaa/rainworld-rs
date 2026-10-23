# Open items

Approved or noted work that is not started, and things to look into. Each needs
the user's go-ahead before it starts. Bugs seen in play are in `BUGS.md`; the lizard
plan is in `../rainworld-re/surveys/lizards.md`.

## Approved, not started
- **Debug UI region switcher** (2026-10-19): the F1 debug UI's Region tab lists the
  regions on disk but its button does nothing; it needs region changes and gates,
  which are not ported.
- **`ExplosiveSpear` behaviour.** The flag is ported; the explosion is not (its
  `SpearFragment` and rag read `room.gravity`, `ExplosiveSpear.cs` 39 and 203).

- **Hunter follow-ups** (user, 2026-10-22, after the spear on the back): `HardmodeStart`
  (the Hunter's scripted LF_H01 first cycle, its spear, NSHSwarmer, `Red_stomach` pearl,
  Deer, scripted input/camera/map) as its own job later; the `sprOnBackStick` in the
  region state's `STICKS` with the save work.

- **Async room loading** (user, 2026-10-22): plan and decisions in
  `../rainworld-re/surveys/async-room-loading.md`; later. `ShortcutHelper` its own job
  after it.
- **Water with the rain (M8)**: `Room.AddWater` makes a `Water` (so its surface loop and
  `WaterLight`) in rooms with no level water (rain flood, `WaterCycleTop`/`Bottom`,
  `BrokenShelterWaterLevel`). The other water effects are done (2026-10-22).

## Planned
- **Room visual effects, lights and camera shake** (user, 2026-10-22): every non-DLC
  room visual effect, rooms' lights and `RoomCamera` screen shake, as the step before the
  rain. Surveys: `../rainworld-re/surveys/room-effects.md`, `lights-camera-shake.md`.
  Effects step 1 (level shader and palette machinery) left for later steps: the rain's
  `GlobalRain.ShaderLight` after the cycle ends (the port keeps its initial -1; goes
  with the rain), ghost
  mode's `_light`/`_cloudsSpeed`/palette grey-out (with echoes).
  Effects step 2 (full-screen effects, post-level grab, wet terrain, the Ant's
  `SpecificDepth`) and the `DeathFallGraphic` sprite done; left for later steps: `VoidMelt`'s `LevelMelt2` and
  the echo branch of `ApplyPalette` (no effect selected meanwhile), full-screen effects
  and the death fall in MCP screenshots of other rooms, the death fall's counter from
  `Room.Loaded` (it restarts when the camera enters; D7), and holograms drawn under a re-raised `Bloom`
  effect (D7).
  Effects step 4 (`Lightning`, `BkgOnlyLightning`, `ExtraLoudThunder`; D25, notes
  `lightning.md`) done: flashes, `num4`/`_light`, `LightBloom` alpha, sky and fog texels,
  thunder sounds and shake, background noise in hearing.
  Camera shake core (S1) done: `rw_sim::CameraShake`, notes `camera-shake.md`. Left:
  the rain-derived shake (S2, with the rain: `RainCycle.ScreenShake`/`MicroScreenShake`,
  `GlobalRain` modes, flood `waterFlux.roomShake`, the `Screen_Shake_LOOP` rumble, gamepad
  rumble from `controllerShake`), callers that come with their objects (explosive spears' and scavenger
  bombs' explosions, cicadas, leviathans, region gates, Moon's wake-up), and
  `PausedDrawUpdate`'s micro shake (skipped for now, user 2026-10-22; paused frames repeat
  the last offset, D1), and the Remix clamps (with Remix).
  User 2026-10-22 on the wave 1 omissions: wet terrain and the Ant's `SpecificDepth`
  shader (D23) go with the full-screen effects step; the echo branches (ghost mode, the
  insects' `CreaturesSleepInRoom` early-out) with echoes; PinkSky and DayNight with the
  DLCs; arena non-story light and `startUpTicks` 600 with arena sessions; insect
  explosions with explosions; insect terrain curves when terrain curves are ported; the
  `SunBlocker`'s (empty) update-list entry skipped.
  Placed lights (L1, D24, notes `lights.md`) done: placed `LightSource`s, `Alpha` with
  `fadeWithSun`, colour from the level image and effect colours, the glowing player's
  light, `LightSourceExposure`/`LightSourceColor`/`DarknessOfPoint`, `Needed` reading
  `Alpha`. `LightBeam`/`SpotLight` (L3) done (D24). Fixtures without electricity (L2a: `Redlight`,
  `HolyFire`, `SlimeMoldLight`) and the rooms' waterfalls done (D30; the emergency red
  lights follow `ElectricPower` since zero gravity, D34). Left: the SS
  electricity fixtures (L2b: `ZapCoilLight`, `DeepProcessing`, the superstructure), owners' lights with
  their owners (L4); blinking and night lights with the DLCs.
  Step 3 (cosmetic insects, `rw_sim::insect`, `../rainworld-re/notes/cosmetic-insects.md`)
  done; its omissions are among the decisions above. Draw order for later steps:
  `Room.NowViewed` makes shared-stream draws for GreenSparks, ZeroGSpecks (`BlinkSpeck`),
  CorruptionSpores and FairyParticles in its effect loop before the insect roll
  (`Room.cs` ~2632-2700, then `insectCoordinator.NowViewed`); the steps porting those
  effects place their draws before `apply_insect_view_change`'s roll. No base room has
  both today. Step 6 (SkyDandelions, GreenSparks, ZeroGSpecks, CorruptionSpores, D29,
  notes `particles.md`) done that way (`particles_now_viewed`); zero gravity is ported (D34), so the specks
  drift in `ZeroG` rooms.
  Ordering item (review of step 6): (F4) `Room.Loaded`
  adds effect objects (~863) and placed objects (~1055) before `AddRoomSpecificScript`
  (~1873); the port adds them in `add_trigger_checkers`, after the scripts. No base room
  is affected today. The `NowViewed` order (F1) is a divergence (D29), by the user's
  decision.
- **Code quality pass** (user, 2026-10-16; moved to M14 cleanup by the user 2026-10-22,
  not after the lizard steps): go over the workspace one crate at a time, a step per crate,
  finding code quality improvements (duplicated code to merge, and the like) and
  bringing them to the user before changing anything. This is in addition to the
  end-of-project review and optimisation pass in AGENTS.md.
- **End-of-project cleanup items** (user, 2026-10-19), for the review pass in AGENTS.md:
  - `rw-hud` holds the sleep, death, pause and slugcat select screens beside the
    in-game HUD; decide whether they move (e.g. into `rw-menu` or their own crate).
  - `rw-graphics`' `cosmetic_graphics.rs` is a catch-all (sparks, explosion spikes,
    bubbles); split or rename it.
  - `rw-sim`'s file split: `game.rs` (2051 lines, sessions, lobby, tick order),
    lizard and fly logic outside `creature/` (`world/lizards.rs`, `game_lizard_*.rs`,
    `game_flies.rs`), weapons in `world/weapons.rs`, `item/weapon.rs` and
    `game_weapons.rs`, save-state writing across about six files, `trigger.rs` vs
    `game_triggers.rs`, `noise.rs` vs `world/hearing.rs`.
  - `rw-menu`'s `font_test.rs` (port-only screen) stays for now.
- **Optimisation pass: rooms with many sprites** (user, 2026-10-23): SS_E05 now holds
  240 fps, but zero-g speck/spore rooms are worth profiling in M14. Measured 2026-10-23
  (`ss-e05-lag` investigation and the sprite-order review): ~3700 sprites after re-entries
  cost ~1.4 ms/tick (0.6 ms speck updates, as the game does; 0.36 ms `SpriteOrder::update`);
  the pile-up itself is game-exact (D29 F2). Leads: per-frame sprite-list building for
  thousands of specks, `SpriteOrder::position`/`re_add`/`apply` and `run_light_source`'s
  per-light `find` (linear, cheap today), `Scene::draw` calling `sync_sprite_order` twice,
  the GPU cost of the `LightSource` shader when SS_E05's power fails (10 to 30 lit lights;
  not measured), and headless screenshot readback/PNG encoding (~80 ms).

## Waiting for the user's answer
- **Lizard steps after 7c** (7a done: trackers, utility comparer, den finder,
  `DetermineBehavior`, the calm bodies; 7b done: Hunt and Flee, `AggressiveBehavior`,
  the lounge, `FleeTo`, `RequestHeavyAi`; step 8 done: the player grabbed, bitten,
  struggling free, `GameOver(grasp)`, jaw-held sprites, D19; 7c done: Fighting (and
  FightingStance's lean), rivalry, `LizardCommunication`, `MissionTracker` and
  ActingOutMission; `rainworld-re/notes/lizard-ai.md`). Go-ahead for each. Order
  agreed (user, 2026-10-21): 7b (done), step 8 (done; player side: grabbed, bitten,
  `GameOver(grasp)`), 7c (done), the player picking up, carrying and eating dead
  lizards (done; living and stunned ones cannot be grabbed in the base game), then
  step 9 (done: lineages, respawn lists, shelters, census). Details in
  `../rainworld-re/surveys/lizards.md` (user decisions).

## To look into
- **Full-screen effects leftovers** (review, small): `DeathFall::camera_viewing_point`
  belongs on the room (`Room.CameraViewingPoint`); stale "not ported" rows for LightBurn,
  SkyAndLightBloom, SkyBloom and wet terrain in `../rainworld-re/surveys/room-effects/`
  (README, `screen-effects.md` §1.5 and the `_WetTerrain`/`_waterLevel` rows) and
  `notes/room-rendering.md` §9.
- **Camera room switches mid-tick** (2026-10-22, camera-shake review; inferred): the
  port's `camera_moves_to` sets the camera's room at once, where the game's
  `cameras[0].room` stays the old room until `ApplyPositionChange` at the next
  `RoomCamera.Update`. For the rest of that tick `ScreenMovement`s and the drips'
  viewed check go to the new room, as does `Room.Update`'s ceiling drip (`BeingViewed`).
- **Creatures at open room edges** (user, 2026-10-22, for when more creatures get
  bodies): room borders are not solid (`Room.GetTile` copies the edge tile outside the
  room, so open air edges extend forever; the room file's `Border:` field is unread).
  Check how the game handles a creature flying or falling past an open edge (bat flies
  now, flying creatures later): sky and side exits (`SkyExit`/`SideExit`, border exit
  nodes), `RoomBorderPushBack` (ported, D33: base `Room.Loaded`, no `ModManager` guard), removal when too far out,
  and compare with the port.
- **Loop emitters when a room abstractizes** (2026-10-17): a swallowed rock's loop now
  lives one more update, as in the game, but an item whose room abstractizes (e.g. one
  held there) still drops its loop emitters at once. Check against the game which
  objects and paths this covers and whether they should live one more update too.
- **Headless speed.** MCP `step` runs about 20 ticks/s with lizards around (seen
  2026-10-16), below the game's 40, while the window keeps full fps. A profile showed
  lizard pathfinding, limbs and graphics, the HUD map and text prompt spread across the
  time. Survey why before the end-of-project optimisation pass, sooner if it slows
  agents.
- **Cicadas at their den.** A cicada at its den node holding a fly never enters the
  den. Believed to be the game's behaviour; confirm once cicadas have bodies.
- **`ShelterFinder` panics** (`ShelterFinder::update`, test `overseers_world_ai.rs`
  ~288) in test worlds without shelters. Real regions always have shelters.
- **Held creature's double update** (user, 2026-10-22: later): a creature placed in a
  room while held (`AddObject` twice) has two entries in the game's `updateList` and
  `drawableObjects` and updates twice a tick; the port keeps one entry (D14 in
  `docs/divergences/sim.md`).
- **First room's sprite order at game start** (user, 2026-10-22: survey later): the
  game realizes the first room in the `RainWorldGame` constructor, but the camera's first
  `ChangeRoom`/`NowViewed` waits for its level texture (`RoomCamera.Update` →
  `ApplyPositionChange`). Whether the first room's creatures join the sprite order before
  or after depends on frame timing (`../rainworld-re/notes/creature-model/model.md` §4).
- **Edible slime mold item** (user, 2026-10-22): `Room.Loaded`'s second placed-object
  loop makes an `AbstractConsumable(SlimeMold)` for each plain `SlimeMold` (`GetNewID`,
  `ItemConsumed` check). Only the cosmetic patch and its `SeededRandom` gate are ported.
- **Collected-token stalks** (user, 2026-10-22): `GoldToken`/`BlueToken` placed objects
  make a dead `TokenStalk` in a story session when
  `miscProgressionData.GetTokenCollected(tokenString, isBlue)`; otherwise a live
  `CollectToken`. Needs the collected-token data, which an imported save already carries
  (`MISCPROG` `LEVELTOKENS`/`SANDBOXTOKENS`, kept untouched by `rw-save`).
- **More Slugcats token stalk** (user, 2026-10-22): `TokenStalk`'s MSC branches
  (satellite head, `SataFlasher` sprite, `forceSatellite`), with the DLC.
- **M7 effects: behaviour waiting on its dependency** (user, 2026-10-23: port each when
  its dependency is ported):
  - Lightning: `DaddyLongLegs` closing its eyes above `BackgroundNoise` 0.35, with that
    creature. (The `game.pauseUpdate` branch of `backgroundNoise` is MSC-only, D25.)
  - Rainbow: the camera's `ghostMode` hiding it, with echoes; the `IsArenaSession` branch
    (always made, fade 1, never destroyed) and arena's 600-tick start-up, with arena
    sessions (D26).
  - SkyDandelions: `globalRain.Intensity` in `SpawnChance`, with the rain; dandelion
    stalks reaching the chunks of every physical object, as those objects are ported.
  - Background scenes' `DayNight` blocks (sky alphas, `atmosphereColor`,
    `_MultiplyColor`, the spire lights; needs the rain cycle's `dayNightCounter`; no
    base room has `DayNight`): with the DLCs that use `DayNight` (D32).
  - Zero gravity (D34): the shortcut pushers in `ShortcutHelper`, with it.
  - Zero gravity: the player's cloth (`PlayerGraphics` 1168), with the cloth.
  - Zero gravity: the vine grab and vine pull, with vines.
  - Zero gravity: coral circuit swimming (`circuitSwimResistance`, `curcuitJumpMeter`,
    `swimBits`), with the coral circuit object.
  - Zero gravity: Five Pebbles' and Moon's oracle scenes switching `AntiGravity` off, with
    the oracles.
  - `Room.ElectricPower` / gravity readers: `ZapCoil`, `GravityDisruptor`, `SpinningFan`,
    `SuperStructureFuses`, `MeltLights`, with the SS objects.
  - `Room.ElectricPower` / gravity readers: `SSLightRod`, `DeepProcessing`,
    `DeepProcessingLight`, with the SS objects (`ProjectedScanLines` and
    `SuperStructureProjector` read it since step 10a, D35).
  - `ProjectedCircle` (D35): with its owners `CoralNeuron`, `CoralStem`, `SSOracleSwarmer`
    (and `SpearMasterPearl`, `Inspector` in the DLC).
  - Zero gravity: `ExplosiveSpear` fragments, with explosive spears.
  - Zero gravity: `OverseerCarcass`, with it.
  - Zero gravity: `SegmentRope` and `Dangler`, with them.
  - `Room.ElectricPower` / gravity readers in `HazerGraphics` and `ScavengerGraphics`, with
    Hazers and scavengers.
  - The analogue stick in zero gravity (`ZeroGGamePadIntVec`; user, 2026-10-23): waits until
    the port has analogue gamepad input.
  - ElectricDeath: what it does once the rain ends the cycle (the port holds it at full
    intensity from cycle end + 1200), with the rain (D33). Its DLC/Remix branches wait for
    DLC.
  - `RoomRain`'s shelter sprite scale `ScreenSize.x / shelterTex.width` (the chosen
    resolution, `World::screen_size`), with the rain.
- **Room files `Room::load` rejects** (user, 2026-10-23: not real rooms, but look into
  why): `su_a08`, `su_a09`, `su_a15_tiled`, `su_b01_tiled` in the installed game's
  `su-rooms` fail the port's room parser (seen by the waterfall corpus test). Find what
  in their files the parser rejects and whether the game's `Room.LoadFromDataString`
  would read them.
- **Void effects** (user, 2026-10-23: wait until the ascension ending can be reached in
  the port): `VoidMelt` (with the level shader's `LevelMelt2`; 20 SB rooms), `VoidSpawn`
  (17 SB rooms) and `VoidSea` (sb_l01, the ending). Survey:
  `../rainworld-re/surveys/room-effects/` (step 11, `background-scenes.md` §3).
