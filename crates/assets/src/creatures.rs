//! Creatures as data files name them: `CreatureTemplate.Type` ([`CreatureType`]), the
//! world file's `CREATURES` section (`WorldLoader.AddSpawnersFromString`,
//! `AddLineageFromString`) and room attractions (`Room_Attr` lines of a region's
//! properties). The simulation decides what to make of them (`rw_sim::creature`).
//!
//! Behaviour: `rainworld-re/notes/creature-model/model.md`, `world-loading.md`.
//!
//! Mirrors: CreatureTemplate.cs, WorldLoader.cs (AddSpawnersFromString, AddLineageFromString)

use crate::ActiveMods;
use crate::identity::ext_enum;

ext_enum! {
    /// `CreatureTemplate.Type`: a creature type by its `ExtEnum` value.
    pub enum CreatureType {
        Base {
            StandardGroundCreature = "StandardGroundCreature",
            Slugcat = "Slugcat",
            LizardTemplate = "LizardTemplate",
            PinkLizard = "PinkLizard",
            GreenLizard = "GreenLizard",
            BlueLizard = "BlueLizard",
            YellowLizard = "YellowLizard",
            WhiteLizard = "WhiteLizard",
            RedLizard = "RedLizard",
            BlackLizard = "BlackLizard",
            Salamander = "Salamander",
            CyanLizard = "CyanLizard",
            Fly = "Fly",
            Leech = "Leech",
            SeaLeech = "SeaLeech",
            Snail = "Snail",
            Vulture = "Vulture",
            GarbageWorm = "GarbageWorm",
            LanternMouse = "LanternMouse",
            CicadaA = "CicadaA",
            CicadaB = "CicadaB",
            Spider = "Spider",
            JetFish = "JetFish",
            BigEel = "BigEel",
            Deer = "Deer",
            TubeWorm = "TubeWorm",
            DaddyLongLegs = "DaddyLongLegs",
            BrotherLongLegs = "BrotherLongLegs",
            TentaclePlant = "TentaclePlant",
            PoleMimic = "PoleMimic",
            MirosBird = "MirosBird",
            TempleGuard = "TempleGuard",
            Centipede = "Centipede",
            RedCentipede = "RedCentipede",
            Centiwing = "Centiwing",
            SmallCentipede = "SmallCentipede",
            Scavenger = "Scavenger",
            Overseer = "Overseer",
            VultureGrub = "VultureGrub",
            EggBug = "EggBug",
            BigSpider = "BigSpider",
            SpitterSpider = "SpitterSpider",
            SmallNeedleWorm = "SmallNeedleWorm",
            BigNeedleWorm = "BigNeedleWorm",
            DropBug = "DropBug",
            KingVulture = "KingVulture",
            Hazer = "Hazer",
        }
        DlcShared {
            MirosVulture = "MirosVulture",
            SpitLizard = "SpitLizard",
            EelLizard = "EelLizard",
            MotherSpider = "MotherSpider",
            TerrorLongLegs = "TerrorLongLegs",
            AquaCenti = "AquaCenti",
            StowawayBug = "StowawayBug",
            ScavengerElite = "ScavengerElite",
            Inspector = "Inspector",
            Yeek = "Yeek",
            BigJelly = "BigJelly",
            JungleLeech = "JungleLeech",
            ZoopLizard = "ZoopLizard",
        }
        Msc {
            HunterDaddy = "HunterDaddy",
            FireBug = "FireBug",
            SlugNpc = "SlugNPC",
            ScavengerKing = "ScavengerKing",
            TrainLizard = "TrainLizard",
        }
        Watcher {
            DrillCrab = "DrillCrab",
            TowerCrab = "TowerCrab",
            Barnacle = "Barnacle",
            SandGrub = "SandGrub",
            BigSandGrub = "BigSandGrub",
            BigMoth = "BigMoth",
            SmallMoth = "SmallMoth",
            BoxWorm = "BoxWorm",
            FireSprite = "FireSprite",
            Rattler = "Rattler",
            SkyWhale = "SkyWhale",
            ScavengerTemplar = "ScavengerTemplar",
            ScavengerDisciple = "ScavengerDisciple",
            Loach = "Loach",
            RotLoach = "RotLoach",
            BlizzardLizard = "BlizzardLizard",
            BasiliskLizard = "BasiliskLizard",
            IndigoLizard = "IndigoLizard",
            PeachLizard = "PeachLizard",
            Rat = "Rat",
            Frog = "Frog",
            Tardigrade = "Tardigrade",
            GrappleSnake = "GrappleSnake",
            Millipede = "Millipede",
            Angler = "Angler",
            RippleSpider = "RippleSpider",
            MothGrub = "MothGrub",
        }
    }
}

impl CreatureType {
    /// The registered value spelled `value` (`new CreatureTemplate.Type(value)` with an
    /// index).
    #[must_use]
    pub fn from_value(value: &str, mods: ActiveMods) -> Option<Self> {
        Self::registered(mods)
            .into_iter()
            .find(|t| t.value() == value)
    }

    /// `Index`.
    #[must_use]
    pub fn index(self, mods: ActiveMods) -> Option<usize> {
        Self::registered(mods).iter().position(|&t| t == self)
    }

    /// `CreatureTemplate.name` as the constructor sets it ("???" for types it does not
    /// name, the DLC ones among them).
    #[must_use]
    pub fn template_name(self) -> &'static str {
        match self {
            Self::LizardTemplate => "Lizard",
            Self::PinkLizard => "Pink Lizard",
            Self::GreenLizard => "Green Lizard",
            Self::BlueLizard => "Blue Lizard",
            Self::YellowLizard => "Yellow Lizard",
            Self::WhiteLizard => "White Lizard",
            Self::RedLizard => "Red Lizard",
            Self::BlackLizard => "Black Lizard",
            Self::CyanLizard => "Cyan Lizard",
            Self::SeaLeech => "Sea Leech",
            Self::GarbageWorm => "Garbage Worm",
            Self::LanternMouse => "Lantern Mouse",
            Self::CicadaA => "Cicada A",
            Self::CicadaB => "Cicada B",
            Self::JetFish => "Jet Fish",
            Self::BigEel => "Big Eel",
            Self::TubeWorm => "Tube Worm",
            Self::DaddyLongLegs => "Daddy Long Legs",
            Self::BrotherLongLegs => "Brother Long Legs",
            Self::TentaclePlant => "Tentacle Plant",
            Self::PoleMimic => "Pole Mimic",
            Self::MirosBird => "Miros Bird",
            Self::TempleGuard => "Temple Guard",
            Self::RedCentipede => "Red Centipede",
            Self::SmallCentipede => "Small Centipede",
            Self::VultureGrub => "Vulture Grub",
            Self::BigSpider => "Big Spider",
            Self::SpitterSpider => "Spitter Spider",
            Self::EggBug => "Egg Bug",
            Self::SmallNeedleWorm => "Small Needle",
            Self::BigNeedleWorm => "Big Needle",
            Self::DropBug => "Drop Bug",
            Self::KingVulture => "King Vulture",
            Self::StandardGroundCreature
            | Self::Slugcat
            | Self::Salamander
            | Self::Fly
            | Self::Leech
            | Self::Snail
            | Self::Vulture
            | Self::Spider
            | Self::Deer
            | Self::Centipede
            | Self::Scavenger
            | Self::Centiwing
            | Self::Overseer
            | Self::Hazer => self.value(),
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
            "pink" => Some(Self::PinkLizard),
            "green" => Some(Self::GreenLizard),
            "blue" => Some(Self::BlueLizard),
            "yellow" => Some(Self::YellowLizard),
            "white" => Some(Self::WhiteLizard),
            "red" => Some(Self::RedLizard),
            "black" => Some(Self::BlackLizard),
            "cyan" => Some(Self::CyanLizard),
            "leech" => Some(Self::Leech),
            "sea leech" | "sealeech" => Some(Self::SeaLeech),
            "snail" => Some(Self::Snail),
            "vulture" => Some(Self::Vulture),
            "cicada a" | "cicadaa" | "cicada" => Some(Self::CicadaA),
            "cicada b" | "cicadab" => Some(Self::CicadaB),
            "lantern mouse" | "lanternmouse" | "mouse" => Some(Self::LanternMouse),
            "spider" => Some(Self::Spider),
            "worm" | "garbage worm" | "garbageworm" => Some(Self::GarbageWorm),
            "leviathan" | "lev" | "bigeel" | "big eel" => Some(Self::BigEel),
            "tube" | "tube worm" | "tubeworm" => Some(Self::TubeWorm),
            "daddy" | "daddy long legs" | "daddylonglegs" => Some(Self::DaddyLongLegs),
            "bro" | "bro long legs" | "brolonglegs" => Some(Self::BrotherLongLegs),
            "tentacleplant" | "tentacle plant" | "tentacle" => Some(Self::TentaclePlant),
            "polemimic" | "pole mimic" | "mimic" => Some(Self::PoleMimic),
            "mirosbird" | "miros bird" | "miros" => Some(Self::MirosBird),
            "centipede" | "cent" => Some(Self::Centipede),
            "jetfish" | "jet fish" => Some(Self::JetFish),
            "eggbug" | "egg bug" => Some(Self::EggBug),
            "bigspider" | "big spider" => Some(Self::BigSpider),
            "spitterspider" | "spitter spider" => Some(Self::SpitterSpider),
            "bigneedle" | "big needle" | "needle" | "needle worm" => Some(Self::BigNeedleWorm),
            "smallneedle" | "small needle" => Some(Self::SmallNeedleWorm),
            "dropbug" | "drop bug" | "dropwig" | "drop wig" => Some(Self::DropBug),
            "kingvulture" | "king vulture" => Some(Self::KingVulture),
            "red centipede" | "redcentipede" | "red centi" | "redcenti" => Some(Self::RedCentipede),
            _ => None,
        };
        alias.or_else(|| {
            Self::registered(mods)
                .into_iter()
                .find(|t| s == t.template_name() || s == t.value())
        })
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
