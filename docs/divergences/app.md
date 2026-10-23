# Divergences: rw-app

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D18: Debug UI and dev spawns (port-only dev tools)

**What.** The windowed app has a debug UI the game does not have (Dear ImGui over the
window, F1; `crates/app/src/debug_ui`): a region switcher (a stub until regions and
gates are ported), a teleport to a room of the current world, the list of every
abstract creature (with going to its room), a rain timer tab (pause, stop, restart, set the time or cycle length, timer speed; MCP's
`set_rain_timer`; the held timer is a dev flag in `RainWorldGame`, the game's own
`RainCycle.pause` countdown cleared only by restart and stop; the tutorial's
`pauseRain` pin at 2000 stays, no override option (decided 2026-10-23); speeds above
1x skip per-tick checks, fine for a dev tool), and spawning creatures and items. The
teleport (`dev_teleport_player`, also MCP's `teleport_player`) has no counterpart in
the game: it puts the player and what it carries in a shortcut vessel where it stands
(`SuckInCreature` without the pull into an entrance), activates the target room as a
room exit does, and pushes the vessel into `betweenRoomsWaitingLobby` with the room's
first exit node (or a given one) as its entrance node; from there the game's own
lobby code moves the abstract creature, spits the player out of that exit once the
room is `ReadyForPlayer` and moves the camera. The world and the session are kept.
Spawns go through `dev_spawn_creature` (the game's dev-tools creature) and
`dev_spawn_object` (an item made as the arena sandbox's `SpawnItems` makes items,
`RealizeInRoom` once the room's shortcuts are ready), which MCP's `spawn_creature`
and `spawn_object` call too. Rocks and spears are realized; the other offered items
(`RainWorldGame::DEV_SPAWN_OBJECTS`) are plain abstract objects without their
subclass's fields (pearl type, colour seed, consumable origin), as D14's spore plants.
The UI's actions apply between frames, before the next ticks; the UI itself makes no
random draws. The headless MCP path and the `--mcp --window` mirror have no debug UI.

**Why.** Developer tooling for testing in the window.

**Affects.** Nothing unless used; a spawn draws from the game's random stream as the
game's own constructors do.

## D20: A hold cycle between lizards' jaws stays drawn

**What.** When two lizards hold each other in their jaws (or a longer cycle forms),
the port draws them: `Jaws` (`crates/app/src/jaws.rs`) nests each
held lizard's sprites into its holder's list and stops where the cycle comes round
again, so the cycle draws nested inside the member that comes first in camera 0's
container order (D21) (A with B inside, B's own hold of A ignored). In the game none of the
cycle draws, nor anything nested in it, until one of them lets go.

**The game (code reading, not observed in play).**
- Reachable: `Lizard.Update` runs `Act()` (`AI.Update`, whose aggressive behaviour calls
  `AttemptBite`) only `if (Consious)`, before `base.Update(eu)`. The pacifying grasp's
  `stun = 10` is set in `Creature.Update`, which is that base call. A lizard bitten
  earlier in the same tick is therefore still conscious when its own update runs, and
  can bite its holder. For pairs whose relationship is `GoForKill`, `Room.Update`'s
  collision loop calls `a.Collide(b)` and then `b.Collide(a)` in the same pair
  iteration, which gives the same window. `Creature.Grab` has no check against grabbing
  one's own holder, `Lizard` overrides neither `CanBeGrabbed` nor `Grabbed`, and
  `Bite`'s early return (`grasps[0] != null && grabbedBy.Count == 0`) doesn't apply to a
  lizard with free jaws. From then on both are stunned and both keep carrying each other
  (`CarryObject`). The hold ends on its 1/40-per-tick drop (when not `Eats`) or on the
  distance and velocity break.
- Drawn: the second bite runs `LizardGraphics.AddToContainer(sLeaser, rCam, CB)` for the
  first holder A, where CB is B's `containers[0]`, which already sits inside A's
  container CA. `RemoveAllSpritesFromContainer` takes A's sprites and CA out of
  `Midground` (`FContainer.RemoveChild` → `HandleRemovedFromStage`, recursive, so B's
  sprites and CB leave the stage too). `CB.AddChild` then adds A's sprites and CA to a
  node that is off the stage, so `HandleAddedToStage` is never called. `FStage`/
  `FContainer.Redraw` walk only the stage tree, so neither lizard draws. The first
  release (`MoveObjectToContainer(obj, null)`) puts the released lizard back into
  `Midground` with the other nested inside it, and both draw again.

**Why.** The user's decision (2026-10-22): never leave both undrawn.

**Affects.** Only the frames while a hold cycle lasts (a few ticks up to about a
second).

## D21: The flies' sprite re-adds placed at the end of their tick

**What.** Camera 0's container order is the game's (`crates/app/src/sprite_order.rs`,
`rainworld-re/notes/sprite-containers.md`): a room the camera moves to gets its
leasers in `drawableObjects` order (`AddObject`ed drawables, then the graphics modules
`NowViewed` builds by collision layer, then the shortcut graphics; weapons in `Items`),
every later `AddObject` and re-add goes on top, a weapon's `ChangeOverlap` picks
`Items` or `Background`, and what lizards' jaws hold follows the latest
`AddObjectToInternalContainer`. The record is brought up to date after each tick. The
jaw moves and the last `ChangeOverlap` of each weapon carry the update-list counter, so
they fall between the tick's `AddObject`s as in the game (several `ChangeOverlap`s of
one weapon in a tick end as the last one does). The flies' re-adds (`Fly.Grabbed`,
`BringSpritesToFront` when picked up or joining a chain) do not, and are applied after
everything else that tick, in order.

**Why.** The sim reports them through the graphics observer without the update-list
counter; carrying it there touches the player's grab code.

**Affects.** Only a tick where a fly's re-add and an `AddObject` (or a jaw move or
`ChangeOverlap`) of an overlapping `Midground` drawable both happen: their two sprites
may stack the other way round until the next re-add.
