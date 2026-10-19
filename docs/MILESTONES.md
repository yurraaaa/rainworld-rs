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

## Current

**M6 - Green lizards** (current; `surveys/lizards.md`, steps S1 to S9)
Done: S1 AI map, S2 abstract AI for the other creatures, S3 body, violence and
death, S4 path finder and movement, S5 graphics and voice, S6a tracker, vision and
relationships, S6b hearing, noises and social influences.
Left: S7 the lizard's decision layer (prey, threat, aggression, den finder, utility,
behaviours), S8 the player grabbed by a lizard (death by bite, escape), S9 lineage
plumbing and acceptance (respawn, lizards in shelters, saved lineage).
Exit check: a green lizard hunts, bites and flees as in the game, and the player
can be killed by one, across a cycle.

## Planned (order proposed, to decide)

**M7 - Rain and the cycle's end**
The rain cycle's rain, flood and rain death; waking up in the rain; the threat
music layers' triggers.

**M8 - Gates and region changes**
Region gates and the karma gate rules, moving between regions, per-region state
across the world; the debug UI's Region tab.

**M9 - Creature bodies, wave 1**
The creatures that matter next in the early regions, each as a body, AI, graphics
and sound step like M6 (which first is a user decision): e.g. the other lizard
colours, cicadas, centipedes, scavengers, vultures, needle worms, drop bugs.

**M10 - Creature bodies, wave 2 and the rest of base game**
The remaining base-game creatures, items, plants and room objects, dreams and
the PASSAGE screens, and the other starting slugcats' mechanics.

**M11 - Whole base game playable**
The base-game campaigns playable start to finish with Remix off.

**M12 - Remix and the DLCs**
Remix's menu and options, More Slugcats, Watcher and Jolly Co-op, each taking effect
as in the game (omitted DLC branches are listed in `divergences/`).

**M13 - Cleanup, review and optimisation**
A code cleanliness and quality review across the workspace, one crate at a time,
then an optimisation pass. Starts from the end-of-project items in `OPEN-ITEMS.md`
(`rw-hud` screens, `cosmetic_graphics.rs`, the `rw-sim` file split).

**M14 - Modding API**
A built-in scripting language (Wren, Lua or another) replacing C# mod loading.

**M15 - Android port**
Touch input, getting the game files onto the device, the platform seams already
kept in the windowing, path and input code.
