# rw-save

The game's save data in its own text format: the slot-file container, `PlayerProgression`,
`SaveState` with its sub-records, the survived/starved cycle's save, options, and the importer
of the real game's slots. Depends on `rw-assets` (installed game files, the checksum salt from
`Assembly-CSharp.dll`), `rw-random`, `md-5`, `directories`. No `rw-sim` dependency: the sim
hands it plain data (`HibernationWorld`, `PlayerSessionRecord`). Everything unrecognised (DLC,
Remix, mods) is kept as read and written back last (`unrecognizedSaveStrings`). Never writes
the real game's save storage.

## Map (`crates/save/src/`)
- `lib.rs`: module list only.
- `user_data.rs`: `UserDataFile`, the `sav`/`sav2`/`sav3` container (`Kittehface.Framework20.UserData`, XML Hashtable, NUL padding).
- `checksum.rs`: `ChecksumSalt`, `md5_sum` (`Custom.Md5Sum`); the salt is read from the installed DLL.
- `cs_float.rs`: `CsFloat`, Mono `float.ToString` formatting (7 digits).
- `progression.rs`: `PlayerProgression` (load, immediate writes, `SAVE STATE`/`MAP`/`MISCPROG` lines, wipe, `CampaignStart`, `SaveError`).
- `save_state.rs`: `SaveState` (`new`, `load_game`, `save_to_string`, kills, shelters, `reds_cycles`).
- `death_persistent.rs`: `DeathPersistentSaveData` (karma, mark, passages, deaths/quits, `Tutorial`).
- `win_state.rs`: `WinState`, `EndgameId` passage trackers (`Progress`, display names, thresholds).
- `misc.rs`: `MiscProgression` (`MiscProgressionData`: selected slugcat, `INTEGERS`, Hunter unlock, `WatchedScreen`).
- `misc_world.rs`: `MiscWorldSaveData`, `PlayerGuideState`.
- `communities.rs`: `CreatureCommunities`, `CommunityId`, `CommunityInfluence`.
- `dreams.rs`: `DreamsState`, `DreamId`, `DreamContext`.
- `songs.rs`: `SongPlayRecord` (`SONGSPLAYRECORDS`).
- `coordinate.rs`: `SavedCoordinate` (`WorldCoordinate` text).
- `session.rs`: `PlayerSessionRecord`, `KillRecord`, `EatenKind`, `HibernationStats`.
- `hibernation.rs`: `RainWorldGame.Win` save side, `SessionEnded` survived/died, `WinOutcome`, `HibernationWorld`.
- `miner.rs`: `SaveStateMiner`, reads single values from a stored `SAVE STATE` string (slugcat select).
- `options.rs`: `Options` (`options` key and `localoptions.txt`), `Language`.
- `dirs.rs`: `Platform`, `SaveSlot`, port save dir vs the game's `persistentDataPath` (Proton prefix).
- `import.rs`: `import_slot`, `ImportReport` (backup to `backup/<unix>_import/`).

## Where is X?
- Parse/write a slot file: `user_data.rs` (container), `progression.rs` (`save` value, lines), `checksum.rs`.
- A campaign's saved game text: `save_state.rs`; its `save_to_string` order matters.
- Karma, passages earned, deaths: `death_persistent.rs`; passage trackers: `win_state.rs`.
- Region state (`RegionState`): NOT here; `crates/sim/src/world/region_state.rs` (text format in notes/saves.md).
- Communities / creature likes: `communities.rs`.
- Cycle end (Win, starve, death save): `hibernation.rs` + `session.rs`.
- Tutorial counters, Hunter unlock, selected slugcat: `misc.rs`, `death_persistent.rs`.
- Options file and language: `options.rs`; where files live: `dirs.rs`.
- Import the game's save: `import.rs` (UI trigger in `crates/menu/src/options_menu.rs`; MCP `import_save`).
- Slugcat select reading saves without loading: `miner.rs`; sleep/death screen data: `crates/hud/src/sleep_screen.rs`.
- Float/coordinate text formats: `cs_float.rs`, `coordinate.rs`.

## Links
- Notes: `../rainworld-re/notes/saves.md`, `campaign-model.md`, `shelter.md`; surveys: `campaign.md`, `sleep-death-screens.md`.
- Divergences: `docs/divergences/save.md` D16 Saves, D17 Shelters and hibernation, D13 (save import button).
