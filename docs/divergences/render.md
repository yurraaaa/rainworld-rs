# Divergences: rw-render

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D7: Room graphics (level image and palette)

The level graphic follows `rainworld-re/notes/room-rendering.md`: settings with
template inheritance (`rw_assets::RoomSettings`), palette with effect colours and the
per-camera fade (`PaletteTexture`), the camera image, `level.wgsl` (the basic path of
the game's level shader, checked against a CPU reference), sprites behind the level
showing only through sky and depth >= 5, and the room camera's zone switching and lean
(`rw_render::RoomCamera`).

**What differs:**
- **DXT texture decode.** `_NoiseTex` (a 64x64 DXT1 texture) and the sprite atlases
  (DXT5) are decoded on the CPU from `resources.assets` by `rw_assets::unity` and
  uploaded as RGBA8; the game's GPU samples the compressed blocks and interpolates with
  its own rounding, so texels may differ by 1/255 from what the game shows (the CPU
  decode truncates, matching AssetRipper's export of every texture exactly). Every texture from `resources.assets` is uploaded with its stored mip chain (`shelterGate` 9,
  `Rainbow` 7, `LightMask0` 8, `maze` 8 (D42), `textGradient` 8, `_NoiseTex` 7, `_NoiseTex2` 9,
  `cloudsTexture` 10; `rainWorld`, `uiSprites`, `glyphs`, `fontAtlas`, `cityPalette` and
  `apartments` store one), and each shader that reads a mipped one picks the level by
  derivatives, taps before any early return (D37). Two import settings are not honoured, as no
  texture uses them: a non-zero `m_MipBias` (wgpu samplers have none) and trilinear filtering
  (filter 2).
- **Room sprite shaders** (`Hologram`). `_NoiseTex2` is read at the derivative-picked mip
  level, taps before any early return, as in the game (not a divergence). `Futile_White`
  (its own 16x16 white texture in the game) is drawn from the atlas's `RainWorld_White`
  element; a `Hologram` sprite of `Futile_White` would read its sideways-shifted alpha
  from the atlas around that element instead of a white texture (the overseer's
  hologram lines use `pixel`, which is in the atlas as in the game).
  `LightSource` (placed lights, the player's glow, an overseer hologram's light and the guide light's `LightSource`; the
  game draws the latter as a 64-segment `CircularSprite`, the port as a quad, the same
  pixels as the shader is 0 outside the inscribed circle) runs the base-game path only (sloped
  terrain masked black, creatures masked with a grab of the frame taken before the level
  image is drawn; the game reads the last unnamed GrabPass, which differs only if a
  projected image was drawn earlier in the Bloom container); the grab is sampled
  nearest. `HologramImage` reads its projection cell from the sprite colour as the game
  does; that the grab's y orientation and WGSL's rounding of ties match the game's HLSL
  is inferred, not checked. The projection images (`Projections/RND_PROJ`, `STR_PROJ`)
  load at startup, where the game loads them on first use.
  The Watcher ripple clip (`rippleClip`, `smoothRippleClip`, off without the `RIPPLE`
  keyword) is not ported.
- **Window and scaling.** As in the game, the whole frame (level, sprites, debug
  overlays) is drawn without antialiasing into an offscreen frame of the `ScreenSize`
  option (`--resolution`, the game's five choices, 1366x768 by default), shown full
  screen by default, scaled to the largest fitting rectangle of its aspect (fractional
  scale) with black bars, or with `--windowed` 1:1 in a non-resizable window of that
  size (`rainworld-re/notes/room-rendering.md` section 10). Differences: the filter of
  the scaled case is Unity-internal and not in the game's code; the port assumes
  bilinear. `--screenshot` runs and the MCP mirror window are always windowed (MCP
  screenshots are headless and return the frame itself, at a size the caller can
  change), and a window the window manager sizes differently from the resolution is
  letterboxed the same way. The Steam Deck case (1229x768 full screen as a 1280x800
  window) is not ported.
- **Half-pixel camera offset.** The port always draws as the game's D3D build at
  render scale 1 (Windows, also under Proton): Futile's `screenPixelOffset`, screen =
  stage + (-0.5, +0.5) (`Camera::SCREEN_PIXEL_OFFSET`). The game's OpenGL path (no
  offset) and its render-scale > 1 offset (0, 1) are not modelled
  (rainworld-re/notes/sprite-atlases.md section 3).
- **Camera switch timing.** The game loads the next camera's image asynchronously and
  cuts when it has loaded (usually the next frame); the port decodes every camera's
  image when the camera enters the room (`MoveCamera`, synchronous: D14) and cuts in
  the same tick.
- **Shortcut graphics** (`rw_graphics::ShortcutGraphics`, `rainworld-re/notes/shortcuts.md`
  section 10). The level view draws the game's pipe pixels (`ShortCut0`, masked to the
  level image's shortcut texels), entrance symbols and glows, the blink pulses and the
  travelling vessels in their creatures' colours; the debug view keeps the player's
  vessel marker.
  Differences: the entrance pixels' `RippleBasicClipDistortion` is drawn as `Basic`
  (its Watcher ripple clip is off in the base game); the glow's `Futile_White` is the
  atlas's `RainWorld_White` (as for the room sprite shaders above); within the `Items`
  container the shortcut sprites are drawn after the overseers' (the game's order there
  follows when each was added, INFERRED irrelevant since `ShortCut0` only shows on pipe
  texels). `ShortCutColor` (vessel pixels and entrance flashes) is the template's
  `shortcutColor`, the lizard's effect colour, the slugcat's colour and the constant
  overrides (big eel, deer, jet fish, scavenger); the overrides that read body state
  (centipedes, daddy long legs, big spiders, cicadas, egg bugs, vultures, snails,
  lantern mice) wait for those bodies and fall back to the template meanwhile. Not
  ported: the arena's `DarkenExitSymbol` and `ChangeAllExitsToSheltersOrDots`, the More
  Slugcats symbols (dens, NPC and region transports, and showing them while
  controlling a creature) and the Remix gate symbol, underwater shortcuts and grayscale.
- **Camera in shortcuts.** While the player is in a shortcut the camera follows the
  centre of the vessel's tile; the game uses the vessel's interpolated on-screen position.
- **Fade quantisation.** The blended palette is stored as 8-bit texels like the game's
  ARGB32 palette texture; the rounding mode of Unity's `SetPixel` is assumed to be
  round-to-nearest.
- **Object shadows and `SunBlock`.** The object-shadow lookup reads the "behind" target
  (the frame before the level, the game's `_GrabTexture`) with bilinear filtering and
  clamped edges; Unity's grab texture filter and wrap are not in the game's code
  (INFERRED: render textures default to bilinear). The lookup follows the D3D build's
  coordinates (`UNITY_UV_STARTS_AT_TOP`, as the half-pixel offset above). The
  `SunBlocker` is drawn (its sprite in `Shadows`); its `UpdatableAndDeletable` is not
  added to the room's update list (its `Update` does nothing).
- **`_rimFix`** is set from the room's effects (`AboveCloudsView`, `RoofTopView`,
  `PinkSky`; 11 base rooms) with the sky scenes' day `atmosphereColor` (41, 59, 81); the scenes
  themselves are ported (D31, D32), and `PinkSky`'s pink and the `DayNight` colour
  changes are not set (no base room uses either).
- **Full-screen effects** (`RoomCamera.ApplyPalette`'s one effect per room: `SkyBloom`,
  `SkyAndLightBloom`, `LightBurn`/`Lightning` (`LightBloom`), `Fog`, `Bloom`; the
  shaders in `full_screen.wgsl`, `rainworld-re/notes/room-rendering.md` section 12).
  Differences: the grab of the `Bloom` shaders is sampled bilinear with clamped edges
  (Unity's default, INFERRED as for the object shadows); `CreatureMask` tests only the
  pre-level grab's red channel (INFERRED from HLSL's `bool4` to `bool` conversion);
  `Fog` and `SpecificDepth` read `LevelColor`'s unnamed pre-level `GrabPass { }` as
  their `_GrabTexture` (neither has a `GrabPass` of its own; INFERRED that no other
  unnamed-grab sprite draws in `Foreground` between the level and them); holograms in `Bloom` are always drawn
  after the effect, where the game puts the effect back on top of its container after
  every `ApplyPalette`; `num4` is `ShaderLight`, or the room's lightning level (D25). Not selected: the echo
  branch (ghost mode) and `VoidMelt` (`LevelMelt2`), which leave the room with no
  effect. `_waterLevel` uses the `DeathFallGraphic`'s height (focal points included).
  The death fall's sprite (`death_fall.wgsl`) is drawn as the game's; its alpha pulse's
  `counter` starts when the camera enters the room, where the game's counts from
  `Room.Loaded` while the room stays realized (the port keeps a death fall for the
  camera's room only), so the pulse's phase differs on re-entering a still-realized
  room. Its element is `Futile_White`, where the game takes the room's `RainMask_` once
  the rain or the `WaterLight` has made it (the shader reads only the quad's uv, the
  same either way).
  The room camera's `WaterLight` (`app/src/water_light.rs`, `fs_water_light`) is drawn
  as the game's, first in `Foreground` after the level. Its `RainMask_<room>` texture
  (`rw_assets::RainMask`, `GenerateShelterTex` without the rain's outputs) is made from
  the realized room's tiles on each camera entry into a room with water, where the game
  makes it once per room name per process (the first `RoomRain` or `WaterLight` to ask)
  and keeps it, so a room whose tiles changed since (a shelter door) could differ;
  rooms without water get none (only the rain would make theirs). `SetPixel` past the
  top row wraps to row 0 (INFERRED: Unity's default `Repeat` wrap, set to `Clamp` only
  after the fill). The mask is sampled bilinear at mip 0 (the game's has mips, but
  WaterLight magnifies it); `half` maths runs in f32. The generator's `HasAnySolid`
  reads `Tile.Solid` only: `Room.terrain.ObstructsTile` (the curve of a base-game
  `TerrainHandle` placed object, no `ModManager` gate) is not ported, as no installed
  base room or level places a `TerrainHandle`. Watcher's `rippleFade` branch of
  `PaletteDarkness` is omitted.
  Another room's MCP screenshot draws no full-screen effect, death fall or water light
  and takes `_waterLevel` from the water alone.
- **Not drawn:** the rain's `ShaderLight` after the rain hits (`GlobalRain` is not ported: it stays
  -1), room
  effects that grade colours (Darkness, Brightness, Contrast, Desaturation, Hue, DayNight:
  their uniforms keep the game's defaults; no base-game room uses them, they come with
  the DLCs)
  or modify effect colours (`ModifyEffectColorA/B`, unused in base-game rooms), ghost /
  rot / mushroom palettes, the rain's screen shake (and the sandstorm's, blizzard's,
  flood rooms' `waterFlux.roomShake`), `_bkg` backgrounds, the heat / melt / ripple
  shader variants, swarm rooms, and the properties-file line preprocessing (no base-game
  `Room Setting Templates` line uses it).

**Why.** The port draws the camera's room with the player; these depend on systems
not ported yet (the rain itself and the global rain, echoes,
the room effects' own objects and scenes) or are invisible at this stage. The rain
cycle's light and palette darkening before the rain are ported (D22).

**Affects.** Exact pixel colours of cloud shadows (noise rounding) and rooms with the
listed effects; the softness of the frame when scaled full screen.

## D22: Room light and palette darkening without a rain cycle

`_light` and `DarkPalette` follow the game (`RoomCamera.DrawUpdate`, `DarkPalette`,
`ApplyFade`; `RainCycle.ShaderLight`, `RainDarkPalette`, `CycleStartUp` with its
2400-tick start-up after a story's first cycle): in a room whose `DangerType` is not
`None` the light is the rain cycle's, before `Clouds`, and the palette turns toward its
rain-dark half by `RainDarkPalette * RainIntensity` in the last 60 seconds before the
rain (story).

**What differs:** a world without a rain cycle (`cycle_length` 0, the default the
port's `World` starts with; every MCP start, dev starts and arena levels included, gets
the story cycle of `RainCycle::new_story`, so this is only a guard) would read to the
game's formulas as a cycle already over (full darkness); there the port keeps the light
at 1 and the palette undarkened: a port-only guard that no real start reaches. Dev
starts and arena levels use the story formulas because the port gives them a story
cycle (D4, D16); in the game an arena session uses the non-story ones
(`LerpMap(TimeUntilRain, 200, 880, -1, 1)`, `RainDarkPalette` `InverseLerp(1000, 400,
TimeUntilRain)`) on its own cycle, with `startUpTicks` 600 outside sandbox (the port
gives arena levels none). The palette is rebuilt at the start of the tick when
`DarkPalette` changed, as the game's camera `Update` does before the rain cycle's.

**Why.** Starts without a cycle are not a game situation.

**Affects.** Lighting of cloudy rooms and the palette in dev starts and arena levels.


## D26: Custom decals and rainbows (`Decal`, `Rainbow`)

`CustomDecal` (`Decal` shader, `Foreground`) and `Rainbow` (`Rainbow` shader,
`GrabShaders`, a `GrabPass`) follow the game's code and shader sources. What differs:

- The decal's object-shadow lookup (a shadowed pixel from layer 5 on is lit again where
  something was drawn behind the level toward the light) reads that grab at the nearest
  pixel; the game's `tex2D(_GrabTexture, ...)` is bilinear. The level shader's own lookup
  is bilinear (D7); only creature edges can differ, by one pixel.
- Decal images are uploaded as atlas pages when a room with the decal is first shown,
  and stay uploaded; the game unloads an image when its last room unloads
  (`usersPerImage`, `RoomUnloaded`).
- The grid mesh is built when the decal is made, not at the first `DrawSprites`; its
  draws are on a stream of their own (`Random.InitState` with the state restored in the
  game), so nothing observable differs.
- Omitted branches (DLC, not installed): the shaders' `RIPPLE` (`smoothRippleClip`),
  `COMBINEDLEVEL` (`dynamicLevelClip`) and `URBANLIFE` keywords; `Rainbow`'s
  `PrecycleModule` spawn; `AetherRainbow` (Watcher). The camera's `ghostMode` (echoes)
  is 0, so `Rainbow`'s `InverseLerp(0.2, 0, ghostMode)` is 1 and the spawn test needs no
  ghost check, until echoes are ported.
- Arena sessions: every port `World` is a story one (`story = true`, arena levels loaded
  with `load_room` included; D4, D16, D22), so `Rainbow`'s `IsArenaSession` behaviour
  (always made, fade 1, never destroyed) never fires and the arena cycle's 600-tick
  start-up does not exist. The code tests `!world.story` and takes effect once arena
  sessions are ported.
- Decal image names were checked against the `rainWorld`, `uiSprites` and `shelterGate`
  atlases and the projections: no clash (265 names), so the shared element lookup is safe.
- `RainbowNoFade` is ported (same data, always shown) but no base room places one.

**Why.** Unreachable in the base game, or the port's grab textures (D7).

**Affects.** Pixels along creature edges under decals; memory after a long session.


## D31: Background scenes (`AboveCloudsView`, `RoofTopView`): the shaders and their inputs

The scenes' elements draw in the `Water` container after the level image, each with the
game's shader (`Background`, `Cloud`, `CloudDistant`, `DistantBkgObject`,
`DistantBkgObjectRepeatHorizontal`, `BkgFloor`, `House`, `Dust`, `Spores`;
`background_scene.wgsl`), clipped by the level image and by what was drawn behind the
level as `BackgroundClipVanilla` does. What differs:

- **Image sampling is partly in the shader.** Each scene image is an atlas page of its
  own (`LoadGraphic` makes an atlas of one element) and gets its sampler per page from
  its `LoadGraphic` call (`crispPixels`: point, else bilinear; `clampWrapMode`: clamp,
  else repeat): sky point/clamp; clouds, flying clouds and smoke bilinear/repeat; city
  buildings bilinear/clamp; the other distant buildings, lightning, floor and rubble
  point/repeat. `Background` and `DistantBkgObject` read the page through that sampler
  (`textureSample`), so they filter once, as the game does. The shaders that need
  bilinear or repeating reads of their own (`Cloud`, `CloudDistant`, `Dust`, `House`,
  `BkgFloor`, `DistantBkgObjectRepeatHorizontal`) read the page's texels with
  `textureLoad` and apply the image's filter and wrap themselves; the sampler is not
  used there. The result is Unity's bilinear filter; sampling at a texel's centre is
  exact. `_ApartmentsTex` and `_CityPalette` are read with a point, clamped sampler.
- **`_CloudsTex`** is `resources.assets`' `Illustrations/cloudsTexture` (DXT5, 10 mips,
  bilinear, repeat) read with every mip and sampled with the nearest mip by the screen
  derivatives, as Unity's bilinear filter does. Its import settings are read
  (`m_Aniso` 1, `m_MipBias` 0); the game's quality level ("Sweet", `globalgamemanagers`)
  has `anisotropicTextures` 0 (Disable) and no code changes it, so the sampler uses no
  anisotropy. wgpu has no sampler mip bias; the texture's is 0.
- **`_GrabTexture`** is the frame drawn before the level (D7), as for the other
  shaders that read it.
- **Vertex colours are 8 bits a channel** (Unity stores a mesh's `Color[]` so), which
  the clouds' `randomOffset` and every haze value pass through.
- **`2/3` in `Cloud.shader`** (`dpOffst = half2(0.5, 2/3) - scrPos`) is HLSL integer
  division, 0; ported as 0 (observed: the compiled `Futile/Cloud` programs in
  `resources.assets` have no 0.6666667 constant, only one 0.33333334, the same as
  `CloudDistant`'s `/3.0`).
- **Globals.** `_windDir` is 1 and `_MultiplyColor` white, as the base game leaves them
  (`DayNight` changes the colour); `_AboveCloudsAtmosphereColor` and `_WorldCamPos` are
  set for the frame from the camera room's scene (the game keeps the last constructed
  scene's value after the room is left).
- **Images load when a room with a scene is first shown** and stay uploaded, as the
  decals' (D26).
- Omitted branches: `RIPPLE` (`smoothRippleClip`, `rippleClip`) and the sloped-terrain
  mask (black in the base game, so its tests never cut), Watcher's.

**Why.** The port's atlas pages and per-page samplers (D7); the rest unreachable in the base game.

**Affects.** One-pixel rounding where the
game's own sampling differs from the shader's emulation.
