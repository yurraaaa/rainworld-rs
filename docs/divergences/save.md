# Divergences: rw-save

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D16: Saves

**What.** `rw-save` reads and writes the game's save format (`rainworld-re/notes/saves.md`):
the `UserData` container (a serialised `Hashtable`), the salted MD5 checksum (the salt is
read from the installed `Assembly-CSharp.dll`, not shipped) and the progression lines.
The port keeps its saves in its own directory (`~/.local/share/rainworld-rs/`,
`%APPDATA%\rainworld-rs\`, `--save-dir`), never the game's; `import_save` copies one of
the game's slots in, after moving the port's previous file to `backup/<seconds>_import/`.
What is ported of the save cycle: a story game's start (`GetOrInitiateSaveState`, saved
at once as a death and a quit), Continue from the stored state's den, food, karma, cycle,
`NEXTID`, `SEED`, respawn list and the den region's `REGIONSTATE` (population, consumed
items, swarm rooms, `ROOMSVISITED`), New Game's `WipeSaveState`, `ExitGame`'s save
(`SaveProgressionAndDeathPersistentDataOfCurrentState`, from the pause menu's Exit) and
`Win`'s whole save state (`SessionEnded(survived: true)`, `BringUpToDate`,
`SaveWorldStateAndProgression`, the once-per-run `_Backup` copy; D17).
Differences:
- **Writing what the port does not own.** The game rebuilds `MISCPROG`, the save state
  and its death-persistent data from its fields, in its order, with unknown (DLC, Remix,
  mod) entries moved to the end, and drops every line of the progression it does not
  own on `SaveToDisk` (stored `MAPUPDATE` times of maps not loaded, other slugcats'
  `MAPUPDATE_<slugcat>` lines) and, with Remix off, Remix's `OBJECTS`/`FRIENDS` save-state
  entries. The port keeps every line and entry it does not own as read, in place (user
  decision: imported DLC data is kept and round-trips unchanged). It rewrites only
  `CURRENTSLUGCAT`, `INTEGERS` and `CONDITIONALSHELTERDATA` in `MISCPROG` (adding the
  first two if missing), the death-persistent data (typed fields in the game's order,
  every other entry kept, unknown ones at the end as the game writes them) and map
  lines it holds a texture or time for. `Win` writes the whole save state in the game's
  order, its unknown entries kept at the end.
- **What a hibernation saves.** A creature's state string is read and written as the
  game's `CreatureState` and its base subclasses do (`creature/state_save.rs`): `Dead`,
  `Social` (the social memory, `SocialMemory.ToString` with its trailing separator when
  the last relationship is not saved), `SpawnData`, `MeatLeft` (written when it differs
  from the template's `meatPoints`), `MiscFlags`, `MiscData`, `HealthState`'s `Health`, the
  lizard's throat, limbs and rot type, the mouse's battery, the vulture's `NOMASK`, the
  needle worm's `EGGSPAWN`, the centipede's shells and `MEATINIT`, then the other
  entries in the order read (the DLC ones among them, as with the DLCs off). A region
  loaded cycles later ticks each state once (`RainCycleTick`: health, a lizard's limbs,
  the social memory fading, a centipede's shells); rocks and
  spears are saved as the game saves them (in shelters, and spears stuck in walls), but
  saved objects of other kinds are kept as text and written before them (the game
  writes those it recognizes in room order with the rest, and drops karma flowers);
  sticks (`STICKS`, which the game rebuilds from the shelters' objects) are kept as
  read; `Win` writes the player's held objects (`PLAYERGRASPS`) and swallowed object
  (`SWALLOWEDITEMS`) as `BringUpToDate` does, but a swallowed creature or object of a
  kind the port does not type stays as its save string (it cannot be coughed up) and
  is written back as read, and so are friends in the den. `Win` appends the cycle's kills
  (`AppendKills`; a saved kill the port cannot type stays as read), applies the
  session's `InfluenceLikeOfPlayer` calls to the communities, adds the cycle's respawn
  and wait-respawn spawners, and writes the guide's whole `PlayerGuideState`. A saved creature the world cannot place
  stays in the saved list. Session time is the sim's ticks.
- **Win state.** Integer and float passage trackers are read and written as the game
  does, including the death penalty (applied, as in the game, only to a tracker whose
  last shown progress is below its maximum); the bool-array and list trackers (Traveller,
  Dragon Slayer, Scholar) are written back as read, so the game's re-encoding of a
  More Slugcats Dragon Slayer list as base-game bools and its Traveller slot count are
  not applied.
- **Screens' data.** The sleep and death screens' save-side reads are ported and used
  (D13): `WinState.PlayerDied`, each tracker's `GoalFullfilled`/`AnyProgressToShow`,
  the `MiscProgressionData` watched-screen counters (`INTEGERS` 1-4), the
  death-persistent `METERSSHOWN` list, `DEATHPOSS` and `FLOWERPOS`;
  `ResetLastShownValues` runs where the game runs it (a starved state taken up by
  `GetOrInitiateSaveState`). `GetNextEndGame` waits for the PASSAGE button, which is
  disabled.
- **Container.** A new key goes last (the game's `Hashtable` orders keys by string
  hash); the files have no NUL padding (the game writes `MemoryStream.GetBuffer()`);
  they are written to a temporary file and renamed.
- **Continue.** The den region's saved region state and the respawn list are applied
  (`AdaptWorldToRegionState`, `GeneratePopulation(fresh: false)`) and a swallowed rock
  or spear is back in the stomach (the `Player` constructor); held objects are not
  restored, as in the base game (only More Slugcats' forced end rooms read
  `PLAYERGRASPS`); the guide's like, hand holding and anger are applied, while
  communities (no creature reads them), kills and dreams are kept in the save.
  `BackwardsCompability` world-version updates and the automatic `backup/` copies
  (`CreateCopyOfSaves`) are not ported. The slot is `options.saveSlot` (D13).
- **Reset progress.** `WipeAll` keeps only `MISCPROG`, as the game, with its shelters,
  conditional shelters, menu region, the Hunter's flower, campaign timers and the
  base `INTEGERS` it resets cleared; the DLCs' integer lists (`INTEGERSMMF`, ...) are
  kept as read. There is no `SJ_<slot>` folder to delete.
- **Starving and dying.** The starve and death screens follow (D13).
  `GoToStarveScreen` (a door closing on less than one pip, or on less than a full meter
  in a malnourished cycle; D17) and `GoToDeathScreen` run `SessionEnded(survived: false)`
  and its death save, with the game-over prompt's death room and position (room
  0 and (0, 0) on a starve, as in the game). `GoToDeathScreen` comes from the game-over
  prompt's map press (D15) or a shelter door closing on a dead slugcat. The save's
  `FLOWERPOS` is written from `PlaceKarmaFlowerOnDeathSpot` (D17).
- **Dev starts save nothing** (port-only; user decision). A dev start (`--room`,
  `load_process`'s game, arena levels; D4) plays on a copy of the slot's progression
  with no file (`PlayerProgression::detached`), made afresh for each dev game, with a
  new game's save state at the start room. `Win`, starving, a death's game over and the sleep,
  starve and death screens run on it as in a story, but nothing reaches the disk (no save, backup or
  map write, and its `Win` does not use up the run's `_Backup`); the slot's
  progression comes back before any process but the game and its screens.
- **Session time.** The game stops counting `SessionRecord.time` while the prompt is in
  game-over mode (`Player.Update`, `TimeTick`) and counts `playerGrabbedTime` while a
  living player is in it; the port's session time is the sim's ticks, which go on.
- **Map discovery.** `progression.mapDiscoveryTextures` and `mapLastUpdatedTime` are
  split: the game's copy is the sim's `MapDiscovery`, and the progression holds their
  save form (`map_discovery`, `map_last_updated`). The map's first update loads a
  region from the progression, else its `MAP`/`MAPUPDATE` lines (`LoadMapTexture`); the
  pause menu's Exit copies the sim's textures and times into the progression before
  saving. A game left any other way loses its discovery, which the game keeps in memory.
- **Songs and tutorial messages.** `SONGSPLAYRECORDS` is typed (`rw_save::SongPlayRecord`)
  and live: `GameRequestsSong` reads and writes the current save state's records (D12),
  saved with the rest of the death-persistent data. `TUTMESSAGES` is read in both the
  old digit format and the name list (`ParseTutorialMessages`); setting a message
  (`DangleFruitInWaterMessage`, D14) writes it back as a name list, as the game does.
  The guide's `PlayerGuideState` is read and written with every base-game field
  (`rw_save::PlayerGuideState`; the legacy digit item and creature lists are read as
  the game's `ParseItemTypes`/`ParseCreatureTypes` do), its `ToString` included: a
  state with forced directions given saves only those (the game's `text =` bug). The
  per-cycle fields (`want*HandHoldingThisCycle`, `hasBeenToASwarmRoomThisCycle`,
  `increaseLikeOnSave`) are not saved and live on with the state in memory, as in the
  game. Unknown entries are kept. Moon's state (`SLaiState`) is kept as read, so
  `SLOrcacleState.ToString`'s raising of her like on each save is not applied.
- **Play time.** `AppendTimeOnCycleEnd` is not ported: `ExitGame`, starving and dying add
  nothing to `DEATHTIME` or `TOTTIME`; `Win` adds the cycle's ticks / 40 to `TOTTIME`.

**Why.** The creature states, items and kills the save keeps come with their
creatures and objects; the user decided imported DLC and mod data must survive the
port's writes.

**Affects.** Saves the port writes; the game reads them, but they can differ from what
the game would write in line and entry order and in the data it would drop.

## Not ported yet (not divergences, just absent)

Passage triggers (`WinState.CycleCompleted` and `PlayerDied` are ported whole; these
inputs to them are not): kills of anything but bat flies, overseers and green lizards
(a green counts 2, so it feeds the Outlaw, breaks the Saint's streak and marks the
Dragon Slayer's first slot; no other lizard has a body yet); plant food, so the Monk
only resets;
pearls found (Scholar); a friend in the den (the Friend only resets); changes to the
scavengers' like (Chieftain: the saved like is read); region gates, so the Wanderer
(Traveller) can only mark SU. More Slugcats' Nomad, Pilgrim, Martyr, Mother and
Gourmand trackers are kept unrecognized, as the game does with More Slugcats off.
