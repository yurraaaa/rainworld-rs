# rw-sim divergences: the shortcut helper (D43)

Fifth file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D43: `ShortcutHelper`

`ShortcutHelper`, `ShortcutPusher` and `ObjectPusher` (`game/shortcut_helper.rs`,
`shortcut_helper_turn.rs`, `player/shortcut_push.rs`), added by `ShortCutsReady`. Behaviour notes:
`../rainworld-re/notes/shortcut-helper.md`.

**What differs.**
- (user, 2026-10-24: the port stops pushing an object that no longer exists, whether or not the
  game's stale push draws.) An `ObjectPusher` whose body is gone (the object abstracted, or
  destroyed, while pushed) is dropped on its next turn. The game keeps the old `PhysicalObject` and
  goes on pushing its stale chunks (with the side/jitter draws and the sound's) for up to 400 ticks
  (`obj.room == null`).
- `Room.lockedShortcuts` and the Challenge arena's closed exits (`flag`/`flag2` in
  `ShortcutHelper.Update`) are omitted. Five Pebbles' `SSOracleBehavior.LockShortcuts` (base game,
  5 calls) fills the list, so the omission holds until the oracle is ported (OPEN-ITEMS);
  `SB_D03ShortcutLock` is Remix-gated and the Challenge exits are DLC.
- A dead or stunned overseer is pushed like any other body but never gets `enteringShortCut`: the
  port's overseer has no such state.
- `CanPullIntoShortcut` is `grabber is not Frog`; the base game has no frog.

`BodyChunk.goThroughFloors` is read through `CanBePulledIntoShortcut` in the helper's
`GoThroughFloors` test; the chunk physics still read the raw flag, which `Grabbed` sets (not a
divergence in the helper; the getter for the physics is not ported).

**Why.** No body, no state, or no DLC (AGENTS.md).

**Affects.** Objects dropped in a shortcut entrance that leave the room while being pushed; rooms
that lock shortcuts.

(The SU_C04 start is game behaviour, not an effect of this entry: `PlaceInRoom`'s spread puts a
chunk in the creature den's box at (5, 28), so the game's helper shoves the player there too.)
