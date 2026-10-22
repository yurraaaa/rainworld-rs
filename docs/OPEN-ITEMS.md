# Open items

Approved or noted work that is not started, and things to look into. Each needs
the user's go-ahead before it starts. Bugs seen in play are in `BUGS.md`; the lizard
plan is in `../rainworld-re/surveys/lizards.md`.

## Approved, not started
- **Debug UI region switcher** (2026-10-19): the F1 debug UI's Region tab lists the
  regions on disk but its button does nothing; it needs region changes and gates,
  which are not ported.
- **`ExplosiveSpear` behaviour.** The flag is ported; the explosion is not.

- **Hunter follow-ups** (user, 2026-10-22, after the spear on the back): `HardmodeStart`
  (the Hunter's scripted LF_H01 first cycle, its spear, NSHSwarmer, `Red_stomach` pearl,
  Deer, scripted input/camera/map) as its own job later; `SocialEventRecognizer.
  CreaturePutItemOnGround` in `ReleaseObject`, the back spear's lay-down drop and a
  dropped or tossed creature (small job; user 2026-10-22); the `sprOnBackStick` in the
  region state's `STICKS` with the save work; the back spear's zero-gravity placement
  with zero gravity.

- **`Jump`'s slope push** (user, 2026-10-22): the `terrainCurveNormal` branch
  (Player.cs:13247-13251) was never ported in `crates/sim/src/player/jump.rs`. Small job.

- **Graphics module order and lifetime** (user, 2026-10-22, found in the draw-order
  review; inferred from code): `camera_changed_room` (`crates/sim/src/game_flies.rs`)
  builds overseer, fly and lizard modules in one list by seq, where `Room.NowViewed`
  builds layer 0 (flies, overseers) before layer 1 (lizards); both draw from the shared
  stream. `LizardGraphicsLink.drawn_in` is never pruned, so a lizard that left a room
  loses its module at that room's `NoLongerViewed` and is rebuilt (with draws); the game
  removes it from the old room's `drawableObjects` in `CleanOutObjectNotInThisRoom` and
  keeps it. Small job.

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
  with the rain), `BrokenZeroG`'s `DarkPalette` (`1 - CurrentLightsOn`, with zero
  gravity), lightning's `num4` and palette texel overrides (with Lightning), ghost
  mode's `_light`/`_cloudsSpeed`/palette grey-out (with echoes), the sky scenes behind
  rimFix rooms.
  Effects step 2 (full-screen effects, post-level grab, wet terrain, the Ant's
  `SpecificDepth`) and the `DeathFallGraphic` sprite done; left for later steps: `Lightning`'s flashes, `num4` and palette
  overrides (the alpha's seam is `Visuals::effect_alpha`), `VoidMelt`'s `LevelMelt2` and
  the echo branch of `ApplyPalette` (no effect selected meanwhile), full-screen effects
  and the death fall in MCP screenshots of other rooms, the death fall's counter from
  `Room.Loaded` (it restarts when the camera enters; D7), and holograms drawn under a re-raised `Bloom`
  effect (D7).
  Camera shake core (S1) done: `rw_sim::CameraShake`, notes `camera-shake.md`. Left:
  the rain-derived shake (S2, with the rain: `RainCycle.ScreenShake`/`MicroScreenShake`,
  `GlobalRain` modes, flood `waterFlux.roomShake`, the `Screen_Shake_LOOP` rumble, gamepad
  rumble from `controllerShake`), the `Lightning` and `AntiGravity` callers (S3, with
  their effects), callers that come with their objects (explosive spears' and scavenger
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
  `Alpha`. Left: fixtures (L2a, L2b), `LightBeam`/`SpotLight` (L3), owners' lights with
  their owners (L4); blinking and night lights with the DLCs.
  Step 3 (cosmetic insects, `rw_sim::insect`, `../rainworld-re/notes/cosmetic-insects.md`)
  done; its omissions are among the decisions above. Draw order for later steps:
  `Room.NowViewed` makes shared-stream draws for GreenSparks, ZeroGSpecks (`BlinkSpeck`),
  CorruptionSpores and FairyParticles in its effect loop before the insect roll
  (`Room.cs` ~2632-2700, then `insectCoordinator.NowViewed`); the steps porting those
  effects place their draws before `apply_insect_view_change`'s roll. No base room has
  both today.
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
  nodes), `RoomBorderPushBack` (unported; base game or not?), removal when too far out,
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
