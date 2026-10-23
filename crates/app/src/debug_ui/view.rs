//! What the debug UI's tabs read of the running game, gathered once per UI frame.
//!
//! Mirrors: - (port-only: dev UI)

use rw_assets::{AbstractObjectType, CreatureType};

use crate::game::Game;
use crate::mcp::reply::RainState;

/// The running game as the tabs see it; `None` outside the game process.
#[derive(Debug, Clone, Default)]
pub struct DebugView {
    pub game: Option<GameView>,
}

#[derive(Debug, Clone)]
pub struct GameView {
    /// `World.name` in upper case, the region's acronym (e.g. `SU`).
    pub region: String,
    pub camera_room: String,
    /// The player's tile when it is in the camera's room.
    pub player_tile: Option<(i32, i32)>,
    /// Every room of the world but the off-screen den, in world order.
    pub rooms: Vec<String>,
    pub creatures: Vec<CreatureRow>,
    pub creature_kinds: Vec<CreatureType>,
    pub rain: RainState,
}

/// One abstract creature, as `get_state.creatures` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureRow {
    pub kind: CreatureType,
    pub id: [i32; 2],
    pub room: String,
    pub node: i32,
    pub tile: (i32, i32),
    pub in_den: bool,
    pub realized: bool,
    pub alive: bool,
}

/// The object kinds a dev spawn makes (only rocks and spears are realized).
pub const SPAWN_OBJECT_KINDS: &[AbstractObjectType] = &rw_sim::RainWorldGame::DEV_SPAWN_OBJECTS;

impl DebugView {
    #[must_use]
    pub fn of(frontend: &crate::frontend::Frontend) -> Self {
        Self {
            game: frontend.game().map(|g| GameView::of(g.scene.game())),
        }
    }
}

impl GameView {
    #[must_use]
    pub fn of(game: &Game) -> Self {
        let sim = game.sim();
        let world = sim.world();
        let player_tile = sim
            .player()
            .filter(|p| !p.in_shortcut() && sim.player_in_camera_room())
            .map(|p| p.chunks()[0].pos.tile());
        Self {
            region: world.name().to_uppercase(),
            camera_room: game.room_name(sim.camera_room()),
            player_tile,
            rooms: world
                .data()
                .rooms
                .iter()
                .filter(|r| !r.offscreen_den)
                .map(|r| r.name.clone())
                .collect(),
            creatures: CreatureRow::rows(game),
            creature_kinds: sim.dev_spawnable_creatures(),
            rain: RainState::of(sim),
        }
    }
}

impl CreatureRow {
    /// The world's creatures but the player, in creation order (`get_state`'s list).
    #[must_use]
    pub fn rows(game: &Game) -> Vec<Self> {
        let sim = game.sim();
        sim.world()
            .creatures()
            .filter(|(id, c)| *id != sim.player_creature() && !c.slated_for_deletion)
            .map(|(_, c)| Self {
                kind: c.kind(),
                id: [c.id.spawner, c.id.number],
                room: game.room_name(c.pos.room),
                node: c.pos.node,
                tile: c.pos.tile,
                in_den: c.in_den,
                realized: c.is_realized(),
                alive: c.state.alive,
            })
            .collect()
    }

    /// Whether `query` (case-insensitive; empty matches all) is in the kind, the room
    /// or the ID written `spawner.number`.
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.kind.value().to_lowercase().contains(&query)
            || self.room.to_lowercase().contains(&query)
            || self.id_text().contains(&query)
    }

    #[must_use]
    pub fn id_text(&self) -> String {
        format!("{}.{}", self.id[0], self.id[1])
    }
}

/// A case-insensitive substring filter over room names.
pub struct RoomFilter<'a>(pub &'a str);

impl RoomFilter<'_> {
    #[must_use]
    pub fn apply<'r>(&self, rooms: &'r [String]) -> Vec<&'r str> {
        let query = self.0.trim().to_lowercase();
        rooms
            .iter()
            .filter(|r| r.to_lowercase().contains(&query))
            .map(String::as_str)
            .collect()
    }
}
