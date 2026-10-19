//! Debug geometry of a room's tile grid.
//!
//! Slope orientation comes from [`Room::identify_slope`]; the solid half of each
//! direction is the table in `rainworld-re/notes/slope-collision.md` section 2 (the
//! names are the game's and are not geometric).
//!
//! Mirrors: -

use rw_assets::{Room, Shortcut, Slope, TILE_SIZE, Terrain};

use crate::Mesh;

/// What to draw for one tile feature, in tile coordinates (y up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomShape {
    Solid {
        x: i32,
        y: i32,
    },
    Slope {
        x: i32,
        y: i32,
        slope: Slope,
    },
    /// A `Slope` tile without a usable orientation (the game's `Broken`; no collision).
    BrokenSlope {
        x: i32,
        y: i32,
    },
    /// One-way platform, solid from above: a bar along the tile's top edge.
    Floor {
        x: i32,
        y: i32,
    },
    VerticalPole {
        x: i32,
        y: i32,
    },
    HorizontalPole {
        x: i32,
        y: i32,
    },
    /// `hole` is the open side, see [`Room::shortcut_hole_direction`].
    ShortcutEntrance {
        x: i32,
        y: i32,
        hole: (i32, i32),
    },
    /// One step of a traced shortcut tube, between neighbouring tile centres.
    ShortcutLink {
        from: (i32, i32),
        to: (i32, i32),
    },
    /// A shortcut flag on a tile that is not an entrance (tube, exit, den, transport).
    ShortcutMark {
        x: i32,
        y: i32,
        kind: Shortcut,
    },
}

/// Colours of the debug room.
mod palette {
    use crate::Color;

    pub const SOLID: Color = Color::rgb(0.42, 0.45, 0.53);
    pub const BROKEN_SLOPE: Color = Color::rgba(0.9, 0.2, 0.6, 0.45);
    pub const FLOOR: Color = Color::rgb(0.80, 0.66, 0.40);
    pub const POLE: Color = Color::rgb(0.70, 0.55, 0.35);
    pub const ENTRANCE_FILL: Color = Color::rgb(0.20, 0.18, 0.10);
    pub const ENTRANCE_EDGE: Color = Color::rgb(1.0, 0.85, 0.2);
    pub const PATH: Color = Color::rgba(1.0, 0.85, 0.2, 0.45);
    pub const LINK: Color = Color::rgba(1.0, 0.9, 0.4, 0.75);
    pub const ROOM_EXIT: Color = Color::rgb(0.3, 0.95, 0.4);
    pub const DEN: Color = Color::rgb(0.95, 0.3, 0.3);
    pub const TRANSPORT: Color = Color::rgb(0.65, 0.4, 1.0);
    pub const GRID: Color = Color::rgba(1.0, 1.0, 1.0, 0.10);
    pub const BORDER: Color = Color::rgba(1.0, 1.0, 1.0, 0.35);
}

/// Floor bar thickness, px.
const FLOOR_THICKNESS: f32 = 4.0;
/// Pole line width, px.
const POLE_WIDTH: f32 = 2.0;
/// Shortcut tube line width, px.
const LINK_WIDTH: f32 = 3.0;

/// Drawing geometry of a slope orientation.
pub trait SlopeGeometry {
    /// Solid triangle of the slope tile, relative to the tile's bottom-left corner.
    fn solid_triangle(self) -> [[f32; 2]; 3];
}

impl SlopeGeometry for Slope {
    /// | Slope | solid half |
    /// |---|---|
    /// | `UpLeft` | lower-right, surface `h = lx` |
    /// | `UpRight` | lower-left, surface `h = 20 - lx` |
    /// | `DownLeft` | upper-right, surface `h = 20 - lx` |
    /// | `DownRight` | upper-left, surface `h = lx` |
    fn solid_triangle(self) -> [[f32; 2]; 3] {
        const S: f32 = TILE_SIZE;
        match self {
            Self::UpLeft => [[0.0, 0.0], [S, 0.0], [S, S]],
            Self::UpRight => [[0.0, 0.0], [0.0, S], [S, 0.0]],
            Self::DownLeft => [[0.0, S], [S, S], [S, 0.0]],
            Self::DownRight => [[0.0, 0.0], [0.0, S], [S, S]],
        }
    }
}

/// Debug drawing geometry of a room.
pub trait RoomDebugGeometry {
    /// Every drawable tile feature, column by column, bottom row first, then the links
    /// of every traced shortcut tube (each step once).
    fn debug_shapes(&self) -> Vec<RoomShape>;
    /// Triangles for the tile geometry in room pixels: terrain first, then floors,
    /// poles and shortcut markers on top.
    fn debug_mesh(&self) -> Mesh;
    /// Thin lines on every tile boundary plus the room border.
    fn debug_grid_mesh(&self) -> Mesh;
}

impl RoomDebugGeometry for Room {
    fn debug_shapes(&self) -> Vec<RoomShape> {
        let mut out = Vec::new();
        for x in 0..self.width() {
            for y in 0..self.height() {
                let tile = self.tile(x, y);
                match tile.terrain {
                    Terrain::Air => {}
                    Terrain::Solid => out.push(RoomShape::Solid { x, y }),
                    Terrain::Slope => out.push(match self.identify_slope(x, y) {
                        Some(slope) => RoomShape::Slope { x, y, slope },
                        None => RoomShape::BrokenSlope { x, y },
                    }),
                    Terrain::Floor => out.push(RoomShape::Floor { x, y }),
                    Terrain::ShortcutEntrance => out.push(RoomShape::ShortcutEntrance {
                        x,
                        y,
                        hole: self.shortcut_hole_direction(x, y),
                    }),
                }
                if tile.vertical_pole() {
                    out.push(RoomShape::VerticalPole { x, y });
                }
                if tile.horizontal_pole() {
                    out.push(RoomShape::HorizontalPole { x, y });
                }
                if tile.shortcut != Shortcut::None && tile.terrain != Terrain::ShortcutEntrance {
                    out.push(RoomShape::ShortcutMark {
                        x,
                        y,
                        kind: tile.shortcut,
                    });
                }
            }
        }
        // A dead end's path repeats its last tile; that step draws nothing.
        let mut links: Vec<((i32, i32), (i32, i32))> = self
            .shortcut_map()
            .shortcuts()
            .iter()
            .flat_map(|s| s.path.windows(2).map(|w| (w[0].min(w[1]), w[0].max(w[1]))))
            .filter(|(a, b)| a != b)
            .collect();
        links.sort_unstable();
        links.dedup();
        out.extend(
            links
                .into_iter()
                .map(|(from, to)| RoomShape::ShortcutLink { from, to }),
        );
        out
    }

    fn debug_mesh(&self) -> Mesh {
        let mut shapes = self.debug_shapes();
        shapes.sort_by_key(RoomShape::layer);
        let mut mesh = Mesh::new();
        for shape in shapes {
            shape.push_to(&mut mesh);
        }
        mesh
    }

    fn debug_grid_mesh(&self) -> Mesh {
        let (w, h) = (self.pixel_width(), self.pixel_height());
        let mut mesh = Mesh::new();
        for x in 0..=self.width() {
            let px = x as f32 * TILE_SIZE;
            mesh.line([px, 0.0], [px, h], 1.0, palette::GRID);
        }
        for y in 0..=self.height() {
            let py = y as f32 * TILE_SIZE;
            mesh.line([0.0, py], [w, py], 1.0, palette::GRID);
        }
        mesh.rect_outline([0.0, 0.0], [w, h], 2.0, palette::BORDER);
        mesh
    }
}

/// Pixel rectangle of one tile.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TileRect {
    min: [f32; 2],
    max: [f32; 2],
}

impl TileRect {
    fn at(x: i32, y: i32) -> Self {
        let min = [x as f32 * TILE_SIZE, y as f32 * TILE_SIZE];
        Self {
            min,
            max: [min[0] + TILE_SIZE, min[1] + TILE_SIZE],
        }
    }

    fn mid(self) -> [f32; 2] {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
        ]
    }

    /// The edge on side `dir` (a unit grid direction).
    fn edge(self, dir: (i32, i32)) -> ([f32; 2], [f32; 2]) {
        let (min, max) = (self.min, self.max);
        match dir {
            (-1, 0) => (min, [min[0], max[1]]),
            (1, 0) => ([max[0], min[1]], max),
            (0, -1) => (min, [max[0], min[1]]),
            _ => ([min[0], max[1]], max),
        }
    }

    /// Shrunk by `d` on every side.
    fn inset(self, d: f32) -> Self {
        Self {
            min: [self.min[0] + d, self.min[1] + d],
            max: [self.max[0] - d, self.max[1] - d],
        }
    }
}

impl RoomShape {
    /// Tile coordinates of the shape.
    #[must_use]
    pub fn tile(self) -> (i32, i32) {
        match self {
            Self::Solid { x, y }
            | Self::Slope { x, y, .. }
            | Self::BrokenSlope { x, y }
            | Self::Floor { x, y }
            | Self::VerticalPole { x, y }
            | Self::HorizontalPole { x, y }
            | Self::ShortcutEntrance { x, y, .. }
            | Self::ShortcutMark { x, y, .. } => (x, y),
            Self::ShortcutLink { from, .. } => from,
        }
    }

    /// Draw order: lower layers first.
    fn layer(&self) -> u8 {
        match self {
            Self::Solid { .. } | Self::Slope { .. } | Self::BrokenSlope { .. } => 0,
            Self::Floor { .. } | Self::ShortcutEntrance { .. } => 1,
            Self::VerticalPole { .. } | Self::HorizontalPole { .. } => 2,
            Self::ShortcutLink { .. } => 3,
            Self::ShortcutMark { .. } => 4,
        }
    }

    /// Appends this shape's triangles to `mesh`.
    fn push_to(self, mesh: &mut Mesh) {
        let (x, y) = self.tile();
        let r = TileRect::at(x, y);
        match self {
            Self::Solid { .. } => mesh.rect(r.min, r.max, palette::SOLID),
            Self::Slope { slope, .. } => {
                let [a, b, c] = slope
                    .solid_triangle()
                    .map(|[px, py]| [r.min[0] + px, r.min[1] + py]);
                mesh.triangle(a, b, c, palette::SOLID);
            }
            Self::BrokenSlope { .. } => {
                mesh.line(r.min, r.max, 2.0, palette::BROKEN_SLOPE);
                mesh.line(
                    [r.min[0], r.max[1]],
                    [r.max[0], r.min[1]],
                    2.0,
                    palette::BROKEN_SLOPE,
                );
            }
            Self::Floor { .. } => {
                mesh.rect(
                    [r.min[0], r.max[1] - FLOOR_THICKNESS],
                    r.max,
                    palette::FLOOR,
                );
            }
            Self::VerticalPole { .. } => {
                let cx = r.mid()[0];
                mesh.line([cx, r.min[1]], [cx, r.max[1]], POLE_WIDTH, palette::POLE);
            }
            Self::HorizontalPole { .. } => {
                let cy = r.mid()[1];
                mesh.line([r.min[0], cy], [r.max[0], cy], POLE_WIDTH, palette::POLE);
            }
            Self::ShortcutEntrance { hole, .. } => {
                mesh.rect(r.min, r.max, palette::ENTRANCE_FILL);
                let inner = r.inset(3.0);
                for side in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
                    if side != hole {
                        let (a, b) = inner.edge(side);
                        mesh.line(a, b, 2.0, palette::ENTRANCE_EDGE);
                    }
                }
                let c = r.mid();
                mesh.circle(c, 3.0, palette::ENTRANCE_EDGE);
                if hole != (0, 0) {
                    let (hx, hy) = (hole.0 as f32, hole.1 as f32);
                    mesh.triangle(
                        [c[0] + hx * 9.0, c[1] + hy * 9.0],
                        [c[0] + hx * 4.0 - hy * 4.0, c[1] + hy * 4.0 + hx * 4.0],
                        [c[0] + hx * 4.0 + hy * 4.0, c[1] + hy * 4.0 - hx * 4.0],
                        palette::ENTRANCE_EDGE,
                    );
                }
            }
            Self::ShortcutLink { to, .. } => {
                let b = TileRect::at(to.0, to.1).mid();
                mesh.line(r.mid(), b, LINK_WIDTH, palette::LINK);
            }
            Self::ShortcutMark { kind, .. } => {
                let (color, radius) = match kind {
                    Shortcut::None => return,
                    Shortcut::Path => (palette::PATH, 2.0),
                    Shortcut::RoomExit => (palette::ROOM_EXIT, 5.0),
                    Shortcut::CreatureDen => (palette::DEN, 5.0),
                    Shortcut::NpcTransportation | Shortcut::RegionTransportation => {
                        (palette::TRANSPORT, 5.0)
                    }
                };
                mesh.circle(r.mid(), radius, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_room::FromAscii;

    struct Triangle([[f32; 2]; 3]);

    impl Triangle {
        /// Strictly inside (points on an edge are excluded).
        fn contains(&self, p: [f32; 2]) -> bool {
            let [a, b, c] = self.0;
            let s = [
                Self::side(a, b, p),
                Self::side(b, c, p),
                Self::side(c, a, p),
            ];
            s.iter().all(|&v| v > 0.0) || s.iter().all(|&v| v < 0.0)
        }

        /// Cross product sign of `p` relative to edge `a -> b`.
        fn side(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        }
    }

    /// Solid side of each slope per slope-collision.md section 2: the surface is
    /// `h(lx) = offset + rise * lx`; floors are solid below it, ceilings above.
    #[test]
    fn slope_triangles_match_collision_surface() {
        // (slope, offset, rise, floor)
        let cases = [
            (Slope::UpLeft, 0.0, 1.0, true),
            (Slope::UpRight, 20.0, -1.0, true),
            (Slope::DownLeft, 20.0, -1.0, false),
            (Slope::DownRight, 0.0, 1.0, false),
        ];
        for (slope, offset, rise, floor) in cases {
            let t = Triangle(slope.solid_triangle());
            for lx in [2.0, 5.0, 10.0, 15.0, 18.0] {
                let h = offset + rise * lx;
                let (below, above) = ([lx, h - 1.0], [lx, h + 1.0]);
                let (solid, open) = if floor {
                    (below, above)
                } else {
                    (above, below)
                };
                // A solid probe outside the tile cannot be inside the triangle.
                if (0.0..=20.0).contains(&solid[1]) {
                    assert!(t.contains(solid), "{slope:?} lx {lx}: {solid:?} solid");
                }
                assert!(!t.contains(open), "{slope:?} lx {lx}: {open:?} open");
            }
        }
    }

    #[test]
    fn shapes_classify_terrain_and_flags() {
        // y = 2 top row, y = 0 bottom row.
        let shapes = Room::from_ascii(&["|=E.", "#/-x", "####"]).debug_shapes();
        for want in [
            RoomShape::VerticalPole { x: 0, y: 2 },
            RoomShape::HorizontalPole { x: 1, y: 2 },
            // Left is a horizontal pole on Air: open to the left.
            RoomShape::ShortcutEntrance {
                x: 2,
                y: 2,
                hole: (-1, 0),
            },
            RoomShape::Solid { x: 0, y: 1 },
            // Left Solid and below Solid: UpRight, the lower-left half.
            RoomShape::Slope {
                x: 1,
                y: 1,
                slope: Slope::UpRight,
            },
            RoomShape::Floor { x: 2, y: 1 },
            RoomShape::ShortcutMark {
                x: 3,
                y: 1,
                kind: Shortcut::Path,
            },
        ] {
            assert!(shapes.contains(&want), "missing {want:?}");
        }
        // The entrance's own path flag is not a separate mark.
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(s, RoomShape::ShortcutMark { x: 2, y: 2, .. }))
        );
        assert_eq!(
            shapes
                .iter()
                .filter(|s| matches!(s, RoomShape::Solid { .. }))
                .count(),
            6
        );
    }

    #[test]
    fn broken_slope_is_reported() {
        let r = Room::from_ascii(&["...", "./.", "..."]);
        assert_eq!(
            r.debug_shapes(),
            vec![RoomShape::BrokenSlope { x: 1, y: 1 }]
        );
    }

    #[test]
    fn slope_mesh_is_placed_at_its_tile() {
        // Right Solid and above Solid: DownLeft at (0,0), upper-right half.
        let mesh = Room::from_ascii(&["#.", "/#"]).debug_mesh();
        let tri: Vec<[f32; 2]> = mesh
            .vertices()
            .chunks(3)
            .map(|t| [t[0].pos, t[1].pos, t[2].pos])
            .find(|t| t.iter().all(|p| p[0] <= 20.0 && p[1] <= 20.0))
            .expect("slope triangle in tile (0,0)")
            .to_vec();
        assert_eq!(tri, vec![[0.0, 20.0], [20.0, 20.0], [20.0, 0.0]]);
    }

    #[test]
    fn floor_bar_sits_on_tile_top() {
        let mesh = Room::from_ascii(&["-"]).debug_mesh();
        let ys: Vec<f32> = mesh.vertices().iter().map(|v| v.pos[1]).collect();
        assert!(ys.iter().all(|&y| (16.0..=20.0).contains(&y)));
        assert!(ys.contains(&20.0));
    }

    #[test]
    fn grid_covers_every_boundary() {
        let r = Room::from_ascii(&["...", "..."]);
        // (3 + 1) vertical + (2 + 1) horizontal lines + 4 border segments, 6 vertices each.
        assert_eq!(r.debug_grid_mesh().vertices().len(), (4 + 3 + 4) * 6);
    }

    #[test]
    fn pipe_links_are_drawn_once() {
        let r = Room::from_ascii(&[".....", "#E#E#", "#x#x#", "#xxx#", "#####"]);
        let links: Vec<RoomShape> = r
            .debug_shapes()
            .into_iter()
            .filter(|s| matches!(s, RoomShape::ShortcutLink { .. }))
            .collect();
        assert_eq!(links.len(), 6);
        assert!(links.contains(&RoomShape::ShortcutLink {
            from: (1, 2),
            to: (1, 3)
        }));
    }

    #[test]
    fn entrance_outline_is_open_on_the_hole_side() {
        // Entrance (1,1) opens upwards. Its inner outline runs at x = 23, 37 and
        // y = 23, 37 (width 2); only a horizontal edge has a vertex at (23, y +- 1).
        let mesh = Room::from_ascii(&["...", "#E#", "###"]).debug_mesh();
        let edge: Vec<[f32; 2]> = mesh
            .vertices()
            .iter()
            .filter(|v| v.color == palette::ENTRANCE_EDGE.0)
            .map(|v| v.pos)
            .collect();
        assert!(edge.contains(&[23.0, 22.0]), "bottom edge drawn");
        assert!(!edge.contains(&[23.0, 38.0]), "top edge open");
    }
}
