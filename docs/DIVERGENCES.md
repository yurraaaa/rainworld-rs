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

## D2: Spawning in rooms without shortcuts and in shelters

**What.**
- The game's first spawn phase throws (`shortcuts[0]` on an empty array) in a room with
  no shortcuts. The port skips that phase and goes on to the fallbacks.
- The shelter branches of `DetermineSafeSpawnTile` are not ported, because room files do
  not say whether a room is a shelter.
- The game draws the node from the abstract room's leading Exit and Den nodes; the port
  counts the room-exit and den tiles of the shortcut map instead (INFERRED to be the
  same count; the abstract node list comes from world data the port does not load).

The shortcut map itself (`rw_assets::ShortcutMap`) is the game's `ShortcutMapper`
(node order, shortcut order, tube walk, types), no longer inferred.

**Why.** The game's behaviour is a crash or depends on world data the room file lacks.

**Affects.** Spawn tiles in rooms without shortcuts and in shelters.

## D3: Player movement (stage one)

`rw_sim::Player` ports base slugcat movement (`rainworld-re/notes/slugcat-movement.md`,
`horizontal-beams.md`, `slope-collision.md` section 5) for the Survivor with every
`SlugcatStats` factor 1, no adrenaline, never exhausted. Stun and death are D4.

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
- **Input.** `Input` carries x, y and jump only; `downDiagonal` is derived digitally
  (down plus left/right gives that side, the Remix rule). No analogue stick, map
  button stand-still, throw / pick-up / special.

**Why.** Each is either outside the stage-one scope or depends on randomness (D1).

**Affects.** Movement in corridors, on vertical poles, belly slides, skids, the rare
stacked-head case and initial facing.

## D4: Falls, stun and death

Body chunks report flat-tile hits (`TerrainImpact`), and `Player` ports the Survivor's
fall damage, stun and death (`rainworld-re/notes/falls.md`) with Remix off: death on a
floor first contact above 60, stun `LerpMap(speed, 35, 60, 40, 140, 2.5)` above 35,
neutral input and no `MovementUpdate` while stunned or dead, surface friction 0.3 while
not conscious, and death plus destruction below the room bottom (-500, -250 in
`WallClimb`) unless the room's water catches the body. As in the game, a destroyed body
is gone (`Player::is_destroyed`; the app drops it and draws nothing until R).

**What differs:**
- **Death freezes the body** (CLAUDE.md decision). The game keeps simulating a dead
  player as plain physics and shows the death screen (story) or ends the round (arena).
  `rw_sim::Player` keeps simulating it; the app (`Game::tick`) stops ticking a dead
  player and draws it where it stopped until R respawns a new one. R also respawns
  after the body fell out of the room (the game has no respawn).
- **Landing roll never happens.** The first branch of `Player.TerrainImpact` (a
  down-diagonal landing becomes a `Roll` and skips fall damage) is a stub that never
  rolls (`Player::lands_in_roll`), so a held down-diagonal does not save the player.
- **Impacts are applied after physics.** The game calls `TerrainImpact` in the middle of
  each chunk's collision; the port collects the calls in the same order and handles them
  right after `PhysicalObject::update`. No ported branch touches chunk state, so this is
  exact for now; the landing roll changes velocities and will need the mid-sweep call.
- **`Player.Stun` side effects.** The game un-stands the player (and drops grasps) when
  the stun exceeds `Random.Range(5, 12)`; the port uses 8. Fall stuns are at least 40, so
  falls are unaffected. Malnourished stun scaling, grasps, spear and slugpup drops, sounds
  and the face blink are not ported.
- **Remix (MMF) branches omitted:** falling into a room-exit or normal shortcut entrance
  at speed > 5 (enters the shortcut instead of taking damage) and the wall pounce.
- **More Slugcats omitted:** Gourmand's thresholds (death above 80, stun above 40
  with the 40..140 ticks mapped over speeds 40..80), tongue attachment and being carried by another player blocking fall damage,
  Sofanthiel's sound. Base-game branches with no ported counterpart: the Pomegranate
  grasp and the deer-antlers ride cancelling fall damage.
- **Water.** Only the out-of-room exemption reads the water level (no water physics);
  inverted water (DLC) is not modelled.

**Why.** The freeze and the respawn are design decisions for the single-room
player-controller phase;
the roll belongs to the advanced-moves stage; the rest is out of the base-game scope or
randomness (D1).

**Affects.** What happens after death, hard landings with a held down-diagonal, and
Remix / More Slugcats play.

## D5: Shortcuts

`rw_assets::ShortcutMap` is the game's `ShortcutMapper`; `Player` ports entering by
pressing into a hole, the pull-in, the vessel (`rw_sim::ShortcutHandler`, one tile per
three ticks) and the spit-out at the far entrance (`rainworld-re/notes/shortcuts.md`),
base game with Remix off.

**What differs:**
- **Room-exit entrances can't be entered** (CLAUDE.md decision; there are no other
  rooms). The game lets the player enter a RoomExit shortcut and takes it to the
  connected room (or, in an arena, ends the player's round). `DirectIntoHoles` still
  steers toward room-exit holes, as in the game.
- **One vessel, owned by the player.** The game's `ShortcutHandler` keeps a list of
  vessels for every creature, with Jolly co-op waits and the between-rooms lobby; the
  port has one player whose vessel lives on `Player`, `wait` is always 0, and the
  handler is only the three-tick divider.
- **Not ported:** being pulled in by a carried object (no grasps), `cantBeGrabbedCounter`,
  shortcut sounds and the entrance blink, MSC tongue release, the arena hooks of
  `SpitOutOfShortCut`, and Remix falling into a shortcut (D4).

**Why.** The room-exit block is a design decision for the single-room phase; the rest
is outside the single-player, base-game scope.

**Affects.** Room-exit shortcuts, multiple creatures in tubes, co-op.

## Not ported yet (not divergences, just absent)

Player (stage one leaves out): landing roll and MMF wall pounce in `TerrainImpact`,
`LedgeGrab`, `CorridorClimb` / `CorridorTurn` and corridor jumps, roll, belly slide,
flip, rocket jump, water (`Swimming`, `DeepSwim`, `SurfaceSwim`, submersion lifts),
zero gravity, grabbing / throwing / eating (`GrabUpdate`), `slowMovementStun` sources, adrenaline, the WALLSTUCK safety
(needs `VisualContact`), `dropGrabTile`, carried-object speed factors, vines, tube worm,
Jolly co-op, MSC / Watcher class mechanics. Ported because ground classification
produces them anyway: wall slide and wall jump (`WallClimb`, `canWallJump`, `jumpStun`;
only the wall-jump impulse has a test), `LedgeCrawl`, `CrawlTurn`, the crouch super-jump
charge and dive force (untested).

Physics: Water (buoyancy, water friction), burrowing, terrain curves (`TerrainCurve`), `setPos`
(cross-object position writes), the curved-terrain `TerrainImpact` call, grabs (`grabbedBy` skips the room-range clamp, and grabs force `goThroughFloors`), and
`DefaultTile`.
