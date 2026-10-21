//! `DenFinder` (a `NodeFinder`): the state of the search for a den when the AI has
//! none. The den position itself is the abstract AI's `denPosition`. Its
//! `AbstractSpaceNodeFinder` is run to the end when started, and the result is kept
//! back for the number of `Update`s the game's one-node-a-tick search takes (the
//! cost flood makes no draws, so only the timing is the game's to keep). The updates
//! that read the world are `world/ai_modules.rs`. Decompile: `DenFinder.cs`,
//! `NodeFinder.cs`, `AbstractSpaceNodeFinder.cs` (Update).
//!
//! Mirrors: DenFinder.cs, NodeFinder.cs

use super::WorldCoordinate;

/// `NodeFinder.Status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeFinderStatus {
    Working,
    Found,
    NoAccessible,
}

impl NodeFinderStatus {
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Found => "Found",
            Self::NoAccessible => "NoAccessible",
        }
    }
}

/// A running `AbstractSpaceNodeFinder`: what it will return and after how many more
/// `Update`s.
#[derive(Debug, Clone, PartialEq)]
pub struct DenSearch {
    pub path: Option<Vec<WorldCoordinate>>,
    pub updates_left: i32,
}

/// `DenFinder`.
#[derive(Debug, Clone, PartialEq)]
pub struct DenFinder {
    pub status: NodeFinderStatus,
    pub search: Option<DenSearch>,
}

impl DenFinder {
    /// The constructor: found when the AI has a den position (else the caller resets
    /// the mapping).
    #[must_use]
    pub fn new(has_den: bool) -> Self {
        Self {
            status: if has_den {
                NodeFinderStatus::Found
            } else {
                NodeFinderStatus::Working
            },
            search: None,
        }
    }

    /// One `AbstractSpaceNodeFinder.Update` of the running search: its result once it
    /// has finished (the search is then dropped).
    pub fn step_search(&mut self) -> Option<Option<Vec<WorldCoordinate>>> {
        let s = self.search.as_mut()?;
        s.updates_left -= 1;
        if s.updates_left > 0 {
            return None;
        }
        self.search.take().map(|s| s.path)
    }

    /// `Finished(path)`: the den found (the goal, first) or none.
    pub fn finished(&mut self, path: Option<&[WorldCoordinate]>) -> Option<WorldCoordinate> {
        match path.and_then(|p| p.first()) {
            Some(&den) => {
                self.status = NodeFinderStatus::Found;
                Some(WorldCoordinate::new(den.room, -1, -1, den.node))
            }
            None => {
                self.status = NodeFinderStatus::NoAccessible;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A search the game takes three updates for finishes on the third.
    #[test]
    fn search_finishes_after_its_updates() {
        let mut f = DenFinder::new(false);
        f.search = Some(DenSearch {
            path: Some(vec![WorldCoordinate::new(3, 4, 5, 2)]),
            updates_left: 3,
        });
        assert_eq!(f.step_search(), None);
        assert_eq!(f.step_search(), None);
        let path = f.step_search().expect("finished");
        assert!(f.search.is_none());
        assert_eq!(
            f.finished(path.as_deref()),
            Some(WorldCoordinate::new(3, -1, -1, 2))
        );
        assert_eq!(f.status, NodeFinderStatus::Found);
        assert_eq!(f.finished(None), None);
        assert_eq!(f.status, NodeFinderStatus::NoAccessible);
    }
}
