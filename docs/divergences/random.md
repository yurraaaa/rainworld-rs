# Divergences: rw-random

See [README](README.md) for the format. IDs are global; code and notes cite them.

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
- **Background scenes** (D32): the shared stream gets a `DistantLightning`'s timers every
  tick (per state: `randomLevelChange` `Range(1, 6)` and `value`; `Reset` 3; `tinyThunderWait`
  `Range(10, 80)` then `Range(5, wait)`; tiny thunder 1 per tick), the 4 `Smoke` draws of
  `RoofTopView`'s constructor, and the `DustpuffSpawner`'s and each `DustPuff`'s draws. The
  clouds' `randomOffset` runs on the private `InitState(12)` stream (state restored) and the
  roof's rubble on a `SeededRandom`; `LightIntensity` (1 or 3 draws) is on the cosmetic stream,
  drawn in the sprite leaser's (add) order.
- **Call order.** The game interleaves draws from every other object, sound, particle,
  rain, camera and AI in the room and world; the port has only the realized rooms'
  water and the player, so its sequence is the game's only for those draws. Sounds of
  rooms the camera is not in make no draws, as in the game (`Room.PlaySound` only
  reaches the microphone of a camera in that room). Draws of unported code in
  the same functions (the rain-warning look-up,
  grasps of objects other than bat flies,
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
  (`rainworld-re/notes/creature-model/overseers-and-flies.md`). Every tick `FliesWorldAI.Update` makes its
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
  search and migrate-active counter), and `MigrationBehavior`'s `Range(0, count)` over
  the social memory of a creature that has relationships and follows no one (only
  saved relationships exist yet: nothing in the port makes one). Den entry
  (`IsEnteringDen`'s killing of carried prey) draws nothing unless the prey's death
  progresses a lineage (`AddCreatureToRespawn`). A region loaded cycles after it was
  last updated ticks each creature's state once (`RainCycleTick`): a living centipede
  with saved shells rolls `value < 0.2` per shell and, when it passes, one more for
  the shell. From the first creature out of its den (100 to
  1000 ticks into a cycle) every story run's stream has shifted from before, as the
  game's does. Missing: the draws a creature other than a bat fly, an overseer or a
  green lizard would make on being realized (`PlaceInRealizedRoom`, its body, AI and
  graphics), as it stays abstract (D14), and
  per tick the scavengers' abstract AI and `ScavengersWorldAI` (one `Range` over the
  scavengers every tick once a world has any), which are not ported (D14).
  `LocalCoordinateOfNode` draws for bat hives, garbage holes and border exits (a border
  exit's tile is one of its mapped border tiles).
  A realized green lizard makes its movement's draws (lizard step 4): its path
  finder's cells (two each, made for every node of the world and every tile of each
  room it enters), the AI's abstract-behaviour roll, `AwayFromForbiddenEntrance`,
  `NewRoom`'s random destination tile, the bubble roll, `GetFrameSpeed`,
  `MovementAnimation` (three per destination a tick), `GripPointBehavior`, the random
  step and the desperate shove, the shake, `EnterAnimation`'s and the shortcut sounds,
  and `DefineNodeOfLocalCoordinate`'s `LocalCoordinateOfNode` draws in a room ready for
  AI. Its tracker and relationship tracker (lizard step 6a) make theirs: the casual
  aggression's representation pick, `DoIWantToBiteThisCreature`'s rolls and the bite
  coin, `Tracker.Update`'s representation check (a ghost pick, a ghost pushed on with
  its connection rolls), its room-creature pick, each new elaborate representation's
  first `Ghost` (`moveBuffer`), the ghosts' moves and clones, and `CreatureSpotted`'s
  look reevaluation and eye pick in a viewed room.
  In a viewed room a green lizard has its `LizardGraphics` (lizard step 5a), whose
  `Update` runs after the lizard's in its slot of the update list (and once more while
  it is pulled into a shortcut) and makes the game's draws: culling's `Reset` (12),
  `showDominance`'s (1), the `CreatureLooker` focus roll (1, then 1 more and 1 or 2 for
  `LookAtNothing`; 1 per spotted animation), the eyes (4), the blink (1, or 2 during a
  stun flicker), the tail direction (1, and 2 when it changes), the tail's dominance
  shake (2 per segment while `showDominance` is above 0, unless a conscious lizard
  swims), each limb's health rolls
  and foot sounds, and the bubbles (11 each: a lunge, a threat display, a jumping head,
  `bubble` ticks or a voice shown above 0.5, a hard landing's 1 + 12 each), a lunging
  green lizard's drips and the voice's body shake (3 while it shows above 0.2). Its
  cosmetics draw only while `showDominance` is above 0 (after a lizard's dominance
  signal, lizard step 7c). Its
  `LizardVoice` (lizard step 5b) draws the intensity of each `MakeSound` without one
  (1, whether or not it sounds) and, in the camera's room, the microphone's 7 each time
  its sound is made (again whenever the clip has played out while the emotion lasts);
  its constructor draws on the ID's seed. Its
  constructor draws on a stream seeded by the creature's ID, the global one restored, as
  the game's does. A spear lodging in it with graphics rolls for the body part (1, then
  the part's), the head shield's sparks draw 4 each, and the sick lizard's white flicker
  is drawn. `LizardGraphics.DrawSprites`' draws (the ready jaw's twitch, the head and
  threatened body's jitter, the flicker colour, the every-second-draw toggle) and the
  head shield flash's come from the app's cosmetic stream, for the same per-frame
  reason as the holograms'. A lizard in a room nobody views has no graphics module and
  makes none of these draws, as in the game.
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
  switch's delay ends, and `ProceduralMusic.Reset`'s stem choice (`Random.Range` per
  non-empty threat layer, 5 for SU, whatever the threat): when `NewRegion` builds the
  bank (the first room of a game, or once the old piece faded), 200 silent updates
  later (the first day/night reading), and after each later re-scramble (an audible
  spell, a new subregion, nightfall). No draws are made without a music player
  (`musicVolume` 0). `ThreatDetermination` draws nothing. The Monk's `HologramLight`
  draws three to nine a tick in its slot of the room's update list (its `DrawSprites`
  draws come from the app's cosmetic stream, per frame).
- **Slime molds and dead token stalks.** `Room.Loaded`'s `CosmeticSlimeMold`s, the
  plain `SlimeMold`'s `SeededRandom` gate and the `DeadTokenStalk`'s constructor draw
  under `InitState` with the global state restored, so none on the shared stream. Each
  dead stalk draws 2 a tick in its slot of the room's update list (`sinCounter`, the
  new-pose roll), 4 more on a new pose; its lamp's `DrawSprites` draw comes from the
  app's cosmetic stream, per frame.
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
- **Camera shake.** `RoomCamera.Update`'s jump draws 1 (`Random.value * 360`) each
  tick while `screenShake` is above 0, at the head of the sim tick, before the room
  realizer, as the game's cameras update before the rooms. `Room.ScreenMovement` draws
  its ceiling drips where the caller moves the screen (a terrain impact before its
  noise): 2 per drip (tile, x jitter) and, for a drip a camera sees within 300 px, 2
  more (x speed, `WaterDrip`'s life), viewed or not; only a bump and shake above 2
  (`|bump| / 4 + 3 shake`) drips, never a `HitWall` or a shelter door. `DrawUpdate`'s
  micro shake (2 per drawn frame while `microShake` is above 0) comes from the app's
  cosmetic stream, per frame; a paused frame repeats the last one's (the menus' pause
  divergence), where `PausedDrawUpdate` would draw a new offset each frame.
- **Lightning.** `Lightning.Update` draws on the shared stream in the room's update
  list (7 per `Reset`, 2 per `randomLevelChange`, 1 at a background source's flash
  start, 1 per source with `CycleEndIntensityBoost`, 7 per sound played); the
  camera's `LightIntensity` flicker draws come from the app's cosmetic stream, per
  frame (D25).
- **Ambient drips.** `Room.Update`'s ceiling drip, after the water's update, in camera
  0's room (not a shelter, danger type not `None`, with ceiling tiles): 2 draws a tick
  (`value < gravity`, then the chance test), 5 with a drip (tile, x jitter,
  `WaterDrip`'s life); each `WaterDrip` draws 1 a tick (`randomLightness`), its sprites
  on the cosmetic stream (D1). `DrippingSound`'s loop makes its sound draws once when
  it starts in the camera's room, and again whenever its unheard emitter died and the
  camera is back.
- **Water surface loop.** `Water.Update` starts with `waterSounds.Update()`: in a
  water room viewed by camera 0 the loop's start makes the sound draws once
  (`Water_Surface_Calm_LOOP`: one sample, 7 draws), again whenever its unheard emitter
  died and the camera is back, and never in an unviewed room. Fully `SilenceWater`ed
  (no base-game room) it still starts once at volume 1.
- **Weapons hitting creatures.** An overseer hearing of a flying weapon
  (`OverseerAI.FlyingWeapon`) draws once; a rock hitting a fly draws once (kill or
  stun), one hitting an overseer (no small creature) or a spear none; `Overseer.HitByWeapon` draws `Range(200, 1500)` when the
  guide goes away; a killed lineage's `ChanceToProgress` draws once below its last
  stage; a dead overseer draws four a tick and `Overseer_Death`'s sound draws, and in
  view its graphics' ten `OverseerEffect`s (three each, then two a tick each) and three
  sparks; a guide killed out of view takes a new ID (`GetNewID`, no draw). The order is
  the game's (`rainworld-re/notes/items.md` section 12).
- **Placed spears and items.** `Room.Loaded`'s story `MultiplayerItem`s draw once
  against their chance (on every realization) and take an ID, then on a first
  realization each `ReliableSpear` takes an ID (no draw), before the random items.
- **Overseers.** A realized overseer's update makes the game's draws in order
  (`Overseer.Update`, `OverseerAI` with its communication module and the guide's
  tutorial, then `OverseerGraphics.Update` while its room is viewed;
  `rainworld-re/notes/overseer.md`), and so does each of its holograms' `Update` in its
  own slot of the room's update list. `ReevaluateConcern`'s creature draw is over the
  room's abstract creatures and its object draw over the room's whole entity list in
  the game's order (creatures, then objects; `World::room_entities`), so a room with
  rocks or spears takes the game's draws.
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
