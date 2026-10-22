# rw-hud

The in-game HUD of a story game (text prompt, food, karma and rain meters,
region map, controls page, pause menu) and the screens built on the same
widgets: sleep, death and starve screens (karma ladder, end-game meters,
tokens, kills) and the slugcat select menu. Depends on `rw-assets`, `rw-sim`,
`rw-random`, `rw-render` (sprite output), `rw-menu` (menu widgets),
`rw-graphics`, `rw-save`. Widgets read their owner through small view structs
(`PromptNeighbours`, `RainCycleView`, `MapOwner`, `OwnerTick`) rather than the sim
directly.

## File map (`crates/hud/src/`)

| File | Holds | Game class |
|---|---|---|
| `lib.rs` | `Hud` (assembly and per-tick update), `HudSetup`, `HudFx`, `OwnerTick` | `HUD.HUD.InitSinglePlayerHud` |
| `text_prompt.rs` | `TextPrompt` (game over waiting on a grasp: `PromptOwner`), `Message`, `SubregionTracker` | `HUD.TextPrompt` |
| `food_meter.rs` | `FoodMeter`, `MeterCircle`, `QuarterPipShower` | `HUD.FoodMeter` |
| `karma_meter.rs` | `KarmaMeter` | `HUD.KarmaMeter` |
| `rain_meter.rs` | `RainMeter`, `RainCycleView` | `HUD.RainMeter` |
| `circle.rs` | `HUDCircle`, `FadeCircle` | `HUDCircle` |
| `map/mod.rs` | `Map`, `MapOwner`, reveal, panning, layers | `HUD.Map` |
| `map/data.rs` | map data from the world, gate requirements | `HUD.Map.MapData` |
| `map/connection.rs` | dashed connection lines | `Map.OnMapConnection` |
| `map/markers.rs` | gate, shelter, death, karma-flower markers | `Map.FadeInMarker` |
| `map/shelter_items.rs` | a shelter marker's item icons: `ShelterSymbols`, `ItemInShelterMarker`, `ItemInShelterData` | `Map.ShelterMarker`, `GetItemInShelter` |
| `icon_symbol.rs` | `IconSymbol`, `IconSymbolData` (item and creature icons), `SymbolWidths` | `IconSymbol`, `ItemSymbol` |
| `controls_page.rs` | SU_C04 keyboard controls picture | `TutorialControlsPage` |
| `pause_menu.rs` | pause menu of a story game | `Menu.PauseMenu` |
| `sleep_screen.rs` | `SleepAndDeathScreen`: sleep, death, starve screens | `Menu.SleepAndDeathScreen` |
| `karma_ladder.rs` | karma ladder and symbols | `Menu.KarmaLadder` |
| `endgame_meter.rs`, `endgame_tokens.rs` | passage meters and tokens | `Menu.EndgameMeter`, `EndgameTokens` |
| `sleep_kills.rs` | kill trophies row | `SleepScreenKills` |
| `sleep_hud.rs` | HUD a sleep/death screen owns | `HUD.InitSleepHud` |
| `screen_package.rs` | data handed to the sleep/death screens | `SleepDeathScreenDataPackage` |
| `select_hud.rs` | karma and food meters on a select continue page | `SlugcatPageContinue` as `IOwnAHUD` |
| `slugcat_select.rs`, `slugcat_select/lights.rs` | slugcat select menu; page mark and glow | `SlugcatSelectMenu`, `SlugcatPage` |
| `tests.rs`, `*/tests.rs` | unit tests | |

## Where is X?

- Food meter pips, hibernation line, quarter pips: `food_meter.rs`.
- Region map: `map/mod.rs`; data: `map/data.rs`; markers: `map/markers.rs`; icons
  of what lies in a shelter: `map/shelter_items.rs`, `icon_symbol.rs`.
- Text prompt, game-over prompt, subregion names: `text_prompt.rs`.
- Karma symbol in play: `karma_meter.rs`; ladder on the sleep screen: `karma_ladder.rs`.
- Rain cycle ring: `rain_meter.rs`.
- Sleep, death and starve screens: `sleep_screen.rs`, `screen_package.rs`.
- Passage meters and tokens: `endgame_meter.rs`, `endgame_tokens.rs`.
- Kill trophies: `sleep_kills.rs`.
- Pause menu: `pause_menu.rs`.
- Tutorial controls page: `controls_page.rs`.
- Slugcat select menu: `slugcat_select.rs`, `select_hud.rs`.
- HUD assembly and per-tick update: `lib.rs` (`Hud`).

## Cross-links

Notes in `../../../rainworld-re/notes/`: `hud.md`, `menu.md`, `campaign-model.md`,
`saves.md`, `shelter.md`. Surveys: `sleep-death-screens.md`, `menus.md`,
`food-pip-clip.md`, `malnourishment.md`, `campaign.md`.

Divergences: `../../docs/divergences/` (one file per crate, IDs global): D15 the in-game HUD, D13 front end (menus,
select, PASSAGE greyed out), D16 saves, D17 shelters.
