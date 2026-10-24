# Where the port stands

Moved from AGENTS.md "Decisions". Update when a step changes behaviour.

Current phase: the player controller in a region whose behaving creatures
are overseers, bat flies and green lizards (all 9 lizard steps
done: decisions, calm behaviours, hunting and fleeing, the player's side of a
bite, fights, communication, missions, carrying and eating, lineages), not the full game. Details
live in `docs/` and `../rainworld-re`; in short:
- A story starts at the campaign's start den; room-exit shortcuts carry the
  player between the region's rooms (loaded on each room's preparer thread,
  handing its two stages to the main thread as the game's, D14). The
  population is generated with the game's draws; every creature but the
  scavengers leaves its den and runs the general or its type's abstract AI
  (D14), unrealized without a body. Green lizards, flies and overseers
  have bodies when their room is realized.
- Lizards: path finder and movement, tracker, vision, relationships,
  noise hearing, social events, graphics, voice (D12), rock/spear hits,
  death and kills. Their AI decides as the game's (step 7a: utility
  comparer, prey/threat/aggression/rain trackers, den finder) and they walk
  on their own (idle, travel, den-going, frustrated, investigating sounds),
  hunt (approach, lounge, bite and hold) and
  flee (7b); rivals fight, signal dominance and submission, and a lizard
  that submits leaves the room on a mission (7c). Their deaths register with
  their spawners (lineages progress on the slugcat's kills; respawn and wait
  lists carried across cycles and saves); lizards stay in shelters and are
  saved there (step 9).
- Overseers: body, AI, graphics, guide tutorial text, Monk's guide light,
  holograms whose triggers are ported (D14). Bat flies: world process,
  realization, flocks, hives, chains, pipe travel, sounds, graphics.
- Player: catches, holds, throws, eats flies (SU_A42's Eat lesson, then
  SU_A43's super-jump); picks up, carries, swaps, drops, swallows,
  regurgitates and throws rocks and spears; the Hunter carries one spear on its
  back (D3); drags dead lizards (through shortcuts, into shelters), which
  the Hunter eats (D3). Bitten by a lizard (step 8,
  D19): held in its jaws (stunned, `dangerGrasp`), struggles free with a
  throw (a weapon at the grabber) or grabs a weapon, dies of a hurting bite;
  60 ticks held is `GameOver(dangerGrasp)`, cancelled if it gets free.
- Saves (`rw-save`, D16): Continue, New Game wipe, start's death-and-quit
  save. Shelters (D17): door closes on a fed, still slugcat, `Win` writes
  the save and region state, the sleep screen (D13) continues the next
  cycle; too little food shows the starve screen. PASSAGE stays greyed out
  (D13). Dreams are a black stand-in. The rain pushes, kills and floods (water rises in
  flood rooms, D47); gates and region changes are not there yet.
- Death in a story session is the game's: the body keeps simulating,
  `GameOver` puts the HUD prompt in game-over mode, the map button 40 ticks
  later saves the death and shows the death screen, whose CONTINUE starts
  the next cycle. Dev starts (`load_room`, `--room`, arena levels) game
  over the same way on an unsaved state (D4, D16), and only there R
  respawns, leaving game-over mode. A body that falls out of the room is destroyed.
- Water uses the game's full wavy surface simulation.
