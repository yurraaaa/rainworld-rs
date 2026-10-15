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
player and its graphics module draw from it in the game's update order. Every sound the
ported code plays makes `VirtualMicrophone.PlaySound`'s draws on it where the game calls
`room.PlaySound` (`rw_sim::Sounds`; the order is in `rainworld-re/notes/sound.md` 2.8),
loaded from `sounds.txt` in the window and in MCP alike, so a run's sequence does not
depend on whether audio is playing. The menus draw from the same stream before the game
does: the intro roll's `RainEffect` (2508 draws when built: five per drop, eight for
the wind; every tick two per drop, five more per drop that resets, eight for the wind,
one when the highlight wraps, one or two for the lightning) and its random lightning
past 14 s (one draw per tick, three more on a spike), the flat scenes' camera drift
(`InteractiveMenuScene`, every tick of a shown scene) and the menu sounds (`MenuMicrophone.PlaySound`/`PlayLoop` make the same
`SoundLoader` draws), and the game process takes the stream over where the menus left
it (D13). What still differs is only the seed and the call order:
- **Seed.** The game seeds at start-up from OS entropy and never re-seeds for anything
  ported (its `InitState` sites are story-session cycle seeds, DLC and other objects'
  save/restore sub-sequences). The app seeds with `UnityRandom::new(seed)` from
  `--seed` / the MCP `seed` (low 32 bits) once, when the process manager starts; a
  run that boots into the menus reaches the game with the stream already advanced.
- **Call order.** The game interleaves draws from every other object, sound, particle,
  rain, camera and AI in the room and world; the port has only the realized rooms'
  water and the player, so its sequence is the game's only for those draws. Sounds of
  rooms the camera is not in make no draws, as in the game (`Room.PlaySound` only
  reaches the microphone of a camera in that room). Draws of unported code in
  the same functions (the rain-warning look-up, ceiling drips, `WaterDrip`s, grasps,
  exhaustion, DLC, and the sounds of those) are missing, as is the draw-skipping of
  `AllowSound`'s crowd cap (D12); the rain-warning look-up short-circuits before drawing
  while rain is far off, as at the start of a cycle.
- **Spawn path.** A new story starts as the game does (start den, `TryGetPlayerStartPos`,
  the node drawn in `RealizeInRoom`). In the game only Remix key-item tracking and
  Watcher warps call `DetermineSafeSpawnTile`; the port's dev starts (`--room` without
  `--spawn`, MCP `load_room`, R) use it to place the player, so these draws are the
  port's choice of game function, made in the order the game makes them.

**Why.** Matching the game's draw sequence would need every system that draws. Seeding
from the command line keeps runs reproducible.

**Affects.** Exact random outcomes compared to a particular game run; distributions and
edge cases (`value == 1`) match.

## D2: Spawning in rooms without shortcuts and in shelters

**What.**
- The game's first spawn phase throws (`shortcuts[0]` on an empty array) in a room with
  no shortcuts. The port skips that phase and goes on to the fallbacks.
- The shelter branches of `DetermineSafeSpawnTile` are not ported: it only places the
  player in the port's dev starts (D1), never in a story start.

The shortcut map itself (`rw_assets::ShortcutMap`) is the game's `ShortcutMapper`
(node order, shortcut order, tube walk, types), no longer inferred.

**Why.** The game's behaviour is a crash; the shelter branches serve no story path.

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
  `rw_sim::Player` keeps simulating it; `RainWorldGame::tick` stops ticking a dead
  player (the world, its rooms and the realizer go on) and the app draws it where it
  stopped until R respawns a new one in the camera's room. R also respawns after the
  body fell out of the room (the game has no respawn).
- **`Player.Stun` side effects.** The game un-stands the player (and drops grasps) when
  the stun exceeds `Random.Range(5, 12)` and draws `Random.Range(40, 80)` for the spear
  on the back; the port makes both draws on every path (no spear to drop) and plays
  `UI_Slugcat_Stunned_Init`. Malnourished stun scaling, grasps, slugpup drops and the
  face blink are not ported.
- **Remix (MMF) branches omitted:** falling into a room-exit or normal shortcut entrance
  at speed > 5 (enters the shortcut instead of taking damage) and the wall pounce (D9).
- **More Slugcats omitted:** Gourmand's thresholds (death above 80, stun above 40
  with the 40..140 ticks mapped over speeds 40..80), tongue attachment and being carried by another player blocking fall damage,
  Sofanthiel's sound. Base-game branches with no ported counterpart: the Pomegranate
  grasp and the deer-antlers ride cancelling fall damage.
- **Water.** Drowning is D11; inverted water (DLC) is not modelled.

**Why.** The freeze and the respawn are design decisions for the player-controller
phase (until the game loop with death screens exists); the rest is out of the base-game scope or randomness (D1).

**Affects.** What happens after death and Remix / More Slugcats play.

## D5: Shortcuts

`rw_assets::ShortcutMap` is the game's `ShortcutMapper`; `Player` ports entering by
pressing into a hole, the pull-in, the vessel (`rw_sim::ShortcutHandler`, one tile per
three ticks) and the spit-out at the far entrance (`rainworld-re/notes/shortcuts.md`),
base game with Remix off.

Room exits are entered and lead to the connected room as in `ShortcutHandler`: going
in activates the connected room (`SuckInCreature`), and the vessel reaching the exit node moves to `betweenRoomsWaitingLobby` with the
destination's exit leading back (`ExitIndex`) as its entrance node; the lobby realizes
an unrealized destination (the followed creature) and moves the vessel in once the
room is `ReadyForPlayer` (`waitToEnterAfterFullyLoaded` below 1: 40 ticks with
`SSSwarmers`, 80 with active coral, wall mycelia, corruption or vines, 80 in the story
`SS_AI`/`SL_AI`), restarting it on that node with the entrance blink skipped and the
camera moved to the node's `viewedByCamera`. An exit with no connection (`-1`) stops
the whole shortcut update that tick, a node past the connections bounces the vessel
back ("faulty room exit"), as in the game.

**What differs:**
- **One vessel, owned by the player.** The game's `ShortcutHandler` keeps a list of
  vessels for every creature, with Jolly co-op waits and border travel; the port has
  one player whose vessel lives on `Player` or in the lobby, `wait` is always 0, and
  the handler is the three-tick divider plus that lobby.
- **Not ported:** being pulled in by a carried object (no grasps), `cantBeGrabbedCounter`,
  the entrance blink (`BlinkShortCut`, also on arrival), MSC tongue release, the arena
  hooks of `SpitOutOfShortCut` and den entry (`PlayerTryingToEnterDen`), dens and NPC
  transports for creatures, region transportation, and Remix falling into a shortcut
  (D4).

**Why.** Outside the single-player, creature-free, base-game scope.

**Affects.** Multiple creatures in tubes, co-op, arena.

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
- **Not ported entries to the pole:** `ZeroGPoleGrab` (zero gravity) and the deer antlers. The blink and
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
  image when the camera enters the room (`MoveCamera`, synchronous: D14) and cuts in
  the same tick.
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

**Why.** The port draws the camera's room with the player; these depend on systems
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
- **Not ported:** adrenaline factors, `WaterDrip` and `ExplosionSpikes`
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
  the zero-G slides), carried objects dragged through corridors, adrenaline factors and
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
  flooding, lethal water, splashes and bubbles, ripple waves from explosions,
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

## D12: Sound

`rw_sim::Sounds` is the `Room.PlaySound` layer (one-shots disembodied, at a position or
on a body chunk; chunk loops with handles), `rw_assets::SoundTable` the `SoundLoader`,
`rw_assets::unity::Fsb5` the clip decoder, and `rw_audio` the `VirtualMicrophone` with
its `AmbientSoundPlayer`s, playing on kira (`rainworld-re/notes/sound.md`). Every base-game
`Player` sound, the shortcut tick and the water-surface splashes are ported. The menus'
`MenuMicrophone` is `rw_menu::MenuMic` (what plays, the draws, the loops' fades) and
`rw_audio::MenuPlayback` (the sources), with the default options' sound-effects volume
0.8. `rw_audio::music` is the `MusicPlayer` side process with `Song`, `MenuOrSlideShowSong`
and `IntroRollMusic` (`music.md` section 4.1), streaming `music_songs` clips; the app
runs it as the game's side process (D13), silent on `TimedSources` without audio
(`--mute`, `--screenshot`, MCP).

**What differs:**
- **Music not ported:** `GameRequestsSong` (room-trigger `MusicEvent`s, play records),
  the in-game `PlayerThreatTracker` (threat, ghost mode, room switches and their
  `Random.Range(80, 400)`; outside the game its zeroing is ported), `ProceduralMusic`
  (threat layers, `music_procedural`, its `Random.Range` stem choice), the arena
  `MultiplayerDJ` (its shuffle draws), `GhostSong`, `SSSong`, `VoidSeaMusic`, the DLC
  songs, `ResetSongStream`, MMF stream resync, and loose mod `.ogg` overrides. None of
  the ported paths draws from `UnityEngine.Random`.
- **Music timing.** A sub-track's clip loads on a background thread and becomes
  ready on the first update after it arrived (the game's `AssetBundle` load and
  `LoadAudioData` take a frame or more, so this is at least as fast); headless runs use
  `TimedSources`, where clips load at once and play for their FSB5 length in ticks.
  `IntroRollMusic.StartMusic` takes effect in the music player's next update (in the
  game it starts the source at once; the player runs right after the main process in
  the same tick, so nothing reads the difference). A missing `music_songs` bundle still
  sets `assetBundlesLoaded` (the game would wait forever) and its songs stay silent.
- **Not ported:** the rain and thunder loops and one-shots and the
  drown-out of volume group 0, creature, item and object sounds, the sounds of
  unported menu screens and objects, `PlayCustomSound` / loose mod clips next to
  `sounds.txt` (a name found only there counts
  as present for the draws but is not decoded), the Watcher ripple ambience and DLC
  sound IDs. Deafness, fade to black and `globalSoundMuffle` stay 0; time speed is 1.
- **`AllowSound`'s crowd cap is applied by the audio, after the sim made the draws.** In
  the game a refused sound makes no draws after `silentChance`'s; the cap depends on how
  many sounds the audio thread still plays, which the tick-deterministic sim does not
  know. With audio off nothing is capped. PLAYALL triggers are capped per sample, not
  once per call.
- **Music volume and options.** The default options (music volume 0.8, `playMusic` on);
  the options menu and `setup.txt` are not read.
- **Voices.** Unity mixes 32 real voices chosen by priority (all 128) and audibility; the
  port silences the quietest past 32 each frame. There is no `AudioSource` start
  latency, and a sound plays from the frame its clip finishes decoding (the game's
  `loadOp`; decoding is on two background threads, cached for the session).
- **Mixing.** Pan uses kira's constant-power law, not Unity's `panStereo` curve; the
  `AudioLowPassFilter` (22 kHz to 1.5 kHz by the root of the effect, Q 1) is kira's
  state-variable filter at the same cutoff and Q; the master gain of 6 (`AudioListener.volume`)
  clips in the output device as in Unity, with no limiter.
- **Doppler** samples every 0.05 s of wall-clock time since the audio started, as the
  game samples `Time.time`; neither is tick-deterministic.
- **Listener.** `RoomCamera.pos` is the window origin the room camera draws this tick
  (whole pixels); without a level graphic it is the room origin.

**Why.** Out of the base-game player scope (in-game music, rain, creatures, UI), or tied to
real-time audio rather than the sim.

**Affects.** What is heard, never the sim (its draws do not depend on the audio).

## D13: Front end (process manager, menus, menu input)

**What.**
- **Start process.** `Initialization` is reduced to one tick: fonts, strings and
  settings are loaded before the manager starts, and it requests the start process at
  once. As in the game's distribution build (`setup.startScreen`) it starts `IntroRoll`,
  which leads to the main menu. `--spawn`, `--script` and `--screenshot` start `Game`
  directly, and `--process` picks (`intro-roll`, `main-menu`, `slugcat-select`, `game`,
  `font-test`). The boot fade, the switches and the fades follow the game.
- **Intro roll.** Ported in full (`rw_menu::IntroRoll`, `RainEffect`). The game's
  waits on the sound loader, platform, options and progression are always satisfied
  (all loaded before the manager starts), so `delayedTime` waits only for the music
  bundles; `requestIntroMusic` (options not ready) never arises. `ProcessManager.
  initializationAndIntroRollStarted` and `activityID` (console activities) are not
  ported, nor `Cursor.visible`. `RainEffect`'s light source uses `FlatLightNoisy`
  with `_spriteRect` unset (0, 0, 0, 0), as in the menus at boot: the noise lookup
  divides by zero in both, and what the GPU then samples is whatever the driver does
  with non-finite coordinates (the effect on the light's falloff is small).
- **Port-only process.** `ProcessId::FontTest` (labels in both fonts over the flat
  main-menu scene) is not a game screen; it exists to compare drawing by eye. Its
  labels sit on whole pixels, where `font`'s glyphs lose a pixel row to point sampling;
  the game's menus place labels a third of a pixel off (the page sits at (1/3, 1/3)),
  and the port's menus draw them whole.
- **Unported screens.** Every process but the game, the main menu and slugcat select
  (the main menu's Regions, Arena, Remix and Options, and the pause, sleep and death
  screens) is a black screen naming the process; the pause button (Escape) returns to
  the main menu. In the game Escape still quits the app until the pause menu exists.
- **Flat scenes.** Menu scenes are shown in the game's flat mode (`flatmode.txt`: one
  `<scene> - Flat.png`; `rainWorld.flatIllustrations`) until depth scenes
  (`MenuDepthIllustration`, its shaders) are ported. The flat-mode branches follow the
  game: no slugcat select rain effect; the scenes' idle camera still runs and draws
  random numbers every tick, as in the game.
- **Saves, options and progression.** Nothing is read from or written to the game's
  saves or options: the progression is a new one (no saved games, no discovered
  shelters so Regions is greyed, Hunter locked unless `StreamingAssets/unlockred.txt`
  exists or the main menu's unlock code is entered, which unlocks for the session only)
  and the options are the defaults, except that the Downpour/Remix first-launch notices
  (`dlcTutorialShown`, `remixTutorialShown`) count as already shown, so the main menu
  opens without its popup. `options.Save()` and `SaveProgression` are not called.
- **DLC detection.** Until DLC support exists the port treats every DLC as not
  installed (user decision): `dlcsInstalled` is empty, so the main menu shows the plain
  `Main Menu` scene and the intro roll the plain `Intro_Roll_C` (no random slugcat
  draw). The game finds `dlcsInstalled` from files (`DLCOwnershipCheckSwitchPC`); with
  this install's Downpour files it would show `Main Menu - Downpour` and a random
  `Intro_Roll_C_<slugcat>` (one `Random.Range(0, 5)` after the rain effect's draws).
  The check is kept as `rw_menu::Dlcs::check` and the Downpour branches are ported, for
  DLC support to turn back on.
- **Starting a game.** The slugcat select menu's held START starts a new game
  (`StoryGameInitCondition.New`) at the character's start: its region's world, the
  start den (`GetStoryDenPosition`: `SU_C04`, the Hunter `LF_H01`) and
  `TryGetPlayerStartPos` (D14). The Survivor's and the Monk's intro slideshows are
  skipped (the game would play them first unless S is held) and the save is not wiped
  (there is none). The chosen slugcat colours the body's graphics; its body moves as
  the Survivor's (the Monk's and the Hunter's `SlugcatStats` are in
  `rw_sim::campaign::SlugcatCampaign` but the player does not read them yet). Continue,
  Restart and Statistics need saves and are not reachable. Started directly
  (`--process game`, `--spawn`, MCP), the game is the Survivor in the given room's
  world.
- **Side processes.** The music player (made in the first `ProcessManager.Update`,
  ticking on its own time stacker after the main process, `UpdateMusicContext` after
  every switch) and the menu microphone run, in the game's order; no dialogs or Steam
  manager, so the switch queue stays empty. The game drops `manager.menuMic` on a switch
  to the game but leaves the old microphone running as a side process (its loops fade
  out) and makes a new one on the way back; the port keeps one microphone and tracks
  whether the manager holds it, which sounds the same. Mod finalization completes in the
  same frame (no mods); `progression.Revert` is not done. Of `HardCleanFutileAssets`
  only the illustration atlases a process loaded are freed.
- **File resolution.** `AssetManager.ResolveFilePath` looks in `mergedmods/` and the
  active mods first; the port runs the base game with no mods and reads
  `StreamingAssets/<lower-cased path>` only.
- **Strings.** English only; an encrypted `strings.txt` (leading `1`, xor with a key
  string in the game's code) is not read (no vanilla file is encrypted). Mods' string
  files are not merged.
- **Text layout.** `FFont`'s Thai placement rules and other languages' fonts are not
  ported.
- **Menu input.** Keyboard and mouse. Keys come from Rewired's shipped default UI map
  (decoded from `level0`, `rainworld-re/notes/menu.md`): arrows move, Return/Space/Z
  submit, Escape cancel, X "alternate"; the pause button is Escape. Saved custom
  bindings (`RewiredSaveData` in PlayerPrefs), gamepads and the options'
  `mouseButtonMappings` are not read. When two opposite keys of an axis are held the
  port sums them to 0 (Rewired's rule not checked). Whether the default mouse map (left
  button = UI submit, map category 1) is active for player 0 is not settled; the menus
  read the mouse position and buttons directly, as `Menu` does with
  `Input.GetMouseButton`. The cursor is shown full screen and windowed alike
  (`options.fullScreen` stays at its default).
- **Gameplay keys.** Rewired's default gameplay keyboard map: arrows move, Z jumps. Its
  other keys (X throw, left Shift pick up, Space map, C special) have nothing to drive
  yet. The intro roll's "any button" (`ControlSetup.GetAnyButton`) is any key of the
  default maps or any mouse button; Rewired's handling of keys bound in no map is not
  checked (the port knows no such keys).
- **Frame time.** `Time.deltaTime` is capped at 1/3 s, Unity's default
  `maximumDeltaTime`; the game's own setting is not checked. (The game previously ran on
  a fixed timestep with a 250 ms cap; it now runs under `MainLoopProcess.RawUpdate`:
  ticks only while more than one is accumulated, at most three per frame.)
- **Random stream (D1).** One stream, seeded once from `--seed`, from the first menu to
  the game.

**Why.** The menus are ported screen by screen, with no saves or options yet; the
creature-free world (D14) stands in for a story session.

**Affects.** What is shown at start-up and between processes, and where the random
stream stands when the game starts; no gameplay.

## D14: Worlds, rooms and the room realizer

`rw_assets::world` loads `world/regions.txt` and a region's `world_xx.txt` as
`WorldLoader` does (`rainworld-re/notes/world-loading.md`): line preprocessing with
timeline filters and `{...}` conditions kept as data, the `EXCLUSIVEROOM` merge, the
conditional links, `ROOMS` with tags, global room indices, `CappingBrokenExits` and the
off-screen den. `rw_sim::world::World` keeps the abstract rooms and the realized ones,
`RoomRealizer` (`room-realizer.md`) realizes and abstractizes them around the player,
and `RainWorldGame` runs them in the game's update order. A world comes from the first
room as `OverWorld.LoadFirstWorld` picks it (the region before the first `_`, a gate's
second part, else a single-room world for arena levels). The campaign description
(`rw_sim::campaign`, `campaign-model.md`) gives the timeline, start den and stats.

**What differs:**
- **Creature-free.** No creatures, `CREATURES` lines, dens, spawners, bat blockages,
  swarm maps or abstract-space AI; the player is the only object. Rooms are realized
  and abstractized with nothing in them; the realizer's per-creature bookkeeping only
  sees the player.
- **Synchronous loading.** The game's `RoomPreparer` loads a room over several frames
  (and `RoomCamera.MoveCamera` its images asynchronously); the port loads a room's
  files and builds its water in one call, finishing at the end of the tick the room is
  activated. A room is therefore `fullyLoaded` sooner than in the game: a room exit's
  destination (activated when the player goes in) is always loaded by the time the
  vessel reaches the exit node, so the lobby only waits out the room's entry delay
  (`waitToEnterAfterFullyLoaded`, counting from the tick it loaded), and the
  realizer's "currently loading" pause never spans ticks.
- **Next-room prediction.** The realizer's guess (`RoomRealizer` with the followed
  creature's AI map, `ExitDistanceForCreatureAndCheckNeighbours`) needs the baked AI
  maps of the room files, not loaded. The port measures the distance to each exit by a
  breadth-first search through non-solid tiles, plus the straight-line term the game
  adds; the rest of the realizer (budget 1500, estimates, neighbours, distant deletion,
  recently-abstracted list) is the game's.
- **Re-preprocessing old room files.** The game re-preprocesses room files baked before
  generation 20 (and writes them back). Every room the vanilla worlds list is generation
  20 or later; for older files the port derives only the shortcut nodes (exits, dens,
  region transports with length, submersion and nearest camera), not border exits,
  bat hives or garbage holes.
- **Off-screen den and gates.** The off-screen den is never realized; gates are
  ordinary rooms (no gate mechanism, no region change), and a `RegionTransportation`
  node goes nowhere.
- **Not ported:** the rain cycle, karma and karma gates, shelters closing and
  hibernation, saves, the map, `DisabledMapRooms` beyond the tag drop, and Remix's
  `RemoveNotVisitedRooms` branch.

**Why.** Creatures, AI maps and the rain cycle are later milestones; loading at once
is simpler and loses nothing gameplay-visible beyond the shorter waits.

**Affects.** How long a vessel waits to enter a room, which neighbour rooms are
realized ahead (and so the random draws the realizer makes), and anything creatures
would do.

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

Front end: the options, regions, arena and Remix menus, sleep and death
screens, the pause menu, slideshows, dialogs, the slugcat select menu's continue pages,
colour and Jolly Co-op controls, depth scenes, menu objects those use (sliders, check
box groups, symbol buttons, scroll boxes, `UIelementWrapper`), and in-game music (D12).
