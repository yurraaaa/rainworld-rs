//! Room effects and placed objects of a settings file, by type name: what decides a
//! room's entry delay (`Room.Loaded`'s `waitToEnterAfterFullyLoaded`) and, later, what it
//! places. `RoomSettings.LoadEffects`, `LoadPlacedObjects`, `PlacedObject.FromString`,
//! `PlacedObject.FilterData`.

use crate::{ActiveMods, SlugcatName, Timeline};

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
    fn legacy_digit_availability() {
        let f = FilterData::parse("3~4~0~0~010", ActiveMods::NONE).unwrap();
        assert_eq!(f.available, [SlugcatName::Yellow]);
        assert_eq!(f.rad(), 5.0);
    }
}
