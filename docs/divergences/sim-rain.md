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
  `ControllerHandler.UpdateWarning`. The game's dev `ResetRain` is not
  ported; the dev rain timer's `restart`/`stop` reset the global rain instead (port-only),
  while setting the timer back leaves a started death rain running.
- **Branches of the DLCs and Remix (listed, not ported):** the precycle (`preTimer`, the pulse
  waves, `drainWorldFlood`, `FloodHelpUp`, `forceSlowFlood`, the flood speed's second
  arm), More Slugcats' `DeathRainMode.Pulses` (the second 0.7 draw), the forced
  `GradeAPlateu` in `MS_CORE` and for Rivulet, the `RM` region exception, Watcher's
  passive-rain scaling, `rippleFade`, the sandstorm and blizzard shake terms, the Saint
  `Blizzard` danger, Remix's `cfgDisableScreenShake` and the `RumbleIntensity > 0` test
  of `GeneralUpsetSurface`.
- **Kept as the game has it (user, 2026-10-24):** `RoomCamera.Update` raises `screenShake`
  when `screenShake < rainCycle.ScreenShake`, a comparison without `RumbleIntensity`, so
  with a rumble below 1 the shake is reset to `ScreenShake * RumbleIntensity` every tick
  while the cycle's value is above it.
- **`FluxWaterfall` and `FluxDrain`** (the placed objects that read `floodSpeed`) are not
  DLC-only: their types are registered in the base game and `Room.Loaded` makes them
  without a `ModManager` test. No installed base room places one, and the port does not
  make them.
- **`waterFlux.roomShake`** (the `WaterFlux` effects; no base room has one).
- **Not yet (steps R3-R5):** the flood's water and `RoomRain`'s flood arm (R4), its loops,
  sprites and shader (R3), the bullet drips (R5). The room rain's push, `rainDeath` and
  `Room.RainDrownOutSounds` are D45.
- **The paused camera's micro shake** is drawn on the cosmetic stream (D1) in every drawn
  frame, paused or not, as `PausedDrawUpdate` draws it in the game.

**Why.** Scope decisions of the user (surveys `rain/core.md` and `rain/consumers.md` in
`rainworld-re`); the rest waits for the objects that use it.

**Affects.** The stream (D1), the sky light and the shake from the cycle's last 2400
ticks, the dusk of the 30 % path, the sky look of the slugcat.

## D45: The room rain (`RoomRain`'s intensity, push and kill)

`rw_sim::world::RoomRain` ports `RoomRain` for the base game's story: `Room.Loaded` makes
one in every room that is not an intact shelter and whose settings' `DangerType` is `Rain`,
`FloodAndRain`, `Flood` or `AerieBlizzard` (a broken shelter's is a `Flood` one; its
region-change case is M9), joining the update list just before the `DeathFallGraphic`.
`GenerateShelterTex`'s `rainReach` and `splashTiles` (`HasAnySolid` is `Tile.Solid`, D7)
and `splashes` are kept. Its turn follows `intensity` to `globalRain.Intensity` at 0.2 a
tick, capped by the room's `RainIntensity` (1 in an `AerieBlizzard`), and while
`AnyPushAround` runs `ThrowAroundObjects` over the room's physical objects (the port's
`physical_objects` order: layers, then update order; the player, lizards, flies,
overseers, rocks, spears and coral segments) with every draw in the game's order.
`Creature.rainDeath` is kept on the player and on each other creature, decays in
`Creature.Update`, is capped into `Player.LungUpdate`, and `Creature.TerrainImpact`'s first
floor contact calls `CreatureSmashedInGround` for the player, lizards and flies (overseers
never touch terrain). `Room.RainDrownOutSounds` reads `roomRain.intensity`.

**Kept as the game has it (user, 2026-10-24):** `lastIntensity` is assigned after
`intensity`, so they are equal; `rainDeath` is added from the main chunk only while the
kill test runs on any chunk; `CreatureSmashedInGround` adds `InverseLerp(-2.5, -15, speed)
* ...`, and the impact speed is positive, so the value is always 0 (the draws are made, the
sum is 0; probably the game's sign slip, ported as is, not a decision of the user's: to
confirm).

**What differs:**
- **Where the creature hook runs.** The player's, lizards' and flies' impacts are handled
  inside or right after the body's physics (D14 order); the rain's draws sit between the
  base `PhysicalObject.TerrainImpact` and the ceiling drips, as in the game. A lizard's or
  fly's impact is logged with every chunk's position during the update and `CreatureSmashedInGround`
  reads that snapshot, as the game reads the bodies mid-update.
- **Listed, not ported (DLC and Remix, base game with Remix off):** the MMF gate
  (`RainIntensity < 0.02`), More Slugcats' `OE` gate (`RainIntensity <= 0.2`), Watcher's
  `rippleLayer` skip and `WaterCycleBottom/Top` objects (which also make a `RoomRain`), the
  precycle intensity and flood arms, the Saint `Blizzard` type and `rippleFade`.
- **Not yet:** the flood arm of `Update` (R4), the loops (R3), the drips (R5), `DrawSprites`
  and the splash sprites (R3).

**Why.** Scope of step R2 (survey `rain/core.md`); the rest waits for R3-R5.

**Affects.** The stream (each pushed chunk makes 3 to 7 draws a tick in a pushing room),
the bodies' velocities and stuns in a death rain, the player's lungs and death.
