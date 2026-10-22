# Divergences: rw-audio

See [README](README.md) for the format. IDs are global; code and notes cite them.

## D12: Sound

`rw_sim::Sounds` is the `Room.PlaySound` layer (one-shots disembodied, at a position or
on a body chunk; chunk loops with handles), `rw_assets::SoundTable` the `SoundLoader`,
`rw_assets::unity::Fsb5` the clip decoder, and `rw_audio` the `VirtualMicrophone` with
its `AmbientSoundPlayer`s, playing on kira (`rainworld-re/notes/sound.md`). Every base-game
`Player` sound, the shortcut tick, the water-surface splashes, the water's calm surface
loop (a `RectangularSound`: heard from the point of its rect nearest the listener) and the bat flies' sounds
(their wings, idle and afraid calls, the hover loop, burrowing, emerging, chaining and
the NPC shortcut sounds) are ported. The menus'
`MenuMicrophone` is `rw_menu::MenuMic` (what plays, the draws, the loops' fades) and
`rw_audio::MenuPlayback` (the sources), with the options' sound-effects volume (D13). `rw_audio::music` is the `MusicPlayer` side process with `Song`, `MenuOrSlideShowSong`
and `IntroRollMusic` (`music.md` section 4.1), streaming `music_songs` clips; the app
runs it as the game's side process (D13), silent on `TimedSources` without audio
(`--mute`, `--screenshot`, MCP). In a game, the sim queues the music player's requests
(`rw_sim::trigger::MusicRequest`: a room trigger's `MusicEvent` or `StopMusicEvent`,
`RainCycle`'s `RainRequestStopSong`), the app hands them to `GameRequestsSong` /
`GameRequestsSongStop` / `RainRequestStopSong` with the save state's `songsPlayRecords`
(D16), and the `PlayerThreatTracker`'s room switches, drone level and
`Random.Range(80, 400)` delays run from the player's room (`rw_audio::music::GameMusicView`).
`NewRegion` builds the region's `ProceduralMusic` (`rw_audio::music::procedural`: the
`music/procedural/<bank>.txt` parse with its `{D}`/`{N}`/`{M}`/`{Sub}` prefixes and
`ONELAYERGROUP`, `Reset`'s stem choice, the layer volumes by threat, the re-scramble on
silence, subregion and day/night, the `nextProcedural` fade), whose stems stream from
`music_procedural` and play looped from the same tick, at volume 0 while the threat is
0. It reads the threat tracker's `currentThreat` (0 until `ThreatDetermination` is
ported). `NewCycleEvent` is sent when a story game starts (D17), `DeathEvent` on death
and on a starve, `FadeOutAllSongs(30)` when the dream stand-in starts (the `DreamScreen`
constructor) and `FadeOutAllSongs(20)` for the Hunter out of cycles
(`GoToRedsGameOver`, D17). The sleep, death and starve screens ask nothing of the music
player in the game either. The credits' song (`EndCredits`: `desiredCreditsSong`, 400
updates in) and the passage screen's `Passages` song are ported as hooks
(`CreditsMusic`, `MusicPlayer::custom_end_game_screen`) for screens not ported yet.

**What differs:**
- **Threat.** `ThreatDetermination.Update` is ported with `ThreatOfCreature` over the
  realized creatures of the player's room (`rw_sim::creature::CreatureDanger`: the
  templates' `dangerousToPlayer`, `DangerPos`, the AI's `CurrentPlayerAggression` and
  its tracker's view of the player: a green lizard's tracker, lizard step 6a; the
  overseer's AI has none and a bat fly no AI, so theirs read false and 0 as in the
  game). The `VisualContact` getter is read without its side effect (it clears the
  field when the creature has left the AI's room; the tracker's own update clears it
  the same tick). Ripple layers (Watcher) are not read; any ghost
  mode counts as 1 (no world ghost is ported).
- **Music not ported:** the echoes' `GhostSong` and ghost mode
  (`ResumeProcedural` is there, unused), `SSSong`, gate and region-change songs (no
  gates yet), `VoidSeaMusic`, the ending songs, the arena `MultiplayerDJ` (its shuffle
  draws), the DLC songs and their branches (rot presence, the Watcher's spinning tops,
  Challenge threat music, the MSC credits songs), `ResetSongStream`, MMF stream resync,
  and loose mod `.ogg` overrides. Mushroom mode is 0 (no mushrooms), so `{M}` stems stay
  silent. The ported in-game paths draw what the game draws (D1).
- **No pitch trim on the threat layers.** The game nudges each stem's pitch by up to
  0.8% to keep it in phase with the first (`source.time`); the port starts every stem in
  the same tick on kira's one clock and leaves them at pitch 1 (on `TimedSources` the
  positions are ticks and never drift). No draws are involved.
- **Music timing.** A sub-track's clip loads on a background thread and becomes
  ready on the first update after it arrived (the game's `AssetBundle` load and
  `LoadAudioData` take a frame or more, so this is at least as fast); headless runs use
  `TimedSources`, where clips load at once and play for their FSB5 length in ticks.
  `IntroRollMusic.StartMusic` takes effect in the music player's next update (in the
  game it starts the source at once; the player runs right after the main process in
  the same tick, so nothing reads the difference). A missing `music_songs` bundle still
  sets `assetBundlesLoaded` (the game would wait forever) and its songs stay silent.
- **Fly one-shots stay where they started.** The game attaches a fly's one-shots to its
  body chunk (`PlaySound(id, chunk)`), so they move with it; the port plays them at the
  chunk's position on the tick they start (the hover loop does follow its fly). The
  draws are the game's; only what is heard differs.
- **Item sounds.** A spear sticking into the ground plays `Spear_Stick_In_Ground`; a
  skidding rock runs `Rock_Skidding_On_Ground_LOOP` as a `ChunkDynamicSoundLoop` (its
  loop is made only in a room a camera shows, as the game's `DynamicSoundLoop` makes
  one only with a microphone in the room). The `Through_Air` loops are commented out
  of the game's `sounds.txt` and never play. Throws play `Slugcat_Throw_Rock` /
  `Slugcat_Throw_Spear`, wall hits `Rock_Hit_Wall` / `Spear_Bounce_Off_Wall`, sticking
  `Spear_Stick_In_Wall`, skims `Weapon_Skid`, the backward belly-slide throw
  `Slugcat_Rocket_Jump`, the long slide's end `Slugcat_Normal_Jump`. A weapon stopped
  by a creature plays `Spear_Hit_Small_Creature`, a dying overseer `Overseer_Death`.
- **Not ported:** the rain and thunder loops and one-shots and the
  drown-out of volume group 0, other creatures', item and object sounds, the sounds of
  unported menu screens and objects, `PlayCustomSound` / loose mod clips next to
  `sounds.txt` (a name found only there counts
  as present for the draws but is not decoded), the Watcher ripple ambience and DLC
  sound IDs (a lizard voice's Remix `Love` call and the rot voices among them). Deafness,
  fade to black and `globalSoundMuffle` stay 0; time speed is 1.
- **The lizard voice's visualization is computed in the sim.** `LizardVoice` (with
  `CreatureVoice`) is ported whole: its seeded articulations, `MakeSound`, the volume
  and pitch it gives its `Lizard_Voice_*` one-shot on the head chunk each tick (a
  `requireActiveUpkeep` emitter in the room's update list, replayed while the emotion
  lasts once the clip has played out or the emitter died), and the sound let go while
  the jaws hold something. The game's `IntensityVisualizer` reads
  `AudioSource.GetSpectrumData` (64 bins, Blackman-Harris) of the playing sound, which
  Unity's audio thread fills, so in the game it depends on real-time audio. The port
  computes it in the sim from the clip, decoded once when the sound table loads
  (`SoundTable::pcm`, the `Lizard_Voice_*` and `Lizard_Love_Voice_*` clips): from the
  tick after the sound is made, at the position the ticks have played it to (`sample
  rate * pitch / 40` per tick), a 128-point FFT of the last 128 mixer-rate samples,
  the mean of the 64 bins through the game's `Pow(InverseLerp(0.0002, 0.003, mean),
  0.5)`, times the voice's volume. INFERRED, not read from Unity: the 48 kHz mixer
  rate, the window's periodic form, the `2 / N` magnitude scale, that the spectrum is
  taken before the source's volume, and the frame alignment (the game's spectrum is of
  whatever the audio thread mixed last). It is 0 when the camera is not in the voice's
  room (no sound object), as in the game, but the crowd cap (above) does not silence it.
  Its draws (the body shake and voice bubbles) follow from it, so they are the port's
  and not the game's real-time ones.
- **`AllowSound`'s crowd cap is applied by the audio, after the sim made the draws.** In
  the game a refused sound makes no draws after `silentChance`'s; the cap depends on how
  many sounds the audio thread still plays, which the tick-deterministic sim does not
  know. With audio off nothing is capped. PLAYALL triggers are capped per sample, not
  once per call.
- **Music volume and options.** The options' music volume (D13), read every frame;
  `playMusic` is on (`setup.txt` is not read).
- **Voices.** Unity mixes 32 real voices chosen by priority (all 128) and audibility; the
  port silences the quietest past 32 each frame. There is no `AudioSource` start
  latency, and a sound plays from the frame its clip finishes decoding (the game's
  `loadOp`; decoding is on two background threads, cached for the session).
- **Mixing.** Pan uses kira's constant-power law, not Unity's `panStereo` curve; the
  `AudioLowPassFilter` (22 kHz to 1.5 kHz by the root of the effect, Q 1) is kira's
  state-variable filter at the same cutoff and Q; the master gain of 6 (`AudioListener.volume`)
  clips in the output device as in Unity, with no limiter.
- **Doppler** samples every 0.05 s of wall-clock time since the audio started, as the
  game samples `Time.time`; neither is tick-deterministic.
- **Listener.** `RoomCamera.pos` is the window origin the room camera draws this tick
  (whole pixels); without a level graphic it is the room origin.
- **Static loops on return.** A `StaticSoundLoop` whose room the camera re-enters
  restarts on the first tick back; the game's dead emitter takes up to about 3 ticks
  (`SoundEmitter.Update`) before the loop remakes it.

**Why.** Out of the base-game player scope (in-game music, rain, creatures, UI), or tied to
real-time audio rather than the sim.

**Affects.** What is heard, never the sim (its draws do not depend on the audio).
