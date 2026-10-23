# rw-sim divergences: climbable vines (D39)

Fifth file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D39: Climbable vines (`ClimbableVinesSystem`, the player's `VineGrab`)

`ClimbableVinesSystem` and `IClimbableVine` (`rw_sim::vines`), the player's grab in
`MovementUpdate` and the `VineGrab` animation (`player/vines.rs`), the hands on the vine
(`rw-graphics` `player/hand.rs`), the three grab sounds of the base-game implementers, and the
zero-gravity vine grab and vine pull. Behaviour: `../rainworld-re/notes/climbable-vines.md`.
No `UnityEngine.Random` call is made by the vine code of the base game (the sounds' draws
are the usual one-shot's).

**What differs:**
- **The vines' owners are reached through a host, and only registered ones.** The game's
  `vines` list holds the `CoralNeuron`, `CoralStem`, ... objects and the player calls them
  directly. The port's system keeps the list as `VineRef`s (world-unique, compared by
  identity, in registration order) and every read (`Pos`, `Rad`, `Mass`, `CurrentlyClimbable`)
  and call (`Push`, `BeingClimbedOn`, with the random stream and sounds) goes to the live
  owner through a `VineHost` / `VineHostMut` at the moment the game makes it, so a push is
  seen by the next read and by the owners' own updates. The player gets the room's system and
  host through `NoiseSink::vines()` (the update's `PlayerHearing` holds the world), the hands
  through `RealizedRoom::vine_view()`. `VineRef::Placed` (vines registered at run time:
  `RealizedRoom::register_climbable_vine`, tests only, live in the room's `PlacedVines`) and
  `VineRef::CoralNeuron` (10d3a, D40: the neurons live in the coral system) and
  `VineRef::CoralStem` (10d3b, D41: the stems live there too, their segments as physical objects)
  exist; later `CorruptionTube` and `PoleMimic`. The host is `RoomVines`, built from split
  borrows of the room for each call (`RealizedRoom::vine_access`; `vine_view` gives
  `RoomVinesView`); each owner adds a variant, a branch in it and its storage.
- **A position on a vine that is not registered** (`VineGrab` with the vine gone) lets go in
  `MovementUpdate`, as the game; the graphics skip the hands' vine branch for it (the game
  would still read the removed object). Nothing in the base game removes a vine.
- **Exceptions are panics.** A vine queried must have two or more points (`Pos(-1)` would
  throw in the game); NaN and out-of-range fractions behave as the game's floats (`PrevSegAtFloat`
  gives the last point, `FloatAtSegment` of coincident points is NaN).
- **`wantToGrab` is a field of the player** read by the grab as last tick's value (the game
  sets it at the end of `MovementUpdate`, and the grab's `wantToGrab = 0` and the beam-tip's
  `-1` are overwritten at once); only `Deer`'s antlers (not ported) also write it.
- **Mod-manager branches resolved for the target (Remix off, base game):** the vine grab is
  allowed while `ClimbOnBeam` (MMF off); a jump press always lets go (no `TubeWorm`
  check); `VineSwitch` is not gated by `GrabbedByDaddyCorruption`.

**Omitted branches (listed, not dropped; each comes with its owner):**
- Remix (MMF): the `ClimbOnBeam` exclusion of the grab (Player 12366), the `VineSwitch` gate
  on `GrabbedByDaddyCorruption` (8834), the `TubeWorm` tongue check on the jump press (8863),
  `TubeWorm.cs` 1197 (a jump press while `VineGrab` releases or shoots the tongue).
- More Slugcats: `MoreSlugcats.ClimbableVine` (placed `Vine`, Outer Expanse): its `Leaves`
  grab sound (1 draw for the pitch) and the climbing rustle (1 draw per tick a direction is
  held, 1 more when it plays); the `animation != VineGrab` terms of the Artificer pyro jump
  (Player 3041, 3224), the Saint tongue (3574), ascension (3713) and the `EnergyCell` check
  (10554).
- Watcher: `SkyWhale.SkyWhaleWhisker` (registers and unregisters per room,
  `CurrentlyClimbable` follows the ripple layer).
- Base game, later steps (`CoralNeuron` is done, D40; `CoralStem`, D41): `ClimbableCorruptionTube`
  with `DaddyCorruption` (`Grab_Corruption_Tube` is ported and reachable then), `PoleMimic`
  with its creature (`BeingClimbedOn` wakes it and plays `Player_Grab_Pole_Mimic`; the
  `VineKind` exists, the grab plays no sound for it yet).
