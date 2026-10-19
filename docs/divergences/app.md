# Divergences: rw-app

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D18: Debug UI and dev spawns (port-only dev tools)

**What.** The windowed app has a debug UI the game does not have (Dear ImGui over the
window, F1; `crates/app/src/debug_ui`): a region switcher (a stub until regions and
gates are ported), a teleport to a room of the current world, the list of every
abstract creature (with going to its room), and spawning creatures and items. The
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
