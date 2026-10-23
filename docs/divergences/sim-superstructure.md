# rw-sim: the superstructure (Five Pebbles' rooms)

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
