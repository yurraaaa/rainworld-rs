//! `NoiseTracker`, the AI module that hears: the sources it believes its noises came
//! from (`TheorizedSource`), the mysterious noises and the sound to examine. Hearing a
//! noise and the module's update read the world and are `world/hearing.rs`. Decompile:
//! `NoiseTracker.cs`.
//!
//! Mirrors: NoiseTracker.cs

use super::RepKey;
use crate::{FloatExt, InGameNoise, Vec2};

/// How many heard noises [`NoiseTracker::heard`] keeps, and for how many ticks.
const HEARD_LOG: usize = 8;
const HEARD_LOG_TICKS: i32 = 400;

/// A noise the tracker heard (a record for inspection, not the game's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeardNoise {
    pub noise: InGameNoise,
    pub ticks_ago: i32,
    /// It was attributed to no tracked creature.
    pub mysterious: bool,
}

/// `NoiseTracker.TheorizedSource`.
#[derive(Debug, Clone, PartialEq)]
pub struct TheorizedSource {
    /// Tells the sources apart (`soundToExamine` refers to one).
    pub serial: u32,
    pub pos: Vec2,
    pub age: i32,
    pub slated_for_deletion: bool,
    /// `creatureRep`: the tracked creature it is believed to be.
    pub creature_rep: Option<RepKey>,
    /// `LASTSOUNDSTRENGTH` (read only by the game's debug view).
    pub last_sound_strength: f32,
}

impl TheorizedSource {
    /// `Update`: a tick older, letting go of a representation the tracker deletes,
    /// destroyed past `forget_time`; the remembered strength fades to 10.
    pub fn update(&mut self, forget_time: i32, rep_deleted: bool) {
        self.age += 1;
        if rep_deleted {
            self.creature_rep = None;
        }
        if self.age > forget_time {
            self.slated_for_deletion = true;
        }
        self.last_sound_strength = (self.last_sound_strength * 0.6).max(10.0);
    }

    /// `NoiseMatch` of a source without a creature: the distance plus 200, plus a fifth
    /// of its age.
    #[must_use]
    pub fn unknown_match(&self, noise_pos: Vec2) -> f32 {
        self.pos.distance(noise_pos) + 200.0 + self.age as f32 / 5.0
    }
}

/// What makes a noise more or less interesting to an AI (`HowInterestingIsThisNoiseToMe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseKind {
    /// A creature of the AI's own family (`TopAncestor`).
    Kin,
    Rock,
    Other,
}

impl NoiseKind {
    /// `HowInterestingIsThisNoiseToMe`: a tenth from kin, five times from a rock.
    #[must_use]
    pub fn interest(self, interesting: f32) -> f32 {
        match self {
            Self::Kin => interesting * 0.1,
            Self::Rock => interesting * 5.0,
            Self::Other => interesting,
        }
    }
}

/// `NoiseTracker`.
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseTracker {
    /// `room`, by index: -1 before the AI's first `NewRoom`.
    pub room: i32,
    pub sources: Vec<TheorizedSource>,
    pub ignore_seen_noises: bool,
    pub forget_time: i32,
    /// Set by the AI's update (0 until then).
    pub hearing_skill: f32,
    pub mysterious_noises: f32,
    pub mysterious_noise_counter: i32,
    /// `soundToExamine`, by serial.
    pub sound_to_examine: Option<u32>,
    /// The last noises heard, newest last (not the game's).
    pub heard: Vec<HeardNoise>,
    next_serial: u32,
}

impl Default for NoiseTracker {
    /// The constructor: sounds forgotten after 1200 ticks, seen noises ignored.
    fn default() -> Self {
        Self {
            room: -1,
            sources: Vec::new(),
            ignore_seen_noises: true,
            forget_time: 1200,
            hearing_skill: 0.0,
            mysterious_noises: 0.0,
            mysterious_noise_counter: 0,
            sound_to_examine: None,
            heard: Vec::new(),
            next_serial: 0,
        }
    }
}

impl NoiseTracker {
    /// `Utility`: with a sound to examine, `InverseLerp(1, 20, mysteriousNoises)^0.5`.
    #[must_use]
    pub fn utility(&self) -> f32 {
        if self.sound_to_examine.is_none() {
            return 0.0;
        }
        self.mysterious_noises
            .inverse_lerp(1.0, 20.0)
            .mathf_pow(0.5)
    }

    /// `NewRoom`: in another room the sources are forgotten (and with them the sound to
    /// examine).
    pub fn new_room(&mut self, room: i32) {
        if self.room != room {
            self.room = room;
            self.sources.clear();
            self.sound_to_examine = None;
        }
    }

    /// The end of `Update`: the mysterious noises wait 200 ticks after the last, then
    /// fade by 0.0125 a tick while above 0 (the game's `Mathf.Max` around the decrement
    /// is discarded, so they can end a little below 0).
    pub fn fade_mysterious_noises(&mut self) {
        if self.mysterious_noise_counter > 0 {
            self.mysterious_noise_counter -= 1;
        } else if self.mysterious_noises > 0.0 {
            self.mysterious_noises -= 0.0125;
        }
    }

    /// Notes a noise heard (for inspection).
    pub fn log_heard(&mut self, noise: InGameNoise, mysterious: bool) {
        if self.heard.len() == HEARD_LOG {
            self.heard.remove(0);
        }
        self.heard.push(HeardNoise {
            noise,
            ticks_ago: 0,
            mysterious,
        });
    }

    /// Ages the log of heard noises.
    pub fn age_heard(&mut self) {
        for h in &mut self.heard {
            h.ticks_ago += 1;
        }
        self.heard.retain(|h| h.ticks_ago <= HEARD_LOG_TICKS);
    }

    /// A mysterious noise was heard.
    pub fn add_mysterious(&mut self, interest: f32) {
        self.mysterious_noises += interest;
        self.mysterious_noise_counter = 200;
    }

    /// `ClearAllUnkown`: the sources not attributed to a creature are destroyed.
    pub fn clear_all_unknown(&mut self) {
        for s in &mut self.sources {
            if s.creature_rep.is_none() {
                s.slated_for_deletion = true;
            }
        }
    }

    /// A new `TheorizedSource` at `pos` (not yet refreshed); returns its index.
    pub fn add_source(&mut self, pos: Vec2, creature_rep: Option<RepKey>) -> usize {
        let serial = self.next_serial;
        self.next_serial += 1;
        self.sources.push(TheorizedSource {
            serial,
            pos,
            age: 0,
            slated_for_deletion: false,
            creature_rep,
            last_sound_strength: 0.0,
        });
        self.sources.len() - 1
    }

    /// `soundToExamine`.
    #[must_use]
    pub fn sound_to_examine(&self) -> Option<&TheorizedSource> {
        let serial = self.sound_to_examine?;
        self.sources.iter().find(|s| s.serial == serial)
    }

    /// The representations the sources refer to (kept while the tracker drops them).
    pub fn held_reps(&self) -> impl Iterator<Item = RepKey> + '_ {
        self.sources.iter().filter_map(|s| s.creature_rep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NoiseTracker.cs` 32-42: a tick older, the strength `Max(10, 0.6x)`, destroyed
    /// once older than the forget time, and the representation let go when deleted.
    #[test]
    fn source_update() {
        let mut t = NoiseTracker::default();
        let i = t.add_source(Vec2::new(10.0, 20.0), Some(RepKey(4)));
        let s = &mut t.sources[i];
        s.last_sound_strength = 150.0;
        s.update(3, false);
        assert_eq!((s.age, s.last_sound_strength), (1, 90.0));
        assert_eq!(s.creature_rep, Some(RepKey(4)));
        s.update(3, true);
        s.update(3, false);
        assert!(!s.slated_for_deletion);
        assert_eq!(s.creature_rep, None);
        s.update(3, false);
        assert!(s.slated_for_deletion);
        assert!((s.last_sound_strength - 19.44).abs() < 1e-3);
        s.update(3, false);
        s.update(3, false);
        assert_eq!(s.last_sound_strength, 10.0);
        assert_eq!(
            s.unknown_match(Vec2::new(10.0, 50.0)),
            30.0 + 200.0 + 6.0 / 5.0
        );
    }

    /// `Utility` (`NoiseTracker.cs` 155-162) and the fade at the end of `Update`.
    #[test]
    fn utility_and_fade() {
        let mut t = NoiseTracker {
            mysterious_noises: 5.0,
            ..NoiseTracker::default()
        };
        assert_eq!(t.utility(), 0.0);
        t.sound_to_examine = Some(0);
        assert!((t.utility() - (4.0f32 / 19.0).sqrt()).abs() < 1e-6);
        t.add_mysterious(0.0);
        assert_eq!(t.mysterious_noise_counter, 200);
        t.mysterious_noises = 0.01;
        t.mysterious_noise_counter = 1;
        t.fade_mysterious_noises();
        assert_eq!((t.mysterious_noise_counter, t.mysterious_noises), (0, 0.01));
        t.fade_mysterious_noises();
        assert!((t.mysterious_noises - (0.01 - 0.0125)).abs() < 1e-7);
        t.fade_mysterious_noises();
        assert!((t.mysterious_noises - (0.01 - 0.0125)).abs() < 1e-7);
    }

    /// `HowInterestingIsThisNoiseToMe` (`NoiseTracker.cs` 288-299).
    #[test]
    fn interest() {
        assert_eq!(NoiseKind::Kin.interest(4.0), 0.4);
        assert_eq!(NoiseKind::Rock.interest(4.0), 20.0);
        assert_eq!(NoiseKind::Other.interest(4.0), 4.0);
    }
}
