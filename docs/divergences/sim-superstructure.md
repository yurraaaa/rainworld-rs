# rw-sim divergences: the superstructure (D35, D36, D37)

Fifth file of the `rw-sim` divergences. IDs are global. D35 (the projector family) is step
10a's; D36 (the electric fixtures) is step 10b's; D37 (the screen-grab effects) is step 10c's.

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
- **`ProjectedCircle` comes with its owners.** In the base game only `CoralNeuron`, `CoralStem`
  and `SSOracleSwarmer` make it (when the room has a `SuperStructureProjector` effect; the
  DLC's `SpearMasterPearl` and `Inspector` too). It is ported with the first owner, `CoralNeuron`
  (D40); the stems' and swarmers' circles come with them.
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
- **Not here:** `DeepProcessing` and `DeepProcessingLight` are D37's (step 10c).

## D37: Screen-grab effects (`DeepProcessing`, `DeepProcessingLight`, `GravityDisruptor`)

Ported in `crates/sim/src/{deep_processing,gravity_disruptor,game_ss_grab}.rs` (the objects,
their placed-object creation in `Room.Loaded`'s order, the disruptor's push on the room's
bodies), the sprites in `crates/graphics/src/ss_grab.rs`, the shaders in
`crates/render/shaders/ss_grab.wgsl` with the grab changes in `crates/render/src/gpu.rs`,
the data in `rw-assets` `placed_electric.rs`. Notes: `../rainworld-re/notes/superstructure-grab.md`.

- **`_GrabTexture` is the last unnamed `GrabPass`, for every reader (not a divergence).**
  `DeepProcessing.shader` has no `GrabPass`; `GravityDisruptor.shader` has one. The grab
  texture starts every frame as the screen behind the level (`LevelColor`'s own `GrabPass
  { }`) and each grabbing draw replaces it with the frame it found; every shader without a
  `GrabPass` that reads `_GrabTexture` (`LightSource`, `UnderWaterLight`, `HoloGrid`,
  `ElectricDeath`, `FlareBomb`, `WaterSplash`, `WaterLight`, `LightBeam`, `SpecificDepth`,
  `CustomDepth`, `Decal`, `Fog`,
  the background scenes, `WaterSurface`/`DeepWater`, `DeepProcessing`) reads that texture.
  `LevelColor`'s and `DeathFall`'s/the bloom effects' `_PreLevelColorGrab` stays the
  pre-level frame. A reader drawn after a disruptor reads the frame the disruptor grabbed
  (before its own draw), not the rippled result.
- **`_NoiseTex` mips (not a divergence).** `Palettes/noise` has 7 mips, bilinear (the nearest
  level), repeat, `m_MipBias` 0 (read with the port's `Texture2D` reader). Every shader
  reading it (`DeepProcessing`, the level and water shaders, the background scenes, the
  glyph projection, `WaterSplash`, `WaterLight`, `UnderWaterLight`, `ElectricDeath`, `Fog`,
  `Decal`, `Rainbow`, `WaterFall`) samples it with the level the GPU's derivatives pick over
  the 2x2 pixel block, taking its taps before any early return, as D3D does. (`_NoiseTex2`
  is still read at level 0: D-notes in `docs/divergences/render.md`.)
- **`HUD` container order (not a divergence).** Sprites draw in the order they were added to
  their container. The disc is in the room camera's `HUD` container, above `Bloom`. The HUD's
  first container (`fContainers[0]`, the region map; `HUD2` above everything) gets the map's
  sprites on `Map.Update`'s first run (`!mapLoaded`), one `RoomCamera.Update` after the camera's
  first `ChangeRoom` made the room's sprites: in the session's first room the map is above the
  disc (not rippled); after any later camera move `ChangeRoom` adds the room's sprites after
  the map's, so the disc ripples it. The app queues the map's quads (`Scene::prepare_hud`)
  after the disc until a camera move comes after the map's first update (`MapOrder`), before
  it from then on; the rest of the HUD after the room. Other `HUD` container occupants the game has (arena score labels, debug
  sprites, MSC and Watcher sprites, a lizard cosmetic with `SpritesOverlap.HUD`) are not
  ported.
- **`roomSpecks`.** `GravityDisruptor.AIMapReady` collects the `GenericZeroGSpeck`s in the
  update list. The session's first room is made by `RoomPreparer`'s constructor (`Loaded`),
  then `RainWorldGame`'s constructor moves the camera to it (`NowViewed`: the specks), and
  only later does `RoomPreparer.Update` run `AIMapReady`: its `roomSpecks` hold the room's
  specks. The port links the first room after its `NowViewed` (the camera's first tick) and
  any room realized ahead of the camera at its load, so before the camera enters it. That is
  the game's order too: `ReadyForPlayer` needs the loading (`loadingProgress` 3, then
  `AIMapReady`) done, so the camera's `NowViewed` comes after and `roomSpecks` is empty. The
  port's loads are synchronous (D14), the same two cases come out; the game's thread can
  only make `AIMapReady` later, never earlier than `Loaded`.
- **Draws:** `DeepProcessing`: 1 a tick, 3 on a tick that moves its power (1 in 14); no
  constructor or draw-time draws. `DeepProcessingLight`: 1 a tick. `GravityDisruptor`:
  98 in the constructor (20 specks times 4, 16 streaks, `pointDir`, `dirFac`), at least 34 a
  tick (the two 0.0125 rolls and two a streak), a hit adding 1 or 2; a `DisruptorSpeck` 1 a
  tick, 2 on a new orbit. The constructor's draws are in `Room.Loaded`'s placed-object
  order with the other ported objects' (the unported placed objects' draws are missing, as
  for every object ported so far).
- **Only the bodies the port realizes are swirled:** the player, bat flies, lizards,
  overseers, rocks and spears (the room's `physicalObjects` the port has); the rest comes with
  their ports (as D36's zaps).
- **Skipped, a Rain World dev tool (user decision):** `DeepProcessing`'s `meshDirty` (the quad's
  corners changing in the editor: `UpdateVerts` and the re-made mesh when the ideal grid
  division changes) and the disruptor's moved placed object (`debugMode` is drawn as the
  game does, but `placedObject.pos` is constant): the quad is read once.
- **The disc's `normalize(0)`.** `GravityDisruptor.shader` normalises `uv - 0.5`, NaN at
  the exact centre pixel (undefined on a GPU); the port uses a zero direction there (no
  displacement). No pixel centre of an even-sized disc lies on its centre.
- **Omitted DLC branches:** none in these classes. The `GravityDisruptor` shader is also More
  Slugcats' `EnergyCell`'s (not ported); `DeepProcessingLight` has no DLC code; Remix's
  `ScreenSize` is not read by any of them (`_screenSize` is the chosen resolution, as
  everywhere).

## D38: Coral (`CoralNeuronSystem`, `WallMycelia`, `Mycelium`, `NeuronSpark`; step 10d1)

Ported in `crates/sim/src/{coral_system,mycelium,wall_mycelia,neuron_spark,game_coral}.rs`
(the system, the strands, the wall object, the spark as a `CosmeticKind`), the sprites in
`crates/graphics/src/{mycelium,wall_mycelia_draw,cosmetic_graphics}.rs`, MCP `coral_systems`,
`wall_mycelia`, `neuron_sparks`. Notes: `../rainworld-re/notes/superstructure-coral.md`.
Closes the missing `NeuronSpark` sound of `random.md`'s overseer item.

- **One pool for every strand (design).** The game keeps a `Mycelium` in its owner and the
  system's `mycelia` list of references; a strand pairs with another owner's by reading and
  writing it. The port keeps every strand of the world in one `MyceliumPool` (sim) and an owner
  holds `MyceliumId`s: the room's `WallMycelia`, and an `OverseerGraphics` module (the strands that
  were in `rw-graphics` for the overseers, moved: `rw_sim::Mycelium`, the same maths; the sprites stay
  in `rw-graphics`). Each system's registry is the pool's, by room (so a module's strands change
  registry in `UpdateNeuronSystemForMycelia` with the module living on in another room). Owners are
  compared by an `OwnerKey` (one per `WallMycelia`, one per module). Each system has its own
  `SystemId` (a re-created system is another object, as in the game, so a strand pointing at the old one is
  not "in" the new). Strands of a dropped module stay registered while their system's registry lives and
  can still be picked and paired, as in the game; they are freed when the registry goes with its room
  (`World::abstractize`) or at once when their system is none, so the pool stays bounded; a room's wall
  strands (and a room's neuron strands, D40) go with the room. Reachability is the collector's: a strand also lives while a living strand's
  `connection` names it (its partner keeps the link, drawing as the game does, until it breaks), and a
  dropped system's last `Frozen` is kept while a strand still points at it. The observer
  hooks `overseer_added/reset/emerge/updated` take a `MyceliaCx` (the pool and the room's reads) and a new
  `overseer_new_room` runs `UpdateCoralNeuronSystem`.
- **Timing.** `Room.Loaded` makes the system in the effects loop (`SSSwarmers`) or the placed-object loop
  (the first active coral or wall mycelia object), with its two wind draws, in settings order; the
  port's collect-then-add pass keeps that order. `CoralNeuronSystem.AIMapReady` runs in the loading
  step after `ReadyForAI` realized the room's creatures (so an overseer's strands are registered before
  the wall's); the session's first room waits for its `NowViewed` (the camera's first tick), as D37's
  `roomSpecks` do, and the wall mycelia take their place in the update list then. The wall mycelia are
  built from the `WallMycelia` objects the slugcat's `Filter`s leave active (26 of the 29 rooms for
  the Survivor and the Monk, all for the Hunter); an unreadable `ResizableObjectData` (none installed) is
  skipped with a warning where the game throws at load.
- **`camerasChangedTick`** is bumped on the room the camera shows at every camera move, as the game's
  `ApplyPositionChange` does: `apply_insect_view_change` (a room move, the first room too, also to the
  same room and position) and `camera_moved` (the app's `RoomCamera` cutting to another position,
  `Scene::update_room_camera`). A strand re-reads `ViewedByAnyCamera` only after a bump,
  so a room the camera left keeps its strands "viewed" until its system freezes (as the game).
- **The camera's room is `BeingViewed`.** The system's `notViewedCounter` and a strand's camera test read the
  port's `camera_room`, which `camera_moves_to` sets at once (see OPEN-ITEMS, "Camera room switches
  mid-tick").
- **Draws.** `CoralNeuronSystem`: 2 in the constructor, 2 a tick. `WallMycelia`: none in the constructor
  (seeded `(int)x + (int)y` of the first place, the stream restored; 2 for a strand count a free tile with a
  wall beside it, 2 for a strand's offset, 1 for its length when its ray hits), per tick 1 a strand plus its
  `Mycelium`'s: culled 0; live and unlinked with the registry non-empty and `rest < 1` 1
  (`Range(0, Count)`); linked 1 (`value < 0.005`) or 2 when the tips meet (3 with a spark, whose
  constructor draws once); a broken link `Range(20, 200)` 1 (after the chance draw 2); the tick a strand
  comes back into view `2 + 2 * points` more. `NeuronSpark`: 1 in the constructor, the
  `SS_Mycelia_Spark` sound's draws on its first update (the microphone's, `sound.rs`), and **5** a frame in
  `DrawSprites`, which the app makes on its cosmetic stream (D1). The survey's 8 was wrong.
- **`float2.normalized` (`ExtraExtentions`, read with the existing `ilspycmd`)** returns zero at or
  under length 1e-5, as the port's `Vec2::normalized`. **`LerpMap` with an exponent** uses `Mathf.Pow`
  (double); `FloatExt::lerp_map_pow` now does, no expected value in any crate's tests moved.
- **`Mycelium`'s Burst job is not a job.** `Phase1`/`Phase4` are plain loops in this build and the
  `JobHandle` is never scheduled (observed in `Mycelium.cs`); the port runs the same operations in the
  same order on `f32`.
- **Wall mycelia are drawn with the `Basic` shader** (`RippleBasicBothSides` is a plain vertex-colour times
  texture shader in the base game: observed in `StreamingAssets/shaders/RippleMaskedBasic.shader`
  (`tex2D * vertex colour`, `Blend SrcAlpha OneMinusSrcAlpha`, `Cull Off`, as `Basic.shader`) and
  `_RippleClip.cginc` (`ripple_both_sides` returns before any discard)), in `Items`. They are not
  drawn in the debug view or in an other-room MCP screenshot (as D36's objects; OPEN-ITEMS).
- **What the next steps add to `AIMapReady`** (not ported here): the `CoralCircuit` clusters (their
  `CircuitConnector` owners), then the `CoralStem`s with their `StemSegment`s (each taking `GetNewID`),
  then `PlaceSwarmers` (`SSOracleSwarmer`s, whose suckling strikes `NeuronSpark`s). The `CoralNeuron`s
  (made as the placed-object loop meets them, before the wall mycelia) are D40. Every wall strand can
  only pair with another owner's strands: in a room with wall mycelia alone only an overseer's lens can
  link to them; a room with neurons has theirs (the strands of a neuron and of the walls pair).
- **Omitted DLC branches** (listed, not dropped): More Slugcats' `Inspector` and `SingularityBomb` own
  strands and systems (`new CoralNeuronSystem` in their code); `ModManager.MSC`'s abstract-room add of
  the swarmers (10e); nothing in the ported classes reads a DLC flag.

## D40: Coral neurons and projected circles (`CoralNeuron`, `ProjectedCircle`; step 10d3a)

Ported in `crates/sim/src/{coral_neuron,projected_circle,game_coral,game_projections,
climbable_vines}.rs` (the neuron, the circle, their turns, the room's vine host), the sprites in
`crates/graphics/src/{coral_neuron_draw,projected_circle_draw}.rs`, the `CoralNeuron` shader in
`crates/render/shaders/sprite.wgsl`, MCP `coral_neurons`, `projected_circles`. Notes:
`../rainworld-re/notes/superstructure-coral.md`. 43 placed neurons in 15 `SS_*` rooms (the
Survivor and the Hunter all 43, the Monk 42), every one `ResizableObjectData`; the port builds
them all (`crates/sim/tests/installed_coral_neurons.rs`: 43 parsed, 0 rejected, the shortest
119 px, so the game's throw for a neuron under 40 px never happens).

- **Order.** `AIMapReady`'s placed loop makes each neuron as it meets it: the constructor
  (seeded by `(int)length`, the shared stream restored; its strands `Mycelium` constructors, bumps
  and the `ProjectedCircle`s it adds, in that order, each circle's constructor seeing the circles
  already in the update list) and then the caller's `AddObject`, so the update-list places grow
  circle 0, circle 1, neuron. The port makes the neurons after collecting the loop's other objects
  (nothing else in that loop draws or adds), then the `WallMycelia`, as the game's order is.
  A neuron's turn updates its strands one at a time with the chain's current geometry, as the
  game (`ConnectionPos` read live). The constructor's wall snap writes back into its parameters
  only: the fields `posA`/`posB` keep the placed positions (so `ConnectToWalls` holds a rooted end at
  `placed - rootDir * 10`), and only the initial line of segments uses the snapped ones. The first
  neuron in a room makes its `ClimbableVinesSystem`, which takes its update-list place just before
  the neuron's circles. A neuron shorter than 40 px (one segment) is skipped where the
  game throws (`new Mycelium[-1, 2]`); none exists installed.
- **Storage (design).** A neuron lives in its room's `CoralNeuronSystem.neurons` and nowhere
  else; the player's push, the hands' read, the neuron's own update and the draw all reach that one
  copy. `VineRef::CoralNeuron { room, order }` names it (its place in the update list); the
  room's host is `RoomVines` (`placed` vines and the system's neurons, split borrows of the room)
  built for each call of the player's update, `RoomVinesView` for the hands and MCP. A circle
  lives in the room's `projections.circles` with its owner as a `CircleOwner` (a neuron's
  place in the update list), and stays there after it is slated (other circles' connections read
  it); a slated circle leaves the update list and the draws.
- **`UpdateDepth` runs when the circle is drawn.** The game reads `rCam.DepthAtCoordinate` of the
  owner's centre in `DrawSprites`, once after each `Update` (`updateDepth`). The app calls
  `RainWorldGame::update_circle_depths` before it builds the projections' draw lists, headless
  runs without a screenshot keep the last drawn depth (0 at first); it is read only for drawing and
  makes no draws. With no camera picture the depth is sky (1), as `SSLightRod`'s.
- **Draws.** Per neuron constructor, from the seed: per segment 4 with two ends (the position's
  and the velocity's `RNV() * value`), 2 otherwise (the velocity's), per strand `1 + (2 + 2 * points)`, per bump 2, and per circle
  `1` for each circle already there (until five connect), 1 for the off-screen choice (then `Range(1,
  4)` and 3 a line), 2 for the turn. Per tick a neuron makes one draw per bump whose ping is 0 and its strands'
  (`Range(0, Count)` 1 each while unlinked and not resting; none while the system is frozen: the
  whole `Update` is skipped). A circle's `Update`: 1 for `getToRad` (+1), 1 for the blink (+1,
  `Range(7, 230)`), 1 for each connection not blinking (+1, `Range(10, 50)`). Draw time: 1 for a
  blinking connection's width (`value < 0.7`), on the app's cosmetic stream (D1).
- **Sprites.** Two meshes (`MakeLongMesh`, the first with the `MeshColor` ramp, the second with the
  `CoralNeuron` shader and `alpha = 1 / segments`), a mesh a strand, three `deerEyeB` discs a bump; all
  in `Items` except the ping dots with `WaterLights` (`Water`). `RippleBasicBothSides` is the `Basic`
  shader and `CoralNeuronBothSides` the `CoralNeuron` shader (the `ripple_both_sides` keyword only
  matters with Watcher's ripple, as D38). The shader's `sin` is the GPU's: at the large arguments of a
  neuron of many segments (`uv.y / alpha` reaches the segment count) this GPU's `sin` drifts from the
  CPU's, so the lines of a long neuron are not pixel-identical to the game's on another GPU; the GPU
  test compares the maths up to arguments around 20 rad.
- **MCP.** `coral_neurons` (room, id, length recovered from `conRad * segments / 1.5`, segments, ends,
  rooted flags, frozen, bumps, strand ids, `being_climbed`) and `projected_circles`; the neurons are
  also in `climbable_vines` with kind `CoralNeuron`.
- **Omitted branches (listed, not dropped):** `ProjectedCircle`'s `ModManager.MSC` code (a `baseRad`
  change moves `getToRad` to 0.8 or 1.2 of it, `lastBaseRad`; a circle whose owner left the room is
  slated, `HostingCircleFromRoom`) and `ModManager.MMF` code (connected circles that were slated are
  dropped from the list in `DrawSprites`; `Position` without a room reads the owner only); the
  owners `SSOracleSwarmer`, `CoralStem` (10d3b, 10e), `SpearMasterPearl` and `Inspector` (DLC)
  make circles with `SetBaseRad` and their own `HostingCircleFromRoom`. Nothing in `CoralNeuron.cs`
  reads a DLC flag.
