//! Room visuals from the installed game (read-only): settings inheritance, camera
//! images, palettes with effect colours and fades, and the noise texture.
//!
//! Images and palettes are checked against `rainworld-re/tools/rw_roomimg.py` (an
//! independent stdlib PNG decoder) when `python3` and the tool are present. Set
//! `RW_DIR` (game install) and `RW_RE_DIR` (the rainworld-re checkout) to override the
//! defaults; tests skip when the files are absent.

use std::path::PathBuf;
use std::process::Command;

use rw_assets::{
    EffectColors, NoiseTexture, PaletteImage, PaletteTexture, PngImage, RoomSettings, RoomVisuals,
    StreamingAssets, UnityResources,
};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const DEFAULT_RE_DIR: &str = "/home/yura/Documents/experiments/rainworld-re";

fn assets() -> Option<StreamingAssets> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if sa.join("palettes").is_dir() {
        Some(StreamingAssets::new(sa))
    } else {
        eprintln!("skipping: no palettes dir under {}", sa.display());
        None
    }
}

/// Runs `rw_roomimg.py pal <png> <rows>` and returns each row as RGB triples.
fn oracle_rows(png: &std::path::Path, rows: &[usize]) -> Option<Vec<Vec<[u8; 3]>>> {
    let tool = std::env::var_os("RW_RE_DIR")
        .map_or_else(|| PathBuf::from(DEFAULT_RE_DIR), PathBuf::from)
        .join("tools/rw_roomimg.py");
    if !tool.is_file() {
        eprintln!("skipping oracle: no {}", tool.display());
        return None;
    }
    let out = Command::new("python3")
        .arg("-I")
        .arg(&tool)
        .arg("pal")
        .arg(png)
        .args(rows.iter().map(ToString::to_string))
        .output()
        .ok()?;
    assert!(out.status.success(), "rw_roomimg.py failed");
    let text = String::from_utf8(out.stdout).expect("utf-8");
    Some(
        text.lines()
            .filter_map(|l| l.split_once(": "))
            .filter(|(k, _)| k.starts_with("row "))
            .map(|(_, v)| {
                v.split(' ')
                    .map(|h| {
                        let n = u32::from_str_radix(h, 16).expect("hex");
                        [(n >> 16) as u8, (n >> 8) as u8, n as u8]
                    })
                    .collect()
            })
            .collect(),
    )
}

#[test]
fn settings_inherit_like_the_game() {
    let Some(sa) = assets() else { return };
    // Arena level, Template: NONE, no region: root defaults for what it leaves out.
    let summit = RoomSettings::load(&sa, "Summit", None, Some("white")).expect("summit");
    assert_eq!(summit.palette, 2);
    assert_eq!((summit.effect_color_a, summit.effect_color_b), (0, 3));
    assert_eq!(summit.clouds, 0.217_391_3);
    assert_eq!(summit.grime, 0.5);
    assert_eq!(summit.fade_palette, None);

    // No Template line: first SU template (Outside) gives Palette 0.
    let b04 = RoomSettings::load(&sa, "SU_B04", Some("SU"), Some("white")).expect("su_b04");
    assert_eq!(b04.palette, 0);
    assert_eq!((b04.effect_color_a, b04.effect_color_b), (9, 7));
    assert_eq!((b04.clouds, b04.grime), (0.282_608_7, 0.293_478_3));
    let fade = b04.fade_palette.expect("su_b04 has a FadePalette");
    assert_eq!(fade.palette, 8);
    assert_eq!(fade.fades, vec![0.076_086_95, 0.076_086_95]);

    // Template: SU_SettingsTemplate_Inside: Grime and effect colours from Inside.
    let a32 = RoomSettings::load(&sa, "SU_A32", Some("SU"), None).expect("su_a32");
    assert_eq!(
        (a32.palette, a32.effect_color_a, a32.effect_color_b),
        (0, 1, 6)
    );
    assert_eq!((a32.clouds, a32.grime), (0.141_304_3, 0.173_913));

    assert_eq!(sa.region_of_room("su_b04").as_deref(), Some("su"));
    assert_eq!(sa.region_of_room("summit"), None);
}

#[test]
fn camera_images_match_the_oracle() {
    let Some(sa) = assets() else { return };
    for (room, suffix) in [("summit", "_1.png"), ("su_b04", "_2.png")] {
        let path = sa.find_room_file(room, suffix).expect("image exists");
        let image = PngImage::load(&path).expect("decodes");
        assert_eq!((image.width(), image.height()), (1400, 800));
        let rows = [0, 1, 37, 400, 799];
        let Some(oracle) = oracle_rows(&path, &rows) else {
            return;
        };
        for (row, want) in rows.iter().zip(&oracle) {
            assert_eq!(want.len(), 1400);
            for (x, w) in want.iter().enumerate() {
                let [r, g, b, a] = image.pixel(x, *row);
                assert_eq!([r, g, b], *w, "{room}{suffix} pixel ({x}, {row})");
                assert_eq!(a, 255);
            }
        }
        // Unity order: the PNG's last row comes first.
        let bytes = image.rgba_bottom_up();
        assert_eq!(bytes[..3], image.pixel(0, 799)[..3]);
    }
}

#[test]
fn palette_with_effect_colours_and_fade() {
    let Some(sa) = assets() else { return };
    let effects = EffectColors::load(&sa).expect("effect colours");
    let mut pal = PaletteImage::load(&sa, 0).expect("palette0");
    let file = PaletteImage::load(&sa, 0).expect("palette0");
    pal.apply_effect_colors(&effects, 9, 7).expect("in range");

    let pal_path = sa.file("palettes/palette0.png");
    let ec_path = sa.file("palettes/effectcolors.png");
    let (Some(rows), Some(ec)) = (
        oracle_rows(&pal_path, &(0..16).collect::<Vec<_>>()),
        oracle_rows(&ec_path, &[0, 1, 2, 3]),
    ) else {
        return;
    };
    for png_row in 0..16 {
        let y = 15 - png_row;
        for x in 0..32 {
            assert_eq!(
                file.texel(x, y)[..3],
                rows[png_row][x],
                "file texel ({x}, {png_row})"
            );
            // Effect A (9): PNG rows 2, 3 <- effectcolors rows 0, 1; dark rows 10, 11 <-
            // rows 2, 3. Effect B (7): PNG rows 4, 5 and 12, 13 likewise.
            let want = match (x, png_row) {
                (30..=31, 2 | 3) => ec[png_row - 2][18 + x - 30],
                (30..=31, 10 | 11) => ec[png_row - 8][18 + x - 30],
                (30..=31, 4 | 5) => ec[png_row - 4][14 + x - 30],
                (30..=31, 12 | 13) => ec[png_row - 10][14 + x - 30],
                _ => rows[png_row][x],
            };
            assert_eq!(pal.texel(x, y)[..3], want, "applied texel ({x}, {png_row})");
        }
    }
    assert!(
        pal.apply_effect_colors(&effects, 22, -1).is_err(),
        "only 22 colours"
    );

    // No fade: the runtime texture is the top half, row r = PNG row 7 - r.
    let plain = PaletteTexture::blend(&pal, None, 0.0);
    for r in 0..8 {
        for x in 0..32 {
            assert_eq!(plain.texel(x, r), pal.texel(x, r + 8));
        }
    }
    // palette0 fog amount: texel (9, PNG row 0).
    assert!((plain.fog_amount() - (1.0 - f32::from(rows[0][9][0]) / 255.0)).abs() < 1e-6);

    // su_b04: palette 0 faded 7.6% toward palette 8, effect colours 9 and 7 in both.
    let visuals = RoomVisuals::load(&sa, "SU_B04", 2, Some("white")).expect("visuals");
    let mut b = PaletteImage::load(&sa, 8).expect("palette8");
    b.apply_effect_colors(&effects, 9, 7).expect("in range");
    let faded = visuals.palette_for_camera(1, 0.0);
    let amount = 0.076_086_95_f32;
    for r in 0..8 {
        for x in 0..32 {
            let (pa, pb) = (pal.texel(x, r + 8), b.texel(x, r + 8));
            for c in 0..3 {
                let fa = f32::from(pa[c]) / 255.0;
                let want = (fa + (f32::from(pb[c]) / 255.0 - fa) * amount) * 255.0;
                let got = f32::from(faded.texel(x, r)[c]);
                assert!(
                    (got - want).abs() <= 0.5 + 1e-3,
                    "faded ({x}, {r}) channel {c}"
                );
            }
        }
    }
    // A camera past the fade list blends 0.
    assert_eq!(visuals.palette_for_camera(5, 0.0), plain);
}

#[test]
fn noise_texture_is_dxt1_copy_of_noise_png() {
    let Some(sa) = assets() else { return };
    let resources = UnityResources::open(sa.data_dir()).expect("resources.assets");
    let noise = NoiseTexture::load(&resources).expect("noise texture");
    let streaming = PngImage::load(&sa.file("palettes/noise.png")).expect("noise.png");
    let png = NoiseTexture::from_image(&streaming).expect("64x64");
    let (max, mean) = noise_diff(&noise, &png, false);
    assert!(
        max <= 21 && mean < 5.0,
        "same image up to DXT1 error: max {max} mean {mean}"
    );
    let (_, flipped_mean) = noise_diff(&noise, &png, true);
    assert!(flipped_mean > 20.0, "orientation matters: {flipped_mean}");
}

/// Max and mean absolute red difference, optionally with `b` flipped vertically.
fn noise_diff(a: &NoiseTexture, b: &NoiseTexture, flip: bool) -> (u32, f64) {
    let d: Vec<u32> = (0..64)
        .flat_map(|y| (0..64).map(move |x| (x, y)))
        .map(|(x, y)| {
            u32::from(
                a.red(x, y)
                    .abs_diff(b.red(x, if flip { 63 - y } else { y })),
            )
        })
        .collect();
    (
        *d.iter().max().unwrap(),
        f64::from(d.iter().sum::<u32>()) / 4096.0,
    )
}
