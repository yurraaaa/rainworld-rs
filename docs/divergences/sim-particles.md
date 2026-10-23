# rw-sim divergences: depth-faded particles (D29)

Second file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D29: Sky dandelions, green sparks, zero-gravity specks and corruption spores

`SkyDandelions` (with `DandelionPatch`), `GreenSparks`, `ZeroGSpecks` and
`CorruptionSpores` (`rw_sim::sky_dandelion`, `green_sparks`, `zero_g_speck`,
`game_particles`; sprites `rw_graphics::sky_dandelion`, `green_spark`, `zero_g_speck`;
the `CustomDepth` shader in `rw-render`). Every shared-stream draw is ported in the
game's order. Notes: `../../rainworld-re/notes/particles.md`.

- **No zero gravity, so the specks fall.** `Room.gravity` is read from
  `RealizedRoom::gravity`, which is 1 until `AntiGravity` (the `ZeroG` and
  `BrokenZeroG` effects, a later gameplay step) sets it. In the Superstructure rooms
  (all `ZeroGSpecks` and `CorruptionSpores` rooms) a speck made by `NowViewed` starts
  at the origin; with gravity on its first update puts it out of the camera window or
  inside a wall, so it is `reset` at once, hidden, and only comes back when the room's
  gravity is 0. The zero-gravity branches (drift, wall push, window wrap, window
  placement, replacing a fallen speck) are ported and tested with `gravity` 0 but not
  reachable in play. The seeding's `2000 * amount` / `1600 * amount` draws still run.
- **Reset specks are never destroyed here (F2).** `GenericZeroGSpeck.Update` returns in
  its `reset` branch before the `!BeingViewed` destroy, so with the constant gravity 1 a
  `reset` speck or spore outlives its camera's visit, keeps making its per-tick draws
  (the blink's, the spore's two) and each camera entry adds `1000 * amount` or
  `200 * amount` more. Game-exact for gravity other than 0; it matters only because
  `AntiGravity` is not ported and all 40 speck and spore settings files carry `ZeroG`
  or `BrokenZeroG`.
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
