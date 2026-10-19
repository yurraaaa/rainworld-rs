//! `WinState`: the passages' endgame trackers, as `DeathPersistentSaveData` keeps them
//! (`WINSTATE<dpB>...`, trackers split by `<wsA>`, fields by `<egA>`).
//!
//! Integer and float trackers are read; bool-array and list trackers (Traveller,
//! Dragon Slayer, Scholar) are written back as read until a cycle changes them
//! (docs/divergences/).
//!
//! Mirrors: WinState.cs

use rw_assets::SlugcatName;

use crate::cs_float::CsFloat;
use crate::session::PlayerSessionRecord;

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

    /// `SlugcatStoryRegions` with every DLC off.
    pub const STORY_REGIONS: [&'static str; 12] = [
        "SU", "HI", "DS", "CC", "GW", "SH", "SL", "SI", "LF", "UW", "SS", "SB",
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

    /// `CreateAndAddTracker`'s `IntegerTracker` max, for the integer trackers (min 0).
    #[must_use]
    pub fn integer_max(self) -> Option<i32> {
        match self {
            Self::Survivor => Some(5),
            Self::Hunter | Self::Saint | Self::Monk => Some(12),
            Self::Outlaw => Some(7),
            _ => None,
        }
    }

    /// `IntegerTracker.showFrom` (the float trackers' is 0).
    #[must_use]
    pub fn show_from(self) -> i32 {
        match self {
            Self::Hunter | Self::Saint | Self::Monk | Self::Outlaw => 2,
            _ => 0,
        }
    }

    fn float_tracker(self) -> bool {
        matches!(self, Self::Chieftain | Self::Friend)
    }

    /// A `BoolArrayTracker`'s slots: the story regions, or the six lizards.
    fn slots(self) -> Option<usize> {
        match self {
            Self::Traveller => Some(Self::STORY_REGIONS.len()),
            Self::DragonSlayer => Some(6),
            _ => None,
        }
    }

    /// The Scholar's `totItemsToWin`.
    pub const LIST_MAX: usize = 3;

    /// `PassageDisplayName`.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Survivor => "The Survivor",
            Self::Hunter => "The Hunter",
            Self::Saint => "The Saint",
            Self::Traveller => "The Wanderer",
            Self::Chieftain => "The Chieftain",
            Self::Monk => "The Monk",
            Self::Outlaw => "The Outlaw",
            Self::DragonSlayer => "The Dragon Slayer",
            Self::Scholar => "The Scholar",
            Self::Friend => "The Friend",
        }
    }

    /// `DeathModifyTracker`'s base branch: these lose a point on death.
    fn loses_on_death(self) -> bool {
        matches!(self, Self::Hunter | Self::Saint | Self::Monk | Self::Outlaw)
    }

    /// `CreateAndAddTracker` with every DLC off.
    fn new_progress(self) -> Progress {
        if self.integer_max().is_some() {
            Progress::Integer {
                progress: 0,
                last_shown: 0,
            }
        } else if self.float_tracker() {
            Progress::Float {
                progress: 0.0,
                last_shown: 0.0,
                text: None,
            }
        } else if let Some(n) = self.slots() {
            Progress::Bools {
                progress: vec![false; n],
                last_shown: vec![false; n],
            }
        } else {
            Progress::List {
                progress: Vec::new(),
                last: Vec::new(),
            }
        }
    }
}

impl EndgameId {
    /// A bool-array or list tracker's saved progress as `FromString` reads it (a
    /// Dragon Slayer list of lizard indices, More Slugcats' format, included).
    fn parse_raw(self, raw: &str) -> Progress {
        let values: Vec<&str> = raw.split('.').collect();
        let ints: Vec<i32> = values
            .iter()
            .filter(|v| !v.is_empty())
            .map(|v| v.trim().parse().unwrap_or(0))
            .collect();
        match self.slots() {
            Some(n) => {
                let mut bools = vec![false; n];
                let ones = ints.iter().filter(|&&v| v == 1).count();
                let zeros = ints.iter().filter(|&&v| v == 0).count();
                if self == Self::DragonSlayer && ones <= 1 && zeros <= 1 {
                    for i in ints {
                        if let Some(b) = usize::try_from(i).ok().and_then(|i| bools.get_mut(i)) {
                            *b = true;
                        }
                    }
                } else {
                    for (b, v) in bools.iter_mut().zip(&values) {
                        *b = *v == "1";
                    }
                }
                Progress::Bools {
                    progress: bools.clone(),
                    last_shown: bools,
                }
            }
            None => Progress::List {
                progress: ints.clone(),
                last: ints,
            },
        }
    }
}

/// A tracker's progress.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// `IntegerTracker`: progress and `lastShownProgress` (equal once loaded).
    Integer { progress: i32, last_shown: i32 },
    /// `FloatTracker`; `text` is the progress as read, written while unchanged.
    Float {
        progress: f32,
        last_shown: f32,
        text: Option<String>,
    },
    /// Bool-array and list trackers not changed since read: the text after the
    /// `consumed` field.
    Raw(String),
    /// `BoolArrayTracker` (Traveller, the base game's Dragon Slayer) once changed.
    Bools {
        progress: Vec<bool>,
        last_shown: Vec<bool>,
    },
    /// `ListTracker` (Scholar) once changed.
    List { progress: Vec<i32>, last: Vec<i32> },
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

/// What `WinState.CycleCompleted` reads of the game.
#[derive(Debug, Clone, PartialEq)]
pub struct CycleCompletion<'a> {
    pub slugcat: SlugcatName,
    /// Karma and cap before the cycle's rise.
    pub karma: i32,
    pub karma_cap: i32,
    pub record: &'a PlayerSessionRecord,
    /// `creatureCommunities.LikeOfPlayer(Scavengers, -1, 0)`.
    pub scavenger_like: f32,
}

impl EndgameTracker {
    /// `AnyProgressToSave`.
    fn any_progress_to_save(&self, progress: &Progress) -> bool {
        match progress {
            Progress::Integer {
                progress,
                last_shown,
            } => {
                self.id.integer_max().is_some_and(|max| *progress >= max)
                    || *progress != 0
                    || *last_shown != 0
            }
            Progress::Float {
                progress,
                last_shown,
                ..
            } => *progress >= 1.0 || *progress != 0.0 || *last_shown != 0.0,
            // As the game would read it (a Dragon Slayer "0" is the first lizard).
            Progress::Raw(_) => {
                let mut parsed = self.clone();
                parsed.progress = progress.clone();
                parsed.parse_raw();
                !matches!(parsed.progress, Progress::Raw(_))
                    && parsed.any_progress_to_save(&parsed.progress)
            }
            Progress::Bools {
                progress,
                last_shown,
            } => progress.iter().chain(last_shown).any(|&b| b),
            Progress::List { progress, .. } => !progress.is_empty(),
        }
    }

    /// `GoalAlreadyFullfilled`: fulfilled as last shown.
    #[must_use]
    pub fn goal_already_fulfilled(&self) -> bool {
        match self.parsed() {
            Progress::Integer { last_shown, .. } => {
                self.id.integer_max().is_some_and(|max| last_shown >= max)
            }
            Progress::Float { last_shown, .. } => last_shown >= 1.0,
            Progress::Bools { last_shown, .. } => last_shown.iter().all(|&b| b),
            Progress::List { last, .. } => last.len() >= EndgameId::LIST_MAX,
            Progress::Raw(_) => false,
        }
    }

    fn integer(&self) -> i32 {
        match self.progress {
            Progress::Integer { progress, .. } => progress,
            _ => 0,
        }
    }

    /// `IntegerTracker.SetProgress` (clamped to min and max).
    fn set_integer(&mut self, v: i32) {
        if let (Progress::Integer { progress, .. }, Some(max)) =
            (&mut self.progress, self.id.integer_max())
        {
            *progress = v.clamp(0, max);
        }
    }

    /// `FloatTracker.SetProgress` (clamped to 0..1).
    fn set_float(&mut self, v: f32) {
        if let Progress::Float { progress, text, .. } = &mut self.progress {
            *progress = v.clamp(0.0, 1.0);
            *text = None;
        }
    }

    /// A raw bool-array or list tracker read as the game's `FromString` reads it.
    fn parse_raw(&mut self) {
        if let Progress::Raw(raw) = &self.progress {
            self.progress = self.id.parse_raw(raw);
        }
    }

    /// The progress, a raw tracker read as the game reads it.
    #[must_use]
    pub fn parsed(&self) -> Progress {
        match &self.progress {
            Progress::Raw(raw) => self.id.parse_raw(raw),
            p => p.clone(),
        }
    }

    /// `GoalFullfilled`.
    #[must_use]
    pub fn goal_fulfilled(&self) -> bool {
        match self.parsed() {
            Progress::Integer { progress, .. } => {
                self.id.integer_max().is_some_and(|max| progress >= max)
            }
            Progress::Float { progress, .. } => progress >= 1.0,
            Progress::Bools { progress, .. } => progress.iter().all(|&b| b),
            Progress::List { progress, .. } => progress.len() >= EndgameId::LIST_MAX,
            Progress::Raw(_) => false,
        }
    }

    /// `AnyProgressToShow`: whether the sleep and death screens show a meter for it.
    #[must_use]
    pub fn any_progress_to_show(&self) -> bool {
        let show_from = self.id.show_from();
        match self.parsed() {
            Progress::Integer {
                progress,
                last_shown,
            } => progress > show_from || last_shown > show_from,
            Progress::Float {
                progress,
                last_shown,
                ..
            } => progress > show_from as f32 || last_shown > show_from as f32,
            p => self.any_progress_to_save(&p),
        }
    }

    fn write(&self, progress: &Progress, out: &mut String) {
        out.push_str(self.id.value());
        out.push_str(if self.consumed { "<egA>1" } else { "<egA>0" });
        match progress {
            Progress::Integer { progress, .. } => out.push_str(&format!("<egA>{progress}")),
            Progress::Float { text: Some(t), .. } => {
                out.push_str("<egA>");
                out.push_str(t);
            }
            Progress::Float { progress, .. } => {
                out.push_str("<egA>");
                out.push_str(&progress.cs_string());
            }
            Progress::Raw(raw) if raw.is_empty() => {}
            Progress::Raw(raw) => {
                out.push_str("<egA>");
                out.push_str(raw);
            }
            Progress::Bools { progress, .. } => {
                out.push_str("<egA>");
                for b in progress {
                    out.push_str(if *b { "1." } else { "0." });
                }
            }
            Progress::List { progress, .. } => {
                out.push_str("<egA>");
                let items: Vec<String> = progress.iter().map(i32::to_string).collect();
                out.push_str(&items.join("."));
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
                match parts.get(2).map(|p| p.trim().parse()) {
                    Some(Ok(v)) => Progress::Integer {
                        progress: v,
                        last_shown: v,
                    },
                    // `int.Parse` throws: the game's `catch` keeps it unrecognized.
                    Some(Err(_)) => {
                        state.unrecognized.push(piece.to_owned());
                        continue;
                    }
                    None => id.new_progress(),
                }
            } else if id.float_tracker() {
                match parts.get(2).map(|p| (p, p.trim().parse())) {
                    Some((p, Ok(v))) => Progress::Float {
                        progress: v,
                        last_shown: v,
                        text: Some((*p).to_owned()),
                    },
                    Some((_, Err(_))) => {
                        state.unrecognized.push(piece.to_owned());
                        continue;
                    }
                    None => id.new_progress(),
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

    /// `GetTracker(ID, addIfMissing)`, as an index.
    fn tracker(&mut self, id: EndgameId, add_if_missing: bool) -> Option<usize> {
        if let Some(i) = self.trackers.iter().position(|t| t.id == id) {
            return Some(i);
        }
        if !add_if_missing {
            return None;
        }
        self.trackers.push(EndgameTracker {
            id,
            consumed: false,
            progress: id.new_progress(),
        });
        Some(self.trackers.len() - 1)
    }

    /// `CycleCompleted` with every DLC off and no friend in the den. Returns the
    /// `menuRegion` to set, if the player slept in a story region.
    pub fn cycle_completed(&mut self, cx: &CycleCompletion<'_>) -> Option<String> {
        let record = cx.record;
        let (low, high) = if cx.slugcat == SlugcatName::Yellow {
            (0.42, 0.9)
        } else {
            (0.1, 0.8)
        };
        let like = ((cx.scavenger_like - low) / (high - low)).clamp(0.0, 1.0);
        let like = (like * 20.0).floor() / 20.0;
        let no_kills = record.kills.iter().all(|k| k.counts_as_a_kill <= 0);
        let big_kills = record
            .kills
            .iter()
            .filter(|k| k.counts_as_a_kill > 1)
            .count();
        let big_kills = i32::try_from(big_kills).unwrap_or(i32::MAX);
        let mut lizards: Vec<usize> = Vec::new();
        for i in record
            .kills
            .iter()
            .filter(|k| k.lizard)
            .filter_map(|k| k.lizard_index)
        {
            if !lizards.contains(&i) {
                lizards.push(i);
            }
        }
        let slept = record.went_to_sleep_in_region.as_deref().unwrap_or("");
        let region_index = EndgameId::STORY_REGIONS.iter().position(|&r| r == slept);
        let menu_region = region_index.map(|_| slept.to_owned());
        let mut pearls: Vec<i32> = Vec::new();
        for p in &record.pearls_found {
            if !pearls.contains(p) {
                pearls.push(*p);
            }
        }

        let survivor = self.tracker(EndgameId::Survivor, true)?;
        if cx.karma >= cx.karma_cap - 1 && cx.karma_cap >= 4 {
            let p = self.trackers[survivor].integer() + 1;
            self.trackers[survivor].set_integer(p);
        }
        if !self.trackers[survivor].goal_already_fulfilled() {
            return menu_region;
        }
        self.streak(EndgameId::Hunter, record.carnivorous, record.ate_anything);
        if let Some(i) = self.tracker(EndgameId::Saint, record.peaceful) {
            if record.peaceful {
                let p = self.trackers[i].integer() + 2;
                self.trackers[i].set_integer(p);
            } else if !self.trackers[i].goal_already_fulfilled() {
                self.trackers[i].set_integer(0);
            }
        }
        if let Some(r) = region_index
            && let Some(i) = self.tracker(EndgameId::Traveller, true)
        {
            self.trackers[i].parse_raw();
            if let Progress::Bools { progress, .. } = &mut self.trackers[i].progress
                && let Some(b) = progress.get_mut(r)
            {
                *b = true;
            }
        }
        if let Some(i) = self.tracker(EndgameId::Chieftain, like > 0.0) {
            self.trackers[i].set_float(like);
        }
        self.streak(EndgameId::Monk, record.vegetarian, record.ate_anything);
        if let Some(i) = self.tracker(EndgameId::Outlaw, big_kills > 0) {
            if big_kills > 0 {
                let p = self.trackers[i].integer() + big_kills;
                self.trackers[i].set_integer(p);
            } else if !self.trackers[i].goal_already_fulfilled() && no_kills {
                self.trackers[i].set_integer(0);
            }
        }
        if !lizards.is_empty()
            && let Some(i) = self.tracker(EndgameId::DragonSlayer, true)
        {
            self.trackers[i].parse_raw();
            if let Progress::Bools { progress, .. } = &mut self.trackers[i].progress {
                for l in &lizards {
                    if let Some(b) = progress.get_mut(*l) {
                        *b = true;
                    }
                }
            }
        }
        if !pearls.is_empty()
            && let Some(i) = self.tracker(EndgameId::Scholar, true)
        {
            self.trackers[i].parse_raw();
            if let Progress::List { progress, .. } = &mut self.trackers[i].progress {
                for p in pearls {
                    if progress.len() < EndgameId::LIST_MAX && !progress.contains(&p) {
                        progress.push(p);
                    }
                }
            }
        }
        // No friend in the den (`num4 == -1`).
        if let Some(i) = self.tracker(EndgameId::Friend, false)
            && !self.trackers[i].goal_already_fulfilled()
        {
            self.trackers[i].set_float(0.0);
        }
        menu_region
    }

    /// The Hunter's and the Monk's rule: +2 for a cycle of only meat (only plants)
    /// with something eaten, back to 0 otherwise unless already fulfilled.
    fn streak(&mut self, id: EndgameId, kept: bool, ate: bool) {
        let Some(i) = self.tracker(id, kept) else {
            return;
        };
        if kept {
            if ate {
                let p = self.trackers[i].integer() + 2;
                self.trackers[i].set_integer(p);
            }
        } else if !self.trackers[i].goal_already_fulfilled() {
            self.trackers[i].set_integer(0);
        }
    }

    /// `PlayerDied`: the death screen's penalty, kept in memory (`DeathModifyTracker`
    /// on each tracker not already fulfilled as last shown; unclamped).
    pub fn player_died(&mut self) {
        for t in &mut self.trackers {
            if !t.id.loses_on_death() || t.goal_already_fulfilled() {
                continue;
            }
            if let Progress::Integer { progress, .. } = &mut t.progress {
                *progress -= 1;
            }
        }
    }

    /// `GetNextEndGame`: the first fulfilled passage not yet used.
    #[must_use]
    pub fn next_end_game(&self) -> Option<EndgameId> {
        self.trackers
            .iter()
            .find(|t| t.goal_fulfilled() && !t.consumed)
            .map(|t| t.id)
    }

    /// `ResetLastShownValues` (the list trackers are left as they are).
    pub fn reset_last_shown_values(&mut self) {
        for t in &mut self.trackers {
            match &mut t.progress {
                Progress::Integer {
                    progress,
                    last_shown,
                } => *last_shown = *progress,
                Progress::Float {
                    progress,
                    last_shown,
                    ..
                } => *last_shown = *progress,
                Progress::Bools {
                    progress,
                    last_shown,
                } => {
                    for (l, p) in last_shown.iter_mut().zip(progress.iter()) {
                        *l = *p;
                    }
                }
                Progress::List { .. } | Progress::Raw(_) => {}
            }
        }
    }

    /// `SaveToString(saveAsIfPlayerDied)`: on a death, the trackers that lose a point
    /// are written a point lower unless already fulfilled as last shown.
    #[must_use]
    pub fn save_to_string(&self, died: bool) -> String {
        let mut text = String::new();
        for t in &self.trackers {
            let progress = match t.progress {
                Progress::Integer {
                    progress,
                    last_shown,
                } if died && t.id.loses_on_death() && !t.goal_already_fulfilled() => {
                    Progress::Integer {
                        progress: progress - 1,
                        last_shown,
                    }
                }
                _ => t.progress.clone(),
            };
            if t.any_progress_to_save(&progress) {
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

    fn record(carnivorous: bool) -> PlayerSessionRecord {
        PlayerSessionRecord {
            ate_anything: true,
            vegetarian: !carnivorous,
            carnivorous,
            went_to_sleep_in_region: Some("SU".to_owned()),
            ..PlayerSessionRecord::default()
        }
    }

    #[test]
    fn first_cycle_adds_an_empty_survivor() {
        let mut w = WinState::default();
        let r = record(true);
        let cx = CycleCompletion {
            slugcat: SlugcatName::White,
            karma: 0,
            karma_cap: 4,
            record: &r,
            scavenger_like: 0.0,
        };
        assert_eq!(w.cycle_completed(&cx), Some("SU".to_owned()));
        assert_eq!(w.trackers.len(), 1);
        assert_eq!(w.save_to_string(false), "");
        // At karma 3 of 4 the Survivor counts.
        let cx = CycleCompletion { karma: 3, ..cx };
        w.cycle_completed(&cx);
        assert_eq!(w.save_to_string(false), "Survivor<egA>0<egA>1<wsA>");
    }

    #[test]
    fn after_the_survivor_the_streaks_and_traveller_count() {
        let mut w = WinState::from_string(
            "Survivor<egA>1<egA>5<wsA>Monk<egA>0<egA>4<wsA>Traveller<egA>0<egA>0.0.0.1.<wsA>",
        );
        let r = record(true);
        let cx = CycleCompletion {
            slugcat: SlugcatName::White,
            karma: 0,
            karma_cap: 5,
            record: &r,
            scavenger_like: 0.0,
        };
        w.cycle_completed(&cx);
        // Survivor stays at 5; Monk breaks to 0; Hunter +2, Saint +2 (peaceful);
        // Traveller marks SU in all 12 slots.
        assert_eq!(
            w.save_to_string(false),
            "Survivor<egA>1<egA>5<wsA>Monk<egA>0<egA>0<wsA>\
             Traveller<egA>0<egA>1.0.0.1.0.0.0.0.0.0.0.0.<wsA>Hunter<egA>0<egA>2<wsA>\
             Saint<egA>0<egA>2<wsA>"
        );
    }

    #[test]
    fn death_screen_penalty_and_display() {
        let mut w = WinState::from_string(
            "Survivor<egA>0<egA>5<wsA>Hunter<egA>0<egA>3<wsA>Monk<egA>0<egA>12<wsA>\
             Outlaw<egA>0<egA>2<wsA>Chieftain<egA>0<egA>0.05<wsA>Traveller<egA>0<egA>0.0.1.<wsA>",
        );
        // `showFrom` 2 for Hunter and Outlaw, 0 for Survivor and Chieftain.
        let shown: Vec<bool> = w
            .trackers
            .iter()
            .map(EndgameTracker::any_progress_to_show)
            .collect();
        assert_eq!(shown, [true, true, true, false, true, true]);
        assert_eq!(w.next_end_game(), Some(EndgameId::Survivor));
        w.player_died();
        // Hunter and Outlaw lose a point (unclamped); the fulfilled Monk and the
        // Survivor do not.
        let p: Vec<Progress> = w
            .trackers
            .iter()
            .take(4)
            .map(EndgameTracker::parsed)
            .collect();
        assert_eq!(
            p,
            [
                Progress::Integer {
                    progress: 5,
                    last_shown: 5
                },
                Progress::Integer {
                    progress: 2,
                    last_shown: 3
                },
                Progress::Integer {
                    progress: 12,
                    last_shown: 12
                },
                Progress::Integer {
                    progress: 1,
                    last_shown: 2
                },
            ]
        );
        // Hunter still shows: last shown 3 is above 2.
        assert!(w.trackers[1].any_progress_to_show());
        w.reset_last_shown_values();
        assert!(!w.trackers[1].any_progress_to_show());
        assert!(!w.trackers[5].goal_fulfilled());
        w.trackers[0].consumed = true;
        assert_eq!(w.next_end_game(), Some(EndgameId::Monk));
    }

    #[test]
    fn death_save_spares_trackers_fulfilled_as_last_shown() {
        let mut w = WinState::from_string("Hunter<egA>0<egA>11<wsA>");
        // Brought to 12 this session but last shown at 11: still loses a point.
        w.trackers[0].set_integer(12);
        assert_eq!(w.save_to_string(true), "Hunter<egA>0<egA>11<wsA>");
    }
}
