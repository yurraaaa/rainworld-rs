# `superstructure/electric/`

| File | Holds (key types) | Ports |
|---|---|---|
| `turn.rs` | the superstructure's electric objects in the game: `Room.Loaded` making `SSLightRod`s, `ZapCoil`s, `SuperStructureFuses` and `ZapCoilLight`s (and `AIMapReady` linking each light to its coil), their turns in the update list (a coil's loops, the zaps over the room's `physicalObjects` that flash, kick and kill, a flash's light made on its first update) | `Room.Loaded`, `ZapCoil.Update`, `ZapCoilLight.AIMapReady`, `SuperStructureFuses.Update` |
| `light_rod.rs` | `SSLightRod` and its `LightVessel`s (a light every 45 px riding the rod, faded by what the camera sees of the rod's depth, scaled by `ElectricPower`) | `SSLightRod` |
| `zap_coil.rs` | `ZapCoil` (rectangle, disruption, spells off, power switch, the two rectangular sound loops, `TriggerZap`), `ZapFlash` (a blue flash with its light) and `ZapCoilLight` (a blue light following its coil) | `ZapCoil`, `ZapCoilLight` |
| `room_fixtures.rs` | a realized room's fixtures, falls and superstructure electric and screen-grab objects in its update list | `Room.updateList` |
| `fuses.rs` | `SuperStructureFuses` (a grid of blinking cell lights: modes, malfunction spells, gravity fade; no update while culled) | `SuperStructureFuses` |
