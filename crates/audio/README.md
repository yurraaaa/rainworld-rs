# rw-audio (crates/audio)

Sound playback on `kira` (cpal backend; desktop and Android-capable). It plays the
sim's `SoundEvent`s and the room's ambient loops mixed as the game's `VirtualMicrophone`
does, the menus' `MenuMicrophone` sounds, and the `Music` namespace (`MusicPlayer`,
songs, procedural threat layers). Depends on `rw-sim` (events, chunks, water),
`rw-assets` (Unity bundles, `Fsb5`), `rw-random` (`UnityRandom`), `rw-save`,
`rustc-hash`, `kira`. The sim makes the random draws for sounds; this crate only plays.
Silent, tick-timed music sources keep headless runs deterministic.

## File map (`src/`)

| File | Holds |
|---|---|
| `lib.rs` | `Audio`: `new`, `new_room`, `tick`, `play_events`, `set_pause_mix`, `frame`; sound-count limit `allowed` |
| `output.rs` | kira backend on cpal, stops and joins its thread on drop |
| `clips.rs` | `ClipLoader`, `ClipKey`, `ClipSlot`: clips decoded on background threads (`CacheAllAudio`) |
| `mic.rs` | `VirtualMicrophone` formulas: pan, volume, low-pass, the volume groups (the rain's drown-out of group 0) |
| `voice.rs` | sound objects (`DisembodiedSound`, `StaticPositionSound`, `ObjectSound`, `DisembodiedLoop`, `RectangularSound`) and their emitters (a disembodied loop's pan follows its emitter's) |
| `ambient.rs` | `AmbientSoundPlayer`: room `AmbientSounds:` loops |
| `menu_mic.rs` | `MenuMicrophone` playback (`play`, `set_loop`, `stop`) |
| `music/mod.rs` | module root |
| `music/player.rs` | `MusicPlayer`, `MusicContext`, `MusicSettings`; `game_requests_song`, `menu_requests_song`, `update` |
| `music/song.rs` | `MusicPiece`, `SubTrack`, `Song`, `MenuOrSlideShowSong`, `IntroRollMusic` |
| `music/procedural.rs` | `ProceduralMusic`: region threat stems faded by threat |
| `music/threat.rs` | `PlayerThreatTracker`, `ThreatDetermination` (tests in `threat_tests.rs`) |
| `music/bank.rs` | `music_songs` / `music_procedural` bundles: clips by name |
| `music/stream.rs` | streams FSB5 songs through kira (`SongDecoder`, `Fsb5Stream`) |
| `music/sources.rs` | the audio-source trait the player drives, and the silent timed implementation |
| `music/screens.rs` | what menu screens and endings ask of the player |
| `tests/` | `music_player.rs`, `procedural_music.rs`, `game_music.rs`, `installed_music.rs` (installed game) |

## Where is X?

- FSB5 container / Vorbis setup headers: in `rw-assets` (`rw_assets::unity::Fsb5`, `Fsb5Stream`), not here; used by `clips.rs` (`bundle.decode_audio`) and `music/stream.rs`.
- Effect clip loading and caching: `clips.rs`.
- Sim sound events to playback: `lib.rs` `Audio::play_events`.
- Positional pan/volume/low-pass: `mic.rs`.
- Emitters following body chunks (loops): `voice.rs`.
- Rect sounds (the water's surface loop: nearest point of the rect to the listener): `voice.rs` `Voice::mix`.
- Room ambient loops: `ambient.rs`, `Audio::set_ambients` / `new_room`.
- Menu sounds: `menu_mic.rs`.
- Music player and song switching: `music/player.rs`.
- Region threat music: `music/procedural.rs` + `music/threat.rs`.
- Music streaming via kira: `music/stream.rs`; silent mode `music/sources.rs`.
- Kira manager/backend: `lib.rs` `Audio::new`, `output.rs`.
- Pause mix / sfx volume: `lib.rs` `set_pause_mix`, `set_sound_effects_volume`.
- App side of music requests: `../app/src/game_music.rs`.

## Cross-links

- Divergences: `../../docs/divergences/audio.md` (D12 Sound).
- Notes (`../../../rainworld-re/notes/`): `sound.md`, `music.md`, `unity-assets.md`.
- Surveys (`../../../rainworld-re/surveys/`): `music.md`.
