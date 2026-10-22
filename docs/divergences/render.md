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
  decode truncates, matching AssetRipper's export of every texture exactly). Only mip 0
  is kept (the noise shader always magnifies it; the atlases have one mip).
- **Room sprite shaders** (`FlatLight`, `OverseerZip`, `Hologram`). `_NoiseTex2`
  (256x256 DXT1, 9 mips, bilinear, repeat) is sampled at mip 0 only; in the room the
  game minifies it vertically (`tc.y * 8` over the 800 px level image, about 2.6 texels
  per pixel) and so reads a lower mip, giving coarser noise than the port. `Futile_White`
  (its own 16x16 white texture in the game) is drawn from the atlas's `RainWorld_White`
  element; a `Hologram` sprite of `Futile_White` would read its sideways-shifted alpha
  from the atlas around that element instead of a white texture (the overseer's
  hologram lines use `pixel`, which is in the atlas as in the game).
  `LightSource` (an overseer hologram's light and the guide light's `LightSource`; the
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
  themselves are not ported, so their sky shows only that colour, and `PinkSky`'s pink
  and the `DayNight` colour changes are not set (no base room uses either).
- **Full-screen effects** (`RoomCamera.ApplyPalette`'s one effect per room: `SkyBloom`,
  `SkyAndLightBloom`, `LightBurn`/`Lightning` (`LightBloom`), `Fog`, `Bloom`; the
  shaders in `full_screen.wgsl`, `rainworld-re/notes/room-rendering.md` section 12).
  Differences: the grab of the `Bloom` shaders is sampled bilinear with clamped edges
  (Unity's default, INFERRED as for the object shadows); `CreatureMask` tests only the
  pre-level grab's red channel (INFERRED from HLSL's `bool4` to `bool` conversion);
  `Fog` reads the pre-level grab as its `_GrabTexture` (it has no `GrabPass` of its own;
  INFERRED that the last named grab is that one); holograms in `Bloom` are always drawn
  after the effect, where the game puts the effect back on top of its container after
  every `ApplyPalette`; `num4` is `ShaderLight` without lightning (`Lightning`'s flashes
  and palette are not ported, so a `Lightning` room's alpha is its amount times
  `InverseLerp(0, 0.2, amount)` with `num4` from the rain only). Not selected: the echo
  branch (ghost mode) and `VoidMelt` (`LevelMelt2`), which leave the room with no
  effect. `_waterLevel` uses the `DeathFallGraphic`'s height (focal points included),
  but the death fall's own sprite is not drawn; another room's MCP screenshot draws no
  full-screen effect and takes `_waterLevel` from the water alone.
- **Not drawn:** the rain's `ShaderLight` after the rain hits (`GlobalRain` is not ported: it stays
  -1), `BrokenZeroG`'s dark palette (zero gravity is not ported), lightning, room
  effects that grade colours (Darkness, Brightness, Contrast, Desaturation, Hue, DayNight:
  their uniforms keep the game's defaults; no base-game room uses them, they come with
  the DLCs)
  or modify effect colours (`ModifyEffectColorA/B`, unused in base-game rooms), ghost /
  rot / mushroom palettes, the rain's screen shake (and the sandstorm's, blizzard's,
  flood rooms' `waterFlux.roomShake`, lightning's and `AntiGravity`'s), `_bkg` backgrounds, the heat / melt / ripple
  shader variants, swarm rooms, and the properties-file line preprocessing (no base-game
  `Room Setting Templates` line uses it).

**Why.** The port draws the camera's room with the player; these depend on systems
not ported yet (the rain itself and the global rain, lightning, zero gravity, echoes,
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

