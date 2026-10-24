# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Creatures turn into debug bodies when leaving a room** (2026-10-24): on leaving a
  room (async loading), the creatures still drawn there switch to their debug-view
  outlines; the game keeps drawing them with their graphics.
