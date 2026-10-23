# rw-sim divergences: gameplay room effects (D33)

Fourth file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D33: `ElectricDeath` and `BorderPushBack`

The `ElectricDeath` room effect (28 Underhang rooms; `rw_sim::electric_death`,
`game_electric_death`; sprites `rw_graphics::electric_death`; the `ElectricDeath` and
`FlareBomb` shaders in `rw-render`) and the `BorderPushBack` effect (20 Sunlit Lands rooms of
`world_sl.txt`, from `sl_settingstemplate_fog.txt`; `rw_sim::border_push_back`,
`game_border_push_back`). Every shared-stream draw is ported in the game's order (the flashes'
`Reset` 4, the colour chain, the spark gate 1 + 4, the strike gates per chunk, the strikes'
7-draw sounds, `SparkFlash`'s and `Spark`'s constructors), with local draw-count and order
tests; the draws at draw time (two a light, six a flash) are on the cosmetic stream (D1).
Notes: `../../rainworld-re/notes/electric-death-and-border-push-back.md`.

Creation order: `ElectricDeath`, `RoomBorderPushBack` and `Lightning` (D25) are made in the same
settings-order loop as the other room-effect objects (`add_room_effect_particles`), as
`Room.Loaded` does, so their update-list slots follow the settings' order.

- **DLC and Remix branches omitted:** `ElectricDeath.Intensity`'s Sofanthiel branch (More
  Slugcats: `InverseLerp(cycleLength - 400, cycleLength + 1200, timer)^2.2` is the same, so
  only the story character check differs) and `PrecycleModule`'s (`preTimer` rain: its
  `Intensity` mix, the kill gate that skips the beam test, and `damage = 0`); the
  `ElectricSpear` charge or short circuit on a struck spear (`DLCShared`); `Player.Update`'s
  `RequestMSSirenSong` and the `MSSirenSong` volume 0.35 in a room with the effect
  (`MS`, `LM`, `DM` worlds); the Watcher's `ripple_both_sides` keyword on the `*BothSides`
  shaders `SparkFlash` uses (without `RIPPLE` it does nothing, so `LightSourceBothSides`,
  `FlatLightBothSides` and `FlareBombBothSides` are `LightSource`, `FlatLight` and `FlareBomb`).
- **The rain is not ported, so the cycle just runs on.** `Intensity` reads `rainCycle.timer`,
  `cycleLength` and `TimeUntilRain` (`rw_sim::RainCycle`), which the port ticks; nothing ends
  the cycle at the rain (the death of the slugcat at `cycleLength` is not there yet), so the
  effect reaches its full intensity 1200 ticks after the cycle's end and stays. `get_state`'s
  `rain.timer` shows it, and stepping `cycle_length + 1200` ticks (about 28 000, a few
  seconds headless) gets there without a cheat.
- **A newborn `SparkFlash` is NaN for its first frame, and a sprite with a NaN scale or
  colour is not drawn.** `Pow(Sin(life * PI), 0.5)` with `life` 1 takes the sine of `(float)PI`,
  just below 0, so the game's scale and alpha are NaN; the port keeps the NaN in the draw list
  and the renderer skips such a sprite (INFERRED: Unity draws nothing for a NaN mesh).
- **`LightFlash` 0 is a unit disc at the screen's lower-left corner.** The game never
  updates or draws `flashes[0]` (sprite 1), so its `FSprite` keeps Futile's defaults
  (position 0, scale 1, alpha 1) with the `LightSource` shader and green: drawn as such.
- **The sound loops start twice in one tick, the game's way:** `DynamicSoundLoop.Update`
  clears `lastSound` while the loop has no emitter, so the tick a volume rises from 0 starts
  the loop for the changed `sound` and again for the rise, the first emitter forgotten. The
  port's `ChunkDynamicSoundLoop` does the same (as `DrippingSound`'s), so each loop plays
  twice at its start.
- **A `Violence` on a creature is the creature's existing port** (`creature_violence`:
  the base `Creature.Violence`, the lizard's own, the player's `Stun`), with `source` null (no
  kill tag), `directionAndMomentum` null (no push) and the chunk's index. The templates'
  damage resistances to `Electric` apply (0 for the slugcat, green lizard, bat fly and
  overseer; 102 for a few water creatures, none of which has a body yet).
- **Not checked against the game's picture:** the web's look. The `ElectricDeath` and
  `FlareBomb` shaders are ported from the shader files (float precision, as Unity's desktop
  `half`), the sprites' positions, scales and colours from `ElectricDeath.cs`; there is no
  reference screenshot of the original.
