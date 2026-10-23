//! `ThreatTracker`'s state and the threat of a tile or area from its threat points.
//! The threat creatures' updates (they read the tracker and the world) are
//! `world/ai_modules.rs`; `FleeTo`, `GenerateRandomPath`, `EvaluateFlightDestThreat`
//! and `FindMostAttractiveExit` are `world/threat_flight.rs`. Decompile:
//! `ThreatTracker.cs`.
//!
//! Mirrors: ThreatTracker.cs

use super::path_finder::EIGHT_DIRECTIONS_AND_ZERO;
use super::{CreatureTemplate, RepKey, WorldCoordinate};
use crate::FloatExt;
use crate::world::AiMap;

/// `ThreatTracker.ThreatPoint`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatPoint {
    /// Its identity (the game keeps object references to it).
    pub id: u32,
    pub crit: Option<CreatureTemplate>,
    pub pos: WorldCoordinate,
    pub severity: f32,
}

/// `ThreatTracker.ThreatCreature`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatCreature {
    pub rep: RepKey,
    pub severity: f32,
    /// Its threat point's id.
    pub point: u32,
}

impl ThreatCreature {
    /// `CurrentThreat` from the representation's `EstimatedChanceOfFinding` (0 once
    /// it is deleted).
    #[must_use]
    pub fn current_threat(&self, chance_of_finding: f32, deleted: bool) -> f32 {
        if deleted {
            return 0.0;
        }
        (chance_of_finding.mathf_pow(0.75) * self.severity).inverse_lerp(0.05, 0.5)
    }
}

/// `ThreatTracker`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreatTracker {
    /// The room of `aiMap` (the last `NewRoom`'s), -1 before one.
    pub map_room: i32,
    pub points: Vec<ThreatPoint>,
    pub creatures: Vec<ThreatCreature>,
    pub most_threatening: Option<RepKey>,
    pub saved_flee_dest: WorldCoordinate,
    pub test_flee_dest: WorldCoordinate,
    pub anti_flicker_counter: i32,
    pub reset_counter: i32,
    pub max_remembered_creatures: usize,
    pub accessibility_consideration: f32,
    pub current_threat: f32,
    /// The threat of the test destination at the last `FleeTo` (port-only, for MCP).
    pub flee_threat: f32,
    next_point: u32,
}

impl ThreatTracker {
    #[must_use]
    pub fn new(max_remembered_creatures: usize) -> Self {
        Self {
            map_room: -1,
            points: Vec::new(),
            creatures: Vec::new(),
            most_threatening: None,
            saved_flee_dest: WorldCoordinate::new(0, 0, 0, 0),
            test_flee_dest: WorldCoordinate::new(0, 0, 0, 0),
            anti_flicker_counter: 0,
            reset_counter: 0,
            max_remembered_creatures,
            accessibility_consideration: 5.0,
            current_threat: 0.0,
            flee_threat: 0.0,
            next_point: 0,
        }
    }

    /// `Utility`.
    #[must_use]
    pub fn utility(&self) -> f32 {
        (-0.01f32).lerp(1.01, self.current_threat).clamp(0.0, 1.0)
    }

    /// `AddThreatPoint`.
    pub fn add_point(
        &mut self,
        crit: Option<CreatureTemplate>,
        pos: WorldCoordinate,
        severity: f32,
    ) -> u32 {
        let id = self.next_point;
        self.next_point += 1;
        self.points.push(ThreatPoint {
            id,
            crit,
            pos,
            severity,
        });
        id
    }

    pub fn point_mut(&mut self, id: u32) -> Option<&mut ThreatPoint> {
        self.points.iter_mut().find(|p| p.id == id)
    }

    /// `ThreatCreature.Destroy`'s own part: its point and itself removed.
    pub fn remove_creature(&mut self, rep: RepKey) {
        if let Some(i) = self.creatures.iter().position(|c| c.rep == rep) {
            let point = self.creatures[i].point;
            self.points.retain(|p| p.id != point);
            self.creatures.remove(i);
        }
    }

    #[must_use]
    pub fn reps(&self) -> Vec<RepKey> {
        self.creatures.iter().map(|c| c.rep).collect()
    }

    /// `ThreatOfTile(coord)` (the accessibility flag is unused) for a creature in room
    /// `ai_room`, `map` the tracker's AI map: each threat point in the room adds
    /// `severity^1.5 * 10 / max(1, dist^1.25)`, scaled by how accessible the tile is to
    /// the threat's creature (more so beyond 2 tiles) and, for the strongest threats,
    /// by the tile's visibility.
    #[must_use]
    pub fn threat_of_tile(&self, coord: WorldCoordinate, ai_room: i32, map: Option<&AiMap>) -> f32 {
        if coord.room != ai_room {
            return 0.0;
        }
        let mut num = 0.0f32;
        for p in self.points.iter().filter(|p| p.pos.room == ai_room) {
            let Some(map) = map else {
                continue;
            };
            let dx = (coord.tile.0 - p.pos.tile.0) as f32;
            let dy = (coord.tile.1 - p.pos.tile.1) as f32;
            let mut d = (dx.mathf_pow(2.0) + dy.mathf_pow(2.0)).sqrt();
            let near = d.inverse_lerp(2.0, 10.0).clamp(0.0, 1.0);
            d = d.mathf_pow(1.25);
            let mut v = p.severity.mathf_pow(1.5) * 10.0 / d.max(1.0);
            if let Some(t) = &p.crit {
                let acc = map.accessibility_for_creature(coord.tile, t);
                if acc > 0.0 {
                    v *= acc.mathf_pow(self.accessibility_consideration * near);
                }
            }
            let vis = map.tile_at(coord).visibility as f32 / (map.width() * map.height()) as f32;
            v *= 1.0f32.lerp(vis, v.inverse_lerp(15.0, 25.0));
            num += v;
        }
        num
    }

    /// `ThreatOfArea`: the mean threat of the tile and its eight neighbours.
    #[must_use]
    pub fn threat_of_area(&self, coord: WorldCoordinate, ai_room: i32, map: Option<&AiMap>) -> f32 {
        let mut num = 0.0;
        for (dx, dy) in EIGHT_DIRECTIONS_AND_ZERO {
            let c = WorldCoordinate {
                tile: (coord.tile.0 + dx, coord.tile.1 + dy),
                ..coord
            };
            num += self.threat_of_tile(c, ai_room, map);
        }
        num / 9.0
    }
}

#[cfg(test)]
#[path = "tests/threat_tracker.rs"]
mod tests;
