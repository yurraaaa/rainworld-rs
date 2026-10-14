# Divergences from the original game

Each entry: what differs, why, and what it affects. Behaviour sources are the notes in
`../rainworld-re/notes/`.

## D1: Random number sequence

**What.** All randomness draws from `rw_random::UnityRandom`, a port of Unity's
`UnityEngine.Random` (xorshift128, `InitState`, `value` in `[0, 1]` inclusive,
`Range(float)`, `Range(int)`; `rainworld-re/notes/unity-random.md`), with each call
mapped to the game's call at that spot (`Random.value`, `Random.Range(int, int)` with
the same bounds, `Custom.RNV` / `DegToVec(Random.value * 360)` as
`rw_sim::Vec2::random_unit`). The app keeps one stream, as the game's single global
generator: spawning (`DetermineSafeSpawnTile`, the `Player` constructor, the graphics
module built by `PlaceInRoom`'s `AddObject`, then the chunk offsets), the water, the
player and its graphics module draw from it in the game's update order. What still
differs is only the seed and the call order:
- **Seed.** The game seeds at start-up from OS entropy and never re-seeds for anything
  ported (its `InitState` sites are story-session cycle seeds, DLC and other objects'
  save/restore sub-sequences). The app seeds with `UnityRandom::new(seed)` from
  `--seed` / the MCP `seed` (low 32 bits).
- **Call order.** The game interleaves draws from every other object, sound, particle,
  rain, camera and AI in the room and world; the port has only the room's water and the
  player, so its sequence is the game's only for those draws. Draws of unported code in
  the same functions (the rain-warning look-up, ceiling drips, `WaterDrip`s, grasps,
  exhaustion, DLC) are missing; the rain-warning look-up short-circuits before drawing
  while rain is far off, as at the start of a cycle.
- **Spawn path.** In the game only Remix key-item tracking and Watcher warps call
  `DetermineSafeSpawnTile`; the port's single-room phase uses it to spawn the player,
  so these draws are the port's choice of game function, made in the order the game
  makes them.

**Why.** Matching the game's draw sequence would need every system that draws. Seeding
from the command line keeps runs reproducible.

**Affects.** Exact random outcomes compared to a particular game run; distributions and
edge cases (`value == 1`) match.

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
`SlugcatStats` factor 1, no adrenaline, never exhausted. Stun and death are D4; vertical
poles and ledge grabs are D6; skid, flip, roll, belly slide and rocket jump are D9;
corridors are D10.

**What differs from the game where the game's behaviour is kept out of scope:**
- **Random numbers.** The player's draws (stacked-body nudge `Random.Range(-1, 1)`,
  initial facing in `Player::create`, the `Stun` threshold, the corridor-turn jiggle,
  the water) use the game's generator and calls; only seed and call order differ (D1).
  `Player::new` (tests) faces right.
- **Input.** `Input` carries x, y and jump only; `downDiagonal` is derived digitally
  (down plus left/right gives that side, the Remix rule). No analogue stick, map
  button stand-still, throw / pick-up / special. The game's `checkInput` substitution
  of `x := downDiagonal` while rolling with x = 0 therefore never applies (a digital
  down-diagonal always has x set).

**Why.** Each is either outside the stage-one scope or depends on randomness (D1).

**Affects.** Exact random outcomes (stacked head, initial facing, stun threshold).

## D4: Falls, stun and death

Body chunks report flat-tile hits (`TerrainImpact`) to their owner at the moment the game
calls it: inside the chunk's collision, after the position snap and before the bounce
(`PhysicalObject::update_with` and the `TerrainImpactHandler` trait; `Player` is the
handler), so the landing roll (D9) changes velocities in the middle of the sweep as in
the game. `Player` ports the landing roll's pre-emption of damage on that impact and the
Survivor's fall damage, stun and death (`rainworld-re/notes/falls.md`) with Remix off:
death on a
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
- **`Player.Stun` side effects.** The game un-stands the player (and drops grasps) when
  the stun exceeds `Random.Range(5, 12)`; the port draws it (`UnityRandom`, D3) except
  on the fall-impact path, where the stun is at least 40 and the threshold at most 11, so
  the port skips the draw (one fewer draw in the sequence). Malnourished stun scaling, grasps, spear and slugpup drops, sounds
  and the face blink are not ported.
- **Remix (MMF) branches omitted:** falling into a room-exit or normal shortcut entrance
  at speed > 5 (enters the shortcut instead of taking damage) and the wall pounce (D9).
- **More Slugcats omitted:** Gourmand's thresholds (death above 80, stun above 40
  with the 40..140 ticks mapped over speeds 40..80), tongue attachment and being carried by another player blocking fall damage,
  Sofanthiel's sound. Base-game branches with no ported counterpart: the Pomegranate
  grasp and the deer-antlers ride cancelling fall damage.
- **Water.** Drowning is D11; inverted water (DLC) is not modelled.

**Why.** The freeze and the respawn are design decisions for the single-room
player-controller phase; the rest is out of the base-game scope or randomness (D1).

**Affects.** What happens after death and Remix / More Slugcats play.

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

## D6: Vertical poles and ledge grabs

`Player` ports `GrabVerticalPole`, the pole under-hang grab, `ClimbOnBeam` (climb and
slide forces, side offset and switching, the neighbour-pole hop, the exits to horizontal
beams, `StandUp` at the bottom, the slide-up hop), `GetUpToBeamTip`, `BeamTip` and its
up-press trigger, `HangUnderVerticalBeam`, the `ClimbOnBeam` branch of `Jump`, and
`LedgeGrab` (trigger, haul-up, drop, the `WallJump` hop and the no-x jump)
(`rainworld-re/notes/vertical-poles.md`, `ledge-grab.md`), base game with Remix off.
`slowMovementStun` is ported as state with its effects on the run force, the beam
shuffle and pole climbing; the slide-up hop sets it, as do the belly slide (D9) and
corridors (D10).

**What differs:**
- **Remix (MMF) branches omitted:** the climbing-grip throw recoil on a pole
  (`cfgClimbingGrip`; throwing is not ported), the vine grab's exclusion of
  `ClimbOnBeam` (vines are not ported). The corridor exit onto a pole is in D10.
- **More Slugcats / Watcher omitted:** the slugpup's smaller slide-up hop (2.25 px,
  +1 vy, 0.3x lift, 0.5x slide forces) and the slugpup, Rivulet and Gourmand
  (`PainJumps`) sideways pole jump values, Saint's `monkAscension` blocking the grab,
  and the ripple-levitation freeze of the facing (`rippleActivating`).
- **Not ported entries to the pole:** `ZeroGPoleGrab` (zero gravity) and the deer antlers. Sounds, the blink and
  `AerobicIncrease` on the pole are not ported (no effect on movement for the
  Survivor, who is never exhausted).

**Why.** Remix and DLC are out of the base-game scope; the rest waits for zero gravity,
grabbing and audio.

**Affects.** Remix / DLC play on poles.

## D7: Room graphics (level image and palette)

The level graphic follows `rainworld-re/notes/room-rendering.md`: settings with
template inheritance (`rw_assets::RoomSettings`), palette with effect colours and the
per-camera fade (`PaletteTexture`), the camera image, `level.wgsl` (the basic path of
the game's level shader, checked against a CPU reference), sprites behind the level
showing only through sky and depth >= 5, and the room camera's zone switching and lean
(`rw_render::RoomCamera`).

**What differs:**
- **DXT texture decode.** `_NoiseTex` (a 64x64 DXT1 texture) and the sprite atlases
  (DXT5) are decoded on the CPU from `resources.assets` by `rw_assets::unity` and
  uploaded as RGBA8; the game's GPU samples the compressed blocks and interpolates with
  its own rounding, so texels may differ by 1/255 from what the game shows (the CPU
  decode truncates, matching AssetRipper's export of every texture exactly). Only mip 0
  is kept (the noise shader always magnifies it; the atlases have one mip).
- **Window and scaling.** As in the game, the whole frame (level, sprites, debug
  overlays) is drawn without antialiasing into an offscreen frame of the `ScreenSize`
  option (`--resolution`, the game's five choices, 1366x768 by default), shown full
  screen by default, scaled to the largest fitting rectangle of its aspect (fractional
  scale) with black bars, or with `--windowed` 1:1 in a non-resizable window of that
  size (`rainworld-re/notes/room-rendering.md` section 10). Differences: the filter of
  the scaled case is Unity-internal and not in the game's code; the port assumes
  bilinear. `--screenshot` runs and the MCP mirror window are always windowed (MCP
  screenshots are headless and return the frame itself, at a size the caller can
  change), and a window the window manager sizes differently from the resolution is
  letterboxed the same way. The Steam Deck case (1229x768 full screen as a 1280x800
  window) is not ported.
- **Camera switch timing.** The game loads the next camera's image asynchronously and
  cuts when it has loaded (usually the next frame); the port decodes every camera's
  image when the room loads and cuts in the same tick.
- **Camera in shortcuts.** While the player is in a shortcut the camera follows the
  centre of the vessel's tile; the game uses the vessel's interpolated on-screen position.
- **Fade quantisation.** The blended palette is stored as 8-bit texels like the game's
  ARGB32 palette texture; the rounding mode of Unity's `SetPixel` is assumed to be
  round-to-nearest.
- **Not drawn:** object-cast shadows (needs the room's light angle and the edge-stretch
  helper of the shader includes), wet-terrain ripples (`_WetTerrain` is on by default in
  the game, shifting level pixels by up to one row in thin bands above the water line),
  rain-cycle darkening (`DarkPalette`, `ShaderLight` before rain), lightning, room
  effects that grade colours (Darkness, Brightness, Contrast, Desaturation, Hue, DayNight)
  or modify effect colours (`ModifyEffectColorA/B`, unused in base-game rooms), ghost /
  rot / mushroom palettes, screen shake, `_bkg` backgrounds, the heat / melt / ripple
  shader variants, swarm rooms, and the properties-file line preprocessing (no base-game
  `Room Setting Templates` line uses it).

**Why.** The single-room phase draws one room with the player; these depend on systems
not ported yet (rain cycle, room effects, objects) or are invisible at this stage.

**Affects.** Exact pixel colours of cloud shadows (noise rounding) and rooms with the
listed effects; the softness of the frame when scaled full screen.

## D8: Slugcat graphics (`rw_graphics::PlayerGraphics`)

`PlayerGraphics` ports the base-game Survivor's `PlayerGraphics`, `SlugcatHand`, `Limb`,
`BodyPart`, `TailSegment` and `GenericBodyPart` for the ported body modes and animations, including the vertical-pole poses
(`ClimbOnBeam`, `GetUpToBeamTip`, `BeamTip`, `HangUnderVerticalBeam`), `LedgeGrab`,
the roll/flip tail fling and the corridor branches (`CorridorClimb` legs and
`LegsAClimbing` frames, the doubled downward head lead, the hands gripping the corridor
walls ahead, the `CorridorTurn` jitter and blink), plus the pokes `Player.Update` makes
into its graphics: `Player.Blink` from hard landings (`TerrainImpact`, speed > 12) and
the pole slide-up hop, and the 10 px/tick push of every body part when a corridor turn
ends (`rainworld-re/notes/slugcat-graphics.md`), Remix off. The skid, belly slide,
rocket jump, corridor shoots and slides and corridor drops have no branch of their own
in `PlayerGraphics` (they use the generic poses of their body mode, as in the game).

**What differs:**
- **Random numbers.** Cosmetic randomness (blink timing, the 1 px reset jitter, beam
  balance, long-fall hand flailing, the corridor-turn jitter, the empty-room object
  looker's draws) uses the game's generator and calls, from the same stream as the
  player right after `Player.Update`, as the game's `Room.Update`; only seed and call
  order differ (D1).
- **Swimming** is ported (draw offset from `swimCycle`, tail drag scaled by hip
  submersion, the `lungsExhausted` head bob and blink, the swimming hand targets, legs
  hidden in `DeepSwim`); bubbles and splashes are not (D11). Under water the sprites
  are tinted by the water's `DeepWater` shader, as in the game (D11).
- **`aerobicLevel` is 0.** The player has no aerobic level yet, so breathing runs at the
  resting pace (`breath += 1/60` per tick), never pants, and the exhausted face and
  head bob never show.
- **Shortcut entry timing.** The game's extra graphics update while being pulled into a
  shortcut runs inside `Creature.Update`, before the pull moves the chunks; the port runs
  it after `Player::update`, so it sees the pulled positions. The reset on leaving a
  tube likewise uses the chunk positions after that tick's movement.
- **No look targets.** With no other objects in the room the game's `PlayerObjectLooker`
  never finds a target, so `lookDirection` stays 0 as in the game; only its look timer
  and random draws are ported. The rain-warning look-up, `LookAtNothing` and the afraid-of-creature poses (and their
  blink and aerobic effects) are not ported.
- **Not ported:** grasps (holding, throwing, eating, swallowing, spear on back, reaching
  for flies, heavy carry, and the `Blink` calls of eating, mauling and throwing),
  the map-button stand-still blink,
  zero gravity (`ZeroGPoleGrab`, `ZeroGSwim`), antler and vine climbing, the sleep curl,
  adrenaline jitter, malnourishment and hypothermia tints, the Mark (sprites 10 and 11
  have alpha 0 and are left out of the draw list), the light source, and every More
  Slugcats, Watcher and Jolly Co-op branch (pups, Gourmand/Spear/Saint/Rivulet/Artificer
  parts and sizes, gowns, custom and Jolly colours). The Nightcat colour is not offered
  (`SlugcatName` has White, Yellow and Red).
- **Drawing (`rw_render` sprites).** `Futile_White` meshes (the tail) sample the centre
  of `RainWorld_White` in the `rainWorld` atlas instead of the runtime 16x16 white
  texture (both opaque white, so the colour is the same). Sprites are drawn without
  antialiasing into the game-resolution frame (D7), so edges are hard pixel edges as in
  the game. The debug view (V) is not the game's: it shows the physical body (chunk
  circles with their radii, connection rods, contact marks, stun/dead styling) instead
  of the sprites, whose state is still updated every frame.
- **Legs on curved terrain.** The grounded legs ignore `terrainCurveNormal` (not
  ported; it is only set by `TerrainCurve` objects, which the port has none of).

**Why.** Randomness as D1; the rest depends on player state that is not ported yet or
is outside the base-game scope.

**Affects.** Exact blink timing and jitter; the look of the slugcat in the unported
states (they fall back to the nearest ported pose: Default for the airborne ones).

## D9: Skid, flip, roll, belly slide and rocket jump

`Player` ports the Stand-mode turn-around skid and its flip jump, `Flip`, the landing
roll in `TerrainImpact` (`allowRoll`, `consistentDownDiagonal`), `Roll` and the roll
jump, `BellySlide` (start, forces, drop through a one-tile hole, exits, the rocket and
whiplash jumps), `RocketJump`, the 10 px rod while `rollDirection != 0` and the belly
slide's `slowMovementStun` (`rainworld-re/notes/advanced-moves.md`), base game with Remix
off, Survivor values, no adrenaline. As in the game, the whiplash flip turns with the
last `slideDirection` written by a Stand-mode skid check (0 if none), not the slide's
direction.

**What differs:**
- **Remix (MMF) wall pounce omitted** (`cfgWallpounce`: a fast head-first wall hit with
  a buffered jump launches a `RocketJump` away from the wall).
- **Held items:** the long belly slide (`longBellySlide`, started by throwing during a
  slide, with its 39-tick push, 34-tick jump window and own exit and jump values),
  picking up an item during the whiplash jump, and the thrown-spear roll are not ported
  (no grabbing or throwing).
- **Other creatures:** a belly slide or roll hitting a creature (stun, `rollCounter`
  cap, "slugsmash") is not ported (no creatures).
- **More Slugcats / Watcher omitted:** Rivulet, slugpup and Gourmand values (skid and
  flip jump, roll jump, belly-slide push, kick and jumps, roll durations), the pyro
  jump's `Flip`, and the Watcher curved-terrain drop-through test.
- **Not ported:** adrenaline factors, sounds, `WaterDrip` and `ExplosionSpikes`
  particles (their random draws are cosmetic), `AerobicIncrease` (the Survivor is never
  exhausted).

**Why.** Remix and DLC are out of the base-game scope; the rest waits for grabbing,
throwing, creatures, adrenaline and audio.

**Affects.** Remix wall pounces, item tricks during slides, slides and rolls into
creatures, DLC slugcats.

## D10: Corridors

`Player` ports corridor detection (`goIntoCorridorClimb` over the eight directions, the
3-tick entry with ground classification skipped while counting, immediate exit), the
`CorridorClimb` body mode (damping, gravity compensation, the climb push for the first
chunk with an open tile ahead, centring, `backwardsCounter`), the shaft shoot and tunnel
slide with their counters and `slowMovementStun`, the U-turn (`CorridorTurn`, 40 ticks
on a `Pull` rod), `corridorDrop`, `dropGrabTile` and its catch in `Default` mode, the
exits onto vertical poles and horizontal beams, the `Jump` branch, `shootUpCounter`,
`timeSinceInCorridorMode` and the corridor exclusions of wall-slide braking and the run
force (`rainworld-re/notes/corridors.md`), base game with Remix off, Survivor values.

**What differs:**
- **U-turn jiggle.** Ported (`Vec2::random_unit` on the first 30 ticks); the draws come
  from `UnityRandom` in a different call order than the game's (D3).
- **Remix (MMF) omitted:** the pole exit from a corridor ignoring `shootUpCounter`
  (`ModManager.MMF ||` in the up-exit test), and the MMF use of `EffectiveRoomGravity`
  in the water term.
- **More Slugcats / Watcher omitted:** the slugpup's halved shoot, slide and slide
  damping factor, the Rivulet (14) and slugpup (4) `jumpBoost` of the corridor `Jump`
  branch, Saint's `monkAscension` blocking the beam exits.
- **Not ported:** the zero-gravity branches (shaft `canCorridorJump` with the head down,
  the zero-G slides), carried objects dragged through corridors, adrenaline factors, sounds and
  the `WaterDrip` particles of the slides (their random draws are cosmetic).

**Why.** Remix, DLC, zero gravity and grabbing are out of scope.

**Affects.** Remix / DLC play in corridors.

## D11: Water and swimming

`rw_sim::Water` ports the room's `Water` object with the full wavy surface (front and
back point rows every 20 px, splashes when a chunk crosses at |vy| > 3, straddle drag,
the wave equation with solid-tile damping, jitter, drift correction and neighbour blend,
the smoothing and the `waveSpeed`/`waveLength`/`waveAmplitude`/second-wave offsets from
the room settings). `BodyChunk` ports submersion, buoyancy, water friction and the skim
rule; `Player` ports `LungUpdate` (lungs 128 as in a release build, drowning), the
`Swimming` body mode, `DeepSwim` and `SurfaceSwim`, the swimming exits and the grab
restrictions (`rainworld-re/notes/water.md`), base game with Remix off, Survivor values.

**What differs:**
- **Random numbers.** The game's generator and calls; only seed and call order
  differ (D1).
- **Remix (MMF) omitted:** `swimBoostCost` 0.18 (base 0.2), the `cfgSwimBreathLeniency`
  curve, `cfgFreeSwimBoosts`, `EffectiveRoomGravity` in the corridor buoyancy term and
  the submersion cap above the room.
- **DLC omitted:** inverted water, the More Slugcats `PointSubmerged` `<` test (base
  uses `<=`), the Rivulet/Artificer/Saint/Watcher/energy-cell/jetfish branches and the
  More Slugcats flood.
- **Not ported:** `WaterCutoff` and `AirPocket` placed objects (no base-game room
  uses them), `WaterViscosity` (unused in the base game), rain rumble, screen shake and
  flooding, lethal water, splashes, bubbles and sounds, ripple waves from explosions,
  `aerobicLevel` (stays 0; for the Survivor it only feeds graphics), the analogue swim
  direction (digital input only).
- **Slerp for opposite vectors.** Unity's `Vector3.Slerp` between antiparallel vectors
  picks an axis the notes do not record; the port turns counter-clockwise.
- **Drawing.** The level view draws the water as `Water.DrawSprites` does
  (`rw_render::WaterSprites`, `shaders/water.wgsl`; `rainworld-re/notes/water-rendering.md`):
  both rows with `ApplyDepthOnVector`, the `WaterSurface` band and the `DeepWater` body,
  palette water colours, shine, the player light with `outOfBreathFac`, `_waterDepth`
  from `waterInFrontOfTerrain` (2/31 for `GATE_` rooms, taken from the name), in the
  `Water` container after the level, foreground sprites and shortcuts. The debug view
  keeps its own flat translucent water. Still differing: no light sources screen onto
  the surface colour (`GetSurfaceColor`'s light loop; no lights are ported), splashes
  and bubbles (separate `Splash` / `Bubble` objects, not part of `Water`'s sprites) are
  not drawn, the water gates' `cosmeticLowerBorder` / `cosmeticSurfaceDisplace` and the
  DLC branches (`WaterSlush`, air pockets, inverted water, `Gutter`, ripple, brain
  mold, `WaterDepth` effect) are left out. The `_GrabTexture` the shaders read is the
  pre-level target (what LevelColor's grab pass holds in the game, INFERRED), and
  `room.waterGlitterCycle` starts at 0 when the room is loaded.

**Why.** Remix and DLC are out of the base-game scope; the rest is cosmetic or needs
objects and effects that are not ported.

**Affects.** Exact wave and swim paths; Remix / DLC water; how the water looks.

## Not ported yet (not divergences, just absent)

Player (stage one leaves out): zero gravity, grabbing / throwing / eating (`GrabUpdate`),
`slowMovementStun` from exhaustion, throwing, adrenaline, the WALLSTUCK safety
(needs `VisualContact`), carried-object speed factors, vines, tube worm,
Jolly co-op, MSC / Watcher class mechanics. Ported because ground classification
produces them anyway: wall slide and wall jump (`WallClimb`, `canWallJump`, `jumpStun`;
only the wall-jump impulse has a test), `LedgeCrawl`, `CrawlTurn`, the crouch super-jump
charge and dive force (untested).

Physics: burrowing, terrain curves (`TerrainCurve`), `setPos`
(cross-object position writes), the curved-terrain `TerrainImpact` call, grabs (`grabbedBy` skips the room-range clamp, and grabs force `goThroughFloors`), and
`DefaultTile`.
