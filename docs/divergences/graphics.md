# Divergences: rw-graphics

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D8: Slugcat graphics (`rw_graphics::PlayerGraphics`)

`PlayerGraphics` ports the base-game Survivor's `PlayerGraphics`, `SlugcatHand`, `Limb`,
`BodyPart`, `TailSegment` and `GenericBodyPart` for the ported body modes and animations, including the vertical-pole poses
(`ClimbOnBeam`, `GetUpToBeamTip`, `BeamTip`, `HangUnderVerticalBeam`), `LedgeGrab`,
the roll/flip tail fling and the corridor branches (`CorridorClimb` legs and
`LegsAClimbing` frames, the doubled downward head lead, the hands gripping the corridor
walls ahead, the `CorridorTurn` jitter and blink), plus the pokes `Player.Update` makes
into its graphics: `Player.Blink` from hard landings (`TerrainImpact`, speed > 12) and
the pole slide-up hop, and the 10 px/tick push of every body part when a corridor turn
ends (`rainworld-re/notes/slugcat-graphics.md`), Remix off. The skid, belly slide,
rocket jump, corridor shoots and slides and corridor drops have no branch of their own
in `PlayerGraphics` (they use the generic poses of their body mode, as in the game).

**What differs:**
- **Random numbers.** Cosmetic randomness (blink timing, the 1 px reset jitter, beam
  balance, long-fall hand flailing, the corridor-turn jitter, the empty-room object
  looker's draws) uses the game's generator and calls, from the same stream as the
  player right after `Player.Update`, as the game's `Room.Update`; only seed and call
  order differ (D1).
- **Swimming** is ported (draw offset from `swimCycle`, tail drag scaled by hip
  submersion, the `lungsExhausted` head bob and blink, the swimming hand targets, legs
  hidden in `DeepSwim`); its bubbles are `Bubble`s in the room (D11), splashes are
  not yet (D11). Under water the sprites
  are tinted by the water's `DeepWater` shader, as in the game (D11).
- **Breathing, exhaustion and malnourishment** are ported: breathing quickens and the
  body pants with `aerobicLevel`; the exhausted eyes and head bob; the malnourished
  slugcat's thinner body, hips and tail, grey body and lighter eyes (toward the
  palette's fog colour), fading by 0.005 a tick once cured. Red's illness (also
  `malnourished` 1) is not ported.
- **Looking and holding.** `PlayerObjectLooker` is ported whole and looks at the
  room's bat flies, overseers, rocks and spears; the hands hold bat flies, rocks and
  spears (a spear swung forward while walking, `spearDir`, with `ChangeOverlap`; the
  throwing hand of `ThrowObject`), reach
  for and bite bat flies and bring a rock to the mouth to swallow it (`SlugcatHand`
  grasp branch, `BiteFly`, `LookAtObject`); the swallow and regurgitation gulp, blink
  and heave of `PlayerGraphics.Update` are ported, and so is the look up at the sky in the
  cycle's last 2400 ticks of a rain room (`lookAtPoint`, D44). The afraid-of-creature
  poses (and their blink and aerobic effects) are not ported.
- **Not ported:** two-handed objects, mauling and
  its `Blink`s,
  the map-button stand-still blink,
  antler climbing (the hands on a vine are ported, D39),
  adrenaline jitter, hypothermia and poison tints, the Mark (sprites 10 and 11
  have alpha 0 and are left out of the draw list), the light source, and every More
  Slugcats, Watcher and Jolly Co-op branch (pups, Gourmand/Spear/Saint/Rivulet/Artificer
  parts and sizes, gowns, custom and Jolly colours). The Nightcat colour is not offered
  (`default_slugcat_color` covers White, Yellow and Red).
- **Drawing (`rw_render` sprites).** `Futile_White` meshes (the tail) sample the centre
  of `RainWorld_White` in the `rainWorld` atlas instead of the runtime 16x16 white
  texture (both opaque white, so the colour is the same). Sprites are drawn without
  antialiasing into the game-resolution frame (D7), so edges are hard pixel edges as in
  the game. The debug view (V) is not the game's: it shows the physical body (chunk
  circles with their radii, connection rods, contact marks, stun/dead styling) instead
  of the sprites, whose state is still updated every frame.
- **Legs on curved terrain.** The grounded legs ignore `terrainCurveNormal` (not
  ported; it is only set by `TerrainCurve` objects, which the port has none of).

**Why.** Randomness as D1; the rest depends on player state that is not ported yet or
is outside the base-game scope.

**Affects.** Exact blink timing and jitter; the look of the slugcat in the unported
states (they fall back to the nearest ported pose: Default for the airborne ones).

**Lizard graphics (`rw_graphics::LizardGraphics`, lizard step 5a).** The green
lizard's `LizardGraphics` constructor, `Reset`, `Update` (culling, the belly-drag and
lounge loop, the `CreatureLooker` over the AI's tracker, eyes, head colour setter and
its bubbles and drips, blink, the head light in a dark room, breath, `LizardLimb`s with
their grip, flip, foot sounds and `LegsGripping`, the depth rotations, the tail, the
head with its jump and `bubble` bubbles), `SuckedIntoShortCut`, `Stun`, `WhiteFlicker`,
`TerrainImpact`, `DrawSprites` for the body circles and mesh, tail mesh, legs and their
colour layer and the five head sprites, in `AddToContainer`'s order, coloured as
`ApplyPalette` and `HeadColor`; `LizardBubble` in full, the head shield's sparks and
`StationaryEffect` flash, and the body part a lodged spear follows. Differences:
- **Cosmetics** (lizard step 5b). The green lizard's (`SpineSpikes`, `BumpHawk`,
  `LongShoulderScales`, `ShortBodyScales`, `TailTuft`, `LongHeadScales`) are ported
  whole: the choice, the constructors, `LongBodyScales.Update` (the scales trailing
  their roots, their shake while `showDominance` is above 0, after a dominance
  signal), `DrawSprites`, the `ApplyPalette` colours (applied every frame, as nothing
  changes them between palette changes for a green) and `AddToContainer`'s overlap
  places, over `SpinePosition` and `GetBackPos`. `graphicHeight` is the
  `LizardScaleA` element's height, kept as a table in `rw_graphics::elements` and
  checked against the installed atlas. The other breeds' choices and cosmetics come
  with them; a HUD cosmetic (none in the base game) would be drawn after those in
  front, in the midground.
- **The voice** (lizard step 5b) is the game's: `voiceVisualization` and its intensity
  pull the jaw open, light the head, blow bubbles and shake the body as the game's do;
  `voiceVisualization` itself is computed in the sim (D12).
- **Objects held in the jaws** (lizard step 8) draw in the head's internal container,
  between the lower and upper head sprites (`AddObjectToInternalContainer`, the
  releases); let go, they draw on top of their containers (`sprite_order.rs` in the
  app, D21).
- **Left out until their step, breed or DLC** (user, 2026-10-16; the AI's own `MakeSound` calls, the
  communication's dominance and submission included, are ported): the yellow pack's `PreyReSpotted`
  voice and other breeds' colour paths (white camouflage and so on; with those breeds),
  More Slugcats' rot voices, Remix's Love voice and safari-control voices (with those
  DLCs).
- **The AI's looks** (lizard step 6a). The looker picks among the tracked creatures,
  the look position follows the one looked at (where it is seen, else its best
  guessed tile), the eye beams follow how well it is seen, and `CreatureSpotted`
  reevaluates the looker and flashes an eye, favouring the prey tracker's
  `MostAttractivePrey` (prey spotted) or the threat tracker's `mostThreateningCreature`
  (threat spotted); a fleeing lizard looks at its path's destination (step 7b). The looker's state lives in
  the sim's graphics link (the AI reevaluates it); the eye flash `CreatureSpotted` sets
  is applied at the start of the graphics' next update, before anything else reads
  the eyes, rather than at once.
- **A dropped module's bubbles, sparks and flash.** A bubble, and a head-shield spark
  or flash, keeps the module it was given, as the game's keep their `LizardGraphics`
  (a spark reads its `whiteFlicker`, frozen once dropped), and reads the lizard's
  excitement and jaw from the creature's realized body, else as the module last read
  them. The game reads the body object the module was made for, so a lizard abstracted
  and realized again within one's life differs.
- **Not ported:** the white lizard's camouflage, the black lizard's head lighting up from
  noise, the salamander's colours, the cyan lizard's tail, the tongue, the vision beams
  (`lizardLaserEyes` is a dev setup value), the debug visualization,
  `overrideHeadGraphic` (Saint timeline and Winterized), mud, and every More
  Slugcats, Watcher and Remix branch (rot, the DLC breeds' cosmetics and colours).
