# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Dead lizard's jaw keeps moving** (2026-10-22): a dead green lizard's jaw opens and
  closes as the player moves around it (even jumping over it without touching). Checked:
  the jaw value stays fixed; the jaw sprites scale with `headDepthRotation`, which follows
  `lookPos`, which keeps chasing the last look creature because `LizardGraphics.Update`'s
  look code has no dead gate and the dead lizard's tracker keeps its last visual contact.
  The port matches the decompile (`LizardGraphics.cs` ~901-921, 1208-1210, 1876-1884), so
  believed to be the game's own behaviour; remove once compared with the real game.
