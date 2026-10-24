# `vines/`

| File | Holds (key types) | Ports |
|---|---|---|
| `mod.rs` | `ClimbableVinesSystem` (a room's list of `VineRef`s and the geometry the player climbs them with: overlap, place, direction, radius, climb speed, switch, connect, push), `IClimbableVine` as the `VineHost` / `VineHostMut` traits the live owners are read and called through, `PlacedVines` (vines registered at run time: tests), `RoomVines` / `RoomVinesView` (the room's host built from split borrows: placed vines and the coral system's neurons and stems), `VinePosition`, `VineKind`; no corruption vine object exists yet (D39) | `ClimbableVinesSystem`, `IClimbableVine` |
