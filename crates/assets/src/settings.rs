//! Room settings files (`<room>_settings.txt`) and their template inheritance, for the
//! values the room graphics need: `Palette`, `FadePalette`, `EffectColorA/B`, `Clouds`
//! and `Grime`. Mirrors `RoomSettings` (constructor, `FindParent`, `Load`) and
//! `Region.GetRoomSettingsTemplate`; see `rainworld-re/notes/room-format.md` section 6.
//!
//! A value missing from a room's file comes from its parent: the template named by its
//! `Template:` line, else the region's first template, else the root defaults
//! ([`RoomSettings::ANCESTOR`]). Templates inherit from the root only. `FadePalette` is
//! never inherited.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{AssetError, StreamingAssets};

/// `FadePalette: <palette>, <fade>, <fade>, ...`: a second palette and one blend amount
/// per camera.
#[derive(Debug, Clone, PartialEq)]
pub struct FadePalette {
    pub palette: i32,
    pub fades: Vec<f32>,
}

impl FadePalette {
    /// Blend toward the fade palette for camera `camera`; 0 past the end of the list.
    #[must_use]
    pub fn fade(&self, camera: usize) -> f32 {
        self.fades.get(camera).copied().unwrap_or(0.0)
    }
}

/// The values one settings file sets itself (`None` = inherited).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsFile {
    /// Value of the first `Template:` line, as written.
    pub template: Option<String>,
    pub palette: Option<i32>,
    pub effect_color_a: Option<i32>,
    pub effect_color_b: Option<i32>,
    pub clouds: Option<f32>,
    pub grime: Option<f32>,
    pub fade_palette: Option<FadePalette>,
}

/// A room's settings after inheritance.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomSettings {
    pub palette: i32,
    /// Index into `effectcolors.png`; -1 keeps the palette file's own texels.
    pub effect_color_a: i32,
    pub effect_color_b: i32,
    /// 0 = full sun, 1 = no sun.
    pub clouds: f32,
    pub grime: f32,
    pub fade_palette: Option<FadePalette>,
}

/// A region's settings templates (`Room Setting Templates:` in `properties.txt`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegionTemplates {
    /// Lowercase names with their settings, first one is the default parent.
    templates: Vec<(String, RoomSettings)>,
}

impl SettingsFile {
    /// Parses `Key: value` lines like `RoomSettings.Load`: a line counts only when
    /// splitting it on `": "` gives exactly two parts; unknown keys are ignored.
    ///
    /// # Errors
    /// A message when a value the game parses is not a number (the game throws there).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut file = Self::default();
        for line in text.lines() {
            let parts: Vec<&str> = line.split(": ").collect();
            if parts.first() == Some(&"Template") && file.template.is_none() {
                // FindParent reads the value even when the line has extra ": " parts.
                let value = parts.get(1).ok_or("Template line without a value")?;
                file.template = Some((*value).to_owned());
            }
            let [key, value] = parts[..] else { continue };
            match key {
                "Palette" => file.palette = Some(Self::int(value)?),
                "EffectColorA" => file.effect_color_a = Some(Self::int(value)?),
                "EffectColorB" => file.effect_color_b = Some(Self::int(value)?),
                "Clouds" => file.clouds = Some(Self::float(value)?),
                "Grime" => file.grime = Some(Self::float(value)?),
                "FadePalette" => file.fade_palette = Some(Self::fade_palette(value)?),
                _ => {}
            }
        }
        Ok(file)
    }

    /// `int.Parse` with `NumberStyles.Any`: surrounding spaces and a zero fraction allowed.
    fn int(value: &str) -> Result<i32, String> {
        let v = value.trim();
        v.parse::<i32>()
            .ok()
            .or_else(|| {
                let f = v.parse::<f64>().ok()?;
                (f.fract() == 0.0 && f.abs() <= f64::from(i32::MAX)).then_some(f as i32)
            })
            .ok_or_else(|| format!("invalid integer {value:?}"))
    }

    fn float(value: &str) -> Result<f32, String> {
        value
            .trim()
            .parse()
            .map_err(|_| format!("invalid number {value:?}"))
    }

    /// Values are split on `", "` after the game appends a space to a trailing comma,
    /// so a trailing comma leaves an empty (invalid) fade, as in the game.
    fn fade_palette(value: &str) -> Result<FadePalette, String> {
        let spaced = if value.ends_with(',') {
            format!("{value} ")
        } else {
            value.to_owned()
        };
        let mut parts = spaced.split(", ");
        let palette = Self::int(parts.next().unwrap_or(""))?;
        let fades = parts.map(Self::float).collect::<Result<_, _>>()?;
        Ok(FadePalette { palette, fades })
    }

    /// Fills unset values from `parent`. `FadePalette` is taken from this file only.
    #[must_use]
    pub fn resolve(&self, parent: &RoomSettings) -> RoomSettings {
        RoomSettings {
            palette: self.palette.unwrap_or(parent.palette),
            effect_color_a: self.effect_color_a.unwrap_or(parent.effect_color_a),
            effect_color_b: self.effect_color_b.unwrap_or(parent.effect_color_b),
            clouds: self.clouds.unwrap_or(parent.clouds),
            grime: self.grime.unwrap_or(parent.grime),
            fade_palette: self.fade_palette.clone(),
        }
    }

    /// Reads and parses a settings file.
    ///
    /// # Errors
    /// [`AssetError::Io`] or [`AssetError::Format`].
    pub fn load(path: &Path) -> Result<Self, AssetError> {
        let text = fs::read_to_string(path).map_err(|e| AssetError::io(path, e))?;
        Self::parse(&text).map_err(|m| AssetError::format(path, m))
    }
}

impl RoomSettings {
    /// The root defaults (`DefaultRoomSettings.ancestor`).
    pub const ANCESTOR: Self = Self {
        palette: 0,
        effect_color_a: 0,
        effect_color_b: 0,
        clouds: 0.0,
        grime: 0.5,
        fade_palette: None,
    };

    /// Loads the settings of `room` as the game does: `<room>_settings-<timeline>.txt`
    /// if it exists, else `<room>_settings.txt` (both via the room file lookup), parent
    /// chosen from `region`'s templates (`None` for arena levels: root defaults, and the
    /// `Template:` line is not read). A room without a settings file gets its parent's
    /// values.
    ///
    /// # Errors
    /// When a settings, template or properties file exists but cannot be read or parsed.
    pub fn load(
        assets: &StreamingAssets,
        room: &str,
        region: Option<&str>,
        timeline: Option<&str>,
    ) -> Result<Self, AssetError> {
        let room = room.to_lowercase();
        let path = Self::settings_path(assets, &room, timeline);
        let templates = match region {
            Some(region) => RegionTemplates::load(assets, region, timeline)?,
            None => RegionTemplates::default(),
        };
        let file = match &path {
            Some(path) => SettingsFile::load(path)?,
            None => SettingsFile::default(),
        };
        let parent = if region.is_some() {
            templates.parent_for(file.template.as_deref())
        } else {
            Self::ANCESTOR
        };
        Ok(file.resolve(&parent))
    }

    fn settings_path(
        assets: &StreamingAssets,
        room: &str,
        timeline: Option<&str>,
    ) -> Option<PathBuf> {
        timeline
            .and_then(|t| {
                assets.find_room_file(room, &format!("_settings-{}.txt", t.to_lowercase()))
            })
            .or_else(|| assets.find_room_file(room, "_settings.txt"))
            .or_else(|| {
                // RoomSettings retries next to the geometry file.
                let geometry = assets.find_room_file(room, ".txt")?;
                let path = geometry.with_file_name(format!("{room}_settings.txt"));
                path.is_file().then_some(path)
            })
    }
}

impl RegionTemplates {
    /// Reads `world/<region>/properties[-<timeline>].txt` and each listed template
    /// `world/<region>/<region>_settingstemplate_<name>.txt` (a missing template file
    /// leaves root defaults). A region without properties or list has no templates.
    ///
    /// # Errors
    /// When an existing properties or template file cannot be read or parsed.
    pub fn load(
        assets: &StreamingAssets,
        region: &str,
        timeline: Option<&str>,
    ) -> Result<Self, AssetError> {
        let region = region.to_lowercase();
        let properties = timeline
            .map(|t| assets.file(&format!("world/{region}/properties-{t}.txt")))
            .filter(|p| p.is_file())
            .unwrap_or_else(|| assets.file(&format!("world/{region}/properties.txt")));
        if !properties.is_file() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(&properties).map_err(|e| AssetError::io(&properties, e))?;
        let Some(names) = Self::template_names(&text) else {
            return Ok(Self::default());
        };
        let templates = names
            .into_iter()
            .map(|name| {
                let path = assets.file(&format!(
                    "world/{region}/{region}_settingstemplate_{name}.txt"
                ));
                let settings = if path.is_file() {
                    SettingsFile::load(&path)?.resolve(&RoomSettings::ANCESTOR)
                } else {
                    RoomSettings::ANCESTOR
                };
                Ok((name, settings))
            })
            .collect::<Result<_, AssetError>>()?;
        Ok(Self { templates })
    }

    /// The lowercased names on the `Room Setting Templates:` line, if any.
    fn template_names(properties: &str) -> Option<Vec<String>> {
        properties.lines().find_map(|line| {
            let value = line.strip_prefix("Room Setting Templates: ")?;
            let value = if value.ends_with(',') {
                format!("{value} ")
            } else {
                value.to_owned()
            };
            Some(
                value
                    .to_lowercase()
                    .split(", ")
                    .map(str::to_owned)
                    .collect(),
            )
        })
    }

    /// The parent of a room whose file has `template` as its `Template:` value: `NONE`
    /// is the root; `<REGION>_SettingsTemplate_<Name>` is the template `<name>` (the third
    /// `_`-separated part; root when unknown); no line is the first template.
    #[must_use]
    pub fn parent_for(&self, template: Option<&str>) -> RoomSettings {
        match template {
            Some("NONE") => RoomSettings::ANCESTOR,
            Some(value) => {
                let lower = value.to_lowercase();
                let name = lower.split('_').nth(2).unwrap_or("");
                self.template(name)
            }
            None => self
                .templates
                .first()
                .map_or(RoomSettings::ANCESTOR, |(_, s)| s.clone()),
        }
    }

    /// `Region.GetRoomSettingsTemplate`: `first` or a listed name, else the root.
    #[must_use]
    pub fn template(&self, name: &str) -> RoomSettings {
        let name = name.to_lowercase();
        let found = if name == "first" {
            self.templates.first()
        } else {
            self.templates.iter().find(|(n, _)| *n == name)
        };
        found.map_or(RoomSettings::ANCESTOR, |(_, s)| s.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: &str = "Template: SU_SettingsTemplate_Inside\nClouds: 0.25\nPalette: 7\n\
                        EffectColorB: 3\nFadePalette: 8, 0.5, 0.25\nEffects: a-1-2, \n";

    #[test]
    fn parses_own_values() {
        let f = SettingsFile::parse(ROOM).expect("parses");
        assert_eq!(f.template.as_deref(), Some("SU_SettingsTemplate_Inside"));
        assert_eq!(f.palette, Some(7));
        assert_eq!(f.effect_color_a, None);
        assert_eq!(f.effect_color_b, Some(3));
        assert_eq!(f.clouds, Some(0.25));
        assert_eq!(f.grime, None);
        assert_eq!(
            f.fade_palette,
            Some(FadePalette {
                palette: 8,
                fades: vec![0.5, 0.25]
            })
        );
    }

    #[test]
    fn lines_with_extra_separators_are_skipped() {
        let f = SettingsFile::parse("Palette: 3: 4\nGrime: 0.1\r\n").expect("parses");
        assert_eq!(f.palette, None);
        assert_eq!(f.grime, Some(0.1));
    }

    #[test]
    fn bad_numbers_and_trailing_fade_comma_fail() {
        assert!(SettingsFile::parse("Palette: x").is_err());
        assert!(SettingsFile::parse("FadePalette: 8, 0.5,").is_err());
        assert_eq!(
            SettingsFile::parse("Palette: 4.0").expect("ok").palette,
            Some(4)
        );
    }

    #[test]
    fn inheritance_fills_unset_values() {
        let templates = RegionTemplates {
            templates: vec![
                (
                    "outside".into(),
                    SettingsFile::parse("Grime: 0.4\nPalette: 1")
                        .unwrap()
                        .resolve(&RoomSettings::ANCESTOR),
                ),
                (
                    "inside".into(),
                    SettingsFile::parse("Grime: 0.2\nEffectColorA: 5")
                        .unwrap()
                        .resolve(&RoomSettings::ANCESTOR),
                ),
            ],
        };
        let f = SettingsFile::parse(ROOM).unwrap();
        let s = f.resolve(&templates.parent_for(f.template.as_deref()));
        assert_eq!((s.palette, s.effect_color_a, s.effect_color_b), (7, 5, 3));
        assert_eq!((s.clouds, s.grime), (0.25, 0.2));

        // No Template line: first template. NONE: root. Unknown name: root.
        assert_eq!(templates.parent_for(None).grime, 0.4);
        assert_eq!(templates.parent_for(Some("NONE")), RoomSettings::ANCESTOR);
        assert_eq!(
            templates.parent_for(Some("SU_SettingsTemplate_Attic")),
            RoomSettings::ANCESTOR
        );
        assert_eq!(templates.template("FIRST").palette, 1);
    }

    #[test]
    fn fade_past_the_list_is_zero() {
        let fp = FadePalette {
            palette: 3,
            fades: vec![0.5],
        };
        assert_eq!((fp.fade(0), fp.fade(1)), (0.5, 0.0));
    }

    #[test]
    fn template_names_lowercased() {
        assert_eq!(
            RegionTemplates::template_names(
                "Palette: 0\nRoom Setting Templates: Outside, Inside\n"
            ),
            Some(vec!["outside".to_owned(), "inside".to_owned()])
        );
        assert_eq!(RegionTemplates::template_names("Palette: 0\n"), None);
    }
}
