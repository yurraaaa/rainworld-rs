//! The debug UI's tools, one tab each.

use std::borrow::Cow;

use imgui::{TableFlags, Ui};
use rw_assets::{AbstractObjectType, CreatureType};

use super::view::{CreatureRow, GameView, RoomFilter, SPAWN_OBJECT_KINDS};
use super::{DebugAction, DebugTab, TabCx};

/// The tabs, in tab-bar order.
pub fn all() -> Vec<Box<dyn DebugTab>> {
    vec![
        Box::new(RegionTab::default()),
        Box::new(TeleportTab::default()),
        Box::new(CreaturesTab::default()),
        Box::new(SpawnCreatureTab::default()),
        Box::new(SpawnObjectTab::default()),
    ]
}

/// Region switching: a stub until gates and region changes are ported.
#[derive(Default)]
struct RegionTab {
    selected: usize,
    pressed: bool,
}

impl DebugTab for RegionTab {
    fn title(&self) -> &'static str {
        "Region"
    }

    fn draw(&mut self, ui: &Ui, cx: &mut TabCx<'_>) {
        if let Some(game) = &cx.view.game {
            ui.text(format!("Current region: {}", game.region));
        }
        let regions: Vec<String> = cx
            .catalog
            .map(|c| c.regions().iter().map(|r| r.to_uppercase()).collect())
            .unwrap_or_default();
        if regions.is_empty() {
            ui.text_disabled("No region directories found.");
            return;
        }
        ui.combo_simple_string("Region", &mut self.selected, &regions);
        if ui.button("Switch region") {
            self.pressed = true;
        }
        if self.pressed {
            ui.text_wrapped(
                "Not ported yet: region changes and gates are not in the port \
                 (docs/OPEN-ITEMS.md). Use Teleport within the current region.",
            );
        }
    }
}

/// The current world's rooms; the player travels to one through the shortcut lobby
/// (`teleport_player`), the world kept.
#[derive(Default)]
struct TeleportTab {
    filter: String,
    selected: Option<String>,
}

impl DebugTab for TeleportTab {
    fn title(&self) -> &'static str {
        "Teleport"
    }

    fn draw(&mut self, ui: &Ui, cx: &mut TabCx<'_>) {
        let Some(game) = &cx.view.game else {
            ui.text_disabled("No game is running.");
            return;
        };
        ui.text(format!(
            "Region {}, camera in {}",
            game.region, game.camera_room
        ));
        ui.text_disabled("The player comes out of the room's first exit (MCP teleport_player).");
        ui.input_text("Filter", &mut self.filter).build();
        let rooms = RoomFilter(&self.filter).apply(&game.rooms);
        let mut load = None;
        ui.child_window("rooms").size([0.0, -30.0]).build(|| {
            for &room in &rooms {
                let label = if room == game.camera_room {
                    format!("{room}  (camera)")
                } else {
                    room.to_owned()
                };
                let selected = self.selected.as_deref() == Some(room);
                if ui.selectable_config(&label).selected(selected).build() {
                    self.selected = Some(room.to_owned());
                }
                if ui.is_item_hovered() && ui.is_mouse_double_clicked(imgui::MouseButton::Left) {
                    load = Some(room.to_owned());
                }
            }
        });
        let chosen = self.selected.clone();
        ui.disabled(chosen.is_none(), || {
            if ui.button("Go to room") {
                load = chosen.clone();
            }
        });
        ui.same_line();
        ui.text(format!("{} rooms", rooms.len()));
        if let Some(room) = load {
            cx.actions.push(DebugAction::Teleport(room));
        }
    }
}

/// Every abstract creature, as `get_state.creatures` lists them.
#[derive(Default)]
struct CreaturesTab {
    filter: String,
    selected: Option<[i32; 2]>,
}

impl CreaturesTab {
    const COLUMNS: [&'static str; 8] = [
        "Kind", "ID", "Room", "Node", "Tile", "In den", "Realized", "Alive",
    ];

    fn row(&mut self, ui: &Ui, c: &CreatureRow) {
        ui.table_next_row();
        ui.table_next_column();
        let selected = self.selected == Some(c.id);
        let label = format!("{}##{}", c.kind.value(), c.id_text());
        if ui
            .selectable_config(&label)
            .selected(selected)
            .span_all_columns(true)
            .build()
        {
            self.selected = Some(c.id);
        }
        let cells = [
            c.id_text(),
            c.room.clone(),
            c.node.to_string(),
            format!("{}, {}", c.tile.0, c.tile.1),
            YesNo(c.in_den).to_string(),
            YesNo(c.realized).to_string(),
            YesNo(c.alive).to_string(),
        ];
        for cell in cells {
            ui.table_next_column();
            ui.text(cell);
        }
    }
}

/// `yes` or blank, for table cells.
struct YesNo(bool);

impl std::fmt::Display for YesNo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0 { "yes" } else { "" })
    }
}

impl DebugTab for CreaturesTab {
    fn title(&self) -> &'static str {
        "Creatures"
    }

    fn draw(&mut self, ui: &Ui, cx: &mut TabCx<'_>) {
        let Some(game) = &cx.view.game else {
            ui.text_disabled("No game is running.");
            return;
        };
        ui.input_text("Filter", &mut self.filter).build();
        let rows: Vec<&CreatureRow> = game
            .creatures
            .iter()
            .filter(|c| c.matches(&self.filter))
            .collect();
        ui.text(format!(
            "{} of {} creatures",
            rows.len(),
            game.creatures.len()
        ));
        let selected = rows.iter().find(|c| Some(c.id) == self.selected).copied();
        let here = selected.filter(|c| c.realized && c.room == game.camera_room);
        ui.disabled(here.is_none(), || {
            if ui.button("Move player to it")
                && let Some(c) = here
            {
                cx.actions.push(DebugAction::MovePlayer(c.tile));
            }
        });
        let elsewhere =
            selected.filter(|c| c.room != game.camera_room && game.rooms.contains(&c.room));
        ui.same_line();
        ui.disabled(elsewhere.is_none(), || {
            if ui.button("Go to its room")
                && let Some(c) = elsewhere
            {
                cx.actions.push(DebugAction::Teleport(c.room.clone()));
            }
        });
        let flags = TableFlags::ROW_BG
            | TableFlags::BORDERS_INNER_V
            | TableFlags::SCROLL_Y
            | TableFlags::RESIZABLE;
        if let Some(_table) = ui.begin_table_with_flags("creatures", Self::COLUMNS.len(), flags) {
            for name in Self::COLUMNS {
                ui.table_setup_column(name);
            }
            ui.table_headers_row();
            for c in rows {
                self.row(ui, c);
            }
        }
    }
}

/// Where a spawn goes: a room of the world (blank for the camera's) and a tile.
#[derive(Default)]
pub struct SpawnPlace {
    pub room: String,
    pub tile: [i32; 2],
}

impl SpawnPlace {
    /// The room's name as the world spells it, and the tile.
    ///
    /// # Errors
    /// For a room the world does not have.
    pub fn resolve(&self, game: &GameView) -> Result<(String, (i32, i32)), String> {
        let want = self.room.trim();
        let room = if want.is_empty() {
            game.camera_room.clone()
        } else {
            game.rooms
                .iter()
                .find(|r| r.eq_ignore_ascii_case(want))
                .cloned()
                .ok_or_else(|| format!("no room {want} in this world"))?
        };
        Ok((room, (self.tile[0], self.tile[1])))
    }

    fn draw(&mut self, ui: &Ui, game: &GameView) {
        ui.input_text("Room (blank: camera's)", &mut self.room)
            .build();
        ui.input_int2("Tile", &mut self.tile).build();
        ui.disabled(game.player_tile.is_none(), || {
            if ui.button("Player's tile")
                && let Some((x, y)) = game.player_tile
            {
                self.room.clear();
                self.tile = [x, y];
            }
        });
    }
}

#[derive(Default)]
struct SpawnCreatureTab {
    kind: usize,
    place: SpawnPlace,
    error: Option<String>,
}

impl DebugTab for SpawnCreatureTab {
    fn title(&self) -> &'static str {
        "Spawn creature"
    }

    fn draw(&mut self, ui: &Ui, cx: &mut TabCx<'_>) {
        let Some(game) = &cx.view.game else {
            ui.text_disabled("No game is running.");
            return;
        };
        let kinds: &[CreatureType] = &game.creature_kinds;
        ui.combo("Kind", &mut self.kind, kinds, |k| Cow::Borrowed(k.value()));
        self.place.draw(ui, game);
        ui.text_disabled("As MCP spawn_creature: realized where the port has the body.");
        if ui.button("Spawn") {
            match (kinds.get(self.kind), self.place.resolve(game)) {
                (Some(&kind), Ok((room, tile))) => {
                    self.error = None;
                    cx.actions
                        .push(DebugAction::SpawnCreature { kind, room, tile });
                }
                (_, Err(e)) => self.error = Some(e),
                (None, _) => self.error = Some("no kind chosen".to_owned()),
            }
        }
        if let Some(e) = &self.error {
            ui.text_colored([1.0, 0.45, 0.4, 1.0], e);
        }
    }
}

#[derive(Default)]
struct SpawnObjectTab {
    kind: usize,
    explosive: bool,
    place: SpawnPlace,
    error: Option<String>,
}

impl DebugTab for SpawnObjectTab {
    fn title(&self) -> &'static str {
        "Spawn object"
    }

    fn draw(&mut self, ui: &Ui, cx: &mut TabCx<'_>) {
        let Some(game) = &cx.view.game else {
            ui.text_disabled("No game is running.");
            return;
        };
        ui.combo("Kind", &mut self.kind, SPAWN_OBJECT_KINDS, |k| {
            Cow::Borrowed(k.value())
        });
        let kind = SPAWN_OBJECT_KINDS
            .get(self.kind)
            .copied()
            .unwrap_or(AbstractObjectType::Rock);
        if kind == AbstractObjectType::Spear {
            ui.checkbox("Explosive", &mut self.explosive);
        }
        self.place.draw(ui, game);
        ui.text_disabled("As MCP spawn_object: only rocks and spears get a body.");
        if ui.button("Spawn") {
            match self.place.resolve(game) {
                Ok((room, tile)) => {
                    self.error = None;
                    cx.actions.push(DebugAction::SpawnObject {
                        kind,
                        room,
                        tile,
                        explosive: self.explosive && kind == AbstractObjectType::Spear,
                    });
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(e) = &self.error {
            ui.text_colored([1.0, 0.45, 0.4, 1.0], e);
        }
    }
}
