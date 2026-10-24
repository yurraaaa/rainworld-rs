# rw-sim

The gameplay simulation: a fixed 40 Hz tick over the world, rooms, the slugcat,
creatures (abstract and realized), items, water, shortcuts, shelters, noise and the
game's random draws. It is engine-agnostic: no I/O, no windowing, no rendering, no
cosmetic graphics (those are `rw-graphics`, `rw-render`), no save-file I/O
(`rw-save`), no HUD (`rw-hud`), no menus (`rw-menu`). Where the sim needs a graphics
module's state it keeps a small "link" struct (e.g. `creature/lizard/graphics_link.rs`).
Entry point: `RainWorldGame` in `game/mod.rs` (the app's `Game` wraps it). Draws come from the
shared `UnityRandom` stream (`rw-random`); every sound makes its draws too (`sound/mod.rs`).

Navigation aids: module headers (`grep -n '^//!' <file>`); comments name the decompiled
`Type.Member` they port, readable with `../../../rainworld-re/tools/cs.py show Type.Member`.
Notes/surveys are in `../../../rainworld-re/` (see Cross-links).

## Layout

`src/` is split by topic (table below). The module tree is the folder tree. `lib.rs` declares the topics and re-exports the crate's API at the
root (`rw_sim::Vec2`, ...); the public modules are `creature`, `item`, `world` and the
re-exported `campaign`, `color`, `insect`, `map_discovery`, `room_script`, `shelter`,
`trigger`. `world/mod.rs` and `creature/mod.rs` re-export their topics' types
(`rw_sim::world::FliesWorldAi`, `rw_sim::creature::Tracker`) wherever those now live.

A topic folder holds three kinds of file beside each other:

- **Types** (`x.rs`): the decompiled classes the topic ports.
- **Turn files** (`turn.rs`, `<part>_turn.rs`): the topic's `impl RainWorldGame` code
  (making its objects in `Room.Loaded`, their turns in the update list). `turn.rs` is
  the folder's own topic, `<part>_turn.rs` one part of it (`creature/lizard/act_turn.rs`).
- **World files** (`world.rs`, `<part>_world.rs`, or a type's own file such as
  `creature/fly/world_ai.rs`): the topic's `impl World` code (realizing, abstracting,
  the world's update of an AI module).

`RainWorldGame`'s and `World`'s fields and the helpers these files share are
`pub(crate)`.

**Tests**: unit tests of `x.rs` are `tests/x.rs` in the same folder, declared from `x.rs`
as `#[cfg(test)] #[path = "tests/x.rs"] mod tests;` (a child module, so private items
stay reachable); a folder's `mod.rs` tests are `tests/mod.rs` (`game/tests/mod.rs` has
one child module per topic of `game/mod.rs`'s own code). `src/tests/mod.rs` holds the
test rooms the unit tests share. `../tests/` holds only the `installed_*.rs`
integration tests.

## Files by topic

Each topic folder has a `README.md` with its file map (file, what it holds, what it ports);
read this overview plus the one topic you need. `lib.rs` is the only file mapped here.

| Topic | Holds |
|---|---|
| [`game/`](src/game/README.md) | `RainWorldGame`, tick order, shortcuts, shelters, rain turns, camera, triggers, dev tools |
| [`math/`](src/math/README.md) | vectors, colour and scalar helpers |
| [`physics/`](src/physics/README.md) | chunks, bodies, collision |
| [`player/`](src/player/README.md) | the slugcat (`Player.cs` split by topic) |
| [`creature/`](src/creature/README.md) | shared creature layer, creature kinds, path finder; sub-READMEs for `abstract_ai/`, `ai_modules/`, `social/`, `lizard/` |
| [`item/`](src/item/README.md) | weapons, spear sticks, objects |
| [`sound/`](src/sound/README.md) | sounds, noise, water and dripping sounds |
| [`water/`](src/water/README.md) | water surface, bubbles, splashes, drips, waterfalls |
| [`effects/`](src/effects/README.md) | cosmetic and gameplay effects; sub-READMEs for `lights/`, `sky/`, `particles/` (with insects), `gameplay/` |
| [`superstructure/`](src/superstructure/README.md) | `projections/`, `electric/`, `grab/`, `coral/` (each with a README) |
| [`vines/`](src/vines/README.md) | climbable vines |
| [`world/`](src/world/README.md) | the loaded world, realizer, rain cycle, AI map, abstract layer |

## Where is X?

| Question | Look in |
|---|---|
| Shortcuts / pipes, vessels, lobby | `game/shortcut.rs`, `player/shortcut.rs`, `game/shortcut_turn.rs` (lobby), `game/shortcut_helper.rs` + `shortcut_helper_turn.rs` + `player/shortcut_push.rs` (`ShortcutHelper`: wrong holes, objects pushed out of entrances), `creature/lizard/turn.rs`, `creature/fly/turn.rs`, `player/carried_turn.rs` (what a vessel carries) |
| Water, swimming, drowning | `water/mod.rs`, `water/world.rs`, `water/bubble.rs`, `water/splash.rs`, `player/swim.rs`, `water/water_drip.rs` |
| The water's surface sound | `sound/water_sound.rs`, `sound/water_sound_turn.rs` (audio mix: `rw-audio` `voice.rs`) |
| Ceiling drips, the cycle start's dripping sound | `water/water_drip.rs`, `sound/dripping_sound.rs`, `water/drips_turn.rs`, `game/screen_shake.rs` (shaken drips) |
| `rainWorld.screenSize` (the chosen resolution) | `World::screen_size`/`set_screen_size` (`world/mod.rs`; the app sets it before the first room loads), read by `effects/gameplay/border_push_back.rs`, `effects/gameplay/electric_death.rs` (`LightFlash`), `effects/sky/background_scene_turn.rs` (convergence point), `CameraShake` |
| Camera shake, `ScreenMovement` | `game/screen_shake.rs` (sim side), `rw-render` `room_camera.rs`, app `scene.rs` |
| Abstract creature AI (roam, migrate) | `creature/abstract_ai/` (`world.rs`, `path_finder.rs`, `mod.rs`) |
| Type-specific abstract AI | `creature/abstract_ai/kinds_world.rs`, `kinds.rs`, `creature/overseer/abstract_ai.rs` |
| Den logic | `world/dens.rs`, `creature/abstract_ai/world.rs` (den counters), `world/creatures.rs` |
| Population generation | `creature/population.rs` |
| Tracker / relationships | `creature/ai_modules/` (`tracker.rs`, `relationship_tracker.rs`, `tracker_turn.rs`, `tracker_world.rs`) |
| Noise and hearing | `sound/noise.rs`, `sound/noise_turn.rs`, `creature/ai_modules/noise_tracker_world.rs`, `noise_tracker.rs` |
| Spear sticking, lodged spears | `item/spear_stick.rs`, `item/weapon.rs`, `creature/lizard/hits_turn.rs`, `item/hits_turn.rs` |
| Thrown weapon hits | `item/hits_turn.rs`, `item/weapon.rs` |
| Lizard movement / path following | `creature/lizard/act_turn.rs`, `creature/lizard/movement.rs`, `creature/path_finder/` |
| Lizard decisions (behaviour, utilities, idle spot, den) | `creature/lizard/ai_turn.rs`, `creature/lizard/ai.rs`, `creature/ai_modules/world.rs`, `creature/ai_modules/utility_comparer.rs` |
| Lizard hunting, lounging, fleeing | `creature/lizard/hunt_turn.rs`, `creature/ai_modules/threat_tracker_world.rs`, `creature/ai_modules/heavy_ai_turn.rs` |
| Lizards fighting, signalling, leaving on a mission | `creature/lizard/fight_turn.rs`, `creature/ai_modules/mission_tracker.rs`, `mission_tracker_world.rs` |
| Room tile AI map | `world/ai_map/` |
| Shelters, sleep, hibernation | `game/shelter.rs`, `game/shelter_turn.rs`, `player/sleep.rs` |
| Save-state writing (sim side) | `world/region_state.rs`, `creature/state_save.rs`, `item/mod.rs` (object strings), `creature/overseer/guide_state.rs`, `game/map_discovery.rs`; text format in `rw-save` |
| Death, game over, karma flower | `game/game_over_turn.rs`, `creature/death.rs`, `player/falls.rs` |
| The player bitten and held by a lizard, struggling free | `player/grabbed_turn.rs`, `player/grabbed.rs`, `creature/lizard/hits_turn.rs` (`Bite`, the melee throw's hit) |
| Random draws / sounds | `sound/mod.rs`, `rw-random` crate; draws live beside each ported function |
| Camera visibility | `game/camera.rs` |
| `ElectricDeath` (Underhang), `BorderPushBack` (Sunlit Lands) | `effects/gameplay/electric_death.rs`, `effects/gameplay/electric_death_turn.rs`; `effects/gameplay/border_push_back.rs`, `effects/gameplay/border_push_back_turn.rs`; sprites in `rw-graphics` `effects/gameplay/electric_death.rs`, shaders in `rw-render` |
| Room triggers, guide instructions | `game/trigger.rs`, `game/trigger_turn.rs`, `game/instructions_turn.rs`, `game/room_script.rs` |
| Room realization / loading | `world/realizer.rs`, `world/realized.rs`, `world/room_preparer.rs` (the loading thread), `world/loading_pace.rs`, `world/mod.rs`, `game/loading_turn.rs` |
| Rain cycle | `world/rain_cycle.rs` |
| The room rain (push, stun, rain death) | `world/room_rain.rs`, `game/room_rain_turn.rs`; `Creature.rainDeath` in `creature/mod.rs` / `player/mod.rs`, the lung cap in `player/swim.rs`; D45 |
| The bullet rain (heavy drops before the slam) | `world/bullet_drip.rs`, `game/bullet_drip_turn.rs`; the density is `GlobalRain.bullet_rain_density`; D50 |
| The flood's water (rising level, `AddWater`) | `game/room_rain_turn.rs` (flood arm), `world/room_rain.rs` (`FloodLevel`), `world/realized.rs` (`add_water`), `water/mod.rs` (`f_water_level`); D47 |
| The global rain, the death rain's modes, the rain's shake and sky look | `world/global_rain.rs`, `game/rain_turn.rs`, `game/screen_shake.rs`, `water/mod.rs` (`SurfaceRumble`), `rw-graphics` `player/graphics.rs` (`sky_look`); D44 |
| Eating, picking up, throwing | `player/turn.rs`, `item/turn.rs`, `player/grab.rs`; creatures' meat: `player/eat_meat_turn.rs`; carrying a heavy body: `player/carry.rs`, `player/turn.rs` |
| The Hunter's spear on the back | `player/spear_on_back_turn.rs`, `player/spear_on_back.rs`, `item/on_back_stick.rs` |
| Body physics / collision | `physics/chunk.rs`, `physics/object.rs`, `player/mod.rs` |
| Climbable vines (the player climbing, the vine geometry) | `vines/mod.rs`, `player/vines.rs`; hands: `rw-graphics` `player/hand.rs` |
| Zero gravity (room `gravity`, `ElectricPower`, the broken switch, the player floating) | `effects/gameplay/anti_gravity.rs`, `effects/gameplay/anti_gravity_turn.rs`, `player/zero_g.rs`, `PhysicsParams::room_gravity` in `physics/chunk.rs`; the SS song: `game/trigger_turn.rs` (`SsMusic`) and `rw-audio` `music/song.rs` |
| Overseer / guide | `creature/overseer/` (`turn.rs`: in the game), `creature/overseer/world_ai.rs`, `creature/overseer/holo_light_turn.rs` |
| Placed light beams, spot lights | `effects/lights/light_beam.rs`, `effects/lights/light_beam_turn.rs`; sprites in `rw-graphics` `effects/lights/light_beam.rs` (beams and spot lights), `effects/lights/quad_grid.rs` |
| Superstructure rods, zap coils (and their lights, flashes), fuses | `superstructure/electric/light_rod.rs`, `superstructure/electric/zap_coil.rs`, `superstructure/electric/fuses.rs`, `superstructure/electric/turn.rs`; sprites in `rw-graphics` `superstructure/electric.rs` |
| Coral (`CoralNeuronSystem`, `CoralNeuron`, `ProjectedCircle`, `WallMycelia`, `Mycelium`, `NeuronSpark`) | `superstructure/coral/system.rs`, `superstructure/coral/neuron.rs`, `superstructure/projections/projected_circle.rs`, `superstructure/coral/wall_mycelia.rs`, `superstructure/coral/mycelium.rs`, `superstructure/coral/neuron_spark.rs`, `superstructure/coral/turn.rs`, `superstructure/projections/turn.rs`; sprites in `rw-graphics` `superstructure/coral/neuron.rs`, `superstructure/projections/projected_circle.rs`, `superstructure/coral/mycelium.rs` (strands and wall mycelia), `effects/cosmetic.rs` (neuron sparks) |
| Coral circuits (`CoralCircuit`, `CircuitBit`, `CircuitConnector`, `CircuitCrab`) and the player swimming through them | `superstructure/coral/circuit.rs`, `superstructure/coral/circuit_connector.rs`, `superstructure/coral/circuit_crab.rs`, `superstructure/coral/circuit_turn.rs`, `player/circuit.rs`, `player/zero_g.rs`; sprites in `rw-graphics` `superstructure/coral/circuit.rs`, shaders in `rw-render` `sprite.wgsl` (`fs_coral_circuit`) |
| Superstructure screen-grab effects (`DeepProcessing`, `DeepProcessingLight`, `GravityDisruptor`) | `superstructure/grab/deep_processing.rs`, `superstructure/grab/gravity_disruptor.rs`, `superstructure/grab/turn.rs`; sprites in `rw-graphics` `superstructure/grab.rs`, shaders in `rw-render` `ss_grab.wgsl` |
| Light fixtures (`Redlight`, `HolyFire`, `SlimeMoldLight`), waterfalls | `effects/lights/light_fixture.rs`, `water/waterfall.rs`, `superstructure/electric/room_fixtures.rs`, `effects/lights/light_fixture_turn.rs`, `water/waterfall_turn.rs`; sprites in `rw-graphics` `effects/lights/light_fixture.rs`, `water/waterfall.rs` |
| Lights (placed, player glow, room light queries) | `effects/lights/light_source.rs`, `effects/lights/turn.rs`, `game/camera_image.rs`; sprites in `rw-graphics` `effects/lights/light_source.rs` |
| Bat flies | `creature/fly/`, `creature/fly/world_ai.rs`, `creature/fly/turn.rs` |
| Cosmetic insects (Flies, RockFlea, ... effects, `InsectGroup`) | `effects/particles/insect/`, `effects/particles/insect/turn.rs`; sprites in `rw-graphics` `effects/particles/insect.rs` |
| Dev spawns / teleport | `game/dev_turn.rs`, `game/spawn.rs` |
| Custom decals, rainbows | `effects/particles/decal.rs`, `effects/particles/rainbow.rs`, `effects/particles/decal_turn.rs`; sprites in `rw-graphics`, shaders in `rw-render` |
| Cosmetic slime molds, dead token stalks | `effects/particles/slime_mold.rs`, `effects/particles/token_stalk.rs`, `effects/particles/placed_cosmetics_turn.rs`; sprites in `rw-graphics` |
| Background scenes (`AboveCloudsView`, `RoofTopView`), distant lightning, dust puffs | `effects/sky/background_scene.rs`, `effects/sky/above_clouds_view.rs` (distant lightning), `effects/sky/roof_top_view.rs` (dust puffs), `effects/sky/background_scene_turn.rs`; sprites in `rw-graphics` `effects/sky/background_scene.rs`, shaders in `rw-render` `background_scene.wgsl` |

## Cross-links

Notes (`../../../rainworld-re/notes/`): `physics-core.md`, `slope-collision.md`, `slugcat-movement.md`, `advanced-moves.md`, `falls.md`, `corridors.md`, `vertical-poles.md`, `horizontal-beams.md`, `ledge-grab.md`, `shortcuts.md`, `water.md`, `ai-map.md`, `creature-model/`, `items.md`, `shelter.md`, `saves.md`, `sound.md`, `unity-random.md`, `overseer.md`, `lizard-ai.md`, `room-realizer.md`, `world-format.md`, `world-loading.md`, `room-format.md`, `campaign-model.md`, `music.md`. Surveys (`../../../rainworld-re/surveys/`): `lizards.md`, `bat-flies.md`, `rocks-spears.md`, `spawning.md`, `violence-templates.md`, `items-room-edges.md`, `campaign.md`, `sleep-death-screens.md`, `malnourishment.md`, `food-pip-clip.md`. (`notes/intro-overseer-survey.md` covers the guide and `game/room_script.rs`.)

Divergences: `../../docs/divergences/` (IDs are global, `grep -n '^## D'`). In `sim.md`: D2 spawning, D3 movement, D4 falls, D5 shortcuts, D6 poles/ledges, D9 skid/roll/belly slide, D10 corridors, D11 water, D14 worlds/rooms/realizer (creatures, lizards, flies, overseers), D17 shelters, D19 the player held by a creature; D33 (`sim-gameplay-effects.md`) `ElectricDeath` and `BorderPushBack`. Elsewhere: D1 `random.md`, D12 `audio.md`, D13 `menu.md`, D16 `save.md`, D18 `app.md`.

Tests: `tests/` folders beside their modules (see Layout), and `../tests/` (18 `installed_*.rs` integration tests that read the installed game, plus `data/`). 