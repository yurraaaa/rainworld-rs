# rw-sim divergences: the shortcut helper (D43)

Fifth file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D43: `ShortcutHelper`

`ShortcutHelper`, `ShortcutPusher` and `ObjectPusher` (`game/shortcut_helper.rs`,
`shortcut_helper_turn.rs`, `player/shortcut_push.rs`), added by `ShortCutsReady`. Behaviour notes:
`../rainworld-re/notes/shortcut-helper.md`.

**What differs.**
- An `ObjectPusher` whose body is gone (the object abstracted, or destroyed, while pushed) is
  dropped on its next turn. The game keeps the old `PhysicalObject` and goes on pushing its stale
  chunks (with the side/jitter draws and the sound's) for up to 400 ticks (`obj.room == null`).
  The port has no body to read.
- `Room.lockedShortcuts` and the Challenge arena's closed exits (`flag`/`flag2` in
  `ShortcutHelper.Update`) are omitted: only the Remix-gated `SB_D03ShortcutLock`, the Five
  Pebbles oracle's behaviour and DLC arena code fill them; with Remix off (and the oracle not
  ported) the list is always empty.
- `ObjectPusher`'s `enteringShortCut` for a creature is set only for the creatures with that
  state in the port (the player, lizards, flies). An overseer never pops out (it cannot be
  stunned or killed in a way that reaches here except `dead`) and has no entering state.
- The `BodyChunk.goThroughFloors` getter (`CanBePulledIntoShortcut(owner) && grabsDisableFloors`
  reads true) is not a getter in the port: `Grabbed` sets the raw flag at grab time (as before).
  `GoThroughFloors` in the helper reads the raw flag.
- `CanPullIntoShortcut` is `grabber is not Frog`; the base game has no frog, so it is
  `CreatureType::Frog` only for a spawned DLC creature.

**Why.** No body, no state, or no DLC (AGENTS.md).

**Affects.** The SU_C04 start: the player begins at the mouth of the creature den (5, 28), a wrong hole, so the
helper's box shoves its head to x 149 and the rod leaves the middle near 167 (about 16 px, 8 ticks of the start script's
push, further right than without the helper); `mcp/tests.rs`'s start-up ticks follow. Objects dropped in a shortcut entrance that leave the room while being pushed; rooms
that lock shortcuts.
