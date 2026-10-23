# rw-sim divergences: zero gravity (D34)

Fourth file of the `rw-sim` divergences (`sim.md` is past 1000 lines). IDs are global.

## D34: Zero gravity (`ZeroG`, `BrokenZeroG`)

`AntiGravity` and `BrokenAntiGravity` (`rw_sim::anti_gravity`, `game_anti_gravity`), the
room's `gravity` and `Room.ElectricPower`, `PhysicsParams::room_gravity`
(`PhysicalObject.gravity = g * room.gravity`, `EffectiveRoomGravity`), the player's `ZeroG`
body mode with the `ZeroGSwim` and `ZeroGPoleGrab` animations (`player/zero_g.rs`), the
corridor kick, `SSMusicTrigger` with `SSSong`, the SS ambience fade and the slugcat's
graphics in zero gravity. Every shared-stream draw is ported in the game's order with local
draw-count tests. Notes: `../../rainworld-re/notes/zero-gravity.md`; surveys
`../../rainworld-re/surveys/zero-g-player.md`, `zero-g-others.md`.

- **The keyboard's `analogueDir`.** The input is digital: the direction keys give
  `analogueDir` as `(x, y)` clamped to length 1 (the game does the same for keys), so
  `ZeroGGamePadIntVec` is `(x, y)` and `SwimDir(false)` equals `SwimDir(true)`. A gamepad's
  analogue stick (partial deflection, the 0.1 dead zone) is not ported.
- **Coral circuit swimming is off.** `circuitSwimResistance`, `curcuitJumpMeter` and
  `swimBits` come from `CollideWithCoralCircuitBit`, a coral object that is not ported; they
  are 0 and none, so the `ZeroGSwim` terms that use them vanish.
- **The oracle scenes' `active = false` is not ported.** Five Pebbles' and Moon's rooms
  switch their `AntiGravity` off in `Oracle` scenes; the port's `AntiGravity.active` stays
  true.
- **Only camera 0.** `BrokenAntiGravity.Update`'s sound and screen movement go to
  `game.cameras[i].room` for each camera; the port has one camera.
- **`SSMusicTrigger` counts a player in a shortcut as out of the room.** The game compares
  `realizedCreature.room` with the room; a creature in a shortcut has no room.
- **Omitted DLC and Remix branches** (listed, not dropped): `BrokenAntiGravity`'s
  constructor and update branches for Moon-heart (`MS`, `SL`), `pebblesEnergyTaken` (`RM`),
  the dream/wait-for-rain branches (`DM`, `LM`) and the story/arena coin flips; the
  filtration power in `Room.ElectricPower`; Remix's `brokenAntiGrav != null` test there;
  `EffectiveRoomGravity`'s `customPlayerGravity` (Saint ascension, Energy cell, Watcher
  levitation); the Saint tongue's `isZeroGMode`; Artificer's zero-gravity pyro jump;
  Gourmand's slam; Remix's upward spear throw in zero gravity and `lastGroundY`; Remix's lizard
  branches (`IsWallClimber` and speed at `room.gravity <= 0.3`, `AImap` accessibility,
  `LizardGraphics` tail snap, `LizardSpit`); `Room.cs` 2333 (random items in `ZeroG` rooms).
- **Zero-gravity readers of objects that are not ported**: scavengers, centipedes and the
  other creatures' own code; `ExplosiveSpear`'s fragments and rag; `OverseerCarcass`;
  `SegmentRope`, `Dangler`; `MeltLights`; `SpinningFan` (the rest of the
  superstructure; `ZapCoil` and `SuperStructureFuses` are ported, D36, `GravityDisruptor`, D37); the shortcut pushers in
  `ShortcutHelper` (`player.gravity * room.gravity`, twice scaled); the player's cloth
  (`PlayerGraphics` 1168). They come with their objects. The zero-gravity vine grab and
  vine pull are ported with the vines (D39).
