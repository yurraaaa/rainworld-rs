# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Pipe entrances flash white for lizards** (2026-10-22): a lizard about to come out of
  a pipe should flash the entrance in its `ShortCutColor()` (its effect colour; other
  creatures their template's `shortcutColor`). The port records only the creature kind
  (`EntranceColor`, `crates/sim/src/shortcut.rs`) and
  `ShortcutGraphics::creature_color` (`crates/graphics/src/shortcut_graphics.rs`) maps
  every non-slugcat to white.
