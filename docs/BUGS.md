# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Panic after a lizard falls out of the room** (2026-10-22): with ~100 green lizards
  spawned (F1 debug UI), about a minute later, right after `lizard ... fell out of
  room!`: `range end index 28 out of range for slice of length 0` (core slice index).
  Being traced.
