# Divergences: rw-menu

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D13: Front end (process manager, menus, menu input)

**What.**
- **Start process.** `Initialization` is reduced to one tick: fonts, strings and
  settings are loaded before the manager starts, and it requests the start process at
  once. As in the game's distribution build (`setup.startScreen`) it starts `IntroRoll`,
  which leads to the main menu. `--spawn`, `--script` and `--screenshot` start `Game`
  directly, and `--process` picks (`intro-roll`, `main-menu`, `slugcat-select`, `game`,
  `font-test`). The boot fade, the switches and the fades follow the game. In MCP's
  game mode (`new_game`, `load_room`) there is no process manager: the session counts
  `InitialBlackSeconds` down per tick so room scripts read `FadeDelayInProgress` as in
  the game (3 s for a new game, the Hunter 5.5 s, none for `load_room`), but no fade is
  drawn.
- **Intro roll.** Ported in full (`rw_menu::IntroRoll`, `RainEffect`). The game's
  waits on the sound loader, platform, options and progression are always satisfied
  (all loaded before the manager starts), so `delayedTime` waits only for the music
  bundles; `requestIntroMusic` (options not ready) never arises. `ProcessManager.
  initializationAndIntroRollStarted` and `activityID` (console activities) are not
  ported. `RainEffect`'s light source uses `FlatLightNoisy`
  with `_spriteRect` unset (0, 0, 0, 0), as in the menus at boot: the noise lookup
  divides by zero in both, and what the GPU then samples is whatever the driver does
  with non-finite coordinates (the effect on the light's falloff is small).
- **Port-only process.** `ProcessId::FontTest` (labels in both fonts over the flat
  main-menu image) is not a game screen; it exists to compare drawing by eye. Its
  labels sit on whole pixels, where `font`'s glyphs lose a pixel row to point sampling;
  the game's menus place labels a third of a pixel off (the page sits at (1/3, 1/3)),
  and the port's menus draw them whole.
- **Unported screens.** Every process but the game, the main menu, the options menu,
  slugcat select and the intro slideshows (the main menu's Regions, Arena and Remix,
  the options menu's Input Settings, Credits and Backups) is a black screen naming the
  process; the pause button (Escape) returns to the main menu. The dream screen is a
  black stand-in (D17).
- **Sleep, death and starve screens.** `SleepAndDeathScreen` on `KarmaLadderScreen`
  (`rw_hud::SleepAndDeathScreen`, `rainworld-re/notes/menu.md` "Sleep and death
  screens") for the `SleepScreen`, `DeathScreen` and `StarveScreen` processes: the depth
  scene, the full `KarmaLadder` (its phases, symbols, fade circles, reinforcement, sounds
  and every random draw; the selection glow's `LocalBloom` GrabPass blur reads the
  port's grab texture), the screen HUD's food meter and region map (D15), CONTINUE
  (`StoryGameInitCondition.Load`; after a death screen the cycle starts 340 ticks in)
  and EXIT (main menu), the info texts, the starving warning, the watched-screen
  counters and their forced watch, the map toggle and the illustration's fade. The
  end-game meters (`EndgameMeter`: float and notch meters for every base tracker kind,
  the power-on flicker, the fill animation, the fulfilment, their sounds and draws), the
  tokens (`EndgameTokens`), the `endGameMetersEverShown` save (`METERSSHOWN`) and the
  watch a passage fulfilled on the sleep screen forces are ported.
  - **PASSAGE is disabled (user decision).** The button appears where and when the game
    adds it (a token of a passage fulfilled before the screen, or once a passage
    fulfilled on it shows), fading in from black the same way, but stays greyed out:
    `CustomEndGameScreen`, `FastTravelScreen` and `ApplyCustomEndGame` are not ported,
    so `EndgameTokens.Passage` (the token's activation and the black fade) never runs
    and passages are never consumed. Passages are still achieved: the trackers progress
    and fulfil wherever their triggers are ported ("Not ported yet").
  - `CueAchievement` on a fulfilment is not made (the port has no achievements).
  - The tokens' glow size draws `Random.value` once per token and rendered frame; the
    port takes it from the screen's own cosmetic stream (D1).
  - The kills display (`SleepScreenKills`, `KillTrophy`, `CreatureSymbol`) is ported
    for the session's kills (a quantified bat fly earns no trophy; overseers do).
    Not ported: the statistics screen's `ScoreFlash`,
    the Hunter's `GoToRedsGameOver` (D17), and the Expedition, Watcher and More
    Slugcats branches (Remix's Traveller region colours and list-change animation, the
    collectibles tracker).
  - The game's `PostSwitchMainProcess` runs `progression.Revert()` before every process
    but these screens, dreams and the ghost and karma-to-max screens; the port does the
    same (`PlayerProgression::revert`), so CONTINUE loads the stored state (D17).
- **Options menu.** `OptionsMenu` with Remix off and no console (`rw_menu::OptionsMenu`,
  `rainworld-re/notes/menu.md` "OptionsMenu"), with its `SelectOneButton`s,
  `HorizontalSlider`s, `CustomMessageButton`, `ControlsButton`, check boxes, the reset
  `HoldButton` and the `Landscape_SU` depth scene. What it does in the port:
  - The volumes drive the audio (sound effects for game and menu sounds, music for the
    music player), the screen mode and resolution the window (borderless full screen or
    a window of the size; `Screen.SetResolution` and the ten-tick mode fix-up are
    followed, `Cursor.visible` with them), the save slot loads that slot of the port's
    saves, and RESET PROGRESS is `WipeAll` on the port's slot (D16).
  - Stored only: the language (the port has the English strings and fonts only, so a
    change re-opens the menu in English), vibration (no rumble), speedrun validation
    (no loading screens) and the arena music volume (no arena).
  - Progression loads synchronously, so `ProgressionBusy` is never true (nothing greys
    out) and `HandleSaveSlotChangeFailed`'s dialog never shows: a slot that cannot be
    read is kept in memory and logged.
  - `ControlSetup` is not ported: the `InputSetup` entries are kept as read; a new
    file gets what `new ControlSetup(i, i == 0)` writes (inferred: no mouse mappings).
    An unregistered language value reads as English.
  - Turning the music to 0 and leaving drops the music player (`StopSideProcess`)
    without its songs' shut-down.
  - The dialogs (`DialogConfirm`, `DialogNotify`) are measured with the font's line
    height for `LabelTest.LineHeight` (inferred equal).
- **The save importer (port only).** The game has no such option. The options menu has
  an IMPORT SAVE FROM RAIN WORLD button (a simple button, 220 x 30, centred between BACK
  and CREDITS at y 50; BACK's right and CREDITS' left now lead to it). It asks with a
  `DialogConfirm`, then copies the game's save of the selected slot over the port's
  (`rw_save::import_slot`, the port's file moved to `backup/` first) and reports the
  result in a `DialogNotify`. Its texts are English, untranslated.
- **Pause menu.** `PauseMenu` of a story game is ported (`rw_hud::PauseMenu`,
  `rainworld-re/notes/menu.md` "PauseMenu"): CONTINUE, EXIT and the confirmation, the
  keyboard control map with the pick-up instructions, the dimming, its sounds through
  the game's microphone (volume group 1) and the microphone's dimming and slowing, the
  HUD's pause text, and `ExitToMenu`. While paused the room's frame repeats the last
  unpaused one (the game does not redraw it): the port redraws it at that frame's time
  stacker with the cosmetic stream restored. Not ported: the arena and sandbox buttons
  (RESTART, EDIT, PLAY), the gamepad presets' maps, controller rumble, Expedition's
  unlocks and karma warning, and Platform `systemMenuShowingExplicit`. `RedIsOutOfCycles`
  is the sim's (`RedsCycles` 19, 24 with the save's `RedExtraCycles`). `ExitGame` saves
  as D16 describes (`AppContext::exit_game`); `warnedAboutKarmaLossOnExit` is the
  progression's and saved with it. The port's R respawn (dev starts only, D4) still
  works while paused. During game over the pause menu does not open, as in the game.
- **Depth and flat scenes.** As the game decides, scenes are in depth unless
  `StreamingAssets/flatmode.txt` exists (`rainWorld.flatIllustrations`; the installed
  game has none). Ported in depth (`rw_menu::menu::DepthScene`, `MenuShader`,
  `rainworld-re/notes/menu.md` "Depth scenes"): `MenuDepthIllustration` (the
  double-height layer, parallax `pos - CamPos * 80 / depth`, `DepthAtPosition`,
  `positions.txt`), `InteractiveMenuScene`'s focus (the idle pick from the idle depths
  or layers: `Range`, then `value`; the depth under a still mouse), the globals
  `_BlurDepth`, `_BlurRange`, `_MenuCamPos` and the shaders `SceneBlur`,
  `SceneBlurLightEdges`, `SceneLighten`, `SceneOverlay`, `SceneSoftLight` in WGSL. In
  depth: `MainMenu`, `Landscape_SU` (the options menu), the slugcat select
  pages (`Slugcat_White`, `_Yellow`, `_Red` with the locked Hunter's dark layer,
  `Slugcat_Dead_Red`, the three ghosts; `SlugcatPage.GrafUpdate` places each layer at
  `pos - CamPos * 80 / depth + sceneOffset` slid by the scroll), the sleep, starve and
  new-death scenes of the base game's slugcats (their idle depths; the death scene's
  three flowers, each a layer and a `Lighten` layer, shown one after the other over
  the scene timer's 160 ticks; the screens' map dimming of the back layers by
  `fadeOutIllustration`) and the intro slideshows' scenes (below). The More Slugcats and
  Watcher branches of these (Spearmaster's random sleep layer, Inv's images, the
  Watcher's sleep scene and ripple ladder) are not ported. With `flatmode.txt` the
  flat branches follow the game: the idle camera draws every tick
  (its idle focus pick draws nothing more without layers); the sleep scene of the
  slugcat being played (`PlayingAsSlugcat`: built after the manager switches, so never
  the story character); the death scene's `Flower` flat faded in over the scene
  timer's first 160 ticks, which only the screen starts. Layer images load
  synchronously when the scene is built, as in the game, and stay decoded on the CPU
  (for `DepthAtPosition`) and on the GPU: about 80 MiB each for the main menu's 15
  layers, 36 MiB for `Landscape_SU`'s 3. `SceneRain` and `SceneMultiply` (no base-game
  scene uses them) are not ported; such a layer would draw with `Basic`. The scene
  globals are kept per menu (the last scene `GrafUpdate` of that menu), where Unity's
  are process-wide; only scene layers read them, and a scene sets them before its
  layers draw. As in the game (`Shader.SetGlobal*` in each shown scene's `GrafUpdate`,
  every draw after all of them), the slugcat select menu's three scenes share the last
  page's values: the Hunter's scene's focus and camera blur every page. The grab is sampled at the pixel's own screen position, the net of the
  shaders' `UNITY_UV_STARTS_AT_TOP` flip and `1 - scrPos.y` read on Direct3D (the
  Windows build); on OpenGL the game's would read the grab mirrored.
- **Intro slideshows.** `SlideShow` (`WhiteIntro`, `YellowIntro`; `rw_menu::SlideShow`,
  `rainworld-re/notes/slideshow.md`) is ported: the playlists, the wait for
  `RW_Intro_Theme`, the manager fade and the skip (the pause button, level-triggered
  because `lastPauseButton` is never written). Its `SlideShowMenuScene`s
  (`menu::slide_show_scene`) are in depth as the game shows them: the 15 intro scene
  tables (the slideshow's depths where Intro 1, 3, 4 and 5 differ from an interactive
  scene's; only Remix's background options and Expedition's jukebox build those
  interactively, so those arrays are not ported), the camera path along `camera.txt`
  (the Bezier through the points, `/ 300`, keyed to the scene's time; the focus from
  the third column, -1 interpolated), the constructor's first camera position left
  undivided as the game leaves it (the first update divides it), and the depth and
  flat rules of `ApplySceneSpecificAlphas`. The scenes draw no random numbers. The intro
  playlists add no crossfades. The clock follows the song's first source as the game
  follows `AudioSource.time`: kira's position of the streamed sound in the window, the
  ticks played on `TimedSources` headless (deterministic). The outros, crossfades, the
  More Slugcats and Watcher slideshows and the dev move editor are not ported.
- **Saves, options and progression.** The progression is the port's own slot of
  `options.saveSlot` (`sav`, `sav2`, `sav3` in its save directory, D16): the selected slugcat, the Hunter unlock
  (`unlockred.txt` or the main menu's unlock code, saved with the next progression
  save) and the discovered shelters (Regions) come from it. The options are the port's
  own (`options` and `localoptions.txt` in its save directory, in the game's format,
  unknown entries kept as the game keeps them), saved where the options menu saves
  them; the game's are never read or written. The Downpour/Remix first-launch notices
  (`dlcTutorialShown`, `remixTutorialShown`) count as already shown, so the main menu
  opens without its popup.
- **DLC detection.** Until DLC support exists the port treats every DLC as not
  installed (user decision): `dlcsInstalled` is empty, so the main menu shows the plain
  `Main Menu` scene and the intro roll the plain `Intro_Roll_C` (no random slugcat
  draw). The game finds `dlcsInstalled` from files (`DLCOwnershipCheckSwitchPC`); with
  this install's Downpour files it would show `Main Menu - Downpour` and a random
  `Intro_Roll_C_<slugcat>` (one `Random.Range(0, 5)` after the rain effect's draws).
  The check is kept as `rw_menu::Dlcs::check` and the Downpour branches are ported, for
  DLC support to turn back on.
- **Starting a game.** The slugcat select menu's held START starts a new game
  (`StoryGameInitCondition.New`) at the character's start: its region's world, the
  start den (`GetStoryDenPosition`: `SU_C04`, the Hunter `LF_H01`) and
  `TryGetPlayerStartPos` (D14). The Survivor and the Monk see their intro slideshow
  first and the Hunter none (`StartGame`); S, or the map button (action 11), held as
  START completes skips it. The map button is read as Space, Rewired's default gameplay
  keyboard binding; that the gameplay map is active in menus is inferred, not checked.
  Space also submits in menus, so filling START with Space skips the intro. The game
  then waits `InitialBlackSeconds` in black (3 s, the Hunter 5.5 s) and fades in over
  `FadeInTime` (2 s). On a page with a save, START continues it
  (`StoryGameInitCondition.Load`: 0.75 s black, then the save state's `SlowFadeIn`) and
  the restart check box rises; checked, START (held 120 ticks, "DELETE SAVE") wipes the
  save (`WipeSaveState`) and starts a new game, which is the game's only confirmation.
  A page with a save is `SlugcatPageContinue` (`rw_hud::SlugcatSelectMenu`,
  `rainworld-re/notes/menu.md` "Continue pages"): its scene (the ghost after
  ascending, the dead Hunter's once its cycles ran out), the region and cycle label (the
  Hunter's cycles left) and its HUD's karma meter and food meter, placed and faded as
  the game does; with a single save its page opens first. `MineForSaveData` reads the
  current save state, else mines the stored one (`SaveStateMiner`); a room exists when
  its file does (the game's `roomNameToIndex`, compared case-insensitively). In depth
  mode the page has the mark of communication and the glow its save has (`HasMark`,
  `HasGlow`: not after ascending or for the dead Hunter), the glow among the scene's
  layers where `BuildScene` calls `AddGlow`, and the menu has its `RainEffect` behind
  the pages (fading in to 0.3, no lightning). The mark and glow are drawn after the
  menu's other objects, as the HUDs are. Not ported: the Remix speedrun time in the
  label and
  the campaign time tracker's `LoadOldTimings`, and the More Slugcats, Watcher and Jolly
  branches (alternate endings, `rippleLevel` symbols, custom colours). A shelter name
  without `_` makes the game throw; the port takes the whole name as the region. The
  continue pages' HUDs are drawn after the menu's other objects (the game draws each
  after its page's sub-objects); nothing else draws to their sprites, so the frame is
  the same. The Hunter's ended save goes to the unported Statistics screen. The chosen slugcat colours the body's graphics; its body moves as
  the Survivor's (the Monk's and the Hunter's `SlugcatStats` are in
  `rw_sim::campaign::SlugcatCampaign` but the player does not read them yet). Started
  directly
  (`--process game`, `--spawn`, MCP), the game is the Survivor in the given room's
  world.
- **Side processes.** The music player (made in the first `ProcessManager.Update`,
  ticking on its own time stacker after the main process, `UpdateMusicContext` after
  every switch) and the menu microphone run, in the game's order; no dialogs or Steam
  manager, so the switch queue stays empty. The game drops `manager.menuMic` on a switch
  to the game but leaves the old microphone running as a side process (its loops fade
  out) and makes a new one on the way back; the port keeps one microphone and tracks
  whether the manager holds it, which sounds the same. Mod finalization completes in the
  same frame (no mods). Of `HardCleanFutileAssets`
  only the illustration atlases a process loaded are freed.
- **File resolution.** `AssetManager.ResolveFilePath` looks in `mergedmods/` and the
  active mods first; the port runs the base game with no mods and reads
  `StreamingAssets/<lower-cased path>` only.
- **Strings.** English only; an encrypted `strings.txt` (leading `1`, xor with a key
  string in the game's code) is not read (no vanilla file is encrypted). Mods' string
  files are not merged.
- **Text layout.** `FFont`'s Thai placement rules and other languages' fonts are not
  ported.
- **Menu input.** Keyboard and mouse. Keys come from Rewired's shipped default UI map
  (decoded from `level0`, `rainworld-re/notes/menu.md`): arrows move, Return/Space/Z
  submit, Escape cancel, X "alternate"; the pause button is Escape. Saved custom
  bindings (`RewiredSaveData` in PlayerPrefs), gamepads and the options'
  `mouseButtonMappings` are not read. When two opposite keys of an axis are held the
  port sums them to 0 (Rewired's rule not checked). Whether the default mouse map (left
  button = UI submit, map category 1) is active for player 0 is not settled; the menus
  read the mouse position and buttons directly, as `Menu` does with
  `Input.GetMouseButton`. `options.fullScreen` is the window mode (the options menu
  sets it; `--windowed`, and so `--screenshot`, sets it for the run); as in the game
  (`Options.OnLoadFinished`), the OS cursor is shown only when it is off, and the menus'
  `MouseCursor` sprite only fades in when it is on (`ShowCursor`). The sprite sits at
  the mouse position of each rendered frame, not the last tick's, as in
  `MouseCursor.GrafUpdate`. The dev-tools cases (`Cursor.visible = true`) are not
  ported.
- **Gameplay keys.** Rewired's default gameplay keyboard map: arrows move, Z jumps. Its
  other keys (X throw, left Shift pick up, Space map, C special) have nothing to drive
  yet, but for Space skipping the intro when START completes. The intro roll's "any button" (`ControlSetup.GetAnyButton`) is any key of the
  default maps or any mouse button; Rewired's handling of keys bound in no map is not
  checked (the port knows no such keys).
- **Frame time.** `Time.deltaTime` is capped at 1/3 s, Unity's default
  `maximumDeltaTime`; the game's own setting is not checked. (The game previously ran on
  a fixed timestep with a 250 ms cap; it now runs under `MainLoopProcess.RawUpdate`:
  ticks only while more than one is accumulated, at most three per frame.)
- **Random stream (D1).** One stream, seeded once from `--seed`, from the first menu to
  the game.

**Why.** The menus are ported screen by screen; the world with only overseers
behaving (D14) stands in for a story session. The importer is the user's request: the
port never writes the game's saves, so continuing a game needs a copy.

**Affects.** What is shown at start-up and between processes, and where the random
stream stands when the game starts; no gameplay.

## Not ported yet (not divergences, just absent)

Front end: the regions, arena and Remix menus, input settings, credits, backups,
the dream screens, PASSAGE's custom end-game and fast travel screens, the outro and DLC slideshows, the slugcat select menu's
colour and Jolly Co-op controls, the `SceneRain` and `SceneMultiply` layers, menu objects those use (vertical and
subtle sliders, check box groups, symbol buttons, scroll boxes, `UIelementWrapper`),
Remix's options (FPS, VSync, quality, analogue sensitivity, backgrounds), and the
in-game music beyond the triggers' songs and the threat layers (D12).
