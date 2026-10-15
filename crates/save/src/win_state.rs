//! `WinState`: the passages' endgame trackers, as `DeathPersistentSaveData` keeps them
//! (`WINSTATE<dpB>...`, trackers split by `<wsA>`, fields by `<egA>`).
//!
//! Integer and float trackers are read; bool-array and list trackers (Traveller,
//! Dragon Slayer, Scholar) are written back as read (docs/DIVERGENCES.md).

/// `WinState.EndgameID` of the base game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndgameId {
    Survivor,
    Hunter,
    Saint,
    Traveller,
    Chieftain,
    Monk,
    Outlaw,
    DragonSlayer,
    Scholar,
    Friend,
}

impl EndgameId {
    pub const ALL: [Self; 10] = [
        Self::Survivor,
        Self::Hunter,
        Self::Saint,
        Self::Traveller,
        Self::Chieftain,
        Self::Monk,
        Self::Outlaw,
        Self::DragonSlayer,
        Self::Scholar,
        Self::Friend,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Survivor => "Survivor",
            Self::Hunter => "Hunter",
            Self::Saint => "Saint",
            Self::Traveller => "Traveller",
            Self::Chieftain => "Chieftain",
            Self::Monk => "Monk",
            Self::Outlaw => "Outlaw",
            Self::DragonSlayer => "DragonSlayer",
            Self::Scholar => "Scholar",
            Self::Friend => "Friend",
        }
    }

    /// The base game's value (DLC passages are unregistered with their mods off).
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.value() == value)
    }

    /// `CreateAndAddTracker`'s `IntegerTracker` max, for the integer trackers.
    fn integer_max(self) -> Option<i32> {
        match self {
            Self::Survivor => Some(5),
            Self::Hunter | Self::Saint | Self::Monk => Some(12),
            Self::Outlaw => Some(7),
            _ => None,
        }
    }

    fn float_tracker(self) -> bool {
        matches!(self, Self::Chieftain | Self::Friend)
    }

    /// `DeathModifyTracker`'s base branch: these lose a point on death.
    fn loses_on_death(self) -> bool {
        matches!(self, Self::Hunter | Self::Saint | Self::Monk | Self::Outlaw)
    }
}

/// A tracker's progress.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// `IntegerTracker`: progress (also `lastShownProgress` once loaded), default 0.
    Integer(i32),
    /// `FloatTracker`: the progress's text as read, and its value.
    Float(String, f32),
    /// Bool-array and list trackers: the text after the `consumed` field, as read.
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndgameTracker {
    pub id: EndgameId,
    pub consumed: bool,
    pub progress: Progress,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WinState {
    pub trackers: Vec<EndgameTracker>,
    /// `unrecognizedTrackers`: DLC and mod passages, kept whole.
    pub unrecognized: Vec<String>,
}

impl EndgameTracker {
    /// `AnyProgressToSave` with `lastShownProgress` as loaded.
    fn any_progress_to_save(&self, progress: &Progress, loaded: &Progress) -> bool {
        match (progress, loaded) {
            (Progress::Integer(p), Progress::Integer(last)) => {
                self.id.integer_max().is_some_and(|max| *p >= max) || *p != 0 || *last != 0
            }
            (Progress::Float(_, p), _) => *p >= 1.0 || *p != 0.0,
            (Progress::Raw(raw), _) => raw.split('.').any(|v| !v.is_empty() && v != "0"),
            _ => true,
        }
    }

    fn write(&self, progress: &Progress, out: &mut String) {
        out.push_str(self.id.value());
        out.push_str(if self.consumed { "<egA>1" } else { "<egA>0" });
        match progress {
            Progress::Integer(p) => out.push_str(&format!("<egA>{p}")),
            Progress::Float(raw, _) => {
                out.push_str("<egA>");
                out.push_str(raw);
            }
            Progress::Raw(raw) if raw.is_empty() => {}
            Progress::Raw(raw) => {
                out.push_str("<egA>");
                out.push_str(raw);
            }
        }
    }
}

impl WinState {
    /// `FromString` into a fresh `WinState`.
    #[must_use]
    pub fn from_string(s: &str) -> Self {
        let mut state = Self::default();
        for piece in s.split("<wsA>") {
            let parts: Vec<&str> = piece.split("<egA>").collect();
            if piece.is_empty() || parts[0].is_empty() {
                continue;
            }
            let Some(id) = EndgameId::from_value(parts[0]) else {
                state.unrecognized.push(piece.to_owned());
                continue;
            };
            let consumed = parts.get(1) == Some(&"1");
            let progress = if id.integer_max().is_some() {
                match parts.get(2) {
                    Some(p) => match p.trim().parse() {
                        Ok(v) => Progress::Integer(v),
                        Err(_) => {
                            // `int.Parse` throws: the game's `catch` keeps it unrecognized.
                            state.unrecognized.push(piece.to_owned());
                            continue;
                        }
                    },
                    None => Progress::Integer(0),
                }
            } else if id.float_tracker() {
                match parts.get(2) {
                    Some(p) => match p.trim().parse() {
                        Ok(v) => Progress::Float((*p).to_owned(), v),
                        Err(_) => {
                            state.unrecognized.push(piece.to_owned());
                            continue;
                        }
                    },
                    None => Progress::Float("0".to_owned(), 0.0),
                }
            } else {
                Progress::Raw(parts.get(2..).map(|p| p.join("<egA>")).unwrap_or_default())
            };
            let tracker = EndgameTracker {
                id,
                consumed,
                progress,
            };
            // `CreateAndAddTracker` replaces a tracker of the same ID in place.
            match state.trackers.iter_mut().find(|t| t.id == id) {
                Some(t) => *t = tracker,
                None => state.trackers.push(tracker),
            }
        }
        state
    }

    /// `SaveToString(saveAsIfPlayerDied)`: on a death, the trackers that lose a point
    /// are written a point lower unless already fulfilled.
    #[must_use]
    pub fn save_to_string(&self, died: bool) -> String {
        let mut text = String::new();
        for t in &self.trackers {
            let progress = match t.progress {
                Progress::Integer(p)
                    if died
                        && t.id.loses_on_death()
                        && !t.id.integer_max().is_some_and(|max| p >= max) =>
                {
                    Progress::Integer(p - 1)
                }
                _ => t.progress.clone(),
            };
            if t.any_progress_to_save(&progress, &t.progress) {
                t.write(&progress, &mut text);
                text.push_str("<wsA>");
            }
        }
        for u in &self.unrecognized {
            text.push_str(u);
            text.push_str("<wsA>");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_lowers_unfulfilled_trackers() {
        let s = "Traveller<egA>0<egA>0.1.1.<wsA>Survivor<egA>1<egA>5<wsA>Hunter<egA>0<egA>-4<wsA>\
                 Monk<egA>0<egA>12<wsA>Outlaw<egA>0<egA>0<wsA>Nomad<egA>0<egA>6.2<wsA>\
                 Chieftain<egA>0<egA>0.05<wsA>";
        let w = WinState::from_string(s);
        assert_eq!(w.unrecognized, ["Nomad<egA>0<egA>6.2"]);
        // Alive: Outlaw at 0 (and shown at 0) has nothing to save; Nomad goes last.
        assert_eq!(
            w.save_to_string(false),
            "Traveller<egA>0<egA>0.1.1.<wsA>Survivor<egA>1<egA>5<wsA>Hunter<egA>0<egA>-4<wsA>\
             Monk<egA>0<egA>12<wsA>Chieftain<egA>0<egA>0.05<wsA>Nomad<egA>0<egA>6.2<wsA>"
        );
        // Dead: Hunter and Outlaw lose a point; Monk is fulfilled; Survivor never loses.
        assert_eq!(
            w.save_to_string(true),
            "Traveller<egA>0<egA>0.1.1.<wsA>Survivor<egA>1<egA>5<wsA>Hunter<egA>0<egA>-5<wsA>\
             Monk<egA>0<egA>12<wsA>Outlaw<egA>0<egA>-1<wsA>Chieftain<egA>0<egA>0.05<wsA>\
             Nomad<egA>0<egA>6.2<wsA>"
        );
    }
}
