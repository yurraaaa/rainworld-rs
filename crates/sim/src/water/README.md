# `water/`

| File | Holds (key types) | Ports |
|---|---|---|
| `drips_turn.rs` | the ceiling drip in `Room.Update`, `Room.Loaded`'s `DrippingSound` and its turns (sound, emitters) in the update list | `Room.Update`, `Room.Loaded` |
| `waterfall.rs` | `WaterFall` (the falls a room's tiles make, ends, drips, surface hit, bubbles, push) | `WaterFall`, `Room.LoadFromDataString` |
| `waterfall_turn.rs` | a fall's turn in the update list: its drips, bubbles and push on bodies (made in `effects/lights/light_fixture_turn.rs`) | `WaterFall.Update` |
| `mod.rs` | wavy surface sim, `FloatWaterLevel`, the level (`originalWaterLevel`, `fWaterLevel`, `lastFWaterLevel`), `GeneralUpsetSurface`, `Water.Update`'s bottom roll, entry splashes and bubbles (`WaterObject`), `BubbleEmitter`s, `WaterfallHitSurface` | `Water`, `DefaultSurface` |
| `bubble.rs` | `Bubble` (`BubbleMode`): air bubbles in a room's update list | `Bubble` |
| `splash.rs` | `Splash`: water thrown up by bodies crossing the surface (update, sounds, terrain drips, `WaterfallHitSurface`) | `SplashWater.Splash`, `WaterParticle` |
| `water_drip.rs` | `WaterDrip` droplets; `Room.Update`'s ambient ceiling drip roll | `WaterDrip`, `Room.Update` |
| `world.rs` | a room's `Water.Update` fed every physical object's chunks (`physicalObjects` order), its splashes and bubbles into the room | `Water.Update`, `Room.physicalObjects` |
