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
- **Intro and start script.** The intro slideshows draw nothing (`SlideShowMenuScene` has
  no idle camera; its `Custom.RNV` is only in the dev move editor), nor do the process
  fades and `SU_C04StartUp`.
- **World load.** A new story game makes the game's draws in its order from
  `nextIssuedId` through the `SaveState` seed, the `World` constructor (cycle length,
  sundown, rainbow seed, `WaterLevelCycle`'s three), `GeneratePopulation` (every
  creature's constructor and AI draws, scavenger gear, vulture check rooms, the guide's
  spawn condition, the region overseers) and `FliesWorldAI`'s constructor, then the
  start room's overseers realized (`PlaceInRealizedRoom`, `Overseer`, `PlaceInRoom`,
  the graphics module's mycelia in the viewed room) before the player
  (`rainworld-re/notes/creature-model.md`). Per tick it parts from the game's again:
  `FliesWorldAI.Update`, the abstract AI of every creature but overseers (D14), and
  `LocalCoordinateOfNode`'s draws for border exits, bat hives and garbage holes when an
  overseer is realized there are not made.
- **Overseers.** A realized overseer's update makes the game's draws in order
  (`Overseer.Update`, `OverseerAI` with its communication module and the guide's
  tutorial, then `OverseerGraphics.Update` while its room is viewed;
  `rainworld-re/notes/overseer.md`), and so does each of its holograms' `Update` in its
  own slot of the room's update list; the guide's run in SU_C04 and its arrival in
  SU_A41, and a whole tutorial run from SU_C04 into the shelter SU_S01 (with and without
  food, every guide and hologram call), match an independent model draw for draw.
  Missing: the `NeuronSpark` a pair of linked
  mycelia makes (its constructor's draw is made; its `SS_Mycelia_Spark` sound on its
  first update is not), and `HologramMatrix.DrawSprites`'s per-frame draws (the lens's
  colour and line flicker) and the holograms' `DrawSprites` (line flicker, symbols, keys,
  the gamepad silhouette, the light effect), which the app takes from a cosmetic stream
  of its own: the game makes them on the global stream once per rendered frame, so their
  number follows the frame rate, not the tick.
- **Shortcut blinks.** Each realized room's `shortcutsBlinking` update makes the game's
  draws (`Room.Update`, after the water: an idle row one, or two below 0.01; a
  flickering row two) in that order. `ShortcutGraphics.Draw`'s draws (a blinking pipe's
  flicker, a flashing glow's size) come from the app's cosmetic stream, for the same
  per-frame reason as the holograms'.

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

The entrance blink is ported: `Room.shortcutsBlinking` and its update, `BlinkShortCut`
from `Creature.NewTile` (the player and realized overseers, on the tick their tile or
room changes, including the room-exit blink behind in the room left) and from the
lobby's arrival (2.5 for the slugcat, 1.5 otherwise), with `ColorEntrance` on camera 0.

**What differs:**
- **One vessel, owned by the player.** The game's `ShortcutHandler` keeps a list of
  vessels for every creature, with Jolly co-op waits and border travel; the port has
  one player whose vessel lives on `Player` or in the lobby, `wait` is always 0, and
  the handler is the three-tick divider plus that lobby.
- **Not ported:** being pulled in by a carried object (no grasps), `cantBeGrabbedCounter`,
  MSC tongue release, the arena
  hooks of `SpitOutOfShortCut` and den entry (`PlayerTryingToEnterDen`), dens and NPC
  transports for creatures, region transportation, and Remix falling into a shortcut
  (D4).

**Why.** Outside the single-player, base-game scope: no realized creature but the player
uses shortcuts yet (D14).

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
- **Room sprite shaders** (`FlatLight`, `OverseerZip`, `Hologram`). `_NoiseTex2`
  (256x256 DXT1, 9 mips, bilinear, repeat) is sampled at mip 0 only; in the room the
  game minifies it vertically (`tc.y * 8` over the 800 px level image, about 2.6 texels
  per pixel) and so reads a lower mip, giving coarser noise than the port. `Futile_White`
  (its own 16x16 white texture in the game) is drawn from the atlas's `RainWorld_White`
  element; a `Hologram` sprite of `Futile_White` would read its sideways-shifted alpha
  from the atlas around that element instead of a white texture (the overseer's
  hologram lines use `pixel`, which is in the atlas as in the game).
  `LightSource` (an overseer hologram's light) runs the base-game path only (sloped
  terrain masked black, creatures masked with a grab of the frame taken before the level
  image is drawn; the game reads the last unnamed GrabPass, which differs only if a
  projected image was drawn earlier in the Bloom container); the grab is sampled
  nearest. `HologramImage` reads its projection cell from the sprite colour as the game
  does; that the grab's y orientation and WGSL's rounding of ties match the game's HLSL
  is inferred, not checked. The projection images (`Projections/RND_PROJ`, `STR_PROJ`)
  load at startup, where the game loads them on first use.
  The Watcher ripple clip (`rippleClip`, `smoothRippleClip`, off without the `RIPPLE`
  keyword) is not ported.
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
- **Shortcut graphics** (`rw_graphics::ShortcutGraphics`, `rainworld-re/notes/shortcuts.md`
  section 10). The level view draws the game's pipe pixels (`ShortCut0`, masked to the
  level image's shortcut texels), entrance symbols and glows, the blink pulses and the
  travelling vessel in its creature's colour; the debug view keeps its vessel marker.
  Differences: the entrance pixels' `RippleBasicClipDistortion` is drawn as `Basic`
  (its Watcher ripple clip is off in the base game); the glow's `Futile_White` is the
  atlas's `RainWorld_White` (as for the room sprite shaders above); within the `Items`
  container the shortcut sprites are drawn after the overseers' (the game's order there
  follows when each was added, INFERRED irrelevant since `ShortCut0` only shows on pipe
  texels); `ShortCutColor` is known only for the slugcat and the overseer (white). Not
  ported: the arena's `DarkenExitSymbol` and `ChangeAllExitsToSheltersOrDots`, the More
  Slugcats symbols (dens, NPC and region transports, and showing them while
  controlling a creature) and the Remix gate symbol, underwater shortcuts and grayscale.
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
  (`default_slugcat_color` covers White, Yellow and Red).
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
  cap, "slugsmash") is not ported (no realized creatures to hit; D14).
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
- **Slerp is reconstructed.** `Vector3.Slerp` is native code. `rw_sim::Vec3::slerp`
  follows Unity's known implementation: lerp below magnitude 1e-5 or when nearly
  parallel, `OrthoNormalVectorFast` axis when opposite, else an axis-angle turn about
  the cross product (INFERRED: `rainworld-re/notes/overseer.md` §Slerp). Swimming
  straight down while drowning therefore turns about the x axis: the push shrinks,
  then points up, and the body rises to the surface. The game bends the swim
  direction up as air runs out (`Player` swim, decompile line 90046); the user
  checked the result by feel against the game (2026-10-15).
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
  `font-test`). The boot fade, the switches and the fades follow the game. In MCP's
  game mode (`new_game`, `load_room`) there is no process manager: the session counts
  `InitialBlackSeconds` down per tick so room scripts read `FadeDelayInProgress` as in
  the game (3 s for a new game, the Hunter 5.5 s, none for `load_room`), but no fade is
  drawn.
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
- **Unported screens.** Every process but the game, the main menu, slugcat select and
  the intro slideshows (the main menu's Regions, Arena, Remix and Options, and the pause, sleep and death
  screens) is a black screen naming the process; the pause button (Escape) returns to
  the main menu. In the game Escape still quits the app until the pause menu exists.
- **Flat scenes.** Menu scenes are shown in the game's flat mode (`flatmode.txt`: one
  `<scene> - Flat.png`; `rainWorld.flatIllustrations`) until depth scenes
  (`MenuDepthIllustration`, its shaders) are ported. The flat-mode branches follow the
  game: no slugcat select rain effect; the scenes' idle camera still runs and draws
  random numbers every tick, as in the game; the slideshows' scenes have none.
- **Intro slideshows.** `SlideShow` (`WhiteIntro`, `YellowIntro`; `rw_menu::SlideShow`,
  `rainworld-re/notes/slideshow.md`) is ported in flat mode: the playlists, the wait for
  `RW_Intro_Theme`, the manager fade, the scene alphas (Rain Drop's and Yellow A's
  `Flat2`, the title's fade-out) and the skip (the pause button, level-triggered because
  `lastPauseButton` is never written). Depth scenes (layers, camera paths along
  `camera.txt`, blur) are not. The clock follows the song's first source as the game
  follows `AudioSource.time`: kira's position of the streamed sound in the window, the
  ticks played on `TimedSources` headless (deterministic). The outros, crossfades, the
  More Slugcats and Watcher slideshows and the dev move editor are not ported.
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
  `TryGetPlayerStartPos` (D14). The Survivor and the Monk see their intro slideshow
  first and the Hunter none (`StartGame`); S, or the map button (action 11), held as
  START completes skips it. The map button is read as Space, Rewired's default gameplay
  keyboard binding; that the gameplay map is active in menus is inferred, not checked.
  Space also submits in menus, so filling START with Space skips the intro. The game
  then waits `InitialBlackSeconds` in black (3 s, the Hunter 5.5 s) and fades in over
  `FadeInTime` (2 s); saved games' `SlowFadeIn` is not reached. The save is not wiped
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
  yet, but for Space skipping the intro when START completes. The intro roll's "any button" (`ControlSetup.GetAnyButton`) is any key of the
  default maps or any mouse button; Rewired's handling of keys bound in no map is not
  checked (the port knows no such keys).
- **Frame time.** `Time.deltaTime` is capped at 1/3 s, Unity's default
  `maximumDeltaTime`; the game's own setting is not checked. (The game previously ran on
  a fixed timestep with a 250 ms cap; it now runs under `MainLoopProcess.RawUpdate`:
  ticks only while more than one is accumulated, at most three per frame.)
- **Random stream (D1).** One stream, seeded once from `--seed`, from the first menu to
  the game.

**Why.** The menus are ported screen by screen, with no saves or options yet; the
world with only overseers behaving (D14) stands in for a story session.

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
- **Only overseers behave.** The `CREATURES` section is read in full (simple spawners
  and lineages with their dens, amounts, spawn data and night flags; quantified
  counts; `rw_assets::creatures`), and a new game's `GeneratePopulation` builds every
  `AbstractCreature` with all of its constructor draws (D1): the creatures go into
  their dens and stay there (`InDenUpdate` lets only overseers out), quantified
  creatures (flies, the swarm rooms' bat flies) are only counted per node, and items
  (scavenger gear, drop bugs' items) are kept as IDs and kinds on their carrier. The
  overseers' abstract AI is ported (`OverseerAbstractAI.AbstractBehavior`, `Roam`,
  `PlayerGuideUpdate`, `RoomAllowed`, `HowInterestingIsCreature`), run by
  `AbstractRoom.Update` and `InDenUpdate`; it never picks a target, as nothing it
  finds interesting is ever out of a den. A realized overseer is the game's
  (`rainworld-re/notes/overseer.md`): `Overseer` with its zipping, hovering, root and
  appendage, `OverseerAI` with the AI-map parts it reads (`ai-map.md`), the communication
  module and the guide's `OverseerTutorialBehavior` (messages through the HUD's text
  prompt, `pauseRain`, `bringInRain`), its graphics module in a viewed room, and
  `ChangeRooms`'s `CreatureEnterFromAbstractRoom` with the lobby's teleportation
  vessels and `TeleportingIntoRoom`, and its holograms (`TryAddHologram` with every
  base-game hologram, `Projecting`, the tutorial's input instructions and pointers, the
  communication module's pointers, images and anger). Holograms that cannot appear yet,
  for want of what triggers them: the creature and food pointers (no dangerous creature
  or food item is realized), the angry hologram (`angryWithPlayer` is never set: no
  weapons), the gate scene (`RegionGate` is not ported: in Moon's gates the concern is
  `None`, as the game's is with a closed gate), the placed scenes
  (`ShowProjectedImageEvent` triggers are not ported, and an image's
  `ProjectedImagePosition` placed object is not read), the forced direction
  (`ReliableIggyDirection` is not ported; the port makes no pointer for it), the
  in-world super jump, pick-up and scavenger-trade instructions (their placed triggers are
  not ported), and the tutorial's Eat instruction (it needs edible objects in SU_A42 or
  SU_A44). A door pointer without the tutorial would follow the player with a
  `CreatureDoorDistanceTracker`, which needs the fly's exit-distance AI maps: it is made
  but never updates, so the pointer keeps pointing. The bat pointer's `CycleStartUp` is
  1 (the cycle's start-up ticks are not ported), and a new game has discovered no
  shelters and visited no rooms. The controls picture is always the keyboard's (the
  gamepad and Switch pictures are ported but no other control preset exists). A pointer
  whose overseer has left the room goes on updating where the game's would throw (shelter
  and direction pointers without `FORCEPOINT`): the port logs and stops there. Food can
  only be set by a tool (`set_player`'s `food`): nothing is eaten. Missing as well: every
  creature but the player and overseers in what the AI looks at. Which camera's room
  counts as viewed when `AddObject` builds the graphics module is inferred from
  `RoomCamera` and `Room.AddObject`, not checked.
  `OverseersWorldAI` is ported (its shelter finder and the guide's direction finder,
  on the room files' baked node connectivity), as are the flies' swarm room map (with
  the world file's bat migration blockages), `GetSwarmRoomDistance`, `ActiveSwarmRoom`
  and a new game's `RegionState` swarm counters. Nothing records `roomsVisited` (the
  caller fills the region state's set; a new game's is empty), and only a new game's
  region state exists. Remix's `BringToRoomAndGuidePlayer` branch of the world
  process's update and More Slugcats' goal rooms and guide symbols are left out.
  Not ported: `FliesWorldAI.Update`, scavenger and vulture world AI,
  `CreatureCommunities`, `AbstractCreature.Update` for other types, and
  `GeneratePopulation`'s non-fresh branch (saved populations).
- **Hologram graphics.** A hologram's colour base (`Color.Lerp(MainColor,
  ColorOfSegment(0, 0), 0.5)`) is taken at its overseer's last update (holograms update
  before their overseer, so it is the one the game reads); a hologram slated for removal
  is not drawn on its last frame, where the game draws it once more. Not drawn: the
  PS4/PS5 controller symbol (`ps*controllertiny` is not in the two atlases loaded,
  rainWorld and uiSprites), and More Slugcats' projections and parts.
- **Overseer graphics.** `OverseerGraphics.Emerge` restarts the mycelia from the eye's
  position of the last rendered frame in the game (`DrawSprites` caches it); the port
  uses the last update's, as a headless run would. Sprites are drawn per container in
  the game's container order, but within the Foreground container the overseers' come
  after the player's; the game orders them by when the objects joined the room (the
  guide, realized first, sits under the player). `NowViewed` builds graphics modules in
  the abstract room's creature order, not in `physicalObjects`' (the order the bodies
  were added); they differ only with several overseers in the room.
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
  bat hives or garbage holes, and no connectivity between them (every path -1).
- **Off-screen den and gates.** The off-screen den is never realized; gates are
  ordinary rooms (no gate mechanism, no region change), and a `RegionTransportation`
  node goes nowhere.
- **Not ported:** the rain (the cycle's timer, day-night counter and water level stages
  run; the overseers' AI and the HUD's rain meter read them, and the guide's tutorial
  pins the timer), karma and karma gates, shelters closing and
  hibernation, saves, the map, `DisabledMapRooms` beyond the tag drop, and Remix's
  `RemoveNotVisitedRooms` branch.
- **Room scripts.** `AddRoomSpecificScript` is `rw_sim::RoomScript::for_room`, called
  when a room with `RoomSpecificScriptActive` loads; only `SU_C04StartUp` is ported (the
  new Survivor or Monk held in the tunnel, pushed out, standing up). It updates after
  the player, as the game's reversed update list puts it. The
  `TutorialControlsPageOwner` it makes 100 ticks after the slugcat stands is the app's
  (`rw_hud::ControlsPage`, D15): the script raises `showed_controls` when the HUD is
  there and `controls_page_continue` once the slugcat is past x 800, and the app opens
  and updates the page from those, first updating it on the tick after (the owner joins
  the end of the update list, which runs backwards). It leaves out Jolly Co-op's players
  1-3 (pushed from tick `5 * n`). The Hunter's `HardmodeStart` in `LF_H01` (the scripted
  first cycle with its creatures and spear) is not ported, so the Hunter starts in its
  den with nothing happening; the registry keeps its place. The other base-game scripts
  (`SS_E08`, `SU_A22`, `SL_C12`, `SB_A14`, `SB_D03`, `SU_A43`, `deathPit`, `LF_A03`,
  `SS_D02`) are not ported.

**Why.** Other creatures' AI, AI maps and the rain are later milestones; loading at once
is simpler and loses nothing gameplay-visible beyond the shorter waits.

**Affects.** How long a vessel waits to enter a room, which neighbour rooms are
realized ahead (and so the random draws the realizer makes), and anything creatures
would do.

## D15: The in-game HUD

`rw_hud` ports `HUD.HUD` as `InitSinglePlayerHud` builds it for a story game
(`rainworld-re/notes/hud.md`): the text prompt with its subregion tracker, the food
meter, the karma meter, the rain meter and the tutorial controls page of SU_C04. It
fires up in the camera update once the player is realized in a region world (not for
single-room worlds), updates its parts in the game's reverse order and draws into its
own Futile stage above the room. The map button is `Space` (the default keyboard
binding); holding it stands the slugcat still, as `standStillOnMapButton` does in a
story session.

**What differs:**
- **No region map (user decision).** `HUD.Map` is not drawn and cannot be panned. What
  is kept is its fade (the karma meter's target and the controls page read it) and the
  `Random.value` draw its update makes every tick, so the shared stream stays in step.
  The swarm-circle draws it would make on top (only with the map shown over a region
  with swarm rooms) are counted, not made. Whether building the map's `MapData` draws
  from the stream is not checked.
- **Rain.** The rain meter reads the world's `RainCycle` (its drawn length and
  ticking timer, which the guide's `pauseRain` pins at 2000 in the tutorial rooms; D14).
  The rain itself is not ported, so nothing reads a room's rain `DangerType`.
- **New-game state only.** Food is 0 and stays 0 (no eating), karma and cap are the
  campaign's new-game values (karma 0, cap 4; the Hunter karma 2), never reinforced, and
  the cycle number is the session's (0 in a new game, so the meters start hidden). No
  saves feed it.
- **Shelters.** Every shelter counts as intact (broken shelters are not read), and the
  owner is never `stillInStartShelter` or `readyForWin`, so in any shelter the meters
  show and the hibernation line warns. `forceSleepCounter` is always 0.
- **Game over.** The prompt's game-over mode is ported but not entered: death freezes
  the body until R (current phase), so nothing calls `EnterGameOverMode`.
- **Not ported:** the pause menu and its text, symbol and music messages, the Hunter's
  cycle count-up, the karma meter's gate and scavenger blinking, the gamepad pictures of
  the controls page, the sleep, death and character-select screens' meters, and every
  DLC part (More Slugcats pup meters, ammo meter, Watcher ripple symbols, Jolly Co-op
  meters, Remix's options).

**Why.** The user decided against the map for now; the rain cycle, eating, saves and
shelters are later milestones.

**Affects.** What the meters show (food, rain) and when they show in shelters; no
gameplay beyond the map button standing the slugcat still.

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
screens, the pause menu, the outro and DLC slideshows, dialogs, the slugcat select menu's continue pages,
colour and Jolly Co-op controls, depth scenes, menu objects those use (sliders, check
box groups, symbol buttons, scroll boxes, `UIelementWrapper`), and in-game music (D12).
