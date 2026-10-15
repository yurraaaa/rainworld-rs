//! A room's abstract nodes (`AbstractRoomNode`): the baked connectivity map on line 10 (`roomText[9]`) of
//! the geometry file (`RoomPreprocessor.StringToConnMap`), or, for files baked by an
//! older generation, the shortcut nodes the game's re-preprocessing derives
//! (`AIdataPreprocessor.ReturnNodes`).
//!
//! Behaviour: `rainworld-re/notes/world-loading.md`.

use crate::{Room, ShortcutMap, tile_centre};

/// `AbstractRoomNode.Type`, in registration order (the integer in the file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Exit,
    Den,
    RegionTransportation,
    SideExit,
    SkyExit,
    SeaExit,
    BatHive,
    GarbageHoles,
}

impl NodeType {
    pub const ALL: [Self; 8] = [
        Self::Exit,
        Self::Den,
        Self::RegionTransportation,
        Self::SideExit,
        Self::SkyExit,
        Self::SeaExit,
        Self::BatHive,
        Self::GarbageHoles,
    ];

    #[must_use]
    pub fn from_index(index: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(index).ok()?).copied()
    }

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// `borderExit`: side, sky and sea exits.
    #[must_use]
    pub fn is_border_exit(self) -> bool {
        matches!(self, Self::SideExit | Self::SkyExit | Self::SeaExit)
    }
}

/// One `AbstractRoomNode` without its per-creature connectivity table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractRoomNode {
    pub kind: NodeType,
    /// Tube length for exits and dens, -1 otherwise.
    pub shortcut_length: i32,
    pub submerged: bool,
    /// `Room.CameraViewingNode`: the camera a creature arriving here is shown by.
    pub viewed_by_camera: i32,
    pub entrance_width: i32,
}

/// Separated fields of a line, read as the game's `Convert.ToInt32`.
struct Fields<'a>(Vec<&'a str>);

impl Fields<'_> {
    fn int(&self, i: usize) -> Result<i32, String> {
        let s = self.0.get(i).ok_or_else(|| format!("missing field {i}"))?;
        s.trim().parse().map_err(|_| format!("bad number {s:?}"))
    }
}

/// The parsed node map line (`gen|count|layers|node|node|...`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnMap {
    /// Preprocessing generation the file was baked with.
    pub generation: i32,
    pub nodes: Vec<AbstractRoomNode>,
}

impl ConnMap {
    /// `RoomPreprocessor.StringToConnMap`.
    ///
    /// # Errors
    /// A message where the game would throw: missing fields, bad numbers, an
    /// unregistered node type.
    pub fn parse(line: &str) -> Result<Self, String> {
        let parts = Fields(line.split('|').collect());
        let generation = parts.int(0)?;
        let count = usize::try_from(parts.int(1)?).map_err(|_| "negative node count".to_owned())?;
        let layers =
            usize::try_from(parts.int(2)?).map_err(|_| "negative layer count".to_owned())?;
        let nodes = (0..count)
            .map(|i| {
                let text = parts
                    .0
                    .get(3 + i)
                    .ok_or_else(|| format!("node {i} missing"))?;
                AbstractRoomNode::parse(text, layers * count)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { generation, nodes })
    }
}

impl AbstractRoomNode {
    /// One node: `type,length,submerged,camera,width,` then `cost generation` pairs.
    fn parse(text: &str, connectivity: usize) -> Result<Self, String> {
        let fields = Fields(text.split(',').collect());
        if fields.0.len() < 5 + connectivity {
            return Err(format!("node {text:?} has too few fields"));
        }
        let kind = NodeType::from_index(fields.int(0)?)
            .ok_or_else(|| format!("node type {}", fields.0[0]))?;
        // The connectivity is not kept, but each `cost gen` pair must parse.
        for pair in &fields.0[5..5 + connectivity] {
            let pair = Fields(pair.split(' ').collect());
            pair.int(0)?;
            pair.int(1)?;
        }
        Ok(Self {
            kind,
            shortcut_length: fields.int(1)?,
            submerged: fields.int(2)? == 1,
            viewed_by_camera: fields.int(3)?,
            entrance_width: fields.int(4)?,
        })
    }
}

impl Room {
    /// The nodes `WorldLoader.LoadAbstractRoom` gives this room's abstract room: the
    /// baked ones when baked at `generation` or later, else what re-preprocessing derives
    /// from the shortcuts ([`Room::shortcut_nodes`]).
    ///
    /// # Errors
    /// When the node map line does not parse.
    pub fn abstract_nodes(&self, generation: i32) -> Result<Vec<AbstractRoomNode>, String> {
        let map = self.conn_map()?;
        if map.generation >= generation {
            Ok(map.nodes.clone())
        } else {
            Ok(self.shortcut_nodes())
        }
    }

    /// `AIdataPreprocessor.ReturnNodes` for the shortcut nodes: one per
    /// [`ShortcutMap::nodes`] entry, typed by pass (exit, den, region transport), with
    /// the leading tube's length, `CameraViewingNode` and whether its entrance is under
    /// water. Border exits, bat hives and garbage holes (which need `MapRoomBorders` and
    /// the AI map) are not derived.
    #[must_use]
    pub fn shortcut_nodes(&self) -> Vec<AbstractRoomNode> {
        let map: &ShortcutMap = self.shortcut_map();
        map.nodes()
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                let kind = match self.tile(x, y).shortcut {
                    crate::Shortcut::RoomExit => NodeType::Exit,
                    crate::Shortcut::CreatureDen => NodeType::Den,
                    _ => NodeType::RegionTransportation,
                };
                let shortcut = map.leading_to_node(i);
                let shortcut_node = matches!(kind, NodeType::Exit | NodeType::Den);
                AbstractRoomNode {
                    kind,
                    shortcut_length: match shortcut {
                        Some(s) if shortcut_node => i32::try_from(s.length).unwrap_or(i32::MAX),
                        _ => -1,
                    },
                    submerged: shortcut_node
                        && shortcut.is_some_and(|s| self.under_water(s.start.1)),
                    viewed_by_camera: if shortcut_node {
                        self.camera_viewing_node(i)
                    } else {
                        -1
                    },
                    entrance_width: 1,
                }
            })
            .collect()
    }

    /// `Room.CameraViewingNode`: the camera whose centre (`pos + (700, 402)`) is nearest
    /// the node tile's middle; 0 without cameras.
    #[must_use]
    pub fn camera_viewing_node(&self, node: usize) -> i32 {
        let Some(&(x, y)) = self.shortcut_map().nodes().get(node) else {
            return 0;
        };
        let [nx, ny] = tile_centre(x, y);
        let mut best = (-1, f32::MAX);
        for (i, cam) in self.cameras().iter().enumerate() {
            let (dx, dy) = (nx - (cam[0] + 700.0), ny - (cam[1] + 402.0));
            let d = (dx * dx + dy * dy).sqrt();
            if d < best.1 {
                best = (i32::try_from(i).unwrap_or(i32::MAX), d);
            }
        }
        best.0.max(0)
    }

    /// `Tile.AnyWater` for row `y` with the file's water level.
    fn under_water(&self, y: i32) -> bool {
        self.water_level().is_some_and(|w| y <= w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nodes_and_skips_connectivity() {
        // Two nodes, one layer: two `cost gen` pairs per node.
        let map = ConnMap::parse("20|2|1|0,3,0,1,1,0 0,5 1,|1,4,1,0,1,5 1,0 0,|").unwrap();
        assert_eq!(map.generation, 20);
        assert_eq!(map.nodes.len(), 2);
        assert_eq!(map.nodes[0].kind, NodeType::Exit);
        assert_eq!(map.nodes[0].viewed_by_camera, 1);
        assert_eq!(map.nodes[1].kind, NodeType::Den);
        assert!(map.nodes[1].submerged);
        assert!(ConnMap::parse("20|1|1|0,3,0,0,1,|").is_err());
        assert!(ConnMap::parse("20|1|0|9,3,0,0,1,|").is_err());
    }
}
