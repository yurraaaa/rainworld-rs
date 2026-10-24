# rw-graphics

The cosmetic graphics simulation of creatures and objects (tails, limbs,
cosmetics, overseer lens and mycelia) plus a backend-agnostic draw list
(`DrawList`) that `rw-render` draws. Depends on `rw-sim` (bodies it reads),
`rw-assets`, `rw-random`, `serde`. It does no GPU work; `rw-sim` stays
gameplay-only. This crate ticks after the sim; for lizards it feeds leg grip
back through the body's `LegsGripping`. `rw-hud` depends on it.

## Layout

`src/` is split by topic, following `rw-sim`'s folders: `core/` (draw list, atlas, math),
`body/`, `player/`, `creature/{lizard,fly,overseer}/`, `item/`, `water/`,
`effects/{lights,sky,particles,gameplay}/` (plus `effects/cosmetic.rs`),
`superstructure/{projections,coral}/` (plus `electric.rs`, `grab.rs`) and `game/`. The
module tree is the folder tree; a folder's `draw.rs` is the `DrawSprites` of the type
beside it. The modules are private: `lib.rs` re-exports the crate's API at the root
(`rw_graphics::PlayerGraphics`, ...), and `elements` (`player/elements.rs`) stays public.

**Tests**: unit tests of `x.rs` are `tests/x.rs` in the same folder, declared from `x.rs`
as `#[cfg(test)] #[path = "tests/x.rs"] mod tests;`. `PlayerGraphics`' tests are
`player/tests/mod.rs` (the player helpers) with one child a subject (`frames.rs`,
`poles.rs`, `corridor.rs`, `malnourished.rs`, `zero_g.rs`). `src/tests/mod.rs` holds the
shared fixtures (test rooms, float asserts, a sprite filter, a lizard-module stand-in). `../tests/` holds
only the `installed_scene_images.rs` integration test.

## Files by folder (`crates/graphics/src/`)

`lib.rs`: module list, re-exports. Each folder also has a `mod.rs` (module index):
`core/`, `body/`, `player/`, `creature/`, `creature/lizard/`, `creature/fly/`,
`creature/overseer/`, `item/`, `water/`, `effects/`, `effects/lights/`, `effects/sky/`,
`effects/particles/`, `effects/gameplay/`, `superstructure/`,
`superstructure/projections/`, `superstructure/coral/`, `game/`.

### `core/`

| File | Holds | Game class |
|---|---|---|
| `draw.rs` | `DrawList`, `Color`, sprite records, `InternalContainer` (an object's sprites moved into another's container) | Futile sprites |
| `atlas.rs` | `Atlas`, `AtlasFrame`: TexturePacker JSON frames | Futile atlas |
| `math.rs` | `Mathf` / `Custom` helpers as traits | `Custom` |

### `body/`

| File | Holds | Game class |
|---|---|---|
| `body_part.rs` | `BodyPart` point mass pushed out of terrain | `BodyPart` |
| `tail.rs` | `TailSegment` | `TailSegment` |
| `limb.rs` | `Limb`, `LimbMode`, grip search | `Limb` |

### `player/`

| File | Holds | Game class |
|---|---|---|
| `graphics.rs` | `PlayerGraphics` simulation | `PlayerGraphics` |
| `draw.rs` | the 12 slugcat sprites into a `DrawList` | `PlayerGraphics.DrawSprites` |
| `hand.rs` | `SlugcatHand` (incl. the hands swinging along a vine in `VineGrab`) | `SlugcatHand` |
| `looker.rs` | `ObjectLooker` (with `lookAtPoint`, set by the sky look) | `PlayerGraphics.PlayerObjectLooker` |
| `elements.rs` | slugcat atlas element names (the public `rw_graphics::elements`) | |

### `creature/`

| File | Holds | Game class |
|---|---|---|
| `lizard/graphics.rs` | `LizardGraphics`, `IndividualVariations`: legs, tail, head | `LizardGraphics` |
| `lizard/limb.rs` | lizard leg stepping and grip | `LizardLimb` |
| `lizard/draw.rs` | body, tail mesh, legs, head sprites, the jaws' container place | `LizardGraphics.DrawSprites`, `AddToContainer` |
| `lizard/cosmetics.rs` | cosmetic kinds and the choice among them | `LizardCosmetics.*` |
| `lizard/cosmetics_draw.rs` | cosmetics' update, draw, palette | `LizardCosmetics` |
| `lizard/bubble.rs` | a lizard's bubble and its head shield's flash (`Midground`; the flash draws on the cosmetic stream) | `LizardBubble`, `StationaryEffect` |
| `fly/graphics.rs` | bat fly body, wings | `FlyGraphics` |
| `overseer/graphics.rs` | `OverseerGraphics` simulation | `OverseerGraphics` |
| `overseer/draw.rs` | overseer sprites | `OverseerGraphics.DrawSprites` |
| `overseer/hologram_matrix.rs` | overseer lens wireframe | `OverseerGraphics.HologramMatrix` |
| `overseer/hologram_shape.rs` | line geometry of hologram parts | `OverseerHolograms` |
| `overseer/hologram_draw.rs` | hologram part sprites and lines | `OverseerHologram.DrawSprites` |
| `overseer/holo_light.rs` | guide light | `HologramLight` |

### `item/`

| File | Holds | Game class |
|---|---|---|
| `weapon.rs` | rock and spear sprites | `Rock`, `Spear` |

### `water/`

| File | Holds | Game class |
|---|---|---|
| `water_drip.rs` | drips (`DripPalette`, water-coloured too) | `WaterDrip` |
| `waterfall.rs` | `WaterFall`'s quad (`WaterFall` shader) and bubbles in `Background` | `WaterFall` |
| `bubble.rs` | an air bubble (`LizardBubble5`, `Midground`) | `Bubble` |
| `splash.rs` | a splash's two `WaterSplash` quads (`Water`, `GrabShaders`) | `Splash` |

### `effects/`

| File | Holds | Game class |
|---|---|---|
| `cosmetic.rs` | the `CosmeticSprites` dispatch of every cosmetic's draw; `LizardModule`, `LizardColors`, `NoLizards` (what cosmetics read of lizard graphics); sparks, explosion spikes, a `NeuronSpark`'s disc and pixel (`Foreground`, five cosmetic draws) | `Spark`, `ExplosionSpikes`, `NeuronSpark` |
| `lights/light_source.rs` | light disc (`Futile_White`, or `LightMask0` for a `RedlightLight`) | `LightSource` |
| `lights/light_fixture.rs` | `HolyFire`'s embers (`deerEyeB`, `Midground`) and `SlimeMoldLight`'s five discs (`Foreground`, `LightSource` shader) | `HolyFire.HolyFireSprite`, `SlimeMoldLight` |
| `lights/light_beam.rs` | placed light beam mesh and per-frame alpha (`ForegroundLights`); spot light mesh (`Shadows`) | `LightBeam`, `SpotLight` |
| `lights/quad_grid.rs` | the grid mesh over a quad (`GetIdealGridDiv` 250/150 px, `MakeGridMesh`) | `TriangleMesh.MakeGridMesh` |
| `lights/sun_blocker.rs` | the `SunBlock` effect's screen-covering sprite in `Shadows` | `SunBlocker` |
| `sky/background_scene.rs` | a background scene element's sprites in `Water` (`DrawPos` for the frame's camera, scale, colour as shader input, 8-bit colours), the lightning's brightness draw; a `DustPuff`'s `Futile_White` disc (`Spores`, `Background`), palette texel (9, 5) | `BackgroundSceneElement.DrawSprites` and the element classes (`AboveCloudsView`, `RoofTopView`, `RoofTopView.DustPuff`) |
| `particles/insect.rs` | cosmetic insects' sprites (all kinds) | each insect's `InitiateSprites`/`DrawSprites`/`ApplyPalette` |
| `particles/sky_dandelion.rs`, `particles/green_sparks.rs`, `particles/zero_g_speck.rs` | sky dandelions (`Items`/`Foreground`/`Shadows`) and their stalk mesh; green sparks; zero-gravity specks and corruption spores (`Midground`); depth ones use the `CustomDepth` shader | `SkyDandelions`, `GreenSparks`, `BlinkSpeck`, `CorruptionSpore` |
| `particles/slime_mold.rs`, `particles/token_stalk.rs` | cosmetic slime-mold dots (`Items`); the dead token stalk's arm, head, rope and lamp (`Midground`, lamp flicker on the cosmetic stream) | `SlimeMold.CosmeticSlimeMold`, `CollectToken.TokenStalk` |
| `particles/decal.rs` | `CustomDecal`'s grid mesh over the decal's image in `Foreground` (`Decal` shader), `Unorm8` vertex colours | `CustomDecal.InitiateSprites`, `DrawSprites`, `TriangleMesh.MakeGridMesh` |
| `particles/rainbow.rs` | `Rainbow`'s square over the `rainbow` gradient in `GrabShaders` (`Rainbow` shader) | `Rainbow.InitiateSprites`, `DrawSprites`, `CustomFSprite` |
| `gameplay/electric_death.rs` | `ElectricDeath`'s ten sprites (the `ElectricDeath` web and nine green `LightSource` discs, `Water`) and a `SparkFlash`'s three (`LightSource`, `FlatLight`, `FlareBomb` discs); positions in room pixels (`cam_pos` plus screen offsets) | `ElectricDeath`, `LightFlash.Draw`, `SparkFlash` |
| `gameplay/room_rain.rs` | `RoomRain`'s sprites: the `DeathRain` quad (`GrabShaders`) and the visible splash sprites (`Items`; the last is never added), the shader globals (`RainShaderGlobals`), the splash draws on the cosmetic stream | `RoomRain` (`InitiateSprites`, `DrawSprites`, `AddToContainer`) |

### `superstructure/`

| File | Holds | Game class |
|---|---|---|
| `projections/projector.rs` | a `SuperStructureProjector`'s parts in `Shadows`: scan line bars, cursor box with a `glyphs` sprite, single glyph, matrix; `ProjectionView` (camera position and the room's camera); a `ProjectedScanLines` line (one bar, `Shadows`) | `SuperStructureProjector`, `ProjectedScanLines` |
| `projections/star_matrix.rs` | a `StarMatrix`'s squares, lines and `VectorCircle` rings in `BackgroundShortcuts` | `StarMatrix` |
| `projections/projected_circle.rs` | a `ProjectedCircle`'s two `VectorCircle` discs, spokes and lines in `BackgroundShortcuts` (`CircleSurroundings`: the owners and circles it reads) | `ProjectedCircle` |
| `electric.rs` | an `SSLightRod`'s bar (`Water`, `CustomDepth`), a `ZapCoil`'s eight-vertex flare mesh (`FlareBomb`), a `ZapFlash`'s two discs (`FlareBomb`, `FlatLight`, seven cosmetic draws a frame, setting its light) and `SuperStructureFuses`' cell lights (`Foreground`, `CustomDepth`, `ApplyDepth`) | `SSLightRod`, `ZapCoil`, `ZapCoil.ZapFlash`, `SuperStructureFuses` (InitiateSprites, DrawSprites) |
| `grab.rs` | a `DeepProcessing`'s grid mesh (`Foreground`, `DeepProcessing` shader, vertex colour `(fromDepth, toDepth, power, intensity)`), a `GravityDisruptor`'s disc (`HUD`, `GravityDisruptor` shader) with its sixteen streaks (`Foreground`, `CustomDepth`, `ApplyDepth`) and a `DisruptorSpeck`'s pixel | `DeepProcessing`, `GravityDisruptor`, `GravityDisruptor.DisruptorSpeck` (InitiateSprites, DrawSprites) |
| `coral/mycelium.rs` | a strand's mesh (`MyceliumSprites`) and `TriangleMesh.MakeLongMesh`'s triangles and UVs (`LongMesh`), the strands being `rw-sim`'s; a `WallMycelia`'s strands in `Items` (transparent black fading in, blue tip), culled ones left out | `Mycelium` (InitiateSprites, DrawSprites, UpdateColor), `WallMycelia` |
| `coral/stem.rs` | a `StemSegment`'s sprites in `Items`: its strands, the bridge `Circle20` to the next segment, two meshes along a curve through the chunks (base colour and highlight from the camera's darkness) and three `Circle20` a dot (glow in `Water` with `WaterLights`; its blink draws from the cosmetic stream) | `StemSegment` (BaseColor, HighLightColor, InitiateSprites, DrawSprites, ApplyPalette, AddToContainer) |
| `coral/neuron.rs` | a `CoralNeuron`'s two meshes (colour ramp; `CoralNeuron` shader lines), its strands and bumps in `Items` (ping dots in `Water` with `WaterLights`) | `CoralNeuron` (MeshColor, InitiateSprites, DrawSprites, AddToContainer) |
| `coral/circuit.rs` | a `CoralCircuit`'s sprites: each bit one `maze` sprite in `Background` (the window of the texture in the 8-bit vertex colour, the flip as `scaleX`, `CoralCircuit` or `DeadCoralCircuit` by `InPosition`), each connector's strands and four `Circle20` (the last one's colour from the cosmetic stream) and each crab's two `Circle20` and leg strips in `Midground` (`CircuitSprites`) | `CircuitBit`, `CircuitConnector`, `CircuitCrab`, `CrabLeg` (InitiateSprites, DrawSprites) |

### `game/`

| File | Holds | Game class |
|---|---|---|
| `shortcut.rs` | pipe pixels, entrance symbols | `ShortcutGraphics` |
| `shelter_door.rs` | shelter door sprites | `ShelterDoor` |

## Where is X?

- Slugcat tail and limbs: `player/graphics.rs`, `body/tail.rs`, `player/hand.rs`, `body/limb.rs`.
- Slugcat sprites and frame choice: `player/draw.rs`; atlas names in `player/elements.rs`.
- Slugcat gaze: `player/looker.rs`.
- Lizard body, legs, head: `creature/lizard/graphics.rs`, `limb.rs`, `draw.rs`.
- Lizard scales, spikes, tufts: `creature/lizard/cosmetics.rs`, `cosmetics_draw.rs`; bubbles and the head shield's flash: `creature/lizard/bubble.rs`.
- Overseer body and eye: `creature/overseer/graphics.rs`, `draw.rs`; hairs: `superstructure/coral/mycelium.rs` (the strands in `rw-sim`'s `superstructure/coral/mycelium.rs`), which also draws wall mycelia; coral neurons, stem segments and their projected circles: `superstructure/coral/neuron.rs`, `stem.rs`, `superstructure/projections/projected_circle.rs`; coral circuits (bits, connectors, crabs): `superstructure/coral/circuit.rs`.
- Overseer lens and holograms: `creature/overseer/hologram_matrix.rs`, `hologram_shape.rs`, `hologram_draw.rs`.
- Bat flies: `creature/fly/graphics.rs`.
- Cosmetic insects: `effects/particles/insect.rs` (simulation in `rw-sim` `effects/particles/insect/`).
- Sky dandelions, green sparks, zero-g specks and spores: `effects/particles/sky_dandelion.rs`, `green_sparks.rs`, `zero_g_speck.rs` (simulation in `rw-sim`).
- Background scenes (sky, clouds, distant buildings, a roof's city) and dust puffs: `effects/sky/background_scene.rs` (simulation in `rw-sim` `effects/sky/background_scene.rs`).
- Custom decals, rainbows: `effects/particles/decal.rs`, `rainbow.rs` (simulation in `rw-sim` `effects/particles/decal.rs`, `rainbow.rs`; items other than `Futile_White` meshes are `DrawItem::ElementMesh`).
- Slime-mold patches, dead token stalks: `effects/particles/slime_mold.rs`, `token_stalk.rs` (simulation in `rw-sim`).
- Sparks, explosion spikes, neuron sparks and the dispatch of every cosmetic: `effects/cosmetic.rs`; air bubbles and splashes: `water/bubble.rs`, `water/splash.rs`.
- Rocks and spears: `item/weapon.rs`.
- Shelter door, pipes: `game/shelter_door.rs`, `game/shortcut.rs`.
- `ElectricDeath` web and flashes, `SparkFlash`: `effects/gameplay/electric_death.rs` (simulation in `rw-sim` `effects/gameplay/electric_death.rs`, shaders in `rw-render` `electric_death.wgsl`).
- Superstructure projections (glyph projector, scan lines, stars): `superstructure/projections/projector.rs`, `star_matrix.rs` (simulation in `rw-sim`, `GlyphProjection` shader in `rw-render`, D35).
- Superstructure rods, zap coils, flashes, fuses: `superstructure/electric.rs` (simulation in `rw-sim` `superstructure/electric/light_rod.rs`, `zap_coil.rs`, `fuses.rs`).
- Superstructure `DeepProcessing` quads, `GravityDisruptor` discs and specks: `superstructure/grab.rs` (simulation in `rw-sim` `superstructure/grab/deep_processing.rs`, `gravity_disruptor.rs`).
- Light discs: `effects/lights/light_source.rs`, `creature/overseer/holo_light.rs`; light fixtures' embers and discs: `effects/lights/light_fixture.rs`; waterfalls: `water/waterfall.rs`; beams and spots: `effects/lights/light_beam.rs`, `quad_grid.rs`.
- Atlas frame lookup: `core/atlas.rs`.
- The draw list a renderer consumes: `core/draw.rs`.

## Cross-links

Notes in `../../../rainworld-re/notes/`: `slugcat-graphics.md`, `overseer.md`,
`sprite-atlases.md`, `room-rendering.md`, `shortcuts.md`, `shelter.md`,
`water-rendering.md`. Surveys: `lizards.md`, `bat-flies.md`,
`rocks-spears.md`, `depth-scenes.md`.

Divergences: `../../docs/divergences/` (one file per crate, IDs global): D8 slugcat graphics, D5 shortcuts, D7 room
graphics, D14 worlds, rooms and the realizer (creature and overseer
graphics), D17 shelters.
