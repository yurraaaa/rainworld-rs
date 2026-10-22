//! Room effects and placed objects of a settings file, by type name: what decides a
//! room's entry delay (`Room.Loaded`'s `waitToEnterAfterFullyLoaded`) and, later, what it
//! places. `RoomSettings.LoadEffects`, `LoadPlacedObjects`, `PlacedObject.FromString`,
//! `PlacedObject.FilterData`.
//!
//! Mirrors: RoomSettings.cs (LoadEffects, LoadPlacedObjects), PlacedObject.cs

use crate::identity::ext_enum;
use crate::{ActiveMods, MultiplayerItemType, PlacedObjectType, SlugcatName, Timeline};

/// `RoomSettings.RoomEffect`: type name and amount.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomEffect {
    pub kind: String,
    pub amount: f32,
}

/// `PlacedObject`: type name, room-pixel position and the unparsed data field.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    pub kind: String,
    pub pos: [f32; 2],
    pub data: String,
    /// Cleared by a filter that excludes the timeline.
    pub active: bool,
}

impl RoomEffect {
    /// The entries of an `Effects:` value (`Name-amount-...`, separated by `", "`).
    #[must_use]
    pub fn parse_list(value: &str) -> Vec<Self> {
        SpacedList::split(value)
            .filter_map(|entry| {
                let mut fields = entry.split('-');
                let kind = fields.next().filter(|k| !k.is_empty())?;
                let amount = fields
                    .next()
                    .and_then(|a| a.trim().parse().ok())
                    .unwrap_or(0.0);
                Some(Self {
                    kind: kind.to_owned(),
                    amount,
                })
            })
            .collect()
    }

    /// `InheritEffects`: own effects, then the parent's of a type the room lacks.
    #[must_use]
    pub fn inherit(own: &[Self], parent: &[Self]) -> Vec<Self> {
        let mut effects = own.to_vec();
        for p in parent {
            if !own.iter().any(|e| e.kind == p.kind) {
                effects.push(p.clone());
            }
        }
        effects
    }
}

impl PlacedObject {
    /// The object's type, if `mods` register it.
    #[must_use]
    pub fn object_type(&self, mods: ActiveMods) -> Option<PlacedObjectType> {
        PlacedObjectType::from_value(&self.kind, mods)
    }

    /// The entries of a `PlacedObjects:` value (`Type><x><y><data`, separated by `", "`).
    /// An entry whose position is not a number is skipped (the game would throw).
    #[must_use]
    pub fn parse_list(value: &str) -> Vec<Self> {
        SpacedList::split(value)
            .filter_map(|entry| {
                let fields: Vec<&str> = entry.trim().split("><").collect();
                let kind = *fields.first().filter(|k| !k.is_empty())?;
                let x = fields.get(1)?.trim().parse().ok()?;
                let y = fields.get(2)?.trim().parse().ok()?;
                Some(Self {
                    kind: kind.to_owned(),
                    pos: [x, y],
                    data: fields.get(3).copied().unwrap_or("").to_owned(),
                    active: true,
                })
            })
            .collect()
    }

    /// `LoadPlacedObjects`' filtering: every `Filter` whose slugcats exclude `timeline`
    /// deactivates the deactivatable objects within its radius. (Remix, More Slugcats
    /// and Watcher filter types are not read.)
    pub fn apply_filters(objects: &mut [Self], timeline: Timeline, mods: ActiveMods) {
        let filters: Vec<([f32; 2], f32)> = objects
            .iter()
            .filter(|o| o.kind == "Filter")
            .filter_map(|o| FilterData::parse(&o.data, mods).map(|f| (o.pos, f)))
            .filter(|(_, f)| !f.active(timeline))
            .map(|(pos, f)| (pos, f.rad()))
            .collect();
        for o in objects.iter_mut().filter(|o| o.deactivatable()) {
            if filters.iter().any(|&(pos, rad)| o.within(pos, rad)) {
                o.active = false;
            }
        }
    }

    /// `PlacedObject.deactivattable`: everything but decals, filters and terrain shapes.
    #[must_use]
    pub fn deactivatable(&self) -> bool {
        !matches!(
            self.kind.as_str(),
            "CustomDecal"
                | "Filter"
                | "RippleLevelFilter"
                | "PrinceFilter"
                | "RippleEggFilter"
                | "RippleEggDestination"
                | "WarpFilter"
                | "SuperSlope"
                | "CurvedSlope"
                | "LocalTerrain"
                | "CompetitiveFilter"
                | "TerrainHandle"
        )
    }

    /// `Custom.DistLess(pos, centre, rad)`.
    fn within(&self, centre: [f32; 2], rad: f32) -> bool {
        let (dx, dy) = (self.pos[0] - centre[0], self.pos[1] - centre[1]);
        dx * dx + dy * dy < rad * rad
    }
}

/// `PlacedObject.ResizableObjectData` (a `NoSpearStickZone`'s): the handle, whose length
/// is the radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizableData {
    pub handle: [f32; 2],
}

impl ResizableData {
    /// `handle.x~handle.y~...`; `None` where the game's `float.Parse` would throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let mut fields = data.split('~');
        let x = fields.next()?.trim().parse().ok()?;
        let y = fields.next()?.trim().parse().ok()?;
        Some(Self { handle: [x, y] })
    }

    /// `Rad`: the handle's length.
    #[must_use]
    pub fn rad(self) -> f32 {
        (self.handle[0] * self.handle[0] + self.handle[1] * self.handle[1]).sqrt()
    }
}

/// `PlacedObject.QuadObjectData` (a `SuperJumpInstruction`'s): three handles relative
/// to the object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadData {
    pub handles: [[f32; 2]; 3],
}

impl QuadData {
    /// `h0.x~h0.y~h1.x~h1.y~h2.x~h2.y~...`; `None` where the game's `float.Parse` would
    /// throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let mut fields = data.split('~');
        let mut handles = [[0.0; 2]; 3];
        for h in &mut handles {
            h[0] = fields.next()?.trim().parse().ok()?;
            h[1] = fields.next()?.trim().parse().ok()?;
        }
        Some(Self { handles })
    }
}

ext_enum! {
    /// `ReliableIggyDirection.ReliableIggyDirectionData.Symbol`.
    pub enum IggySymbol {
        Base {
            Shelter = "Shelter",
            DynamicDirection = "DynamicDirection",
            SlugcatFace = "SlugcatFace",
            Food = "Food",
            Bat = "Bat",
            Danger = "Danger",
        }
    }
}

ext_enum! {
    /// `ReliableIggyDirection.ReliableIggyDirectionData.Condition`.
    pub enum IggyCondition {
        Base {
            AnyTime = "AnyTime",
            BeforeMoon = "BeforeMoon",
            AfterMoon = "AfterMoon",
            AfterPebblesAndMoon = "AfterPebblesAndMoon",
        }
    }
}

/// `ReliableIggyDirectionData`: where and when the guide is made to point at an exit.
/// A symbol or condition no mod registers is `None` (it equals none of the game's).
#[derive(Debug, Clone, PartialEq)]
pub struct ReliableIggyDirectionData {
    pub handle: [f32; 2],
    pub panel: [f32; 2],
    pub exit: i32,
    pub symbol: Option<IggySymbol>,
    pub condition: Option<IggyCondition>,
    pub cycles_to_show: i32,
    pub point_player_back: bool,
    pub available_to_players: Vec<SlugcatName>,
}

impl ReliableIggyDirectionData {
    /// `FromString`: `handle~panel~exit~symbol~condition~cycles~back~players~...`, the
    /// symbol and condition as names or old numbers, the players as `|`-separated names
    /// or old digits. `None` where the game's parsing would throw.
    #[must_use]
    pub fn parse(data: &str, mods: ActiveMods) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        let symbol = match f.int(5) {
            // `ParseReliableIggySymbol`: an unknown number is `Shelter`.
            Some(n) => usize::try_from(n)
                .ok()
                .and_then(|i| IggySymbol::ALL.get(i))
                .copied()
                .or(Some(IggySymbol::Shelter)),
            None => IggySymbol::from_value(f.get(5)?),
        };
        let condition = match f.int(6) {
            Some(n) => usize::try_from(n)
                .ok()
                .and_then(|i| IggyCondition::ALL.get(i))
                .copied()
                .or(Some(IggyCondition::AnyTime)),
            None => IggyCondition::from_value(f.get(6)?),
        };
        let players = f.get(9)?;
        let available_to_players =
            if !players.is_empty() && players.bytes().all(|b| b.is_ascii_digit()) {
                // `ParsePlayerAvailability`: digit i set to '1' allows the i-th base name.
                let base = [
                    SlugcatName::White,
                    SlugcatName::Yellow,
                    SlugcatName::Red,
                    SlugcatName::Night,
                ];
                players
                    .bytes()
                    .zip(base)
                    .filter(|&(b, _)| b == b'1')
                    .map(|(_, n)| n)
                    .collect()
            } else {
                players
                    .split('|')
                    .filter_map(|n| SlugcatName::from_value(n, mods))
                    .collect()
            };
        Some(Self {
            handle: [f.float(0)?, f.float(1)?],
            panel: [f.float(2)?, f.float(3)?],
            exit: f.int(4)?,
            symbol,
            condition,
            cycles_to_show: f.int(7)?,
            point_player_back: f.get(8)? == "1",
            available_to_players,
        })
    }

    /// `Rad`: the handle's length.
    #[must_use]
    pub fn rad(&self) -> f32 {
        (self.handle[0] * self.handle[0] + self.handle[1] * self.handle[1]).sqrt()
    }
}

impl IggySymbol {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.value() == value)
    }
}

impl IggyCondition {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.value() == value)
    }
}

/// A placed object's data split on `~`.
pub(crate) struct DataFields<'a>(pub(crate) Vec<&'a str>);

impl<'a> DataFields<'a> {
    pub(crate) fn get(&self, i: usize) -> Option<&'a str> {
        self.0.get(i).copied()
    }

    pub(crate) fn float(&self, i: usize) -> Option<f32> {
        self.get(i)?.trim().parse().ok()
    }

    pub(crate) fn int(&self, i: usize) -> Option<i32> {
        self.get(i)?.trim().parse().ok()
    }
}

/// `PlacedObject.MultiplayerItemData`: what spawns and how likely.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiplayerItemData {
    /// `type`; `None` for a value no mod registers (it matches none of the game's
    /// branches, so a rock spawns).
    pub kind: Option<MultiplayerItemType>,
    pub panel: [f32; 2],
    pub chance: f32,
}

impl MultiplayerItemData {
    /// `type~panel.x~panel.y~chance~...`; `None` where the game's `float.Parse` would
    /// throw.
    #[must_use]
    pub fn parse(data: &str, mods: ActiveMods) -> Option<Self> {
        let fields: Vec<&str> = data.split('~').collect();
        let kind = MultiplayerItemType::from_value(fields.first()?, mods);
        let panel = [
            fields.get(1)?.trim().parse().ok()?,
            fields.get(2)?.trim().parse().ok()?,
        ];
        let chance = fields.get(3)?.trim().parse().ok()?;
        Some(Self {
            kind,
            panel,
            chance,
        })
    }
}

/// `PlacedObject.FilterData`: the handle (radius) and the slugcats it lets through.
#[derive(Debug, Clone, PartialEq)]
struct FilterData {
    handle: [f32; 2],
    available: Vec<SlugcatName>,
}

impl FilterData {
    /// `handle.x~handle.y~panel.x~panel.y~<excluded names | legacy 0/1 digits>`.
    fn parse(data: &str, mods: ActiveMods) -> Option<Self> {
        let fields: Vec<&str> = data.split('~').collect();
        let handle = [
            fields.first()?.trim().parse().ok()?,
            fields.get(1)?.trim().parse().ok()?,
        ];
        let list = fields.get(4).copied().unwrap_or("");
        let available = if !list.is_empty() && list.bytes().all(|b| b.is_ascii_digit()) {
            // `ParsePlayerAvailability`: digit i set to '1' allows player number i.
            list.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'1')
                .filter_map(|(i, _)| SlugcatName::from_player_number(i32::try_from(i).ok()?, mods))
                .collect()
        } else {
            let excluded: Vec<&str> = list.split('|').collect();
            SlugcatName::registered(mods)
                .into_iter()
                .filter(|n| !n.hidden_or_unplayable() && !excluded.contains(&n.value()))
                .collect()
        };
        Some(Self { handle, available })
    }

    /// `Active(timeline)`: the timeline of an allowed slugcat.
    fn active(&self, timeline: Timeline) -> bool {
        self.available.iter().any(|n| n.timeline() == timeline)
    }

    /// `ResizableObjectData.Rad`: the handle's length.
    fn rad(&self) -> f32 {
        (self.handle[0] * self.handle[0] + self.handle[1] * self.handle[1]).sqrt()
    }
}

/// A `", "`-separated settings value after `Custom.ValidateSpacedDelimiter`.
struct SpacedList;

impl SpacedList {
    fn split(value: &str) -> impl Iterator<Item = &str> {
        let value = value.strip_suffix(',').unwrap_or(value);
        value.split(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_parse_and_inherit() {
        let own = RoomEffect::parse_list("SSSwarmers-0.5-1, Fog-0.25-34-91, ");
        assert_eq!(own.len(), 2);
        assert_eq!(own[0].kind, "SSSwarmers");
        assert_eq!(own[0].amount, 0.5);
        let parent = RoomEffect::parse_list("Fog-1, Clouds-0.2");
        let all = RoomEffect::inherit(&own, &parent);
        assert_eq!(
            all.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
            ["SSSwarmers", "Fog", "Clouds"]
        );
    }

    #[test]
    fn filters_deactivate_by_timeline() {
        let mut objects = PlacedObject::parse_list(
            "Filter><100><100><50~0~0~0~White|Red, CoralStem><110><100><0, \
             CoralStem><300><100><0, Filter><300><100><10~0~0~0~Yellow",
        );
        assert_eq!(objects.len(), 4);
        PlacedObject::apply_filters(&mut objects, Timeline::White, ActiveMods::NONE);
        let active: Vec<bool> = objects.iter().map(|o| o.active).collect();
        assert_eq!(active, [true, false, true, true]);
        let mut objects = PlacedObject::parse_list(
            "Filter><100><100><50~0~0~0~White|Red, CoralStem><110><100><0, \
             CoralStem><300><100><0, Filter><300><100><10~0~0~0~Yellow",
        );
        PlacedObject::apply_filters(&mut objects, Timeline::Yellow, ActiveMods::NONE);
        let active: Vec<bool> = objects.iter().map(|o| o.active).collect();
        assert_eq!(active, [true, true, false, true]);
    }

    #[test]
    fn resizable_and_multiplayer_item_data() {
        // SU_A04's `NoSpearStickZone><918.743><558.3001><-138~111`.
        let r = ResizableData::parse("-138~111").unwrap();
        assert_eq!(r.rad(), (138.0f32 * 138.0 + 111.0 * 111.0).sqrt());
        let m = MultiplayerItemData::parse("Spear~-10~17.32~0.5", ActiveMods::NONE).unwrap();
        assert_eq!(m.kind, Some(MultiplayerItemType::Spear));
        assert_eq!((m.panel, m.chance), ([-10.0, 17.32], 0.5));
        let odd = MultiplayerItemData::parse("Lantern~0~0~1", ActiveMods::NONE).unwrap();
        assert_eq!(odd.kind, None);
        assert!(MultiplayerItemData::parse("Rock~x~0~1", ActiveMods::NONE).is_none());
    }

    /// SU_A13's `SuperJumpInstruction><475.6972><329.9665><-271~-102~-265~-126~-6~-32`.
    #[test]
    fn quad_data() {
        let q = QuadData::parse("-271~-102~-265~-126~-6~-32").unwrap();
        assert_eq!(
            q.handles,
            [[-271.0, -102.0], [-265.0, -126.0], [-6.0, -32.0]]
        );
        assert!(QuadData::parse("1~2~3").is_none());
    }

    /// Names or the old numbers (`ParseReliableIggySymbol` / `Condition`: unknown numbers
    /// are `Shelter` / `AnyTime`), players by name or digit.
    #[test]
    fn reliable_iggy_direction_data() {
        let d = ReliableIggyDirectionData::parse(
            "30~40~50~50~2~Food~AfterMoon~5~0~White|Yellow",
            ActiveMods::NONE,
        )
        .unwrap();
        assert_eq!(d.rad(), 50.0);
        assert_eq!(d.exit, 2);
        assert_eq!(d.symbol, Some(IggySymbol::Food));
        assert_eq!(d.condition, Some(IggyCondition::AfterMoon));
        assert_eq!((d.cycles_to_show, d.point_player_back), (5, false));
        assert_eq!(
            d.available_to_players,
            [SlugcatName::White, SlugcatName::Yellow]
        );
        let d =
            ReliableIggyDirectionData::parse("0~100~0~0~0~1~9~0~1~0110", ActiveMods::NONE).unwrap();
        assert_eq!(d.symbol, Some(IggySymbol::DynamicDirection));
        assert_eq!(d.condition, Some(IggyCondition::AnyTime));
        assert!(d.point_player_back);
        assert_eq!(
            d.available_to_players,
            [SlugcatName::Yellow, SlugcatName::Red]
        );
        let d =
            ReliableIggyDirectionData::parse("0~100~0~0~0~Odd~Odd~0~1~", ActiveMods::NONE).unwrap();
        assert_eq!((d.symbol, d.condition), (None, None));
        assert!(d.available_to_players.is_empty());
        assert!(
            ReliableIggyDirectionData::parse("0~100~0~0~0~1~1~0~1", ActiveMods::NONE).is_none()
        );
    }

    #[test]
    fn legacy_digit_availability() {
        let f = FilterData::parse("3~4~0~0~010", ActiveMods::NONE).unwrap();
        assert_eq!(f.available, [SlugcatName::Yellow]);
        assert_eq!(f.rad(), 5.0);
    }
}
