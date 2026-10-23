//! The mip chains of `Palettes/noise2` and `Atlases/maze` in the installed game's
//! `resources.assets` (set `RW_DIR`; skipped if absent). The expected texels are what the
//! reader decodes from the stored DXT1 levels (the texture's own mips, not generated).

use rw_assets::{PngImage, UnityResources};

fn open() -> Option<UnityResources> {
    let game = std::env::var_os("RW_DIR").map_or_else(
        || std::path::PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        std::path::PathBuf::from,
    );
    match UnityResources::open(game.join("RainWorld_Data")) {
        Ok(resources) => Some(resources),
        Err(_) => {
            eprintln!("skipping: no game under {}", game.display());
            None
        }
    }
}

fn sides(image: &PngImage) -> Vec<usize> {
    std::iter::once(image)
        .chain(image.mips())
        .map(PngImage::width)
        .collect()
}

/// `noise2`: 256 x 256, 9 levels, bilinear, repeat, no mip bias.
#[test]
fn noise2_has_nine_stored_levels() {
    let Some(resources) = open() else { return };
    let (texture, image) = resources.texture_with_mips("Palettes/noise2").unwrap();
    assert_eq!(
        (texture.width, texture.height, texture.mip_count),
        (256, 256, 9)
    );
    assert_eq!(
        (
            texture.filter_mode,
            texture.wrap[..2].to_vec(),
            texture.mip_bias
        ),
        (1, vec![0, 0], 0.0)
    );
    assert_eq!(sides(&image), [256, 128, 64, 32, 16, 8, 4, 2, 1]);
    assert_eq!(image.mips()[0].pixel(0, 0), [189, 117, 211, 255]);
    assert_eq!(image.mips()[7].pixel(0, 0), [194, 194, 194, 255]);
}

/// `maze` (the page the `CoralCircuit` shaders read): 128 x 128, 8 levels, bilinear, repeat.
#[test]
fn maze_has_eight_stored_levels() {
    let Some(resources) = open() else { return };
    let (texture, image) = resources.texture_with_mips("Atlases/maze").unwrap();
    assert_eq!(
        (texture.width, texture.height, texture.mip_count),
        (128, 128, 8)
    );
    assert_eq!(
        (
            texture.filter_mode,
            texture.wrap[..2].to_vec(),
            texture.mip_bias
        ),
        (1, vec![0, 0], 0.0)
    );
    assert_eq!(sides(&image), [128, 64, 32, 16, 8, 4, 2, 1]);
    assert_eq!(image.mips()[0].pixel(0, 0), [49, 52, 49, 255]);
    assert_eq!(image.mips()[6].pixel(0, 0), [162, 163, 162, 255]);
}

/// Every `resources.assets` texture the port loads: path, mip count, filter, wrap (U, V).
const LOADED: [(&str, u32, i32, [i32; 2]); 14] = [
    ("Atlases/rainWorld", 1, 0, [1, 1]),
    ("Atlases/uiSprites", 1, 0, [1, 1]),
    ("Atlases/shelterGate", 9, 0, [0, 0]),
    ("Atlases/Rainbow", 7, 1, [0, 0]),
    ("Atlases/LightMask0", 8, 1, [0, 0]),
    ("Atlases/glyphs", 1, 0, [1, 1]),
    ("Atlases/maze", 8, 1, [0, 0]),
    ("Atlases/fontAtlas", 1, 0, [1, 1]),
    ("Palettes/textGradient", 8, 1, [0, 0]),
    ("Palettes/cityPalette", 1, 0, [1, 1]),
    ("Palettes/noise2", 9, 1, [0, 0]),
    ("Palettes/noise", 7, 1, [0, 0]),
    ("Illustrations/cloudsTexture", 10, 1, [0, 0]),
    ("Illustrations/apartments", 1, 0, [1, 1]),
];

/// Each texture's import settings, and `texture_with_mips` carries every stored level.
#[test]
fn every_loaded_texture_keeps_its_stored_chain() {
    let Some(resources) = open() else { return };
    for (path, mips, filter, wrap) in LOADED {
        let (texture, image) = resources.texture_with_mips(path).expect(path);
        assert_eq!(
            (
                texture.mip_count,
                texture.filter_mode,
                texture.wrap[..2].to_vec()
            ),
            (mips, filter, wrap.to_vec()),
            "{path}"
        );
        assert_eq!(image.mip_level_count(), mips, "{path}");
        assert_eq!(texture.mip_bias, 0.0, "{path}");
        if mips > 1 {
            let last = image.mips().last().unwrap();
            assert_eq!((last.width(), last.height()), (1, 1), "{path}");
        }
    }
}
