# `effects/particles/`

| File | Holds (key types) | Ports |
|---|---|---|
| `turn.rs` | the depth-faded particles in the game: `Room.Loaded`'s `SkyDandelions`/`GreenSparks` objects, `NowViewed`'s seeding of sparks, specks and spores (and its making the `SuperStructureProjector` and `ProjectedScanLines`), each one's turn in the update list, a spark's `LightSource`, the chunks the stalks read | `Room.Loaded`, `Room.NowViewed`, `GreenSparks` |
| `decal_turn.rs` | custom decals and rainbows joining the update list (the rainbow's `SeededRandom` gate), the rainbows' turns | `Room.Loaded` (`CustomDecal`, `Rainbow`, `RainbowNoFade`) |
| `placed_cosmetics_turn.rs` | cosmetic slime molds and dead token stalks joining the update list, the stalks' turns | `Room.Loaded` (`CosmeticSlimeMold`, `CosmeticSlimeMold2`, `SlimeMold`, `DeadTokenStalk`) |
| `decal.rs` | `CustomDecal`: grid division and the seeded grid mesh (positions, per-vertex colours) | `CustomDecal` |
| `rainbow.rs` | `Rainbow`/`RainbowNoFade`: position, radius, fade over the cycle's start-up, spawn condition | `Rainbow`, `RainbowNoFade` |
| `slime_mold.rs` | `CosmeticSlimeMold`: the seeded dots on the walls; the plain `SlimeMold`'s `SeededRandom` gate | `SlimeMold.CosmeticSlimeMold` |
| `token_stalk.rs` | `TokenStalk` without a token: arm, head, rope, two shared draws a tick | `CollectToken.TokenStalk` |
| `sky_dandelion.rs` | `SkyDandelions` spawner, `SkyDandelion`, `DandelionStalk`, the `DandelionPatch` planting (seeded) | `SkyDandelions.cs` |
| `green_sparks.rs` | `GreenSparks` spawner, `GreenSpark`, the `NowViewed` seeding | `GreenSparks.cs` |
| `zero_g_speck.rs` | `ZeroGSpeck` (`BlinkSpeck`, `CorruptionSpore`) on the `GenericZeroGSpeck` base; reads `Room.gravity` | `GenericZeroGSpeck.cs`, `BlinkSpeck.cs`, `CorruptionSpore.cs` |
| `room_particles.rs` | `RoomParticles`: a realized room's dandelions, stalks, sparks, specks and their spawners | `Room.updateList` |

## `insect/` (cosmetic insects; shared-stream draws, so simulated here)

`mod.rs` (`InsectType`, `InsectBase`, `CosmeticInsect`, `InsectCx`), `coordinator.rs`
(`InsectCoordinator`, `InsectGroup` placed objects, spawning), `avoider.rs`
(`CreatureAvoider`), one file per kind: `mini_fly.rs`, `fire_fly.rs`, `tiny_dragonfly.rs`,
`rock_flea.rs`, `grass_hopper.rs`, `red_swarmer.rs`, `ant.rs`, `beetle.rs` (also the
sitting/landing helpers Wasp and Moth share), `water_glowworm.rs`, `wasp.rs`, `moth.rs`;
`turn.rs` (the camera's deferred room change `apply_view_change`: `NoLongerViewed`/`NowViewed` of modules, particles and insects; cosmetic insects in the update list, creatures for the avoiders, the firefly's light); `tests/`.
