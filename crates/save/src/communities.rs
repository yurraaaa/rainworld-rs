//! `CreatureCommunities` of a story session: the player's standing per community and
//! region (`COMMUNITIES<svB>SCAVSHY<coB>x<coA>All<coB>EVERY:0|CC:0|...`).

use rw_assets::SlugcatName;

use crate::cs_float::CsFloat;

/// `CreatureCommunities.CommunityID` of the base game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunityId {
    All,
    Scavengers,
    Lizards,
    Cicadas,
    GarbageWorms,
    Deer,
    JetFish,
}

impl CommunityId {
    /// Registration order after `None` (index 1..).
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Scavengers,
        Self::Lizards,
        Self::Cicadas,
        Self::GarbageWorms,
        Self::Deer,
        Self::JetFish,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Scavengers => "Scavengers",
            Self::Lizards => "Lizards",
            Self::Cicadas => "Cicadas",
            Self::GarbageWorms => "GarbageWorms",
            Self::Deer => "Deer",
            Self::JetFish => "JetFish",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.value() == value)
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }
}

/// A story session's communities (one player).
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureCommunities {
    /// `progression.regionNames`: region `i` is opinion column `i + 1`, column 0 is
    /// everywhere.
    regions: Vec<String>,
    pub scavenger_shyness: f32,
    /// `playerOpinions[community, region + 1, 0]`.
    opinions: Vec<Vec<f32>>,
}

impl CreatureCommunities {
    /// The constructor: every opinion 0.
    #[must_use]
    pub fn new(regions: &[String]) -> Self {
        Self {
            regions: regions.to_vec(),
            scavenger_shyness: 0.0,
            opinions: vec![vec![0.0; regions.len() + 1]; CommunityId::ALL.len()],
        }
    }

    /// What `StoryGameSession`'s constructor loads: the saved string, else
    /// `LoadDefaultCommunityAlignments` from `World/default alignments.txt`.
    #[must_use]
    pub fn for_session(
        saved: Option<&str>,
        regions: &[String],
        default_alignments: &str,
        slugcat: SlugcatName,
    ) -> Self {
        let mut c = Self::new(regions);
        match saved {
            Some(s) => c.from_string(s),
            None => c.load_default_alignments(default_alignments, slugcat),
        }
        c
    }

    fn region_column(&self, region: &str) -> Option<usize> {
        self.regions.iter().position(|r| r == region).map(|i| i + 1)
    }

    /// `LoadDefaultCommunityAlignments`.
    pub fn load_default_alignments(&mut self, text: &str, slugcat: SlugcatName) {
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.len() <= 2 || line.starts_with("//") {
                continue;
            }
            let mut parts = line.split(" : ");
            let Some(community) = parts.next().and_then(CommunityId::from_value) else {
                continue;
            };
            let Some(values) = parts.next() else { continue };
            for pair in values.split(',').map(str::trim) {
                let mut kv = pair.split('=');
                let region = kv.next().unwrap_or("");
                let Some(Ok(n)) = kv.next().map(|v| v.trim().parse::<i32>()) else {
                    continue;
                };
                // An unknown region (or EVERYWHERE) is column 0.
                let column = if region == "EVERYWHERE" {
                    0
                } else {
                    self.region_column(region).unwrap_or(0)
                };
                self.opinions[community.index()][column] = n as f32 / 100.0;
            }
        }
        let (to, t) = match slugcat {
            SlugcatName::Yellow => (1.0, 0.25),
            SlugcatName::Red => (-1.0, 0.35),
            _ => return,
        };
        for row in &mut self.opinions {
            for v in row {
                *v += (to - *v) * t;
            }
        }
    }

    /// `FromString` of the `<coA>` format (the old `<ccA>` one is not read).
    pub fn from_string(&mut self, s: &str) {
        for entry in s.split("<coA>") {
            let mut kv = entry.split("<coB>");
            let key = kv.next().unwrap_or("");
            let value = kv.next().unwrap_or("");
            if key == "SCAVSHY" {
                self.scavenger_shyness = value.trim().parse().unwrap_or(0.0);
                continue;
            }
            let Some(community) = CommunityId::from_value(key) else {
                continue;
            };
            for cell in value.split('|') {
                let mut rc = cell.split(':');
                let region = rc.next().unwrap_or("");
                let Some(Ok(like)) = rc.next().map(|v| v.trim().parse::<f32>()) else {
                    continue;
                };
                let column = if region == "EVERY" {
                    Some(0)
                } else {
                    self.region_column(region)
                };
                if let Some(column) = column {
                    self.opinions[community.index()][column] = like.clamp(-1.0, 1.0);
                }
            }
        }
    }

    /// `CycleTick(cycle, saveStateNumber)` with More Slugcats off.
    pub fn cycle_tick(&mut self, cycle: i32, slugcat: SlugcatName) {
        if cycle > 5 {
            self.scavenger_shyness = (self.scavenger_shyness - 0.07).max(0.0);
        }
        for v in self.opinions.iter_mut().flatten() {
            if slugcat == SlugcatName::Yellow {
                if *v < 0.25 {
                    *v = (*v + 0.08).min(0.25);
                }
            } else if *v < 0.0 {
                // `LerpMap(v, -1, 0, 0.01, 0.1)`.
                let t = (*v + 1.0).clamp(0.0, 1.0);
                *v = (*v + 0.01 + (0.1 - 0.01) * t).min(0.0);
            }
        }
    }

    /// `LikeOfPlayer(Scavengers or All, -1, 0)` in a story session: the everywhere
    /// column through `ExponentMap(-1, 1, 1 + 0.2 * difficulty)`.
    #[must_use]
    pub fn like_of_player_everywhere(&self, community: CommunityId, difficulty: f32) -> f32 {
        let mut v = self.opinions[community.index()][0];
        if community != CommunityId::All && community != CommunityId::Scavengers {
            v = Self::minus_one_to_one_influence(v, self.opinions[0][0]);
        }
        let t = ((v + 1.0) / 2.0).clamp(0.0, 1.0);
        -1.0 + 2.0 * t.powf(1.0 + 0.2 * difficulty)
    }

    /// `Custom.MinusOneToOneRangeFloatInfluence`.
    fn minus_one_to_one_influence(f: f32, infl: f32) -> f32 {
        // `Mathf.Sign` is 1 at 0; `Math.Sign` is 0 there.
        let f_sign = if f >= 0.0 { 1.0 } else { -1.0 };
        let infl_sign = if infl > 0.0 {
            1.0
        } else if infl < 0.0 {
            -1.0
        } else {
            0.0
        };
        if f_sign != infl_sign || f.abs() < infl.abs() {
            f + (infl - f) * infl.abs().min(1.0)
        } else {
            f
        }
    }

    /// `ToString`.
    #[must_use]
    pub fn save_string(&self) -> String {
        let mut text = format!("SCAVSHY<coB>{}<coA>", self.scavenger_shyness.cs_string());
        for (i, community) in CommunityId::ALL.iter().enumerate() {
            text.push_str(community.value());
            text.push_str("<coB>");
            let row = &self.opinions[i];
            for (j, v) in row.iter().enumerate() {
                let region = if j == 0 {
                    "EVERY"
                } else {
                    &self.regions[j - 1]
                };
                text.push_str(&format!("{region}:{}", v.cs_string()));
                if j + 1 < row.len() {
                    text.push('|');
                }
            }
            if i + 1 < CommunityId::ALL.len() {
                text.push_str("<coA>");
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALIGNMENTS: &str =
        "//All - General\n\nLizards : SU=-3, GW=2, HI=3, CC=2, SB=-18, UW=-3, LF=-2\n";

    fn regions() -> Vec<String> {
        [
            "CC", "DS", "HI", "GW", "SI", "SU", "SH", "SL", "LF", "UW", "SB", "SS",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    #[test]
    fn default_alignments_tick_and_write() {
        let mut c =
            CreatureCommunities::for_session(None, &regions(), ALIGNMENTS, SlugcatName::White);
        let s = c.save_string();
        assert!(s.starts_with("SCAVSHY<coB>0<coA>All<coB>EVERY:0|CC:0|DS:0|"));
        assert!(s.contains(
            "<coA>Lizards<coB>EVERY:0|CC:0.02|DS:0|HI:0.03|GW:0.02|SI:0|SU:-0.03|SH:0|SL:0|LF:-0.02|UW:-0.03|SB:-0.18|SS:0<coA>Cicadas"
        ));
        assert!(s.ends_with(
            "JetFish<coB>EVERY:0|CC:0|DS:0|HI:0|GW:0|SI:0|SU:0|SH:0|SL:0|LF:0|UW:0|SB:0|SS:0"
        ));
        // Negative opinions move toward 0 by LerpMap(v, -1, 0, 0.01, 0.1).
        c.cycle_tick(0, SlugcatName::White);
        assert!(c.save_string().contains("SU:0|SH"));
        let again = CreatureCommunities::for_session(Some(&s), &regions(), "", SlugcatName::White);
        assert_eq!(again.save_string(), s);
    }
}
