# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Blocky black lines around SB red lights** (user, 2026-10-23; SB_J03 cameras 0-2).
  `RedlightLight` draws the `LightSource` shader masked by `LightMask0`, a 128x128 white
  disc crossed by a black X (the game's own unlit stripes). The game samples it
  bilinearly (its import settings: filter 1, 8 mips, repeat); the port puts it on an atlas
  page read through the one nearest-filter, non-filterable atlas sampler
  (`crates/render/src/sprite_gpu.rs` ~205, `fs_light_source`), so at `scale = rad/8` each
  texel becomes a ~6 px staircase. Fix: sample each page with its texture's own filter and
  wrap (per-page samplers from `Texture2D`'s import settings); check other bilinear pages
  (`Rainbow`) too.
