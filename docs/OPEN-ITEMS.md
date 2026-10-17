# Open items

Approved or noted work that is not started, and things to look into. Each needs
the user's go-ahead before it starts. Bugs seen in play are in `BUGS.md`; the lizard
plan is in `../rainworld-re/surveys/lizards.md`.

## Approved, not started
- **The Hunter's `SpearOnBack`**, and the LF_H01 spear.
- **`ExplosiveSpear` behaviour.** The flag is ported; the explosion is not.

## Planned
- **Code quality pass after the lizard steps** (user, 2026-10-16): once lizard steps
  6 to 9 are merged, go over the workspace one crate at a time, a step per crate,
  finding code quality improvements (duplicated code to merge, and the like) and
  bringing them to the user before changing anything. This is in addition to the
  end-of-project review and optimisation pass in AGENTS.md.

## Waiting for the user's answer
- **Lizard step 6b** (noise tracker and hearing, `InGameNoise`, the chunks'
  loudness, community influence changes beyond kills, the pather's threat and
  obstacle costs): go-ahead. Step 6a (tracker, vision with the player's `privSneak`,
  relationships, the eyes looking) is done; its left-out items are listed in
  `docs/DIVERGENCES.md` (green lizard section). See `../rainworld-re/surveys/lizards.md`.
- **Player grabbing lizards earlier:** picking up, carrying and eating dead or stunned
  lizards needs no AI, so it could be its own small step before 6. Lizards grabbing
  the player stays in step 8.

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
