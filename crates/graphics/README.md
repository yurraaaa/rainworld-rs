# rw-graphics

The cosmetic graphics simulation of creatures and objects (tails, limbs,
cosmetics, overseer lens and mycelia) plus a backend-agnostic draw list
(`DrawList`) that `rw-render` draws. Depends on `rw-sim` (bodies it reads),
`rw-assets`, `rw-random`, `serde`. It does no GPU work; `rw-sim` stays
gameplay-only. This crate ticks after the sim; for lizards it feeds leg grip
back through the body's `LegsGripping`. `rw-hud` depends on it.

## File map (`crates/graphics/src/`)

| File | Holds | Game class |
|---|---|---|
| `lib.rs` | module list, re-exports | |
| `draw.rs` | `DrawList`, `Color`, sprite records, `InternalContainer` (an object's sprites moved into another's container) | Futile sprites |
| `atlas.rs` | `Atlas`, `AtlasFrame`: TexturePacker JSON frames | Futile atlas |
| `elements.rs` | slugcat atlas element names | |
| `math.rs` | `Mathf` / `Custom` helpers as traits | `Custom` |
| `body_part.rs` | `BodyPart` point mass pushed out of terrain | `BodyPart` |
| `tail.rs` | `TailSegment` | `TailSegment` |
| `limb.rs` | `Limb`, `LimbMode`, grip search | `Limb` |
| `hand.rs` | `SlugcatHand` (incl. the hands swinging along a vine in `VineGrab`) | `SlugcatHand` |
| `looker.rs` | `ObjectLooker` | `PlayerGraphics.PlayerObjectLooker` |
| `player_graphics.rs` | `PlayerGraphics` simulation | `PlayerGraphics` |
| `player_draw.rs` | the 12 slugcat sprites into a `DrawList` | `PlayerGraphics.DrawSprites` |
| `lizard_graphics.rs` | `LizardGraphics`, `IndividualVariations`: legs, tail, head | `LizardGraphics` |
| `lizard_limb.rs` | lizard leg stepping and grip | `LizardLimb` |
| `lizard_cosmetics.rs` | cosmetic kinds and the choice among them | `LizardCosmetics.*` |
| `lizard_cosmetics_draw.rs` | cosmetics' update, draw, palette | `LizardCosmetics` |
| `lizard_draw.rs` | body, tail mesh, legs, head sprites, the jaws' container place | `LizardGraphics.DrawSprites`, `AddToContainer` |
| `fly_graphics.rs` | bat fly body, wings | `FlyGraphics` |
| `insect_graphics.rs` | cosmetic insects' sprites (all kinds) | each insect's `InitiateSprites`/`DrawSprites`/`ApplyPalette` |
| `slime_mold.rs`, `token_stalk.rs` | cosmetic slime-mold dots (`Items`); the dead token stalk's arm, head, rope and lamp (`Midground`, lamp flicker on the cosmetic stream) | `SlimeMold.CosmeticSlimeMold`, `CollectToken.TokenStalk` |
| `sky_dandelion.rs`, `green_spark.rs`, `zero_g_speck.rs` | sky dandelions (`Items`/`Foreground`/`Shadows`) and their stalk mesh; green sparks; zero-gravity specks and corruption spores (`Midground`); depth ones use the `CustomDepth` shader | `SkyDandelions`, `GreenSparks`, `BlinkSpeck`, `CorruptionSpore` |
| `background_scene.rs` | a background scene element's sprites in `Water` (`DrawPos` for the frame's camera, scale, colour as shader input, 8-bit colours), the lightning's brightness draw | `BackgroundSceneElement.DrawSprites` and the element classes (`AboveCloudsView`, `RoofTopView`) |
| `dust_puff.rs` | a `DustPuff`'s `Futile_White` disc (`Spores`, `Background`), palette texel (9, 5) | `RoofTopView.DustPuff` |
| `decal.rs` | `CustomDecal`'s grid mesh over the decal's image in `Foreground` (`Decal` shader), `Unorm8` vertex colours | `CustomDecal.InitiateSprites`, `DrawSprites`, `TriangleMesh.MakeGridMesh` |
| `rainbow.rs` | `Rainbow`'s square over the `rainbow` gradient in `GrabShaders` (`Rainbow` shader) | `Rainbow.InitiateSprites`, `DrawSprites`, `CustomFSprite` |
| `overseer_graphics.rs` | `OverseerGraphics` simulation | `OverseerGraphics` |
| `overseer_draw.rs` | overseer sprites | `OverseerGraphics.DrawSprites` |
| `mycelium.rs` | a strand's mesh (`MyceliumSprites`) and `TriangleMesh.MakeLongMesh`'s triangles and UVs (`LongMesh`); the strands are `rw-sim`'s | `Mycelium` (InitiateSprites, DrawSprites, UpdateColor) |
| `coral_neuron_draw.rs` | a `CoralNeuron`'s two meshes (colour ramp; `CoralNeuron` shader lines), its strands and bumps in `Items` (ping dots in `Water` with `WaterLights`) | `CoralNeuron` (MeshColor, InitiateSprites, DrawSprites, AddToContainer) |
| `projected_circle_draw.rs` | a `ProjectedCircle`'s two `VectorCircle` discs, spokes and lines in `BackgroundShortcuts` (`CircleSurroundings`: the owners and circles it reads) | `ProjectedCircle` |
| `wall_mycelia_draw.rs` | a `WallMycelia`'s strands in `Items` (transparent black fading in, blue tip), culled ones left out | `WallMycelia` |
| `hologram_matrix.rs` | overseer lens wireframe | `OverseerGraphics.HologramMatrix` |
| `hologram_shape.rs` | line geometry of hologram parts | `OverseerHolograms` |
| `hologram_draw.rs` | hologram part sprites and lines | `OverseerHologram.DrawSprites` |
| `light_beam.rs`, `spot_light.rs`, `quad_grid.rs` | placed light beam mesh and per-frame alpha (`ForegroundLights`), spot light mesh (`Shadows`), the grid mesh over a quad (`GetIdealGridDiv` 250/150 px, `MakeGridMesh`) | `LightBeam`, `SpotLight`, `TriangleMesh.MakeGridMesh` |
| `holo_light.rs`, `light_source.rs` | guide light; light disc (`Futile_White`, or `LightMask0` for a `RedlightLight`) | `HologramLight`, `LightSource` |
| `light_fixture.rs` | `HolyFire`'s embers (`deerEyeB`, `Midground`) and `SlimeMoldLight`'s five discs (`Foreground`, `LightSource` shader) | `HolyFire.HolyFireSprite`, `SlimeMoldLight` |
| `electric_death.rs` | `ElectricDeath`'s ten sprites (the `ElectricDeath` web and nine green `LightSource` discs, `Water`) and a `SparkFlash`'s three (`LightSource`, `FlatLight`, `FlareBomb` discs); positions in room pixels (`cam_pos` plus screen offsets) | `ElectricDeath`, `LightFlash.Draw`, `SparkFlash` |
| `projector_draw.rs` | a `SuperStructureProjector`'s parts in `Shadows`: scan line bars, cursor box with a `glyphs` sprite, single glyph, matrix; `ProjectionView` (camera position and the room's camera) | `SuperStructureProjector` |
| `projected_scan_lines_draw.rs` | a `ProjectedScanLines` line (one bar, `Shadows`) | `ProjectedScanLines` |
| `star_matrix_draw.rs` | a `StarMatrix`'s squares, lines and `VectorCircle` rings in `BackgroundShortcuts` | `StarMatrix` |
| `ss_grab.rs`, `ss_grab_tests.rs` | a `DeepProcessing`'s grid mesh (`Foreground`, `DeepProcessing` shader, vertex colour `(fromDepth, toDepth, power, intensity)`), a `GravityDisruptor`'s disc (`HUD`, `GravityDisruptor` shader) with its sixteen streaks (`Foreground`, `CustomDepth`, `ApplyDepth`) and a `DisruptorSpeck`'s pixel | `DeepProcessing`, `GravityDisruptor`, `GravityDisruptor.DisruptorSpeck` (InitiateSprites, DrawSprites) |
| `ss_electric.rs`, `ss_electric_tests.rs` | an `SSLightRod`'s bar (`Water`, `CustomDepth`), a `ZapCoil`'s eight-vertex flare mesh (`FlareBomb`), a `ZapFlash`'s two discs (`FlareBomb`, `FlatLight`, seven cosmetic draws a frame, setting its light) and `SuperStructureFuses`' cell lights (`Foreground`, `CustomDepth`, `ApplyDepth`) | `SSLightRod`, `ZapCoil`, `ZapCoil.ZapFlash`, `SuperStructureFuses` (InitiateSprites, DrawSprites) |
| `water_fall.rs` | `WaterFall`'s quad (`WaterFall` shader) and bubbles in `Background` | `WaterFall` |
| `weapon_graphics.rs` | rock and spear sprites | `Rock`, `Spear` |
| `water_drip.rs`, `cosmetic_graphics.rs` | drips (`DripPalette`, water-coloured too); sparks, explosion spikes, lizard and air bubbles, splashes, a `NeuronSpark`'s disc and pixel (`Foreground`, five cosmetic draws) | `WaterDrip`, `Spark`, `ExplosionSpikes`, `LizardBubble`, `Bubble`, `Splash` |
| `shortcut_graphics.rs` | pipe pixels, entrance symbols | `ShortcutGraphics` |
| `shelter_door_graphics.rs` | shelter door sprites | `ShelterDoor` |
| `sun_blocker.rs` | the `SunBlock` effect's screen-covering sprite in `Shadows` | `SunBlocker` |
| `tests.rs`, `coral_tests.rs`, `coral_neuron_tests.rs`, `hologram_tests.rs`, `shortcut_tests.rs`, `placed_visuals_tests.rs`, `projection_tests.rs` | unit tests | |

## Where is X?

- Slugcat tail and limbs: `player_graphics.rs`, `tail.rs`, `hand.rs`, `limb.rs`.
- Slugcat sprites and frame choice: `player_draw.rs`; atlas names in `elements.rs`.
- Slugcat gaze: `looker.rs`.
- Lizard body, legs, head: `lizard_graphics.rs`, `lizard_limb.rs`, `lizard_draw.rs`.
- Lizard scales, spikes, tufts: `lizard_cosmetics.rs`, `lizard_cosmetics_draw.rs`.
- Overseer body and eye: `overseer_graphics.rs`, `overseer_draw.rs`; hairs: `mycelium.rs` (the strands in `rw-sim`'s `mycelium.rs`); wall mycelia: `wall_mycelia_draw.rs`; coral neurons and their projected circles: `coral_neuron_draw.rs`, `projected_circle_draw.rs`.
- Overseer lens and holograms: `hologram_matrix.rs`, `hologram_shape.rs`, `hologram_draw.rs`.
- Bat flies: `fly_graphics.rs`.
- Cosmetic insects: `insect_graphics.rs` (simulation in `rw-sim` `insect/`).
- Sky dandelions, green sparks, zero-g specks and spores: `sky_dandelion.rs`, `green_spark.rs`, `zero_g_speck.rs` (simulation in `rw-sim`).
- Background scenes (sky, clouds, distant buildings, a roof's city), dust puffs: `background_scene.rs`, `dust_puff.rs` (simulation in `rw-sim` `background_scene.rs`).
- Custom decals, rainbows: `decal.rs`, `rainbow.rs` (simulation in `rw-sim` `decal.rs`, `rainbow.rs`; items other than `Futile_White` meshes are `DrawItem::ElementMesh`).
- Slime-mold patches, dead token stalks: `slime_mold.rs`, `token_stalk.rs` (simulation in `rw-sim`).
- Rocks and spears: `weapon_graphics.rs`.
- Shelter door, pipes: `shelter_door_graphics.rs`, `shortcut_graphics.rs`.
- `ElectricDeath` web and flashes, `SparkFlash`: `electric_death.rs` (simulation in `rw-sim` `electric_death.rs`, shaders in `rw-render` `electric_death.wgsl`).
- Superstructure projections (glyph projector, scan lines, stars): `projector_draw.rs`, `projected_scan_lines_draw.rs`, `star_matrix_draw.rs` (simulation in `rw-sim`, `GlyphProjection` shader in `rw-render`, D35).
- Superstructure rods, zap coils, flashes, fuses: `ss_electric.rs` (simulation in `rw-sim` `ss_light_rod.rs`, `zap_coil.rs`, `ss_fuses.rs`).
- Superstructure `DeepProcessing` quads, `GravityDisruptor` discs and specks: `ss_grab.rs` (simulation in `rw-sim` `deep_processing.rs`, `gravity_disruptor.rs`).
- Light discs: `light_source.rs`, `holo_light.rs`; light fixtures' embers and discs: `light_fixture.rs`; waterfalls: `water_fall.rs`; beams and spots: `light_beam.rs`, `spot_light.rs`, `quad_grid.rs`.
- Atlas frame lookup: `atlas.rs`.
- The draw list a renderer consumes: `draw.rs`.

## Cross-links

Notes in `../../../rainworld-re/notes/`: `slugcat-graphics.md`, `overseer.md`,
`sprite-atlases.md`, `room-rendering.md`, `shortcuts.md`, `shelter.md`,
`water-rendering.md`. Surveys: `lizards.md`, `bat-flies.md`,
`rocks-spears.md`, `depth-scenes.md`.

Divergences: `../../docs/divergences/` (one file per crate, IDs global): D8 slugcat graphics, D5 shortcuts, D7 room
graphics, D14 worlds, rooms and the realizer (creature and overseer
graphics), D17 shelters.
