# Divergences: rw-sim, the rain

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D44: The global rain (`GlobalRain`, `DeathRain`) and what reads it

`rw_sim::GlobalRain` ports `GlobalRain` whole for the base game: the direction draws every
tick from the start of the game (right after camera 0's `Update`, before the realizer),
the pre-rain `Intensity` ramp with the camera room's `LightRain`, `HeavyRain(Flux)` and
`BulletRain(Flux)` effects, and `DeathRain` with every mode, its draws at each entry on
both paths (`RainHit` runs in `RainCycle.Update`, so the first `DeathRainUpdate` is the
tick after), the flood's speed and offset (the value; the water does not follow yet) and
the game's index comparison (`AlternateBuildUp` counts as after `GradeABuildUp`).
`RainCycle`'s `ShaderLight`, `ScreenShake` and `MicroScreenShake` return its values from
`RainGameOver`; camera 0's `Update` takes the cycle's shake times the room's
`RumbleIntensity` (not in shelters; `AerieBlizzard` rooms follow the cycle's), the water
surface draws its rumble terms (three per point a tick in a room whose `DangerType` is
not `None`), `PlayerGraphics` looks up at the sky in the cycle's last 2400 ticks of a
`Rain` or `FloodAndRain` room, `RoomRealizer` stops its random neighbour step from
`RainGameOver`, and the microphone's volume group 0 takes `Room.RainDrownOutSounds`.
Game bugs are kept (user decision, 2026-10-24).

**What differs:**
- **Where it lives.** The game keeps `globalRain` on `RainWorldGame` and `RainCycle` reads
  it through `world.game`; the port keeps it in `RainCycle::global_rain`, which every
  reader has to hand. A world replaced at a gate (M9) has to carry it over; the game's
  `OverWorld` copies the cycle's fields into a fresh `RainCycle` (which has its own
  `deathRainHasHit`) while `globalRain` persists.
- **Skipped by the user (2026-10-24), until their owners exist.** Arena sessions' rain
  (`IsArenaSession`: the forced 70 % path, progression 3.2 times faster, the second flood
  arm up to 1.8, `ArenaEndSessionRain`, `NoRain`); every port start is a story session
  (D4), so none of those arms runs. The ghost-mode gate of `AllowRainCounterToTick`
  (echoes). Gamepad rumble: `controllerShake` and
  `ControllerHandler.UpdateWarning`. The dev `ResetRain`: the dev rain timer control leaves
  a death rain that has started as it is when the timer is moved back.
- **Branches of the DLCs and Remix (listed, not ported):** the precycle (`preTimer`, the pulse
  waves, `drainWorldFlood`, `FloodHelpUp`, `forceSlowFlood`, the flood speed's second
  arm), More Slugcats' `DeathRainMode.Pulses` (the second 0.7 draw), the forced
  `GradeAPlateu` in `MS_CORE` and for Rivulet, the `RM` region exception, Watcher's
  passive-rain scaling, `rippleFade`, the sandstorm and blizzard shake terms, the Saint
  `Blizzard` danger, Remix's `cfgDisableScreenShake` and the `RumbleIntensity > 0` test
  of `GeneralUpsetSurface`.
- **`waterFlux.roomShake`** (the `WaterFlux` effects; no base room has one).
- **Not yet (steps R2-R5):** `RoomRain`, so `Room.RainDrownOutSounds` is 0 in every room
  (it reads `roomRain.intensity`), `Creature.rainDeath`, the push and the flood's water.
- **The paused camera's micro shake** is drawn on the cosmetic stream (D1) in every drawn
  frame, paused or not, as `PausedDrawUpdate` draws it in the game.

**Why.** Scope decisions of the user (surveys `rain/core.md` and `rain/consumers.md` in
`rainworld-re`); the rest waits for the objects that use it.

**Affects.** The stream (D1), the sky light and the shake from the cycle's last 2400
ticks, the dusk of the 30 % path, the sky look of the slugcat.
