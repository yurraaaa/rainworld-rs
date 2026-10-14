//! Fonts, label layout and strings from the installed game (read-only). Set `RW_DIR`
//! to override the default install; tests skip when it is absent.
//!
//! The font check compares against the raw BMFont text, re-read here with a separate
//! minimal reader, and the atlas frame from the TexturePacker JSON. The label checks
//! compare against positions worked out by hand from the BMFont numbers following
//! `FFont.GetQuadInfoForText` and `FLabel.UpdateLocalPosition`.

use std::collections::HashMap;
use std::path::PathBuf;

use rw_assets::{StreamingAssets, UnityResources};
use rw_menu::{Alignment, AtlasManager, Font, Label, Rect, Translator};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed {
    assets: StreamingAssets,
    resources: UnityResources,
}

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let data = root.join("RainWorld_Data");
        if !data.join("resources.assets").is_file() {
            eprintln!("skipping: no game under {}", root.display());
            return None;
        }
        Some(Self {
            assets: StreamingAssets::new(data.join("StreamingAssets")),
            resources: UnityResources::open(&data).expect("resources"),
        })
    }

    fn atlases(&self) -> AtlasManager {
        let mut atlases = AtlasManager::new();
        atlases
            .load_english_fonts(&self.assets, &self.resources)
            .expect("fonts load");
        atlases
    }

    fn raw_text(&self, path: &str) -> String {
        String::from_utf8(self.resources.text(path).expect("text asset")).expect("utf-8")
    }
}

/// `key=value` pairs of every line starting with `kind `, read independently of the
/// port's parser.
fn raw_records(text: &str, kind: &str) -> Vec<HashMap<String, i64>> {
    text.lines()
        .filter(|l| l.starts_with(&format!("{kind} ")))
        .map(|l| {
            l.split_whitespace()
                .filter_map(|t| t.split_once('='))
                .filter_map(|(k, v)| Some((k.to_owned(), v.parse().ok()?)))
                .collect()
        })
        .collect()
}

/// The `frame` of `name` in the fontAtlas JSON, as (x, y, w, h), and the image size.
fn json_frame(json: &str, name: &str) -> ([f32; 4], [f32; 2]) {
    let v: serde_json::Value = serde_json::from_str(json).expect("atlas json");
    let f = &v["frames"][name]["frame"];
    let s = &v["meta"]["size"];
    (
        [num(&f["x"]), num(&f["y"]), num(&f["w"]), num(&f["h"])],
        [num(&s["w"]), num(&s["h"])],
    )
}

fn num(v: &serde_json::Value) -> f32 {
    v.as_f64().expect("number") as f32
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn check_font(game: &Installed, atlases: &AtlasManager, name: &str, config: &str, png: &str) {
    let font = atlases.font(name).expect("font loaded");
    let text = game.raw_text(config);
    let json = game.raw_text("Atlases/fontAtlas");
    let (frame, size) = json_frame(&json, png);
    let chars = raw_records(&text, "char");
    for c in &chars {
        let id = c["id"] as u32;
        let info = font.char_info(id).expect("char present");
        let (x, y, w, h) = (
            c["x"] as f32,
            c["y"] as f32,
            c["width"] as f32,
            c["height"] as f32,
        );
        assert_eq!(
            [info.x, info.y, info.width, info.height],
            [x, y, w, h],
            "char {id}"
        );
        // The space's offsets are zeroed by `LoadAndParseConfigFile`.
        let (ox, oy) = if id == 32 {
            (0.0, 0.0)
        } else {
            (c["xoffset"] as f32, c["yoffset"] as f32)
        };
        assert_eq!(
            [info.offset_x, info.offset_y],
            [ox, oy],
            "char {id} offsets"
        );
        assert_eq!(info.xadvance, c["xadvance"] as f32, "char {id} xadvance");
        // Texture coordinates: the glyph's pixels inside the element's frame, v from
        // the bottom of the atlas image.
        let want = Rect::new(
            (frame[0] + x) / size[0],
            (size[1] - (frame[1] + y) - h) / size[1],
            w / size[0],
            h / size[1],
        );
        let r = info.uv_rect;
        assert!(
            close(r.x, want.x)
                && close(r.y, want.y)
                && close(r.width, want.width)
                && close(r.height, want.height),
            "char {id} uv {r:?} want {want:?}"
        );
    }
    let kernings = raw_records(&text, "kerning");
    let mut pairs = HashMap::new();
    for k in &kernings {
        pairs.insert((k["first"], k["second"]), k["amount"]);
    }
    for (&(first, second), &amount) in &pairs {
        assert_eq!(
            font.kerning(first as i32, second as i32),
            Some(amount as f32),
            "kerning {first} {second}"
        );
    }
    assert_eq!(font.kerning_count(), pairs.len());
    eprintln!(
        "{name}: {} chars, {} kerning lines ({} pairs), line height {}",
        chars.len(),
        kernings.len(),
        pairs.len(),
        font.line_height()
    );
}

#[test]
fn fonts_match_their_bmfont_configs() {
    let Some(game) = Installed::open() else {
        return;
    };
    let atlases = game.atlases();
    check_font(&game, &atlases, Font::FONT, "Atlases/font", "font.png");
    check_font(
        &game,
        &atlases,
        Font::DISPLAY_FONT,
        "Atlases/DisplayFont",
        "DisplayFont.png",
    );
    let font = atlases.font(Font::FONT).unwrap();
    let display = atlases.font(Font::DISPLAY_FONT).unwrap();
    assert_eq!(font.line_height(), 15.0);
    assert_eq!(display.line_height(), 30.0);
    // 329 chars + the empty one + 4 PlayStation glyphs from ps4Glyphs.
    assert_eq!(font.char_count(), 334);
    assert_eq!(display.char_count(), 329);
}

#[test]
fn ps4_glyphs_are_combined_into_font_at_half_scale() {
    let Some(game) = Installed::open() else {
        return;
    };
    let atlases = game.atlases();
    let font = atlases.font(Font::FONT).unwrap();
    let raw = raw_records(&game.raw_text("Atlases/ps4Glyphs"), "char");
    for c in raw.iter().filter(|c| c["id"] != 32) {
        let info = font.char_info(c["id"] as u32).expect("glyph added");
        assert_eq!(info.width, c["width"] as f32 * 0.5);
        assert_eq!(info.xadvance, c["xadvance"] as f32 * 0.5);
        // Both line heights are 15, so only the half-scaled yoffset remains.
        assert_eq!(info.offset_y, c["yoffset"] as f32 * 0.5);
        assert_eq!(info.specific_line_height, 15.0);
    }
    // The base font's space stays.
    assert_eq!(font.char_info(32).unwrap().xadvance, 3.0);
}

/// The label's quads as (TL, BR) pairs.
fn corners(label: &Label) -> Vec<([f32; 2], [f32; 2])> {
    label
        .lines()
        .iter()
        .flat_map(|l| &l.quads)
        .map(|q| (q.corners[0], q.corners[2]))
        .collect()
}

#[test]
fn label_layout_matches_hand_computed_positions() {
    let Some(game) = Installed::open() else {
        return;
    };
    let atlases = game.atlases();
    let font = atlases.font(Font::FONT).unwrap().clone();
    let display = atlases.font(Font::DISPLAY_FONT).unwrap().clone();

    // font: A (w 7, h 9, yoffset 3, xadvance 8), V (w 7, h 9, yoffset 3), kerning
    // A V = -1, line height 15. A at x 0, V at 0 + 8 - 1 = 7; line bounds (0, -15, 14,
    // 15). Centred: dy = -(-15 + 15 * 0.5) = 7.5, dx = -14 * 0.5 = -7.
    let mut av = Label::new(font.clone(), "AV");
    assert_eq!(
        corners(&av),
        [([-7.0, 4.5], [0.0, -4.5]), ([0.0, 4.5], [7.0, -4.5])]
    );
    assert_eq!(av.text_rect(), Rect::new(-7.0, -7.5, 14.0, 15.0));
    // Right aligned: dx = -14.
    av.set_alignment(Alignment::Right);
    assert_eq!(
        corners(&av),
        [([-14.0, 4.5], [-7.0, -4.5]), ([-7.0, 4.5], [0.0, -4.5])]
    );

    // Two lines, left aligned: T (w 6, h 9, yoffset 3, xadvance 7), o (w 6, h 6,
    // yoffset 6), kerning T o = -1: o at 7 - 1 = 6. The second line starts 15 lower;
    // A at x 0, rect y = -15 - 3 - 9 = -27. Bounds y -30..0, so dy = 15.
    let mut two = Label::new(font.clone(), "To\nA");
    two.set_alignment(Alignment::Left);
    assert_eq!(
        corners(&two),
        [
            ([0.0, 12.0], [6.0, 3.0]),
            ([6.0, 9.0], [12.0, 3.0]),
            ([0.0, -3.0], [7.0, -12.0]),
        ]
    );
    assert_eq!(two.text_rect(), Rect::new(0.0, -15.0, 12.0, 30.0));

    // Centred lines are centred one by one: "To" is 12 wide (dx -6), "A" 7 (dx -3.5).
    let centred = Label::new(font.clone(), "To\nA");
    assert_eq!(corners(&centred)[0], ([-6.0, 12.0], [0.0, 3.0]));
    assert_eq!(corners(&centred)[2], ([-3.5, -3.0], [3.5, -12.0]));

    // DisplayFont: T (w 12, h 15, yoffset 9, xadvance 12), A (w 14, h 15, yoffset 9),
    // kerning T A = -2: A at 10, line 24 wide, line height 30, so dy = 15, dx = -12.
    let ta = Label::new(display, "TA");
    assert_eq!(
        corners(&ta),
        [([-12.0, 6.0], [0.0, -9.0]), ([-2.0, 6.0], [12.0, -9.0])]
    );

    // The space's xoffset (-1) and yoffset (14) are zeroed, so its 3x1 quad sits at
    // the top of the line; A follows at the space's xadvance, 3. Anchored top-left
    // (dy = 0).
    let mut spaced = Label::new(font, " A");
    spaced.set_anchor([0.0, 1.0]);
    assert_eq!(
        corners(&spaced),
        [([0.0, 0.0], [3.0, -1.0]), ([3.0, -3.0], [10.0, -12.0])]
    );
}

#[test]
fn english_strings() {
    let Some(game) = Installed::open() else {
        return;
    };
    let translator = Translator::load(&game.assets).expect("strings.txt");
    let strings = translator.strings();
    eprintln!(
        "strings.txt: {} entries, {} lines rejected, {} keys replaced",
        strings.entries.len(),
        strings.rejected,
        strings.replaced
    );
    assert_eq!(strings.entries.len(), 1351);
    // The empty string after the final CRLF.
    assert_eq!(strings.rejected, 1);
    assert_eq!(translator.translate("Loading..."), "Loading...");
    assert_eq!(translator.translate("creaturetype-BigEel"), "Leviathan");
    assert_eq!(translator.translate("not a key"), "not a key");
}
