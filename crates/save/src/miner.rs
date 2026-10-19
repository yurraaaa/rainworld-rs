//! `SaveStateMiner`: pulls single values out of a stored `SAVE STATE` string without
//! loading it (the slugcat select menu's `MineForSaveData`).
//!
//! Mirrors: SaveStateMiner.cs

/// `SaveStateMiner.Target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinerTarget {
    /// The key with its leading `>` (the end of the section separator before it).
    pub data_id: &'static str,
    /// `None` for a flag, which is found or not.
    pub divider: Option<&'static str>,
    pub cap: &'static str,
    /// How far past the search string the cap may end.
    pub max_chars: usize,
}

/// `SaveStateMiner.Result`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinerResult {
    pub name: String,
    /// `None` for a flag.
    pub data: Option<String>,
}

/// `SaveStateMiner`.
pub struct SaveStateMiner;

impl MinerTarget {
    #[must_use]
    pub const fn new(
        data_id: &'static str,
        divider: Option<&'static str>,
        cap: &'static str,
        max_chars: usize,
    ) -> Self {
        Self {
            data_id,
            divider,
            cap,
            max_chars,
        }
    }

    /// `SearchString`.
    #[must_use]
    pub fn search_string(&self) -> String {
        format!("{}{}", self.data_id, self.divider.unwrap_or(""))
    }
}

impl SaveStateMiner {
    /// `Mine`: for each target in order, its first occurrence, cut at the first cap
    /// that ends within `max_chars` of the search string; a target not found, or whose
    /// cap is too far, gives nothing. A valued target's result is the text before and
    /// after its divider (to the next divider, as `Regex.Split` cuts).
    #[must_use]
    pub fn mine(save_state: &str, targets: &[MinerTarget]) -> Vec<MinerResult> {
        let mut results = Vec::new();
        for target in targets {
            let search = target.search_string();
            let Some(start) = save_state.find(&search) else {
                log::warn!("Savestate miner couldn't find: {search}");
                continue;
            };
            let mut end = start + (search.len() + target.max_chars).min(save_state.len() - start);
            while !save_state.is_char_boundary(end) {
                end -= 1;
            }
            let window = &save_state[start..end];
            let Some(end) = window.find(target.cap) else {
                log::warn!(
                    "Savestate miner couldn't find cap for {search} (cap: {}, length: {})",
                    target.cap,
                    target.max_chars
                );
                continue;
            };
            let input = &window[..end];
            let Some(divider) = target.divider else {
                results.push(MinerResult {
                    name: target.data_id.to_owned(),
                    data: None,
                });
                continue;
            };
            let mut parts = input.split(divider);
            if let (Some(name), Some(data)) = (parts.next(), parts.next()) {
                results.push(MinerResult {
                    name: name.to_owned(),
                    data: Some(data.to_owned()),
                });
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = "SAV STATE NUMBER<svB>White<svA>SEED<svB>1234<svA>DENPOS<svB>SU_S01<svA>\
        CYCLENUM<svB>12<svA>FOOD<svB>3<svA>HASTHEGLOW<svA>DEATHPERSISTENTSAVEDATA<svB>\
        KARMA<dpB>2<dpA>KARMACAP<dpB>6<dpA>REINFORCEDKARMA<dpB>1<dpA><svA>";

    #[test]
    fn values_flags_and_misses() {
        let targets = [
            MinerTarget::new(">DENPOS", Some("<svB>"), "<svA>", 35),
            MinerTarget::new(">CYCLENUM", Some("<svB>"), "<svA>", 50),
            MinerTarget::new(">HASTHEGLOW", None, "<svA>", 20),
            MinerTarget::new(">KARMA", Some("<dpB>"), "<dpA>", 20),
            MinerTarget::new(">KARMACAP", Some("<dpB>"), "<dpA>", 20),
            MinerTarget::new(">REINFORCEDKARMA", Some("<dpB>"), "<dpA>", 20),
            MinerTarget::new(">HASTHEMARK", None, "<dpA>", 20),
        ];
        let got = SaveStateMiner::mine(STATE, &targets);
        let pairs: Vec<(&str, Option<&str>)> = got
            .iter()
            .map(|r| (r.name.as_str(), r.data.as_deref()))
            .collect();
        assert_eq!(
            pairs,
            [
                (">DENPOS", Some("SU_S01")),
                (">CYCLENUM", Some("12")),
                (">HASTHEGLOW", None),
                (">KARMA", Some("2")),
                (">KARMACAP", Some("6")),
                (">REINFORCEDKARMA", Some("1")),
            ]
        );
    }

    #[test]
    fn a_cap_past_max_chars_is_a_miss() {
        // ">FOOD<svB>" is 10 characters; the cap starts 6 after it and ends at 11.
        let state = "x<svA>FOOD<svB>123456<svA>";
        assert!(
            SaveStateMiner::mine(
                state,
                &[MinerTarget::new(">FOOD", Some("<svB>"), "<svA>", 10)]
            )
            .is_empty()
        );
        let got = SaveStateMiner::mine(
            state,
            &[MinerTarget::new(">FOOD", Some("<svB>"), "<svA>", 11)],
        );
        assert_eq!(got[0].data.as_deref(), Some("123456"));
    }
}
