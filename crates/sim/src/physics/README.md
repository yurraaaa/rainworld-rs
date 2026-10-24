# `physics/`

| File | Holds (key types) | Ports |
|---|---|---|
| `chunk.rs` | `BodyChunk` integration + tile collision, the `goThroughFloors` getter (`pulled_into_shortcut`, `grabs_disable_floors`) | `BodyChunk.Update` |
| `object.rs` | `PhysicalObject`: chunks + distance constraints | `PhysicalObject` |
| `shared_physics.rs` | point terrain collision, exact tile ray trace | `SharedPhysics`, `Custom.RectCollision` |
| `room_ext.rs` | position lookups on `Room` (`RoomExt`) | - |
