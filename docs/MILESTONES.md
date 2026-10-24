# Milestones

Each milestone is a playable or checkable state, accepted by the user's hand
review in the game (see `AGENTS.md`, Testing). A milestone is done when its
exit check holds; leftovers move to `OPEN-ITEMS.md` or the next milestone.
Detail lives in the surveys in `../rainworld-re/surveys/`, deviations in
`divergences/`, per-type progress in `PORT-STATUS.md`.

Status: done, current, planned.

## Done

**M1 - Front end and campaign selection** (done)
Boot, intro roll, main menu, options, slugcat select with a saved campaign's
page, the intro slideshows, the game's fonts, sounds and menu music.
Exit check: boot to the slugcat select and start a campaign.

**M2 - Room loading, rendering and the player controller** (done)
Rooms and worlds read from the installed game, the level image and palette,
water, the slugcat's movement, physics and graphics, corridors and shortcuts.
Exit check: run, jump, climb, swim and take pipes in any room of the start region.

**M3 - A region you can walk through** (done)
A whole region loads, room-exit pipes carry the player between rooms, the camera
follows, sound effects, ambience and trigger songs, the HUD (food, karma, rain
meters, text prompt, region map).
Exit check: walk SU from the start den through several rooms with the map filling in.

**M4 - Cycle loop: saves, shelters, sleep, death** (done)
The game's save format in the port's own directory, importing the real game's
saves, shelters and hibernation, the sleep, death and starve screens, karma,
malnourishment, the SU_C04 opening script.
Exit check: sleep in a shelter, die, starve, continue each from the saved state.

**M5 - Items, overseers and bat flies** (done)
Rocks and spears (throw, stick, swallow), the creature framework (population,
abstract AI, den logic), overseers with the guide's tutorial, bat flies with
catching, carrying and eating, the Eat and super-jump lessons.
Exit check: play the SU_A42 to SU_A43 tutorial through.

**M6 - Green lizards** (done; `surveys/lizards.md`, steps S1 to S9)
Done: S1 AI map, S2 abstract AI for the other creatures, S3 body, violence and
death, S4 path finder and movement, S5 graphics and voice, S6a tracker, vision and
relationships, S6b hearing, noises and social influences, S7a decisions and calm
behaviours, S7b hunting and fleeing, S8 the player grabbed by a lizard (death by bite,
escape, game over on the grasp), S7c lizard fights, communication and missions, the
player dragging dead lizards (through shortcuts, into shelters) and the Hunter eating
them, S9 lineages and acceptance (every death path to `AddCreatureToRespawn`, the
respawn lists across cycles and saves, a progressed lineage's next stage, lizards in
shelters, the SU census 11/10/8 checked).
Exit check: a green lizard hunts, bites and flees as in the game, and the player
can be killed by one, across a cycle.

**M8 - Rain and the cycle's end** (done, accepted in the user's play 2026-10-24)
The rain cycle's rain, flood and rain death, in steps R0+R1 (done), R2 (done: the room rain's push, stun and
kill, `rainDeath`, D45), R3 (done: the `DeathRain` shader, splash sprites and the seven rain loops, D46), R4 (done: the flood's water, `AddWater`, D47), R5 (done: the bullet rain, D50)
(`../rainworld-re/surveys/rain/`). The threat music's rain inputs are already ported.
Waking up in the rain is More Slugcats' precycle: with the DLC work (user, 2026-10-24).
Exit check: after the cycle's end the rain falls, pushes, stuns and kills, rooms flood and
the bullet rain comes in its cycles, as in the game.

## Current

**M7 - Room visual effects, lights and camera shake** (current; `surveys/room-effects/`,
`surveys/lights-camera-shake.md`)
Every base-game room visual effect (level shader and palette machinery, full-screen
effects, cosmetic insects, lightning, decals, particles, background scenes, then the
gameplay effects: electric death, border push-back, zero gravity, the SS superstructure),
placed lights and light fixtures, and camera shake. Effects no base room uses
and lights owned by unported things come later with their owners or DLCs. The void
effects (`VoidMelt`, `VoidSpawn`, `VoidSea`) wait until the ascension ending can be
reached (user, 2026-10-23).
Exit check: rooms look and shake as in the game.

**M9 - Gates and region changes** (current; survey `surveys/m9/` in progress)
Region gates and the karma gate rules, moving between regions, per-region state
across the world; the debug UI's Region tab.

## Planned (order proposed, to decide)

**M10 - Creature bodies, wave 1**
The creatures that matter next in the early regions, each as a body, AI, graphics
and sound step like M6 (which first is a user decision): e.g. the other lizard
colours, cicadas, centipedes, scavengers, vultures, needle worms, drop bugs.

**M11 - Creature bodies, wave 2 and the rest of base game**
The remaining base-game creatures, items, plants and room objects, dreams and
the PASSAGE screens, and the other starting slugcats' mechanics.

**M12 - Whole base game playable**
The base-game campaigns playable start to finish with Remix off.
Includes the SS swarmers (`SSSwarmers` effect, `SSOracleSwarmer`: edible, carried, feeding on
mycelia, leaving through exits; user, 2026-10-23, moved out of M7 step 10e).
Also the rot (`DaddyCorruption`: bulbs, eyes, darkness, grabbing and eating creatures,
the corruption tubes as climbable vines with `Rope`; user, 2026-10-23, moved out of M7
step 10f); its `StuckDaddy` with DaddyLongLegs.
Also the Hunter follow-ups: `HardmodeStart` (the scripted LF_H01 first cycle) and the
`sprOnBackStick` in the region state's `STICKS` (user, 2026-10-23).

**M13 - Remix and the DLCs**
Remix's menu and options, More Slugcats, Watcher and Jolly Co-op, each taking effect
as in the game (omitted DLC branches are listed in `divergences/`).

**M14 - Cleanup, review and optimisation**
A code cleanliness and quality review across the workspace, one crate at a time,
then an optimisation pass. Starts from the end-of-project items in `OPEN-ITEMS.md`
(`rw-hud` screens, `rw-sim` visibility tightening) and its
optimisation leads (headless speed, rooms with many sprites such as SS_E05).

**M15 - Modding API**
A built-in scripting language (Wren, Lua or another) replacing C# mod loading.

**M16 - Android port**
Touch input, getting the game files onto the device, the platform seams already
kept in the windowing, path and input code.
