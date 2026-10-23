//! The background scenes' images in the installed game (read-only): every name the
//! scenes load (`LoadGraphic`) is a file in `Illustrations` and no element of the
//! `rainWorld`, `uiSprites` or `shelterGate` atlases has the same name (the sprite
//! lookup takes the first page holding a name). Set `RW_DIR` to the game install
//! directory; skipped if absent.

use std::path::PathBuf;

use rw_assets::{StreamingAssets, UnityResources};
use rw_graphics::Atlas;
use rw_random::UnityRandom;
use rw_sim::{AboveCloudsContext, BackgroundScene, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn install() -> Option<PathBuf> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    if root
        .join("RainWorld_Data")
        .join("resources.assets")
        .is_file()
    {
        Some(root.join("RainWorld_Data"))
    } else {
        eprintln!("skipping: no game under {}", root.display());
        None
    }
}

fn clouds(region: &'static str, room: &'static str) -> BackgroundScene {
    BackgroundScene::above_clouds_view(&AboveCloudsContext {
        region: Some(region),
        room_name: room,
        room_origin: Vec2::ZERO,
        arena: false,
        effect_amount: 1.0,
        clouds: (None, None),
    })
}

fn scene_names() -> Vec<&'static str> {
    let roof = BackgroundScene::roof_top_view(Vec2::ZERO, (100, 50), &mut UnityRandom::new(1));
    let mut names = Vec::new();
    for scene in [
        clouds("UW", "UW_A01"),
        clouds("SI", "SI_A06"),
        clouds("SI", "SI_A07"),
        roof,
    ] {
        for name in scene.image_names() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

#[test]
fn scene_images_are_files_and_clash_with_no_atlas_element() {
    let Some(data) = install() else { return };
    let assets = StreamingAssets::new(data.join("StreamingAssets"));
    let resources = UnityResources::open(&data).expect("resources.assets");
    let atlases: Vec<Atlas> = ["rainWorld", "uiSprites", "shelterGate"]
        .iter()
        .map(|n| Atlas::from_resources(&resources, n).expect("atlas"))
        .collect();
    let names = scene_names();
    // AboveCloudsView: 3 skies, 6 structures, 9 spires, 3 lights, 3 clouds, the flying
    // clouds (25); RoofTopView: 3 skies, floor, rubble, hole fix, city1, city2, three
    // city buildings, smoke (12).
    assert_eq!(names.len(), 37, "{names:?}");
    for name in &names {
        let path = assets.resolve_file_path(&format!("Illustrations/{name}.png"));
        assert!(path.is_file(), "no file for {name}: {}", path.display());
        for atlas in &atlases {
            assert!(atlas.get(name).is_none(), "{name} is also an atlas element");
        }
    }
    eprintln!("{} scene images", names.len());
}
