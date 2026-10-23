//! `ShortcutGraphics`: the room camera's shortcut sprites. Every pipe tile (`shortCut ==
//! 1`) has a `pixel` lit by the pipe's blink pulse or, under a travelling vessel, in
//! the creature's shortcut colour; every shown entrance has its symbol (arrow, exit
//! dots, shelter, ancient shelter) and a flash glow.
//!
//! Behaviour: `rainworld-re/notes/shortcuts.md` section 10. Base game: the More Slugcats
//! symbols (dens, NPC and region transports, shown when controlling a creature), the
//! Remix gate symbol and underwater shortcuts, and the arena's exit darkening are left
//! out; the blink state is the room's (`rw_sim::ShortcutsBlinking`).
//!
//! Mirrors: ShortcutGraphics.cs

use rw_assets::{
    PlacedObjectType, Room, Shortcut, ShortcutType, SlugcatName, Terrain, tile_centre,
};
use rw_random::UnityRandom;
use rw_sim::{FloatExt, ShortCutColor, Vec2, Water};

use crate::core::math::CustomVec;
use crate::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// What decides a room exit's symbol: the room, its world and what lies behind its exits.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutRoom<'a> {
    pub room: &'a Room,
    pub water: Option<&'a Water>,
    /// `world.singleRoomWorld`.
    pub single_room_world: bool,
    /// `abstractRoom.gate`.
    pub gate: bool,
    /// `Room.IsGateRoom()`: a gate, or the `FakeGate` effect.
    pub gate_room: bool,
    /// `abstractRoom.connections`, one per exit node.
    pub connections: &'a [i32],
    /// The room behind each exit node (`world.GetAbstractRoom(connections[n])`), `None`
    /// where there is none.
    pub behind: &'a [Option<ExitBehind>],
    /// The active placed objects in file order with their tiles
    /// (`room.GetTilePosition(pos)`).
    pub placed: &'a [(PlacedObjectType, (i32, i32))],
}

/// The room behind an exit, as the exit symbol reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExitBehind {
    pub shelter: bool,
    /// `world.brokenShelters[shelterIndex]`.
    pub broken: bool,
    /// `isAncientShelter`.
    pub ancient: bool,
}

/// A vessel travelling in the room (`ShortcutHandler.transportVessels` of the room).
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutVessel {
    pub pos: (i32, i32),
    pub last_positions: Vec<(i32, i32)>,
    /// `creature.ShortCutColor()`.
    pub color: Color,
    /// `Template.shortcutSegments`.
    pub segments: i32,
}

/// The palette colours the shortcuts use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShortcutPalette {
    /// `shortcutColors`: dark, middle and lit pipe.
    pub colors: [Color; 3],
    /// `shortCutSymbol`.
    pub symbol: Color,
}

/// One frame's input to [`ShortcutGraphics::draw`].
#[derive(Debug, Clone, Copy)]
pub struct ShortcutFrame<'a> {
    pub room: &'a Room,
    pub water: Option<&'a Water>,
    /// `room.shortcutsBlinking`.
    pub blinking: &'a [[f32; 4]],
    pub palette: ShortcutPalette,
    /// `CamPos(currentCameraPosition)`: the camera image's bottom-left in the room.
    pub camera: Vec2,
    pub vessels: &'a [ShortcutVessel],
}

/// A pipe tile's `pixel`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PipeSprite {
    tile: (i32, i32),
    layer: Layer,
    shader: Shader,
    scale: f32,
    /// `DisplayLayer == 0`: drawn at depth -5, else 5.
    front: bool,
}

/// An entrance's symbol (`entranceSprites[k, 0]`); its glow (`[k, 1]`) is only in a
/// container when the symbol is not under water.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EntranceSprite {
    element: &'static str,
    /// Futile degrees.
    rotation: f32,
    /// `entranceSpriteLocations`.
    location: Vec2,
    layer: Layer,
    glow: bool,
}

/// The shortcut sprites of the room camera 0 shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutGraphics {
    pipes: Vec<PipeSprite>,
    /// Index into `pipes` per tile (`x * height + y`), the game's `sprites` keys.
    by_tile: Vec<Option<usize>>,
    height: i32,
    /// `lightness`, per pipe sprite (the game keeps a key per sprite tile).
    lightness: Vec<f32>,
    /// Per shortcut: its symbol, `None` where the game makes none.
    entrances: Vec<Option<EntranceSprite>>,
    /// `entranceSpriteColors`: an entrance's flash in a creature's colour, alpha fading.
    entrance_colors: Vec<Color>,
}

impl ShortcutGraphics {
    /// `pixel`'s scale on a pipe tile.
    const PIPE_SCALE: f32 = 15.0;
    /// `pixel`'s scale on an entrance tile.
    const ENTRANCE_TILE_SCALE: f32 = 20.0;
    /// The colour of whatever is under water in front of the terrain.
    const UNDER_WATER: Color = Color::rgb(0.0, 2.0 / 255.0, 0.0);

    /// `GenerateSprites` (on `NewRoom`).
    #[must_use]
    pub fn new(r: &ShortcutRoom<'_>) -> Self {
        let room = r.room;
        let height = room.height();
        let water_row = Self::water_row(room, r.water);
        let in_front = room.water_in_front();
        let mut pipes = Vec::new();
        let mut by_tile = vec![None; (room.width() * height).max(0) as usize];
        for x in 0..room.width() {
            for y in 0..height {
                let tile = room.tile(x, y);
                if tile.shortcut != Shortcut::Path {
                    continue;
                }
                let pipe = if tile.terrain == Terrain::ShortcutEntrance {
                    // `RippleBasicClipDistortion`: Basic without the Watcher's ripple.
                    PipeSprite {
                        tile: (x, y),
                        layer: Layer::Background,
                        shader: Shader::Basic,
                        scale: Self::ENTRANCE_TILE_SCALE,
                        front: true,
                    }
                } else if tile.terrain != Terrain::Air {
                    PipeSprite {
                        tile: (x, y),
                        layer: if y < water_row && in_front {
                            Layer::Items
                        } else {
                            Layer::Shortcuts
                        },
                        shader: Shader::ShortCut0,
                        scale: Self::PIPE_SCALE,
                        front: true,
                    }
                } else {
                    PipeSprite {
                        tile: (x, y),
                        layer: Layer::BackgroundShortcuts,
                        shader: Shader::ShortCut0,
                        scale: Self::PIPE_SCALE,
                        front: false,
                    }
                };
                by_tile[(x * height + y) as usize] = Some(pipes.len());
                pipes.push(pipe);
            }
        }
        let shortcuts = room.shortcut_map().shortcuts();
        let entrances = shortcuts
            .iter()
            .map(|s| Self::entrance(r, s.kind, s.start, s.dest_node))
            .collect();
        Self {
            lightness: vec![0.0; pipes.len()],
            pipes,
            by_tile,
            height,
            entrances,
            entrance_colors: vec![Color::default(); shortcuts.len()],
        }
    }

    /// `Room.defaultWaterLevel`: the water's row, -1 without water.
    fn water_row(room: &Room, water: Option<&Water>) -> i32 {
        water.map_or_else(|| room.water_level().unwrap_or(-1), Water::level_row)
    }

    /// The symbol of the shortcut starting at `start`: the arrow of a pipe, the exit
    /// dots, shelter or ancient shelter of a room exit (by the room behind and the
    /// `ExitSymbol` objects on the entrance), none for a hidden exit or any other kind.
    fn entrance(
        r: &ShortcutRoom<'_>,
        kind: ShortcutType,
        start: (i32, i32),
        dest_node: Option<usize>,
    ) -> Option<EntranceSprite> {
        let element = match kind {
            ShortcutType::Normal => "ShortcutArrow",
            ShortcutType::RoomExit => Self::exit_symbol(r, start, dest_node)?,
            _ => return None,
        };
        let room = r.room;
        let hole = room.shortcut_hole_direction(start.0, start.1);
        let hole_v = Vec2::new(hole.0 as f32, hole.1 as f32);
        let location = Vec2::from(tile_centre(start.0, start.1)) + hole_v * 15.0;
        let submerged = r
            .water
            .is_some_and(|w| w.in_front() && Self::submerged(w, location + Vec2::new(0.0, 5.0)));
        Some(EntranceSprite {
            element,
            rotation: Vec2::ZERO.aim_to(-hole_v),
            location,
            layer: if submerged {
                Layer::Items
            } else {
                Layer::Shortcuts
            },
            glow: !submerged,
        })
    }

    /// A room exit's symbol, `None` when hidden.
    fn exit_symbol(
        r: &ShortcutRoom<'_>,
        start: (i32, i32),
        dest_node: Option<usize>,
    ) -> Option<&'static str> {
        let mut shelter = false;
        let mut shown = true;
        let mut ancient = false;
        let connection = dest_node.and_then(|n| r.connections.get(n)).copied();
        if !r.single_room_world
            && let Some(behind) = dest_node.and_then(|n| r.behind.get(n).copied().flatten())
            && behind.shelter
        {
            shelter = true;
            shown = !behind.broken;
            ancient = behind.ancient;
        }
        for &(kind, tile) in r.placed {
            if tile != start {
                continue;
            }
            let shelter_symbol = if r.single_room_world {
                true
            } else {
                r.gate_room || !shelter
            };
            match kind {
                PlacedObjectType::ExitSymbolAncientShelter => {
                    shelter = shelter_symbol;
                    ancient = true;
                }
                PlacedObjectType::ExitSymbolShelter => shelter = shelter_symbol,
                PlacedObjectType::ExitSymbolHidden => shown = false,
                _ => continue,
            }
            break;
        }
        if !r.gate && !r.single_room_world && connection.is_none_or(|c| c < 0) {
            shown = false;
        }
        shown.then_some(match (shelter, ancient) {
            (false, _) => "ShortcutDots",
            (true, false) => "ShortcutShelter",
            (true, true) => "ShortcutAShelter",
        })
    }

    /// `Room.PointSubmerged` (`DetailedWaterLevel`).
    fn submerged(water: &Water, p: Vec2) -> bool {
        p.y < water.level_at(p.x)
    }

    /// `Update` (each tick of the room camera): entrance flashes fade by 0.025.
    pub fn update(&mut self) {
        for c in &mut self.entrance_colors {
            if c.a > 0.0 {
                c.a = (c.a - 0.025).max(0.0);
            }
        }
    }

    /// `ColorEntrance`.
    pub fn color_entrance(&mut self, entrance: usize, color: Color) {
        if let Some(c) = self.entrance_colors.get_mut(entrance) {
            *c = color;
        }
    }

    /// A creature's `ShortCutColor()`, the player's in `slugcat`'s colour.
    #[must_use]
    pub fn creature_color(color: ShortCutColor, slugcat: SlugcatName) -> Color {
        match color {
            ShortCutColor::Slugcat => crate::PlayerGraphics::default_slugcat_color(slugcat),
            ShortCutColor::Rgb([r, g, b]) => Color::rgb(r, g, b),
        }
    }

    /// `ColorFromLightness`: dark to middle pipe colour up to 0.5, middle to lit above.
    fn color_from_lightness(palette: &ShortcutPalette, l: f32) -> Color {
        if l < 0.5 {
            palette.colors[0].lerp(palette.colors[1], l.inverse_lerp(0.0, 0.5))
        } else {
            palette.colors[1].lerp(palette.colors[2], l.inverse_lerp(0.5, 1.0))
        }
    }

    fn pipe_at(&self, (x, y): (i32, i32)) -> Option<usize> {
        if x < 0 || y < 0 || y >= self.height {
            return None;
        }
        self.by_tile
            .get((x * self.height + y) as usize)
            .copied()
            .flatten()
    }

    /// `Draw`: the blink pulses light the pipes, the entrances take their colours and
    /// flashes, the pipes their lightness, then each vessel lights its tile (and, for a
    /// creature of several segments, the tiles behind it) in its colour. Its draws (a
    /// blinking pipe's flicker, a flashing glow's size) are cosmetic: `rng` stands in
    /// for the game's per-frame draws on the global stream.
    pub fn draw(&mut self, f: &ShortcutFrame<'_>, rng: &mut UnityRandom) -> DrawList {
        let shortcuts = f.room.shortcut_map().shortcuts();
        for (row, s) in f.blinking.iter().zip(shortcuts) {
            if row[0] <= 0.0 {
                continue;
            }
            let along = if row[1] > 0.0 {
                0.0_f32.lerp(2.0, row[1])
            } else {
                -1.0
            };
            let spread = (-1.5_f32).lerp(1.5, rng.value());
            let on = if rng.value() < 1.0 / 11.0 { 1.0 } else { 0.0 };
            let mut flicker = row[0] + (0.5 - (row[0] - 0.5).abs()) * spread * on;
            if rng.value() < 1.0 / 30.0 {
                flicker = row[0] * rng.value();
            }
            for (j, &tile) in s.path.iter().enumerate() {
                let Some(i) = self.pipe_at(tile) else {
                    continue;
                };
                let d = (1.0 - (j as f32 / s.length as f32 - along).abs()).clamp(0.0, 1.0);
                let l = d.mathf_pow(5.0) * 0.5 + flicker * 0.5;
                // `Mathf.Max(lightness, l)`: `a > b ? a : b`.
                let old = self.lightness[i];
                self.lightness[i] = if old > l { old } else { l };
            }
        }
        let depth_point = f.camera + Vec2::new(700.0, 533.3334);
        let mut list = DrawList::default();
        let mut index = 0;
        let mut icons = Vec::new();
        for (k, entrance) in self.entrances.iter().enumerate() {
            let Some(e) = entrance else {
                continue;
            };
            let pos = e.location.apply_depth(depth_point, -5.0);
            let row = f.blinking.get(k).copied().unwrap_or_default();
            let flash = row[3].abs();
            let mut glow = None;
            let mut color = if row[3] < 0.0 {
                let mut alpha = flash.lerp_map(0.0, 20.0, 0.0, 0.35);
                let mut scale = (flash + rng.value() * 10.0).lerp_map(0.0, 20.0, 1.5, 2.5);
                if flash > 20.0 && (flash as i32) % 4 < 3 {
                    scale += flash.lerp_map_pow(20.0, 50.0, 0.0, 1.6, 1.4);
                    alpha += flash.lerp_map(20.0, 50.0, 0.0, 0.35);
                }
                glow = Some((scale, alpha));
                if (flash as i32) % 4 < 3 {
                    f.palette.symbol
                } else {
                    Self::color_from_lightness(&f.palette, 0.0)
                }
            } else {
                let start = shortcuts[k].start;
                let l = self.pipe_at(start).map_or(0.0, |i| self.lightness[i]);
                let symbol = f.palette.colors[2].lerp(f.palette.symbol, 0.75);
                Self::color_from_lightness(&f.palette, l.max(0.0)).lerp(symbol, row[2])
            };
            let flash_color = self.entrance_colors[k];
            color = color.lerp(flash_color.with_alpha(1.0), flash_color.a);
            let glow_color = Color::WHITE.lerp(flash_color.with_alpha(1.0), flash_color.a);
            if let Some(w) = f.water
                && w.in_front()
                && Self::submerged(w, e.location + Vec2::new(0.0, 5.0))
            {
                color = Self::UNDER_WATER;
            }
            icons.push(Sprite {
                index: k,
                element: e.element,
                pos,
                rotation: e.rotation,
                scale: Vec2::new(1.0, 1.0),
                anchor: Vec2::new(0.5, 0.5),
                color,
                shader: Shader::Basic,
                layer: e.layer,
            });
            if let Some((scale, alpha)) = glow.filter(|_| e.glow) {
                icons.push(Sprite {
                    index: k,
                    element: "Futile_White",
                    pos,
                    rotation: 0.0,
                    scale: Vec2::new(scale, scale),
                    anchor: Vec2::new(0.5, 0.5),
                    color: glow_color.with_alpha(alpha),
                    shader: Shader::FlatLight,
                    layer: Layer::Water,
                });
            }
        }
        let mut colors: Vec<Color> = self
            .lightness
            .iter()
            .map(|&l| Self::color_from_lightness(&f.palette, l))
            .collect();
        self.lightness.fill(0.0);
        for v in f.vessels {
            let Some(i) = self.pipe_at(v.pos) else {
                continue;
            };
            colors[i] = Self::vessel_color(f, v, v.pos);
            if v.segments <= 1 {
                continue;
            }
            for &p in &v.last_positions {
                if let Some(i) = self.pipe_at(p) {
                    colors[i] = Self::vessel_color(f, v, p);
                }
            }
        }
        for (pipe, color) in self.pipes.iter().zip(colors) {
            let middle = Vec2::from(tile_centre(pipe.tile.0, pipe.tile.1));
            let depth = if pipe.front { -5.0 } else { 5.0 };
            list.items.push(DrawItem::Sprite(Sprite {
                index,
                element: "pixel",
                pos: middle.apply_depth(depth_point, depth),
                rotation: 0.0,
                scale: Vec2::new(pipe.scale, pipe.scale),
                anchor: Vec2::new(0.5, 0.5),
                color,
                shader: pipe.shader,
                layer: pipe.layer,
            }));
            index += 1;
        }
        list.items.extend(icons.into_iter().map(DrawItem::Sprite));
        list
    }

    /// `ShortCutColor(crit, pos)`: the creature's colour, or the under-water colour on a
    /// pipe tile below the water line that is behind the water.
    fn vessel_color(f: &ShortcutFrame<'_>, v: &ShortcutVessel, pos: (i32, i32)) -> Color {
        let Some(w) = f.water else {
            return v.color;
        };
        let tile = f.room.tile(pos.0, pos.1);
        let behind_water = tile.terrain == Terrain::Air || w.in_front();
        if pos.1 < w.level_row() && tile.terrain != Terrain::ShortcutEntrance && behind_water {
            Self::UNDER_WATER
        } else {
            v.color
        }
    }

    /// The symbol of shortcut `k`, if it has one.
    #[must_use]
    pub fn entrance_element(&self, k: usize) -> Option<&'static str> {
        self.entrances.get(k).copied().flatten().map(|e| e.element)
    }
}

#[cfg(test)]
#[path = "tests/shortcut.rs"]
mod tests;
