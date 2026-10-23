//! The import settings that choose atlas page sampling, read from the installed game
//! (set `RW_DIR`; skipped if absent).

use rw_render::TextureSampling;

/// `LightMask0` is filter 1 (bilinear), repeat, 8 mips; `rainWorld` point and clamped.
#[test]
fn installed_atlas_pages_import_settings() {
    let game = std::env::var_os("RW_DIR").map_or_else(
        || std::path::PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        std::path::PathBuf::from,
    );
    let Ok(resources) = rw_assets::UnityResources::open(game.join("RainWorld_Data")) else {
        eprintln!("skipping: no game under {}", game.display());
        return;
    };
    let (mask, _) = resources
        .texture_with_settings("Atlases/LightMask0")
        .unwrap();
    assert_eq!((mask.filter_mode, mask.wrap[..2].to_vec()), (1, vec![0, 0]));
    assert_eq!(mask.mip_count, 8);
    let (main, _) = resources
        .texture_with_settings("Atlases/rainWorld")
        .unwrap();
    assert_eq!(
        TextureSampling::from_import(main.filter_mode, [main.wrap[0], main.wrap[1]]),
        TextureSampling::POINT_CLAMP
    );
}
