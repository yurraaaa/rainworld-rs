//! The room's border exits (`ShortcutMapper.MapRoomBorders`, `RoomBorderExit`): runs
//! of open tiles along the room's edge that side, sky and sea highways enter by.
//!
//! Mirrors: ShortcutMapper.cs (MapRoomBorders), RoomBorderExit.cs

use crate::{NodeType, Room};

/// `RoomBorderExit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomBorderExit {
    pub tiles: Vec<(i32, i32)>,
    pub kind: NodeType,
}

/// One walk around the border (`MapRoomBorders`' loop state).
struct BorderWalk {
    pos: (i32, i32),
    dir: (i32, i32),
    run: Vec<(i32, i32)>,
    start: Option<(i32, i32)>,
    was_invalid: bool,
}

impl Room {
    /// `MapRoomBorders`: none in shelters and gates; otherwise one clockwise walk along
    /// the edge per type (side, sky, then sea when the room has water), from the bottom
    /// left corner up, each run of valid tiles closed by an invalid one becoming an exit.
    /// The walks share one 100000-step budget.
    #[must_use]
    pub fn border_exits(&self, shelter_or_gate: bool) -> Vec<RoomBorderExit> {
        let mut exits = Vec::new();
        if shelter_or_gate {
            return exits;
        }
        let mut steps = 0;
        for kind in [NodeType::SideExit, NodeType::SkyExit, NodeType::SeaExit] {
            if kind == NodeType::SeaExit && self.water_level().is_none() {
                break;
            }
            let mut w = BorderWalk {
                pos: (0, 0),
                dir: (0, 1),
                run: Vec::new(),
                start: None,
                was_invalid: false,
            };
            while steps < 100_000 {
                steps += 1;
                w.pos = (w.pos.0 + w.dir.0, w.pos.1 + w.dir.1);
                let mut started_here = false;
                if self.valid_border_tile(w.pos, kind) {
                    if w.was_invalid {
                        if w.start.is_none() {
                            w.start = Some(w.pos);
                            started_here = true;
                        } else if w.start == Some(w.pos) {
                            break;
                        }
                        w.run.clear();
                    }
                    if w.start.is_some() {
                        w.run.push(w.pos);
                    }
                } else if !w.was_invalid && w.start.is_some() && !w.run.is_empty() {
                    exits.push(RoomBorderExit {
                        tiles: w.run.clone(),
                        kind,
                    });
                }
                if w.dir.1 == 1 && w.pos.1 == self.height() - 1 {
                    w.dir = (1, 0);
                } else if w.dir.0 == 1 && w.pos.0 == self.width() - 1 {
                    w.dir = (0, -1);
                } else if w.dir.1 == -1 && w.pos.1 == 0 {
                    w.dir = (-1, 0);
                } else if w.dir.0 == -1 && w.pos.0 == 0 {
                    w.dir = (0, 1);
                    if w.start.is_none() {
                        w.start = Some(w.pos);
                    } else if Some(w.pos) == w.start && !started_here {
                        if !w.run.is_empty() {
                            exits.push(RoomBorderExit {
                                tiles: w.run.clone(),
                                kind,
                            });
                        }
                        break;
                    }
                }
                w.was_invalid = !self.valid_border_tile(w.pos, kind);
            }
        }
        exits
    }

    /// `ValidBorderTile`: open, and on the left or right edge out of water (side), on
    /// the top row (sky) or in water (sea).
    fn valid_border_tile(&self, pos: (i32, i32), kind: NodeType) -> bool {
        if self.tile(pos.0, pos.1).is_solid() {
            return false;
        }
        let any_water = self.water_int(pos.0, pos.1) != 0;
        match kind {
            NodeType::SideExit => (pos.0 <= 0 || pos.0 >= self.width() - 1) && !any_water,
            NodeType::SkyExit => pos.1 >= self.height() - 1,
            NodeType::SeaExit => any_water,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 6x5 box open at the top middle, the right edge's middle and nowhere else.
    fn room() -> Room {
        let (w, h) = (6, 5);
        let mut cells = Vec::new();
        for x in 0..w {
            for y in (0..h).rev() {
                let edge = x == 0 || x == w - 1 || y == 0 || y == h - 1;
                let open = !edge || (y == h - 1 && (2..=3).contains(&x)) || (x == w - 1 && y == 2);
                cells.push(if open { "0" } else { "1" });
            }
        }
        let text = format!(
            "box\n{w}*{h}|-1|0\n0*0|0|0\n0,0\nBorder: Passable\n\n\n\n\n0|0|0|\n\n{}|\n",
            cells.join("|")
        );
        Room::parse(&text).expect("box parses")
    }

    #[test]
    fn exits_follow_the_open_border_runs() {
        let r = room();
        let exits = r.border_exits(false);
        assert_eq!(
            exits,
            vec![
                RoomBorderExit {
                    tiles: vec![(5, 2)],
                    kind: NodeType::SideExit
                },
                RoomBorderExit {
                    tiles: vec![(2, 4), (3, 4)],
                    kind: NodeType::SkyExit
                },
            ]
        );
        assert!(r.border_exits(true).is_empty());
    }
}
