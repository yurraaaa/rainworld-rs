//! `MissionTracker`'s state: the missions an AI is acting out (the base game has one
//! kind, `LeaveRoom`, given to a lizard that submits to another), each forgotten when
//! its counter runs out; the most useful is the tracker's utility. `LeaveRoom.Act`,
//! which reads the world, is `world/missions.rs`.
//!
//! Mirrors: MissionTracker.cs (Mission, LeaveRoom, Update, AddMission, Utility)

use super::WorldCoordinate;

/// The mission's class (`GetType()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionKind {
    LeaveRoom,
}

impl MissionKind {
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::LeaveRoom => "LeaveRoom",
        }
    }
}

/// `MissionTracker.LeaveRoom`'s own fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LeaveRoom {
    pub destination: Option<WorldCoordinate>,
    pub dest_unreachable: i32,
}

/// The mission's subclass and its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionData {
    LeaveRoom(LeaveRoom),
}

/// `MissionTracker.Mission`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mission {
    /// Port-only identity (the game holds the object itself as `mostRelevantMission`).
    pub serial: u32,
    pub data: MissionData,
    pub base_utility: f32,
    pub current_utility: f32,
    pub delete_me_next_frame: bool,
    /// -1: never forgotten.
    pub forget_counter: i32,
}

impl Mission {
    /// `new LeaveRoom(tracker, utility, forgetCounter)`: abandoned at once in a
    /// single-room world.
    #[must_use]
    pub fn leave_room(utility: f32, forget_counter: i32, single_room_world: bool) -> Self {
        log::debug!("Start mission! LeaveRoom");
        let mut m = Self {
            serial: 0,
            data: MissionData::LeaveRoom(LeaveRoom::default()),
            base_utility: utility,
            current_utility: utility,
            delete_me_next_frame: false,
            forget_counter,
        };
        if single_room_world {
            m.abandon();
        }
        m
    }

    #[must_use]
    pub fn kind(&self) -> MissionKind {
        match self.data {
            MissionData::LeaveRoom(_) => MissionKind::LeaveRoom,
        }
    }

    /// `UpdateUtility` (`LeaveRoom`'s adds nothing): the forget counter runs down and
    /// the mission is abandoned under 1.
    pub fn update_utility(&mut self) {
        if self.forget_counter > -1 {
            self.forget_counter -= 1;
            if self.forget_counter < 1 {
                self.abandon();
            }
        }
    }

    pub fn abandon(&mut self) {
        log::debug!("Abandon mission! {}", self.kind().value());
        self.delete_me_next_frame = true;
    }
}

/// `MissionTracker`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MissionTracker {
    pub missions: Vec<Mission>,
    utility: f32,
    /// `mostRelevantMission`'s serial.
    pub most_relevant: Option<u32>,
    /// The most relevant mission when `AddMission` has taken it out of the list since
    /// the last `Update`: the game still acts on that object until then.
    pub detached: Option<Mission>,
    next_serial: u32,
}

impl MissionTracker {
    /// `Update`: each mission (last first) updates its utility; abandoned ones go; the
    /// most useful one left (strictly above 0) is the most relevant.
    pub fn update(&mut self) {
        self.utility = 0.0;
        self.most_relevant = None;
        self.detached = None;
        for i in (0..self.missions.len()).rev() {
            self.missions[i].update_utility();
            if self.missions[i].delete_me_next_frame {
                self.missions.remove(i);
            } else if self.missions[i].current_utility > self.utility {
                self.utility = self.missions[i].current_utility;
                self.most_relevant = Some(self.missions[i].serial);
            }
        }
    }

    /// `AddMission(mission, canCoexistWithMissionOfSameType)`: unless it can coexist,
    /// refused when a mission of its kind is above the tracker's utility, else the
    /// missions of its kind are abandoned and removed first.
    pub fn add(&mut self, mut mission: Mission, can_coexist: bool) {
        if !can_coexist {
            let kind = mission.kind();
            if self
                .missions
                .iter()
                .any(|m| m.kind() == kind && m.current_utility > self.utility)
            {
                return;
            }
            for i in (0..self.missions.len()).rev() {
                if self.missions[i].kind() == kind {
                    let mut m = self.missions.remove(i);
                    m.abandon();
                    if Some(m.serial) == self.most_relevant {
                        self.detached = Some(m);
                    }
                }
            }
        }
        self.next_serial += 1;
        mission.serial = self.next_serial;
        self.missions.push(mission);
    }

    /// `mostRelevantMission`, wherever it is held.
    #[must_use]
    pub fn most_relevant(&self) -> Option<&Mission> {
        let s = self.most_relevant?;
        self.missions
            .iter()
            .chain(self.detached.iter())
            .find(|m| m.serial == s)
    }

    pub fn most_relevant_mut(&mut self) -> Option<&mut Mission> {
        let s = self.most_relevant?;
        self.missions
            .iter_mut()
            .chain(self.detached.iter_mut())
            .find(|m| m.serial == s)
    }

    /// `Utility`.
    #[must_use]
    pub fn utility(&self) -> f32 {
        self.utility
    }
}

#[cfg(test)]
#[path = "tests/mission_tracker.rs"]
mod tests;
