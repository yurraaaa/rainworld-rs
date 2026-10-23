# rw-sim divergences: background scenes (D32)

Third file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D32: Background scenes (`AboveCloudsView`, `RoofTopView`): omitted branches

`BackgroundScene`, `AboveCloudsView`, `RoofTopView`, `DistantLightning` and the roof's
`DustpuffSpawner` (`rw_sim::effects::sky::{background_scene, above_clouds_view, roof_top_view,
background_scene_turn}` (the distant lightning and dust puffs in the two views); sprites
`rw_graphics::background_scene`, `dust_puff`; the shaders are D31). Every shared-stream
draw is ported in the game's order: the clouds' `randomOffset` under `InitState(12)`
(no shared draws), a `Smoke`'s in `RoofTopView`'s constructor (4), a `DistantLightning`'s
timers every tick, a dust puff's. Notes: `../../rainworld-re/notes/background-scenes.md`.

- **No `DayNight`.** The scenes' day, dusk and night blocks (sky alphas,
  `atmosphereColor`, `_MultiplyColor`, the spire lights) run only with a `DayNight`
  room effect once the cycle is over, or in Expedition; no base room has the effect, so
  `daySky`, `duskSky` and `nightSky` stay at alpha 1 and the colours at their constructor
  values. The `RainCycle` has no `dayNightCounter` yet.
- **`DustPuff`'s `room.snow` branch** (the `SporesSnow` shader) is left out: snow is
  DLCShared/More Slugcats and no base room has it.
- **`BackgroundSceneElement.blendColor`'s multiply** is left out: nothing in the decompile
  assigns it, so it is unreachable.
- **DLC branches omitted:** `PinkSky` (Watcher images), `OEClouds` and `OEshiftView`,
  the More Slugcats altitudes of `LM` and `SL`, the ending's `animateSMEndingScroll` and
  `yShift` (kept at 0), the Saint timeline's `SAINT` images without lightning, the
  Rivulet's dusk sky, `RoofTopView`'s Lantern-at-the-Edge city (`isLC`), `_DM` images and
  Dust Storm waves, `BackgroundScene.RoomToWorldPos`'s `LC` map position, and
  `convergencePoint` for `waterInverted` rooms.
- **Arena sessions:** the constructor's arena branch (`sceneOrigo` and altitudes from the
  effect's amount, no `SIClouds`) and `Fog`'s zero alpha are ported from `!world.story`;
  every port `World` is a story one (D4, D16, D22), so they do not run.
- **The dev-tool keys** that move `DistantBuilding`s with the mouse are left out.
- **`PosFromDrawPosAtNeutralCamPos` sees a zero `convergencePoint`** in the constructors,
  as the game's do (the first turn sets it later): positions are `input * depth`.
  This is the game's behaviour, noted because the survey assumed otherwise.

**Why.** Unreachable in the base game, or DLC.

**Affects.** Nothing visible in the base game's rooms.
