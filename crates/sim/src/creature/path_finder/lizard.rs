//! The breed-specific parts of a path finder: `LizardPather.HeuristicForCell` and
//! `LizardAI.TravelPreference` (its terrain part; `FollowPath` is
//! [`super::PathFinders::follow_path`]).

use rw_assets::CreatureType;

use super::{CellRef, PathEnv, PathFinder};
use crate::world::{AiMap, Legality, MovementConnection, PathCost};

/// Which `PathFinder` subclass and AI a path finder serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatherKind {
    /// `LizardPather` under a `LizardAI`, by breed.
    Lizard(CreatureType),
}

impl PatherKind {
    /// `HeuristicForCell`. A lizard's in its room: a swimmer in water by the tile
    /// distance to the creature, a cell an impossible path has not reached by 5.5 times
    /// the cost, else half the cost plus the Manhattan distance to the creature. The
    /// yellow lizard keeps the cost.
    pub(crate) fn heuristic_for_cell(
        self,
        pf: &PathFinder,
        env: &PathEnv<'_>,
        r: CellRef,
        to_goal: PathCost,
    ) -> PathCost {
        let Self::Lizard(kind) = self;
        let cell = pf.cell(r);
        if kind == CreatureType::YellowLizard || !pf.in_this_realized_room(cell.coord) {
            return to_goal;
        }
        let (x, y) = cell.coord.tile;
        let (px, py) = env.pos.tile;
        if kind == CreatureType::Salamander
            && pf.map().is_some_and(|m| m.tile_at(cell.coord).any_water())
        {
            let (dx, dy) = ((x - px) as f32, (y - py) as f32);
            return PathCost::new((dx * dx + dy * dy).sqrt(), to_goal.legality);
        }
        if pf.looking_for_impossible_path && !cell.reachable {
            return PathCost::new(to_goal.resistance * 5.5, to_goal.legality);
        }
        let manhattan = (x - px).abs() + (y - py).abs();
        PathCost::new(
            to_goal.resistance * 0.5 + manhattan as f32,
            to_goal.legality,
        )
    }

    /// `TravelPreference` of a connection inside the room: the base's discomfort (none
    /// without tracked creatures), then the breed's terrain taste: green +10 into
    /// narrow spaces, blue `30 - 3 * floorAltitude` (0-10), pink `2 * floorAltitude`,
    /// a swimmer +5 out of water. The obstacle and threat terms are lizard step 6b
    /// and the yellow lizard's `YellowAI` is not ported (both add nothing here).
    pub(crate) fn travel_preference(
        self,
        map: &AiMap,
        connection: &MovementConnection,
        cost: PathCost,
    ) -> PathCost {
        let Self::Lizard(kind) = self;
        let mut cost = cost;
        let dest = map.tile_at(connection.dest);
        match kind {
            CreatureType::GreenLizard => {
                if dest.narrow_space {
                    cost = cost + PathCost::new(10.0, Legality::Allowed);
                }
            }
            CreatureType::BlueLizard => {
                let v = (30 - dest.floor_altitude * 3) as f32;
                cost = cost + PathCost::new(v.clamp(0.0, 10.0), Legality::Allowed);
            }
            CreatureType::PinkLizard => {
                cost = cost + PathCost::new(dest.floor_altitude as f32 * 2.0, Legality::Allowed);
            }
            CreatureType::Salamander if !dest.any_water() => cost.resistance += 5.0,
            _ => {}
        }
        cost
    }
}
