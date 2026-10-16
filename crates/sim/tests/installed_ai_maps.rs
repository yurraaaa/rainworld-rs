//! The port's AI map against the room files' pre-baked creature layers (read-only).
//!
//! Per `preBakedPathingCreatures` template the room files hold the generations of
//! `AIdataPreprocessor.NodeMapper`'s Dijkstra from each of the template's nodes over the
//! incoming paths it may use, and in the node line the `ConnectivityMapper`'s
//! node-to-node costs. This test runs that Dijkstra on the port's `AiMap` (with the
//! game's `RWCustom.MinHeap`) and checks, per layer:
//! - the layer fits the port's accessible tiles and node count (else it is *rejected*:
//!   the game decodes it shifted too);
//! - the baked generations form a shortest-path tree under the port's costs: the same
//!   tiles are reached, the start tiles are generation 1, and every other reached tile
//!   has a neighbour one generation lower through an allowed connection whose cost lands
//!   exactly on the port's least cost. Which of several equal-cost routes the bake took
//!   depends on its queue's tie order, which is not the 1.11.8 heap's (older builds baked
//!   the files), so the generations themselves are not compared;
//! - the baked node-to-node costs (`(int)` of the least cost) equal the port's.
//!
//! That checks the accessibility classes, every connection the mapper makes,
//! `IsConnectionAllowedForCreature`, the connection and tile costs and every template's
//! resistances. The accessibility (float) grids are not compared: their order over
//! thousands of illegal-cost tiles is all ties, and the bake's float precision was wider.
//! Set `RW_DIR` to the game install directory; skipped if absent.

use std::cmp::Ordering;
use std::path::PathBuf;

use rw_assets::world::{
    GameRooms, LineFilter, Regions, RoomSource, StoryConditions, WorldData, WorldRequest,
};
use rw_assets::{ActiveMods, ConnMap, CreatureType, NodeType, Room, ShortcutType, StreamingAssets};
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::creature::CreatureTemplate;
use rw_sim::world::{AiMap, Legality, PathCost};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

/// `RWCustom.MinHeap<PathCost, tile>`.
struct Heap {
    list: Vec<(PathCost, (i32, i32))>,
}

impl Heap {
    fn less(&self, a: usize, b: usize) -> bool {
        self.list[a].0.compare(self.list[b].0) == Ordering::Less
    }

    fn add(&mut self, key: PathCost, tile: (i32, i32)) {
        self.list.push((key, tile));
        let mut i = self.list.len() - 1;
        while i > 0 {
            let up = (i - 1) / 2;
            if !self.less(i, up) {
                break;
            }
            self.list.swap(i, up);
            i = up;
        }
    }

    fn take_min(&mut self) -> Option<(PathCost, (i32, i32))> {
        if self.list.is_empty() {
            return None;
        }
        let min = self.list.swap_remove(0);
        let mut i = 0;
        while i * 2 + 1 < self.list.len() {
            let mut child = i * 2 + 1;
            if child + 1 < self.list.len() && self.less(child + 1, child) {
                child += 1;
            }
            if !self.less(child, i) {
                break;
            }
            self.list.swap(child, i);
            i = child;
        }
        Some(min)
    }
}

/// `NodeMapper`'s Dijkstra from one node: the least cost of every reached tile.
struct NodeMap<'a> {
    map: &'a AiMap,
    template: &'a CreatureTemplate,
    costs: Vec<Option<PathCost>>,
    /// Reached tiles in order, with their cost.
    order: Vec<((i32, i32), PathCost)>,
}

impl<'a> NodeMap<'a> {
    fn index(map: &AiMap, t: (i32, i32)) -> usize {
        usize::try_from(t.0 * map.height() + t.1).expect("in room")
    }

    fn cost(&self, t: (i32, i32)) -> Option<PathCost> {
        self.costs[Self::index(self.map, t)]
    }

    /// `AddCell`: queue the allowed incoming paths from unreached tiles.
    fn add(&mut self, heap: &mut Heap, pos: (i32, i32), cost: PathCost) {
        self.costs[Self::index(self.map, pos)] = Some(cost);
        self.order.push((pos, cost));
        for c in &self.map.tile(pos.0, pos.1).incoming_paths {
            if self
                .map
                .is_connection_allowed_for_creature(c, self.template)
                && self.cost(c.start_tile()).is_none()
            {
                heap.add(
                    cost + self.map.connection_cost_for_creature(c, self.template),
                    c.start_tile(),
                );
            }
        }
    }

    fn run(map: &'a AiMap, template: &'a CreatureTemplate, starts: &[(i32, i32)]) -> Self {
        let mut m = Self {
            map,
            template,
            costs: vec![None; usize::try_from(map.width() * map.height()).expect("size")],
            order: Vec::new(),
        };
        let mut heap = Heap { list: Vec::new() };
        for &t in starts {
            if map.tile_accessible_to_creature(t, template) {
                m.add(&mut heap, t, PathCost::new(0.0, Legality::Allowed));
            }
        }
        while let Some((key, pos)) = heap.take_min() {
            if m.cost(pos).is_none() {
                m.add(&mut heap, pos, key);
            }
        }
        m
    }

    /// Whether the baked generations (by tile; `None` where the layer holds none) form
    /// a shortest-path tree of this run: `None` if so, else the first tile that does not.
    fn check_tree(&self, starts: &[(i32, i32)], generations: &[Option<i32>]) -> Option<String> {
        for x in 0..self.map.width() {
            for y in 0..self.map.height() {
                let Some(g) = generations[Self::index(self.map, (x, y))] else {
                    continue;
                };
                match (g > 0, self.cost((x, y))) {
                    (false, None) => {}
                    (true, None) => return Some(format!("({x}, {y}) baked {g}, not reached")),
                    (false, Some(_)) => return Some(format!("({x}, {y}) reached, baked {g}")),
                    (true, Some(cost)) => {
                        let start = g == 1 && starts.contains(&(x, y));
                        if !start && !self.has_tree_parent((x, y), g, cost, generations) {
                            return Some(format!(
                                "({x}, {y}) baked {g} at cost {}: no parent",
                                cost.resistance
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    /// A tile `p` with `t -> p` allowed, `cost(p) + step == cost(t)` and its generation
    /// one lower (or none kept, outside the layer's covered area).
    fn has_tree_parent(
        &self,
        t: (i32, i32),
        g: i32,
        cost: PathCost,
        generations: &[Option<i32>],
    ) -> bool {
        self.map.tile(t.0, t.1).outgoing_paths.iter().any(|c| {
            c.start_tile() == t
                && self
                    .map
                    .is_connection_allowed_for_creature(c, self.template)
                && self.cost(c.dest_tile()).is_some_and(|p| {
                    (p + self.map.connection_cost_for_creature(c, self.template)).compare(cost)
                        == Ordering::Equal
                        && generations[Self::index(self.map, c.dest_tile())]
                            .is_none_or(|pg| pg == g - 1)
                })
        })
    }
}

/// What the layers compared.
#[derive(Default)]
struct Tally {
    rooms: usize,
    layers: usize,
    rejected: Vec<(CreatureType, String)>,
    matching: usize,
    mismatching: Vec<(CreatureType, String)>,
    node_costs: usize,
    node_costs_mismatching: Vec<(CreatureType, String)>,
}

impl Tally {
    fn room(&mut self, name: &str, room: &Room, map: &AiMap) {
        self.rooms += 1;
        let conn = room.conn_map().ok();
        for (p, &kind) in CreatureTemplate::PRE_BAKED_PATHING_CREATURES
            .iter()
            .enumerate()
        {
            let template = CreatureTemplate::of(kind, ActiveMods::NONE);
            let cm = map.creature_map_at(p).expect("15 maps");
            self.layers += 1;
            if cm.cells() != cm.baked_ints() {
                self.rejected.push((
                    kind,
                    format!(
                        "{name} {kind:?}: {} cells, {} baked ints",
                        cm.cells(),
                        cm.baked_ints()
                    ),
                ));
                continue;
            }
            self.layer(name, map, &template, p, conn);
        }
    }

    fn layer(
        &mut self,
        name: &str,
        map: &AiMap,
        template: &CreatureTemplate,
        layer: usize,
        conn: Option<&ConnMap>,
    ) {
        let cm = map.creature_map(template).expect("pre-baked");
        let order = Self::shortcuts_in_node_order(map);
        let tiles: Vec<(i32, i32)> = (0..map.width())
            .flat_map(|x| (0..map.height()).map(move |y| (x, y)))
            .collect();
        let mut failure = None;
        for n in 0..cm.number_of_nodes() {
            let common = map.creature_specific_to_common_node_index(n, template);
            let starts = map.node_start_tiles(common);
            let run = NodeMap::run(map, template, &starts);
            let generations: Vec<Option<i32>> = tiles
                .iter()
                .map(|&(x, y)| Some(cm.distance_to_exit(x, y, n)).filter(|&g| g != -1))
                .collect();
            if failure.is_none()
                && let Some(why) = run.check_tree(&starts, &generations)
            {
                failure = Some(format!("{name} {:?} node {n}: {why}", template.kind));
            }
            if let Some(conn) = conn {
                self.node_costs(name, map, template, layer, conn, &order, common, &run);
            }
        }
        match failure {
            None => self.matching += 1,
            Some(why) => self.mismatching.push((template.kind, why)),
        }
    }

    /// `ConnectivityMapper`'s `shortCutsInNodeOrder`: the room exit, den and region
    /// transport shortcuts, stably sorted by their node.
    fn shortcuts_in_node_order(map: &AiMap) -> Vec<((i32, i32), ShortcutType)> {
        let mut list: Vec<(usize, (i32, i32), ShortcutType)> = map
            .shortcuts()
            .shortcuts()
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    ShortcutType::RoomExit
                        | ShortcutType::CreatureHole
                        | ShortcutType::RegionTransportation
                )
            })
            .map(|s| (s.dest_node.unwrap_or(usize::MAX), s.start, s.kind))
            .collect();
        list.sort_by_key(|s| s.0);
        list.into_iter().map(|(_, t, k)| (t, k)).collect()
    }

    /// `ConnectivityMapper.TileDiscovered`'s costs from node `b`: to each mapped
    /// shortcut node its entrance's, to a border exit or hive its first reached tile's,
    /// when allowed (the `(int)` of the least cost).
    #[allow(clippy::too_many_arguments)]
    fn node_costs(
        &mut self,
        name: &str,
        map: &AiMap,
        template: &CreatureTemplate,
        layer: usize,
        conn: &ConnMap,
        order: &[((i32, i32), ShortcutType)],
        b: i32,
        run: &NodeMap<'_>,
    ) {
        let mut targets: Vec<(usize, Vec<(i32, i32)>)> = Vec::new();
        for (i, &(tile, kind)) in order.iter().enumerate() {
            let mapped = (template.maps_node(NodeType::Exit) && kind == ShortcutType::RoomExit)
                || (template.maps_node(NodeType::Den) && kind == ShortcutType::CreatureHole)
                || (template.maps_node(NodeType::RegionTransportation)
                    && kind == ShortcutType::RegionTransportation);
            if mapped {
                targets.push((i, vec![tile]));
            }
        }
        for (j, exit) in map.border_exits().iter().enumerate() {
            if template.maps_node(exit.kind) {
                targets.push((order.len() + j, exit.tiles.clone()));
            }
        }
        if template.maps_node(NodeType::BatHive) {
            for (l, hive) in map.hives().iter().enumerate() {
                targets.push((order.len() + map.border_exits().len() + l, hive.clone()));
            }
        }
        let layer = i32::try_from(layer).expect("layer");
        for (i, tiles) in targets {
            if i32::try_from(i).expect("node") == b {
                continue;
            }
            let Some(&(_, cost)) = run
                .order
                .iter()
                .find(|(t, c)| tiles.contains(t) && c.allowed())
            else {
                continue;
            };
            let Some(node) = conn.nodes.get(i) else {
                continue;
            };
            self.node_costs += 1;
            let (port, baked) = (cost.resistance as i32, node.connection_cost(b, layer));
            if port != baked {
                self.node_costs_mismatching.push((
                    template.kind,
                    format!(
                        "{name} {:?}: node {i} from {b}: baked {baked}, port {port}",
                        template.kind
                    ),
                ));
            }
        }
    }
}

fn installed() -> Option<StreamingAssets> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if sa.join("world").join("regions.txt").is_file() {
        Some(StreamingAssets::new(sa))
    } else {
        eprintln!("skipping: no world under {}", sa.display());
        None
    }
}

/// The world rooms of the regions named (all when empty), the off-screen dens left out.
fn tally(sa: &StreamingAssets, only: &[&str]) -> Tally {
    let campaign = &SlugcatCampaign::SURVIVOR;
    let filter = LineFilter {
        timeline: Some(campaign.timeline),
        mods: ActiveMods::NONE,
        conditions: StoryConditions::new_story(),
    };
    let regions = Regions::load(sa).expect("regions");
    let mut tally = Tally::default();
    for region in &regions.0 {
        if !only.is_empty() && !only.contains(&region.name.as_str()) {
            continue;
        }
        let request = WorldRequest {
            name: region.name.clone(),
            single_room: false,
            filter: filter.clone(),
        };
        let data = WorldData::load(sa, &regions, &request).expect("world loads");
        let rooms = GameRooms {
            assets: sa.clone(),
            region: Some(region.name.to_lowercase()),
            timeline: campaign.timeline,
        };
        for r in data.rooms.iter().filter(|r| !r.offscreen_den) {
            let loaded = rooms.load_room(r.file_name()).expect("room loads");
            let map = AiMap::new(&loaded.room, r.index, r.shelter() || r.gate(), false);
            tally.room(&r.name, &loaded.room, &map);
        }
    }
    tally
}

fn report(label: &str, t: &Tally) {
    println!(
        "{label}: {} rooms, {} layers read, {} rejected, {} matching, {} not; {} node costs, {} not matching",
        t.rooms,
        t.layers,
        t.rejected.len(),
        t.matching,
        t.mismatching.len(),
        t.node_costs,
        t.node_costs_mismatching.len(),
    );
    for (_, line) in t
        .rejected
        .iter()
        .chain(&t.mismatching)
        .chain(&t.node_costs_mismatching)
    {
        println!("  {line}");
    }
}

/// The layers older builds baked and the files kept (`../rainworld-re/notes/ai-map.md`):
/// BigEel, Deer and MirosBird layers of a different size in a few rooms; Snail without
/// today's force-allowed drops; DaddyLongLegs with a narrower squeeze (+10 per path);
/// Fly costs to a few hives.
const STALE_LAYERS: [CreatureType; 3] = [
    CreatureType::BigEel,
    CreatureType::Deer,
    CreatureType::MirosBird,
];
const STALE_TREES: [CreatureType; 2] = [CreatureType::Snail, CreatureType::DaddyLongLegs];
const STALE_NODE_COSTS: [CreatureType; 3] = [
    CreatureType::Snail,
    CreatureType::DaddyLongLegs,
    CreatureType::Fly,
];

#[test]
fn su_baked_layers_match_the_preprocessor() {
    let Some(sa) = installed() else { return };
    let t = tally(&sa, &["SU"]);
    report("SU", &t);
    assert_eq!((t.rooms, t.layers, t.rejected.len()), (54, 810, 6));
    assert!(t.rejected.iter().all(|(k, _)| STALE_LAYERS.contains(k)));
    assert!(t.mismatching.iter().all(|(k, _)| STALE_TREES.contains(k)));
    assert!(
        t.node_costs_mismatching
            .iter()
            .all(|(k, _)| STALE_NODE_COSTS.contains(k))
    );
}

/// Every region; run with `--ignored`.
#[test]
#[ignore = "every installed region, slow"]
fn all_baked_layers_match_the_preprocessor() {
    let Some(sa) = installed() else { return };
    let t = tally(&sa, &[]);
    report("all regions", &t);
    assert!(t.mismatching.iter().all(|(k, _)| STALE_TREES.contains(k)));
}
