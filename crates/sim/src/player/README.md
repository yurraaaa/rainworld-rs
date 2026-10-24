# `player/` (slugcat; `Player.cs` split by topic)

`mod.rs` (`Player`, two-chunk body), `input.rs` (`Input`), `movement.rs`
(`MovementUpdate`), `body_mode.rs` (`UpdateBodyMode`), `animation.rs`
(`UpdateAnimation`), `jump.rs`, `jump_on_chunk.rs` (`jumpChunk`: `Collide`'s note of
the chunk under the hips, `JumpOnChunk`), `advanced.rs` (flip, roll, rocket jump, belly slide),
`poles.rs`, `ledge.rs`, `corridor.rs`, `swim.rs` (water, breath, `LungUpdate`),
`falls.rs` (impact, stun, die), `grabbed.rs` (held: `dangerGrasp` state and timers,
`cantBeGrabbedCounter`, the pacifying stun, `WALLSTUCK`), `exhaustion.rs` (aerobic level, malnourishment),
`grab.rs` (hand counters, `ThrowDirection`), `carry.rs` (`HeavyCarry`: run slowdown, drag through narrow spaces, pulled into a shortcut, jump power), `shortcut.rs` (enter/ride/leave tubes),
`circuit.rs` (`CollideWithCoralCircuitBit`, `circuitSwimResistance`, `curcuitJumpMeter`, `swimBits`), `zero_g.rs` (the `ZeroG` body mode, `ZeroGSwim` and `ZeroGPoleGrab`, the entry and exit checks), `vines.rs` (the vine grab in `MovementUpdate` and the `VineGrab` animation; the room's vines come through `NoiseSink::vines()`), `sleep.rs` (`SleepUpdate`), `sounds.rs` (slide loop, plays), `drips.rs` (the skid's and
corridor shoots' `WaterDrip`s), `shortcut_push.rs` (a wrong hole pushing the player out: `ShortcutHelper.Update`'s player part, `PushPlayerTowardsValidNeighbor`), `spear_on_back.rs` (the Hunter's
`SpearOnBack` state, `DropSpear`'s draws in `Stun` and `Die`).

| File | Holds (key types) | Ports |
|---|---|---|
| `carried_turn.rs` | what a creature's vessel carries: `FollowedByCamera`, stuck objects moving out of the lobby, the held player spat out with its grabber or killed in its den | `AbstractCreature.FollowedByCamera`, `Creature.SpitOutOfShortCut` |
| `eat_meat_turn.rs` | eating a held dead creature: `CanEatMeat`, `GrabUpdate`'s meat block, `EatMeatUpdate` (bites, drips, food) | `Player.EatMeatUpdate` |
| `turn.rs` | slugcat hands: bat-fly catch, eat, swallow, throw, hand switch; the heavy carry's rod and the body lent to the player's update; the jump chunk's masses lent and `JumpOnChunk`'s push and stun applied | `Player.GrabUpdate`, `Collide`, `JumpOnChunk` |
| `grabbed_turn.rs` | the player held: `creature_grab` (`CanBeGrabbed`, `Player.Grabbed`), the `dangerGrasp` block, `ThrowToGetFree`, `DangerGraspPickup`, `GameOver(dangerGrasp)`, the release under the room | `Player.Update`, `Creature.Grab` |
| `spear_on_back_turn.rs` | the Hunter's spear on the back: stash/retrieve by holding pick-up, `SpearToBack`, `SpearToHand`, `DropSpear` applied, carried on the back | `Player.SpearOnBack` |
