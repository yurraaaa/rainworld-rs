# Open items

Approved or noted work that is not started, and things to look into. Each needs
the user's go-ahead before it starts. Bugs seen in play are in `BUGS.md`; the lizard
plan is in `../rainworld-re/surveys/lizards.md`.

## Approved, not started
- **Debug UI region switcher** (2026-10-19): the F1 debug UI's Region tab lists the
  regions on disk but its button does nothing; it needs region changes and gates,
  which are not ported.
- **The Hunter's `SpearOnBack`**, and the LF_H01 spear.
- **`ExplosiveSpear` behaviour.** The flag is ported; the explosion is not.

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

## Waiting for the user's answer
- **Lizard step 7 go-ahead.** Split and order agreed (user, 2026-10-21): 7a (trackers,
  utility comparer, den finder, the calm behaviours, `runSpeed`: lizards walk), 7b (Hunt,
  Flee, aggression, lounge, `RequestHeavyAi`), step 8 (player side: grabbed, bitten,
  `GameOver(grasp)`), 7c (lizard fights, communication, missions), then the player
  picking up, carrying and eating dead or stunned lizards as its own step, then step 9.
  Details in `../rainworld-re/surveys/lizards.md` (user decisions).

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
