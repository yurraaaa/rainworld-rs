//! `DreamsState` (the Survivor's and the Monk's): `DREAMSSTATE<svB>integersArray<dsB>...`.

/// `DreamsState.DreamID` of the base game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DreamId {
    MoonFriend,
    MoonThief,
    Pebbles,
    GuideA,
    GuideB,
    GuideC,
    FamilyA,
    FamilyB,
    FamilyC,
    VoidDreamSlugcatUp,
    VoidDreamSlugcatDown,
}

impl DreamId {
    pub const ALL: [Self; 11] = [
        Self::MoonFriend,
        Self::MoonThief,
        Self::Pebbles,
        Self::GuideA,
        Self::GuideB,
        Self::GuideC,
        Self::FamilyA,
        Self::FamilyB,
        Self::FamilyC,
        Self::VoidDreamSlugcatUp,
        Self::VoidDreamSlugcatDown,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::MoonFriend => "MoonFriend",
            Self::MoonThief => "MoonThief",
            Self::Pebbles => "Pebbles",
            Self::GuideA => "GuideA",
            Self::GuideB => "GuideB",
            Self::GuideC => "GuideC",
            Self::FamilyA => "FamilyA",
            Self::FamilyB => "FamilyB",
            Self::FamilyC => "FamilyC",
            Self::VoidDreamSlugcatUp => "VoidDreamSlugcatUp",
            Self::VoidDreamSlugcatDown => "VoidDreamSlugcatDown",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.value() == value)
    }

    /// `(int)dreamID`: the registration index.
    #[must_use]
    pub fn index(self) -> i32 {
        Self::ALL
            .iter()
            .position(|&d| d == self)
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(-1)
    }
}

/// What the end of a cycle reads besides the dreams state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DreamContext<'a> {
    /// `saveState.malnourished` (the new value, during `Win`).
    pub malnourished: bool,
    pub cycle_number: i32,
    /// `miscWorldSaveData.playerGuideState.likesPlayer`.
    pub guide_likes_player: f32,
    pub region: &'a str,
    pub den: &'a str,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DreamsState {
    /// `integers`: cycles since the last dream, family dream and guide dream, the family
    /// and guide threads, the GW/SH counter, slept in SB, slept in SB_S01, ate a Moon
    /// neuron.
    pub integers: [i32; 9],
    unrecognized_integers: Vec<i32>,
    unrecognized: Vec<String>,
    /// Runtime state, not saved.
    pub event_dream: Option<DreamId>,
    pub upcoming_dream: Option<DreamId>,
    pub guide_has_shown_moon_this_round: bool,
    pub guide_has_shown_himself_to_player: bool,
}

impl DreamsState {
    const SINCE_DREAM: usize = 0;
    const SINCE_FAMILY: usize = 1;
    const SINCE_GUIDE: usize = 2;
    const FAMILY_THREAD: usize = 3;
    const GUIDE_THREAD: usize = 4;
    const IN_GW_OR_SH: usize = 5;
    const SLEPT_IN_SB: usize = 6;
    const SLEPT_IN_SB_S01: usize = 7;

    /// `FromString`.
    pub fn from_string(&mut self, s: &str) {
        self.unrecognized.clear();
        for entry in s.split("<dsA>") {
            let parts: Vec<&str> = entry.split("<dsB>").collect();
            if parts[0] == "integersArray" {
                let values: Vec<i32> = parts
                    .get(1)
                    .unwrap_or(&"")
                    .split('.')
                    .map(|v| v.trim().parse().unwrap_or(0))
                    .collect();
                for (i, v) in values.iter().enumerate() {
                    if let Some(slot) = self.integers.get_mut(i) {
                        *slot = *v;
                    }
                }
                self.unrecognized_integers = values.get(9..).unwrap_or(&[]).to_vec();
            } else if !entry.trim().is_empty() && parts.len() >= 2 {
                self.unrecognized.push(entry.to_owned());
            }
        }
    }

    /// `ToString`.
    #[must_use]
    pub fn save_string(&self) -> String {
        let ints: Vec<String> = self
            .integers
            .iter()
            .chain(&self.unrecognized_integers)
            .map(i32::to_string)
            .collect();
        let mut text = format!("integersArray<dsB>{}<dsA>", ints.join("."));
        for u in &self.unrecognized {
            text.push_str(u);
            text.push_str("<dsA>");
        }
        text
    }

    /// `InitiateEventDream`.
    pub fn initiate_event_dream(&mut self, dream: DreamId) {
        if dream == DreamId::MoonThief {
            self.integers[8] = 1;
        }
        if self.event_dream.is_none_or(|e| dream > e) {
            self.event_dream = Some(dream);
        }
    }

    /// `EndOfCycleProgress` (`StaticEndOfCycleProgress`) with every DLC off.
    pub fn end_of_cycle_progress(&mut self, cx: &DreamContext<'_>) {
        if cx.malnourished {
            self.upcoming_dream = None;
            return;
        }
        let i = &mut self.integers;
        i[Self::SINCE_DREAM] += 1;
        i[Self::SINCE_FAMILY] += 1;
        i[Self::SINCE_GUIDE] += 1;
        if i[Self::IN_GW_OR_SH] > 0 {
            i[Self::IN_GW_OR_SH] += 1;
        } else if cx.region == "GW" || cx.region == "SH" {
            i[Self::IN_GW_OR_SH] = 1;
        }
        self.upcoming_dream = None;
        if let Some(event) = self.event_dream {
            i[Self::SINCE_DREAM] = 0;
            self.upcoming_dream = Some(event);
            return;
        }
        if i[Self::SLEPT_IN_SB] != 1 && cx.region == "SB" {
            i[Self::SLEPT_IN_SB] = 1;
            i[Self::SINCE_DREAM] = 0;
            self.upcoming_dream = Some(DreamId::VoidDreamSlugcatUp);
            return;
        }
        if i[Self::SLEPT_IN_SB_S01] != 1 && cx.den == "SB_S01" {
            i[Self::SLEPT_IN_SB_S01] = 1;
            i[Self::SINCE_DREAM] = 0;
            self.upcoming_dream = Some(DreamId::VoidDreamSlugcatDown);
            return;
        }
        if cx.cycle_number > 2
            && i[Self::SINCE_DREAM] > 1
            && self.guide_has_shown_himself_to_player
            && cx.guide_likes_player > -0.75
        {
            let thread = i[Self::GUIDE_THREAD];
            let dream = if thread == 0 {
                Some(DreamId::GuideA)
            } else if thread <= DreamId::GuideA.index()
                && (self.guide_has_shown_moon_this_round || i[Self::IN_GW_OR_SH] > 1)
            {
                Some(DreamId::GuideB)
            } else if thread <= DreamId::GuideB.index() && cx.region == "SL" {
                Some(DreamId::GuideC)
            } else {
                None
            };
            if let Some(dream) = dream {
                i[Self::GUIDE_THREAD] = dream.index();
                self.upcoming_dream = Some(dream);
                i[Self::SINCE_DREAM] = 0;
                i[Self::SINCE_GUIDE] = 0;
                return;
            }
        }
        if i[Self::SINCE_DREAM] <= 2 {
            return;
        }
        let since_family = i[Self::SINCE_FAMILY];
        let dream = match i[Self::FAMILY_THREAD] {
            0 if since_family > 6 => Some(DreamId::FamilyA),
            1 if since_family > 7 => Some(DreamId::FamilyB),
            2 if since_family > 6 => Some(DreamId::FamilyC),
            3 if since_family > 14 => Some(DreamId::VoidDreamSlugcatUp),
            _ => None,
        };
        if let Some(dream) = dream {
            i[Self::FAMILY_THREAD] += 1;
            self.upcoming_dream = Some(dream);
            i[Self::SINCE_DREAM] = 0;
            i[Self::SINCE_FAMILY] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_cycle_counts_up_without_a_dream() {
        let mut d = DreamsState::default();
        d.from_string("integersArray<dsB>0.0.0.0.0.0.0.0.0.5<dsA>X<dsB>y<dsA>");
        let cx = DreamContext {
            malnourished: false,
            cycle_number: 0,
            guide_likes_player: 0.3,
            region: "SU",
            den: "SU_S01",
        };
        d.end_of_cycle_progress(&cx);
        assert_eq!(d.upcoming_dream, None);
        assert_eq!(
            d.save_string(),
            "integersArray<dsB>1.1.1.0.0.0.0.0.0.5<dsA>X<dsB>y<dsA>"
        );
        // Family A after seven cycles without dreams.
        d.integers = [7, 7, 7, 0, 0, 0, 0, 0, 0];
        d.end_of_cycle_progress(&cx);
        assert_eq!(d.upcoming_dream, Some(DreamId::FamilyA));
        assert_eq!(&d.integers[..4], &[0, 0, 8, 1]);
    }
}
