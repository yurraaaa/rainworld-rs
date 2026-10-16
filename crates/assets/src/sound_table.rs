//! `soundeffects/sounds.txt`: which clips a `SoundID` plays and with what volume and
//! pitch ranges. Mirrors `SoundLoader.LoadSounds` with its `SoundTrigger` and
//! `SoundPlayInstruction` option parsing (`rainworld-re/notes/sound.md` section 1.1).
//!
//! Sound IDs match case-insensitively and take the first line naming them; option
//! keys are case sensitive and unknown ones are ignored, so the shipped file's typos
//! (`Vol=`, `rangeFact=`) have no effect. A sample whose clip is in neither the
//! `loadedsoundeffects` bundle nor `soundeffects/` silences its whole ID.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::unity::{AssetBundle, AudioClip, PcmAudio};
use crate::{AssetError, StreamingAssets};

/// The clip names the loader can see: the bundle's asset names (lower case, full path
/// and file stem) and an optional loose-file directory (`soundeffects/`).
#[derive(Debug, Clone, Default)]
pub struct ClipNames {
    names: HashSet<String>,
    loose_dir: Option<PathBuf>,
}

/// One clip name of the table and its variations (`name_1`, `name_2`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundClip {
    /// As written in `sounds.txt`.
    pub name: String,
    /// In the bundle (`audioClipThroughUnity`), else a loose file.
    pub in_bundle: bool,
    pub variations: Vec<ClipVariation>,
}

/// The asset one variation loads, and whether the bundle has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipVariation {
    pub asset: String,
    pub exists: bool,
}

/// One item of a sample list (`SoundPlayInstruction`).
#[derive(Debug, Clone, PartialEq)]
pub struct SoundSample {
    /// Index into [`SoundTable::clips`].
    pub clip: usize,
    pub min_vol: f32,
    pub max_vol: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub ignore_effects: f32,
}

/// A working sound ID (`SoundTrigger`); volumes include the group volume.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundTrigger {
    pub samples: Vec<SoundSample>,
    pub group_volume: f32,
    pub min_vol: f32,
    pub max_vol: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    /// `rangeFac`: divides the distance in the volume falloff.
    pub range: f32,
    pub doppler_fac: f32,
    pub ignore_effects: f32,
    pub play_all: bool,
    pub dont_log: bool,
    /// Probability of not playing; 0 draws nothing.
    pub silent_chance: f32,
    /// Moot: `CacheAllAudio` is hard-coded true.
    pub cache: bool,
}

/// What `sounds.txt` says about one sound ID.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriggerState<'a> {
    /// No line names it.
    NoLine,
    /// Its line names a clip that does not exist: it never plays.
    MissingClip,
    Working(&'a SoundTrigger),
}

/// The parsed `sounds.txt`.
#[derive(Debug, Clone, Default)]
pub struct SoundTable {
    /// `AudioListener.volume` (line 0).
    pub volume: f32,
    /// Exponent of the per-source volume curve (line 1).
    pub volume_exponent: f32,
    /// Lower-case sound ID -> trigger, `None` when a clip is missing.
    triggers: HashMap<String, Option<SoundTrigger>>,
    clips: Vec<SoundClip>,
    /// Decoded clips by variation asset: those whose spectrum the game reads while they
    /// play (`CreatureVoice`'s visualizer), decoded once when the table loads.
    pcm: HashMap<String, Arc<PcmAudio>>,
}

impl ClipNames {
    /// Variation names are probed up to this suffix.
    const MAX_VARIATIONS: usize = 100;

    /// Names of `bundle` (case-insensitive, by full path or file stem) and loose files
    /// in `loose_dir`.
    #[must_use]
    pub fn new(names: impl IntoIterator<Item = String>, loose_dir: Option<PathBuf>) -> Self {
        let mut set = HashSet::new();
        for name in names {
            let lower = name.to_lowercase();
            let file = lower.rsplit('/').next().unwrap_or(&lower);
            set.insert(file.rsplit_once('.').map_or(file, |(s, _)| s).to_owned());
            set.insert(lower);
        }
        Self {
            names: set,
            loose_dir,
        }
    }

    #[must_use]
    pub fn from_bundle(bundle: &AssetBundle, loose_dir: Option<PathBuf>) -> Self {
        Self::new(bundle.asset_paths().map(str::to_owned), loose_dir)
    }

    /// `AssetBundle.Contains`.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(&name.to_lowercase())
    }

    /// `CheckIfFileExistsAsUnityResource`.
    fn in_bundle(&self, name: &str) -> bool {
        self.contains(name) || self.contains(&format!("{name}_1"))
    }

    fn loose(&self, file: &str) -> bool {
        self.loose_dir
            .as_deref()
            .is_some_and(|d| d.join(file.to_lowercase()).is_file())
    }

    /// `CheckIfFileExistsAsExternal`.
    fn external(&self, name: &str) -> bool {
        ["", "_1"]
            .iter()
            .flat_map(|s| [format!("{name}{s}.wav"), format!("{name}{s}.ogg")])
            .any(|f| self.loose(&f))
    }

    /// `VariationsForSound`: probes `name_2`, `name_3`, ... (`.wav` files for loose clips).
    fn variations(&self, name: &str, in_bundle: bool) -> usize {
        let mut n = 1;
        while n < Self::MAX_VARIATIONS + 1 {
            let next = format!("{name}_{}", n + 1);
            let found = if in_bundle {
                self.contains(&next)
            } else {
                self.loose(&format!("{next}.wav"))
            };
            if !found {
                break;
            }
            n += 1;
        }
        n
    }
}

impl SoundTable {
    /// Reads `soundeffects/sounds.txt` and resolves its clips against the
    /// `loadedsoundeffects` bundle.
    ///
    /// # Errors
    /// When the file cannot be read, or holds a line the game's loader would throw on.
    pub fn load(assets: &StreamingAssets, bundle: &AssetBundle) -> Result<Self, AssetError> {
        let path = assets.file("soundeffects/sounds.txt");
        let text = fs::read_to_string(&path).map_err(|e| AssetError::io(&path, e))?;
        let names = ClipNames::from_bundle(bundle, Some(assets.file("soundeffects")));
        let mut table = Self::parse(&text, &names).map_err(|m| AssetError::format(&path, m))?;
        table.decode_visualized(bundle);
        Ok(table)
    }

    /// The trigger prefixes (lower case) of the sounds a `CreatureVoice` visualizes: the
    /// lizard voices, its only subclass's.
    const VISUALIZED: [&'static str; 2] = ["lizard_voice_", "lizard_love_voice_"];

    /// Decodes the bundle clips of the visualized triggers.
    fn decode_visualized(&mut self, bundle: &AssetBundle) {
        let mut assets: Vec<String> = Vec::new();
        for (id, trigger) in &self.triggers {
            let Some(trigger) = trigger else { continue };
            if !Self::VISUALIZED.iter().any(|p| id.starts_with(p)) {
                continue;
            }
            for s in &trigger.samples {
                let clip = &self.clips[s.clip];
                if clip.in_bundle {
                    assets.extend(
                        clip.variations
                            .iter()
                            .filter(|v| v.exists)
                            .map(|v| v.asset.clone()),
                    );
                }
            }
        }
        for asset in assets {
            let decoded = bundle
                .load::<AudioClip>(&asset)
                .and_then(|clip| bundle.decode_audio(&clip));
            match decoded {
                Ok(pcm) => {
                    self.pcm.insert(asset, Arc::new(pcm));
                }
                Err(e) => log::warn!("visualized clip {asset}: {e}"),
            }
        }
    }

    /// A decoded visualized clip by variation asset.
    #[must_use]
    pub fn pcm(&self, asset: &str) -> Option<&Arc<PcmAudio>> {
        self.pcm.get(asset)
    }

    /// Adds a decoded clip (tests build tables without a bundle).
    pub fn insert_pcm(&mut self, asset: &str, pcm: PcmAudio) {
        self.pcm.insert(asset.to_owned(), Arc::new(pcm));
    }

    /// Parses the file text against the clip names.
    ///
    /// # Errors
    /// Where the game's loader throws: a bad number, a missing `=` value, a malformed
    /// volume or group line.
    pub fn parse(text: &str, names: &ClipNames) -> Result<Self, String> {
        let lines: Vec<&str> = text.lines().collect();
        let volume = Self::header_value(&lines, 0)?;
        let volume_exponent = Self::header_value(&lines, 1)?;
        let mut table = Self {
            volume,
            volume_exponent,
            ..Self::default()
        };
        let mut clip_index: HashMap<String, usize> = HashMap::new();
        let mut open_groups: Vec<(String, f32)> = Vec::new();
        for line in &lines {
            let parts: Vec<&str> = line.split(" : ").collect();
            if parts[0] == "START VOLUME GROUP" {
                let name = parts.get(1).ok_or("volume group without name")?;
                let vol = Self::float(parts.get(2).ok_or("volume group without volume")?)?;
                open_groups.push(((*name).to_owned(), vol));
            }
            if parts[0] == "END VOLUME GROUP" {
                let name = parts.get(1).ok_or("volume group end without name")?;
                if let Some(i) = open_groups.iter().position(|(n, _)| n == name) {
                    open_groups.remove(i);
                }
                continue;
            }
            if parts.len() != 2 {
                continue;
            }
            let id_part = parts[0].as_bytes();
            if id_part.is_empty() || (id_part[0] == b'/' && id_part.len() < 2) {
                return Err(format!("line {line:?}: the loader indexes past its start"));
            }
            if id_part.starts_with(b"//") {
                continue;
            }
            let group_volume = open_groups.iter().map(|(_, v)| v).product();
            let id_options: Vec<&str> = parts[0].split('/').collect();
            let id = id_options[0].to_lowercase();
            if table.triggers.contains_key(&id) {
                continue;
            }
            let trigger =
                table.read_trigger(&id_options, parts[1], group_volume, names, &mut clip_index)?;
            table.triggers.insert(id, trigger);
        }
        Ok(table)
    }

    /// Lines 0 and 1: the value after `": "`.
    fn header_value(lines: &[&str], i: usize) -> Result<f32, String> {
        let line = lines.get(i).ok_or("sounds.txt too short")?;
        Self::float(line.split(": ").nth(1).ok_or("volume line without value")?)
    }

    /// One entry line: `None` when a sample's clip is missing.
    fn read_trigger(
        &mut self,
        id_options: &[&str],
        sample_list: &str,
        group_volume: f32,
        names: &ClipNames,
        clip_index: &mut HashMap<String, usize>,
    ) -> Result<Option<SoundTrigger>, String> {
        let list = if sample_list.ends_with(',') {
            format!("{sample_list} ")
        } else {
            sample_list.to_owned()
        };
        let mut samples = Vec::new();
        for item in list.split(", ") {
            let fields: Vec<&str> = item.split('/').collect();
            let mut name = fields[0];
            if name.to_lowercase() == "samename" {
                name = id_options[0];
            }
            let lower = name.to_lowercase();
            let clip = match clip_index.get(&lower) {
                Some(&i) => i,
                None => {
                    let in_bundle = names.in_bundle(name);
                    if !in_bundle && !names.external(name) {
                        return Ok(None);
                    }
                    self.clips.push(Self::clip(name, in_bundle, names));
                    clip_index.insert(lower, self.clips.len() - 1);
                    self.clips.len() - 1
                }
            };
            samples.push(SoundSample::parse(clip, &fields[1..])?);
        }
        SoundTrigger::parse(samples, group_volume, &id_options[1..]).map(Some)
    }

    /// The asset each variation loads (`AddUnityAudioToLoad`, `GetAudioClip`): the bare
    /// name for a single variation, else `name_1`, `name_2`, ...
    fn clip(name: &str, in_bundle: bool, names: &ClipNames) -> SoundClip {
        let count = names.variations(name, in_bundle);
        let variations = (0..count)
            .map(|v| {
                let asset = if count > 1 {
                    format!("{name}_{}", v + 1)
                } else {
                    name.to_owned()
                };
                let exists = !in_bundle || names.contains(&asset);
                ClipVariation { asset, exists }
            })
            .collect();
        SoundClip {
            name: name.to_owned(),
            in_bundle,
            variations,
        }
    }

    /// `float.Parse(s, NumberStyles.Any, InvariantCulture)`: surrounding white space and
    /// thousands separators allowed.
    pub(crate) fn float(value: &str) -> Result<f32, String> {
        value
            .trim()
            .replace(',', "")
            .parse()
            .map_err(|_| format!("invalid number {value:?}"))
    }

    /// The `value` of a `key=value` option.
    fn option_value(fields: &[&str]) -> Result<f32, String> {
        Self::float(
            fields
                .get(1)
                .ok_or_else(|| format!("option {:?} without a value", fields[0]))?,
        )
    }

    /// What the file says about a sound ID (matched case-insensitively).
    #[must_use]
    pub fn state(&self, id: &str) -> TriggerState<'_> {
        match self.triggers.get(&id.to_lowercase()) {
            None => TriggerState::NoLine,
            Some(None) => TriggerState::MissingClip,
            Some(Some(t)) => TriggerState::Working(t),
        }
    }

    /// The trigger of a sound ID that can play (`workingTriggers`).
    #[must_use]
    pub fn trigger(&self, id: &str) -> Option<&SoundTrigger> {
        match self.state(id) {
            TriggerState::Working(t) => Some(t),
            _ => None,
        }
    }

    #[must_use]
    pub fn clips(&self) -> &[SoundClip] {
        &self.clips
    }

    #[must_use]
    pub fn clip_at(&self, index: usize) -> Option<&SoundClip> {
        self.clips.get(index)
    }

    /// Lower-case IDs with a line in the file.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.triggers.keys().map(String::as_str)
    }
}

impl SoundClip {
    /// Whether variation `v` loads a clip (loose clips are assumed to).
    #[must_use]
    pub fn variation_exists(&self, v: usize) -> bool {
        !self.in_bundle || self.variations.get(v).is_some_and(|x| x.exists)
    }
}

impl SoundSample {
    fn parse(clip: usize, options: &[&str]) -> Result<Self, String> {
        let mut s = Self {
            clip,
            min_vol: 1.0,
            max_vol: 1.0,
            min_pitch: 1.0,
            max_pitch: 1.0,
            ignore_effects: 0.0,
        };
        for option in options {
            let kv: Vec<&str> = option.split('=').collect();
            match kv[0] {
                "vol" => {
                    s.min_vol = SoundTable::option_value(&kv)?;
                    s.max_vol = s.min_vol;
                }
                "pitch" => {
                    s.min_pitch = SoundTable::option_value(&kv)?;
                    s.max_pitch = s.min_pitch;
                }
                "minVol" => s.min_vol = SoundTable::option_value(&kv)?,
                "maxVol" => s.max_vol = SoundTable::option_value(&kv)?,
                "minPitch" => s.min_pitch = SoundTable::option_value(&kv)?,
                "maxPitch" => s.max_pitch = SoundTable::option_value(&kv)?,
                "ignoreEffects" => s.ignore_effects = SoundTable::option_value(&kv)?,
                _ => {}
            }
        }
        Ok(s)
    }
}

impl SoundTrigger {
    fn parse(samples: Vec<SoundSample>, group: f32, options: &[&str]) -> Result<Self, String> {
        let mut t = Self {
            samples,
            group_volume: group,
            min_vol: group,
            max_vol: group,
            min_pitch: 1.0,
            max_pitch: 1.0,
            range: 1.0,
            doppler_fac: 1.0,
            ignore_effects: 0.0,
            play_all: false,
            dont_log: false,
            silent_chance: 0.0,
            cache: true,
        };
        for option in options {
            let kv: Vec<&str> = option.split('=').collect();
            match kv[0] {
                "vol" => {
                    t.min_vol = SoundTable::option_value(&kv)? * group;
                    t.max_vol = t.min_vol;
                }
                "pitch" => {
                    t.min_pitch = SoundTable::option_value(&kv)?;
                    t.max_pitch = t.min_pitch;
                }
                "minVol" => t.min_vol = SoundTable::option_value(&kv)? * group,
                "maxVol" => t.max_vol = SoundTable::option_value(&kv)? * group,
                "minPitch" => t.min_pitch = SoundTable::option_value(&kv)?,
                "maxPitch" => t.max_pitch = SoundTable::option_value(&kv)?,
                "ignoreEffects" => t.ignore_effects = SoundTable::option_value(&kv)?,
                "rangeFac" => t.range = SoundTable::option_value(&kv)?,
                "dopplerFac" => t.doppler_fac = SoundTable::option_value(&kv)?,
                "PLAYALL" => t.play_all = true,
                "DONTLOG" => t.dont_log = true,
                "silentChance" => t.silent_chance = SoundTable::option_value(&kv)?,
                _ => {}
            }
        }
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "Volume: 6.0\nVolume exponent: 1.8\n\
        Jump : jump2/vol=0.5, jump2B/minPitch=0.8/maxPitch=1.2\n\
        jump : other\n\
        // Commented : jump2\n\
        //Placeholder\n\
        Hit/rangeFac=0.5/dopplerFac=0/PLAYALL/silentChance=0.25/DONTLOG : hitA/Vol=3/ignoreEffects=1, hitA\n\
        Trailing : hitA,\n\
        Typos/RangeFac=0.3/rangeFact=0.2/minVol=0.4/maxVol=0.6/pitch=1.5 : samename/minPitxh=2\n\
        Dead : jump2, nosuchclip\n\
        Skid : hitA/vol=0.4/Skitter1C/vol=0.5\n\
        START VOLUME GROUP : quiet : 0.5\n\
        Grouped/vol=0.8 : hitA\n\
        END VOLUME GROUP : quiet\n\
        After : hitA\n";

    fn names() -> ClipNames {
        let paths = [
            "assets/loadedsoundeffects/jump2_1.wav",
            "assets/loadedsoundeffects/jump2_2.wav",
            "assets/loadedsoundeffects/jump2_3.wav",
            "assets/loadedsoundeffects/jump2b.wav",
            "assets/loadedsoundeffects/hita.wav",
            "assets/loadedsoundeffects/typos.wav",
        ];
        ClipNames::new(paths.map(str::to_owned), None)
    }

    #[test]
    fn header_and_first_line_per_id() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        assert_eq!((t.volume, t.volume_exponent), (6.0, 1.8));
        let jump = t.trigger("JUMP").unwrap();
        assert_eq!(jump.samples.len(), 2);
        assert_eq!(
            (jump.samples[0].min_vol, jump.samples[0].max_vol),
            (0.5, 0.5)
        );
        let b = &jump.samples[1];
        assert_eq!((b.min_pitch, b.max_pitch, b.min_vol), (0.8, 1.2, 1.0));
        assert_eq!(t.state("Commented"), TriggerState::NoLine);
        assert_eq!(t.state("Placeholder"), TriggerState::NoLine);
    }

    #[test]
    fn variations_probe_the_names() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        let jump = t.trigger("jump").unwrap();
        let v = &t.clips()[jump.samples[0].clip].variations;
        let assets: Vec<&str> = v.iter().map(|v| v.asset.as_str()).collect();
        assert_eq!(assets, ["jump2_1", "jump2_2", "jump2_3"]);
        assert!(v.iter().all(|v| v.exists));
        let single = &t.clips()[jump.samples[1].clip].variations;
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].asset, "jump2B");
    }

    #[test]
    fn trigger_options_and_trailing_comma() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        let hit = t.trigger("hit").unwrap();
        assert_eq!(
            (hit.range, hit.doppler_fac, hit.silent_chance),
            (0.5, 0.0, 0.25)
        );
        assert!(hit.play_all && hit.dont_log && hit.cache);
        assert_eq!(hit.samples.len(), 2);
        // "hitA," gets a space appended, so its last item is an empty clip name.
        assert_eq!(t.state("trailing"), TriggerState::MissingClip);
        // `Vol=` is not `vol`: the sample keeps volume 1; ignoreEffects is read.
        assert_eq!(
            (hit.samples[0].min_vol, hit.samples[0].ignore_effects),
            (1.0, 1.0)
        );
    }

    #[test]
    fn typos_are_ignored_and_samename_resolves() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        let typos = t.trigger("typos").unwrap();
        assert_eq!(typos.range, 1.0);
        assert_eq!((typos.min_vol, typos.max_vol), (0.4, 0.6));
        assert_eq!((typos.min_pitch, typos.max_pitch), (1.5, 1.5));
        assert_eq!(t.clips()[typos.samples[0].clip].name, "Typos");
        assert_eq!(typos.samples[0].min_pitch, 1.0);
    }

    #[test]
    fn a_missing_clip_silences_the_id() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        assert_eq!(t.state("dead"), TriggerState::MissingClip);
        assert!(t.trigger("Dead").is_none());
    }

    #[test]
    fn sample_options_after_a_second_name_still_apply() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        let skid = t.trigger("skid").unwrap();
        assert_eq!(skid.samples.len(), 1);
        assert_eq!(skid.samples[0].min_vol, 0.5);
    }

    #[test]
    fn volume_groups_scale_the_trigger_volume() {
        let t = SoundTable::parse(TEXT, &names()).unwrap();
        let g = t.trigger("grouped").unwrap();
        assert_eq!(g.group_volume, 0.5);
        assert_eq!((g.min_vol, g.max_vol), (0.8 * 0.5, 0.8 * 0.5));
        assert_eq!(t.trigger("after").unwrap().min_vol, 1.0);
    }

    #[test]
    fn bad_numbers_fail_like_the_loader() {
        assert!(SoundTable::parse("Volume: x\nVolume exponent: 1\n", &names()).is_err());
        let text = "Volume: 1\nVolume exponent: 1\nA/vol : hitA\n";
        assert!(SoundTable::parse(text, &names()).is_err());
    }
}
