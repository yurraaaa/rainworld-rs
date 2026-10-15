//! Creatures as data files name them: `CreatureTemplate.Type` ([`CreatureType`]), the
//! world file's `CREATURES` section (`WorldLoader.AddSpawnersFromString`,
//! `AddLineageFromString`) and room attractions (`Room_Attr` lines of a region's
//! properties). The simulation decides what to make of them (`rw_sim::creature`).
//!
//! Behaviour: `rainworld-re/notes/creature-model.md`, `world-loading.md`.

use std::fmt;

use crate::ActiveMods;

/// `CreatureTemplate.Type`: a creature type by its `ExtEnum` value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreatureType(&'static str);

impl CreatureType {
    pub const STANDARD_GROUND_CREATURE: Self = Self("StandardGroundCreature");
    pub const SLUGCAT: Self = Self("Slugcat");
    pub const LIZARD_TEMPLATE: Self = Self("LizardTemplate");
    pub const PINK_LIZARD: Self = Self("PinkLizard");
    pub const GREEN_LIZARD: Self = Self("GreenLizard");
    pub const BLUE_LIZARD: Self = Self("BlueLizard");
    pub const YELLOW_LIZARD: Self = Self("YellowLizard");
    pub const WHITE_LIZARD: Self = Self("WhiteLizard");
    pub const RED_LIZARD: Self = Self("RedLizard");
    pub const BLACK_LIZARD: Self = Self("BlackLizard");
    pub const SALAMANDER: Self = Self("Salamander");
    pub const CYAN_LIZARD: Self = Self("CyanLizard");
    pub const FLY: Self = Self("Fly");
    pub const LEECH: Self = Self("Leech");
    pub const SEA_LEECH: Self = Self("SeaLeech");
    pub const SNAIL: Self = Self("Snail");
    pub const VULTURE: Self = Self("Vulture");
    pub const GARBAGE_WORM: Self = Self("GarbageWorm");
    pub const LANTERN_MOUSE: Self = Self("LanternMouse");
    pub const CICADA_A: Self = Self("CicadaA");
    pub const CICADA_B: Self = Self("CicadaB");
    pub const SPIDER: Self = Self("Spider");
    pub const JET_FISH: Self = Self("JetFish");
    pub const BIG_EEL: Self = Self("BigEel");
    pub const DEER: Self = Self("Deer");
    pub const TUBE_WORM: Self = Self("TubeWorm");
    pub const DADDY_LONG_LEGS: Self = Self("DaddyLongLegs");
    pub const BROTHER_LONG_LEGS: Self = Self("BrotherLongLegs");
    pub const TENTACLE_PLANT: Self = Self("TentaclePlant");
    pub const POLE_MIMIC: Self = Self("PoleMimic");
    pub const MIROS_BIRD: Self = Self("MirosBird");
    pub const TEMPLE_GUARD: Self = Self("TempleGuard");
    pub const CENTIPEDE: Self = Self("Centipede");
    pub const RED_CENTIPEDE: Self = Self("RedCentipede");
    pub const CENTIWING: Self = Self("Centiwing");
    pub const SMALL_CENTIPEDE: Self = Self("SmallCentipede");
    pub const SCAVENGER: Self = Self("Scavenger");
    pub const OVERSEER: Self = Self("Overseer");
    pub const VULTURE_GRUB: Self = Self("VultureGrub");
    pub const EGG_BUG: Self = Self("EggBug");
    pub const BIG_SPIDER: Self = Self("BigSpider");
    pub const SPITTER_SPIDER: Self = Self("SpitterSpider");
    pub const SMALL_NEEDLE_WORM: Self = Self("SmallNeedleWorm");
    pub const BIG_NEEDLE_WORM: Self = Self("BigNeedleWorm");
    pub const DROP_BUG: Self = Self("DropBug");
    pub const KING_VULTURE: Self = Self("KingVulture");
    pub const HAZER: Self = Self("Hazer");

    /// The base game's values in registration order (their `Index`).
    const BASE: [Self; 47] = [
        Self::STANDARD_GROUND_CREATURE,
        Self::SLUGCAT,
        Self::LIZARD_TEMPLATE,
        Self::PINK_LIZARD,
        Self::GREEN_LIZARD,
        Self::BLUE_LIZARD,
        Self::YELLOW_LIZARD,
        Self::WHITE_LIZARD,
        Self::RED_LIZARD,
        Self::BLACK_LIZARD,
        Self::SALAMANDER,
        Self::CYAN_LIZARD,
        Self::FLY,
        Self::LEECH,
        Self::SEA_LEECH,
        Self::SNAIL,
        Self::VULTURE,
        Self::GARBAGE_WORM,
        Self::LANTERN_MOUSE,
        Self::CICADA_A,
        Self::CICADA_B,
        Self::SPIDER,
        Self::JET_FISH,
        Self::BIG_EEL,
        Self::DEER,
        Self::TUBE_WORM,
        Self::DADDY_LONG_LEGS,
        Self::BROTHER_LONG_LEGS,
        Self::TENTACLE_PLANT,
        Self::POLE_MIMIC,
        Self::MIROS_BIRD,
        Self::TEMPLE_GUARD,
        Self::CENTIPEDE,
        Self::RED_CENTIPEDE,
        Self::CENTIWING,
        Self::SMALL_CENTIPEDE,
        Self::SCAVENGER,
        Self::OVERSEER,
        Self::VULTURE_GRUB,
        Self::EGG_BUG,
        Self::BIG_SPIDER,
        Self::SPITTER_SPIDER,
        Self::SMALL_NEEDLE_WORM,
        Self::BIG_NEEDLE_WORM,
        Self::DROP_BUG,
        Self::KING_VULTURE,
        Self::HAZER,
    ];

    /// `ExtEnum<CreatureTemplate.Type>.values` with `mods` active. More Slugcats and the
    /// Watcher append theirs when they are supported.
    #[must_use]
    pub fn registered(_mods: ActiveMods) -> Vec<Self> {
        Self::BASE.to_vec()
    }

    /// The registered value spelled `value` (`new CreatureTemplate.Type(value)` with an
    /// index).
    #[must_use]
    pub fn from_value(value: &str, mods: ActiveMods) -> Option<Self> {
        Self::registered(mods).into_iter().find(|t| t.0 == value)
    }

    /// `Index`.
    #[must_use]
    pub fn index(self, mods: ActiveMods) -> Option<usize> {
        Self::registered(mods).iter().position(|&t| t == self)
    }

    #[must_use]
    pub fn value(self) -> &'static str {
        self.0
    }

    /// `CreatureTemplate.name` as the constructor sets it ("???" for types it does not
    /// name).
    #[must_use]
    pub fn template_name(self) -> &'static str {
        match self.0 {
            "LizardTemplate" => "Lizard",
            "PinkLizard" => "Pink Lizard",
            "GreenLizard" => "Green Lizard",
            "BlueLizard" => "Blue Lizard",
            "YellowLizard" => "Yellow Lizard",
            "WhiteLizard" => "White Lizard",
            "RedLizard" => "Red Lizard",
            "BlackLizard" => "Black Lizard",
            "CyanLizard" => "Cyan Lizard",
            "SeaLeech" => "Sea Leech",
            "GarbageWorm" => "Garbage Worm",
            "LanternMouse" => "Lantern Mouse",
            "CicadaA" => "Cicada A",
            "CicadaB" => "Cicada B",
            "JetFish" => "Jet Fish",
            "BigEel" => "Big Eel",
            "TubeWorm" => "Tube Worm",
            "DaddyLongLegs" => "Daddy Long Legs",
            "BrotherLongLegs" => "Brother Long Legs",
            "TentaclePlant" => "Tentacle Plant",
            "PoleMimic" => "Pole Mimic",
            "MirosBird" => "Miros Bird",
            "TempleGuard" => "Temple Guard",
            "RedCentipede" => "Red Centipede",
            "SmallCentipede" => "Small Centipede",
            "VultureGrub" => "Vulture Grub",
            "BigSpider" => "Big Spider",
            "SpitterSpider" => "Spitter Spider",
            "EggBug" => "Egg Bug",
            "SmallNeedleWorm" => "Small Needle",
            "BigNeedleWorm" => "Big Needle",
            "DropBug" => "Drop Bug",
            "KingVulture" => "King Vulture",
            "StandardGroundCreature"
            | "Slugcat"
            | "Salamander"
            | "Fly"
            | "Leech"
            | "Snail"
            | "Vulture"
            | "Spider"
            | "Deer"
            | "Centipede"
            | "Scavenger"
            | "Centiwing"
            | "Overseer"
            | "Hazer" => self.0,
            _ => "???",
        }
    }

    /// `WorldLoader.CreatureTypeFromString`: the world files' spellings (case-insensitive
    /// aliases), else a template's name or type value. The More Slugcats and Watcher
    /// aliases are not ported; "cicada" draws from `System.Random` in the game and is
    /// not used by the vanilla files (taken as Cicada A here).
    #[must_use]
    pub fn from_world_string(s: &str, mods: ActiveMods) -> Option<Self> {
        let alias = match s.to_lowercase().as_str() {
            "pink" => Some(Self::PINK_LIZARD),
            "green" => Some(Self::GREEN_LIZARD),
            "blue" => Some(Self::BLUE_LIZARD),
            "yellow" => Some(Self::YELLOW_LIZARD),
            "white" => Some(Self::WHITE_LIZARD),
            "red" => Some(Self::RED_LIZARD),
            "black" => Some(Self::BLACK_LIZARD),
            "cyan" => Some(Self::CYAN_LIZARD),
            "leech" => Some(Self::LEECH),
            "sea leech" | "sealeech" => Some(Self::SEA_LEECH),
            "snail" => Some(Self::SNAIL),
            "vulture" => Some(Self::VULTURE),
            "cicada a" | "cicadaa" | "cicada" => Some(Self::CICADA_A),
            "cicada b" | "cicadab" => Some(Self::CICADA_B),
            "lantern mouse" | "lanternmouse" | "mouse" => Some(Self::LANTERN_MOUSE),
            "spider" => Some(Self::SPIDER),
            "worm" | "garbage worm" | "garbageworm" => Some(Self::GARBAGE_WORM),
            "leviathan" | "lev" | "bigeel" | "big eel" => Some(Self::BIG_EEL),
            "tube" | "tube worm" | "tubeworm" => Some(Self::TUBE_WORM),
            "daddy" | "daddy long legs" | "daddylonglegs" => Some(Self::DADDY_LONG_LEGS),
            "bro" | "bro long legs" | "brolonglegs" => Some(Self::BROTHER_LONG_LEGS),
            "tentacleplant" | "tentacle plant" | "tentacle" => Some(Self::TENTACLE_PLANT),
            "polemimic" | "pole mimic" | "mimic" => Some(Self::POLE_MIMIC),
            "mirosbird" | "miros bird" | "miros" => Some(Self::MIROS_BIRD),
            "centipede" | "cent" => Some(Self::CENTIPEDE),
            "jetfish" | "jet fish" => Some(Self::JET_FISH),
            "eggbug" | "egg bug" => Some(Self::EGG_BUG),
            "bigspider" | "big spider" => Some(Self::BIG_SPIDER),
            "spitterspider" | "spitter spider" => Some(Self::SPITTER_SPIDER),
            "bigneedle" | "big needle" | "needle" | "needle worm" => Some(Self::BIG_NEEDLE_WORM),
            "smallneedle" | "small needle" => Some(Self::SMALL_NEEDLE_WORM),
            "dropbug" | "drop bug" | "dropwig" | "drop wig" => Some(Self::DROP_BUG),
            "kingvulture" | "king vulture" => Some(Self::KING_VULTURE),
            "red centipede" | "redcentipede" | "red centi" | "redcenti" => {
                Some(Self::RED_CENTIPEDE)
            }
            _ => None,
        };
        alias.or_else(|| {
            Self::registered(mods)
                .into_iter()
                .find(|t| s == t.template_name() || s == t.0)
        })
    }
}

impl fmt::Debug for CreatureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Display for CreatureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// `AbstractRoom.CreatureRoomAttraction`. An unknown spelling has no index in the game
/// and acts as Neutral everywhere the base game reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoomAttraction {
    #[default]
    Neutral,
    Forbidden,
    Avoid,
    Like,
    Stay,
}

impl RoomAttraction {
    /// A registered value (`new CreatureRoomAttraction(value).Index != -1`).
    #[must_use]
    pub fn registered(value: &str) -> Option<Self> {
        match value {
            "Neutral" => Some(Self::Neutral),
            "Forbidden" | "Avoid" | "Like" | "Stay" => Some(Self::from_value(value)),
            _ => None,
        }
    }

    /// `new CreatureRoomAttraction(value)`.
    #[must_use]
    pub fn from_value(value: &str) -> Self {
        match value {
            "Forbidden" => Self::Forbidden,
            "Avoid" => Self::Avoid,
            "Like" => Self::Like,
            "Stay" => Self::Stay,
            _ => Self::Neutral,
        }
    }

    /// `BackwardsCompatibilityRemix.ParseRoomAttraction` for numeric values.
    #[must_use]
    pub fn from_number(n: i32) -> Self {
        match n {
            1 => Self::Forbidden,
            2 => Self::Avoid,
            3 => Self::Like,
            4 => Self::Stay,
            _ => Self::Neutral,
        }
    }

    /// `CreatureAttractionToFloat`.
    #[must_use]
    pub fn value(self) -> f32 {
        match self {
            Self::Forbidden => 0.0,
            Self::Avoid => 0.25,
            Self::Neutral => 0.5,
            Self::Like => 0.75,
            Self::Stay => 1.0,
        }
    }
}

/// A spawner's den: `WorldCoordinate(room, -1, -1, node)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnerDen {
    /// Global room index; -1 when the line names no room of the world.
    pub room: i32,
    pub node: i32,
}

/// `World.SimpleSpawner`: `amount` creatures of one type in a den.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleSpawner {
    pub den: SpawnerDen,
    pub creature: CreatureType,
    /// The `{...}` part (`spawnDataString`).
    pub spawn_data: Option<String>,
    pub amount: i32,
}

/// `World.Lineage`: a den whose creature advances through `creatures` over cycles.
#[derive(Debug, Clone, PartialEq)]
pub struct Lineage {
    pub den: SpawnerDen,
    /// `None` for NONE or an unknown type (`-1` in the game).
    pub creatures: Vec<Option<CreatureType>>,
    pub progression_chances: Vec<f32>,
    pub spawn_data: Vec<Option<String>>,
    /// Count of earlier lineages at the same den plus one, or 0 for the first.
    pub den_lineage: i32,
    /// A fifth ` : ` field (`nightCreature`).
    pub night_creature: bool,
}

/// One entry of `WorldLoader.spawners`, in file order; its position is the spawner
/// number.
#[derive(Debug, Clone, PartialEq)]
pub enum CreatureSpawner {
    Simple(SimpleSpawner),
    Lineage(Lineage),
}

impl CreatureSpawner {
    #[must_use]
    pub fn den(&self) -> SpawnerDen {
        match self {
            Self::Simple(s) => s.den,
            Self::Lineage(l) => l.den,
        }
    }

    /// `FindingCreatures` over the `CREATURES` lines: `rooms` are the `ROOMS` names in
    /// order (`roomAdder`), the off-screen den follows them. Stops at the first line the
    /// game's loader thread would throw on, returning what was read and that line.
    pub fn read_section<'a>(
        lines: impl IntoIterator<Item = &'a str>,
        rooms: &[String],
        first_room_index: i32,
        mods: ActiveMods,
    ) -> (Vec<Self>, Option<String>) {
        let mut spawners = Vec::new();
        for line in lines {
            let parts: Vec<&str> = line.split(" : ").collect();
            let read = if parts[0] == "LINEAGE" {
                Lineage::read(&parts, rooms, first_room_index, &spawners, mods)
                    .map(|l| l.map(Self::Lineage).into_iter().collect())
            } else {
                SimpleSpawner::read(&parts, rooms, first_room_index, mods)
                    .map(|s| s.into_iter().map(Self::Simple).collect::<Vec<_>>())
            };
            match read {
                Some(new) => spawners.extend(new),
                None => return (spawners, Some(line.to_owned())),
            }
        }
        (spawners, None)
    }
}

impl SimpleSpawner {
    /// `AddSpawnersFromString`; `None` where the game throws.
    fn read(parts: &[&str], rooms: &[String], first: i32, mods: ActiveMods) -> Option<Vec<Self>> {
        let room = if parts[0] == "OFFSCREEN" {
            first + i32::try_from(rooms.len()).ok()?
        } else {
            rooms
                .iter()
                .position(|r| r == parts[0])
                .map_or(-1, |i| first + i32::try_from(i).unwrap_or(0))
        };
        let list = parts.get(1)?;
        let list = if list.ends_with(',') {
            format!("{list} ")
        } else {
            (*list).to_owned()
        };
        let mut spawners = Vec::new();
        for entry in list.split(", ") {
            let fields: Vec<&str> = entry.split('-').collect();
            let creature = CreatureType::from_world_string(fields.get(1)?, mods);
            let (amount, spawn_data) = Self::amount_and_data(&fields[2..]);
            if let Some(creature) = creature {
                spawners.push(Self {
                    den: SpawnerDen {
                        room,
                        node: fields[0].trim().parse().ok()?,
                    },
                    creature,
                    spawn_data,
                    amount,
                });
            }
        }
        Some(spawners)
    }

    /// The fields after the type: a `{...}` block (which may contain `-`) is the spawn
    /// data, any other field the amount (1 if it does not parse).
    fn amount_and_data(fields: &[&str]) -> (i32, Option<String>) {
        let mut amount = 1;
        let mut data = String::new();
        let mut in_data = false;
        for field in fields {
            if field.starts_with('{') {
                data = (*field).to_owned();
                in_data = true;
            } else if in_data {
                data = format!("{data}-{field}");
            } else {
                amount = field.trim().parse().unwrap_or(1);
            }
            if field.contains('}') {
                in_data = false;
            }
        }
        (amount, (!data.is_empty()).then_some(data))
    }
}

impl Lineage {
    /// `AddLineageFromString`: `Some(None)` for a den room the world lacks (skipped),
    /// `None` where the game throws.
    fn read(
        parts: &[&str],
        rooms: &[String],
        first: i32,
        earlier: &[CreatureSpawner],
        mods: ActiveMods,
    ) -> Option<Option<Self>> {
        let room_name = parts.get(1)?;
        let room = if *room_name == "OFFSCREEN" {
            Some(rooms.len())
        } else {
            rooms.iter().position(|r| r == room_name)
        };
        let Some(room) = room else {
            return Some(None);
        };
        let room = first + i32::try_from(room).ok()?;
        let node: i32 = parts.get(2)?.trim().parse().ok()?;
        let list = parts.get(3)?;
        let list = if list.ends_with(',') {
            format!("{list} ")
        } else {
            (*list).to_owned()
        };
        let mut lineage = Self {
            den: SpawnerDen { room, node },
            creatures: Vec::new(),
            progression_chances: Vec::new(),
            spawn_data: Vec::new(),
            den_lineage: 0,
            night_creature: parts.len() > 4,
        };
        for entry in list.split(", ") {
            let fields: Vec<&str> = entry.split('-').collect();
            lineage
                .creatures
                .push(if fields[0].to_lowercase() == "none" {
                    None
                } else {
                    CreatureType::from_world_string(fields[0], mods)
                });
            // The game catches what this throws and drops the lineage.
            let Some((chance, data)) = Self::chance_and_data(&fields[1..]) else {
                return Some(None);
            };
            lineage.progression_chances.push(chance);
            lineage.spawn_data.push(data);
        }
        let same_den = earlier
            .iter()
            .filter(|s| matches!(s, CreatureSpawner::Lineage(l) if l.den == lineage.den))
            .count();
        if same_den > 0 {
            lineage.den_lineage = i32::try_from(same_den).ok()? + 1;
        }
        Some(Some(lineage))
    }

    /// One entry's fields after the type: a `{...}` block is the spawn data, any other
    /// field the progression chance (0 if it does not parse). `None` for an empty field
    /// (`array5[k][0]` throws).
    fn chance_and_data(fields: &[&str]) -> Option<(f32, Option<String>)> {
        let mut chance = 0.0;
        let mut data = String::new();
        let mut in_data = false;
        for field in fields {
            if field.chars().next()? == '{' {
                data = (*field).to_owned();
                in_data = true;
            } else if in_data {
                data = format!("{data}-{field}");
            } else {
                chance = field.trim().parse().unwrap_or(0.0);
            }
            if field.contains('}') {
                in_data = false;
            }
        }
        Some((chance, (!data.is_empty()).then_some(data)))
    }
}
