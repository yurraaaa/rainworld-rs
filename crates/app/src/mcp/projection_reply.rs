//! What `get_state` reports of the realized rooms' superstructure projections: the
//! `SuperStructureProjector`s, the `ProjectedScanLines` and the `StarMatrix`es.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{PartKind, ProjectedScanLines, SingleGlyph, StarMatrix, SuperStructureProjector};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One `Cursor`.
#[derive(Debug, Clone, Serialize)]
pub struct CursorState {
    pub g_pos: [i32; 2],
    pub pos: [f32; 2],
    pub size: i32,
}

/// One scan line.
#[derive(Debug, Clone, Serialize)]
pub struct ScanLineState {
    /// True for a vertical line (it moves along x).
    pub vertical: bool,
    pub pos: f32,
    pub speed: f32,
}

/// One room's `SuperStructureProjector`.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectorState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub amount: f32,
    pub visible: bool,
    /// `gridPos`, `gridSpeed`, `intOffset` and the `_gridOffset` it sets.
    pub grid_pos: [f32; 2],
    pub grid_speed: [f32; 2],
    pub int_offset: [i32; 2],
    pub grid_offset: [f32; 2],
    pub last_cam_pos: usize,
    /// The ideal counts and the lists' lengths.
    pub ideal_glyphs: usize,
    pub ideal_scan_lines: usize,
    pub ideal_cursors: usize,
    pub glyphs: usize,
    /// `glyphsList`'s single glyphs and matrices.
    pub single_glyphs: usize,
    pub glyph_matrices: usize,
    /// Single glyphs waiting to be reused.
    pub inactive_waiting: usize,
    /// Single glyphs whose sprite is the `selected` colour.
    pub selected_glyphs: usize,
    pub scan_lines: Vec<ScanLineState>,
    pub cursors: Vec<CursorState>,
    /// `parts`' length.
    pub parts: usize,
}

/// One room's `ProjectedScanLines`.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectedScanLinesState {
    pub room: String,
    pub id: u64,
    pub amount: f32,
    pub visible: bool,
    pub ideal: usize,
    pub lines: Vec<ScanLineState>,
}

/// One room's `StarMatrix`.
#[derive(Debug, Clone, Serialize)]
pub struct StarMatrixState {
    pub room: String,
    pub id: u64,
    pub pos: [f32; 2],
    pub rad: f32,
    pub stars: usize,
    pub connections: usize,
    pub circles: usize,
    pub sprites: usize,
    /// `xRotat`, `yRotat`, `zRotat` (the last never moves).
    pub rotation: [f32; 3],
    pub focus: [f32; 3],
    pub get_to_focus: [f32; 3],
}

/// The three lists of `get_state`.
#[derive(Debug, Clone, Default)]
pub struct ProjectionStates {
    pub projectors: Vec<ProjectorState>,
    pub scan_lines: Vec<ProjectedScanLinesState>,
    pub star_matrices: Vec<StarMatrixState>,
}

impl ProjectionStates {
    /// Every realized room's projections.
    #[must_use]
    pub fn all(world: &World) -> Self {
        let mut out = Self::default();
        for i in world.active_rooms() {
            let Some(r) = world.realized(i) else { continue };
            let room = RoomName::of(world, i);
            out.projectors.extend(
                r.projections
                    .projectors
                    .iter()
                    .map(|p| ProjectorState::of(&room, p)),
            );
            out.scan_lines.extend(
                r.projections
                    .scan_lines
                    .iter()
                    .map(|p| ProjectedScanLinesState::of(&room, p)),
            );
            out.star_matrices.extend(
                r.projections
                    .star_matrices
                    .iter()
                    .map(|s| StarMatrixState::of(&room, s)),
            );
        }
        out
    }
}

impl ProjectorState {
    fn of(room: &str, p: &SuperStructureProjector) -> Self {
        let mut scan_lines = Vec::new();
        let mut cursors = Vec::new();
        for part in p.parts.iter().filter(|part| !part.slated) {
            match part.kind {
                PartKind::ScanLine(l) => scan_lines.push(ScanLineState {
                    vertical: l.alignment,
                    pos: l.pos,
                    speed: l.speed,
                }),
                PartKind::Cursor(c) => cursors.push(CursorState {
                    g_pos: [c.g_pos.0, c.g_pos.1],
                    pos: c.pos.to_array(),
                    size: c.size,
                }),
                _ => {}
            }
        }
        let single_glyphs = p
            .glyphs_list
            .iter()
            .filter(|&&o| matches!(p.part(o).map(|part| part.kind), Some(PartKind::Single(_))))
            .count();
        Self {
            room: room.to_owned(),
            id: p.update_order,
            amount: p.amount,
            visible: p.visible,
            grid_pos: p.grid_pos.to_array(),
            grid_speed: p.grid_speed.to_array(),
            int_offset: [p.int_offset.0, p.int_offset.1],
            grid_offset: p.grid_offset(1.0).to_array(),
            last_cam_pos: p.last_cam_pos,
            ideal_glyphs: p.ideal_glyph_number(),
            ideal_scan_lines: p.ideal_scan_line_number(),
            ideal_cursors: p.ideal_cursor_number(),
            glyphs: p.glyphs_list.len(),
            single_glyphs,
            glyph_matrices: p.glyphs_list.len() - single_glyphs,
            inactive_waiting: p.inactive_glyphs_waiting_room.len(),
            selected_glyphs: p
                .parts
                .iter()
                .filter(|part| {
                    !part.slated
                        && matches!(
                            part.kind,
                            PartKind::Single(SingleGlyph {
                                sprite_selected: true,
                                inactive: false,
                                ..
                            })
                        )
                })
                .count(),
            scan_lines,
            cursors,
            parts: p.parts.len(),
        }
    }
}

impl ProjectedScanLinesState {
    fn of(room: &str, p: &ProjectedScanLines) -> Self {
        Self {
            room: room.to_owned(),
            id: p.update_order,
            amount: p.amount,
            visible: p.visible,
            ideal: p.ideal_scan_line_number(),
            lines: p
                .lines
                .iter()
                .filter(|l| !l.slated)
                .map(|l| ScanLineState {
                    vertical: l.alignment,
                    pos: l.pos,
                    speed: l.speed,
                })
                .collect(),
        }
    }
}

impl StarMatrixState {
    fn of(room: &str, s: &StarMatrix) -> Self {
        Self {
            room: room.to_owned(),
            id: s.update_order,
            pos: s.pos.to_array(),
            rad: s.rad,
            stars: s.stars.len(),
            connections: s.stars.iter().map(|x| x.connections.len()).sum(),
            circles: s.stars.iter().map(|x| x.circles).sum(),
            sprites: s.total_sprites,
            rotation: [s.x_rotat, s.y_rotat, s.z_rotat],
            focus: [s.focus.x, s.focus.y, s.focus.z],
            get_to_focus: [s.get_to_focus.x, s.get_to_focus.y, s.get_to_focus.z],
        }
    }
}
