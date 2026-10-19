# rw-menu

The front end: Futile's stage, fonts, atlases and labels, menu input, the menu framework
(`Menu`, objects, depth scenes), the screens (`IntroRoll`, `MainMenu`, `OptionsMenu`,
`SlideShow`, dialogs), the menu microphone and the `ProcessManager` that switches between
processes behind a fade. Depends on `rw-assets`, `rw-render` (stage drawing), `rw-graphics`,
`rw-sim`, `rw-random`, `rw-audio`, `rw-save`. Windowing and the real game process live in
`crates/app`. The slugcat select and the sleep/death/starve screens are NOT here but in
`crates/hud` (they own a HUD); only their scenes and `ProcessId`s are. Dev-dep `rw-hud` for tests.

## Map (`crates/menu/src/`)
- `lib.rs`: module list.
- `process.rs`: `ProcessManager`, `MainLoopProcess`, `ProcessFactory`, `ProcessId`, fade (`ProcessManager`).
- `initialization.rs`: `Initialization` (`InitializationScreen`, reduced).
- `intro_roll.rs`: `IntroRoll`. `main_menu.rs`: `MainMenu`, `MainMenuButton`, `UnlockCheatSequence`.
- `options_menu.rs`: `OptionsMenu` incl. save-import button, volume sliders. `dialog.rs`: `Dialog`, `DialogAnswer`.
- `slideshow.rs`: `SlideShow`, `SlideShowId` (intro slideshows). `font_test.rs`: port-only test screen.
- `shared.rs`: `MenuShared`, `MenuContext`, `Progression`, `Dlcs`, `StoryGameInitCondition`, `ScreenState` (RainWorld/manager globals).
- `input.rs`: `UiInputState`, `UiInput`, `KeyboardKey` (`RWInput.PlayerUIInput`, Rewired defaults).
- `sound.rs`: `MenuMic`, `MenuSoundEvent` (`MenuMicrophone`). `translator.rs`: `Translator` (English `InGameTranslator`).
- Futile layer: `stage.rs` (`Container`, `Sprite`, `StageBatch`), `tree.rs` (`StageTree`), `atlas.rs` (`AtlasManager`, `TextureSync`), `font.rs` (`Font`, BMFont layout), `label.rs` (`Label`), `geometry.rs` (`Rect`, `Matrix`), `color.rs` (`HslColor`, `MenuColor`).
- `menu/mod.rs`: `Menu`, `MenuScreen` trait, `MenuCx` (pages, selection, press/hold).
- `menu/object.rs`: `MenuObject`, `Kind`, button/slider/scene data. `menu/build.rs`: object constructors.
- `menu/update.rs`: `Menu.Update` and objects' `Update`. `menu/draw.rs`: `GrafUpdate`.
- `menu/depth.rs` (+ `depth/slugcat_scenes.rs`): `DepthScene`, `MenuDepthIllustration`, `BuildScene` tables, `positions.txt`.
- `menu/scene.rs`: `InteractiveMenuScene.Update`. `menu/slide_show_scene.rs`: `SlideShowMenuScene`, `CameraPath`. `menu/rain.rs`: `RainEffect`.
- `tests/installed_*.rs`: tests against the installed game (depth scenes, fonts, intro roll, menus, options, sleep scenes, slideshow).

## Where is X?
- Process switching / fade / `ProcessId`: `process.rs`.
- Intro roll: `intro_roll.rs` (+ `menu/rain.rs`). Intro slideshow (new Survivor/Monk): `slideshow.rs`, `menu/slide_show_scene.rs`.
- Main menu: `main_menu.rs`. Options and save import: `options_menu.rs` (uses `rw-save` `import_slot`).
- Slugcat select menu: `crates/hud/src/slugcat_select.rs`. Sleep/death/starve screens: `crates/hud/src/sleep_screen.rs` (scenes in `menu/depth.rs`; tests `tests/installed_sleep_scenes.rs`).
- Button press/selection/mouse: `menu/update.rs`, `menu/mod.rs`. Menu object sprites: `menu/build.rs`, `menu/draw.rs`.
- Depth scenes and camera drift: `menu/depth.rs`, `menu/scene.rs`.
- Text/fonts/translation: `font.rs`, `label.rs`, `translator.rs`. Sprite atlases: `atlas.rs`.
- Menu input keys: `input.rs`. Menu sounds and loops: `sound.rs`.
- Hooking the manager into the window/MCP: `crates/app/src/frontend.rs`, `crates/app/src/mcp/session.rs`.

## Links
- Notes: `../rainworld-re/notes/menu.md`, `slideshow.md`, `saves.md`, `sound.md`, `unity-assets.md`, `sprite-atlases.md`, `menu-survey.md`.
- Surveys: `../rainworld-re/surveys/menus.md`, `intro.md`, `depth-scenes.md`, `sleep-death-screens.md`.
- Divergences: `docs/divergences/menu.md` D13 Front end; D1 for menu random draws; D12 for sound.
