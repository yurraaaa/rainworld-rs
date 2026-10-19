//! The debug UI's data (filters, spawn places, creature rows) and one UI frame drawn
//! offscreen (skipped without a Vulkan adapter).

use rw_assets::CreatureType;
use rw_render::{Camera, Color, Renderer, WgpuRenderer};

use super::tabs::SpawnPlace;
use super::view::{CreatureRow, RoomFilter};
use super::*;

fn game_view() -> GameView {
    GameView {
        region: "SU".to_owned(),
        camera_room: "SU_A07".to_owned(),
        player_tile: Some((10, 5)),
        rooms: ["SU_A07", "SU_A41", "SU_C04"].map(str::to_owned).to_vec(),
        creatures: vec![
            CreatureRow {
                kind: CreatureType::GreenLizard,
                id: [-1, 12],
                room: "SU_A07".to_owned(),
                node: -1,
                tile: (10, 5),
                in_den: false,
                realized: true,
                alive: true,
            },
            CreatureRow {
                kind: CreatureType::Fly,
                id: [3, 40],
                room: "SU_C04".to_owned(),
                node: 2,
                tile: (-1, -1),
                in_den: true,
                realized: false,
                alive: true,
            },
        ],
        creature_kinds: vec![CreatureType::GreenLizard, CreatureType::Fly],
    }
}

#[test]
fn room_filter_is_a_case_insensitive_substring() {
    let rooms = ["SU_A07", "SU_A41", "SU_C04"].map(str::to_owned);
    assert_eq!(RoomFilter("a4").apply(&rooms), ["SU_A41"]);
    assert_eq!(RoomFilter(" ").apply(&rooms).len(), 3);
    assert!(RoomFilter("hi_").apply(&rooms).is_empty());
}

#[test]
fn creature_rows_match_kind_room_or_id() {
    let view = game_view();
    let [lizard, fly] = [&view.creatures[0], &view.creatures[1]];
    assert!(lizard.matches("green") && !fly.matches("green"));
    assert!(fly.matches("su_c0") && !lizard.matches("su_c0"));
    assert!(lizard.matches("-1.12") && !fly.matches("-1.12"));
    assert!(lizard.matches("") && fly.matches(""));
}

#[test]
fn spawn_place_defaults_to_the_camera_room_and_spells_rooms_as_the_world() {
    let view = game_view();
    let mut place = SpawnPlace {
        room: String::new(),
        tile: [4, 7],
    };
    assert_eq!(place.resolve(&view), Ok(("SU_A07".to_owned(), (4, 7))));
    place.room = " su_c04 ".to_owned();
    assert_eq!(place.resolve(&view), Ok(("SU_C04".to_owned(), (4, 7))));
    place.room = "HI_A01".to_owned();
    assert!(place.resolve(&view).is_err());
}

/// The one test that makes an imgui context (imgui allows one at a time): a frame with
/// the window open has draws inside the window, and drawn over a black frame it
/// leaves pixels that are not black.
#[test]
fn an_open_frame_draws_the_window() {
    let mut ui = DebugUi::new(None);
    let font = ui.take_font_texture().expect("font atlas");
    assert!(font.width > 0 && font.rgba.len() == (font.width * font.height * 4) as usize);
    let view = DebugView {
        game: Some(game_view()),
    };
    ui.toggle();
    // The first frame lays the window out; the second draws it settled.
    let _ = ui.frame(&view, None, (640, 480), 1.0, 1.0 / 40.0);
    let (overlay, actions) = ui.frame(&view, None, (640, 480), 1.0, 1.0 / 40.0);
    assert!(actions.is_empty());
    assert!(!overlay.is_empty());
    assert!(
        overlay
            .indices
            .iter()
            .all(|&i| (i as usize) < overlay.vertices.len())
    );
    // The window sits at (10, 10), 520 x 440.
    assert!(
        overlay
            .vertices
            .iter()
            .all(|v| v.pos[0] >= 9.0 && v.pos[0] <= 531.0)
    );
    let mut r = match WgpuRenderer::headless((640, 480)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    r.set_overlay_texture(font.width, font.height, &font.rgba);
    r.begin_frame(Camera::window([0.0, 0.0], [640.0, 480.0], (640, 480)));
    r.set_background(Color::rgb(0.0, 0.0, 0.0));
    r.draw_overlay(overlay);
    let image = r.render_image().expect("frame");
    let lit = image
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[..3] != [0, 0, 0])
        .map(|(i, _)| (i % 640, i / 640))
        .collect::<Vec<_>>();
    assert!(lit.len() > 10_000, "{} pixels lit", lit.len());
    assert!(
        lit.iter()
            .all(|&(x, y)| (9..=531).contains(&x) && (9..=451).contains(&y))
    );
}
