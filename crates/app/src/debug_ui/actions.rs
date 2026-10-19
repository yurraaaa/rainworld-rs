//! What the debug UI asks of the game, applied between frames (between ticks) through
//! the code paths the MCP tools use.

use rw_assets::{AbstractObjectType, CreatureType};
use rw_menu::{ProcessId, ProcessRequests};
use rw_sim::Vec2;

use crate::frontend::Frontend;
use crate::game::GameStart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugAction {
    /// `load_room`'s start: the room's region world, the player in that room.
    LoadRoom(String),
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
            Self::LoadRoom(name) => {
                let slugcat = game.sim().campaign().name;
                let assets = &frontend.ctx.menu.assets;
                if assets
                    .find_room_file(&name.to_lowercase(), ".txt")
                    .is_none()
                {
                    return Err(format!("room {name} not found"));
                }
                frontend.ctx.game.start = GameStart::Room {
                    name: name.clone(),
                    tile: None,
                    slugcat,
                };
                frontend.ctx.menu.in_game_slugcat = None;
                frontend
                    .manager
                    .request_switch(ProcessId::Game, ProcessRequests::FADE_OUT_SECONDS);
                Ok(format!("loading {name} (a dev start)"))
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
