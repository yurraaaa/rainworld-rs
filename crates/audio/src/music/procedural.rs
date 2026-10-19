//! `Music.ProceduralMusic`: a region's threat layers, one stem of
//! `music_procedural` per layer, faded in by the threat tracker's threat. Behaviour:
//! `rainworld-re/notes/music.md` section 2.
//!
//! Mirrors: Music/ProceduralMusic.cs

use std::fmt::Debug;

use rw_assets::StreamingAssets;
use rw_random::UnityRandom;
use rw_sim::FloatExt;

use super::bank::{MusicBundle, SongBank};
use super::song::{MusicPiece, PieceInput, SubTrack};
use super::sources::{MusicSources, TrackIds};

/// Where `ProceduralMusicInstruction` reads its bank.
pub trait ProceduralFiles: Debug + Send + Sync {
    /// The lines of `Music/Procedural/<bank>.txt` as `ResolveFilePath` finds it; `None`
    /// without the file.
    fn instruction_text(&self, bank: &str) -> Option<String>;
    /// `music_procedural`'s `AssetBundle.Contains(stem)`.
    fn contains_stem(&self, stem: &str) -> bool;
}

/// The installed game's bank files and `music_procedural` bundle.
#[derive(Debug)]
pub struct InstalledProcedural {
    assets: StreamingAssets,
    bundle: Option<SongBank>,
}

impl InstalledProcedural {
    /// A missing bundle keeps no stem, so every bank is silent and draws nothing.
    #[must_use]
    pub fn open(assets: &StreamingAssets) -> Self {
        let bundle = SongBank::open_bundle(assets, MusicBundle::Procedural)
            .inspect_err(|e| log::warn!("no procedural music bundle ({e})"))
            .ok();
        Self {
            assets: assets.clone(),
            bundle,
        }
    }
}

impl ProceduralFiles for InstalledProcedural {
    fn instruction_text(&self, bank: &str) -> Option<String> {
        let path = self
            .assets
            .resolve_file_path(&format!("Music/Procedural/{bank}.txt"));
        std::fs::read_to_string(path).ok()
    }

    fn contains_stem(&self, stem: &str) -> bool {
        self.bundle.as_ref().is_some_and(|b| b.contains(stem))
    }
}

/// A track's `dayNight`, and the piece's `lastValidDaynight`: 0 (any / not yet read),
/// 1 day, 2 night.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DayNight {
    #[default]
    Any,
    Day,
    Night,
}

/// `ProceduralMusicInstruction.Track`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProceduralTrack {
    pub name: String,
    pub tags: Vec<String>,
    /// The `<PA>` tag; nothing reads it.
    pub remain_in_panic_mode: bool,
    /// A `{Sub|Sub}` prefix's subregions.
    pub sub_regions: Option<Vec<String>>,
    pub day_night: DayNight,
    pub mushroom: bool,
}

impl ProceduralTrack {
    fn allowed_in_sub_region(&self, sub_region: Option<&str>) -> bool {
        self.sub_regions
            .as_ref()
            .is_none_or(|subs| subs.iter().any(|s| Some(s.as_str()) == sub_region))
    }

    fn allowed_in_day_night(&self, day_night: DayNight) -> bool {
        self.day_night == DayNight::Any || self.day_night == day_night
    }
}

/// `ProceduralMusicInstruction`: the bank's tracks and its layers, each a list of
/// indices into the tracks (layers share the track objects, so a prefix in a later
/// layer changes the track for every layer).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProceduralInstruction {
    pub name: String,
    pub tracks: Vec<ProceduralTrack>,
    pub layers: Vec<Vec<usize>>,
    pub use_one_layer_group: bool,
}

impl ProceduralInstruction {
    /// Reads the bank `name`: track lines `<stem> : <tags>` (kept when the bundle has
    /// the stem), `Layer : <stem>, <stem>` lines of tracks defined above, and
    /// `ONELAYERGROUP`.
    #[must_use]
    pub fn load(name: &str, files: &dyn ProceduralFiles) -> Self {
        let mut instruction = Self {
            name: name.to_owned(),
            ..Self::default()
        };
        let Some(text) = files.instruction_text(name) else {
            return instruction;
        };
        for line in text.lines() {
            let parts: Vec<&str> = line.split(" : ").collect();
            if line.trim().to_uppercase() == "ONELAYERGROUP" {
                instruction.use_one_layer_group = true;
            } else if parts[0] == "Layer" {
                instruction.add_layer(parts.get(1).copied().unwrap_or(""));
            } else if !parts[0].is_empty() && files.contains_stem(parts[0]) {
                instruction.add_track(parts[0], parts.get(1).copied().unwrap_or(""));
            }
        }
        instruction
    }

    /// `Custom.ValidateSpacedDelimiter(list, ",")` split on `", "`, empty items dropped.
    fn items(list: &str) -> Vec<String> {
        let mut list = list.to_owned();
        if list.ends_with(',') {
            list.push(' ');
        }
        list.split(", ")
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn add_track(&mut self, name: &str, tags: &str) {
        let mut track = ProceduralTrack {
            name: name.to_owned(),
            ..ProceduralTrack::default()
        };
        for tag in Self::items(tags) {
            if tag == "<PA>" {
                track.remain_in_panic_mode = true;
            } else {
                track.tags.push(tag);
            }
        }
        self.tracks.push(track);
    }

    /// A layer of the named tracks; `{D}`, `{N}`, `{M}` or `{Sub|Sub}` before a name
    /// set the track's day, night, mushroom or subregion filter (none clears them).
    fn add_layer(&mut self, names: &str) {
        let mut layer = Vec::new();
        for item in Self::items(names) {
            let (prefix, name) = match item.find('}') {
                Some(end) if item.len() > 3 && item.starts_with('{') => {
                    (&item[1..end], &item[end + 1..])
                }
                _ => ("", item.as_str()),
            };
            let Some(k) = self.tracks.iter().position(|t| t.name == name) else {
                continue;
            };
            let track = &mut self.tracks[k];
            track.sub_regions = None;
            track.day_night = DayNight::Any;
            track.mushroom = false;
            match prefix {
                "" => {}
                "D" => track.day_night = DayNight::Day,
                "N" => track.day_night = DayNight::Night,
                "M" => track.mushroom = true,
                subs => track.sub_regions = Some(subs.split('|').map(str::to_owned).collect()),
            }
            layer.push(k);
        }
        self.layers.push(layer);
    }
}

/// Where the piece reads the player's subregion: camera 0's HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudSubregion {
    /// No HUD or no text prompt.
    None,
    /// A text prompt without a subregion tracker: the camera room's subregion is
    /// searched instead.
    NoTracker,
    /// The tracker's `lastShownRegion`.
    Tracker { last_shown_region: usize },
}

/// What `ProceduralMusic` reads of a running `RainWorldGame`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProceduralView {
    /// `world.region.subRegions` (index 0 the unnamed one); `None` without a region.
    pub sub_regions: Option<Vec<String>>,
    /// `cameras[0].room != null`.
    pub camera_room: bool,
    /// The camera room's `subregionName`.
    pub camera_room_subregion: Option<String>,
    pub hud: HudSubregion,
    /// `rainCycle.AmountLeft < 0`.
    pub night: bool,
    /// `cameras[0].mushroomMode`.
    pub mushroom_mode: f32,
}

impl ProceduralView {
    /// The index of the camera room's subregion in the region's list (from 1).
    fn camera_subregion_index(&self) -> Option<usize> {
        let regions = self.sub_regions.as_ref()?;
        let name = self.camera_room_subregion.as_deref()?;
        (1..regions.len()).find(|&j| regions[j] == name)
    }

    fn sub_region(&self, index: usize) -> Option<String> {
        let regions = self.sub_regions.as_ref()?;
        (index >= 1 && index < regions.len()).then(|| regions[index].clone())
    }
}

/// What a procedural update reads from the music player.
pub(crate) struct ProceduralInput<'a> {
    pub piece: PieceInput<'a>,
    /// `musicPlayer.threatTracker.currentThreat`, `None` without a tracker.
    pub threat: Option<f32>,
    pub drone_goal_mix: f32,
    /// `None` outside a `RainWorldGame`.
    pub view: Option<&'a ProceduralView>,
}

/// `Music.ProceduralMusic`.
#[derive(Debug, Clone)]
pub struct ProceduralMusic {
    /// The bank's name.
    pub name: String,
    pub instruction: ProceduralInstruction,
    pub piece: MusicPiece,
    pub audible_counter: i32,
    pub silent_counter: i32,
    pub re_scramble_on_next_silence: bool,
    pub remain_in_panic_mode: Vec<bool>,
    pub mushroom_sub_track: Vec<bool>,
    pub last_valid_subregion: Option<String>,
    pub last_valid_daynight: DayNight,
}

impl ProceduralMusic {
    /// `new ProceduralMusic(musicPlayer, name)`: reads the bank and picks the stems
    /// (`Reset`'s draws).
    pub(crate) fn new(
        name: &str,
        files: Option<&dyn ProceduralFiles>,
        view: Option<&ProceduralView>,
        rng: &mut UnityRandom,
        ids: &mut TrackIds,
    ) -> Self {
        let instruction = files.map_or_else(
            || ProceduralInstruction {
                name: name.to_owned(),
                ..ProceduralInstruction::default()
            },
            |f| ProceduralInstruction::load(name, f),
        );
        let mut music = Self {
            name: name.to_owned(),
            instruction,
            piece: MusicPiece::new(MusicBundle::Procedural),
            audible_counter: 0,
            silent_counter: 0,
            re_scramble_on_next_silence: false,
            remain_in_panic_mode: Vec::new(),
            mushroom_sub_track: Vec::new(),
            last_valid_subregion: None,
            last_valid_daynight: DayNight::Any,
        };
        music.scramble(view, rng, ids);
        music.piece.volume = 0.0;
        music
    }

    /// `BuildThreatLayerPool`: the layer's tracks not used yet, filtered by subregion,
    /// time of day and mushroom mode in a game.
    fn threat_layer_pool(
        &mut self,
        tracks: &[usize],
        used: &[usize],
        view: Option<&ProceduralView>,
    ) -> Vec<usize> {
        let mut pool = Vec::new();
        for &k in tracks {
            if used.contains(&k) {
                continue;
            }
            let Some(v) = view else {
                pool.push(k);
                continue;
            };
            let track = &self.instruction.tracks[k];
            if v.sub_regions.is_some() && v.camera_room {
                let index = match v.hud {
                    HudSubregion::Tracker { last_shown_region } => Some(last_shown_region),
                    HudSubregion::None | HudSubregion::NoTracker => {
                        let found = v.camera_subregion_index();
                        if found.is_some() {
                            self.re_scramble_on_next_silence = true;
                        }
                        found
                    }
                };
                let sub_region = index.and_then(|i| v.sub_region(i));
                if track.allowed_in_sub_region(sub_region.as_deref())
                    && track.allowed_in_day_night(self.last_valid_daynight)
                    && !track.mushroom
                {
                    pool.push(k);
                    self.last_valid_subregion = sub_region;
                }
            } else {
                pool.push(k);
            }
            if track.mushroom {
                pool.push(k);
            }
        }
        pool
    }

    /// `Reset`'s choice: one `Random.Range` per layer with a non-empty pool (with
    /// `ONELAYERGROUP`, one for the layer and its whole pool plays). Makes new
    /// sub-tracks and zeroes the piece's volume.
    fn scramble(
        &mut self,
        view: Option<&ProceduralView>,
        rng: &mut UnityRandom,
        ids: &mut TrackIds,
    ) {
        self.piece.sub_tracks.clear();
        self.remain_in_panic_mode.clear();
        self.mushroom_sub_track.clear();
        let mut used = Vec::new();
        let mut picks = Vec::new();
        if self.instruction.use_one_layer_group {
            let layer = rng.range_i32(0, self.instruction.layers.len() as i32);
            let tracks = self
                .instruction
                .layers
                .get(layer as usize)
                .cloned()
                .unwrap_or_default();
            for (j, k) in self
                .threat_layer_pool(&tracks, &used, view)
                .into_iter()
                .enumerate()
            {
                picks.push((j, k));
            }
        } else {
            for layer in 0..self.instruction.layers.len() {
                let tracks = self.instruction.layers[layer].clone();
                let pool = self.threat_layer_pool(&tracks, &used, view);
                if pool.is_empty() {
                    continue;
                }
                let k = pool[rng.range_i32(0, pool.len() as i32) as usize];
                used.push(k);
                picks.push((layer, k));
            }
        }
        for (index, k) in picks {
            let track = &self.instruction.tracks[k];
            log::info!("L{} - {}", self.piece.sub_tracks.len(), track.name);
            self.piece
                .sub_tracks
                .push(SubTrack::new(index, &track.name, ids.next_id()));
            self.piece.volume = 0.0;
            self.remain_in_panic_mode.push(track.remain_in_panic_mode);
            self.mushroom_sub_track.push(track.mushroom);
        }
        self.piece.started_playing = false;
        self.piece.looped = true;
        self.piece.play_when_ready = true;
    }

    /// `Reset`: the old sub-tracks stop, new stems are picked.
    fn reset(
        &mut self,
        view: Option<&ProceduralView>,
        sources: &mut dyn MusicSources,
        rng: &mut UnityRandom,
        ids: &mut TrackIds,
    ) {
        log::info!("SCRAMBLING PROC. MUSIC");
        self.piece.stop_and_destroy(sources);
        self.scramble(view, rng, ids);
    }

    /// `ProceduralMusic.Update`: the layers' volumes by threat (each layer reaches full
    /// volume halfway through its band of the threat), the piece's volume toward
    /// `droneGoalMix`, and the re-scramble after silences and on subregion and
    /// day/night changes.
    pub(crate) fn update(
        &mut self,
        input: &ProceduralInput,
        sources: &mut dyn MusicSources,
        rng: &mut UnityRandom,
        ids: &mut TrackIds,
    ) {
        self.piece.update(&input.piece, sources);
        let mushroom = input.view.map_or(0.0, |v| v.mushroom_mode);
        let threat = input.threat.unwrap_or(0.0);
        let threat = if threat < 0.05 {
            0.0
        } else {
            (threat - 0.05) / 0.95
        };
        let n = self.piece.sub_tracks.len();
        let mut loudest = 0.0f32;
        for i in 0..n {
            let track = &mut self.piece.sub_tracks[i];
            track.volume = if input.threat.is_none() {
                (track.volume - 0.025).max(0.0)
            } else if !self.mushroom_sub_track[i] {
                let band = threat.inverse_lerp(i as f32 / n as f32, (i + 1) as f32 / n as f32);
                band.inverse_lerp(0.0, 0.5)
            } else if mushroom > 0.0 {
                mushroom
            } else {
                (track.volume - 0.025).max(0.0)
            };
            loudest = loudest.max(track.volume);
        }
        self.piece.volume =
            self.piece
                .volume
                .lerp_and_tick(input.drone_goal_mix, 0.0003, 1.0 / 420.0);
        loudest *= self.piece.volume;
        if loudest > 0.0 {
            self.audible_counter += 1;
            self.silent_counter = 0;
            if self.audible_counter == 150 {
                self.re_scramble_on_next_silence = true;
            }
            return;
        }
        self.audible_counter = 0;
        self.silent_counter += 1;
        if self.silent_counter == 150 && self.re_scramble_on_next_silence {
            self.reset(input.view, sources, rng, ids);
            self.re_scramble_on_next_silence = false;
        }
        if self.silent_counter % 150 != 0 {
            return;
        }
        self.check_surroundings(input.view);
    }

    /// Every 150 silent updates: a new subregion or a change between day and night
    /// re-scrambles 50 updates later; outside a game the piece is muted.
    fn check_surroundings(&mut self, view: Option<&ProceduralView>) {
        let Some(view) = view else {
            self.audible_counter = 0;
            self.silent_counter = 150;
            self.re_scramble_on_next_silence = false;
            self.piece.volume = 0.0;
            return;
        };
        let index = match view.hud {
            HudSubregion::None => 0,
            HudSubregion::NoTracker => {
                let found = view
                    .sub_regions
                    .as_ref()
                    .and_then(|_| view.camera_subregion_index());
                if found.is_some() {
                    self.re_scramble_on_next_silence = true;
                }
                found.unwrap_or(0)
            }
            HudSubregion::Tracker { last_shown_region } => last_shown_region,
        };
        let sub_region = view.sub_region(index);
        if index != 0 && self.last_valid_subregion != sub_region {
            self.silent_counter = 100;
            self.re_scramble_on_next_silence = true;
            self.last_valid_subregion = sub_region;
            log::info!("Reactivate scrambling, new sub region!");
        }
        let now = if view.night {
            DayNight::Night
        } else {
            DayNight::Day
        };
        if self.last_valid_daynight != now {
            self.re_scramble_on_next_silence = true;
            self.silent_counter = 100;
            self.last_valid_daynight = now;
            log::info!("Procedural music day/night trigger: {now:?}");
        }
    }

    /// `StopAndDestroy`.
    pub(crate) fn stop_and_destroy(&mut self, sources: &mut dyn MusicSources) {
        self.piece.stop_and_destroy(sources);
    }
}
