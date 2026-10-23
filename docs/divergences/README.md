# Divergences from the original game

Each entry: what differs, why, and what it affects. Behaviour sources are the notes in
`../../rainworld-re/notes/`. One file per crate; entry IDs (D1, D2, ...) are global and stable, so cite them as before. New divergences go in the owning crate's file with the next free ID.

Retired IDs (never reused): D23 (the ant drawn without the `SpecificDepth` shader; the shader was ported in effects step 2).

| Crate | File | Entries |
|---|---|---|
| `rw-app` | [app.md](app.md) | D18 Debug UI and dev spawns (port-only dev tools), D20 A hold cycle between lizards' jaws stays drawn, D21 The flies' sprite re-adds placed at the end of their tick |
| `rw-audio` | [audio.md](audio.md) | D12 Sound |
| `rw-graphics` | [graphics.md](graphics.md) | D8 Slugcat graphics (`rw_graphics::PlayerGraphics`) |
| `rw-hud` | [hud.md](hud.md) | D15 The in-game HUD |
| `rw-menu` | [menu.md](menu.md) | D13 Front end (process manager, menus, menu input) |
| `rw-random` | [random.md](random.md) | D1 Random number sequence |
| `rw-render` | [render.md](render.md) | D7 Room graphics (level image and palette), D22 Room light and palette darkening without a rain cycle, D26 Custom decals and rainbows, D31 Background scenes: the shaders and their inputs |
| `rw-save` | [save.md](save.md) | D16 Saves |
| `rw-sim` | [sim.md](sim.md) | D25 Lightning (omitted DLC branches), D2 Spawning in rooms without shortcuts and in shelters, D3 Player movement (stage one), D4 Falls, stun and death, D5 Shortcuts, D6 Vertical poles and ledge grabs, D9 Skid, flip, roll, belly slide and rocket jump, D10 Corridors, D11 Water and swimming, D14 Worlds, rooms and the room realizer, D17 Shelters and hibernation, D19 The player held by a creature, D24 Light sources; [sim-particles.md](sim-particles.md): D29 Sky dandelions, green sparks, zero-gravity specks and corruption spores; [sim-fixtures.md](sim-fixtures.md): D30 Light fixtures and waterfalls; [sim-scenes.md](sim-scenes.md): D32 Background scenes (omitted branches) |
