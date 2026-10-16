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
does: the intro roll's and the slugcat select menu's `RainEffect` (2508 draws when
built: five per drop, eight for the wind; every tick two per drop, five more per drop
that resets, eight for the wind, one when the highlight wraps, one or two for the
lightning), the intro roll's random lightning past 14 s (one draw per tick, three more
on a spike), the select pages' mark (three or five draws a tick) and glow (two or
three; `SlugcatPage.Update`), the scenes' camera drift and
focus (`InteractiveMenuScene`, every tick of a shown scene; a depth scene's idle focus
pick two more) and the menu sounds (`MenuMicrophone.PlaySound`/`PlayLoop` make the same
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
  the same functions (the rain-warning look-up, `Room.Update`'s ambient ceiling drips,
  the `WaterDrip`s of slides and corridor shoots, grasps of objects other than bat flies,
  DLC, and the sounds of those) are missing, as is the draw-skipping of
  `AllowSound`'s crowd cap (D12); the rain-warning look-up short-circuits before drawing
  while rain is far off, as at the start of a cycle.
- **Spawn path.** A new story starts as the game does (start den, `TryGetPlayerStartPos`,
  the node drawn in `RealizeInRoom`). In the game only Remix key-item tracking and
  Watcher warps call `DetermineSafeSpawnTile`; the port's dev starts (`--room` without
  `--spawn`, MCP `load_room`, R) use it to place the player, so these draws are the
  port's choice of game function, made in the order the game makes them. The dev
  teleport (MCP `set_player`) first releases what the player holds.
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
  (`rainworld-re/notes/creature-model.md`). Every tick `FliesWorldAI.Update` makes its
  draws among the world processes, and a room that finishes loading with quantified
  flies places and realizes them with `PlaceQuantifiedCreaturesInRoom`'s draws. A
  realized fly makes the game's draws in its slot of the room's update list (the `Fly`,
  `FlyAI` and `FlockBehavior` constructors, `PlaceInRoom`, `Fly.Update` with `FlyAI`,
  then `FlyGraphics.Update` while its room is viewed), and so do `FliesRoomAI`, the
  hover-loop emitters, the flies' shortcut vessels and lobby turns, `MoveFly`'s pick of
  a realized fly, the sounds the flies play, and the `WaterDrip`s a hard terrain impact
  of a fly or the player throws in a room with `CeilingDrips`. The player catching,
  holding, biting, eating, throwing and carrying flies through pipes makes the game's
  draws (`Player.Update`'s catch, `GrabUpdate`, `BiteEdibleObject`'s drips, the
  `PlayerObjectLooker` and `SlugcatHand` draws of `PlayerGraphics.Update`), and so do
  picking up, laying down, swallowing and regurgitating rocks and spears (the pick-up
  candidate's `Blink` sound, `PickedUp`'s and `ReleaseObject`'s sounds, `Regurgitate`'s
  22 draws and sound, 2 more with graphics, the `SlugcatHand` swallow shake's 6 and
  `PlayerGraphics`' swallow blink `Range(-5, 8)`). Every base-game creature with an
  abstract AI but the scavengers makes its draws in its room's update: the plain
  `AbstractCreatureAI`'s (`MigrationBehavior`'s laziness `Range` and time-influenced
  rolls, the roam roll, `RandomMoveWithinRoom`, `RandomMoveToOtherRoom`'s two per step,
  `MoveToMoreAttractiveNeighborRoom`'s, `FollowPath`'s stuck roll,
  `DefineNodeOfLocalCoordinate` and `AbstractSpacePathFinder`'s node picks) and the
  subclasses' own (the vultures' `AddRandomCheckRoom`, the cicadas' catch rolls, a caught
  fly's `GetNewID` and constructor draws and the swarm-room search's `Random` flood, the
  big eels' and deer's check rooms, the Miros birds' raids, the needle worms' mother
  search and migrate-active counter): from the first creature out of its den (100 to
  1000 ticks into a cycle) every story run's stream has shifted from before, as the
  game's does. Missing: the draws a creature would make on being realized
  (`PlaceInRealizedRoom`, its body, AI and graphics), as it stays abstract (D14), and
  per tick the scavengers' abstract AI and `ScavengersWorldAI` (one `Range` over the
  scavengers every tick once a world has any), which are not ported (D14).
  `LocalCoordinateOfNode` draws for bat hives, garbage holes and border exits; a border
  exit's tile is not known (its border tiles are not mapped), only its draw is made.
- **Room items.** A room realized for the first time in a cycle makes its items' draws
  where the game makes them, in `RealizeRoom` (`rainworld-re/notes/items.md`): one
  `value` per entry of the room file's item list, then `Room.Loaded`'s random items, two
  `Range` draws per attempt (`(int)(W * H * density^2 / 5) + 1` attempts, so at least
  one even at density 0; none in shelters and gates), one more on a free tile and one
  more for a spear, each item taking an ID (placed spears and multiplayer items: below).
  This shifted the stream of every room
  realization since; no test pinned the old one. The rest of `Room.Loaded`'s draws
  (placed objects and effects the port does not make) are still missing.
- **Triggers and music.** An `ActiveTriggerChecker` firing its event draws one
  `Random.value` when its `fireChance` is not 1 (SU_B04's song, 0.4565), in its slot of
  the room's update list; the guide's `BringGuideToPlayerEvent` and the image and
  pickup instructions make the overseer's draws. The music player, updated after the
  game each tick as the game's side process, draws `Random.Range(80, 400)` when a room
  switch's delay ends. Missing: `ProceduralMusic.Reset`'s stem choice
  (`Random.Range` per non-empty threat layer, 5 for SU, made when `NewRegion` builds the
  bank: in the first room of a game, and after a bank change once the old one faded;
  music step 2).
- **Realized rocks and spears.** Realizing a room's rocks and spears (`ShortCutsReady`,
  in entity order) makes the game's draws: the `Weapon` constructor's rotation and the
  spear's needle `Range(0, 3)`, then `PlaceInRoom`'s `SetRandomSpin` (2 for a rock, 4
  for a spear in all). Each tick, in its slot of the room's update list,
  `PlayerCarryableItem.Update` draws 2 to reset an item found inside terrain, a free
  spear landing point first draws 1 and plays `Spear_Stick_In_Ground`, and
  `Room.Update`'s collision pass draws 2 to nudge two items apart at the same x.
  `Rock.DrawSprites` / `Spear.DrawSprites`' vibrate (2) and blink (1) draws come from
  the app's cosmetic stream, for the same per-frame reason as the holograms' (below).
- **Thrown rocks and spears.** A throw makes the game's draws: `Weapon.Thrown` and
  `ChangeMode` draw nothing, the throw sound draws its 7, `ThrownSpear` draws once for
  a slugcat without throwing skill (the Monk's or a starving slugcat's damage bonus),
  and a belly slide's backward throw adds `ExplosionSpikes` (2 and 4 per spike) and
  `Slugcat_Rocket_Jump`. In flight, a turn back of the thrower draws 1, the tumble to
  `Free` its `SetRandomSpin`, a `HitWall` 4 per spark (seven, in the viewed room) and
  its sound, a floor or slope skim 3 or 4 per spark and `Weapon_Skid`, a spear's stick
  roll 1 (when the hit is in reach and not close) and `Spear_Stick_In_Wall`; a parried
  weapon's `WeaponDeflect` its spin and sparks. Each `Spark` and `ExplosionSpikes`
  updates in its own slot of the room's update list (`rw_sim::cosmetic`).
- **Placed spears and items.** `Room.Loaded`'s story `MultiplayerItem`s draw once
  against their chance (on every realization) and take an ID, then on a first
  realization each `ReliableSpear` takes an ID (no draw), before the random items.
- **Overseers.** A realized overseer's update makes the game's draws in order
  (`Overseer.Update`, `OverseerAI` with its communication module and the guide's
  tutorial, then `OverseerGraphics.Update` while its room is viewed;
  `rainworld-re/notes/overseer.md`), and so does each of its holograms' `Update` in its
  own slot of the room's update list.
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
- **Passage tokens.** `EndgameTokens.Token.GrafUpdate`'s glow-size draw (one per token
  and rendered frame on the sleep and death screens) comes from the screen's own
  cosmetic stream, for the same reason, and so do `SlugcatPage.GrafUpdate`'s draws on
  the slugcat select menu (the mark glow's size and glow B's alpha, one each per page
  that has them and rendered frame).

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
`horizontal-beams.md`, `slope-collision.md` section 5) with the session's
`SlugcatStats` (`characterStats`: run, pole and corridor speeds, body weight and lungs of
the slugcat, the malnourished values in a malnourished cycle until a full stomach cures
it), no adrenaline. `aerobicLevel` and the malnourished exhaustion are ported
(`AerobicIncrease` from jumps, walking steps, beam and pole climbing, rolls and throws;
the exhausted slowness, random `Stun(7)` and falls, the weaker wall jump, the
recovery). Stun and death are D4; vertical
poles and ledge grabs are D6; skid, flip, roll, belly slide and rocket jump are D9;
corridors are D10.

Grabbing is ported for bat flies, rocks and spears (`rainworld-re/notes/items.md`
section 10): `Grabability`, `CanIPickThisUp`, `PickupCandidate` and the item `Blink`,
picking up and laying down, switching hands, swallowing and regurgitating a rock, and
carrying held objects through shortcuts (D5). The carried-object speed factors all
apply to `HeavyCarry` objects only, which no ported object is.

**What differs from the game where the game's behaviour is kept out of scope:**
- **Throwing rocks and spears** is ported (`ThrowObject`'s weapon branch, `Weapon.Thrown`,
  `ThrownSpear` with the slugcats' throwing skills, the belly-slide throws, the
  recoil, `Blink(15)`, `PlayerGraphics.ThrowObject`'s throwing hand; flight, wall hits,
  skims, sticking: `rainworld-re/notes/items.md` section 11). Hitting creatures is
  not: a thrown weapon passes through every creature (`HitSomething`,
  `WeaponAttack`), and nothing hears it fly (`INotifyOfFlyingWeapons`). Adrenaline's
  throw force (`Lerp(1, 1.5, Adrenaline)`) is 1. Remix and More Slugcats branches
  (Gourmand, Saint, Spearmaster, Rivulet's upward throw, the MMF slow-spear and
  climbing-grip rules) are omitted.
- **The Hunter's spear on the back** (`SpearOnBack`, `CanPutSpearToBack`) is not
  ported: a Hunter picks spears up into the hands only.
- **Random numbers.** The player's draws (stacked-body nudge `Random.Range(-1, 1)`,
  initial facing in `Player::create`, the `Stun` threshold, the corridor-turn jiggle,
  the water) use the game's generator and calls; only seed and call order differ (D1).
  `Player::new` (tests) faces right.
- **Input.** `Input` carries x, y, jump, pick-up and throw (and the map button, D15);
  `downDiagonal` is derived digitally (down plus left/right gives that side, the Remix
  rule). No analogue stick or special. The game's `checkInput` substitution
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
is gone (`Player::is_destroyed`; the app drops it and draws nothing). The first `Die`
calls `GameOver` (story sessions; D15, D17); the dead body keeps simulating as plain
physics with neutral input, its water, graphics and room camera as in the game.

**What differs:**
- **R respawn and no game over in dev starts** (user decision). The port's starts in a
  room (`load_room`, `--room`, arena levels: `StartCondition::Dev`) are not story
  sessions: a death there has no game over, and R (MCP `respawn`) replaces the player
  with a fresh one in the camera's room, also after the body fell out of the room (the
  game has no respawn). In a story session (a new or continued campaign) R does
  nothing. Not ported: `setupValues.invincibility` (a dev setting) and `Die` without a
  realized room (unreachable in the port).
- **`Player.Stun` side effects.** The game un-stands the player (and drops grasps) when
  the stun exceeds `Random.Range(5, 12)` and draws `Random.Range(40, 80)` for the spear
  on the back; the port makes both draws on every path (no spear to drop) and plays
  `UI_Slugcat_Stunned_Init`. The malnourished scaling (1.5 times, twice exhausted,
  `Mathf.RoundToInt`) is ported; grasps, slugpup drops and the face blink are not.
- **Remix (MMF) branches omitted:** falling into a room-exit or normal shortcut entrance
  at speed > 5 (enters the shortcut instead of taking damage) and the wall pounce (D9).
- **More Slugcats omitted:** Gourmand's thresholds (death above 80, stun above 40
  with the 40..140 ticks mapped over speeds 40..80), tongue attachment and being carried by another player blocking fall damage,
  Sofanthiel's sound. Base-game branches with no ported counterpart: the Pomegranate
  grasp and the deer-antlers ride cancelling fall damage.
- **Water.** Drowning is D11; inverted water (DLC) is not modelled.

**Why.** The respawn is a development aid the user kept outside story sessions; the
rest is out of the base-game scope or randomness (D1).

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

Creatures travel the tubes too (bat flies so far): `transportVessels` holds the
player's vessel and the creatures' in the game's order, `SuckInCreature` with
`Small_NPC_Enter_Shortcut`, each step's `Small_NPC_Tick_Along_In_Shortcut`, the
spit-out with every stuck object, the room-exit node into the lobby and the lobby's
`CreatureEnterFromAbstractRoom` (abstracted toward an unrealized room, restarted at the
entrance node once the room is ready for the player and for AI), and the bat hive
vessels. `ShortcutGraphics` draws every vessel in the room in its creature's colour.

**What differs:**
- **The player's vessel lives on `Player`.** The game's vessel list holds a plain
  `ShortCutVessel` for the player too; the port keeps the player's on `Player` (with
  its slot in the list) or in the lobby, and its `wait` is always 0 (Jolly co-op).
- **Not ported:** being pulled in by a grabber entering a shortcut (no ported
  creature grabs the player; what the player holds goes through with it),
  `cantBeGrabbedCounter`, MSC tongue release, the arena hooks of `SpitOutOfShortCut` and
  den entry (`PlayerTryingToEnterDen`), dens for creatures other than flies (a fly's
  `OpportunityToEnterDen` never takes it in), NPC transports
  (`NPCTransportationDestination`), border travel, region transportation, and Remix
  falling into a shortcut (D4).

**Why.** Outside the single-player, base-game scope.

**Affects.** Co-op, arena, creatures that use dens and NPC transports.

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
  (`cfgClimbingGrip`), the vine grab's exclusion of
  `ClimbOnBeam` (vines are not ported). The corridor exit onto a pole is in D10.
- **More Slugcats / Watcher omitted:** the slugpup's smaller slide-up hop (2.25 px,
  +1 vy, 0.3x lift, 0.5x slide forces) and the slugpup, Rivulet and Gourmand
  (`PainJumps`) sideways pole jump values, Saint's `monkAscension` blocking the grab,
  and the ripple-levitation freeze of the facing (`rippleActivating`).
- **Not ported entries to the pole:** `ZeroGPoleGrab` (zero gravity) and the deer antlers. The blink
  on the pole is not ported.
- **Spears stuck in walls** make poles as `Spear.addPoles` does (a horizontal beam on the
  spear's tile and its non-solid neighbours along it, or vertical poles for a spear in a floor or
  ceiling, once the room is ready for AI) and `resetHorizontalBeamState` clears them.
  The port writes them into the realized room's own copy of its tiles (as the game
  writes `Room.Tile`), so every tile read sees them; the copy is dropped with the
  realized room, and a re-realized room gets them again from its stuck spears.

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
- **Half-pixel camera offset.** The port always draws as the game's D3D build at
  render scale 1 (Windows, also under Proton): Futile's `screenPixelOffset`, screen =
  stage + (-0.5, +0.5) (`Camera::SCREEN_PIXEL_OFFSET`). The game's OpenGL path (no
  offset) and its render-scale > 1 offset (0, 1) are not modelled
  (rainworld-re/notes/sprite-atlases.md section 3).
- **Camera switch timing.** The game loads the next camera's image asynchronously and
  cuts when it has loaded (usually the next frame); the port decodes every camera's
  image when the camera enters the room (`MoveCamera`, synchronous: D14) and cuts in
  the same tick.
- **Shortcut graphics** (`rw_graphics::ShortcutGraphics`, `rainworld-re/notes/shortcuts.md`
  section 10). The level view draws the game's pipe pixels (`ShortCut0`, masked to the
  level image's shortcut texels), entrance symbols and glows, the blink pulses and the
  travelling vessels in their creatures' colours; the debug view keeps the player's
  vessel marker.
  Differences: the entrance pixels' `RippleBasicClipDistortion` is drawn as `Basic`
  (its Watcher ripple clip is off in the base game); the glow's `Futile_White` is the
  atlas's `RainWorld_White` (as for the room sprite shaders above); within the `Items`
  container the shortcut sprites are drawn after the overseers' (the game's order there
  follows when each was added, INFERRED irrelevant since `ShortCut0` only shows on pipe
  texels); `ShortCutColor` is known only for the slugcat, the overseer and the fly (both
  white), and `shortcutSegments` only for the slugcat (2) and the fly (1). Not
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
  rot / mushroom palettes, screen shake (and the camera bump of a weapon hitting a
  wall, `ScreenMovement`), `_bkg` backgrounds, the heat / melt / ripple
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
- **Breathing, exhaustion and malnourishment** are ported: breathing quickens and the
  body pants with `aerobicLevel`; the exhausted eyes and head bob; the malnourished
  slugcat's thinner body, hips and tail, grey body and lighter eyes (toward the
  palette's fog colour), fading by 0.005 a tick once cured. Red's illness (also
  `malnourished` 1) is not ported.
- **Looking and holding.** `PlayerObjectLooker` is ported whole and looks at the
  room's bat flies, overseers, rocks and spears; the hands hold bat flies, rocks and
  spears (a spear swung forward while walking, `spearDir`, with `ChangeOverlap`; the
  throwing hand of `ThrowObject`), reach
  for and bite bat flies and bring a rock to the mouth to swallow it (`SlugcatHand`
  grasp branch, `BiteFly`, `LookAtObject`); the swallow and regurgitation gulp, blink
  and heave of `PlayerGraphics.Update` are ported. The rain-warning look-up and
  the afraid-of-creature poses (and their blink and aerobic effects) are not ported.
- **Not ported:** the spear on the back, heavy carry, two-handed objects, mauling and
  its `Blink`s,
  the map-button stand-still blink,
  zero gravity (`ZeroGPoleGrab`, `ZeroGSwim`), antler and vine climbing,
  adrenaline jitter, hypothermia and poison tints, the Mark (sprites 10 and 11
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
- **Held items.** The belly-slide throws are ported: forward (with throwing skill) the
  weapon skims the floor; backward the long belly slide (`longBellySlide`: 39-tick
  push of 14, 34-tick jump window, exit after 16, a hop on its end, a 1.2x rocket
  jump, a longer stun on a creature) starts with its spikes. Picking up an item
  during the whiplash jump is not ported.
- **Other creatures:** a belly slide or roll hitting a creature (stun, `rollCounter`
  cap, "slugsmash") is not ported (no realized creatures to hit; D14).
- **More Slugcats / Watcher omitted:** Rivulet, slugpup and Gourmand values (skid and
  flip jump, roll jump, belly-slide push, kick and jumps, roll durations), the pyro
  jump's `Flip`, and the Watcher curved-terrain drop-through test.
- **Not ported:** adrenaline factors.

**Why.** Remix and DLC are out of the base-game scope; the rest waits for
creatures and adrenaline.

**Affects.** Remix wall pounces, picking up during a whiplash jump, slides and rolls
into creatures, DLC slugcats.

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
  flooding, lethal water, splashes and bubbles, ripple waves from explosions, the
  analogue swim direction (digital input only).
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
`Player` sound, the shortcut tick, the water-surface splashes and the bat flies' sounds
(their wings, idle and afraid calls, the hover loop, burrowing, emerging, chaining and
the NPC shortcut sounds) are ported. The menus'
`MenuMicrophone` is `rw_menu::MenuMic` (what plays, the draws, the loops' fades) and
`rw_audio::MenuPlayback` (the sources), with the options' sound-effects volume (D13). `rw_audio::music` is the `MusicPlayer` side process with `Song`, `MenuOrSlideShowSong`
and `IntroRollMusic` (`music.md` section 4.1), streaming `music_songs` clips; the app
runs it as the game's side process (D13), silent on `TimedSources` without audio
(`--mute`, `--screenshot`, MCP). In a game, the sim queues the music player's requests
(`rw_sim::trigger::MusicRequest`: a room trigger's `MusicEvent` or `StopMusicEvent`,
`RainCycle`'s `RainRequestStopSong`), the app hands them to `GameRequestsSong` /
`GameRequestsSongStop` / `RainRequestStopSong` with the save state's `songsPlayRecords`
(D16), and the `PlayerThreatTracker`'s room switches, drone level and
`Random.Range(80, 400)` delays run from the player's room (`rw_audio::music::GameMusicView`).
`NewCycleEvent` is sent when a story game starts (D17) and `DeathEvent` on death.

**What differs:**
- **Music not ported:** `ThreatDetermination.Update` (the threat of the creatures in
  the player's room: the threat stays 0, so no song is refused or faded by threat),
  `ProceduralMusic` (`NewRegion` only records the bank in `nextProcedural`; its
  `Reset`'s stem draws are missing, D1), the screen music (sleep, death, starve) and
  the starve and Hunter fades, the echoes' `GhostSong` and ghost mode, `SSSong`,
  gate and region-change songs (no gates yet), `VoidSeaMusic`, the ending songs, the
  arena `MultiplayerDJ` (its shuffle draws), the DLC songs and their branches
  (rot presence, the Watcher's spinning tops, Challenge threat music),
  `ResetSongStream`, MMF stream resync, the pitch trim and loose mod `.ogg`
  overrides. The ported in-game paths draw what the game draws (D1).
- **Music timing.** A sub-track's clip loads on a background thread and becomes
  ready on the first update after it arrived (the game's `AssetBundle` load and
  `LoadAudioData` take a frame or more, so this is at least as fast); headless runs use
  `TimedSources`, where clips load at once and play for their FSB5 length in ticks.
  `IntroRollMusic.StartMusic` takes effect in the music player's next update (in the
  game it starts the source at once; the player runs right after the main process in
  the same tick, so nothing reads the difference). A missing `music_songs` bundle still
  sets `assetBundlesLoaded` (the game would wait forever) and its songs stay silent.
- **Fly one-shots stay where they started.** The game attaches a fly's one-shots to its
  body chunk (`PlaySound(id, chunk)`), so they move with it; the port plays them at the
  chunk's position on the tick they start (the hover loop does follow its fly). The
  draws are the game's; only what is heard differs.
- **Item sounds.** A spear sticking into the ground plays `Spear_Stick_In_Ground`; a
  skidding rock runs `Rock_Skidding_On_Ground_LOOP` as a `ChunkDynamicSoundLoop` (its
  loop is made only in a room a camera shows, as the game's `DynamicSoundLoop` makes
  one only with a microphone in the room; a stopped loop stops at once, where the
  game fades it out over its last update). The `Through_Air` loops are commented out
  of the game's `sounds.txt` and never play. Throws play `Slugcat_Throw_Rock` /
  `Slugcat_Throw_Spear`, wall hits `Rock_Hit_Wall` / `Spear_Bounce_Off_Wall`, sticking
  `Spear_Stick_In_Wall`, skims `Weapon_Skid`, the backward belly-slide throw
  `Slugcat_Rocket_Jump`, the long slide's end `Slugcat_Normal_Jump`. Creature hits'
  sounds come with them.
- **Not ported:** the rain and thunder loops and one-shots and the
  drown-out of volume group 0, other creatures', item and object sounds, the sounds of
  unported menu screens and objects, `PlayCustomSound` / loose mod clips next to
  `sounds.txt` (a name found only there counts
  as present for the draws but is not decoded), the Watcher ripple ambience and DLC
  sound IDs. Deafness, fade to black and `globalSoundMuffle` stay 0; time speed is 1.
- **`AllowSound`'s crowd cap is applied by the audio, after the sim made the draws.** In
  the game a refused sound makes no draws after `silentChance`'s; the cap depends on how
  many sounds the audio thread still plays, which the tick-deterministic sim does not
  know. With audio off nothing is capped. PLAYALL triggers are capped per sample, not
  once per call.
- **Music volume and options.** The options' music volume (D13), read every frame;
  `playMusic` is on (`setup.txt` is not read).
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
- **Static loops on return.** A `StaticSoundLoop` whose room the camera re-enters
  restarts on the first tick back; the game's dead emitter takes up to about 3 ticks
  (`SoundEmitter.Update`) before the loop remakes it.

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
  ported. `RainEffect`'s light source uses `FlatLightNoisy`
  with `_spriteRect` unset (0, 0, 0, 0), as in the menus at boot: the noise lookup
  divides by zero in both, and what the GPU then samples is whatever the driver does
  with non-finite coordinates (the effect on the light's falloff is small).
- **Port-only process.** `ProcessId::FontTest` (labels in both fonts over the flat
  main-menu image) is not a game screen; it exists to compare drawing by eye. Its
  labels sit on whole pixels, where `font`'s glyphs lose a pixel row to point sampling;
  the game's menus place labels a third of a pixel off (the page sits at (1/3, 1/3)),
  and the port's menus draw them whole.
- **Unported screens.** Every process but the game, the main menu, the options menu,
  slugcat select and the intro slideshows (the main menu's Regions, Arena and Remix,
  the options menu's Input Settings, Credits and Backups) is a black screen naming the
  process; the pause button (Escape) returns to the main menu. The dream screen is a
  black stand-in (D17).
- **Sleep, death and starve screens.** `SleepAndDeathScreen` on `KarmaLadderScreen`
  (`rw_hud::SleepAndDeathScreen`, `rainworld-re/notes/menu.md` "Sleep and death
  screens") for the `SleepScreen`, `DeathScreen` and `StarveScreen` processes: the depth
  scene, the full `KarmaLadder` (its phases, symbols, fade circles, reinforcement, sounds
  and every random draw; the selection glow's `LocalBloom` GrabPass blur reads the
  port's grab texture), the screen HUD's food meter and region map (D15), CONTINUE
  (`StoryGameInitCondition.Load`; after a death screen the cycle starts 340 ticks in)
  and EXIT (main menu), the info texts, the starving warning, the watched-screen
  counters and their forced watch, the map toggle and the illustration's fade. The
  end-game meters (`EndgameMeter`: float and notch meters for every base tracker kind,
  the power-on flicker, the fill animation, the fulfilment, their sounds and draws), the
  tokens (`EndgameTokens`), the `endGameMetersEverShown` save (`METERSSHOWN`) and the
  watch a passage fulfilled on the sleep screen forces are ported.
  - **PASSAGE is disabled (user decision).** The button appears where and when the game
    adds it (a token of a passage fulfilled before the screen, or once a passage
    fulfilled on it shows), fading in from black the same way, but stays greyed out:
    `CustomEndGameScreen`, `FastTravelScreen` and `ApplyCustomEndGame` are not ported,
    so `EndgameTokens.Passage` (the token's activation and the black fade) never runs
    and passages are never consumed. Passages are still achieved: the trackers progress
    and fulfil wherever their triggers are ported ("Not ported yet").
  - `CueAchievement` on a fulfilment is not made (the port has no achievements).
  - The tokens' glow size draws `Random.value` once per token and rendered frame; the
    port takes it from the screen's own cosmetic stream (D1).
  - Not ported: the kills display (`SleepScreenKills`: the port records no kills; the
    only creature it can kill, the bat fly, is quantified and earns no trophy, so the
    game's display would show nothing for it), the statistics screen's `ScoreFlash`,
    the Hunter's `GoToRedsGameOver` (D17), and the Expedition, Watcher and More
    Slugcats branches (Remix's Traveller region colours and list-change animation, the
    collectibles tracker).
  - The game's `PostSwitchMainProcess` runs `progression.Revert()` before every process
    but these screens, dreams and the ghost and karma-to-max screens; the port does the
    same (`PlayerProgression::revert`), so CONTINUE loads the stored state (D17).
- **Options menu.** `OptionsMenu` with Remix off and no console (`rw_menu::OptionsMenu`,
  `rainworld-re/notes/menu.md` "OptionsMenu"), with its `SelectOneButton`s,
  `HorizontalSlider`s, `CustomMessageButton`, `ControlsButton`, check boxes, the reset
  `HoldButton` and the `Landscape_SU` depth scene. What it does in the port:
  - The volumes drive the audio (sound effects for game and menu sounds, music for the
    music player), the screen mode and resolution the window (borderless full screen or
    a window of the size; `Screen.SetResolution` and the ten-tick mode fix-up are
    followed, `Cursor.visible` with them), the save slot loads that slot of the port's
    saves, and RESET PROGRESS is `WipeAll` on the port's slot (D16).
  - Stored only: the language (the port has the English strings and fonts only, so a
    change re-opens the menu in English), vibration (no rumble), speedrun validation
    (no loading screens) and the arena music volume (no arena).
  - Progression loads synchronously, so `ProgressionBusy` is never true (nothing greys
    out) and `HandleSaveSlotChangeFailed`'s dialog never shows: a slot that cannot be
    read is kept in memory and logged.
  - `ControlSetup` is not ported: the `InputSetup` entries are kept as read; a new
    file gets what `new ControlSetup(i, i == 0)` writes (inferred: no mouse mappings).
    An unregistered language value reads as English.
  - Turning the music to 0 and leaving drops the music player (`StopSideProcess`)
    without its songs' shut-down.
  - The dialogs (`DialogConfirm`, `DialogNotify`) are measured with the font's line
    height for `LabelTest.LineHeight` (inferred equal).
- **The save importer (port only).** The game has no such option. The options menu has
  an IMPORT SAVE FROM RAIN WORLD button (a simple button, 220 x 30, centred between BACK
  and CREDITS at y 50; BACK's right and CREDITS' left now lead to it). It asks with a
  `DialogConfirm`, then copies the game's save of the selected slot over the port's
  (`rw_save::import_slot`, the port's file moved to `backup/` first) and reports the
  result in a `DialogNotify`. Its texts are English, untranslated.
- **Pause menu.** `PauseMenu` of a story game is ported (`rw_hud::PauseMenu`,
  `rainworld-re/notes/menu.md` "PauseMenu"): CONTINUE, EXIT and the confirmation, the
  keyboard control map with the pick-up instructions, the dimming, its sounds through
  the game's microphone (volume group 1) and the microphone's dimming and slowing, the
  HUD's pause text, and `ExitToMenu`. While paused the room's frame repeats the last
  unpaused one (the game does not redraw it): the port redraws it at that frame's time
  stacker with the cosmetic stream restored. Not ported: the arena and sandbox buttons
  (RESTART, EDIT, PLAY), the gamepad presets' maps, controller rumble, Expedition's
  unlocks and karma warning, and Platform `systemMenuShowingExplicit`. `RedIsOutOfCycles`
  is the sim's (`RedsCycles` 19, 24 with the save's `RedExtraCycles`). `ExitGame` saves
  as D16 describes (`AppContext::exit_game`); `warnedAboutKarmaLossOnExit` is the
  progression's and saved with it. The port's R respawn (dev starts only, D4) still
  works while paused. During game over the pause menu does not open, as in the game.
- **Depth and flat scenes.** As the game decides, scenes are in depth unless
  `StreamingAssets/flatmode.txt` exists (`rainWorld.flatIllustrations`; the installed
  game has none). Ported in depth (`rw_menu::menu::DepthScene`, `MenuShader`,
  `rainworld-re/notes/menu.md` "Depth scenes"): `MenuDepthIllustration` (the
  double-height layer, parallax `pos - CamPos * 80 / depth`, `DepthAtPosition`,
  `positions.txt`), `InteractiveMenuScene`'s focus (the idle pick from the idle depths
  or layers: `Range`, then `value`; the depth under a still mouse), the globals
  `_BlurDepth`, `_BlurRange`, `_MenuCamPos` and the shaders `SceneBlur`,
  `SceneBlurLightEdges`, `SceneLighten`, `SceneOverlay`, `SceneSoftLight` in WGSL. In
  depth: `MainMenu`, `Landscape_SU` (the options menu), the slugcat select
  pages (`Slugcat_White`, `_Yellow`, `_Red` with the locked Hunter's dark layer,
  `Slugcat_Dead_Red`, the three ghosts; `SlugcatPage.GrafUpdate` places each layer at
  `pos - CamPos * 80 / depth + sceneOffset` slid by the scroll), the sleep, starve and
  new-death scenes of the base game's slugcats (their idle depths; the death scene's
  three flowers, each a layer and a `Lighten` layer, shown one after the other over
  the scene timer's 160 ticks; the screens' map dimming of the back layers by
  `fadeOutIllustration`) and the intro slideshows' scenes (below). The More Slugcats and
  Watcher branches of these (Spearmaster's random sleep layer, Inv's images, the
  Watcher's sleep scene and ripple ladder) are not ported. With `flatmode.txt` the
  flat branches follow the game: the idle camera draws every tick
  (its idle focus pick draws nothing more without layers); the sleep scene of the
  slugcat being played (`PlayingAsSlugcat`: built after the manager switches, so never
  the story character); the death scene's `Flower` flat faded in over the scene
  timer's first 160 ticks, which only the screen starts. Layer images load
  synchronously when the scene is built, as in the game, and stay decoded on the CPU
  (for `DepthAtPosition`) and on the GPU: about 80 MiB each for the main menu's 15
  layers, 36 MiB for `Landscape_SU`'s 3. `SceneRain` and `SceneMultiply` (no base-game
  scene uses them) are not ported; such a layer would draw with `Basic`. The scene
  globals are kept per menu (the last scene `GrafUpdate` of that menu), where Unity's
  are process-wide; only scene layers read them, and a scene sets them before its
  layers draw. As in the game (`Shader.SetGlobal*` in each shown scene's `GrafUpdate`,
  every draw after all of them), the slugcat select menu's three scenes share the last
  page's values: the Hunter's scene's focus and camera blur every page. The grab is sampled at the pixel's own screen position, the net of the
  shaders' `UNITY_UV_STARTS_AT_TOP` flip and `1 - scrPos.y` read on Direct3D (the
  Windows build); on OpenGL the game's would read the grab mirrored.
- **Intro slideshows.** `SlideShow` (`WhiteIntro`, `YellowIntro`; `rw_menu::SlideShow`,
  `rainworld-re/notes/slideshow.md`) is ported: the playlists, the wait for
  `RW_Intro_Theme`, the manager fade and the skip (the pause button, level-triggered
  because `lastPauseButton` is never written). Its `SlideShowMenuScene`s
  (`menu::slide_show_scene`) are in depth as the game shows them: the 15 intro scene
  tables (the slideshow's depths where Intro 1, 3, 4 and 5 differ from an interactive
  scene's; only Remix's background options and Expedition's jukebox build those
  interactively, so those arrays are not ported), the camera path along `camera.txt`
  (the Bezier through the points, `/ 300`, keyed to the scene's time; the focus from
  the third column, -1 interpolated), the constructor's first camera position left
  undivided as the game leaves it (the first update divides it), and the depth and
  flat rules of `ApplySceneSpecificAlphas`. The scenes draw no random numbers. The intro
  playlists add no crossfades. The clock follows the song's first source as the game
  follows `AudioSource.time`: kira's position of the streamed sound in the window, the
  ticks played on `TimedSources` headless (deterministic). The outros, crossfades, the
  More Slugcats and Watcher slideshows and the dev move editor are not ported.
- **Saves, options and progression.** The progression is the port's own slot of
  `options.saveSlot` (`sav`, `sav2`, `sav3` in its save directory, D16): the selected slugcat, the Hunter unlock
  (`unlockred.txt` or the main menu's unlock code, saved with the next progression
  save) and the discovered shelters (Regions) come from it. The options are the port's
  own (`options` and `localoptions.txt` in its save directory, in the game's format,
  unknown entries kept as the game keeps them), saved where the options menu saves
  them; the game's are never read or written. The Downpour/Remix first-launch notices
  (`dlcTutorialShown`, `remixTutorialShown`) count as already shown, so the main menu
  opens without its popup.
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
  `FadeInTime` (2 s). On a page with a save, START continues it
  (`StoryGameInitCondition.Load`: 0.75 s black, then the save state's `SlowFadeIn`) and
  the restart check box rises; checked, START (held 120 ticks, "DELETE SAVE") wipes the
  save (`WipeSaveState`) and starts a new game, which is the game's only confirmation.
  A page with a save is `SlugcatPageContinue` (`rw_hud::SlugcatSelectMenu`,
  `rainworld-re/notes/menu.md` "Continue pages"): its scene (the ghost after
  ascending, the dead Hunter's once its cycles ran out), the region and cycle label (the
  Hunter's cycles left) and its HUD's karma meter and food meter, placed and faded as
  the game does; with a single save its page opens first. `MineForSaveData` reads the
  current save state, else mines the stored one (`SaveStateMiner`); a room exists when
  its file does (the game's `roomNameToIndex`, compared case-insensitively). In depth
  mode the page has the mark of communication and the glow its save has (`HasMark`,
  `HasGlow`: not after ascending or for the dead Hunter), the glow among the scene's
  layers where `BuildScene` calls `AddGlow`, and the menu has its `RainEffect` behind
  the pages (fading in to 0.3, no lightning). The mark and glow are drawn after the
  menu's other objects, as the HUDs are. Not ported: the Remix speedrun time in the
  label and
  the campaign time tracker's `LoadOldTimings`, and the More Slugcats, Watcher and Jolly
  branches (alternate endings, `rippleLevel` symbols, custom colours). A shelter name
  without `_` makes the game throw; the port takes the whole name as the region. The
  continue pages' HUDs are drawn after the menu's other objects (the game draws each
  after its page's sub-objects); nothing else draws to their sprites, so the frame is
  the same. The Hunter's ended save goes to the unported Statistics screen. The chosen slugcat colours the body's graphics; its body moves as
  the Survivor's (the Monk's and the Hunter's `SlugcatStats` are in
  `rw_sim::campaign::SlugcatCampaign` but the player does not read them yet). Started
  directly
  (`--process game`, `--spawn`, MCP), the game is the Survivor in the given room's
  world.
- **Side processes.** The music player (made in the first `ProcessManager.Update`,
  ticking on its own time stacker after the main process, `UpdateMusicContext` after
  every switch) and the menu microphone run, in the game's order; no dialogs or Steam
  manager, so the switch queue stays empty. The game drops `manager.menuMic` on a switch
  to the game but leaves the old microphone running as a side process (its loops fade
  out) and makes a new one on the way back; the port keeps one microphone and tracks
  whether the manager holds it, which sounds the same. Mod finalization completes in the
  same frame (no mods). Of `HardCleanFutileAssets`
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
  `Input.GetMouseButton`. `options.fullScreen` is the window mode (the options menu
  sets it; `--windowed`, and so `--screenshot`, sets it for the run); as in the game
  (`Options.OnLoadFinished`), the OS cursor is shown only when it is off, and the menus'
  `MouseCursor` sprite only fades in when it is on (`ShowCursor`). The sprite sits at
  the mouse position of each rendered frame, not the last tick's, as in
  `MouseCursor.GrafUpdate`. The dev-tools cases (`Cursor.visible = true`) are not
  ported.
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

**Why.** The menus are ported screen by screen; the world with only overseers
behaving (D14) stands in for a story session. The importer is the user's request: the
port never writes the game's saves, so continuing a game needs a copy.

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
- **Only overseers and bat flies behave; the other creatures roam abstractly.** The `CREATURES` section is read in full (simple spawners
  and lineages with their dens, amounts, spawn data and night flags; quantified
  counts; `rw_assets::creatures`), and a new game's `GeneratePopulation` builds every
  `AbstractCreature` with all of its constructor draws (D1) and `setCustomFlags`' base
  flags (`Night`, `Ignorecycle`, `PreCycle`; the region's `globalCreatureFlags` are not
  read, as no vanilla region sets them, nor are `migrationInfluences`: both left out by
  the user's decision): the creatures go into
  their dens and come out as the game's do, but the scavengers stay in theirs (their
  abstract AI and world process are not ported), quantified
  creatures are counted per node (the bat flies move, are placed and realized, below), and items
  (scavenger gear, drop bugs' items) are kept as IDs and kinds on their carrier, not as
  objects in the room's entities. The
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
  or food item is realized: bat flies are small creatures and no food item to the
  communication module), the angry hologram (`angryWithPlayer` is never set: no
  weapons), the gate scene (`RegionGate` is not ported: in Moon's gates the concern is
  `None`, as the game's is with a closed gate), the forced direction
  (`ReliableIggyDirection` is not ported; the port makes no pointer for it), the
  in-world super jump and scavenger-trade instructions (`SuperJumpInstruction` and
  `ScavengerTradeInstructionTrigger` are not ported), and the tutorial's Eat instruction in SU_A44 (its fruit is not realized;
  SU_A42's, over the bat flies, is ported). A door pointer without the tutorial would follow the player with a
  `CreatureDoorDistanceTracker` (on the Fly's exit distances, which now exist): it is
  made but its update is not ported, so the pointer keeps pointing. The bat pointer's `CycleStartUp` is
  1 (the cycle's start-up ticks are not ported), and a new game has discovered no
  shelters and visited no rooms. The controls picture is always the keyboard's (the
  gamepad and Switch pictures are ported but no other control preset exists). A pointer
  whose overseer has left the room goes on updating where the game's would throw (shelter
  and direction pointers without `FORCEPOINT`): the port logs and stops there. The
  overseer's AI looks at the player, overseers and bat flies; no other creature is
  realized. Which camera's room
  counts as viewed when `AddObject` builds the graphics module is inferred from
  `RoomCamera` and `Room.AddObject`, not checked.
  The `AbstractCreature` constructor's abstract AI switch (300531..300570) is ported
  for every base type but the scavenger (`CreatureTemplate::abstract_ai_ported`): the
  vultures', cicadas', big eels', deer's (Remix's deer behaviour off), Miros birds' and
  needle worms' own `AbstractBehavior`s (`world/abstract_ai_kinds.rs`), and the plain
  `AbstractCreatureAI` for the drop bugs and every other type with an AI, on the
  templates' off-screen connection costs and `stowFoodInDen`
  (`creature/template_abstract.rs`) and the relationships (`creature/relationships.rs`,
  for `HavePrey` and prey to stow). Creatures without an AI (vulture grubs, hazers,
  pole mimics) come out of their dens and stand there, as the game's do; the
  quantified leeches and spiders stay counts (the game moves only the bat flies'
  counts). A vulture whose save says `NOMASK` would look for its lost mask; the mask is
  never an object in the port's world, so the search finds nothing and it behaves as
  with its mask, as the game's would without the object. The deer's six hard-coded LF
  rooms are read without a null check in the game (a deer elsewhere would throw); the
  port skips a missing room. A cicada that catches a counted fly carries a new abstract
  fly, which turns back into a count at the next node it is carried to (`Quantify` in
  `Move`); a carried fly in a room that loads stays abstract with its carrier. C#'s
  `(int)` of NaN or infinity (`FollowPath` with an off-screen speed of 0) is
  `int.MinValue` (INFERRED from the x86-64 instruction), not Rust's saturation.
  The plain AI: `InDenUpdate` with
  `WantToStayInDenUntilEndOfCycle` (the rain within 60 s, a pre-cycle creature, which
  without More Slugcats' precycles never comes out, a night creature waiting for dark),
  `Update`, `AbstractBehavior`, `MigrationBehavior`, `RandomMoveWithinRoom`,
  `RandomMoveToOtherRoom`, `MoveToMoreAttractiveNeighborRoom`, `FollowPath` with its
  timing (`offScreenSpeed * offscreenSpeedFac`, the room's connection lengths and the
  shortcuts' tube lengths) and stuck jump, `SetDestination`/`FindPath`,
  `AbstractSpacePathFinder`, `GoToDen` with `AbstractSpaceNodeFinder`, `Moved` and
  `OpportunityToEnterDen` (rainworld-re `creature-model.md` §10). Lizards do not flee
  the rain: there is no rain yet, and the realized `EscapeRain` behaviour waits for it
  (the user's decision); abstractly a lizard only goes into a den it passes once the
  rain is within 60 s, as in the game. Not reachable yet, for want of what triggers
  them: following a creature from a social memory relationship (social memories are not
  kept; a small needle worm's following of its mother is ported), a realized creature's
  prey taken home, and `IsEnteringDen`'s killing and dropping of carried prey (an
  abstract creature only carries a caught fly, which is counted again before it reaches
  a den).
  **A creature entering a realized room stays abstract.** The game realizes it there
  (`ChangeRooms`' `Realize` and `CreatureEnterFromAbstractRoom`, `IsExitingDen` in a
  realized room, `ReadyForAI`'s `RealizeInRoom` with `PlaceInRealizedRoom`), but no
  creature but the overseer and the bat fly has a body yet. The port keeps it in the room's
  lists as an abstract creature, as it does for every other unrealized type: its
  `AbstractCreatureAI.Update` only adds to its time buffer while the room is realized
  (the game skips `AbstractBehavior` there too), so it waits, unseen and harmless,
  until the room is abstracted, then walks on with the buffered time. None of the
  realization's draws are made (D1). `DefineNodeOfLocalCoordinate` in a room ready for
  AI keeps its drawn node instead of choosing by the creature's exit distances (the
  lizards' AI map is step 1); a creature only asks there once it is realized.
  `AbstractCreature.distanceToMyNode` now starts at 0, the game's default (it was -1).
  Left out with their DLC or modes: More Slugcats' flooded dens, battle arenas,
  precycles (`preTimer`/`maxPreTimer`), the slugpup's and the scavenger king's
  migration rules and the MSC-only template flags (`doesNotUseDens`,
  `abstractImmobile`); Expedition's hunted unlocks; arena den emergence; world
  migration influences; named attractions (no vanilla creature has one).
  `OverseersWorldAI` is ported (its shelter finder and the guide's direction finder,
  on the room files' baked node connectivity), as are the flies' swarm room map (with
  the world file's bat migration blockages), `GetSwarmRoomDistance`, `ActiveSwarmRoom`
  and a new game's `RegionState` swarm counters.
- **Rocks and spears (abstract).** A room's `entities` hold creatures and objects
  (`rw_sim::item`, `rainworld-re/notes/items.md`). On its first realization in a cycle a
  room makes the rocks and spears of its file's item list and `Room.Loaded`'s random
  items (with their IDs and draws, D1; explosive spears are flagged, at the game's
  chances: 0 for the Survivor's and Monk's timelines); saved rocks and spears return to
  their rooms, stuck spears count down their cycles, and objects move between rooms'
  lists with `Move`. A realized room realizes them in `ShortCutsReady` (`Rock`,
  `Spear`: free items lie, roll, skid and stick into the ground; saved stuck spears
  hang in their walls and make poles, D6), in the room's update order with the
  collision pass between items (`Room.Update`), in the water's object loop, and
  abstractizes them with the room. An explosive spear is realized as a plain `Spear`
  (its fuse, rag and blast come with `ExplosiveSpear`). In a story, `Room.Loaded`'s
  `MultiplayerItem`s spawn on every realization and `ReliableSpear`s on the first
  (with their `placedObjectOrigin`); of the multiplayer items' kinds a scavenger bomb
  and a spore plant stay abstract (not realized; the spore plant lacks its consumable
  fields). The player picks them up, carries them (through shortcuts and room exits
  too), swallows rocks and throws them (D3); thrown spears stick into walls (the room
  edge ring, `NoSpearStickZone`s and a moving shelter door excepted). Not yet:
  hitting creatures (`HitSomething`, `StuckInCreature`), items falling between rooms
  (an item below the room is clamped at -1200 px as the game does for objects; only
  creatures are destroyed there); and objects of other kinds (placed food,
  pearls, ...).
  Arena levels (single-room worlds) make items as a story room does (the arena's
  `levelItems` setting is not ported). `GeneratePopulation`'s reset clears a room's
  `creatures` but not its `entities`, as the game does.
- **Bat flies.** `FliesWorldAI` is ported whole (the constructor, `Update`
  with the migration, start-up and respawn, the behaviour re-roll, `MigrationDirection`,
  `ClosestSwarmRoom`), with `World.MoveQuantifiedCreatureFromAbstractRoom` and the
  quantified counts (`NumberOfQuantifiedCreatureInRoom/InNode`, `Quantify`,
  `CountQuantifiedCreatures` when a room is abstracted). A room ready for AI places its
  flies (`PlaceQuantifiedCreaturesInRoom` and `PlaceQCScore` on the Fly's baked map)
  and realizes them; so does `MoveFly` into a realized room (through the lobby and the
  pipe) and `CreateFlyInHive` (a bat hive vessel). The realized fly is the game's
  (`rainworld-re/notes/creature-model.md`): `Fly` with its flight modes, burrowing,
  hanging and chains (the grasp framework: `Grasp`, `grabbedBy`, `LoseAllGrasps`),
  water and death, `FlyAI` with its threats, `FlockBehavior` and the swarm, chain and
  leave-room behaviours, `FliesRoomAI` with the hives, the hover loop, the shortcut
  travel (D5), `Abstractize` when its room is abstracted, and `FlyGraphics` in a viewed
  room (`rw_graphics`). The player catches, holds, throws, carries and eats flies (D1,
  D8); the quantifying of a fly held while its room is abstracted is not exercised. Not
  ported: a creature's terrain impact's `InGameNoise` and the rain's
  `CreatureSmashedInGround`, which needs the rain. The fly's `RoomNotACycleHazard` reads the room settings'
  `DangerType` as registered with no DLC (a DLC value such as `Blizzard` counts as
  unregistered).
  The other quantified types (leeches, sea leeches, spiders) are counted but not
  placed; no vanilla SU spawner has them. The `FlyLure` and `Lure` placed objects are
  not read (none in the tutorial rooms; SU_A17 and SU_B08 place one).
- **The AI map.** `AImap` is built as `AImapper` builds it, for every base-game
  template (accessibility classes, terrain proximity, floor altitudes, narrow spaces,
  fall risk, every movement connection, `TileAccessibleToCreature`,
  `IsConnectionAllowedForCreature` and the costs), and each pre-baked pathing creature's
  `CreatureSpecificAImap` is decoded from the room file as the game decodes it, stale
  layers included (`ai-map.md`). The flies read this map. Not ported: the worm grass
  access modifier (the game makes worm grass tiles inaccessible when it builds the map,
  after baking, so in the 28 vanilla rooms with worm grass, SU_A32 and SU_B05 among
  them, its decoded distances shift; the port decodes them as baked), the map rebuild
  when a shelter door opens, Remix's zero-gravity lizard branches, Downpour's
  `TerrainManager` curved floors and sand, More Slugcats' `daddyCorruptionImmune`
  branch, and `ModifyTileWaterLevels` (WaterCutoff and
  AirPocket objects, which no vanilla region room places). Remix's
  `BringToRoomAndGuidePlayer` branch of the world
  process's update and More Slugcats' goal rooms and guide symbols are left out.
  Not ported: the scavengers' abstract AI (`ScavengerAbstractAI`: squads, outposts,
  traders, kill squads, re-gearing) and `ScavengersWorldAI`, and `CreatureCommunities`
  in the sim (the save keeps their likes, D16). `NewWorld` (a region change) is not
  reachable (no gates).
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
- **Fly graphics.** The flies' sprites go into the Midground container after the
  slugcat's; the game orders a container by when each graphics module joined it (flies
  realized before the player's body sit under it).
- **Synchronous loading.** The game's `RoomPreparer` loads a room over several frames
  (and `RoomCamera.MoveCamera` its images asynchronously); the port loads a room's
  files and builds its water in one call, finishing at the end of the tick the room is
  activated. A room is therefore `fullyLoaded` sooner than in the game: a room exit's
  destination (activated when the player goes in) is always loaded by the time the
  vessel reaches the exit node, so the lobby only waits out the room's entry delay
  (`waitToEnterAfterFullyLoaded`, counting from the tick it loaded), and the
  realizer's "currently loading" pause never spans ticks.
- **Re-preprocessing old room files.** The game re-preprocesses room files baked before
  generation 20 (and writes them back). Every room the vanilla worlds list is generation
  20 or later; for older files the port derives only the shortcut nodes (exits, dens,
  region transports with length, submersion and nearest camera), not border exits,
  bat hives or garbage holes, and no connectivity between them (every path -1).
- **Off-screen den and gates.** The off-screen den is never realized; gates are
  ordinary rooms (no gate mechanism, no region change), and a `RegionTransportation`
  node goes nowhere.
- **Not ported:** the rain (the cycle's timer, day-night counter and water level stages
  run; the overseers' AI, the HUD's rain meter and `MusicAllowed` read them, and the
  guide's tutorial pins the timer), karma gates, `DisabledMapRooms` beyond the tag drop, and Remix's
  `RemoveNotVisitedRooms` branch.
- **Room triggers.** A room's settings `Triggers` (`rw_assets::EventTrigger`, every
  base-game trigger and event type) become `ActiveTriggerChecker`s when it loads
  (`Room.Loaded`, after the room script and before its items, filtered by the save's
  cycle and slugcat), updating in the room's update list with their wait, delay,
  karma, `fireChance` (D1) and `multiUse` (a failed chance still removes a single-use
  checker). The events that fire are the game's: `MusicEvent` and `StopMusicEvent`
  (to the music player, D12, gated by `RainCycle.MusicAllowed`), `ShowProjectedImageEvent`
  (the guide shows the room's placed scene, its 0.99 `ShowPlacedScene` concern),
  `PickUpObjectInstruction` (the guide's pick-up hologram over the nearest free rock or
  spear, its text in SU_A23), `BringGuideToPlayerEvent`, `RoomSpecificTextMessage`
  (`SL_C05`'s `DangleFruitInWaterMessage`, saved in the tutorial messages, D16). As in
  the game, the start room's triggers wait until the player leaves its start shelter
  (`stillInStartShelter`), `SeeCreature` never fires, and `PreRegionBump`/`RegionBump`
  read the HUD's subregion tracker. Differs: `SLOracleState.playerEncounters` (Moon) is
  read as 0, so Shoreline's image triggers fire as for a player who never met her; an
  image's `ProjectedImagePosition` placed object is not read (the hologram stands where
  the overseer is); the guide's state (images and pick-ups shown, how much it likes the
  player) starts from its defaults each game, not from the save (D16). The More
  Slugcats, Watcher and Challenge events are parsed and ignored.
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

**Affects.** How long a vessel waits to enter a room, and anything creatures other than
overseers and bat flies would do (the others roam, but never appear in a loaded room).

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
- **Region map.** `HUD.Map` is ported for a player-owned story map in its base-game
  branches (`rw_hud::Map`): the map image in three layers through the `Map` and
  `MapAerial` shaders, the reveal from the discovery texture, pan and layers, the
  connection lines, the player marker, gate and shelter markers, the swarm-room circles
  and the Hunter's cycle label, with every random draw, plus the death markers (the save
  state's `DEATHPOSS`) and the karma flower marker. The same map, owned by a sleep or
  death screen (`SleepHud`, D13), fades in without the
  hold and, on a death screen, has no player marker. The player's exit discovery and
  `ReportRoomVisited` run in `Player.Update` as in the game. Differences: shelters show
  no stored items (the port has none); the flower marker reads the save's `FLOWERPOS`
  (the karma flower is not spawned, so the world load's clearing of the position when
  the flower is gone is not ported); a saved coordinate in the old numeric-room form
  places no death marker or flower (the port does not resolve room indices from old
  saves); `CompleteDarkness` is the camera palette's darkness alone (no
  light sources are ported); in a shortcut the owner's position is its vessel's tile
  even while the vessel waits for another room; the cycle label counts 19 cycles (no
  Remix settings, no extra cycles). Not ported: the More Slugcats gate locks, Remix's
  item and creature markers and fast reveal, Watcher warp and spinning-top markers, and
  the fast-travel and region-overview owners.
- **Rain.** The rain meter reads the world's `RainCycle` (its drawn length and
  ticking timer, which the guide's `pauseRain` pins at 2000 in the tutorial rooms; D14).
  The rain itself is not ported; only the bat flies read a room's rain `DangerType`
  (`RoomNotACycleHazard`, D14).
- **Start state.** Food, karma, cap, reinforcement and the cycle number are the save
  state's (a new game's: food 0, karma 0, cap 4, the Hunter karma 2, cycle 0, so the
  meters start hidden); food then rises only by eating bat flies (a full pip each, a
  quarter pip for the Hunter).
- **Shelters.** The owner's `stillInStartShelter`, `readyForWin` and
  `forceSleepCounter` are the player's (D17); every shelter counts as intact (broken
  shelters are not read).
- **Screen meters.** The food meter's sleep and death screen branches (`SleepUpdate`,
  `DeathUpdate`, the eat circles and `FoodCountDownDone`) and `InitSleepHud` are ported
  (`rw_hud::SleepHud`), with the sleep-death data package `CommunicateWithUpcomingProcess`
  builds (`SleepDeathScreenDataPackage`), for the screens of D13. The package's save
  state is the session's: after a malnourished `Win` the progression's starved state.
- **Game over.** `Player.Die` → `RainWorldGame.GameOver` → `InitGameOverMode` with the
  food in the stomach, `Players[0].pos.room` and the main chunk at the death restricted
  to the room grown by 50 px; then the music's `DeathEvent` (its song fade; the
  threat tracker's `threatDeclineCounter` is not, D12). The prompt's game-over press
  reads this tick's map button (the default keyboard binding, `GetButton(11)`), not the
  dead player's input. Differences: there is no `dependentOnGrasp` (no creature grasps
  the player, so `Player.Update`'s danger-grasp game over and the escape that cancels
  game-over mode are not ported); the gamepad and rebound-map branches (the pause
  button press and their strings) are not ported; dev starts have no game over (D4);
  the Hunter out of cycles (`GoToRedsGameOver`) is not ported (D17).
- **Not ported:** the prompt's symbol and music messages, the Hunter's
  cycle count-up, the karma meter's gate and scavenger blinking, the gamepad pictures of
  the controls page, the character-select screen's meters, and every
  DLC part (More Slugcats pup meters, ammo meter, Watcher ripple symbols, Jolly Co-op
  meters, Remix's options).

**Why.** The rain cycle, eating, saves and shelters are later milestones.

**Affects.** What the meters show (food, rain) and when they show in shelters, and the
map's markers; no gameplay beyond the map button standing the slugcat still.

## D16: Saves

**What.** `rw-save` reads and writes the game's save format (`rainworld-re/notes/saves.md`):
the `UserData` container (a serialised `Hashtable`), the salted MD5 checksum (the salt is
read from the installed `Assembly-CSharp.dll`, not shipped) and the progression lines.
The port keeps its saves in its own directory (`~/.local/share/rainworld-rs/`,
`%APPDATA%\rainworld-rs\`, `--save-dir`), never the game's; `import_save` copies one of
the game's slots in, after moving the port's previous file to `backup/<seconds>_import/`.
What is ported of the save cycle: a story game's start (`GetOrInitiateSaveState`, saved
at once as a death and a quit), Continue from the stored state's den, food, karma, cycle,
`NEXTID`, `SEED`, respawn list and the den region's `REGIONSTATE` (population, consumed
items, swarm rooms, `ROOMSVISITED`), New Game's `WipeSaveState`, `ExitGame`'s save
(`SaveProgressionAndDeathPersistentDataOfCurrentState`, from the pause menu's Exit) and
`Win`'s whole save state (`SessionEnded(survived: true)`, `BringUpToDate`,
`SaveWorldStateAndProgression`, the once-per-run `_Backup` copy; D17).
Differences:
- **Writing what the port does not own.** The game rebuilds `MISCPROG`, the save state
  and its death-persistent data from its fields, in its order, with unknown (DLC, Remix,
  mod) entries moved to the end, and drops every line of the progression it does not
  own on `SaveToDisk` (stored `MAPUPDATE` times of maps not loaded, other slugcats'
  `MAPUPDATE_<slugcat>` lines) and, with Remix off, Remix's `OBJECTS`/`FRIENDS` save-state
  entries. The port keeps every line and entry it does not own as read, in place (user
  decision: imported DLC data is kept and round-trips unchanged). It rewrites only
  `CURRENTSLUGCAT`, `INTEGERS` and `CONDITIONALSHELTERDATA` in `MISCPROG` (adding the
  first two if missing), the death-persistent data (typed fields in the game's order,
  every other entry kept, unknown ones at the end as the game writes them) and map
  lines it holds a texture or time for. `Win` writes the whole save state in the game's
  order, its unknown entries kept at the end.
- **What a hibernation saves.** Creatures keep their state string as read (the port
  has no `CreatureState` beyond alive, so `CycleTick` does not age it); rocks and
  spears are saved as the game saves them (in shelters, and spears stuck in walls), but
  saved objects of other kinds are kept as text and written before them (the game
  writes those it recognizes in room order with the rest, and drops karma flowers);
  sticks (`STICKS`, which the game rebuilds from the shelters' objects) are kept as
  read; `Win` writes the player's held objects (`PLAYERGRASPS`) and swallowed object
  (`SWALLOWEDITEMS`) as `BringUpToDate` does, but a swallowed creature or object of a
  kind the port does not type stays as its save string (it cannot be coughed up) and
  is written back as read, and so are the kills (`AppendKills`), communities' likes beyond
  their cycle tick, and friends in the den. A saved creature the world cannot place
  stays in the saved list. Session time is the sim's ticks.
- **Win state.** Integer and float passage trackers are read and written as the game
  does, including the death penalty (applied, as in the game, only to a tracker whose
  last shown progress is below its maximum); the bool-array and list trackers (Traveller,
  Dragon Slayer, Scholar) are written back as read, so the game's re-encoding of a
  More Slugcats Dragon Slayer list as base-game bools and its Traveller slot count are
  not applied.
- **Screens' data.** The sleep and death screens' save-side reads are ported and used
  (D13): `WinState.PlayerDied`, each tracker's `GoalFullfilled`/`AnyProgressToShow`,
  the `MiscProgressionData` watched-screen counters (`INTEGERS` 1-4), the
  death-persistent `METERSSHOWN` list, `DEATHPOSS` and `FLOWERPOS`;
  `ResetLastShownValues` runs where the game runs it (a starved state taken up by
  `GetOrInitiateSaveState`). `GetNextEndGame` waits for the PASSAGE button, which is
  disabled.
- **Container.** A new key goes last (the game's `Hashtable` orders keys by string
  hash); the files have no NUL padding (the game writes `MemoryStream.GetBuffer()`);
  they are written to a temporary file and renamed.
- **Continue.** The den region's saved region state and the respawn list are applied
  (`AdaptWorldToRegionState`, `GeneratePopulation(fresh: false)`) and a swallowed rock
  or spear is back in the stomach (the `Player` constructor); held objects are not
  restored, as in the base game (only More Slugcats' forced end rooms read
  `PLAYERGRASPS`); communities, kills and dreams are kept in the save but not applied.
  `BackwardsCompability` world-version updates and the automatic `backup/` copies
  (`CreateCopyOfSaves`) are not ported. The slot is `options.saveSlot` (D13).
- **Reset progress.** `WipeAll` keeps only `MISCPROG`, as the game, with its shelters,
  conditional shelters, menu region, the Hunter's flower, campaign timers and the
  base `INTEGERS` it resets cleared; the DLCs' integer lists (`INTEGERSMMF`, ...) are
  kept as read. There is no `SJ_<slot>` folder to delete.
- **Starving and dying.** The starve and death screens follow (D13).
  `GoToStarveScreen` (a door closing on less than one pip, or on less than a full meter
  in a malnourished cycle; D17) and `GoToDeathScreen` run `SessionEnded(survived: false)`
  and its death save, with the game-over prompt's death room and position (room
  0 and (0, 0) on a starve, as in the game). `GoToDeathScreen` comes from the game-over
  prompt's map press (D15) or a shelter door closing on a dead slugcat. The save's
  `FLOWERPOS` is written from `PlaceKarmaFlowerOnDeathSpot` (D17).
- **Session time.** The game stops counting `SessionRecord.time` while the prompt is in
  game-over mode (`Player.Update`, `TimeTick`) and counts `playerGrabbedTime` while a
  living player is in it; the port's session time is the sim's ticks, which go on.
- **Map discovery.** `progression.mapDiscoveryTextures` and `mapLastUpdatedTime` are
  split: the game's copy is the sim's `MapDiscovery`, and the progression holds their
  save form (`map_discovery`, `map_last_updated`). The map's first update loads a
  region from the progression, else its `MAP`/`MAPUPDATE` lines (`LoadMapTexture`); the
  pause menu's Exit copies the sim's textures and times into the progression before
  saving. A game left any other way loses its discovery, which the game keeps in memory.
- **Songs and tutorial messages.** `SONGSPLAYRECORDS` is typed (`rw_save::SongPlayRecord`)
  and live: `GameRequestsSong` reads and writes the current save state's records (D12),
  saved with the rest of the death-persistent data. `TUTMESSAGES` is read in both the
  old digit format and the name list (`ParseTutorialMessages`); setting a message
  (`DangleFruitInWaterMessage`, D14) writes it back as a name list, as the game does.
  The guide's `PlayerGuideState` (images and pick-ups shown, likes) is not read from
  or written to the save: each game starts it at its defaults.
- **Play time.** `AppendTimeOnCycleEnd` is not ported: `ExitGame`, starving and dying add
  nothing to `DEATHTIME` or `TOTTIME`; `Win` adds the cycle's ticks / 40 to `TOTTIME`.

**Why.** The creature states, items and kills the save keeps come with their
creatures and objects; the user decided imported DLC and mod data must survive the
port's writes.

**Affects.** Saves the port writes; the game reads them, but they can differ from what
the game would write in line and entry order and in the data it would drop.

## D17: Shelters and hibernation

**What.** `ShelterDoor` (`rw_sim::shelter`, `rainworld-re/notes/shelter.md`) opens with
the cycle and closes when the player, fed and still, is ready for `Win`; the player's
`SleepUpdate`, `readyForWin`, `forceSleepCounter` and the sleep curl are ported. `Win`
saves (D16) and goes to the sleep screen or a dream; the next cycle, after
`progression.Revert()`, loads the stored state as the game does (a starved cycle's state
and region state stay in memory and are reused, its trackers' shown values reset),
starts in the shelter with its door shut and the slugcat curled up.
Differences:
- **Sleep screen and dreams.** The sleep screen is ported (D13); its CONTINUE starts
  the next cycle. The dream screen (`DreamScreen`) is not: a stand-in shows 40 black
  frames, then starts the next cycle as the sleep screen's continue does
  (`StoryGameInitCondition.Load`). The Moon and Pebbles event dreams are not ported (no
  iterators).
- **Starving.** As in the game, when the closing door passes `closedFac` 0.4 with
  `FoodInRoom < (saveState.malnourished ? maxFood : 1)`, the slugcat starves: a death
  (`GoToStarveScreen`, `SessionEnded(survived: false)`, D16) and the starve screen
  (D13). Otherwise the closed door calls `Win(FoodInRoom < foodToHibernate)`: with
  some food but less than `foodToHibernate` that is a malnourished survival, saved as
  the game does (only the progression and death-persistent data reach the disk; the
  new state stays in memory as the starved state) and the next cycle is malnourished.
- **Objects in shelters.** Rocks and spears in any shelter when `Win` saves, and spears
  stuck in walls anywhere, are written to the region state and come back the next cycle
  (D16) and are realized in their rooms (D14).
- **Karma flowers.** The player's `karmaFlowerGrowPos` is tracked as `Player.Update`
  does (`PlaceKarmaFlower`: reinforced karma, the Monk, the Hunter out of cycles), and
  `PlaceKarmaFlowerOnDeathSpot` stores it in the save state's `karmaFlowerPosition`
  (`FLOWERPOS`, cleared when the player never stood on a spot) on `GameOver` and on a
  starve. Not ported: worm grass tiles (none in the port, so that condition never
  excludes a tile), a grasp by a creature (never grabbed), the karma flower that grows
  there in the next cycle (`AbstractConsumable` placed at world load) and eating it,
  and the Hunter's `miscProgressionData.redsFlower`.
- **Music.** The `RainWorldGame` constructor's `MusicPlayer.NewCycleEvent` (a new
  cycle may play a `oneSongPerCycle` song again) is sent when each story game starts,
  a new cycle after a hibernation included; the dev respawn (R) is no new cycle and
  sends none.
- **The Hunter out of cycles.** `GoToRedsGameOver` (statistics screen, `redsDeath`) is
  not ported: `GameOver` logs and returns, so the dead Hunter gets no prompt and the
  pause menu's exit is the way out; a starve or a door closing on a dead Hunter out of
  cycles saves an ordinary death.
- **Not ported:** More Slugcats' ancient and broken shelters, `PrecycleModule`, the
  shelter tutorials, the dream ghost, `GetStuffInWalls`, the AI map rebuild when the
  door opens, the door's screen shake, the social-memory stun of creatures in the room,
  killing hostiles at close (the check alone is ported), and every Jolly Co-op and More
  Slugcats branch.
- **Malnourishment, DLC and Remix branches omitted:** More Slugcats' slugcats' own
  malnourished stats, pup malnourishment and `malnourishedByCreature`, the Saint's and
  wounded exhaustion, Gourmand's exhaustion, the oracle and Artificer hooks, Expedition's
  agility stats, the Watcher's frogs (malnourishing bites, a latch that skips a
  malnourished slugcat) and camouflage exhaustion, Jolly Co-op's per-player stats, and
  Remix's `cfgVanillaExploits` death save of a malnourished `Win`.

**Why.** Iterators, items, creature states and dreams are later milestones.

**Affects.** What follows a hibernation, a death or a starve on screen, the karma
flower in the next cycle, the Hunter's last cycle, and creatures caught in a closing
shelter.

## Not ported yet (not divergences, just absent)

Player (stage one leaves out): sneaking and visibility (`privSneak`, `Sneak`, `VisibilityBonus`; the `SlugcatStats` stealth values exist but nothing reads them, as no ported creature sees or hears the player), zero gravity, grabbing and eating anything but bat flies, rocks and spears, thrown
weapons hitting creatures (D3), adrenaline, Red's illness, the WALLSTUCK safety
(needs `VisualContact`), vines, tube worm,
Jolly co-op, MSC / Watcher class mechanics. Ported because ground classification
produces them anyway: wall slide and wall jump (`WallClimb`, `canWallJump`, `jumpStun`;
only the wall-jump impulse has a test), `LedgeCrawl`, `CrawlTurn`, the crouch super-jump
charge and dive force (untested).

Physics: burrowing, terrain curves (`TerrainCurve`), the curved-terrain `TerrainImpact`
call, the room-range clamp's skip for grabbed objects, and `DefaultTile`.

Room: `Room.Update`'s ambient ceiling drips (`WaterDrip`s from the ceiling of a room with
`CeilingDrips`), the `WaterDrip`s of skids, slides and corridor shoots, and the
cosmetic insects (`InsectCoordinator`), among them the `MiniFly`s that buzz around
corpses and a malnourished slugcat (the bat flies do not).

Front end: the regions, arena and Remix menus, input settings, credits, backups,
the dream screens, PASSAGE's custom end-game and fast travel screens, the sleep
screen's kills, the outro and DLC slideshows, the slugcat select menu's
colour and Jolly Co-op controls, the `SceneRain` and `SceneMultiply` layers, menu objects those use (vertical and
subtle sliders, check box groups, symbol buttons, scroll boxes, `UIelementWrapper`),
Remix's options (FPS, VSync, quality, analogue sensitivity, backgrounds), and the
in-game music beyond the triggers' songs (D12).

Passage triggers (`WinState.CycleCompleted` and `PlayerDied` are ported whole; these
inputs to them are not): kills (`PlayerSessionRecord.AddKill`, `BreakPeaceful` from
kills and weapon hits), so the Outlaw only resets, the Saint never breaks its streak
and the Dragon Slayer never fills (eating a bat fly kills it in the game, but a fly
counts 0 and is no lizard, so no tracker sees it); plant food, so the Monk only resets;
pearls found (Scholar); a friend in the den (the Friend only resets); changes to the
scavengers' like (Chieftain: the saved like is read); region gates, so the Wanderer
(Traveller) can only mark SU. More Slugcats' Nomad, Pilgrim, Martyr, Mother and
Gourmand trackers are kept unrecognized, as the game does with More Slugcats off.
