//! `Options`: the settings in the game's own format. `ToString` is the `options` key of
//! the `options` file (a `UserData` container, like the slots), `ToStringNonSynced` is
//! `localoptions.txt`; entries split by `<optA>`, fields by `<optB>`. The port keeps
//! both in its own save directory and never writes the game's. Entries the game does
//! not recognise with Remix and the DLCs off are kept as read and written back last,
//! as the game does (`unrecognizedSaveStrings`).

use std::path::Path;

use crate::cs_float::CsFloat;
use crate::progression::SaveError;
use crate::user_data::UserDataFile;

/// `InGameTranslator.LanguageID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    French,
    Italian,
    German,
    Spanish,
    Portuguese,
    Japanese,
    Korean,
    Russian,
    Chinese,
    TraditionalChinese,
    Thai,
}

impl Language {
    pub const ALL: [Self; 12] = [
        Self::English,
        Self::French,
        Self::Italian,
        Self::German,
        Self::Spanish,
        Self::Portuguese,
        Self::Japanese,
        Self::Korean,
        Self::Russian,
        Self::Chinese,
        Self::TraditionalChinese,
        Self::Thai,
    ];

    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "French",
            Self::Italian => "Italian",
            Self::German => "German",
            Self::Spanish => "Spanish",
            Self::Portuguese => "Portuguese",
            Self::Japanese => "Japanese",
            Self::Korean => "Korean",
            Self::Russian => "Russian",
            Self::Chinese => "Chinese",
            Self::TraditionalChinese => "TraditionalChinese",
            Self::Thai => "Thai",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.value() == value)
    }

    /// `BackwardsCompatibilityRemix.ParseLanguage`: the old numeric ids.
    #[must_use]
    pub fn from_legacy_index(index: i32) -> Self {
        usize::try_from(index)
            .ok()
            .filter(|&i| i < 8)
            .map_or(Self::English, |i| Self::ALL[i])
    }
}

/// `Options` with Remix, More Slugcats and Jolly Co-op off.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// An index into `screenResolutions`.
    pub resolution: i32,
    pub windowed: bool,
    /// `controls[i].ToString()`, kept as read (`ControlSetup` is not ported).
    pub controls: [String; 4],
    pub sound_effects_volume: f32,
    pub music_volume: f32,
    pub arena_music_volume: f32,
    pub time_since_last_save_copy: i32,
    pub language: Language,
    pub save_slot: i32,
    pub all_game_pads: bool,
    pub player_to_set_input_for: i32,
    pub vibration: bool,
    pub enabled_mods: Vec<String>,
    pub mod_load_order: Vec<(String, i32)>,
    pub mod_checksums: Vec<(String, String)>,
    pub validation: bool,
    pub remix_tutorial_shown: bool,
    pub dlc_tutorial_shown: bool,
    pub last_game_version: Option<String>,
    unrecognized: Vec<String>,
    unrecognized_non_synced: Vec<String>,
}

impl Default for Options {
    /// The constructor's values off a Steam Deck. The controls are what a new
    /// `ControlSetup(i, i == 0)` writes with no mouse mappings (inferred).
    fn default() -> Self {
        Self {
            resolution: 1,
            windowed: false,
            controls: [
                "ANY<ctrlA>0<ctrlA><ctrlA>0<ctrlA>0<ctrlA>".to_owned(),
                "SPECIFIC_GAMEPAD<ctrlA>0<ctrlA><ctrlA>0<ctrlA>0<ctrlA>".to_owned(),
                "SPECIFIC_GAMEPAD<ctrlA>1<ctrlA><ctrlA>0<ctrlA>0<ctrlA>".to_owned(),
                "SPECIFIC_GAMEPAD<ctrlA>2<ctrlA><ctrlA>0<ctrlA>0<ctrlA>".to_owned(),
            ],
            sound_effects_volume: 0.8,
            music_volume: 0.8,
            arena_music_volume: 0.625,
            time_since_last_save_copy: 0,
            language: Language::English,
            save_slot: 0,
            all_game_pads: false,
            player_to_set_input_for: 0,
            vibration: true,
            enabled_mods: Vec::new(),
            mod_load_order: Vec::new(),
            mod_checksums: Vec::new(),
            validation: false,
            remix_tutorial_shown: false,
            dlc_tutorial_shown: false,
            last_game_version: None,
            unrecognized: Vec::new(),
            unrecognized_non_synced: Vec::new(),
        }
    }
}

impl Options {
    /// The `options` file's name and its key.
    pub const FILE: &'static str = "options";
    pub const LOCAL_FILE: &'static str = "localoptions.txt";
    /// `screenResolutions`.
    pub const SCREEN_RESOLUTIONS: [[f32; 2]; 5] = [
        [1024.0, 768.0],
        [1366.0, 768.0],
        [1360.0, 768.0],
        [1280.0, 768.0],
        [1229.0, 768.0],
    ];
    /// `aspectRatioStrings`.
    pub const ASPECT_RATIOS: [&'static str; 5] = ["4:3", "16:9", "16:9", "5:3", "16:10"];
    const VERSION: &'static str = "v1.11.8";

    /// `fullScreen`.
    #[must_use]
    pub fn full_screen(&self) -> bool {
        !self.windowed
    }

    /// `ScreenSize` (an index out of range, which throws in the game, reads the default).
    #[must_use]
    pub fn screen_size(&self) -> [f32; 2] {
        usize::try_from(self.resolution)
            .ok()
            .and_then(|i| Self::SCREEN_RESOLUTIONS.get(i))
            .copied()
            .unwrap_or(Self::SCREEN_RESOLUTIONS[1])
    }

    /// `IsSteamDeckResolution`.
    #[must_use]
    pub fn is_steam_deck_resolution(size: [f32; 2]) -> bool {
        size == [1229.0, 768.0]
    }

    /// `ToString` (the synced `options` entry).
    #[must_use]
    pub fn to_options_string(&self) -> String {
        let mut s = self.screen_and_controls();
        s += &format!(
            "TimeSinceLastSaveCopy<optB>{}<optA>",
            self.time_since_last_save_copy
        );
        s += &format!("Language<optB>{}<optA>", self.language.value());
        s += &format!("SaveSlot<optB>{}<optA>", self.save_slot);
        s += &self.pads_and_vibration();
        if !self.enabled_mods.is_empty() {
            s += &format!(
                "EnabledMods<optB>{}<optA>",
                self.enabled_mods.join("<optC>")
            );
        }
        let order: Vec<String> = self
            .mod_load_order
            .iter()
            .map(|(k, v)| format!("{k}<optD>{v}"))
            .collect();
        s += &format!("ModLoadOrder<optB>{}<optA>", order.join("<optC>"));
        let sums: Vec<String> = self
            .mod_checksums
            .iter()
            .map(|(k, v)| format!("{k}<optD>{v}"))
            .collect();
        s += &format!("ModChecksums<optB>{}<optA>", sums.join("<optC>"));
        for (on, key) in [
            (self.validation, "Validation"),
            (self.remix_tutorial_shown, "RemixTutorial"),
            (self.dlc_tutorial_shown, "DLCTutorial"),
        ] {
            if on {
                s += key;
                s += "<optA>";
            }
        }
        s += &format!("LastGameVersion<optB>{}<optA>", Self::VERSION);
        for u in &self.unrecognized {
            s += u;
            s += "<optA>";
        }
        s
    }

    /// `ToStringNonSynced` (`localoptions.txt`).
    #[must_use]
    pub fn to_non_synced_string(&self) -> String {
        let mut s = self.screen_and_controls();
        s += &self.pads_and_vibration();
        s += &format!("LastGameVersion<optB>{}<optA>", Self::VERSION);
        for u in &self.unrecognized_non_synced {
            s += u;
            s += "<optA>";
        }
        s
    }

    fn screen_and_controls(&self) -> String {
        let mut s = format!("ScreenResolution<optB>{}<optA>", self.resolution);
        if self.windowed {
            s += "Windowed<optA>";
        }
        for (i, c) in self.controls.iter().enumerate() {
            s += &format!("InputSetup<optB>{i}<optB>{c}<optA>");
        }
        s += &format!(
            "SoundVol<optB>{}<optA>",
            self.sound_effects_volume.cs_string()
        );
        s += &format!("MusicVol<optB>{}<optA>", self.music_volume.cs_string());
        s += &format!(
            "ArenaMusicVol<optB>{}<optA>",
            self.arena_music_volume.cs_string()
        );
        s
    }

    fn pads_and_vibration(&self) -> String {
        format!(
            "AllGamePads<optB>{}<optA>PlayerToSetInputFor<optB>{}<optA>Vibration<optB>{}<optA>",
            u8::from(self.all_game_pads),
            self.player_to_set_input_for,
            u8::from(self.vibration)
        )
    }

    /// `FromString`.
    pub fn from_options_string(&mut self, s: &str) {
        self.validation = false;
        self.dlc_tutorial_shown = false;
        self.remix_tutorial_shown = false;
        self.last_game_version = None;
        self.enabled_mods.clear();
        self.mod_load_order.clear();
        self.mod_checksums.clear();
        self.unrecognized.clear();
        for entry in s.split("<optA>") {
            if !self.apply_option(entry) && !entry.trim().is_empty() {
                self.unrecognized.push(entry.to_owned());
            }
        }
    }

    /// `FromUnsyncedString`.
    pub fn from_non_synced_string(&mut self, s: &str) {
        self.last_game_version = None;
        self.unrecognized_non_synced.clear();
        for entry in s.split("<optA>") {
            if !self.apply_option(entry) && !entry.trim().is_empty() {
                self.unrecognized_non_synced.push(entry.to_owned());
            }
        }
    }

    /// `ApplyOption`: whether the entry was recognised (the game's `false`). A value
    /// that does not parse throws in the game; here it is kept as unrecognised.
    fn apply_option(&mut self, entry: &str) -> bool {
        let f: Vec<&str> = entry.split("<optB>").collect();
        match f[0] {
            "ScreenResolution" => Self::parse_into(&mut self.resolution, Self::field(&f, 1)),
            "Windowed" => {
                self.windowed = true;
                true
            }
            "InputSetup" => match Self::field(&f, 1).trim().parse::<usize>() {
                Ok(i) if i < 4 => {
                    self.controls[i] = Self::field(&f, 2).to_owned();
                    true
                }
                _ => false,
            },
            "SoundVol" => Self::parse_into(&mut self.sound_effects_volume, Self::field(&f, 1)),
            "MusicVol" => Self::parse_into(&mut self.music_volume, Self::field(&f, 1)),
            "ArenaMusicVol" => Self::parse_into(&mut self.arena_music_volume, Self::field(&f, 1)),
            "TimeSinceLastSaveCopy" => {
                Self::parse_into(&mut self.time_since_last_save_copy, Self::field(&f, 1))
            }
            "Language" => {
                self.language = match Self::field(&f, 1).trim().parse::<i32>() {
                    Ok(n) => Language::from_legacy_index(n),
                    // An unregistered value (a mod's language) reads as English.
                    Err(_) => Language::from_value(Self::field(&f, 1)).unwrap_or_default(),
                };
                true
            }
            "SaveSlot" => Self::parse_into(&mut self.save_slot, Self::field(&f, 1)),
            "AllGamePads" => {
                self.all_game_pads = Self::field(&f, 1) == "1";
                true
            }
            "PlayerToSetInputFor" => {
                Self::parse_into(&mut self.player_to_set_input_for, Self::field(&f, 1))
            }
            "Vibration" => {
                let mut n = 0;
                let ok = Self::parse_into(&mut n, Self::field(&f, 1));
                self.vibration = n == 1;
                ok
            }
            "Validation" => {
                self.validation = true;
                true
            }
            "RemixTutorial" => {
                self.remix_tutorial_shown = true;
                true
            }
            "DLCTutorial" => {
                self.dlc_tutorial_shown = true;
                true
            }
            "LastGameVersion" => {
                self.last_game_version = Some(Self::field(&f, 1).to_owned());
                true
            }
            "EnabledMods" => {
                self.enabled_mods.extend(
                    Self::field(&f, 1)
                        .split("<optC>")
                        .filter(|m| !m.is_empty())
                        .map(str::to_owned),
                );
                true
            }
            "ModLoadOrder" => {
                for pair in Self::field(&f, 1).split("<optC>") {
                    if let Some((k, v)) = pair.split_once("<optD>")
                        && let Ok(v) = v.trim().parse()
                    {
                        match self.mod_load_order.iter_mut().find(|(m, _)| m == k) {
                            Some(e) => e.1 = v,
                            None => self.mod_load_order.push((k.to_owned(), v)),
                        }
                    }
                }
                true
            }
            "ModChecksums" => {
                for pair in Self::field(&f, 1).split("<optC>") {
                    if let Some((k, v)) = pair.split_once("<optD>") {
                        match self.mod_checksums.iter_mut().find(|(m, _)| m == k) {
                            Some(e) => e.1 = v.to_owned(),
                            None => self.mod_checksums.push((k.to_owned(), v.to_owned())),
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn field<'a>(fields: &[&'a str], i: usize) -> &'a str {
        fields.get(i).copied().unwrap_or("")
    }

    fn parse_into<T: std::str::FromStr>(field: &mut T, text: &str) -> bool {
        match text.trim().parse() {
            Ok(v) => {
                *field = v;
                true
            }
            Err(_) => false,
        }
    }

    /// `LoadOptions` from the port's directory: the `options` file, then
    /// `localoptions.txt` over it; the defaults where there are none.
    ///
    /// # Errors
    /// When a file exists but cannot be read or is not an options file.
    pub fn load(dir: &Path) -> Result<Self, SaveError> {
        let mut options = Self::default();
        match std::fs::read(dir.join(Self::FILE)) {
            Ok(bytes) => {
                let file = UserDataFile::parse(&bytes)?;
                if let Some(s) = file.get(Self::FILE) {
                    options.from_options_string(s);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        match std::fs::read_to_string(dir.join(Self::LOCAL_FILE)) {
            Ok(s) => options.from_non_synced_string(&s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(options)
    }

    /// `Save`: both files in `dir` (the `options` container keeps its other keys).
    ///
    /// # Errors
    /// When a file cannot be written.
    pub fn save(&self, dir: &Path) -> Result<(), SaveError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(Self::FILE);
        let mut file = match std::fs::read(&path) {
            Ok(bytes) => UserDataFile::parse(&bytes)?,
            Err(_) => UserDataFile::new(),
        };
        file.set(Self::FILE, self.to_options_string());
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, file.to_bytes())?;
        std::fs::rename(&tmp, &path)?;
        std::fs::write(dir.join(Self::LOCAL_FILE), self.to_non_synced_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Options()` (114166) and `ToString` (114524) with Remix and the DLCs off.
    #[test]
    fn defaults_write_the_games_order() {
        let s = Options::default().to_options_string();
        assert!(s.starts_with(
            "ScreenResolution<optB>1<optA>InputSetup<optB>0<optB>ANY<ctrlA>0<ctrlA><ctrlA>0<ctrlA>0<ctrlA><optA>"
        ));
        assert!(s.contains(
            "<optA>SoundVol<optB>0.8<optA>MusicVol<optB>0.8<optA>ArenaMusicVol<optB>0.625<optA>\
             TimeSinceLastSaveCopy<optB>0<optA>Language<optB>English<optA>SaveSlot<optB>0<optA>\
             AllGamePads<optB>0<optA>PlayerToSetInputFor<optB>0<optA>Vibration<optB>1<optA>\
             ModLoadOrder<optB><optA>ModChecksums<optB><optA>LastGameVersion<optB>v1.11.8<optA>"
        ));
        assert!(!s.contains("Windowed"));
    }

    /// The game's own `localoptions.txt` shape round-trips, Remix's entries kept as
    /// unrecognised at the end (`ApplyOption` returns true for them with MMF off).
    #[test]
    fn round_trip_keeps_unrecognised_entries() {
        let local = "ScreenResolution<optB>3<optA>Windowed<optA>InputSetup<optB>0<optB>KEYBOARD\
            <ctrlA>0<ctrlA><ctrlA>0<ctrlA>0<ctrlA>1,0:-1<ctrlB><optA>InputSetup<optB>1<optB>\
            SPECIFIC_GAMEPAD<ctrlA>0<ctrlA><ctrlA>0<ctrlA>0<ctrlA><optA>InputSetup<optB>2<optB>\
            SPECIFIC_GAMEPAD<ctrlA>1<ctrlA><ctrlA>0<ctrlA>0<ctrlA><optA>InputSetup<optB>3<optB>\
            SPECIFIC_GAMEPAD<ctrlA>2<ctrlA><ctrlA>0<ctrlA>0<ctrlA><optA>SoundVol<optB>0.4336972\
            <optA>MusicVol<optB>0.289081<optA>ArenaMusicVol<optB>0.5805964<optA>AllGamePads<optB>0\
            <optA>PlayerToSetInputFor<optB>0<optA>Vibration<optB>0<optA>LastGameVersion<optB>\
            v1.11.8<optA>AnalogSensitivity<optB>1<optA>Quality<optB>HIGH<optA>VSync<optA>";
        let mut o = Options::default();
        o.from_non_synced_string(local);
        assert_eq!(o.resolution, 3);
        assert!(o.windowed && !o.full_screen() && !o.vibration);
        assert_eq!(o.screen_size(), [1280.0, 768.0]);
        assert_eq!(o.sound_effects_volume, 0.433_697_2);
        assert_eq!(o.to_non_synced_string(), local);

        let synced = "ScreenResolution<optB>1<optA>SaveSlot<optB>2<optA>Language<optB>3<optA>\
            EnabledMods<optB>moreslugcats<optC>jollycoop<optA>ModLoadOrder<optB>a<optD>1<optC>b\
            <optD>2<optA>Validation<optA>FpsCap<optB>60<optA>";
        o.from_options_string(synced);
        assert_eq!((o.save_slot, o.language), (2, Language::German));
        assert!(o.validation);
        let out = o.to_options_string();
        assert!(out.contains("Language<optB>German<optA>SaveSlot<optB>2<optA>"));
        assert!(out.contains(
            "EnabledMods<optB>moreslugcats<optC>jollycoop<optA>ModLoadOrder<optB>a<optD>1<optC>b\
             <optD>2<optA>"
        ));
        assert!(
            out.ends_with("Validation<optA>LastGameVersion<optB>v1.11.8<optA>FpsCap<optB>60<optA>")
        );
    }

    #[test]
    fn save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let mut o = Options {
            music_volume: 0.3,
            save_slot: 1,
            windowed: true,
            ..Options::default()
        };
        o.save(dir.path()).unwrap();
        o.last_game_version = Some("v1.11.8".to_owned());
        assert_eq!(Options::load(dir.path()).unwrap(), o);
        let missing = tempfile::tempdir().unwrap();
        assert_eq!(Options::load(missing.path()).unwrap(), Options::default());
    }
}
