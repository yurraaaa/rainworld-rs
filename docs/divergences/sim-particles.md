# rw-sim divergences: depth-faded particles (D29)

Second file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D29: Sky dandelions, green sparks, zero-gravity specks and corruption spores

`SkyDandelions` (with `DandelionPatch`), `GreenSparks`, `ZeroGSpecks` and
`CorruptionSpores` (`rw_sim::effects::particles::{sky_dandelion, green_sparks,
zero_g_speck, turn}`; sprites `rw_graphics::sky_dandelion`, `green_spark`, `zero_g_speck`;
the `CustomDepth` shader in `rw-render`). Every shared-stream draw is ported in the
game's order. Notes: `../../rainworld-re/notes/particles.md`.

- **Zero gravity is ported (D34), so the specks drift where the game's do.**
  `Room.gravity` is set every tick by the room's `AntiGravity` (`ZeroG` amount 1 makes it
  0). In a room whose gravity is 0 a speck made by `NowViewed` is placed in the camera
  window on its first update and drifts; in a `BrokenZeroG` room it falls as dust while the
  switch is off and drifts when it is on. The zero-gravity branches are tested with
  `gravity` 0.
- **Reset specks are never destroyed here (F2).** `GenericZeroGSpeck.Update` returns in
  its `reset` branch before the `!BeingViewed` destroy, so in a room with gravity above 0
  (the `BrokenZeroG` rooms while their switch is off; a `ZeroG` amount below 1) a
  `reset` speck or spore outlives its camera's visit, keeps making its per-tick draws
  (the blink's, the spore's two) and each camera entry adds `1000 * amount` or
  `200 * amount` more. Game-exact.
- **No rain:** `globalRain.Intensity` is 0 in `SkyDandelions.SpawnChance`
  (`rw_sim::RainCycle` has no `GlobalRain`).
- **Sprite-making draws on first draw.** A spore's `Pebble{n}` (`Random.Range(1, 15)`,
  the cosmetic stream, D1) is drawn the first time the app draws it, in update order,
  not when the camera's `ChangeRoom` makes the leasers in `drawableObjects` order.
- **`NowViewed` order (F1; user, 2026-10-23: the order of these draws does not matter,
  keep it as a divergence).** The game's `Room.NowViewed` (inside `RoomCamera.ChangeRoom`)
  runs the effects loop (sparks, specks, spores), then `InitiateGraphicsModule`, then
  `insectCoordinator.NowViewed`. The port makes the graphics modules' draws at once in
  `camera_changed_room` and the particles' and insects' at the next tick's head
  (`apply_insect_view_change`). Same draws, in another order, when the camera enters a
  particle room holding a fly, overseer or lizard.
- **DLC and Remix branches omitted:** `ModManager.MSC` flips every x sign of a
  `GreenSpark` (`+0.11`, the heading's `0.6`, the wind's `0.1`) and lets sparks enter at
  the right edge; Remix (`MMF`) takes the specks' window width from
  `options.ScreenSize.x` instead of 1024, makes a spark's light `noGameplayImpact`
  (a cosmetic light, outside `lightSources`) and Watcher's `cfgReduceFallingParticles`
  is not read; Watcher's calcified spores (rooms named `wssr...`, grey) and sentient
  spores (`DaddyCorruption.SentientRotMode`, `RippleColor`).
