# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Rewinding the rain timer in the debug UI keeps the rain** (2026-10-24): setting the
  Rain timer tab's timer back before the cycle's end leaves the started death rain,
  flood and rooms' rain running (only `restart`/`stop` reset the global rain, D44).
