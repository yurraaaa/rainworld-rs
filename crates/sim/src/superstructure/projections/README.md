# `superstructure/projections/`

| File | Holds (key types) | Ports |
|---|---|---|
| `projector.rs` | `SuperStructureProjector` (the `SuperStructureProjector` effect, 4 SS rooms): scan lines, cursors on a 15 px grid, single glyphs that spread, glyph matrices; its parts are update-list objects kept in creation order | `SuperStructureProjector` |
| `projected_circle.rs` | `ProjectedCircle` (a spoked ring behind a coral neuron's free end: constructor links to the circles already in the room, `Update`'s radius, blink and turn, `UpdateDepth`; base-game branches, D40) | `ProjectedCircle`, `IOwnProjectedCircles` |
| `scan_lines.rs` | `ProjectedScanLines` (3 SS rooms) and its lines | `ProjectedScanLines` |
| `star_matrix.rs` | `StarMatrix` (the placed `ProjectedStars`, 4 SS rooms): stars, connections, rings, the three rotations (the z one never moves), the projection | `StarMatrix` |
| `room_projections.rs` | a realized room's projectors, scan lines, star matrices and projected circles in its update list | `Room.updateList` |
| `turn.rs` | the projections' turns in the update list, `Room.Loaded`'s `ProjectedStars`, the shader's `_gridOffset`; `Room.NowViewed` makes the projector and scan lines in `effects/particles/turn.rs` | `Room.Loaded`, `StarMatrix.Update`, `SuperStructureProjector.UpdateOffset` |
