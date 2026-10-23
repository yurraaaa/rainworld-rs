# rw-sim divergences: light fixtures and waterfalls (D30)

Third file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D30: Light fixtures and waterfalls

`LightFixture` placed objects of the kinds that need no electricity (`Redlight`,
`HolyFire`, `SlimeMoldLight`) and the `WaterFall`s `Room.LoadFromDataString` makes from a
room's waterfall tiles (`rw_sim::light_fixture`, `waterfall`, `room_fixtures`,
`game_light_fixtures`, `game_waterfalls`; sprites `rw_graphics::light_fixture`,
`water_fall`; the `WaterFall` shader in `rw-render`, `LightMask0` in `rw-app`'s atlas
loading). Every shared-stream draw is ported in the game's order, with local draw-count
and order tests. Notes: `../../rainworld-re/notes/lights.md` (fixtures) and
`waterfalls.md`.

- **`Room.ElectricPower` is 1 (no `AntiGravity`).** The game reads
  `rainCycle.brokenAntiGrav.CurrentLightsOn` in a room with the `BrokenZeroG` effect; that
  object (`AntiGravity.BrokenAntiGravity`: a switch that starts on or off by a draw and
  flips every `gravityFlickerCycleMin..Max` seconds, with its own sounds and screen
  movements) is part of the `AntiGravity` effect, which is not ported.
  `RealizedRoom::electric_power` returns 1, the "power on" end. A `Redlight` is
  gravity-dependent when the room's `BrokenZeroG` is above 0 and its seed above 0; 39 red
  lights in 9 Superstructure rooms are (`ss_a13` 4, `ss_b04` 2, `ss_d05` 5, `ss_e05` 10,
  `ss_f03` 10, `ss_lab12` 2, `ss_lab4` 2, `ss_lab5` 2, `ss_s03` 2; all `BrokenZeroG`
  1). With full power they are unpowered: both alphas 0, no draws, the light stays out.
  The game starts each play with the switch on or off at random and flips it later, so
  these lights would come on in the emergency state. The gravity-dependent paths (power-up
  with `Range(1, 15)`, going dark under 0.6 lack of power, the alphas scaled by the lack
  of power) are ported and tested with explicit power values. `Redlight` does not read
  `Room.gravity`.
- **The `Red_Light_On` sound plays nothing.** `room.PlaySound(SoundID.Red_Light_On, ...)`
  is called as the game does, but `sounds.txt` has the line commented out (`//Red_Light_On
  :`), so the sound loader has no working trigger: no clip and no draws.
- **The fixtures that need electricity are not made:** `ZapCoilLight`, `DeepProcessing`
  (SS, with `AntiGravity` and the zap coils; step L2b) and the DLC's `GlowWeedLight`. A
  `LightFixture` whose type the game does not register (`ss_c01`'s `LightRod`) makes
  nothing, as in the game. `RedSubmersible` is made (no base room has one).
- **`IProvideWarmth` (Downpour hypothermia) is omitted.** `LightFixture` implements it
  (warmth `0.3 * DefaultHeatSourceWarmth`, range 320), but `Room.AddObject` lists such
  objects in `blizzardHeatSources` only with `ModManager.HypothermiaModule`, which
  `Creature` and `DiscomfortTracker` read.
- **`WaterFall`s are made before the room has water.** As in the game, a fall is built
  while `waterObject` is null, so its first `strikeLevel` is -200 (`bottomPos` starts
  there, the terrain tiles it drips off are every solid tile under it down to row -9) and
  `Water` joins it when the room finishes loading. The strike level read afterwards is
  the water's still level less 10 (Remix off), which does not move (no flooding, no
  `WaterCutoff`, D11).
- **Omitted DLC and Remix branches:** `waterInverted` rooms (the inverted strike level,
  `WaterFallInverted` shader, the upward drips and bubbles), `PointSubmerged` and the
  `WaveAmplitude` strike offset (`ModManager.MMF`), the `GlowWeedLight`.
- **Not here:** `WaterGate`'s two falls (with the region gates, M9) and `FluxWaterfall`
  (used by no base room).
