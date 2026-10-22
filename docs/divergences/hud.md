# Divergences: rw-hud

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D15: The in-game HUD

`rw_hud` ports `HUD.HUD` as `InitSinglePlayerHud` builds it for a story game
(`rainworld-re/notes/hud.md`): the text prompt with its subregion tracker, the food
meter, the karma meter, the rain meter and the tutorial controls page of SU_C04. It
fires up in the camera update once the player is realized in a region world (not for
single-room worlds), updates its parts in the game's reverse order and draws into its
own Futile stage above the room. The map button is `Space` (the default keyboard
binding); holding it stands the slugcat still, as `standStillOnMapButton` does in a
story session.

**What differs:**
- **Region map.** `HUD.Map` is ported for a player-owned story map in its base-game
  branches (`rw_hud::Map`): the map image in three layers through the `Map` and
  `MapAerial` shaders, the reveal from the discovery texture, pan and layers, the
  connection lines, the player marker, gate and shelter markers, the swarm-room circles
  and the Hunter's cycle label, with every random draw, plus the death markers (the save
  state's `DEATHPOSS`) and the karma flower marker. The same map, owned by a sleep or
  death screen (`SleepHud`, D13), fades in without the
  hold and, on a death screen, has no player marker. The player's exit discovery and
  `ReportRoomVisited` run in `Player.Update` as in the game. Differences: shelters show
  no stored items (the port has none); the flower marker reads the save's `FLOWERPOS`
  (the karma flower is not spawned, so the world load's clearing of the position when
  the flower is gone is not ported); a saved coordinate in the old numeric-room form
  places no death marker or flower (the port does not resolve room indices from old
  saves); in a shortcut the owner's position is its vessel's tile
  even while the vessel waits for another room; the cycle label counts 19 cycles (no
  Remix settings, no extra cycles). Not ported: the More Slugcats gate locks, Remix's
  item and creature markers and fast reveal, Watcher warp and spinning-top markers, and
  the fast-travel and region-overview owners.
- **Rain.** The rain meter reads the world's `RainCycle` (its drawn length and
  ticking timer, which the guide's `pauseRain` pins at 2000 in the tutorial rooms; D14).
  The rain itself is not ported; only the bat flies read a room's rain `DangerType`
  (`RoomNotACycleHazard`, D14).
- **Start state.** Food, karma, cap, reinforcement and the cycle number are the save
  state's (a new game's: food 0, karma 0, cap 4, the Hunter karma 2, cycle 0, so the
  meters start hidden); food then rises only by eating bat flies (a full pip each, a
  quarter pip for the Hunter).
- **Shelters.** The owner's `stillInStartShelter`, `readyForWin` and
  `forceSleepCounter` are the player's (D17); every shelter counts as intact (broken
  shelters are not read).
- **Screen meters.** The food meter's sleep and death screen branches (`SleepUpdate`,
  `DeathUpdate`, the eat circles and `FoodCountDownDone`) and `InitSleepHud` are ported
  (`rw_hud::SleepHud`), with the sleep-death data package `CommunicateWithUpcomingProcess`
  builds (`SleepDeathScreenDataPackage`), for the screens of D13. The package's save
  state is the session's: after a malnourished `Win` the progression's starved state.
- **Game over.** `Player.Die` → `RainWorldGame.GameOver` → `InitGameOverMode` with the
  food in the stomach, `Players[0].pos.room` and the main chunk at the death restricted
  to the room grown by 50 px; then the music's `DeathEvent` (its song fade and the
  threat's decline counter, D12). The prompt's game-over press
  reads this tick's map button (the default keyboard binding, `GetButton(11)`), not the
  dead player's input. A lizard holding the player 60 ticks calls `GameOver(dangerGrasp)`:
  the prompt keeps the grasp (`dependentOnGrasp`) and leaves game-over mode when the
  living player gets free (D19). Differences: the gamepad and rebound-map branches (the pause
  button press and their strings) are not ported; dev starts have no game over (D4);
  the Hunter out of cycles (`GoToRedsGameOver`) is not ported (D17).
- **Not ported:** the prompt's symbol and music messages, the Hunter's
  cycle count-up, the karma meter's gate and scavenger blinking, the gamepad pictures of
  the controls page, the character-select screen's meters, and every
  DLC part (More Slugcats pup meters, ammo meter, Watcher ripple symbols, Jolly Co-op
  meters, Remix's options).

**Why.** The rain cycle, eating, saves and shelters are later milestones.

**Affects.** What the meters show (food, rain) and when they show in shelters, and the
map's markers; no gameplay beyond the map button standing the slugcat still.
