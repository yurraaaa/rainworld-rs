# `sound/`

| File | Holds (key types) | Ports |
|---|---|---|
| `noise_turn.rs` | in-room noises heard at once | `Room.InGameNoise` |
| `noise.rs` | `InGameNoise`, terrain-impact noise, `NoiseSink` (an updating object's noises and screen movements), `RoomCall`, `ImpactCalls` | `Noise/InGameNoise.cs` |
| `mod.rs` | `Room.PlaySound` events + `VirtualMicrophone` draws (disembodied and rect loops too, `SoundRect`) | `VirtualMicrophone.PlaySound`, `PlayDisembodiedLoop`, `PlayRectSound` |
| `water_sound.rs` | `WaterSound`: the water's calm surface loop (`RectangularDynamicSoundLoop` on the band under the surface, `SilenceWater`) | `Water.waterSounds`, `RectangularDynamicSoundLoop` |
| `water_sound_turn.rs` | the surface loop at the head of `Water.Update` and its emitters in the update list | `Room.Update` |
| `dripping_sound.rs` | `DrippingSound`: the cycle start's `Cycle_Start_Drips` loop (`DisembodiedDynamicSoundLoop`) | `DrippingSound` |
