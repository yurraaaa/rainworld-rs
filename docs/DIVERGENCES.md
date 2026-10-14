# Divergences from the original game

Each entry: what differs, why, and what it affects. Behaviour sources are the notes in
`../rainworld-re/notes/`.

## D1: Random number sequence (spawning)

**What.** `rw_sim::determine_safe_spawn_tile` and `rw_sim::place_in_room` draw from a
caller-supplied `rand::Rng` (`rand` 0.10; tests use `rand::rngs::StdRng::seed_from_u64`),
not Unity's `UnityEngine.Random`. For the same game state the port therefore picks a
different spawn tile and different chunk offsets than the game would.
`Random.value` also includes 1.0, while `rand`'s `random::<f32>()` is in `[0, 1)`, so
the port never produces the game's rare `value == 1` edge cases, such as a node index
equal to the exit/den count or an offset of exactly 10 tiles.

**Why.** Unity's generator state comes from the engine (seeded at start-up and shared by
every system that draws from it: sounds, particles, AI), so a draw-for-draw match would
need the whole game's call order, not just these functions. The behaviour that matters
is kept: which tiles can be chosen, in what fallback order, and the offset
distributions. An injectable, seedable generator makes tests deterministic.

**Affects.** Exact spawn tiles and the initial 1 px chunk jitter only. Physics after
placement is deterministic and does not use randomness.

## D2: Spawning in rooms without shortcuts, shelters, and the shortcut map

**What.**
- The game's first spawn phase throws (`shortcuts[0]` on an empty array) in a room with
  no shortcuts. The port skips that phase and goes on to the fallbacks.
- The shelter branches of `DetermineSafeSpawnTile` are not ported, because room files do
  not say whether a room is a shelter.
- Which shortcut leads to which node comes from the game's `ShortcutMapper`, which has
  not been decompiled. The port derives it from tile flags: room exits, then dens,
  then region transports, each scanned top row first and left to right (as in
  `room-format.md` section 5). It traces each shortcut from its entrance along flagged
  tiles, and orders shortcuts by entrance in the same scan order. The tracing and the
  shortcut order are INFERRED.

**Why.** The game's behaviour is a crash or depends on world data the room file lacks.
The shortcut mapping is a stand-in until `ShortcutMapper` is read.

**Affects.** Spawn tiles in rooms without shortcuts, in shelters, and in rooms where
several shortcuts lead to exit/den nodes in a different order than the game's.

## D3: Player movement (stage one)

`rw_sim::Player` ports base slugcat movement (`rainworld-re/notes/slugcat-movement.md`,
`horizontal-beams.md`, `slope-collision.md` section 5) for the Survivor with every
`SlugcatStats` factor 1, no adrenaline, never exhausted, never stunned or dead.

**What differs from the game where the game's behaviour is kept out of scope:**
- **No corridor detection.** The game counts chunks wedged between opposite solid tiles
  (`goIntoCorridorClimb`) and, inside one-tile corridors, skips ground classification and
  switches to `CorridorClimb`. The port always classifies (Default / Stand / Crawl), so
  in a one-tile tunnel the player behaves as on open ground.
- **Vertical poles.** `GrabVerticalPole`, `ClimbOnBeam`, `HangUnderVerticalBeam`,
  `BeamTip`, `GetUpToBeamTip` are not ported. Holding up in a tile with both beam kinds
  hangs from the horizontal beam (the game climbs the pole), and an up press while
  hanging in a tile with a vertical pole does nothing (the game switches to the pole).
- **Belly slide.** A jump while lowering to all fours with down-diagonal toward the
  facing starts `BellySlide` in the game; the port does an ordinary crouch jump.
- **Skid and flip jump.** The Stand-mode turn-around skid (`slideCounter`,
  `initSlideCounter`) is not ported, so the skid flip jump never happens.
- **Stacked-body nudge.** When the head is exactly below the hips with no horizontal
  speed, the game gives the head a random x speed in [-1, 1] (hips the opposite); the
  port uses `0.5 * flipDirection` (deterministic, the mean magnitude).
- **Initial facing.** The game picks `flipDirection` = +/-1 at random in the
  constructor; `Player::new` faces right.
- **`DirectIntoHoles` and shortcut entrances.** The game steers into a hole whose far
  side is a shortcut entrance only for Normal or RoomExit shortcuts. The port classifies
  the entrance with the inferred tile-flag tracing of D2: a tube ending in a room exit,
  or in another entrance or a dead end (indistinguishable from flags), counts as
  drawing the player in; dens and transports do not.
- **Input.** `Input` carries x, y and jump only; `downDiagonal` is derived digitally
  (down plus left/right gives that side, the Remix rule). No analogue stick, map
  button stand-still, throw / pick-up / special.

**Why.** Each is either outside the stage-one scope or depends on randomness (D1).

**Affects.** Movement in corridors, on vertical poles, belly slides, skids, the rare
stacked-head case, initial facing, and steering into some shortcut holes.

## Not ported yet (not divergences, just absent)

Player (stage one leaves out): `TerrainImpact` (fall damage death above speed 60, stun
above 35, landing roll, MMF wall pounce), falling out of the room bottom (death),
`LedgeGrab`, `CorridorClimb` / `CorridorTurn` and corridor jumps, roll, belly slide,
flip, rocket jump, water (`Swimming`, `DeepSwim`, `SurfaceSwim`, submersion lifts),
zero gravity, grabbing / throwing / eating (`GrabUpdate`), shortcuts (entering,
`enteringShortCut`), `slowMovementStun` sources, adrenaline, the WALLSTUCK safety
(needs `VisualContact`), `dropGrabTile`, carried-object speed factors, vines, tube worm,
Jolly co-op, MSC / Watcher class mechanics. Ported because ground classification
produces them anyway: wall slide and wall jump (`WallClimb`, `canWallJump`, `jumpStun`;
only the wall-jump impulse has a test), `LedgeCrawl`, `CrawlTurn`, the crouch super-jump
charge and dive force (untested).

Physics: Water (buoyancy, water friction), burrowing, terrain curves (`TerrainCurve`), `setPos`
(cross-object position writes), the `TerrainImpact` callback (fall damage and stun),
grabs (`grabbedBy` skips the room-range clamp, and grabs force `goThroughFloors`), and
`DefaultTile`.
