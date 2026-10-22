# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Green lizards stuck on a horizontal pole in UW_J01** (2026-10-22): three green lizards
  spawned higher up ended side by side on the horizontal pole at the bottom of the room,
  though green lizards cannot climb poles. The middle one keeps falling and is flung
  around once it goes off screen, presumably into the death pit.
  The user suspects they hold each other in their jaws, though nothing shows it visually.
  Investigated (`../rainworld-re/notes/lizard-ai.md`, "Green lizards hanging on a
  horizontal pole"): the fling is fixed (`Lizard.Stun` now drops the grip point and
  movement animation, so a lizard bitten off the pole is no longer yanked back when the
  stun ends). Landing and hanging on the pole and the neighbours biting each other follow
  the game's code (`LizardPather.FollowPath`, `DropToClimb`, step 7c fights); awaiting the
  user's check that the real game does the same.
