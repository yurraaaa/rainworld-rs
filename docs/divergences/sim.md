# Divergences: rw-sim

See [README](README.md) for the format. IDs are global; code and notes cite them.

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
  skims, sticking: `rainworld-re/notes/items.md` section 11) and so is hitting the
  creatures of collision layer 0, the bat flies and overseers
  (`HitSomethingWithoutStopping`, `TryImpaleSmallCreature`, `INotifyOfFlyingWeapons`;
  section 12), and of collision layer 1, the green lizards (`HitSomething`, `Violence`,
  `LodgeInCreature`, `StuckInCreature`; D14). `SocialEventRecognizer.WeaponAttack`
  breaks the peaceful record and tells the room's lizards (hits and near misses,
  `creature-model.md` section 18), with the fear branch for lizards that fear the thrower (step 7b). Adrenaline's
  throw force (`Lerp(1, 1.5, Adrenaline)`) is 1 (adrenaline is skipped for now, a user
  decision of 2026-10-16). Remix and More Slugcats branches
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
calls `GameOver` (story sessions and dev starts; D15, D17); the dead body keeps simulating as plain
physics with neutral input, its water, graphics and room camera as in the game.

**What differs:**
- **Game over and R respawn in dev starts** (user decision, 2026-10-22). The port's
  starts in a room (`load_room`, `--room`, arena levels: `StartCondition::Dev`) are not
  story sessions, where the game's `GameOver` returns at once (`IsStorySession`); the
  port runs it as in a story: the prompt's game-over mode (its sound, `dependentOnGrasp`
  for a held player), `PlaceKarmaFlowerOnDeathSpot` and the music's `DeathEvent`. The
  map press 40 ticks later goes to the death screen on the dev start's in-memory save
  state, never written (D16), as their hibernation and starving reach the sleep and
  starve screens; its CONTINUE starts a new dev game. R (MCP `respawn`) replaces the
  player with a fresh one in the camera's room, also after the body fell out of the room
  (the game has no respawn), and takes the prompt out of game-over mode
  (`TextPrompt::leave_game_over_mode`: the dependent grasp dropped, no prompt sound or
  death screen after it). In a story session (a new or
  continued campaign) R does nothing. Not ported: `setupValues.invincibility` (a dev setting) and `Die` without a
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

- **Other creatures.** Bat flies and overseers are stunned (`Stun`'s counter) and die
  (`Creature.Die`, `AbstractCreature.Die`) as in the game: the kill tag and its 200
  ticks, the slugcat's `Killing` (kill, peaceful streak, community like) and the
  respawn lists. `SocialEventRecognizer`'s listeners other than the lizards' and the
  More Slugcats and Watcher branches of `Die` are not ported.

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
What is stuck to a creature rides its vessel (`GetAllConnectedObjects`): a lizard
holding the player pulls it to the entrance and takes it in (`inShortcut`, the
lizard's vessel), the lobby realizes an unrealized destination for a vessel that
carries the player (`FollowedByCamera`) and moves the camera with it, and the
spit-out puts the player out with the lizard (D19).

**What differs:**
- **The player's vessel lives on `Player`.** The game's vessel list holds a plain
  `ShortCutVessel` for the player too; the port keeps the player's on `Player` (with
  its slot in the list) or in the lobby, and its `wait` is always 0 (Jolly co-op).
- **Not ported:** MSC tongue release, the arena hooks of `SpitOutOfShortCut` and
  den entry (`PlayerTryingToEnterDen`), dens for creatures other than flies and
  lizards (a fly's `OpportunityToEnterDen` never takes it in; a lizard's vessel at a den
  node takes it in, with what it carries), NPC transports
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
- **Other creatures:** a belly slide into a creature of collision layer 1 (a green
  lizard, D14) stuns it (2 ticks, 4 on a long slide), caps `rollCounter` at 11 and
  pushes the head on, as `Player.Collide` does; the Gourmand's roll attack ("slugsmash")
  is More Slugcats'.
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
  keeps its own flat translucent water. Light sources screen onto the surface colour
  (`GetSurfaceColor`'s light loop) as they were before the frame's drawing, as in the
  game. Still differing: splashes
  and bubbles (separate `Splash` / `Bubble` objects, not part of `Water`'s sprites) are
  not drawn, the water gates' `cosmeticLowerBorder` / `cosmeticSurfaceDisplace` and the
  DLC branches (`WaterSlush`, air pockets, inverted water, `Gutter`, ripple, brain
  mold, `WaterDepth` effect) are left out. The `_GrabTexture` the shaders read is the
  pre-level target (what LevelColor's grab pass holds in the game, INFERRED), and
  `room.waterGlitterCycle` starts at 0 when the room is loaded.

**Why.** Remix and DLC are out of the base-game scope; the rest is cosmetic or needs
objects and effects that are not ported.

**Affects.** Exact wave and swim paths; Remix / DLC water; how the water looks.

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
- **Only overseers and bat flies behave; the other creatures roam abstractly** (green
  lizards also get a body that walks and takes shortcuts, below). The `CREATURES` section is read in full (simple spawners
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
  for want of what triggers them: the creature pointer (`CreatureDangerScore` with
  every template's `dangerousToPlayer`, the aggression of the creature's AI, distance
  and hand-holding, and `CreaturePointer` pointing at the creature's `DangerPos` and
  warning the player of it, are ported; the player, bat flies and overseers score 0 as
  in the game, a realized green lizard scores with `LizardAI.CurrentPlayerAggression`
  (1 until its tracker relates to the player, then `InverseLerp(0.5, 0, LikeOfPlayer)`),
  and a pole mimic's mimicking, which hides it, is not read, pole mimics not
  being realized), the food pointer (no food
  item to the communication module is realized), the angry hologram (`angryWithPlayer` is set only when the
  Moon's swarmer neuron is eaten or carried off, 217937, and is otherwise read from the
  save; no iterator is ported), the gate scene (`RegionGate` is not ported: in Moon's gates the concern is
  `None`, as the game's is with a closed gate), the
  scavenger-trade instruction (`ScavengerTradeInstructionTrigger` is ported and waits,
  as the game's does, for a realized data pearl from its room; data pearls are never
  realized, so in GW_A22 it never fires; its controller is ported), the forced
  direction (`ReliableIggyDirection` and its `ForcedDirectionPointer` are ported, but
  no base-game room places one), and the tutorial's Eat instruction in SU_A44 (its
  fruit is not realized; SU_A42's, over the bat flies, is ported). The in-world super
  jump instruction (`SuperJumpInstruction` in SU_A13, HI_B04 and HI_A06) is ported,
  with its ghost and completion; it is not SU_A43's tutorial lesson, which the
  tutorial behaviour gives and which does not count `superJumpsShown`.
  `FoodDelicousScore` gives a seed cob the game's score without its dead and opened
  checks (seed cobs are never realized). Where the game throws (an in-world super jump
  shown over another instruction, unparsable placed data, a forced hologram without a
  direction) the port logs and goes on. A door pointer without the tutorial would follow the player with a
  `CreatureDoorDistanceTracker` (on the Fly's exit distances, which now exist): it is
  made but its update is not ported, so the pointer keeps pointing. The bat pointer's `CycleStartUp` is
  1 (the cycle's start-up ticks are not ported), and a new game has discovered no
  shelters and visited no rooms. The controls picture is always the keyboard's (the
  gamepad and Switch pictures are ported but no other control preset exists). A pointer
  whose overseer has left the room goes on updating where the game's would throw (shelter
  and direction pointers without `FORCEPOINT`): the port logs and stops there. The
  overseer's AI looks at the player, overseers, bat flies and green lizards; no other
  creature is realized. Which camera's room
  counts as viewed when `AddObject` builds the graphics module is inferred from
  `RoomCamera` and `Room.AddObject`, not checked.
  The Monk's guide light is ported (`Player.Update`'s `HologramLight.TryCreate`, the
  light's `Update` with its draws, `BringGuide(world, -0.8)` when the guide cannot
  project, the communication module dropping a light that left its room) with its
  `LightSource` (made on its first update, hard-set by its drawing, fading and
  destroyed once the light is gone; drawn in `ForegroundLights` with `LightSource`,
  and in rooms with water also in `Water` with `UnderWaterLight`, base-game path:
  no void sea, `strong`, brain mold or ripple). `Needed` and `CompleteDarkness` (the
  map discovery's) read the room's light sources; no lanterns are ported. A light
  source's rad and alpha come from its owner's drawing, as in the game; headless runs
  (MCP `step` draws nothing) leave the guide light's at their last drawn values (0
  until a frame is drawn). The light's `HoloGrid` glow and lines are drawn. The light lives in its realized room
  and goes with it; once its overseer is unrealized the light keeps its last eye
  position and counts as still the guide's (the game's stale overseer object keeps
  both). Skipped by the user for now (2026-10-16): realized data pearls (the
  scavenger-trade hint, the Scholar), `dreamsState.guideHasShownHimselfToPlayer`, Moon's
  like rising on save (`SLaiState`; no iterator is ported), and destroying an angry
  guide in the off-screen den (`OverseerAbstractAI.Moved`, until iterators are ported).
  Placed light sources (`PlacedObject.Type.LightSource` in 126 base-game rooms' settings)
  are not created, so those rooms lack their lamps' glow and `Needed` and
  `CompleteDarkness` don't see them; with them wait `LightSource`'s placed-light paths
  (blinking, `colorFromEnvironment`, `effectColor`, `nightLight`, `fadeWithSun`) and the
  colour setter's alpha quirk. Skipped by the user (2026-10-16), to be ported with the
  rest of the room effects.
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
  `OpportunityToEnterDen` (rainworld-re `creature-model.md` §10). No rain falls yet,
  but the cycle's timer runs, and lizards go home before the rain as in the game (user
  decision 2026-10-21): the realized `RainTracker` reads `TimeUntilRain` and the
  `EscapeRain` behaviour heads for the den in the cycle's last 4000 ticks; abstractly a
  lizard goes into a den it passes once the rain is within 60 s. Following through the social memory
  (`CreatureState.socialMemory`, `creature/social_memory.rs`) is ported: with no
  follow creature, `MigrationBehavior` draws one relationship and a like or hate above
  0.95 follows that creature (searched in the rooms' `creatures`, not dens); a followed
  creature is moved with (`MoveWithCreature`, `offscreenSpeedFac` 1) until the like is
  under 0.9, and a dead one's relationship is discarded (the game reads a null social
  memory there without a check; no base creature reaches it, the port logs). So far
  only a saved relationship (an imported save's tamed lizard) can start it: a lizard
  touching a player it dislikes lowers its temporary like (lizard step 6a), which
  never reaches the following threshold; taming is skipped for now, and no event
  the port produces (kills, weapon hits) touches a social memory in the base game
  without them (the only base-game direct writer is Watcher's graffiti cloud).
  Den entry is ported with what is stuck to the creature (`AbstractPhysicalObject
  .IsEnteringDen`/`IsExitingDen`, `world/dens.rs`): stuck objects are moved to the den
  and into it, a held creature dies there (a creature that would come out sooner then
  waits 200 ticks) and is dropped as `DoIwantToDropThisItemInDen` says (all for the
  plain AI and every subclass but the drop bug, which keeps its item, and the
  scavenger, which keeps everything); what stays stuck comes out with its carrier.
  The realized AI's `preyTracker.ForgetPrey` is skipped (no realized AI). The port
  keeps grip sticks and spear sticks in two lists, so it walks a creature's grip
  sticks before its spear sticks (the game, one list in creation order). An object
  taken into a den leaves the room's entities but joins no den list (the port's hold
  creatures; the game reads only creatures from them); stuck objects moved to the den
  are not realized by `ChangeRooms`. The scavengers' and drop bugs' gear stays IDs
  (`CarriedItem`): neither drops it in a den, so nothing needs it as an object yet.
  **A creature entering a realized room stays abstract** unless it is an overseer, a
  bat fly or a green lizard. The game realizes it there (`ChangeRooms`' `Realize` and
  `CreatureEnterFromAbstractRoom`, `IsExitingDen` in a realized room, `ReadyForAI`'s
  `RealizeInRoom` with `PlaceInRealizedRoom`), but no other creature has a body yet. The port keeps it in the room's
  lists as an abstract creature, as it does for every other unrealized type: its
  `AbstractCreatureAI.Update` only adds to its time buffer while the room is realized
  (the game skips `AbstractBehavior` there too), so it waits, unseen and harmless,
  until the room is abstracted, then walks on with the buffered time. None of the
  realization's draws are made (D1). `DefineNodeOfLocalCoordinate` in a room ready for
  AI chooses the node as the game does: by the creature's exit distances from a tile
  an exit is reachable from, else by `QuickConnectivity` there and back.
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
  and a spore plant stay abstract (not realized, a user decision of 2026-10-16; the
  spore plant lacks its consumable fields). The player picks them up, carries them (through shortcuts and room exits
  too), swallows rocks and throws them (D3); thrown spears stick into walls (the room
  edge ring, `NoSpearStickZone`s and a moving shelter door excepted) and hit bat flies,
  overseers and green lizards (D3; spears lodge in lizards). Not yet: items falling
  between rooms
  (an item below the room is clamped at -1200 px as the game does for objects; only
  creatures are destroyed there); and objects of other kinds (placed food,
  pearls, ...).
  Arena levels (single-room worlds) make items as a story room does (the arena's
  `levelItems` setting is not ported). `GeneratePopulation`'s reset clears a room's
  `creatures` but not its `entities`, as the game does.
- **Green lizards (lizard steps 3 and 4 of 9: the body and movement).** A green lizard is realized as the
  game's does (`ReadyForAI`'s `RealizeInRoom`, `PlaceInRealizedRoom` with
  `RandomizeSpawnPositionInRoom` and the `QuickPathFinder`, a den exit into a realized
  room through the lobby and the pipe, `SpitOutOfShortCut`) and abstracted with its room
  or toward an unrealized one (`rainworld-re` `creature-model.md` section 13). Its body
  is `Lizard`'s with `LizardState`: the constructor (green breed data; the other breeds
  stay abstract until their data is entered), `Update`'s body part (bite delay, jaw,
  the three health rolls, the rock's turn, `Creature.Update`'s health death roll and
  fall out of the room, `PhysicalObject.Update`, the lungs), `Act`'s footing, terrain
  impacts with their sounds, `Violence` with the head shield and hits in the mouth,
  `Stun`, `SpearStick`, `Die`, and the bites (`AttemptBite`, `Bite`, `DamageAttack`,
  `JawsSnapShut`, `CarryObject`; `Collide` sees the creature it touches and bites what
  it would kill by its dynamic relationship). Rocks and spears
  thrown by the player hit it through collision layer 1 (`Rock.HitSomething`,
  `Spear.HitSomething`, `LodgeInCreature`, a spear stuck in a creature and pinned to a
  wall), kill it with the kill tag (the session's kills and the sleep screen, D13), and
  its state saves with its throat health. It moves as the game's (lizard step 4,
  `creature-model.md` section 14): the `PathFinder` with `LizardPather`'s costs and
  `FollowPath`, the shared accessibility mappers and the resource divider, the stuck and
  discomfort trackers, `Act` with `FollowConnection`, the movement animations,
  `GripPointBehavior`, `SwimBehavior`, `GetFrameSpeed`, the wiggle, desperation and
  shake, `ActAnimation`'s and `EnterAnimation`'s physics, entering shortcuts and riding
  them as a three-segment vessel (NPC transport included) into realized and abstract
  rooms. Its `LizardAI` decides as the game's (lizard step 7a, `rainworld-re`
  `notes/lizard-ai.md`): `Update` with the moods from the `UtilityComparer`,
  `DetermineBehavior` over the threat, prey, rain, injury, aggression, mission and
  noise trackers, the `DenFinder` (with `NewArea`'s remap), and the bodies of Idle
  (`IdleSpotScore`, `ComfortableIdlePosition`), Travelling, EscapeRain, ReturnPrey,
  Injured, Frustrated and InvestigateSound with their `runSpeed`, draws and voices
  (boredom, pain, frustration), `ReactToNoise`, and `WantToStayInDenUntilEndOfCycle`.
  Hunt and Flee are the game's (step 7b, `lizard-ai.md` section 9): `AggressiveBehavior`
  (approach, jaw, the lounge decision, `AttemptBite`, `JawsSnapShut`), the blood-lust
  voice, `BitCreature`, the PrepareToLounge/Lounge/ThreatSpotted animations' focus
  terms, `ThreatTracker.FleeTo` (with `EvaluateFlightDestThreat` and
  `QuickConnectivity.QuickPath`), the flee `lookPos` and `RainWorldGame.RequestHeavyAi`
  (a game field, not the game's statics; nothing else reads them). `GenerateRandomPath`
  is ported though nothing in the game calls it. A green lizard fears no creature the
  port realizes, so Flee is reached only in tests. Lizards among themselves are the
  game's (step 7c, `lizard-ai.md` section 11): the Fighting body (the aggression
  target as the focus, the fighting stance and its lean toward the focus in
  `ActAnimation`, the aggression signal, `AggressiveBehavior`), `SendCommunication`
  and `RecieveCommunication` (the dominance display's draw, the dominance and
  submission voices, the anger raised or settled, `submittedTo`), a rival's anger on
  touch (`IncrementAnger` in `Collide`), `MissionTracker` with its `LeaveRoom` mission
  and the ActingOutMission body (`LeaveRoom.Act` calls `FindMostAttractiveExit` while
  the lizard fears something). Lizard bites and `DamageAttack` on a rival are the
  shared ones (step 7b/8; a rival takes no damage from `DamageAttack`). Where the game
  throws on these paths the port logs a warning, stops that call and goes on with the
  tick (divergence; the user's decision, 2026-10-22: these throws stay modelled as
  stop-that-call-and-go-on with a warning): a submitting lizard
  with no representation of the sender (`RecieveCommunication`'s
  `rep.representedCreature`, a NullReferenceException), `LeaveRoom.Act` in a room
  without connections (`connections[-1]`, an IndexOutOfRangeException) and its random
  connection being -1 (`World.GetAbstractRoom(-1)` is null, then `.index`, a
  NullReferenceException). In the game no try/catch stands between `RainWorld.Update`
  and the throw (`ProcessManager.Update`, `RainWorldGame.RawUpdate`,
  `MainLoopProcess.RawUpdate`, `RainWorldGame.Update`, `Room.Update`, `Lizard.Update`,
  `Act`, `LizardAI.Update`), so the rest of that frame is lost: the rest of the
  lizard's update with `base.Update`, the room's remaining objects, the other rooms, the
  rest of `RainWorldGame.Update`, `myTimeStacker--` and `GrafUpdate`; Unity logs the
  exception and the next frame catches up. The port keeps the mission that `AddMission`
  replaced as the one acted on until the tracker's next update, as the game's
  `mostRelevantMission` reference does. Left out with their DLCs: safari control in the
  fighting stance's lean (`!safariControlled`) and in `ActAnimation`'s safari animation
  exit, Watcher's ripple layer in `VisualContact(BodyChunk)`, Remix's zero-gravity
  `IsWallClimber` and DLCShared's eel lizard as a wall climber. The tongue branch of
  `AggressiveBehavior` belongs to tongued breeds (none entered). Left out with their
  DLCs or features (step 7b): in `AggressiveBehavior`, Remix's friend bite
  (`AttemptBite(null)` on a liked friend), DLCShared's SpitLizard jaw clamp, the
  `safariControlled` early return and Watcher's `ApplyLocustDeterrent` in the tongue
  clause; safari control in the Hunt body's blood-lust test (`!safariControlled`, the
  draw is still made; the Flee body has no safari test of its own); More Slugcats' `LickedByPlayer` sighting at
  the top of `Update`; Watcher's indigo lizard locust branch. The friend tracker
  (taming, skipped) has utility 0 and `CareAboutRain` true.
  It sees and tracks creatures (lizard step 6a, `creature-model.md` section 17):
  the `Tracker` with simple and elaborate representations and their ghosts,
  `VisualContact` with `LizardAI.VisualScore` (the breed's vision, water, narrow
  spaces, the angle off the head) and `BodyChunk`/`Player.VisibilityBonus`
  (`privSneak`), `SeeCreature` from `Collide`, the `RelationshipTracker` with
  `UpdateDynamicRelationship` (a slugcat by `LikeOfPlayer`: the region's lizard
  community opinion, spawn evil and the temporary like; the spear it holds), and the
  casual aggression that bites a target in reach. The relationship tracker sorts each
  relationship into the prey, threat or aggression tracker (step 7a). Left out of step 6a, each
  a question to the user: what blinds (`Blinded`, flashes) and the vision obscurers
  (`VisionObscurer` objects), `useTrackedCreatures` (no base lizard sets it), gifts and
  taming (`FriendTracker`, skipped), the black lizard's `HeadLightsUpFromNoise` and the
  white lizard's camouflage in `VisibilityBonus` (with those breeds), and the Remix,
  Jolly (`friendlyLizards`), Expedition, More Slugcats and Watcher branches of
  `UpdateDynamicRelationship`, `VisualScore` and `LikeOfPlayer` (with those DLCs). A
  kill's community influence is applied to the session's communities at once, as the
  game's `InfluenceLikeOfPlayer` does, and again to the saved ones at `Win`.
  It hears (lizard step 6b, `creature-model.md` section 18): the `NoiseTracker` with
  its sources, mysterious noises and sound to examine, `HeardThisCreature` moving or
  cloning ghosts, `hearingSkill` from the run speed; noises from the player's
  footsteps and the terrain impacts of the player, lizards, flies, rocks and spears
  (`BodyChunk.loudness`, the player's from `Sneak`); `SocialEventRecognizer`'s
  `Killing` and `WeaponAttack` (hits and near misses) change its and its community's
  like of the player (`LizardPlayerRelationChange`); the path finder's
  `TravelPreference` adds `DiscomfortOfTile`; `ReactToNoise` and `InvestigateSound`
  answer what it hears (step 7a). Left out of step 6b, each a question
  to the user: `Deaf` and `room.BackgroundNoise` (nothing ported sets them), the
  noise sources whose objects are not ported (explosions, firecrackers, scavenger
  bombs, egg bugs, Miros birds, snails, the explosive spear's blast, karmic armor,
  vultures, lightning, other creatures' impacts), the item tracker's
  uncomfortable items in `DiscomfortOfTile`, and the player's zero-gravity loudness
  (the port has no `ZeroG` body mode). The MCP `noise.heard` log is the port's, not
  the game's. The Watcher ripple layer test, Remix's water splash noises and the More
  Slugcats noise sources are left out with their DLCs.
  The MCP `set_destination` tool sets a destination and a run speed the AI keeps, the
  behaviours' bodies skipped meanwhile (D18): a dev override, not game behaviour. `LegsGripping` and `NoGripCounter` come from its
  graphics' legs in a viewed room (lizard step 5a, D8), and are 2 and 0 without a
  graphics module, as in the game. The travel costs have the threat terms (step 7a;
  the obstacle term is dead code in the game, which never makes an `ObstacleTracker`).
  Not ported: the yellow lizards' pack term of the travel preference and the
  salamander's `IdleSpotScore` taste (with those breeds),
  `nonShortcutRoomEntrancePos` (entering a room from its border), and `PathWithExits`
  and `BestRegionTransportationGoal`, which nothing calls yet. The tongue, spit and
  jump animations belong to breeds whose data is not entered; a green lizard never
  enters them, and the port goes back to Standard if one does. Not yet, with their
  steps: eating a dead lizard (its own step after 7c, user 2026-10-21: the player cannot grab creatures
  yet), the drowning bubble (its draws
  are made, the object is not), lethal water and the rain's flood (no rain yet). The
  `kill_green_lizards` tutorial message is the game's: it marks the state dead without
  touching a realized body. More Slugcats', Watcher's and Remix's lizard branches
  (rot, the other DLC breeds, `LizardState.rotType` behaviour) are left out with their
  DLCs.
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
  ported: the rain's
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
  them, its decoded distances shift; the port decodes them as baked), Remix's
  zero-gravity lizard branches and threaded mapper, Downpour's
  `TerrainManager` curved floors and sand, More Slugcats' `daddyCorruptionImmune`
  branch, and `ModifyTileWaterLevels` (WaterCutoff and
  AirPocket objects, which no vanilla region room places). Remix's
  `BringToRoomAndGuidePlayer` branch of the world
  process's update and More Slugcats' goal rooms and guide symbols are left out.
  Not ported: the scavengers' abstract AI (`ScavengerAbstractAI`: squads, outposts,
  traders, kill squads, re-gearing) and `ScavengersWorldAI`. `CreatureCommunities`
  is the session's copy on the world: a kill's or a lizard's `InfluenceLikeOfPlayer`
  changes it at once and is recorded and applied to the save at `Win` (D16). `NewWorld` (a region change) is not reachable (no gates).
- **Hologram graphics.** A hologram's colour base (`Color.Lerp(MainColor,
  ColorOfSegment(0, 0), 0.5)`) is taken at its overseer's last update (holograms update
  before their overseer, so it is the one the game reads). Not drawn: the
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
- **Overseers hit and killed** (D3). Hearing of a flying weapon, being stunned, the
  guide's lost like (`FlyingWeapon`, `HitByWeapon`), `Overseer.Die` with the guide's
  return in the off-screen den, the dead overseer's six ticks of dying and its graphics'
  fade are ported. Differences: a dying overseer's `OverseerEffect`s are simulated but
  not drawn (the `OverseerZip` shader is not ported), and its effects and sparks start
  on the straight line from root to eye, where the game places them on the drawn
  body's segments; `ApplyForceOnAppendage` and the appendage cross test use the
  simulated root and eye, not the graphics' segments. Not ported: `OverseerCarcass`
  (the dead overseer's falling eye, its `GetNewID` and constructor draws) and the
  `ShockWave` of its death.
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
  guide's tutorial pins the timer; once fewer than 2400 ticks are left, as after the
  SU_A43 tutorial's bring-in-rain jump, `MusicAllowed` stays false for good because
  no rain ends the cycle, so threat music is off in rooms whose `DangerType` isn't None),
  karma gates, `DisabledMapRooms` beyond the tag drop, and Remix's
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
  read as 0, so Shoreline's image triggers fire as for a player who never met her
  (`EverMetMoon`, which gates the image triggers and the gate scene, is read from the
  save's `SLaiState`). An image stands at the room's `ProjectedImagePosition` placed
  objects as the game places it (`DisplayPosScore`, its `Range(0, 25)` jitter draw). The More
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
  the next cycle. The dream screen (`DreamScreen`) is not: a stand-in fades the music
  as its constructor does (`FadeOutAllSongs(30)`), shows 40 black
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
- **The AI map after opening.** As in the game, a door that has opened runs a new
  `AImapper` over the room, 100 steps a tick, and then gives the room's AI map its new
  tiles (the baked visibility is gone from them, as in the game) and resets every path
  finder in the room (`UpdatePathfindingCreatures`). The room does not change while it
  runs, so the port maps it at the first step and hands it over when the game's step
  count (`2 * width * height` plus the terrain-proximity steps) is reached.
- **Objects in shelters.** Rocks and spears in any shelter when `Win` saves, and spears
  stuck in walls anywhere, are written to the region state and come back the next cycle
  (D16) and are realized in their rooms (D14).
- **Kills.** A cycle's kills, its respawn and wait-respawn spawners and its community
  influences reach the save only through `Win` (D16), as in the game; a death or a
  quit drops them.
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
- **The Hunter out of cycles.** Of `GoToRedsGameOver` only the music's
  `FadeOutAllSongs(20)` is ported (from `GameOver`, a starve and the death screen's
  request); the illness fade, `redsDeath`, the statistics screen and its save are not:
  `GameOver` returns with no prompt, so the pause menu's exit is the way out, and a
  starve (without its `DeathEvent`) or a door closing on a dead Hunter out of cycles
  saves an ordinary death.
- **Not ported:** More Slugcats' ancient and broken shelters, `PrecycleModule`, the
  shelter tutorials, the dream ghost, `GetStuffInWalls`, the door's screen shake,
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

## D19: The player held by a creature (lizard step 8)

**What.** A green lizard's bite acts on the player as in the game
(`rainworld-re/notes/lizard-ai.md` section 10): `Creature.Grab` checks
`CanBeGrabbed` (`cantBeGrabbedCounter`, 30 after `SpitOutOfShortCut`), `Player.Grabbed`
lets the body through floors and makes a lizard's grasp the `dangerGrasp`; the bite's
`Violence` (1.5 damage, Bite) stuns 45 and kills (`instantDeathDamageLimit` 1) with the
lizard's kill tag, the hands letting go at once; a pacifying grasp holds the stun at 10
(`Creature.Update`). `Player.Update`'s `dangerGrasp` block counts `dangerGraspTime`: until
30, alive and stunned under 30, a fresh throw press (read from the controls, not the
stunned neutral input) runs `ThrowToGetFree` (a spear, or a rock half the time, thrown at
the grabber's nearest chunk as `meleeHitChunk`) and a fresh pick-up press
`DangerGraspPickup`; 30 drops what the hands hold; 60 calls `GameOver(dangerGrasp)`, whose
prompt (`dependentOnGrasp`) ends if the living player gets free. `Player.Stun` keeps the
hands' objects under a danger grasp; a held head near the room bottom makes every grasp
let go; `WALLSTUCK`, the karma flower spot and map discovery need a free body. With
graphics, what the jaws hold draws inside the lizard's head container
(`AddObjectToInternalContainer`; released on `ReleaseGrasp`, `LoseAllGrasps`,
`Grasp.Release`; re-added on `SpitOutOfShortCut`); released sprites go back on top of
their default containers (`MoveObjectToContainer(obj, null)`, `ReleaseAll...` from the
last entry to the first), and objects that join the room after that draw above them.
A lizard carrying the held player into a shortcut takes it along (`Creature.Update`'s
pull, `SuckedIntoShortCut`, `FollowedByCamera` in the lobby, `SpitOutOfShortCut`'s
connected objects with `Player.SpitOutOfShortCut(false)`), into its den too, where
`IsEnteringDen` kills it (`Player.Die`: `GameOver`, `UI_Slugcat_Die`).
`HardSetPosition` moves a pending `setPos`; a held body is not clamped to the room's
range; `PhysicalObject.Update` prunes stale `grabbedBy` entries (the player and items).

**What differs:**
- **Danger grabbers.** Only lizards realize; `Player.Grabbed`'s other base-game danger
  grabbers (vulture, big spider, drop bug) have no bodies yet, and Watcher's sand grub is
  DLC.
- **Draw order under the released sprites.** Objects that did not go through a
  release keep the port's order by kind (shelter door, items, overseers, flies,
  lizards; the slugcat first in `Midground`), which stands for the game's container
  order; only the released objects, and objects that joined the room after a release,
  are placed as the game's `AddChild` would place them (above the rest). Overseers
  joining after a release stay in the kind order.
- **Not ported:** the Hunter's spear on the back in `DangerGraspPickup` (`SpearToHand`,
  D3).
- **Remix branch omitted:** `ReleaseDoorForbiddenCreatures` in `SuckedIntoShortCut`
  (a grabbed creature whose template forbids shortcuts, or an immobile one, and the
  grasps between it and what enters are let go); the game calls it only with
  `ModManager.MMF`.
- **DLC branches omitted:** a player AI's (slugpup's) nervous struggle draws, the
  Challenge module's player number, `slugOnBack.DropSlug` (More Slugcats, Jolly
  Co-op); Remix's `cfgJetfishItemProtection` in `Stun`; Saint's and Expedition's spear in
  `ThrowToGetFree`; Jolly Co-op's `JollyGameOverEvaluator`, the game-over sound and the
  prompt's other players; More Slugcats' `ArtificerDreamEnd`; Watcher's `IsHidden`
  (camouflage, one-way placement, final warp: always false here, so `VisualContact`'s
  hidden-player clause never fires) and its soft toys in `TossObject`.

**Why.** Base-game scope; the other grabbers come with their creatures, the Hunter's
spear with its own step.

**Affects.** Escapes and deaths in lizards' jaws in co-op, DLC and with other creatures;
the draw order of objects that never went through a jaws release.

## Not ported yet (not divergences, just absent)

Player (stage one leaves out): the zero-gravity `privSneak` and chunk `loudness` (no `ZeroG` body mode), zero gravity, grabbing and eating anything but bat flies, rocks and spears, thrown
weapons hitting creatures other than bat flies and overseers (D3), adrenaline, Red's illness, vines, tube worm,
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
