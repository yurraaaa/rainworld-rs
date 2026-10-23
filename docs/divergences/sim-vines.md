# rw-sim divergences: climbable vines (D39)

Fifth file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D39: Climbable vines (`ClimbableVinesSystem`, the player's `VineGrab`)

`ClimbableVinesSystem` and `IClimbableVine` (`rw_sim::climbable_vines`), the player's grab in
`MovementUpdate` and the `VineGrab` animation (`player/vines.rs`), the hands on the vine
(`rw_graphics::hand`), the three grab sounds of the base-game implementers, and the
zero-gravity vine grab and vine pull. Behaviour: `../rainworld-re/notes/climbable-vines.md`.
No `UnityEngine.Random` call is made by the vine code of the base game (the sounds' draws
are the usual one-shot's).

**What differs:**
- **The system holds snapshots, not the owners.** The game's list holds the live
  `CoralNeuron`, `CoralStem`, ... objects and the player calls them directly. The port's
  owners (10d3 onward) live in the room's update list, which the player's update cannot
  borrow while the world is borrowed for noises, so the room's [`ClimbableVinesSystem`] holds
  `Box<dyn ClimbableVine>` snapshots, is lent to the player for its update (as the heavy
  carry's data is) and taken back. A `Push` or `BeingClimbedOn` the player makes is applied to
  the snapshot at once, so later queries in the same tick see it as in the game, and logged as
  a `VineEvent`; the owner replays the log on itself in its own update (its `Push`
  differs per class: neuron and tube move position and velocity, a stem moves its chunk, the
  pole mimic only its velocity). An owner refreshes its snapshot (`refresh`) before the
  player's turn and drains the events; until the owners exist (10d3), a room has no system and
  tests register a test-only vine.
- **A vine is identified by a `VineId`**, not a reference: `vines.Contains` and
  `vines[i] == vPos.vine` compare ids. A position on a vine that is not registered
  (`VineGrab` with the vine gone) lets go in `MovementUpdate`, as the game; the graphics skip
  the hands' vine branch for it (the game would still read the removed object). Nothing in
  the base game removes a vine.
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
- Base game, later steps: `CoralNeuron` and `CoralStem` (10d3), `ClimbableCorruptionTube`
  with `DaddyCorruption` (`Grab_Corruption_Tube` is ported and reachable then), `PoleMimic`
  with its creature (`BeingClimbedOn` wakes it and plays `Player_Grab_Pole_Mimic`; the
  `VineKind` exists, the grab plays no sound for it yet).
