# `superstructure/grab/`

| File | Holds (key types) | Ports |
|---|---|---|
| `turn.rs` | the superstructure's screen-grab objects in the game: `Room.Loaded` making `DeepProcessing`s, `DeepProcessingLight`s and `GravityDisruptor`s (the twenty specks first), `AIMapReady` collecting the room's zero-gravity specks, their turns in the update list, the disruptor's swirl over the room's `physicalObjects` | `Room.Loaded`, `GravityDisruptor.Update`, `GravityDisruptor.AIMapReady` |
| `deep_processing.rs` | `DeepProcessing` (a placed quad whose `power` chases `Room.ElectricPower`; the shader is `rw-render`'s) and `DeepProcessingLight` (a blue light following the electric power) | `DeepProcessing`, `DeepProcessingLight` |
| `gravity_disruptor.rs` | `GravityDisruptor` (the swirl's numbers for body chunks and zero-gravity specks, the sixteen streaks, the power following `CurrentAntiGravity`) and its `DisruptorSpeck`s | `GravityDisruptor`, `GravityDisruptor.DisruptorSpeck` |
