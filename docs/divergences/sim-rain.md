# Divergences: rw-sim, the rain

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D44: The global rain (`GlobalRain`, `DeathRain`) and what reads it

`rw_sim::GlobalRain` ports `GlobalRain` whole for the base game: the direction draws every
tick from the start of the game (right after camera 0's `Update`, before the realizer),
the pre-rain `Intensity` ramp with the camera room's `LightRain`, `HeavyRain(Flux)` and
`BulletRain(Flux)` effects, and `DeathRain` with every mode, its draws at each entry on
both paths (`RainHit` runs in `RainCycle.Update`, so the first `DeathRainUpdate` is the
tick after), the flood's speed and offset (the water follows it, D47) and
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
  ported; the dev rain timer's edits instead rewind the rain (port-only, user, 2026-10-24): any edit of
  `cycle_length`, `restart`, `stop`, `timer` or `shift` that leaves `TimeUntilRain > 0`
  resets the global rain (`GlobalRain` default, `deathRainHasHit`), each realized room's
  `RoomRain` intensity and `lastIntensity` to 0 (its loops fall silent on their next
  update), its water to `originalWaterLevel` (as realized with flood 0), water that
  `Room.AddWater` gave a dry room (the file's dry room returns; no `Water`, `WaterSound`
  or `added_water`), and every creature's and the player's `rainDeath`; no draws. A timer
  at or past the end changes nothing.
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
- **Not here:** the room rain's loops, sprites and shader are D46, the bullet drips D50,
  the flood's water D47. The room rain's push, `rainDeath` and
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
sum is 0; the game's sign slip, ported as the game has it by the user's decision, 2026-10-24).

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
- **Not yet:** the drips (R5). The loops, `DrawSprites` and the splash sprites are D46;
  the flood arm of `Update` is D47.

**Why.** Scope of step R2 (survey `rain/core.md`); the rest waits for R3-R5.

**Affects.** The stream (each pushed chunk makes 3 to 7 draws a tick in a pushing room),
the bodies' velocities and stuns in a death rain, the player's lungs and death.

## D46: The room rain's sprites, shader and sounds (`RoomRain.DrawSprites`, the seven loops)

`RoomRain`'s sprites are `rw_graphics::RoomRainSprites::draw` (`effects/gameplay/room_rain.rs`):
sprite 0 is a `Futile_White` quad scaled to `options.ScreenSize.x` by 768 at the stage origin
with the `DeathRain` shader in `GrabShaders`, and `splashes` splash sprites of `1 + splashes`
(`90DegreeSplash` below `splashes / 2`, anchor y 0.2, then `TallSplash`, 0.1) in `Items`,
placed on up to five random `splashTiles` a drawn frame with the game's order of draws on the
cosmetic stream (D1). `DeathRain.shader` is `death_rain.wgsl` (a `GrabPass`; the room's
`RainMask_` is the mask texture the `WaterLight` already reads, now generated for every room,
not only those with water). `_rainDirection`, `_RainSpriteRect`, `_rainIntensity` and
`_rainEverywhere` are set by `Renderer::set_rain_globals` from the camera room's `RoomRain`
each drawn frame; `_waterLevel` is the camera's, as for the other shaders. The seven
`DisembodiedDynamicSoundLoop`s are `RainLoops` (`world/room_rain_sound.rs`), updated at the
end of `RoomRain.Update` through the shared `ChunkDynamicSoundLoop`, with their emitters in
the room's update list; the rain loops' volume group (3, or 0 for the distant and screen
shake loops and a non-`Flood` room's flood loop) reaches `rw-audio` as an emitter event.

**Kept as the game has it (user, 2026-10-24):** the last splash sprite (index `splashes`)
is created but never added to a container nor updated, so it is not drawn.

**What differs:**
- **Where the sprites are built.** In `rw-graphics`, as `ElectricDeath`'s, from the sim's
  `RoomRain`; the app hands it the camera room's water, the camera position and the palette's
  fog colour. A hidden sprite (intensity 0, or no splash tile in view) is simply not in the
  draw list; the game's `isVisible` sprites at intensity 0 and the swap to the `Basic`
  shader make no difference to the frame.
- **Listed, not ported (DLC and Remix, base game with Remix off):** `smoothRippleClip`
  (Watcher's ripple; a no-op in the base game), `rippleFade` (1 in the base game), Remix's
  `_RainSpriteRect` width (`sSize.x` instead of the 1366 literal), the `Blizzard` type's
  missing flood loop, the Watcher `Water_Machinery_*` sounds.
- **Not here:** `Bullet_Drip_Strike` and the drips' sprites are D50.

**Why.** Scope of step R3 (survey `rain/core.md` sections 8 and 9).

**Affects.** The stream: a pushing or raining room makes the loops' 7 draws each time one
starts (once, D48) and, on the cosmetic stream, 5 to 8 draws per splash each drawn frame.

## D47: The flood's water (`RoomRain`'s flood arm, `Room.AddWater`, `fWaterLevel`)

`rw_sim::Water` keeps `originalWaterLevel`, `fWaterLevel` and `lastFWaterLevel`: the surface
moves to `fWaterLevel` in each `Surface.Update` (`DefaultSurface.TargetWaterLevel`), so
every `FloatWaterLevel`/`DetailedWaterLevel` reader (submersion, buoyancy, swimming,
lungs, `ThrowAroundObjects`, drips, splashes, bubbles, lizards, flies, lights) follows it a
tick later, as in the game; `Water.Update` ends with `lastFWaterLevel = fWaterLevel`. The
waterfalls' `strikeLevel`, `RoomCamera`'s `_waterLevel` (`fWaterLevel + 100`), the
`WaterLight` sprite and `DrawSprites`' edge level (`Lerp(lastFWaterLevel, fWaterLevel,
timeStacker)`) read it directly. Readers of `defaultWaterLevel` and `room.water` keep the
room's row, which the flood does not move in the game either. The `RoomRain`
constructor sets `fWaterLevel = originalWaterLevel + flood`. `Update`'s flood arm runs
after `ThrowAroundObjects` in a room whose settings' `DangerType` is `Flood` or
`FloodAndRain` once `TimeUntilRain <= 0`, `flood > 0` or the room has level water
(`defaultWaterLevel > -1`). With a water object, `fWaterLevel` lerps 0.2 toward `FloodLevel`
(`originalWaterLevel + flood`, or `originalWaterLevel` where the rain's `dangerType` does
not flood, -100 without water) and `GeneralUpsetSurface(InverseLerp(0, 0.5, Intensity) * 4)`
makes its three draws (`Range(0, totalPoints)`, two values). Without one, once the death
rain is on, `Room.AddWater` gives the room `defaultWaterLevel` -10 (`Arc::make_mut` on the
realized room's `Room`, so a later realization reads the file again, as the game reloads
it) and a `Water` at -190 with its `waterSounds` loop on the band at rows -11 to -10, as
the game's constructor places it. The camera's surface sprites appear with it; the
`WaterLight` only on the camera's next room change, as `ChangeRoom` makes it.

**Kept as the game has it (user, 2026-10-24):** the constructor raises every room with
a water object by the flood, flooding or not, and a `Rain` room's water then never comes
back down (only flooding rooms run the arm). The flood moves no tile water flags and no
AI map (`UpdateWaterAccessibility` is Watcher's), and it is the same pixel offset in every
flooding room. The `RainMask_` texture is the room's first, made at `Room.Loaded` before
any `AddWater`, so flood water never marks it.

**What differs:**
- **Listed, not ported (DLC and Remix):** the precycle arms (`floodLerpSpeed`,
  `drainWorldFlood`, the world-position flood, `RoomRainFloodShake`, `AddWater` at -2000
  and `Room.Update`'s -2000 snap, `fWaterLevel = FloodLevel` after `AddWater`, the
  constructor limited to flooding rooms), Watcher's `waterFlux` (`WaterFlux`,
  `WaterFluxMaxLevel`, the flux branch of `FloodLevel`), `waterLevelMin`/`Max`
  (`WaterCycleTop`/`Bottom`) and their machinery sounds, `UpdateWaterAccessibility`,
  inverted water (`waterInverted`), More Slugcats' constructor branch in `Water` (which
  the next line overwrites), `AlternateSurfaceFloodLevel` (`WaterCutoff`/`AirPocket`
  objects, D11) and Remix's `RumbleIntensity > 0` test.
- **Not yet (M9):** a broken shelter's water (`ShelterDoor`'s `BrokenShelterWaterLevel`
  and its `FloodLevel` copy) and `OverWorld.SyncFloodLevels`' rebase of the flood across
  regions. `FluxWaterfall`/`FluxDrain` are D44.

**Why.** Step R4's scope (surveys `rain/core.md` 4.2, 5.3, 5.5, `rain/consumers.md` 4).

**Affects.** The stream (three draws a tick per flooding room with water), drowning in
flood rooms, the level shader's water line and every body's submersion there.

## D50: The bullet rain (`BulletDrip`, `RoomRain`'s drip upkeep)

`RoomRain.Update` keeps `(int)(TileWidth * globalRain.bulletRainDensity *
roomSettings.RainIntensity)` `BulletDrip`s (one added or the oldest destroyed a tick,
after `ThrowAroundObjects` and before the flood arm); each is an object in the room's
update list. The density is non-zero in the death rain's `CalmBeforeStorm` (`p^8` over
`InverseLerp(0.3, 1, p)`) and `GradeABuildUp` (`1 - p`) modes and in the passive
`BulletRain` effects (D44). A drip counts its `delay` down once landed; `Strike` picks a
column (`Range(0, TileWidth)`), places the tip above the first solid tile (row -10 in a
column without one) and the sky end above the room, restarts the fall and redraws
`delay`, traces the path against the collision-layer-1 chunks (the player, lizards and
loose weapons; radius 0.5, the first `CirclesCollisionTime`), plays
`Small_Object_Into_Water_Fast` after `WaterfallHitSurface(pos, 0, 1)` when it ends below
the water surface (`pos.y` set 30 under it) or `Bullet_Drip_Strike`, and pushes the hit
chunk down by `2 / mass` and stuns a creature by `Range(0, 4)`. The streak (`BulletRain`
shader, a `GrabPass`) and the `RainSplash` (scale `Sin(falling * pi) * 0.4`, a rotation
draw per drawn frame on the cosmetic stream, D1) are drawn in the camera's room by
`rw-graphics`.

**Kept as the game has it (user, 2026-10-24):** `Strike` returns without resetting
`delay` when the chosen column's top tile is solid or its `rainReach` is the room's
height, so the drip strikes again, and draws again, every tick until a column works.

**What differs:**
- **Listed, not ported (DLC and Remix):** the Watcher's `rippleLayer == 0` test on the
  hit chunk and `rippleFade` (1 in the base game) around the sounds; `rippleClip` in the
  shader (a no-op in the base game).
- **No reader in the base game:** `RoomCamera`'s `RoomHasWater` shader keyword is read
  only by `WarpTear` (Watcher); `RainCycle.RegionHidesTimer`'s readers (`RainMeter`) all
  test `ModManager.MMF` first (Remix). Neither is ported.
- A drip the upkeep destroys is dropped from the room's list at once; the game removes it
  at the end of the room's update (its `Update` is skipped in between either way).
- The trace covers the objects the port has bodies for: the player, lizards and loose
  weapons in layer 1 (flies and overseers are layer 0; the game's trace is layer 1 only).

**Why.** Step R5's scope (survey `rain/core.md` 5.3, 6, 7, 8, 9).

**Affects.** The stream: a bullet-rain room makes 1 (a failed strike) or 4 to 5 draws (plus the
sound's, and `WaterfallHitSurface`'s in water, and 1 per creature hit) per landed drip each
time its delay runs out, and 1 cosmetic draw per drip per drawn frame; what the drops hit
is pushed down and stunned.
