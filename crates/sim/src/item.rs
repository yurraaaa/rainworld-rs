//! Physical objects other than creatures, abstract (`AbstractPhysicalObject`,
//! `AbstractSpear`): what they are, where, and their save strings. The world keeps them
//! in an arena ([`crate::world::World::object`]) and in its rooms' entity lists, beside
//! the creatures ([`Entity`]). Rocks and spears are typed; a saved object of any other
//! kind stays a string (docs/DIVERGENCES.md D16). A realized rock or spear is a
//! [`Weapon`]. Behaviour: `rainworld-re/notes/items.md`.

mod spear_stick;
mod weapon;

pub use spear_stick::{SpearStick, SpearStickKind};
pub use weapon::{
    ChunkDynamicSoundLoop, ModeChange, SpearParts, Throw, WEAPON_PARAMS, Weapon, WeaponKind,
    WeaponMode, WeaponTick,
};

use rw_assets::{AbstractObjectType, ActiveMods};

use crate::creature::{CreatureId, EntityId, Grasp, GripStick, WorldCoordinate};

/// Index of an object in the world's object arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub usize);

/// Any physical object: what the game holds as an `AbstractPhysicalObject` (a room's
/// `entities`, later grasps and vessels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entity {
    Creature(CreatureId),
    Object(ObjectId),
}

impl Entity {
    #[must_use]
    pub fn creature(self) -> Option<CreatureId> {
        match self {
            Self::Creature(id) => Some(id),
            Self::Object(_) => None,
        }
    }

    #[must_use]
    pub fn object(self) -> Option<ObjectId> {
        match self {
            Self::Object(id) => Some(id),
            Self::Creature(_) => None,
        }
    }
}

/// `AbstractSpear`'s own fields (the DLC ones, `hue`, `electric`, `needle`, `poison`,
/// are not read with the DLCs off and stay in the unrecognized attributes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AbstractSpear {
    /// Cycles left stuck in a wall: > 0 horizontal, < 0 vertical, 0 free.
    pub stuck_in_wall_cycles: i32,
    /// An `ExplosiveSpear`.
    pub explosive: bool,
}

impl AbstractSpear {
    #[must_use]
    pub fn stuck_in_wall(self) -> bool {
        self.stuck_in_wall_cycles != 0
    }

    /// `StuckInWallTick`: the cycles count `ticks` toward 0.
    pub fn stuck_in_wall_tick(&mut self, ticks: i32) {
        if self.stuck_in_wall_cycles > 0 {
            self.stuck_in_wall_cycles = (self.stuck_in_wall_cycles - ticks).max(0);
        } else if self.stuck_in_wall_cycles < 0 {
            self.stuck_in_wall_cycles = (self.stuck_in_wall_cycles + ticks).min(0);
        }
    }
}

/// The subclass of `AbstractPhysicalObject` and its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectData {
    Plain,
    Spear(AbstractSpear),
}

/// `AbstractPhysicalObject`.
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractObject {
    pub kind: AbstractObjectType,
    pub id: EntityId,
    pub pos: WorldCoordinate,
    pub data: ObjectData,
    /// `rippleLayer` (the Watcher's; 0 otherwise).
    pub ripple_layer: i32,
    pub in_den: bool,
    pub time_spent_here: i32,
    /// `evenUpdate`.
    pub even_update: bool,
    /// Save-string fields past the known ones, written back after them.
    pub unrecognized_attributes: Vec<String>,
    /// `realizedObject`.
    pub realized: Option<Box<Weapon>>,
    /// The realized object's `grabbedBy`.
    pub grabbed_by: Vec<Grasp>,
    /// `stuckObjects`: the grip sticks of the creatures holding it.
    pub sticks: Vec<GripStick>,
    /// `placedObjectOrigin` (`"ROOM:index"` of the placed object that made it; read by
    /// Remix's key-item tracking only).
    pub placed_object_origin: Option<String>,
}

impl AbstractObject {
    /// `new AbstractPhysicalObject(world, Rock, null, pos, id)`.
    #[must_use]
    pub fn new_rock(id: EntityId, pos: WorldCoordinate) -> Self {
        Self::new(AbstractObjectType::Rock, ObjectData::Plain, id, pos)
    }

    /// `new AbstractSpear(world, null, pos, id, explosive)`.
    #[must_use]
    pub fn new_spear(id: EntityId, pos: WorldCoordinate, explosive: bool) -> Self {
        let spear = AbstractSpear {
            stuck_in_wall_cycles: 0,
            explosive,
        };
        Self::new(AbstractObjectType::Spear, ObjectData::Spear(spear), id, pos)
    }

    /// `new AbstractPhysicalObject(world, kind, null, pos, id)` of a kind the port does
    /// not type (it is never realized).
    #[must_use]
    pub fn new_plain(kind: AbstractObjectType, id: EntityId, pos: WorldCoordinate) -> Self {
        Self::new(kind, ObjectData::Plain, id, pos)
    }

    fn new(kind: AbstractObjectType, data: ObjectData, id: EntityId, pos: WorldCoordinate) -> Self {
        Self {
            kind,
            id,
            pos,
            data,
            ripple_layer: 0,
            in_den: false,
            time_spent_here: 0,
            even_update: false,
            unrecognized_attributes: Vec::new(),
            realized: None,
            grabbed_by: Vec::new(),
            sticks: Vec::new(),
            placed_object_origin: None,
        }
    }

    #[must_use]
    pub fn spear(&self) -> Option<&AbstractSpear> {
        match &self.data {
            ObjectData::Spear(s) => Some(s),
            ObjectData::Plain => None,
        }
    }

    pub fn spear_mut(&mut self) -> Option<&mut AbstractSpear> {
        match &mut self.data {
            ObjectData::Spear(s) => Some(s),
            ObjectData::Plain => None,
        }
    }

    /// An `AbstractSpear` with `stuckInWall`.
    #[must_use]
    pub fn stuck_in_wall(&self) -> bool {
        self.spear().is_some_and(|s| s.stuck_in_wall())
    }

    /// `ToString()`, given `pos.SaveToString()`.
    #[must_use]
    pub fn save_string(&self, pos: &str) -> String {
        let mut text = format!(
            "ID.{}.{}<oB>{}<oA>{}<oA>{pos}",
            self.id.spawner,
            self.id.number,
            self.ripple_layer,
            self.kind.value()
        );
        if let ObjectData::Spear(s) = self.data {
            text.push_str(&format!(
                "<oA>{}<oA>{}",
                s.stuck_in_wall_cycles,
                u8::from(s.explosive)
            ));
        }
        for a in &self.unrecognized_attributes {
            text.push_str("<oA>");
            text.push_str(a);
        }
        text
    }
}

/// A saved object string (`SaveState.AbstractPhysicalObjectFromString`) split into its
/// parts, for a kind the port types (Rock, Spear) with the DLCs off.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SavedObject<'a> {
    pub id: EntityId,
    pub ripple_layer: i32,
    pub kind: AbstractObjectType,
    /// `WorldCoordinate.FromString`'s room name (the `INV.` form's unknown name).
    pub room: &'a str,
    pub tile: (i32, i32),
    pub node: i32,
    pub data: ObjectData,
    pub unrecognized_attributes: Vec<String>,
}

impl<'a> SavedObject<'a> {
    /// `None` where the game's parse throws or its type is not registered, and for
    /// kinds the port keeps as text.
    pub fn parse(s: &'a str, mods: ActiveMods) -> Option<Self> {
        let parts: Vec<&str> = s.split("<oA>").collect();
        let (id, ripple_layer) = match parts[0].split_once("<oB>") {
            Some((id, layer)) => (id, layer.trim().parse().ok()?),
            None => (parts[0], 0),
        };
        let id = Self::entity_id(id)?;
        let kind = AbstractObjectType::from_value(parts.get(1)?, mods)?;
        let pos: Vec<&str> = parts.get(2)?.split('.').collect();
        let skip = usize::from(pos.len() == 5 && pos[0] == "INV");
        let room = *pos.get(skip)?;
        let x = Self::int(&pos, 1 + skip)?;
        let y = Self::int(&pos, 2 + skip)?;
        let node = Self::int(&pos, 3 + skip)?;
        let (data, rest) = match kind {
            AbstractObjectType::Rock => (ObjectData::Plain, 3),
            AbstractObjectType::Spear => {
                let spear = AbstractSpear {
                    explosive: *parts.get(4)? == "1",
                    stuck_in_wall_cycles: parts.get(3)?.trim().parse().ok()?,
                };
                (ObjectData::Spear(spear), 5)
            }
            _ => return None,
        };
        Some(Self {
            id,
            ripple_layer,
            kind,
            room,
            tile: (x, y),
            node,
            data,
            unrecognized_attributes: parts
                .iter()
                .skip(rest)
                .filter(|a| !a.trim().is_empty())
                .map(|a| (*a).to_owned())
                .collect(),
        })
    }

    /// `int.Parse` of part `i`.
    fn int(parts: &[&str], i: usize) -> Option<i32> {
        parts.get(i)?.trim().parse().ok()
    }

    /// `EntityID.FromString`.
    fn entity_id(s: &str) -> Option<EntityId> {
        let mut it = s.split('.').skip(1);
        let spawner = it.next()?.trim().parse().ok()?;
        let number = it.next()?.trim().parse().ok()?;
        Some(EntityId::new(spawner, number))
    }

    /// The object, at `room`'s index.
    pub fn into_object(self, room: i32) -> AbstractObject {
        let mut o = AbstractObject::new(
            self.kind,
            self.data,
            self.id,
            WorldCoordinate::new(room, self.tile.0, self.tile.1, self.node),
        );
        o.ripple_layer = self.ripple_layer;
        o.unrecognized_attributes = self.unrecognized_attributes;
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuck_in_wall_tick_counts_toward_zero() {
        let mut s = AbstractSpear {
            stuck_in_wall_cycles: 5,
            explosive: false,
        };
        s.stuck_in_wall_tick(2);
        assert_eq!(s.stuck_in_wall_cycles, 3);
        s.stuck_in_wall_tick(7);
        assert_eq!(s.stuck_in_wall_cycles, 0);
        assert!(!s.stuck_in_wall());
        let mut v = AbstractSpear {
            stuck_in_wall_cycles: -4,
            explosive: false,
        };
        v.stuck_in_wall_tick(1);
        assert_eq!(v.stuck_in_wall_cycles, -3);
        v.stuck_in_wall_tick(3);
        assert_eq!(v.stuck_in_wall_cycles, 0);
    }

    #[test]
    fn spear_string_round_trips_with_extra_fields() {
        // A More Slugcats spear (hue, electric, charge, needle) read with the DLCs off:
        // the four DLC fields are unrecognized attributes, written back after ours.
        let s = "ID.-1.1234<oB>0<oA>Spear<oA>SU_S01.12.5.-1<oA>-3<oA>0<oA>0<oA>0<oA>0<oA>0";
        let saved = SavedObject::parse(s, ActiveMods::NONE).expect("parses");
        assert_eq!(saved.room, "SU_S01");
        assert_eq!((saved.tile, saved.node), ((12, 5), -1));
        assert_eq!(saved.unrecognized_attributes.len(), 4);
        let o = saved.into_object(7);
        assert_eq!(o.spear().map(|s| s.stuck_in_wall_cycles), Some(-3));
        assert!(o.stuck_in_wall());
        assert_eq!(o.save_string("SU_S01.12.5.-1"), s);
    }

    #[test]
    fn rock_without_ripple_layer_gets_one() {
        let saved =
            SavedObject::parse("ID.5.77<oA>Rock<oA>SU_A13.3.4.2", ActiveMods::NONE).expect("rock");
        assert_eq!(saved.id, EntityId::new(5, 77));
        let o = saved.into_object(1);
        assert_eq!(
            o.save_string("SU_A13.3.4.2"),
            "ID.5.77<oB>0<oA>Rock<oA>SU_A13.3.4.2"
        );
    }

    #[test]
    fn other_kinds_and_bad_strings_stay_text() {
        let none = ActiveMods::NONE;
        assert!(
            SavedObject::parse(
                "ID.-1.5<oB>0<oA>DangleFruit<oA>SU_S01.1.1.0<oA>3<oA>2",
                none
            )
            .is_none()
        );
        assert!(SavedObject::parse("ID.-1.5<oB>0<oA>LillyPuck<oA>SU_S01.1.1.0", none).is_none());
        assert!(SavedObject::parse("ID.-1.5<oB>0<oA>Spear<oA>SU_S01.1.1.0<oA>2", none).is_none());
        assert!(SavedObject::parse("ID.-1.x<oB>0<oA>Rock<oA>SU_S01.1.1.0", none).is_none());
        let inv =
            SavedObject::parse("ID.-1.5<oB>0<oA>Rock<oA>INV.XX_A01.1.1.0", none).expect("inv");
        assert_eq!(inv.room, "XX_A01");
    }
}
