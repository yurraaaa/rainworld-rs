# rw-sim divergences: the superstructure (D35, D36)

Fifth file of the `rw-sim` divergences. IDs are global. D35 (the projector family) is step
10a's; D36 (the electric fixtures) is step 10b's.

## D35: Superstructure projections (`SuperStructureProjector`, `ProjectedScanLines`, `ProjectedStars`)

Ported in `crates/sim/src/{projector,projected_scan_lines,star_matrix,room_projections,game_projections}.rs`,
the sprites in `crates/graphics/src/{projector_draw,projected_scan_lines_draw,star_matrix_draw}.rs`,
the `GlyphProjection` shader in `crates/render/shaders/glyph_projection.wgsl`. Notes:
`../rainworld-re/notes/superstructure-projections.md`.

Creation order: the projector and the scan lines are made by `Room.NowViewed`, in the same
settings-order loop as the green sparks, specks and spores (`particles_now_viewed`), so their
draws and update-list slots follow the settings' order; the parts take their slots in the
constructor, before the projector (`AddObject` after the constructor), so the constructor's
parts are older than the projector and the room walks the projector before them; parts added
later (refills, new cursors) are newer and walked before it. `StarMatrix` is made by `Room.Loaded`
(placed objects).

- **DLC branch omitted:** `SuperStructureProjector`'s constructor hides the projector
  (`visible = false` until its first roll) in the More Slugcats region `MS` unless Moon's heart
  is active (`ModManager.MSC`, `IsMoonHeartActive`). Every DLC counts as not installed.
- **`_gridOffset` is a renderer global.** The game sets it (`Shader.SetGlobalVector`) in every
  visible active `SingleGlyph.DrawSprites`, the value the same for all of them, and the
  `GlyphProjection` shader of every glyph, cursor and matrix reads it. The port sets it once per
  frame (`Renderer::set_grid_offset`) when the camera room has a visible projector with an active
  single glyph, and the renderer keeps it until it is set again, as a Unity global stays.
- **`ProjectedCircle` is not ported here.** In the base game only `CoralNeuron`, `CoralStem`
  and `SSOracleSwarmer` make it (when the room has a `SuperStructureProjector` effect; the
  DLC's `SpearMasterPearl` and `Inspector` too), none of which is ported. It comes with them.
- **The GPU's `sin` decides some glyphs.** `GlyphProjection` picks a cell's glyph from
  `frac(sin(x) * 43758.5453)`, which magnifies the last bits of `sin`; the same cell can show
  another glyph on another GPU (and in the port against the game). The picture is the same kind
  (a glyph in each cell, the shader's maths line by line), not pixel-identical. The GPU test
  compares cells whose hash is not near a glyph boundary.
- **The shader reads `_screenSize` and the level's `_spriteRect`** from the port's frame (the
  chosen screen size), as the game does; the sprites' 768 and 1366 px sizes and the `1400 x 800`
  texture grid in the shader are the game's constants (not `ScreenSize`).
- **Not drawn in the debug view** (no atlas): the objects are sprites of the `Shadows` and
  `BackgroundShortcuts` containers.
- **Setup draw order across objects:** `Room.Loaded`'s placed-object loop in the port
  collects first and adds after; `SlimeMold` (item radius) and `Rainbow` (seeded chance) draw
  at collection time while `StarMatrix` draws in the later add pass, where the game draws in
  object order. No installed room with `ProjectedStars` (`ss_c02`, `ss_f01`, `ss_f02`,
  `ss_h01`) places a `SlimeMold` or `Rainbow`, so nothing differs today.
- **Glyph colour note (matches the game):** a `SingleGlyph`'s sprite colour is set once in
  `InitiateSprites` and a glyph reused from the waiting room (`Reset`) keeps it; the port
  keeps `sprite_selected` from the constructor.
- **`Atlases/glyphs` has one mip level** (750 x 15, point filter, repeat), so the shader's
  `tex2D` has no lower mip to pick at cell seams; no mip support is needed.

## D36: Electric fixtures (`SSLightRod`, `ZapCoil`, `ZapCoilLight`, `SuperStructureFuses`)

Five Pebbles' electric placed objects (`rw_sim::ss_light_rod`, `zap_coil`, `ss_fuses`,
`game_ss_electric`; they live in `RoomFixtures`; sprites `rw_graphics::ss_electric`; the
`CustomDepth`, `FlareBomb` and `FlatLight` shaders already existed; MCP `light_rods`,
`zap_coils`, `zap_coil_lights`, `zap_flashes`, `fuses`). Every shared-stream draw is
ported in the game's order, with draw-count and order tests: a rod's vessels draw 3 each
when made and 2 on each wrap; a coil's quiet tick draws 8, a zap 3 or more plus its sound;
a fuse grid draws one for the lightness, one to four or more for each cell (one when the
`Pow(v, 0.5)` gate fails, two in modes 1/2/4/5, three in mode 3, four when the cell wobbles,
more on a jump), 2 or 3 more per tick on a gravity-dependent grid, and one for the
malfunction roll, while a camera sees it. The kill is the game's `Creature.Die` through the
port's creature death paths (D33's route for `ElectricDeath`). Notes:
`../../rainworld-re/notes/superstructure-electric.md`.

- **A `ZapFlash`'s sprites draw on the cosmetic stream (D1).** Its seven draws a frame (four
  before its light exists) and the hard-set of its light happen in the app's draw path
  (`App::ss_electric_draw_lists`), which runs before the lights draw, as the game's
  `drawableObjects` order has the flash before the light it makes. Like every cosmetic draw
  they are not interleaved with the other drawables' in the game's order; they are made in
  the app's list order.
- **`DepthAtCoordinate` without a camera picture is the sky (1).** `SSLightRod` samples
  camera 0's level image; the port reads the picture the app hands the sim (`CameraImage`);
  before the first one exists every sample reaches any depth.
- **An arena session is `!story`.** `SuperStructureFuses`' `broken` (the `CorruptionSpores`
  amount; outside an arena session in a region other than `SS` and `UW`, 1) takes the arena
  branch in dev starts (`load_room`, `--room`) too, as `Rainbow` does (D26). A dev start in
  `SL_*` or `SI_*` therefore keeps the room's amount where a story run has 1.
- **`AIMapReady` runs in the loading step.** A `ZapCoilLight` finds its coil (the first
  `ZapCoil` in the update list whose rectangle holds the light) as the room finishes
  loading, after every placed object exists.
- **An unpowered `ZapCoil` in a `BrokenZeroG` room needs the world's switch.** The
  condition reads `rainCycle.brokenAntiGrav`; a room with `BrokenZeroG` makes it, so the
  missing-switch case (a null reference in the game) reads as "not settled".
- **Only the bodies the port realizes are zapped:** the player, bat flies, lizards,
  overseers, rocks and spears (the room's `physicalObjects` the port has). Other
  creatures and items come with their ports.
- **Omitted DLC and Remix branches** (listed, not dropped): More Slugcats' region colours of
  an `SSLightRod`'s lights (`DM`, `MS`, `LM`, `SL`; `SB`) and its `DrawSprites` fog mix;
  `ZapCoil`'s `powered = false` in `MS` without Moon's heart and `ZapCoilLight`'s dark `MS`
  branch; Watcher's exemption of `BoxWorm` and `FireSprite` from the zap; the
  `ElectricSpear.Recharge` (DLCShared); the other readers of `room.zapCoils`: the Saint tongue
  (`Player.Tongue`), `TubeWorm`, `ElectricSpear`, the `Inspector` and its AI, Watcher's
  `ARZapper` (`SSLightRod` lights) and `WarpTear`; Moon's `SLOracleBehavior.fuses`; Remix's
  `ModManager.MMF` test of `brokenAntiGrav != null` in `Room.ElectricPower` (D34); the
  filtration power in it.
- **Developer tools only:** an `SSLightRod` rebuilds its lights when its data's length
  changes (`UpdateLightAmount`), and a fuse grid's `debugMode` colours show when its object
  is moved: the data never changes in the port, the colours are drawn for `debugMode > 0`
  as the game does.
- **Not here:** `DeepProcessing` and `DeepProcessingLight` (step 10c, with the screen grab).
