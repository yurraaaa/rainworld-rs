# `effects/lights/`

| File | Holds (key types) | Ports |
|---|---|---|
| `light_beam_turn.rs` | placed `LightBeam`s and `SpotLight`s joining the update list, a beam's turn (environment colour), `ApplyPalette` | `Room.Loaded` (`LightBeam`, `SpotLight`), `LightBeam.Update` |
| `light_fixture_turn.rs` | placed `LightFixture`s (`Redlight`, `HolyFire`, `SlimeMoldLight`) and the tile parse's `WaterFall`s joining the update list, their turns in it (the `RedlightLight` made on the first update, the embers shed, a fall's drips, bubbles and push on bodies), `Room.ElectricPower` | `Room.Loaded` (`LightFixture`), `Room.LoadFromDataString` (waterfalls), `Redlight.Update`, `WaterFall.Update` |
| `turn.rs` | light sources in the update list: placed lights (`Room.Loaded`), the glowing player's light (`PlayerGlow`), camera 0's picture they read, `Darkness`, `LightSourceExposure`/`LightSourceColor`/`DarknessOfPoint` | `Room` light members, `LightSource.Update`, `PlayerGraphics.Update` light |
| `light_beam.rs` | `LightBeam` (quad, environment and palette colours), `SpotLight` (quad) | `LightBeam`, `SpotLight` |
| `light_source.rs` | `LightSource` (`Alpha`, `Lightness`, update) | `LightSource` |
| `light_fixture.rs` | `Redlight` (flicker, gravity-dependent power), `HolyFire` with `HolyFireSprite`, `SlimeMoldLight` (seeded discs) | `LightFixture`, `Redlight`, `HolyFire`, `SlimeMoldLight` |
