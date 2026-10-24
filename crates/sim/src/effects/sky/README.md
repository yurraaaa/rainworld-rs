# `effects/sky/`

| File | Holds (key types) | Ports |
|---|---|---|
| `lightning_turn.rs` | a room's `Lightning` in the game: `Room.Loaded` making it, its turn and room calls (`MakeBackgroundNoise`, `ScreenMovement`, `PlaySound`), its loop emitters' turns, hearing dulled by the background noise | `Room.Loaded` lightning, `MakeBackgroundNoise` |
| `lightning.rs` | `Lightning` (`Lightning`/`BkgOnlyLightning` effects): two `LightningSource`s (reset, flash curve, thunder queue, `Electricity_Loop`), the camera's reads (`CurrentLightLevel`, `CurrentBackgroundColor`, `CurrentFogColor`, `LightningFrame`) | `Lightning` |
| `background_scene.rs` | `BackgroundScene`: a room's scene object and its elements (`ElementKind`: skies, fog, distant buildings, clouds, a roof's floor, buildings, rubble and smoke), `DrawPos`, `PosFromDrawPosAtNeutralCamPos`, the add order | `BackgroundScene`, `BackgroundSceneElement` and the element classes' fields |
| `above_clouds_view.rs` | `AboveCloudsView`'s constructor: the 50 (Underhang) or 30 and 38 (Sky Islands) elements, altitudes, the clouds' draws under `InitState(12)`; `DistantLightning`: the bolt's timers (up to 3 shared draws a tick) and its brightness draw (cosmetic stream) | `AboveCloudsView`, `AboveCloudsView.DistantLightning` |
| `roof_top_view.rs` | `RoofTopView`'s constructor: sky, floor, 16 rubble strips, city, 4 smoke layers (4 shared draws); `DustpuffSpawner` (the player's chunks landing or running) and `DustPuff` | `RoofTopView`, `RoofTopView.DustpuffSpawner`, `DustPuff` |
| `background_scene_turn.rs` | scenes in the game: `Room.Loaded` making them, the first turn adding the elements far to near, the lightning's turns, the spawner's and puffs' turns | `Room.Loaded` (`AboveCloudsView`, `RoofTopView`), `BackgroundScene.Update` |
