//! What the debug UI asks of the game, applied between frames (between ticks) through
//! the code paths the MCP tools use.

use rw_assets::{AbstractObjectType, CreatureType};
use rw_sim::Vec2;

use crate::frontend::Frontend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugAction {
    /// `teleport_player` to a room of the world, out of its first exit.
    Teleport(String),
    /// `set_player` onto a tile of the camera's room.
    MovePlayer((i32, i32)),
    SpawnCreature {
        kind: CreatureType,
        room: String,
        tile: (i32, i32),
    },
    SpawnObject {
        kind: AbstractObjectType,
        room: String,
        tile: (i32, i32),
        explosive: bool,
    },
}

impl DebugAction {
    /// Applies the action to the running game; the status line's text, or why not.
    ///
    /// # Errors
    /// Without a running game, or when the game refuses (unknown room, bad tile).
    pub fn apply(self, frontend: &mut Frontend) -> Result<String, String> {
        if frontend.manager.upcoming().is_some() {
            return Err("a process switch is under way".to_owned());
        }
        let game = frontend
            .game_mut()
            .ok_or("no game is running")?
            .scene
            .game_mut();
        match self {
            Self::Teleport(room) => {
                let index = Self::room_index(game, &room)?;
                let r = game.teleport_player(index, None)?;
                Ok(format!("teleporting to {} (exit node {})", r.room, r.node))
            }
            Self::MovePlayer((x, y)) => {
                if game.room().get(x, y).is_none() {
                    return Err(format!("tile [{x}, {y}] is outside the camera's room"));
                }
                let pos = Vec2::from(rw_assets::tile_centre(x, y));
                game.set_player(pos, Vec2::ZERO);
                Ok(format!("player moved to [{x}, {y}]"))
            }
            Self::SpawnCreature { kind, room, tile } => {
                let index = Self::room_index(game, &room)?;
                game.spawn_creature(kind, index, tile, None)
                    .map(|r| Self::spawned(&r))
            }
            Self::SpawnObject {
                kind,
                room,
                tile,
                explosive,
            } => {
                let index = Self::room_index(game, &room)?;
                game.spawn_object(kind, index, tile, explosive)
                    .map(|r| Self::spawned(&r))
            }
        }
    }

    fn room_index(game: &crate::game::Game, room: &str) -> Result<i32, String> {
        game.sim()
            .world()
            .room_index(room)
            .ok_or_else(|| format!("no room {room} in this world"))
    }

    fn spawned(r: &crate::dev_spawn::SpawnReply) -> String {
        let realized = if r.realized { "realized" } else { "abstract" };
        let note = r
            .note
            .as_deref()
            .map_or(String::new(), |n| format!(": {n}"));
        format!(
            "{} {}.{} in {} [{}, {}], {realized}{note}",
            r.kind, r.id[0], r.id[1], r.room, r.tile[0], r.tile[1]
        )
    }
}
