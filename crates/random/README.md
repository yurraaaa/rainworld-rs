# rw-random

Unity's `UnityEngine.Random` (xorshift128 seeded by `InitState`, Unity's float and
integer conversions) as `UnityRandom`. Single file, only dependency `rand_core`
(it also implements `TryRng`/`SeedableRng`; rand's own conversions do NOT match Unity's,
use the inherent methods). Every crate that draws from the game's shared stream uses it.

- `src/lib.rs`: `UnityRandom` (`new`, `init_state`, `from_state`/`state`/`set_state`,
  `next_u32`, `value`, `range_f32`, `range_i32`, `inside_unit_circle`, `on_unit_sphere`,
  `inside_unit_sphere`); tests in the same file.

Notes: `../../../rainworld-re/notes/unity-random.md`. Port-wide draw-order rules: `../../docs/divergences/random.md` D1.
