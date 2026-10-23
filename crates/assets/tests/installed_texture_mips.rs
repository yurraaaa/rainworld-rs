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
