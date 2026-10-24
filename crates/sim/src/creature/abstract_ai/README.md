# `creature/abstract_ai/` (the abstract AI)

| File | Holds | Ports |
|---|---|---|
| `mod.rs`, `kinds.rs` | abstract AI state + constructors | `AbstractCreatureAI` subclasses |
| `world.rs` | plain abstract AI in the world (roam, migrate, den) | `AbstractCreatureAI`, `AbstractCreature` |
| `kinds_world.rs` | vulture, cicada, big eel, deer, Miros bird, needle worm behaviours | `*AbstractAI` |
| `path_finder.rs` | abstract pathfinders | `AbstractSpacePathFinder`, `AbstractSpaceNodeFinder` |
