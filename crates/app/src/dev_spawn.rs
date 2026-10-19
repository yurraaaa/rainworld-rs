//! Dev spawns of creatures and objects, shared by the MCP tools (`spawn_creature`,
//! `spawn_object`) and the debug UI.

use rw_assets::{AbstractObjectType, CreatureType};
use serde::Serialize;

use crate::game::Game;

/// What a dev spawn made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpawnReply {
    /// The new entity's EntityID [spawner, number].
    pub id: [i32; 2],
    pub kind: String,
    pub room: String,
    pub tile: [i32; 2],
    pub realized: bool,
    /// Why it was not realized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Game {
    /// The name of global room `index`, or `#index`.
    #[must_use]
    pub fn room_name(&self, index: i32) -> String {
        self.sim()
            .world()
            .abstract_room(index)
            .map_or_else(|| format!("#{index}"), |r| r.name.clone())
    }

    /// `Err` for a tile outside a realized room.
    fn check_spawn_tile(&self, room: i32, (x, y): (i32, i32)) -> Result<(), String> {
        match self.sim().world().realized(room) {
            Some(r) if r.room.get(x, y).is_none() => {
                Err(format!("tile [{x}, {y}] is outside {}", r.room.name()))
            }
            _ => Ok(()),
        }
    }

    /// Adds a new creature at a tile ([`Game::dev_spawn_creature`]), realized there when
    /// its room is ready and the port has its body.
    ///
    /// # Errors
    /// For a type that is not spawnable, an unknown room, or a tile outside a realized
    /// room.
    pub fn spawn_creature(
        &mut self,
        kind: CreatureType,
        room: i32,
        tile: (i32, i32),
        seed_id: Option<i32>,
    ) -> Result<SpawnReply, String> {
        self.check_spawn_tile(room, tile)?;
        let id = self.dev_spawn_creature(kind, room, tile, seed_id)?;
        let world = self.sim().world();
        let c = world.creature(id);
        let realized = c.is_realized();
        let note = match world.realized(room) {
            _ if realized => None,
            None => Some("its room is not loaded: added abstract".to_owned()),
            Some(r) if !r.ready_for_ai() => {
                Some("its room is still loading: realized when it is ready".to_owned())
            }
            Some(_) => {
                Some("the port has no realized body for this type: added abstract".to_owned())
            }
        };
        Ok(SpawnReply {
            id: [c.id.spawner, c.id.number],
            kind: kind.value().to_owned(),
            room: self.room_name(room),
            tile: [c.pos.tile.0, c.pos.tile.1],
            realized,
            note,
        })
    }

    /// Adds a new rock or spear at a tile ([`Game::dev_spawn_object`]), realized there
    /// when its room's shortcuts are ready.
    ///
    /// # Errors
    /// For a kind the port does not realize, an unknown room, or a tile outside a
    /// realized room.
    pub fn spawn_object(
        &mut self,
        kind: AbstractObjectType,
        room: i32,
        tile: (i32, i32),
        explosive: bool,
    ) -> Result<SpawnReply, String> {
        self.check_spawn_tile(room, tile)?;
        let id = self.dev_spawn_object(kind, room, tile, explosive)?;
        let world = self.sim().world();
        let o = world.object(id);
        let realized = o.realized.is_some();
        let note = match world.realized(room) {
            _ if realized => None,
            None => Some("its room is not loaded: added abstract".to_owned()),
            Some(_) => Some("its room is still loading: realized when it is ready".to_owned()),
        };
        Ok(SpawnReply {
            id: [o.id.spawner, o.id.number],
            kind: kind.value().to_owned(),
            room: self.room_name(room),
            tile: [o.pos.tile.0, o.pos.tile.1],
            realized,
            note,
        })
    }
}
