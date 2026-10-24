# `creature/lizard/` (the lizard)

- `lizard/`: `mod.rs` (`Lizard`), `breed.rs` (`LizardBreedParams`, green only), `ai.rs`
  (`LizardAi` state, `LizardBehavior`, injury utility), `movement.rs` (`MovementAnimation`), `voice.rs` (`LizardVoice`),
  `bubble.rs` (`LizardBubble`), `graphics_link.rs` (state shared with `rw-graphics`, with the
  jaws' `internalContainerObjects`), `world.rs` (lizard realize/abstractize, `RealizeInRoom`);
  turn files in the table below.

| Lizard turn file | Holds |
|---|---|
| `lizard/turn.rs` | realized lizards in the update list, vessels, `SpitOutOfShortCut` |
| `lizard/act_turn.rs` | lizard movement (`Act`, `FollowPath`, animations) |
| `lizard/ai_turn.rs` | lizard AI update: the flee heavy-AI gate, moods, `DetermineBehavior`, behaviours, `IdleSpotScore`, base AI module order |
| `lizard/hunt_turn.rs` | Hunt and Flee bodies, `AggressiveBehavior` (lounge decision, bites), fall risks, `BitCreature` |
| `lizard/fight_turn.rs` | Fighting and ActingOutMission bodies, the signals between lizards (`SendCommunication`, `RecieveCommunication`), a rival's anger on touch |
| `lizard/hits_turn.rs` | lizard damage dealt/taken, bites (and the jaws' internal container), stun, spear stick |
| `lizard/perception_turn.rs` | lizard eyes, tracker representation, relationships |
| `lizard/graphics_turn.rs` | game side of `LizardGraphics` (bubbles, drips, head light) |
| `lizard/voice_turn.rs` | `LizardVoice` update and emitters |
