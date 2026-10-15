//! Regions and their world files as `Region.LoadAllRegions` and `WorldLoader` read them:
//! the region list with global room indices, the line preprocessing (timeline filters
//! `(White)`, `(X-Red)`, custom `{...}` conditions), the `EXCLUSIVEROOM` merge, the
//! `CONDITIONAL LINKS` section, the `ROOMS` section, and the abstract rooms with their
//! connections and nodes, broken exits capped (`MappingRooms`, `CreatingAbstractRooms`,
//! `CappingBrokenExits`, `World.LoadWorld`'s off-screen den).
//!
//! Every conditional is kept as data and decided here for the given timeline and mods,
//! so DLC worlds only add data. Behaviour: `rainworld-re/notes/world-loading.md`.

use std::collections::HashMap;
use std::fmt;
use std::fs;

use crate::creatures::CreatureSpawner;
use crate::region_properties::RegionProperties;
use crate::{
    AbstractRoomNode, ActiveMods, AssetError, NodeType, Room, RoomSettings, SlugcatName,
    StreamingAssets, Timeline,
};

/// `World.preProcessingGeneration`: room files baked earlier are re-preprocessed.
pub const PREPROCESSING_GENERATION: i32 = 20;

/// One line of `world/regions.txt`, with its room numbering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionInfo {
    /// As written in `regions.txt` (upper case, e.g. `SU`).
    pub name: String,
    /// Global index of the region's first room (sum of the earlier regions' counts).
    pub first_room_index: i32,
    /// `NumberOfRoomsInRegion`: raw ` : ` lines of the `ROOMS` section, plus one.
    pub number_of_rooms: i32,
    /// Line in `regions.txt` (`regionNumber`).
    pub region_number: i32,
}

/// `OverWorld.regions`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Regions(pub Vec<RegionInfo>);

impl Regions {
    /// `Region.LoadAllRegions`: every line of `world/regions.txt` is a region (a missing
    /// file is one region named ""); room counts come from the unprocessed world files.
    ///
    /// # Errors
    /// When an existing file cannot be read.
    pub fn load(assets: &StreamingAssets) -> Result<Self, AssetError> {
        let path = assets.file("world/regions.txt");
        let names: Vec<String> = if path.is_file() {
            let text = fs::read_to_string(&path).map_err(|e| AssetError::io(&path, e))?;
            text.lines().map(str::to_owned).collect()
        } else {
            vec![String::new()]
        };
        let mut first = 0;
        let mut regions = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            let count = Self::number_of_rooms(assets, &name)?;
            regions.push(RegionInfo {
                name,
                first_room_index: first,
                number_of_rooms: count,
                region_number: i32::try_from(i).unwrap_or(i32::MAX),
            });
            first += count;
        }
        Ok(Self(regions))
    }

    /// `Region.NumberOfRoomsInRegion`: lines containing ` : ` after `ROOMS` up to
    /// `END ROOMS`, unfiltered, plus one for the off-screen den.
    fn number_of_rooms(assets: &StreamingAssets, name: &str) -> Result<i32, AssetError> {
        let path = assets.file(&format!("world/{name}/world_{name}.txt"));
        if !path.is_file() {
            return Ok(1);
        }
        let text = fs::read_to_string(&path).map_err(|e| AssetError::io(&path, e))?;
        let mut in_rooms = false;
        let mut count = 1;
        for line in text.lines() {
            if line == "END ROOMS" {
                break;
            }
            if in_rooms && line.contains(" : ") {
                count += 1;
            }
            if line == "ROOMS" {
                in_rooms = true;
            }
        }
        Ok(count)
    }

    /// `OverWorld.GetRegion`: by name, ignoring case.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&RegionInfo> {
        self.0.iter().find(|r| r.name.eq_ignore_ascii_case(name))
    }
}

/// The story state the custom `{...}` line conditions read
/// (`WorldLoader.Preprocessing.preprocessorConditions`). All are DLC (Watcher) state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoryConditions {
    /// A story session (not arena).
    pub story: bool,
    pub prince_encounters: i32,
    pub saw_void_bath_slideshow: bool,
    pub final_warp_sequence_started: bool,
    pub beaten_spinning_top: bool,
    pub beaten_void_weaver: bool,
    /// The destination room of the special warp in progress.
    pub special_warp_room: Option<String>,
}

impl StoryConditions {
    /// A new story game's state.
    #[must_use]
    pub fn new_story() -> Self {
        Self {
            story: true,
            ..Self::default()
        }
    }

    /// The registered conditions in order (`PrinceEncounters`, `SpinningTopEndingConditions`,
    /// `WeaverEndingCondition`, `DaemonEndingCondition`); `None` when none knows `text`.
    fn evaluate(&self, text: &str, mods: ActiveMods) -> Vec<bool> {
        let mut answers = Vec::new();
        if let Some(n) = text.strip_prefix("PrinceEncounters:")
            && let Ok(n) = n.parse::<i32>()
        {
            answers.push(self.story && (!mods.watcher || self.prince_encounters >= n));
        }
        match text {
            "PostBathScene" => {
                answers.push(self.saw_void_bath_slideshow || self.final_warp_sequence_started);
            }
            "ToysEnding" => {
                answers.push(self.beaten_spinning_top && !self.final_warp_sequence_started);
            }
            "WeaverEnding" => answers.push(self.beaten_void_weaver),
            "TreeSequence" | "PrinceSequence" => {
                let prefix = if text == "TreeSequence" {
                    "wrsa_tree"
                } else {
                    "wrsa_prince"
                };
                answers.push(
                    self.final_warp_sequence_started
                        && self
                            .special_warp_room
                            .as_deref()
                            .is_some_and(|r| r.to_lowercase().starts_with(prefix)),
                );
            }
            _ => {}
        }
        answers
    }
}

/// Why a world file line was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// Shorter than two characters, or a `//` comment.
    Comment,
    /// A `(...)` timeline filter that excludes the timeline.
    Timeline(String),
    /// A `{...}` condition that fails.
    Condition(String),
    /// A line the game would fail on (its loader thread throws).
    Malformed(String),
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comment => f.write_str("comment or blank"),
            Self::Timeline(t) => write!(f, "timeline filter ({t})"),
            Self::Condition(c) => write!(f, "condition {{{c}}}"),
            Self::Malformed(m) => write!(f, "malformed: {m}"),
        }
    }
}

/// A dropped line (1-based line number in the file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedLine {
    pub line: usize,
    pub text: String,
    pub reason: Rejection,
}

/// What `Preprocessing.PreprocessLine` decides with a timeline, mods and story state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineFilter {
    pub timeline: Option<Timeline>,
    pub mods: ActiveMods,
    pub conditions: StoryConditions,
}

impl LineFilter {
    /// The line with its prefixes removed, or why it is dropped.
    ///
    /// # Errors
    /// The [`Rejection`].
    pub fn preprocess<'a>(&self, line: &'a str) -> Result<&'a str, Rejection> {
        if line.chars().count() < 2 || line.starts_with("//") {
            return Err(Rejection::Comment);
        }
        let mut line = line;
        if line.starts_with('(')
            && let Some(close) = line.find(')')
        {
            let filter = &line[1..close];
            if !self.timeline_match(filter) {
                return Err(Rejection::Timeline(filter.to_owned()));
            }
            line = &line[close + 1..];
        }
        if line.is_empty() {
            // The game indexes `line[0]` here and throws.
            return Err(Rejection::Malformed("nothing after the filter".to_owned()));
        }
        if line.starts_with('{')
            && let Some(close) = line.find('}')
        {
            let conditions = &line[1..close];
            if !self.custom_conditions(conditions) {
                return Err(Rejection::Condition(conditions.to_owned()));
            }
            line = &line[close + 1..];
        }
        Ok(line)
    }

    /// `TimelineMatch`: a comma list of timeline names or player numbers, inverted by a
    /// leading `X-`; with no timeline only inverted filters pass.
    fn timeline_match(&self, text: &str) -> bool {
        let (text, inverted) = match text.strip_prefix("X-") {
            Some(rest) => (rest, true),
            None => (text, false),
        };
        let Some(timeline) = self.timeline else {
            return inverted;
        };
        let listed = text.split(',').any(|entry| {
            let entry = entry.trim();
            match entry.parse::<i32>() {
                Ok(n) => {
                    SlugcatName::from_player_number(n, self.mods)
                        .map_or(Timeline::WHITE, SlugcatName::timeline)
                        == timeline
                }
                Err(_) => entry == timeline.value(),
            }
        });
        listed != inverted
    }

    /// `PreprocessCustomConditions`: comma list, `!` negates; the line is dropped when a
    /// condition's answer equals its negation flag.
    fn custom_conditions(&self, text: &str) -> bool {
        text.split(',').all(|entry| {
            let negated = entry.contains('!');
            let name = entry.replace('!', "");
            !self
                .conditions
                .evaluate(&name, self.mods)
                .contains(&negated)
        })
    }
}

/// A world file's lines after preprocessing and the `EXCLUSIVEROOM` merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldLines {
    pub lines: Vec<String>,
    pub rejected: Vec<RejectedLine>,
}

impl WorldLines {
    /// The `WorldLoader` constructor's reading: each line preprocessed, then every
    /// `X : EXCLUSIVEROOM : room` line (scanning from the end, not line 0) merged per
    /// room and re-inserted before `END CONDITIONAL LINKS`.
    #[must_use]
    pub fn preprocess(text: &str, filter: &LineFilter) -> Self {
        let mut lines = Vec::new();
        let mut rejected = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            match filter.preprocess(raw) {
                Ok(line) => lines.push(line.to_owned()),
                Err(reason) => rejected.push(RejectedLine {
                    line: i + 1,
                    text: raw.to_owned(),
                    reason,
                }),
            }
        }
        // Rooms in first-seen order of the backwards scan, as the dictionary's.
        let mut exclusive: Vec<(String, Vec<String>)> = Vec::new();
        for i in (1..lines.len()).rev() {
            let parts: Vec<&str> = lines[i].split(" : ").collect();
            if parts.len() == 3 && parts[1] == "EXCLUSIVEROOM" {
                let names = parts[0].split(',').map(str::to_owned);
                match exclusive.iter_mut().find(|(room, _)| room == parts[2]) {
                    Some((_, list)) => list.extend(names),
                    None => exclusive.push((parts[2].to_owned(), names.collect())),
                }
                lines.remove(i);
            }
        }
        if let Some(end) = lines.iter().position(|l| l == "END CONDITIONAL LINKS")
            && end >= 1
        {
            for (room, names) in exclusive {
                lines.insert(
                    end - 1,
                    format!("{} : EXCLUSIVEROOM : {room}", names.join(",")),
                );
            }
        }
        Self { lines, rejected }
    }

    /// The last line index of each section marker (`MappingRooms`' scan): `(start, end)`
    /// inclusive bounds of the lines between `name` and `END name`.
    fn section(&self, name: &str) -> Option<(usize, usize)> {
        let end_marker = format!("END {name}");
        let start = self.lines.iter().rposition(|l| l == name)? + 1;
        let end = self.lines.iter().rposition(|l| *l == end_marker)?;
        Some((start, end))
    }
}

/// One abstract room as `WorldLoader` builds it.
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractRoomData {
    pub name: String,
    /// Global index.
    pub index: i32,
    /// Global index of the room behind each exit node, -1 for none.
    pub connections: Vec<i32>,
    pub nodes: Vec<AbstractRoomNode>,
    /// Width and height in tiles.
    pub size: (i32, i32),
    /// Position in the world's swarm room, shelter and gate lists, -1 if not one.
    pub swarm_room_index: i32,
    pub shelter_index: i32,
    pub gate_index: i32,
    /// `roomTags` (SCAVOUTPOST, SCAVTRADER and unknown tags).
    pub tags: Vec<String>,
    /// `REPLACEROOM`: the file loaded instead of `name`.
    pub alt_file_name: Option<String>,
    /// `PERF_HEAVY`.
    pub single_realized_room: bool,
    pub ancient_shelter: bool,
    pub offscreen_den: bool,
    /// `mapPos`: the room's centre on the region map (`map_<region>.txt`), shifted so
    /// the map's lowest corner is at the origin (`LoadMapConfig`); zero when unlisted.
    pub map_pos: [f32; 2],
}

impl AbstractRoomData {
    fn new(name: &str, connections: Vec<i32>, index: i32, lists: [i32; 3]) -> Self {
        let [swarm_room_index, shelter_index, gate_index] = lists;
        Self {
            name: name.to_owned(),
            index,
            connections,
            nodes: Vec::new(),
            size: (0, 0),
            swarm_room_index,
            shelter_index,
            gate_index,
            tags: Vec::new(),
            alt_file_name: None,
            single_realized_room: false,
            ancient_shelter: false,
            offscreen_den: false,
            map_pos: [0.0; 2],
        }
    }

    /// `AbstractRoom.FileName`.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.alt_file_name.as_deref().unwrap_or(&self.name)
    }

    #[must_use]
    pub fn shelter(&self) -> bool {
        self.shelter_index > -1
    }

    #[must_use]
    pub fn gate(&self) -> bool {
        self.gate_index > -1
    }

    /// `ExitIndex`: the exit leading to global room `target`, -1 if none.
    #[must_use]
    pub fn exit_index(&self, target: i32) -> i32 {
        self.connections
            .iter()
            .position(|&c| c == target)
            .map_or(-1, |i| i32::try_from(i).unwrap_or(-1))
    }

    /// `AddTag` (base game; `ARENA` and `NOTRACKERS` are More Slugcats and Remix tags).
    fn add_tag(&mut self, tag: &str, mods: ActiveMods) {
        match tag {
            "PERF_HEAVY" => self.single_realized_room = true,
            "ARENA" if mods.msc => self.single_realized_room = true,
            "NOTRACKERS" if mods.mmf => {}
            "ANCIENTSHELTER" => self.ancient_shelter = true,
            _ => self.tags.push(tag.to_owned()),
        }
    }

    /// `LoadAbstractRoom`: nodes and size from the room file.
    fn load_nodes(&mut self, assets: &StreamingAssets, file: &str) -> Result<(), WorldError> {
        let path = assets
            .find_room_file(&file.to_lowercase(), ".txt")
            .ok_or_else(|| WorldError::MissingRoom(file.to_owned()))?;
        let room =
            Room::load(&path).map_err(|e| WorldError::Room(file.to_owned(), e.to_string()))?;
        self.nodes = room
            .abstract_nodes(PREPROCESSING_GENERATION)
            .map_err(|e| WorldError::Room(file.to_owned(), e))?;
        self.size = (room.width(), room.height());
        Ok(())
    }

    /// `GenerateOffScreenDen`.
    fn offscreen_den(world: &str, index: i32) -> Self {
        let mut den = Self::new(&format!("OffScreenDen_{world}"), Vec::new(), index, [-1; 3]);
        den.nodes = [
            NodeType::Den,
            NodeType::RegionTransportation,
            NodeType::SideExit,
            NodeType::SkyExit,
            NodeType::SeaExit,
        ]
        .into_iter()
        .map(|kind| AbstractRoomNode {
            kind,
            shortcut_length: 0,
            submerged: kind == NodeType::SeaExit,
            viewed_by_camera: 0,
            entrance_width: 100,
        })
        .collect();
        den.offscreen_den = true;
        den
    }
}

/// Why a world could not be built (where the game's loader thread throws).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldError {
    Io(String),
    MissingRoom(String),
    Room(String, String),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) => write!(f, "{m}"),
            Self::MissingRoom(r) => write!(f, "room file of {r} not found"),
            Self::Room(r, m) => write!(f, "room {r}: {m}"),
        }
    }
}

impl std::error::Error for WorldError {}

/// `CONDITIONAL LINKS` entries for the timeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ConditionalLinks {
    /// `(room, target, replacement)`: a named target, or the n-th `DISCONNECTED` (1-based).
    links: Vec<(String, LinkTarget, String)>,
    replace_names: HashMap<String, String>,
    disabled_map_rooms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinkTarget {
    Room(String),
    Disconnected(i32),
}

impl ConditionalLinks {
    /// `LoadConditionalLinks` for one line.
    fn read(&mut self, line: &str, timeline: Option<Timeline>) -> Result<(), String> {
        let parts: Vec<&str> = line.split(" : ").collect();
        if parts.len() < 2 {
            return Err("conditional link without ' : '".to_owned());
        }
        let listed = parts[0]
            .split(',')
            .any(|t| timeline.is_some_and(|tl| t == tl.value()));
        let parts = Parts(parts);
        match parts.0[1] {
            "EXCLUSIVEROOM" => {
                if !listed {
                    self.disabled_map_rooms.push(parts.get(2)?);
                }
            }
            "HIDEROOM" => {
                if listed {
                    self.disabled_map_rooms.push(parts.get(2)?);
                }
            }
            "REPLACEROOM" => {
                if listed {
                    self.replace_names.insert(parts.get(2)?, parts.get(3)?);
                }
            }
            room => {
                if listed {
                    let target = parts.get(2)?;
                    let target = match target.trim().parse::<i32>() {
                        Ok(n) => LinkTarget::Disconnected(n),
                        Err(_) => LinkTarget::Room(target),
                    };
                    self.links.push((room.to_owned(), target, parts.get(3)?));
                }
            }
        }
        Ok(())
    }
}

/// Fields of a ` : `-separated line.
struct Parts<'a>(Vec<&'a str>);

impl Parts<'_> {
    fn get(&self, i: usize) -> Result<String, String> {
        self.0
            .get(i)
            .map(|s| (*s).to_owned())
            .ok_or_else(|| format!("field {i} missing"))
    }
}

/// What reading a world dropped or patched, for checking against the files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorldReport {
    pub rejected: Vec<RejectedLine>,
    /// `(room, exit node)` pairs given a `-1` connection by `CappingBrokenExits`.
    pub capped_exits: Vec<(String, usize)>,
    /// The `CREATURES` line the game's loader thread throws on (the lines after it are
    /// not read), if any.
    pub creatures_stopped_at: Option<String>,
}

/// A loaded world: its abstract rooms (the off-screen den last) and region.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldData {
    /// `World.name`: the region acronym, or the room of a single-room world.
    pub name: String,
    pub region: Option<RegionInfo>,
    pub single_room: bool,
    pub rooms: Vec<AbstractRoomData>,
    /// `DisabledMapRooms` (hidden from the map; swarm/scavenger tags dropped).
    pub disabled_map_rooms: Vec<String>,
    /// The region's `properties.txt` (defaults for a single-room world).
    pub properties: RegionProperties,
    /// The `CREATURES` section (`WorldLoader.spawners`); empty for a single-room world.
    pub spawners: Vec<CreatureSpawner>,
    /// `swarmRooms`: global indices of the `SWARMROOM` rooms, in file order.
    pub swarm_rooms: Vec<i32>,
    pub report: WorldReport,
}

/// Room lines being mapped (`roomAdder`, `roomTags` and the tag lists).
#[derive(Default)]
struct RoomMapping {
    adder: Vec<Vec<String>>,
    tags: Vec<Vec<String>>,
    swarm_rooms: Vec<i32>,
    shelters: Vec<i32>,
    gates: Vec<i32>,
}

impl RoomMapping {
    /// `MappingRooms` for one line: `NAME : conn, conn : TAG : TAG`.
    fn map(&mut self, line: &str, first: i32, disabled: &[String]) {
        if !line.contains(" : ") {
            return;
        }
        let parts: Vec<&str> = line.split(" : ").collect();
        let index = first + i32::try_from(self.adder.len()).unwrap_or(i32::MAX);
        let connections: Vec<String> = if parts[1].is_empty() {
            Vec::new()
        } else {
            let value = if parts[1].ends_with(',') {
                format!("{} ", parts[1])
            } else {
                parts[1].to_owned()
            };
            value.split(", ").map(str::to_owned).collect()
        };
        let mut tags = Vec::new();
        for &tag in &parts[2..] {
            match tag {
                "SWARMROOM" => {
                    if !disabled.iter().any(|d| d == parts[0]) {
                        self.swarm_rooms.push(index);
                    }
                }
                "SHELTER" | "ANCIENTSHELTER" => {
                    self.shelters.push(index);
                    if tag == "ANCIENTSHELTER" {
                        tags.push(tag.to_owned());
                    }
                }
                "GATE" => self.gates.push(index),
                "SCAVOUTPOST" | "SCAVTRADER" => {
                    if !disabled.iter().any(|d| d == parts[0]) {
                        tags.push(tag.to_owned());
                    }
                }
                other => tags.push(other.to_owned()),
            }
        }
        self.tags.push(tags);
        let mut entry = vec![parts[0].to_owned()];
        entry.extend(connections);
        self.adder.push(entry);
    }

    /// The `[swarm, shelter, gate]` list positions of global room `index`.
    fn list_indices(&self, index: i32) -> [i32; 3] {
        [&self.swarm_rooms, &self.shelters, &self.gates].map(|list| {
            list.iter()
                .position(|&i| i == index)
                .map_or(-1, |p| i32::try_from(p).unwrap_or(-1))
        })
    }
}

/// How to load a world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldRequest {
    /// Region acronym (`SU`), or the room name of a single-room world.
    pub name: String,
    pub single_room: bool,
    pub filter: LineFilter,
}

impl WorldRequest {
    /// `OverWorld.LoadFirstWorld`'s choice for a first room: the region named before its
    /// first `_` when `world/<region>` exists, else the one after it (gates,
    /// `GATE_SU_HI` is in SU) when the name has two `_`, else a single-room world of the
    /// room itself (arena levels).
    #[must_use]
    pub fn for_first_room(assets: &StreamingAssets, room: &str, filter: LineFilter) -> Self {
        let parts: Vec<&str> = room.split('_').collect();
        let region = if assets.has_world_dir(parts[0]) {
            Some(parts[0])
        } else if parts.len() > 2 && assets.has_world_dir(parts[1]) {
            Some(parts[1])
        } else {
            None
        };
        Self {
            name: region.unwrap_or(room).to_owned(),
            single_room: region.is_none(),
            filter,
        }
    }
}

impl WorldData {
    /// `WorldLoader` from `NextActivity` to `Finished` for the rooms (creatures, bat
    /// blockages and swarm maps are not loaded), then `World.LoadWorld`'s off-screen den.
    ///
    /// # Errors
    /// When the world file or a listed room file cannot be read.
    pub fn load(
        assets: &StreamingAssets,
        regions: &Regions,
        request: &WorldRequest,
    ) -> Result<Self, WorldError> {
        // `GetRegion`; a single-room world's name is a room, so it has none (the game's
        // `<room>_Properties.txt` region for levels is not read).
        let region = regions.find(&request.name).cloned();
        let first = region.as_ref().map_or(0, |r| r.first_room_index);
        let mut report = WorldReport::default();
        let mut links = ConditionalLinks::default();
        let mut mapping = RoomMapping::default();
        let mut spawners = Vec::new();
        let mut properties = RegionProperties::default();
        if request.single_room {
            mapping.adder.push(vec![request.name.clone()]);
            // `worldCreaturesSpawn` (on in a game) makes the room swarm room 0.
            mapping.swarm_rooms.push(0);
            mapping.tags.push(Vec::new());
        } else {
            let name = request.name.to_lowercase();
            let path = assets.file(&format!("world/{name}/world_{name}.txt"));
            let text = fs::read_to_string(&path)
                .map_err(|e| WorldError::Io(format!("{}: {e}", path.display())))?;
            let lines = WorldLines::preprocess(&text, &request.filter);
            report.rejected = lines.rejected.clone();
            if let Some((start, end)) = lines.section("CONDITIONAL LINKS") {
                for line in lines.lines.get(start..end).unwrap_or_default() {
                    if let Err(reason) = links.read(line, request.filter.timeline) {
                        report.rejected.push(RejectedLine {
                            line: 0,
                            text: line.clone(),
                            reason: Rejection::Malformed(reason),
                        });
                    }
                }
            }
            if let Some((start, end)) = lines.section("ROOMS") {
                for line in lines.lines.get(start..end).unwrap_or_default() {
                    mapping.map(line, first, &links.disabled_map_rooms);
                }
            }
            if let Some((start, end)) = lines.section("CREATURES") {
                let names: Vec<String> = mapping.adder.iter().map(|r| r[0].clone()).collect();
                let section = lines.lines.get(start..end).unwrap_or_default();
                let (read, stopped) = CreatureSpawner::read_section(
                    section.iter().map(String::as_str),
                    &names,
                    first,
                    request.filter.mods,
                );
                spawners = read;
                report.creatures_stopped_at = stopped;
            }
            properties = RegionProperties::load(assets, &request.name, &request.filter);
        }
        let mut rooms = Vec::with_capacity(mapping.adder.len() + 1);
        for i in 0..mapping.adder.len() {
            rooms.push(Self::create_room(
                assets,
                &mut mapping,
                &links,
                i,
                first,
                request.filter.mods,
            )?);
        }
        Self::cap_broken_exits(assets, &mapping, &mut rooms, first, &mut report)?;
        let den_index = first + i32::try_from(rooms.len()).unwrap_or(i32::MAX);
        rooms.push(AbstractRoomData::offscreen_den(&request.name, den_index));
        if !request.single_room {
            Self::load_map_positions(assets, &request.name, request.filter.timeline, &mut rooms);
        }
        Ok(Self {
            name: request.name.clone(),
            region,
            single_room: request.single_room,
            rooms,
            disabled_map_rooms: links.disabled_map_rooms,
            properties,
            spawners,
            swarm_rooms: mapping.swarm_rooms,
            report,
        })
    }

    /// `LoadMapConfig`'s room positions: `map_<region>-<timeline>.txt` if it exists,
    /// else `map_<region>.txt`; each `NAME: x><y><...` line sets that room's `mapPos`
    /// (names compared ignoring case); then every room, the off-screen den included, is
    /// shifted so the lowest `mapPos - size * 1.5` is zero.
    fn load_map_positions(
        assets: &StreamingAssets,
        region: &str,
        timeline: Option<Timeline>,
        rooms: &mut [AbstractRoomData],
    ) {
        let plain = assets.file(&format!("world/{region}/map_{region}.txt"));
        let path = timeline
            .map(|t| assets.file(&format!("world/{region}/map_{region}-{}.txt", t.value())))
            .filter(|p| p.is_file())
            .unwrap_or(plain);
        let Ok(text) = fs::read_to_string(&path) else {
            return;
        };
        for line in text.lines() {
            let line = if line.ends_with(':') {
                format!("{line} ")
            } else {
                line.to_owned()
            };
            let parts: Vec<&str> = line.split(": ").collect();
            if parts.len() != 2
                || parts[0] == "Connection"
                || parts[0].starts_with("SpawnMigration")
            {
                continue;
            }
            let Some(room) = rooms
                .iter_mut()
                .find(|r| r.name.eq_ignore_ascii_case(parts[0]))
            else {
                continue;
            };
            let mut values = parts[1].split("><");
            let x = values.next().and_then(|v| v.trim().parse::<f32>().ok());
            let y = values.next().and_then(|v| v.trim().parse::<f32>().ok());
            if let (Some(x), Some(y)) = (x, y) {
                room.map_pos = [x, y];
            }
        }
        let mut low = [f32::MAX; 2];
        for r in rooms.iter() {
            let size = [r.size.0 as f32, r.size.1 as f32];
            for k in 0..2 {
                low[k] = low[k].min(r.map_pos[k] - size[k] * 3.0 * 0.5);
            }
        }
        for r in rooms.iter_mut() {
            r.map_pos = [r.map_pos[0] - low[0], r.map_pos[1] - low[1]];
        }
    }

    /// `CreatingAbstractRooms` for room line `i`: conditional links, connections by
    /// name (the last room of that name; -1 for none), tags, the room file's nodes.
    fn create_room(
        assets: &StreamingAssets,
        mapping: &mut RoomMapping,
        links: &ConditionalLinks,
        i: usize,
        first: i32,
        mods: ActiveMods,
    ) -> Result<AbstractRoomData, WorldError> {
        let name = mapping.adder[i][0].clone();
        let mut disconnected_patches = Vec::new();
        for (room, target, replacement) in &links.links {
            if *room != name {
                continue;
            }
            match target {
                LinkTarget::Room(target) => {
                    if let Some(slot) = mapping.adder[i][1..].iter_mut().find(|c| *c == target) {
                        slot.clone_from(replacement);
                    }
                }
                LinkTarget::Disconnected(n) => {
                    let slot = mapping.adder[i]
                        .iter()
                        .enumerate()
                        .skip(1)
                        .filter(|(_, c)| *c == "DISCONNECTED")
                        .nth(usize::try_from(*n - 1).unwrap_or(usize::MAX))
                        .map(|(j, _)| j);
                    if let Some(j) = slot {
                        disconnected_patches.push((j, replacement.clone()));
                    }
                }
            }
        }
        for (j, replacement) in disconnected_patches {
            mapping.adder[i][j] = replacement;
        }
        let connections = mapping.adder[i][1..]
            .iter()
            .map(|c| {
                mapping
                    .adder
                    .iter()
                    .rposition(|other| other[0] == *c)
                    .map_or(-1, |m| first + i32::try_from(m).unwrap_or(i32::MAX))
            })
            .collect();
        let index = first + i32::try_from(i).unwrap_or(i32::MAX);
        let mut room =
            AbstractRoomData::new(&name, connections, index, mapping.list_indices(index));
        room.alt_file_name = links.replace_names.get(&name).cloned();
        for tag in &mapping.tags[i] {
            if !matches!(tag.as_str(), "" | "SWARMROOM" | "GATE" | "SHELTER") {
                room.add_tag(tag, mods);
            }
        }
        let file = room.file_name().to_owned();
        room.load_nodes(assets, &file)?;
        Ok(room)
    }

    /// `CappingBrokenExits`: each leading exit node past the end of the connections gets
    /// a `-1` connection, one at a time; each rebuilds the abstract room from its line's
    /// name, losing `REPLACEROOM` and tags as in the game.
    fn cap_broken_exits(
        assets: &StreamingAssets,
        mapping: &RoomMapping,
        rooms: &mut [AbstractRoomData],
        first: i32,
        report: &mut WorldReport,
    ) -> Result<(), WorldError> {
        let faulty: Vec<(usize, usize)> = rooms
            .iter()
            .enumerate()
            .flat_map(|(j, room)| {
                room.nodes
                    .iter()
                    .take_while(|n| n.kind == NodeType::Exit)
                    .enumerate()
                    .filter(|&(k, _)| k >= room.connections.len())
                    .map(move |(k, _)| (j, k))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (j, k) in faulty {
            let index = first + i32::try_from(j).unwrap_or(i32::MAX);
            let mut connections = rooms[j].connections.clone();
            connections.push(-1);
            let name = mapping.adder[j][0].clone();
            let mut room =
                AbstractRoomData::new(&name, connections, index, mapping.list_indices(index));
            room.load_nodes(assets, &name)?;
            rooms[j] = room;
            report.capped_exits.push((name, k));
        }
        Ok(())
    }

    /// A single-room world of a room already in memory (for tests and tools): its
    /// exits all capped to -1, as `CappingBrokenExits` does for a single-room world.
    #[must_use]
    pub fn from_room(room: &Room) -> Self {
        let name = room.name().to_owned();
        let nodes = room
            .abstract_nodes(PREPROCESSING_GENERATION)
            .unwrap_or_else(|_| room.shortcut_nodes());
        let exits = nodes
            .iter()
            .take_while(|n| n.kind == NodeType::Exit)
            .count();
        let mut data = AbstractRoomData::new(&name, vec![-1; exits], 0, [0, -1, -1]);
        data.nodes = nodes;
        data.size = (room.width(), room.height());
        let report = WorldReport {
            rejected: Vec::new(),
            capped_exits: (0..exits).map(|k| (name.clone(), k)).collect(),
            creatures_stopped_at: None,
        };
        Self {
            rooms: vec![data, AbstractRoomData::offscreen_den(&name, 1)],
            name,
            region: None,
            single_room: true,
            disabled_map_rooms: Vec::new(),
            properties: RegionProperties::default(),
            spawners: Vec::new(),
            swarm_rooms: vec![0],
            report,
        }
    }

    /// The room at global `index` (`World.GetAbstractRoom`).
    #[must_use]
    pub fn room(&self, index: i32) -> Option<&AbstractRoomData> {
        let first = self.first_room_index();
        let i = usize::try_from(index.checked_sub(first)?).ok()?;
        self.rooms.get(i)
    }

    /// The room named `name` (case-insensitive), the den included.
    #[must_use]
    pub fn room_named(&self, name: &str) -> Option<&AbstractRoomData> {
        self.rooms
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
    }

    /// `World.firstRoomIndex`.
    #[must_use]
    pub fn first_room_index(&self) -> i32 {
        self.region.as_ref().map_or(0, |r| r.first_room_index)
    }
}

/// A realized room's files: geometry and settings.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedRoom {
    pub room: Room,
    pub settings: RoomSettings,
}

/// Reads the rooms a world realizes (`new Room` and its `RoomSettings`), so the
/// simulation itself does no I/O.
pub trait RoomSource: Send + Sync {
    /// The room stored as `file` (an abstract room's `FileName`).
    ///
    /// # Errors
    /// When its geometry or settings cannot be read.
    fn load_room(&self, file: &str) -> Result<LoadedRoom, AssetError>;
}

/// [`RoomSource`] over the installed game: settings inherit from `region`'s templates
/// and prefer `_settings-<timeline>.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRooms {
    pub assets: StreamingAssets,
    /// Lower-case region whose templates apply; `None` for a single-room world.
    pub region: Option<String>,
    pub timeline: Timeline,
}

impl RoomSource for GameRooms {
    fn load_room(&self, file: &str) -> Result<LoadedRoom, AssetError> {
        let name = file.to_lowercase();
        let path =
            self.assets
                .find_room_file(&name, ".txt")
                .ok_or_else(|| AssetError::Missing {
                    what: format!("room {file}"),
                })?;
        let room = Room::load(&path).map_err(|e| AssetError::format(&path, e.to_string()))?;
        let timeline = self.timeline.value().to_lowercase();
        let settings =
            RoomSettings::load(&self.assets, &name, self.region.as_deref(), Some(&timeline))?;
        Ok(LoadedRoom { room, settings })
    }
}

/// [`RoomSource`] that always gives the same room (a single-room world in memory).
#[derive(Debug, Clone, PartialEq)]
pub struct FixedRoom(pub LoadedRoom);

impl RoomSource for FixedRoom {
    fn load_room(&self, _file: &str) -> Result<LoadedRoom, AssetError> {
        Ok(self.0.clone())
    }
}
