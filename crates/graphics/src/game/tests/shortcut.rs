//! `ShortcutGraphics` against the game's `GenerateSprites` and `Draw` (decompile lines
//! cited per test) on a small test room.

use rw_assets::{PlacedObjectType, Room, ShortcutType};
use rw_random::UnityRandom;
use rw_sim::Vec2;

use crate::{
    Color, DrawItem, DrawList, ExitBehind, Layer, Shader, ShortcutFrame, ShortcutGraphics,
    ShortcutPalette, ShortcutRoom, ShortcutVessel, Sprite,
};

/// Rows top first: `.` air, `#` solid, `E` entrance (with the path flag), `x` solid
/// pipe, `R` solid room exit, `D` solid den.
///
/// Shortcuts in the game's scan order: 0 a dead end at (6, 5), 1 a den at (9, 5),
/// 2 and 3 the two ends of a pipe at (1, 3) (hole right) and (4, 3) (hole left), 4 a
/// room exit at (8, 3) (hole left; node 0).
const ROWS: [&str; 7] = [
    "############",
    "#.....E..E.#",
    "#.....#..D.#",
    "#E..E#..E..#",
    "#x##x###R###",
    "#xxxx#######",
    "############",
];

trait TestRoom {
    fn test_room(water: Option<(i32, bool)>) -> Self;
}

impl TestRoom for Room {
    fn test_room(water: Option<(i32, bool)>) -> Self {
        let (w, h) = (ROWS[0].len(), ROWS.len());
        let mut grid = String::new();
        for x in 0..w {
            for row in ROWS {
                grid.push_str(match row.as_bytes()[x] {
                    b'#' => "1",
                    b'E' => "4,3",
                    b'x' => "1,3",
                    b'R' => "1,4",
                    b'D' => "1,5",
                    _ => "0",
                });
                grid.push('|');
            }
        }
        let (level, front) = water.map_or((-1, 0), |(l, f)| (l, i32::from(f)));
        let text = format!("T\n{w}*{h}|{level}|{front}\n0*0|0|0\n0,0\n\n\n\n\n\n\n\n{grid}\n");
        let room = Room::parse(&text).expect("test room parses");
        room.map_shortcuts();
        room
    }
}

const NOT_SHELTER: ExitBehind = ExitBehind {
    shelter: false,
    broken: false,
    ancient: false,
};

/// The exit's symbol with what is behind node 0 and the placed objects.
fn exit_symbol(
    behind: Option<ExitBehind>,
    connection: i32,
    placed: &[(PlacedObjectType, (i32, i32))],
    gate: bool,
    single_room_world: bool,
) -> Option<&'static str> {
    let room = Room::test_room(None);
    let g = ShortcutGraphics::new(&ShortcutRoom {
        room: &room,
        water: None,
        single_room_world,
        gate,
        gate_room: gate,
        connections: &[connection],
        behind: &[behind],
        placed,
    });
    g.entrance_element(4)
}

fn palette() -> ShortcutPalette {
    ShortcutPalette {
        colors: [
            Color::rgb(0.1, 0.0, 0.0),
            Color::rgb(0.5, 0.0, 0.0),
            Color::rgb(0.9, 0.0, 0.0),
        ],
        symbol: Color::rgb(0.0, 0.0, 1.0),
    }
}

fn graphics(room: &Room) -> ShortcutGraphics {
    ShortcutGraphics::new(&ShortcutRoom {
        room,
        water: None,
        single_room_world: false,
        gate: false,
        gate_room: false,
        connections: &[3],
        behind: &[Some(NOT_SHELTER)],
        placed: &[],
    })
}

/// Draws one frame of `room` with `blinking` rows and `vessels`, camera image at the
/// origin.
fn draw(
    g: &mut ShortcutGraphics,
    room: &Room,
    blinking: &[[f32; 4]],
    vessels: &[ShortcutVessel],
) -> DrawList {
    g.draw(
        &ShortcutFrame {
            room,
            water: None,
            blinking,
            palette: palette(),
            camera: Vec2::ZERO,
            vessels,
        },
        &mut UnityRandom::new(1),
    )
}

fn pixel_at(list: &DrawList, tile: (i32, i32)) -> Option<Sprite> {
    // `ApplyDepth(MiddleOfTile, -5 or 5)` about (700, 533.33): undo it to find the tile.
    list.items.iter().find_map(|i| match i {
        DrawItem::Sprite(s) if s.element == "pixel" => {
            let d = if s.layer == Layer::BackgroundShortcuts {
                5.0
            } else {
                -5.0
            };
            let scale = 1.0 + d / 400.0;
            let dp = Vec2::new(700.0, 533.3334);
            let p = (s.pos - dp) * scale + dp;
            let t = ((p.x / 20.0).floor() as i32, (p.y / 20.0).floor() as i32);
            (t == tile).then_some(*s)
        }
        _ => None,
    })
}

fn close(a: Color, b: Color) -> bool {
    [a.r - b.r, a.g - b.g, a.b - b.b, a.a - b.a]
        .iter()
        .all(|d| d.abs() < 1e-5)
}

#[test]
fn test_room_maps_as_described() {
    let room = Room::test_room(None);
    let kinds: Vec<(ShortcutType, (i32, i32))> = room
        .shortcut_map()
        .shortcuts()
        .iter()
        .map(|s| (s.kind, s.start))
        .collect();
    assert_eq!(
        kinds,
        [
            (ShortcutType::DeadEnd, (6, 5)),
            (ShortcutType::CreatureHole, (9, 5)),
            (ShortcutType::Normal, (1, 3)),
            (ShortcutType::Normal, (4, 3)),
            (ShortcutType::RoomExit, (8, 3)),
        ]
    );
}

/// `GenerateSprites`: `ShortcutArrow` for a pipe; nothing for a dead end
/// or (without More Slugcats) a den; a room exit's dots.
#[test]
fn symbol_per_shortcut_type() {
    let room = Room::test_room(None);
    let g = graphics(&room);
    let got: Vec<Option<&str>> = (0..5).map(|k| g.entrance_element(k)).collect();
    assert_eq!(
        got,
        [
            None,
            None,
            Some("ShortcutArrow"),
            Some("ShortcutArrow"),
            Some("ShortcutDots"),
        ]
    );
}

/// The room-exit branch of `GenerateSprites`.
#[test]
fn room_exit_symbol_by_what_lies_behind() {
    let shelter = ExitBehind {
        shelter: true,
        ..NOT_SHELTER
    };
    let exit = (8, 3);
    // Behind: a shelter, a broken one (hidden), an ancient one.
    assert_eq!(
        exit_symbol(Some(shelter), 3, &[], false, false),
        Some("ShortcutShelter")
    );
    let broken = ExitBehind {
        broken: true,
        ..shelter
    };
    assert_eq!(exit_symbol(Some(broken), 3, &[], false, false), None);
    let ancient = ExitBehind {
        ancient: true,
        ..shelter
    };
    assert_eq!(
        exit_symbol(Some(ancient), 3, &[], false, false),
        Some("ShortcutAShelter")
    );
    // A connection to nowhere is hidden, except in a gate.
    assert_eq!(exit_symbol(None, -1, &[], false, false), None);
    assert_eq!(
        exit_symbol(None, -1, &[], true, false),
        Some("ShortcutDots")
    );
    // `ExitSymbolShelter` on the entrance toggles: a shelter symbol where none leads to a
    // shelter, dots where one does, a shelter in a gate room either way.
    let symbol = [(PlacedObjectType::ExitSymbolShelter, exit)];
    assert_eq!(
        exit_symbol(Some(NOT_SHELTER), 3, &symbol, false, false),
        Some("ShortcutShelter")
    );
    assert_eq!(
        exit_symbol(Some(shelter), 3, &symbol, false, false),
        Some("ShortcutDots")
    );
    assert_eq!(
        exit_symbol(Some(shelter), 3, &symbol, true, false),
        Some("ShortcutShelter")
    );
    // Only on its own tile; other objects there are passed over.
    let elsewhere = [(PlacedObjectType::ExitSymbolShelter, (8, 4))];
    assert_eq!(
        exit_symbol(Some(NOT_SHELTER), 3, &elsewhere, false, false),
        Some("ShortcutDots")
    );
    let hidden = [
        (PlacedObjectType::DataPearl, exit),
        (PlacedObjectType::ExitSymbolHidden, exit),
        (PlacedObjectType::ExitSymbolShelter, exit),
    ];
    assert_eq!(exit_symbol(Some(shelter), 3, &hidden, false, false), None);
    // A single-room world reads only the objects, not the connections.
    assert_eq!(
        exit_symbol(None, -1, &[], false, true),
        Some("ShortcutDots")
    );
    let ancient_symbol = [(PlacedObjectType::ExitSymbolAncientShelter, exit)];
    assert_eq!(
        exit_symbol(None, -1, &ancient_symbol, false, true),
        Some("ShortcutAShelter")
    );
}

/// `ShortcutGraphics.GenerateSprites`: the symbol turned to point into the hole
/// (`AimFromOneVectorToAnother(0, -hole)`), 15 px out from the entrance's middle toward
/// the hole, drawn at depth -5 (`ApplyDepth`, `ShortcutGraphics.Draw`).
#[test]
fn symbol_place_and_rotation() {
    let room = Room::test_room(None);
    let mut g = graphics(&room);
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[]);
    let symbols: Vec<&Sprite> = list
        .items
        .iter()
        .filter_map(|i| match i {
            DrawItem::Sprite(s) if s.element == "ShortcutArrow" => Some(s),
            _ => None,
        })
        .collect();
    // (1, 3): hole right, so the arrow points left (-90 degrees); 15 px right of (30, 70).
    let dp = Vec2::new(700.0, 533.3334);
    let at = (Vec2::new(45.0, 70.0) - dp) / (1.0 - 5.0 / 400.0) + dp;
    assert_eq!(symbols[0].rotation, -90.0);
    assert!(symbols[0].pos.distance(at) < 1e-3, "{:?}", symbols[0].pos);
    // (4, 3): hole left, pointing right.
    assert_eq!(symbols[1].rotation, 90.0);
    assert!(symbols.iter().all(|s| s.layer == Layer::Shortcuts));
}

/// `GenerateSprites`: a `pixel` per pipe tile: 20 px on an entrance in
/// `Background`; 15 px `ShortCut0` in `Shortcuts` on solid pipe, in `Items` below a
/// water line in front of the terrain, in `BackgroundShortcuts` on air. Room exit and
/// den tiles (`shortCut` 2 and 3) get none.
#[test]
fn pipe_sprites_and_containers() {
    let room = Room::test_room(None);
    let mut g = graphics(&room);
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[]);
    let entrance = pixel_at(&list, (1, 3)).expect("entrance pixel");
    assert_eq!(
        (entrance.layer, entrance.shader, entrance.scale.x),
        (Layer::Background, Shader::Basic, 20.0)
    );
    let pipe = pixel_at(&list, (2, 1)).expect("pipe pixel");
    assert_eq!(
        (pipe.layer, pipe.shader, pipe.scale.x),
        (Layer::Shortcuts, Shader::ShortCut0, 15.0)
    );
    assert!(pixel_at(&list, (8, 2)).is_none(), "room exit tile");
    assert!(pixel_at(&list, (9, 4)).is_none(), "den tile");
    let pixels = list
        .items
        .iter()
        .filter(|i| matches!(i, DrawItem::Sprite(s) if s.element == "pixel"))
        .count();
    // Five entrances and six solid pipe tiles.
    assert_eq!(pixels, 11);
    // Water at row 3, in front of the terrain: the solid pipe below it goes to `Items`.
    let wet = Room::test_room(Some((3, true)));
    let mut g = graphics(&wet);
    let list = draw(&mut g, &wet, &[[0.0; 4]; 5], &[]);
    assert_eq!(pixel_at(&list, (2, 1)).expect("pipe").layer, Layer::Items);
}

/// `Draw`: a pulse with brightness 1 (`[0] == 1`, so
/// its flicker term is 0) at progress 0.25 (`num = Lerp(0, 2, 0.25) = 0.5`) lights pipe
/// tile `j` of an `n`-step pipe `Clamp01(1 - |j / n - 0.5|)^5 * 0.5 + 0.5`.
#[test]
fn pulse_lights_the_pipe_by_progress() {
    let room = Room::test_room(None);
    let mut g = graphics(&room);
    let mut rows = [[0.0; 4]; 5];
    rows[2] = [1.0, 0.25, 0.0, 0.0];
    // Seed 1's third draw is not under 1/30, so the flicker is the brightness itself.
    let mut probe = UnityRandom::new(1);
    probe.value();
    probe.value();
    assert!(probe.value() >= 1.0 / 30.0);
    let list = draw(&mut g, &room, &rows, &[]);
    let pipe = &room.shortcut_map().shortcuts()[2];
    let n = pipe.length as f32;
    let p = palette();
    for (j, &tile) in pipe.path.iter().enumerate() {
        let l = (1.0 - (j as f32 / n - 0.5).abs()).clamp(0.0, 1.0).powf(5.0) * 0.5 + 0.5;
        // `ColorFromLightness`, above 0.5: middle to lit colour.
        let want = p.colors[1].lerp(p.colors[2], (l - 0.5) / 0.5);
        let got = pixel_at(&list, tile).expect("pixel").color;
        assert!(close(got, want), "tile {j}: {got:?} vs {want:?}");
    }
    // Lightness is per frame: the next frame without a pulse is dark again.
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[]);
    assert!(close(
        pixel_at(&list, (2, 1)).expect("pixel").color,
        p.colors[0]
    ));
}

/// `ShortcutGraphics`: a vessel colours its tile and, with more than one segment, the tiles
/// behind it; on a tile with no pipe sprite (a node) nothing is coloured.
#[test]
fn vessel_lights_its_tiles() {
    let room = Room::test_room(None);
    let mut g = graphics(&room);
    let white = Color::WHITE;
    let vessel = ShortcutVessel {
        pos: (3, 1),
        last_positions: vec![(4, 1)],
        color: white,
        segments: 2,
    };
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], std::slice::from_ref(&vessel));
    assert_eq!(pixel_at(&list, (3, 1)).expect("pixel").color, white);
    assert_eq!(pixel_at(&list, (4, 1)).expect("pixel").color, white);
    assert_eq!(
        pixel_at(&list, (2, 1)).expect("pixel").color,
        palette().colors[0]
    );
    let one = ShortcutVessel {
        segments: 1,
        ..vessel.clone()
    };
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[one]);
    assert_eq!(
        pixel_at(&list, (4, 1)).expect("pixel").color,
        palette().colors[0]
    );
    let on_node = ShortcutVessel {
        pos: (8, 2),
        last_positions: vec![(4, 1)],
        ..vessel
    };
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[on_node]);
    assert_eq!(
        pixel_at(&list, (4, 1)).expect("pixel").color,
        palette().colors[0]
    );
}

/// `ShortcutGraphics.Draw`: an entrance flashing at `[3] = -50` (a slugcat's arrival,
/// `BlinkShortCut(.., 2.5)`) shows its glow at alpha `0.35 + 0.35` and scale
/// `2.5 + 1.6` (50 % 4 < 3) in `Water`, and the symbol colour; at `-51` (51 % 4 == 3)
/// the symbol is dark (`ColorFromLightness(0)`). Not flashing, the symbol goes from
/// the dark pipe colour toward `Lerp(lit, symbol, 0.75)` by `[2]` (`ShortcutGraphics.Draw`).
#[test]
fn entrance_flash_and_symbol_colour() {
    let room = Room::test_room(None);
    let mut g = graphics(&room);
    let p = palette();
    let mut rows = [[0.0; 4]; 5];
    rows[4][3] = -50.0;
    let list = draw(&mut g, &room, &rows, &[]);
    let glow = list
        .items
        .iter()
        .find_map(|i| match i {
            DrawItem::Sprite(s) if s.layer == Layer::Water => Some(*s),
            _ => None,
        })
        .expect("glow");
    assert_eq!(glow.element, "Futile_White");
    assert_eq!(glow.shader, Shader::FlatLight);
    assert!((glow.scale.x - 4.1).abs() < 1e-5, "{}", glow.scale.x);
    assert!((glow.color.a - 0.7).abs() < 1e-5);
    let dots = symbol(&list, "ShortcutDots");
    assert_eq!(dots.color, p.symbol);
    rows[4][3] = -51.0;
    let list = draw(&mut g, &room, &rows, &[]);
    assert_eq!(symbol(&list, "ShortcutDots").color, p.colors[0]);
    rows[4][3] = 0.0;
    rows[4][2] = 1.0;
    let list = draw(&mut g, &room, &rows, &[]);
    assert!(close(
        symbol(&list, "ShortcutDots").color,
        p.colors[2].lerp(p.symbol, 0.75)
    ));
    assert!(!list.items.iter().any(|i| i.layer() == Layer::Water));
}

/// `ColorEntrance` and `Update`: the colour shows in full, then fades
/// 0.025 a tick.
#[test]
fn entrance_colour_fades() {
    let room = Room::test_room(None);
    let mut g = graphics(&room);
    let green = Color::rgb(0.0, 1.0, 0.0);
    g.color_entrance(4, green);
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[]);
    assert_eq!(symbol(&list, "ShortcutDots").color, green);
    for _ in 0..20 {
        g.update();
    }
    let list = draw(&mut g, &room, &[[0.0; 4]; 5], &[]);
    let want = palette().colors[0].lerp(green, 0.5);
    assert!(close(symbol(&list, "ShortcutDots").color, want));
}

fn symbol(list: &DrawList, element: &str) -> Sprite {
    list.items
        .iter()
        .find_map(|i| match i {
            DrawItem::Sprite(s) if s.element == element => Some(*s),
            _ => None,
        })
        .expect("symbol")
}
