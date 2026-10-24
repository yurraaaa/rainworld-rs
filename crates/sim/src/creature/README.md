# `creature/` (shared creature layer)

| File | Holds | Ports |
|---|---|---|
| `mod.rs` | `AbstractCreature`, `EntityId`, `IdIssuer`, `WorldCoordinate`, `CreatureState`, `RealizedCreature` | `AbstractCreature` |
| `template.rs`, `template_abstract.rs`, `pathing.rs`, `relationships.rs`, `violence.rs` | `CreatureTemplate` data, resistances, relationship tables, damage data | `StaticWorld.InitStaticWorld`, `LizardBreeds` |
| `population.rs` | world creature generation | `WorldLoader.GeneratePopulation` |
| `state_save.rs` | creature state save string | `CreatureState` |
| `centipede.rs` | `AbstractCreature::centipede_generate_size` (size from the ID, for its icon) | `Centipede.GenerateSize` |
| `death.rs`, `grasp.rs`, `danger.rs` | kill tag/die, `Killing`, `AddCreatureToRespawn` and the lineage roll; `Grasp` + grip sticks; threat/danger reads | `Creature.Die`, `Creature.Grab` |

Sub-folders with their own file maps: `abstract_ai/`, `ai_modules/`, `social/`, `lizard/`.

## Creature kinds

- `fly/`: `mod.rs` (`Fly`), `update.rs` (`Fly.Update`), `ai.rs` + `ai_update.rs`
  (`FlyAi`, flocking), `room_ai.rs` (`FliesRoomAi`, hives), `world_ai.rs` (`FliesWorldAI`:
  the bat-fly world process), `swarm_rooms.rs` (`SwarmRoomMapper`), `turn.rs` (bat flies in the tick: world process, `ReadyForAI`, vessels, lobby turns; the creatures' graphics modules, `modules_no_longer_viewed`/`modules_now_viewed`, run by the camera's deferred room change).
- `overseer/`: `abstract_ai.rs` (`OverseerAbstractAi`), `ai.rs` (`OverseerAi` state,
  zip matrix), `body.rs` (`OverseerBody`), `update.rs` (the whole realized update),
  `tutorial.rs` (`TutorialBehavior`, `TutorialMessage`), `guide_state.rs` (`PlayerGuideState`),
  `world_ai.rs` (`OverseersWorldAI`: shelter finder, guide direction),
  `holo_light.rs` (`HologramLight`), `holo_light_turn.rs` (Monk guide light in the update list), `turn.rs` (an overseer's turn in the game: what it reads of the player and story, the effects it hands back), `hologram/` (the projected pictures: `mod.rs`
  `OverseerHolograms`, `add.rs` `TryAddHologram`, `update.rs`, `parts.rs`, `instruction.rs`,
  `world_queries.rs`).
- `path_finder/`: `mod.rs` (`PathFinder`), `mapper.rs` (`AccessibilityMapper`, world path
  finders), `lizard.rs` (`PatherKind`, the lizard pather's cost functions).
- `lizard/`: see `lizard/README.md`.
