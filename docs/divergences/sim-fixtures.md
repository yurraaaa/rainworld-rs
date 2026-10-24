# rw-sim divergences: light fixtures and waterfalls (D30)

Third file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D30: Light fixtures and waterfalls

`LightFixture` placed objects of the kinds that need no electricity (`Redlight`,
`HolyFire`, `SlimeMoldLight`) and the `WaterFall`s `Room.LoadFromDataString` makes from a
room's waterfall tiles (`rw_sim::effects::lights::{light_fixture, light_fixture_turn}`,
`water::{waterfall, waterfall_turn}`, `superstructure::electric::room_fixtures`; sprites `rw-graphics` `effects/lights/light_fixture.rs`,
`water/waterfall.rs`; the `WaterFall` shader in `rw-render`, `LightMask0` in `rw-app`'s atlas
loading). Every shared-stream draw is ported in the game's order, with local draw-count
and order tests. Notes: `../../rainworld-re/notes/lights.md` (fixtures) and
`waterfalls.md`.

- **`Room.ElectricPower` follows the world's `BrokenAntiGravity` (D34).** The 39 emergency
  red lights (gravity-dependent: `BrokenZeroG` above 0 and seed above 0; `ss_a13` 4,
  `ss_b04` 2, `ss_d05` 5, `ss_e05` 10, `ss_f03` 10, `ss_lab12` 2, `ss_lab4` 2, `ss_lab5` 2,
  `ss_s03` 2; all `BrokenZeroG` 1) start unpowered (the constructor sees no room, so
  `NoElectricity` is 0) and power up by chance while the lights are out, with `Range(1, 15)`
  and the `Red_Light_On` call, then shine at the lack of power. `Redlight` does not read
  `Room.gravity`.
- **The `Red_Light_On` sound plays nothing.** `room.PlaySound(SoundID.Red_Light_On, ...)`
  is called as the game does, but `sounds.txt` has the line commented out (`//Red_Light_On
  :`), so the sound loader has no working trigger: no clip and no draws.
- **The fixtures that need electricity are only partly made:** `ZapCoilLight` is (D36),
  `DeepProcessingLight` is (D37); the DLC's `GlowWeedLight` is not. A
  `LightFixture` whose type the game does not register (`ss_c01`'s `LightRod`) makes
  nothing, as in the game. `RedSubmersible` is made (no base room has one).
- **`IProvideWarmth` (Downpour hypothermia) is omitted.** `LightFixture` implements it
  (warmth `0.3 * DefaultHeatSourceWarmth`, range 320), but `Room.AddObject` lists such
  objects in `blizzardHeatSources` only with `ModManager.HypothermiaModule`, which
  `Creature` and `DiscomfortTracker` read.
- Note, not a divergence: as in the game, a `WaterFall` is built while `waterObject` is
  null, so its first `strikeLevel` is -200 (`bottomPos` starts there and the terrain tiles
  it drips off are every solid tile under it down to row -9); `Water` joins it in
  `Room.Loaded`'s `AddWater`, on the realizing call. Afterwards the strike level is the water's `fWaterLevel` less 10
  (Remix off; no `WaterCutoff`, D11), which rises with the rain's flood (D47).
- **Omitted DLC and Remix branches:** `waterInverted` rooms (the inverted strike level,
  `WaterFallInverted` shader, the upward drips and bubbles), `PointSubmerged` and the
  `WaveAmplitude` strike offset (`ModManager.MMF`), the `GlowWeedLight`.
- **Not here:** `WaterGate`'s two falls (with the region gates, M9) and `FluxWaterfall`
  (used by no base room).
