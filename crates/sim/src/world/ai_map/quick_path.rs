//! `QuickPathFinder`: a search back from the goal over the AI map's incoming
//! connections, with the game's sorted-list heap (ties keep insertion order), then the
//! walk forward from the start. Deterministic: no draws.
//!
//! Mirrors: QuickPathFinder.cs

use std::collections::VecDeque;

use crate::creature::CreatureTemplate;
use crate::world::{AiMap, Legality, PathCost};

/// A `PathNode`: a tile, the node one step closer to the goal, the cost to the goal and
/// the heuristic the heap sorts by.
#[derive(Debug, Clone, Copy)]
struct PathNode {
    tile: (i32, i32),
    closer: Option<usize>,
    steps_to_goal: PathCost,
    heuristic: PathCost,
}

/// A finished `QuickPath`: the tiles from the start to the goal.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickPath {
    pub tiles: Vec<(i32, i32)>,
    pub cost: PathCost,
}

/// `QuickPathFinder`.
pub struct QuickPathFinder<'a> {
    map: &'a AiMap,
    template: &'a CreatureTemplate,
    start: (i32, i32),
    goal: (i32, i32),
    nodes: Vec<PathNode>,
    /// `nodeHeap`'s linked list, head first.
    heap: VecDeque<usize>,
    /// 0 unchecked, 1 checked, 2 closed.
    checked: Vec<u8>,
    walker: Option<usize>,
    start_found: bool,
    path: Vec<(i32, i32)>,
    cost: PathCost,
    /// `status`: 0 searching, 1 found, -1 no path.
    pub status: i32,
}

impl<'a> QuickPathFinder<'a> {
    #[must_use]
    pub fn new(
        start: (i32, i32),
        goal: (i32, i32),
        map: &'a AiMap,
        template: &'a CreatureTemplate,
    ) -> Self {
        let (start, goal) = (map.clamp_tile(start), map.clamp_tile(goal));
        let mut f = Self {
            map,
            template,
            start,
            goal,
            nodes: Vec::new(),
            heap: VecDeque::new(),
            checked: Vec::new(),
            walker: None,
            start_found: false,
            path: Vec::new(),
            cost: PathCost::new(0.0, Legality::Allowed),
            status: 0,
        };
        if start == goal {
            f.status = 1;
            f.path.push(start);
            return f;
        }
        f.checked = vec![0; (map.width() * map.height()).max(0) as usize];
        let zero = PathCost::new(0.0, Legality::Allowed);
        f.nodes.push(PathNode {
            tile: goal,
            closer: None,
            steps_to_goal: zero,
            heuristic: zero,
        });
        f.heap.push_back(0);
        f.mark(goal, 1);
        f
    }

    /// Runs `Update` until the search ends; the path if one was found.
    #[must_use]
    pub fn run(mut self) -> Option<QuickPath> {
        while self.status == 0 {
            self.update();
        }
        (self.status == 1).then_some(QuickPath {
            tiles: self.path,
            cost: self.cost,
        })
    }

    fn cell(&self, t: (i32, i32)) -> Option<usize> {
        if t.0 < 0 || t.1 < 0 {
            return None;
        }
        let i = (t.0 + t.1 * self.map.width()) as usize;
        (i < self.checked.len()).then_some(i)
    }

    /// `MarkNodeAsChecked` (`state` 1, only from unchecked) and `MarkNodeAsClosed` (2).
    fn mark(&mut self, t: (i32, i32), state: u8) {
        if let Some(i) = self.cell(t)
            && (state == 2 || self.checked[i] == 0)
        {
            self.checked[i] = state;
        }
    }

    /// `IsNodePreviouslyChecked`; out of the map counts as checked.
    fn previously_checked(&self, t: (i32, i32)) -> bool {
        self.cell(t).is_none_or(|i| self.checked[i] > 0)
    }

    /// `IsNodeClosed`; out of the map counts as closed.
    fn closed(&self, t: (i32, i32)) -> bool {
        self.cell(t).is_none_or(|i| self.checked[i] == 2)
    }

    /// `MinHeap.Add`.
    fn heap_add(&mut self, n: usize) {
        let h = self.nodes[n].heuristic;
        let Some(&head) = self.heap.front() else {
            self.heap.push_back(n);
            return;
        };
        if self.heap.len() == 1 && h.greater_than(self.nodes[head].heuristic) {
            self.heap.push_back(n);
            return;
        }
        if self.nodes[head].heuristic.greater_than(h) {
            self.heap.push_front(n);
            return;
        }
        let mut at = 0;
        while at + 1 < self.heap.len() && self.nodes[self.heap[at + 1]].heuristic.less_than(h) {
            at += 1;
        }
        self.heap.insert(at + 1, n);
    }

    /// `MinHeap.FindAndReplace`: a cheaper node replaces its tile's queued one, unless
    /// that one is the head.
    fn heap_find_and_replace(&mut self, n: usize) {
        let tile = self.nodes[n].tile;
        let Some(at) = self.heap.iter().position(|&q| self.nodes[q].tile == tile) else {
            return;
        };
        if at > 0
            && self.nodes[n]
                .heuristic
                .less_than(self.nodes[self.heap[at]].heuristic)
        {
            self.heap.remove(at);
            self.heap_add(n);
        }
    }

    /// `Update`: one node of the search, or one step of the walk.
    pub fn update(&mut self) {
        if self.status != 0 {
            return;
        }
        if !self.start_found {
            self.search_step();
            return;
        }
        let Some(w) = self.walker else {
            self.status = -1;
            return;
        };
        let node = self.nodes[w];
        self.path.push(node.tile);
        self.cost = self.cost + self.map.tile_cost_for_creature(node.tile, self.template);
        let Some(closer) = node.closer else {
            self.status = -1;
            return;
        };
        let next = self.nodes[closer].tile;
        let mut best = PathCost::new(0.0, Legality::Unallowed);
        for c in &self.map.tile(node.tile.0, node.tile.1).outgoing_paths {
            let r = self.template.connection_resistance(c.kind);
            if c.dest.tile == next && r.allowed() && r.less_than(best) {
                best = r;
            }
        }
        self.cost = self.cost + best;
        self.walker = Some(closer);
        if next == self.goal {
            self.status = 1;
            self.path.push(next);
        }
    }

    fn search_step(&mut self) {
        let Some(n) = self.heap.pop_front() else {
            self.status = -1;
            return;
        };
        let node = self.nodes[n];
        self.mark(node.tile, 2);
        let (w, h) = (self.map.width(), self.map.height());
        let incoming = self
            .map
            .tile(node.tile.0, node.tile.1)
            .incoming_paths
            .clone();
        for c in &incoming {
            let s = c.start.tile;
            if s.0 < 0
                || s.0 >= w
                || s.1 < 0
                || s.1 >= h
                || !self
                    .map
                    .is_connection_allowed_for_creature(c, self.template)
                || self.closed(s)
            {
                continue;
            }
            if s == self.start {
                self.start_found = true;
                self.path.clear();
                self.nodes.push(PathNode {
                    tile: s,
                    closer: Some(n),
                    steps_to_goal: PathCost::new(
                        node.steps_to_goal.resistance + 1.0,
                        node.steps_to_goal.legality,
                    ),
                    heuristic: PathCost::new(0.0, Legality::Allowed),
                });
                self.walker = Some(self.nodes.len() - 1);
                continue;
            }
            let steps = node.steps_to_goal
                + self.template.connection_resistance(c.kind) * c.distance as f32
                + self.map.tile_cost_for_creature(s, self.template);
            let heuristic = PathCost::new(
                steps.resistance
                    + (s.0 - self.start.0).abs() as f32
                    + (s.1 - self.start.1).abs() as f32,
                steps.legality,
            );
            self.nodes.push(PathNode {
                tile: s,
                closer: Some(n),
                steps_to_goal: steps,
                heuristic,
            });
            let new = self.nodes.len() - 1;
            if self.previously_checked(s) {
                self.heap_find_and_replace(new);
            } else {
                self.heap_add(new);
            }
            self.mark(s, 1);
        }
    }
}
