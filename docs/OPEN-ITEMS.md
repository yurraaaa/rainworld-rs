# Open items

Approved or noted work that is not started, and things to look into. Each needs
the user's go-ahead before it starts. Bugs seen in play are in `BUGS.md`; the lizard
plan is in `../rainworld-re/surveys/lizards.md`.

## Approved, not started
- **The Hunter's `SpearOnBack`**, and the LF_H01 spear.
- **`ExplosiveSpear` behaviour.** The flag is ported; the explosion is not.

## To look into
- **Headless speed.** MCP `step` runs about 20 ticks/s with lizards around (seen
  2026-10-16), below the game's 40, while the window keeps full fps. A profile showed
  lizard pathfinding, limbs and graphics, the HUD map and text prompt spread across the
  time. Survey why before the end-of-project optimisation pass, sooner if it slows
  agents.
- **Cicadas at their den.** A cicada at its den node holding a fly never enters the
  den. Believed to be the game's behaviour; confirm once cicadas have bodies.
- **`ShelterFinder` panics** (`ShelterFinder::update`, test `overseers_world_ai.rs`
  ~288) in test worlds without shelters. Real regions always have shelters.
- **Sound timing leftovers** (asked 2026-10-16, no answer yet): swallowing a rock stops
  its loop at once, the player body's sounds stop a tick early when entering a pipe,
  and released emitters aren't reported over MCP. The game lets each live one more
  update.
