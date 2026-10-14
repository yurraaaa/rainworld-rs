# Divergences from the original game

Each entry: what differs, why, and what it affects. Behaviour sources are the notes in
`../rainworld-re/notes/`.

## D1: Random number sequence (spawning)

**What.** `rw_sim::determine_safe_spawn_tile` and `rw_sim::place_in_room` draw from a
caller-supplied `rand::Rng` (`rand` 0.10; tests use `rand::rngs::StdRng::seed_from_u64`),
not Unity's `UnityEngine.Random`. For the same game state the port therefore picks a
different spawn tile and different chunk offsets than the game would.
`Random.value` also includes 1.0, while `rand`'s `random::<f32>()` is in `[0, 1)`, so
the port never produces the game's rare `value == 1` edge cases, such as a node index
equal to the exit/den count or an offset of exactly 10 tiles.

**Why.** Unity's generator state comes from the engine (seeded at start-up and shared by
every system that draws from it: sounds, particles, AI), so a draw-for-draw match would
need the whole game's call order, not just these functions. The behaviour that matters
is kept: which tiles can be chosen, in what fallback order, and the offset
distributions. An injectable, seedable generator makes tests deterministic.

**Affects.** Exact spawn tiles and the initial 1 px chunk jitter only. Physics after
placement is deterministic and does not use randomness.

## D2: Spawning in rooms without shortcuts, shelters, and the shortcut map

**What.**
- The game's first spawn phase throws (`shortcuts[0]` on an empty array) in a room with
  no shortcuts. The port skips that phase and goes on to the fallbacks.
- The shelter branches of `DetermineSafeSpawnTile` are not ported, because room files do
  not say whether a room is a shelter.
- Which shortcut leads to which node comes from the game's `ShortcutMapper`, which has
  not been decompiled. The port derives it from tile flags: room exits, then dens,
  then region transports, each scanned top row first and left to right (as in
  `room-format.md` section 5). It traces each shortcut from its entrance along flagged
  tiles, and orders shortcuts by entrance in the same scan order. The tracing and the
  shortcut order are INFERRED.

**Why.** The game's behaviour is a crash or depends on world data the room file lacks.
The shortcut mapping is a stand-in until `ShortcutMapper` is read.

**Affects.** Spawn tiles in rooms without shortcuts, in shelters, and in rooms where
several shortcuts lead to exit/den nodes in a different order than the game's.

## Not ported yet (not divergences, just absent)

Water (buoyancy, water friction), burrowing, terrain curves (`TerrainCurve`), `setPos`
(cross-object position writes), the `TerrainImpact` callback (fall damage and stun),
grabs (`grabbedBy` skips the room-range clamp, and grabs force `goThroughFloors`), and
`DefaultTile`.
