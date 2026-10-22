# Open items

Approved or noted work that is not started, and things to look into. Each needs
the user's go-ahead before it starts. Bugs seen in play are in `BUGS.md`; the lizard
plan is in `../rainworld-re/surveys/lizards.md`.

## Approved, not started
- **Debug UI region switcher** (2026-10-19): the F1 debug UI's Region tab lists the
  regions on disk but its button does nothing; it needs region changes and gates,
  which are not ported.
- **The LF_H01 spear** (the Hunter's `SpearOnBack` itself is ported, D3): the start
  spear is made and put on the back by `HardmodeStart`, the Hunter's scripted first
  cycle in `LF_H01` (swarmer in hand, pearl in the stomach, a deer, scripted input,
  camera and map; D14), which is not ported. Port it whole, or only its spear?
- **`ExplosiveSpear` behaviour.** The flag is ported; the explosion is not.

- **Draw order by container add order** (user, 2026-10-22): objects that never went
  through a jaws release keep the port's order by kind, standing in for the game's order
  of adding sprites to the camera's containers (`drawableObjects`,
  `RoomCamera.MoveObjectToContainer`); a newly realized room's camera order too (D19).
  Its own small job; touches the draw path for every object.

- **Hunter follow-ups** (user, 2026-10-22, after the spear on the back): `HardmodeStart`
  (the Hunter's scripted LF_H01 first cycle, its spear, NSHSwarmer, `Red_stomach` pearl,
  Deer, scripted input/camera/map) as its own job later; `SocialEventRecognizer.
  CreaturePutItemOnGround` in `ReleaseObject` and the back spear's lay-down drop (small
  job); the `sprOnBackStick` in the region state's `STICKS` with the save work; the
  back spear's zero-gravity placement with zero gravity; `EatMeatUpdate`'s back-spear lock
  with eating meat (the grab-lizards step).

## Planned
- **Code quality pass after the lizard steps** (user, 2026-10-16): once lizard steps
  6 to 9 are merged, go over the workspace one crate at a time, a step per crate,
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

- **Map: items and creatures stored in shelters** (noted 2026-10-22, not started): the
  base game's map shows icons of what lies in each shelter
  (`HUD/Map.cs` `ShelterMarker.ItemInShelterMarker`, `RevealSymbols`,
  `GetItemInShelter`/`GetItemInShelterFromWorld`, `UpdateData(putItemsInShelters)`; no
  Remix gate). The port's map draws the shelter marker but no item icons. Live creature
  icons on the map are Remix's `cfgCreatureSense` and come with Remix.

## Waiting for the user's answer
- **Lizard steps after 7c** (7a done: trackers, utility comparer, den finder,
  `DetermineBehavior`, the calm bodies; 7b done: Hunt and Flee, `AggressiveBehavior`,
  the lounge, `FleeTo`, `RequestHeavyAi`; step 8 done: the player grabbed, bitten,
  struggling free, `GameOver(grasp)`, jaw-held sprites, D19; 7c done: Fighting (and
  FightingStance's lean), rivalry, `LizardCommunication`, `MissionTracker` and
  ActingOutMission; `rainworld-re/notes/lizard-ai.md`). Go-ahead for each. Order
  agreed (user, 2026-10-21): 7b (done), step 8 (done; player side: grabbed, bitten,
  `GameOver(grasp)`), 7c (done), then the player picking up, carrying and eating dead
  or stunned lizards as its own step, then step 9. Details in
  `../rainworld-re/surveys/lizards.md` (user decisions).

## To look into
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
