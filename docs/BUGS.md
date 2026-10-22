# Known bugs

Seen in play, not yet investigated. Remove an entry once fixed.

- **Overseer flickers between two positions** (2026-10-16): rarely, an overseer jumps
  back and forth very quickly between two positions, its body stretching.
- **Slugcat tail stretches out of a pipe** (2026-10-16): when the slugcat comes out of a
  shortcut, its tail teleports and stretches very far before snapping into place.
- **Over-exposed spots in UW rooms outside The Wall** (user, 2026-10-22). Cause
  (observed): these rooms have the `Lightning` room effect (from the Underhang and Leg
  templates; The Wall's template has none), so `FullScreenEffect::select` picks
  `LightBloom` with the Lightning alpha `InverseLerp(0,0.2,amount)*InverseLerp(-0.7,0,num4)`.
  The game's `num4` there is `room.lightning.CurrentLightLevel` (`RoomCamera.DrawUpdate`
  ~1359), -1 between flashes (alpha 0, `_light` -1); the port has no `Lightning` and feeds
  the rain cycle's light (~0.6), so the bloom runs at alpha 1 all the time and the
  terrain is out of cloud shadow (`Visuals::shader_light`, `crates/app/src/visuals.rs`).
  Fixed by porting `Lightning` (M7 effects step 4).
