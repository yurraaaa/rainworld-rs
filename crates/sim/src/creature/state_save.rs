//! The creature state's save string (`CreatureState.ToString`/`LoadFromString`,
//! and the base subclasses' overrides) and its `CycleTick`. Entries are `key<cC>value`,
//! split by `<cB>`; a plain `CreatureState` ends each with `<cB>`, a `HealthState`
//! starts each of its own with it (so a base entry is followed by an empty one).
//!
//! Mirrors: CreatureState.cs, HealthState.cs

use rw_random::UnityRandom;
use rw_save::cs_float::CsFloat;

use super::{AbstractCreature, CreatureState, SocialMemory, StateKind};

/// `CreatureState`'s saved fields that nothing in the port reads, written back.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateSave {
    pub misc_flags: i32,
    pub misc_data: String,
    /// `unrecognizedSaveStrings`: key and value, in the order first read.
    pub unrecognized: Vec<(String, String)>,
}

impl StateSave {
    /// `unrecognizedSaveStrings[key] = value`.
    fn insert(&mut self, key: &str, value: &str) {
        match self.unrecognized.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => value.clone_into(v),
            None => self.unrecognized.push((key.to_owned(), value.to_owned())),
        }
    }

    fn remove(&mut self, key: &str) {
        self.unrecognized.retain(|(k, _)| k != key);
    }
}

/// The base-game `CreatureState` subclasses' own saved fields.
#[derive(Debug, Clone, PartialEq)]
pub enum StateParts {
    None,
    /// `LizardState`: `rot_type` is `RotType`'s index (0 `None`; rot is not ported).
    Lizard {
        throat_health: f32,
        limb_health: [f32; 4],
        rot_type: i32,
    },
    Mouse {
        battery: i32,
    },
    Vulture {
        mask: bool,
    },
    NeedleWorm {
        egg_spawn: bool,
    },
    /// `CentipedeState`: shells only once read from a save (a realized centipede sets
    /// them; not ported).
    Centipede {
        shells: Option<Vec<bool>>,
        meat_initiated: bool,
    },
}

impl super::CreatureState {
    /// `LizardState.throatHealth`.
    pub fn throat_health_mut(&mut self) -> Option<&mut f32> {
        match &mut self.parts {
            StateParts::Lizard { throat_health, .. } => Some(throat_health),
            _ => None,
        }
    }

    /// `LizardState.throatHealth` and `limbHealth`, to change.
    pub fn lizard_health_parts_mut(&mut self) -> Option<(&mut f32, &mut [f32; 4])> {
        match &mut self.parts {
            StateParts::Lizard {
                throat_health,
                limb_health,
                ..
            } => Some((throat_health, limb_health)),
            _ => None,
        }
    }

    /// `LizardState.throatHealth` and `limbHealth`.
    #[must_use]
    pub fn lizard_health_parts(&self) -> Option<(f32, [f32; 4])> {
        match &self.parts {
            StateParts::Lizard {
                throat_health,
                limb_health,
                ..
            } => Some((*throat_health, *limb_health)),
            _ => None,
        }
    }
}

impl StateParts {
    /// The constructor's values for state kind `kind`.
    #[must_use]
    pub fn of(kind: StateKind) -> Self {
        match kind {
            StateKind::Lizard => Self::Lizard {
                throat_health: 1.0,
                limb_health: [1.0; 4],
                rot_type: 0,
            },
            StateKind::Mouse => Self::Mouse { battery: 4000 },
            StateKind::Vulture => Self::Vulture { mask: true },
            StateKind::NeedleWorm => Self::NeedleWorm { egg_spawn: false },
            StateKind::Centipede => Self::Centipede {
                shells: None,
                meat_initiated: false,
            },
            _ => Self::None,
        }
    }

    /// The subclass's `ToString` part between `Health` and the unrecognized entries.
    fn save_string(&self) -> String {
        let mut text = String::new();
        match self {
            Self::Lizard {
                throat_health,
                limb_health,
                rot_type,
            } => {
                if *throat_health < 1.0 {
                    text.push_str(&format!(
                        "<cB>ThroatHealth<cC>{}",
                        throat_health.cs_string()
                    ));
                }
                if limb_health.iter().any(|&h| h < 1.0) {
                    let limbs: Vec<String> = limb_health.iter().map(|h| h.cs_string()).collect();
                    text.push_str(&format!("<cB>LimbHealth<cC>{}", limbs.join("<cC>")));
                }
                if *rot_type != 0 {
                    text.push_str(&format!("<cB>RotType<cC>{rot_type}"));
                }
            }
            Self::Mouse { battery } if *battery < 4000 => {
                text.push_str(&format!("<cB>Battery<cC>{battery}"));
            }
            Self::Vulture { mask: false } => text.push_str("<cB>NOMASK"),
            Self::NeedleWorm { egg_spawn: true } => text.push_str("<cB>EGGSPAWN"),
            Self::Centipede {
                shells,
                meat_initiated,
            } => {
                if let Some(shells) = shells
                    && shells.iter().any(|&s| !s)
                {
                    let bits: String = shells.iter().map(|&s| if s { '1' } else { '0' }).collect();
                    text.push_str(&format!("<cB>Shells<cC>{bits}"));
                }
                if *meat_initiated {
                    text.push_str("<cB>MEATINIT");
                }
            }
            _ => {}
        }
        text
    }

    /// The subclass's `LoadFromString` after the base's: its keys leave the
    /// unrecognized entries.
    fn load(&mut self, entries: &[&str], save: &mut StateSave) {
        for entry in entries {
            let kv: Vec<&str> = entry.split("<cC>").collect();
            match (&mut *self, kv[0]) {
                (Self::Lizard { throat_health, .. }, "ThroatHealth") => {
                    *throat_health = CreatureState::parse_float(kv.get(1));
                }
                (Self::Lizard { limb_health, .. }, "LimbHealth") => {
                    for (limb, value) in limb_health.iter_mut().zip(kv.iter().skip(1)) {
                        *limb = CreatureState::parse_float(Some(value));
                    }
                }
                (Self::Lizard { rot_type, .. }, "RotType") => {
                    *rot_type = CreatureState::parse_int(kv.get(1));
                }
                (Self::Mouse { battery }, "Battery") => {
                    *battery = CreatureState::parse_int(kv.get(1));
                }
                (Self::Vulture { mask }, "NOMASK") => *mask = false,
                (Self::NeedleWorm { egg_spawn }, "EGGSPAWN") => *egg_spawn = true,
                (Self::Centipede { shells, .. }, "Shells") => {
                    *shells = Some(kv.get(1).unwrap_or(&"").chars().map(|c| c == '1').collect());
                }
                (Self::Centipede { meat_initiated, .. }, "MEATINIT") => *meat_initiated = true,
                _ => {}
            }
        }
        let keys: &[&str] = match self {
            Self::Lizard { .. } => &["ThroatHealth", "LimbHealth", "RotType"],
            Self::Mouse { .. } => &["Battery"],
            Self::Vulture { .. } => &["NOMASK"],
            Self::NeedleWorm { .. } => &["EGGSPAWN"],
            Self::Centipede { .. } => &["Shells", "MEATINIT"],
            Self::None => &[],
        };
        for key in keys {
            save.remove(key);
        }
    }
}

impl CreatureState {
    /// `float.Parse` of a save value (0 where the game would throw).
    fn parse_float(value: Option<&&str>) -> f32 {
        value
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or_else(|| {
                log::warn!("bad float in a creature state: {value:?}");
                0.0
            })
    }

    /// `int.Parse` of a save value (0 where the game would throw).
    fn parse_int(value: Option<&&str>) -> i32 {
        value
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or_else(|| {
                log::warn!("bad integer in a creature state: {value:?}");
                0
            })
    }

    /// `CycleTick` (once when a world loads cycles after its region was last updated):
    /// a lizard's throat and limbs heal, a `HealthState` gains a quarter of health up to
    /// 1, the social memory fades, and a living centipede's saved shells each regrow
    /// with a 0.2 roll (lost again with 0.015).
    pub fn cycle_tick(&mut self, rng: &mut UnityRandom) {
        if let StateParts::Lizard {
            throat_health,
            limb_health,
            ..
        } = &mut self.parts
        {
            *throat_health = (*throat_health + 0.25).min(1.0);
            for limb in limb_health.iter_mut() {
                *limb = (*limb + 0.5).min(1.0);
            }
        }
        if self.has_health() {
            self.health = (self.health + 0.25).min(1.0);
        }
        if let Some(m) = &mut self.social_memory {
            m.cycle_tick();
        }
        if let StateParts::Centipede {
            shells: Some(shells),
            ..
        } = &mut self.parts
            && self.alive
        {
            for shell in shells.iter_mut() {
                if rng.value() < 0.2 {
                    *shell = rng.value() < 0.985;
                }
            }
        }
    }
}

impl AbstractCreature {
    /// `CreatureState.BaseSaveString`.
    fn base_save_string(&self) -> String {
        let s = &self.state;
        let mut text = String::new();
        if !s.alive {
            text.push_str("Dead<cB>");
        }
        if s.alive
            && let Some(m) = &s.social_memory
            && !m.relationships.is_empty()
        {
            text.push_str(&format!("Social<cC>{}<cB>", m.save_string()));
        }
        if let Some(data) = &self.spawn_data {
            text.push_str(&format!("SpawnData<cC>{data}<cB>"));
        }
        if s.meat_left != self.template.meat_points() {
            text.push_str(&format!("MeatLeft<cC>{}<cB>", s.meat_left));
        }
        if s.save.misc_flags > 0 {
            text.push_str(&format!("MiscFlags<cC>{}<cB>", s.save.misc_flags));
        }
        if !s.save.misc_data.is_empty() {
            text.push_str(&format!("MiscData<cC>{}<cB>", s.save.misc_data));
        }
        text
    }

    /// `CreatureState.ToString()` of the creature's state subclass.
    #[must_use]
    pub fn state_save_string(&self) -> String {
        let s = &self.state;
        let mut text = self.base_save_string();
        if s.has_health() {
            if s.health < 1.0 {
                text.push_str(&format!("<cB>Health<cC>{}", s.health.cs_string()));
            }
            text.push_str(&s.parts.save_string());
            for (k, v) in &s.save.unrecognized {
                text.push_str(&format!("<cB>{k}<cC>{v}"));
            }
        } else {
            for (k, v) in &s.save.unrecognized {
                text.push_str(&format!("{k}<cC>{v}<cB>"));
            }
        }
        text
    }

    /// `CreatureState.LoadFromString` of the creature's state subclass. The DLC keys
    /// (`ExtractKarma`, `MiscWatcherFlags`, `MiscWatcherData`) stay unrecognized, as
    /// with the DLCs off.
    pub fn load_state(&mut self, state: &str) {
        let entries: Vec<&str> = state.split("<cB>").collect();
        self.state.save.unrecognized.clear();
        for entry in &entries {
            let kv: Vec<&str> = entry.split("<cC>").collect();
            match kv[0] {
                "Dead" => self.state.alive = false,
                "Social" => {
                    if let Some(m) = kv.get(1).and_then(|v| SocialMemory::from_save_string(v)) {
                        self.state.social_memory = Some(m);
                    } else {
                        log::warn!("bad social memory in a creature state: {entry}");
                    }
                }
                "SpawnData" => self.spawn_data = kv.get(1).map(|v| (*v).to_owned()),
                "MeatLeft" => self.state.meat_left = CreatureState::parse_int(kv.get(1)),
                "MiscFlags" => self.state.save.misc_flags = CreatureState::parse_int(kv.get(1)),
                "MiscData" => {
                    self.state.save.misc_data = kv.get(1).copied().unwrap_or("").to_owned();
                }
                key if kv.len() >= 2 => self.state.save.insert(key, kv[1]),
                _ => {}
            }
        }
        if self.state.has_health() {
            for entry in &entries {
                let kv: Vec<&str> = entry.split("<cC>").collect();
                if kv[0] == "Health" {
                    self.state.health = CreatureState::parse_float(kv.get(1));
                }
            }
            self.state.save.remove("Health");
        }
        let state = &mut self.state;
        state.parts.load(&entries, &mut state.save);
    }
}

#[cfg(test)]
#[path = "tests/state_save.rs"]
mod tests;
