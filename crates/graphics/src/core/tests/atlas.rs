//! Futile atlas frames.

use crate::{Atlas, elements};

#[test]
fn atlas_json_parses_frames() {
    // Sizes from sprite-atlases.md section 4; positions are made up.
    let json = r#"{"frames": {
        "BodyA.png": {"frame": {"x":1,"y":2,"w":14,"h":19}, "rotated": false, "trimmed": false,
            "spriteSourceSize": {"x":0,"y":0,"w":14,"h":19}, "sourceSize": {"w":14,"h":19}},
        "FaceA0.png": {"frame": {"x":20,"y":2,"w":12,"h":7}, "rotated": false, "trimmed": true,
            "spriteSourceSize": {"x":2,"y":3,"w":12,"h":7}, "sourceSize": {"w":16,"h":18}},
        "PlayerArm0.png": {"frame": {"x":40,"y":2,"w":5,"h":5}, "rotated": false, "trimmed": true,
            "spriteSourceSize": {"x":23,"y":0,"w":5,"h":5}, "sourceSize": {"w":28,"h":8}}
    },
    "meta": {"image": "rainWorld.png", "size": {"w":464,"h":512}, "scale": "1"}}"#;
    let atlas = Atlas::parse(json).unwrap();
    assert_eq!(atlas.frames.len(), 3);
    assert_eq!((atlas.size.w, atlas.size.h), (464, 512));
    let body = atlas.get("BodyA").unwrap();
    assert_eq!((body.frame.w, body.frame.h, body.trimmed), (14, 19, false));
    let face = atlas.get("FaceA0").unwrap();
    assert_eq!((face.source_size.w, face.source_size.h), (16, 18));
    assert_eq!(
        (face.sprite_source_size.x, face.sprite_source_size.y),
        (2, 3)
    );
    let arm = atlas.get("PlayerArm0").unwrap();
    assert_eq!(arm.sprite_source_size.x, 23);
    let rotated = json.replacen("\"rotated\": false", "\"rotated\": true", 1);
    assert!(Atlas::parse(&rotated).is_err());
}

#[test]
fn installed_atlas_has_the_slugcat_frames() {
    // Reads the rainWorld atlas from the installed game (`RW_DIR`) when present.
    let game = std::env::var_os("RW_DIR").map_or_else(
        || std::path::PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        std::path::PathBuf::from,
    );
    let Ok(resources) = rw_assets::UnityResources::open(game.join("RainWorld_Data")) else {
        eprintln!("skipped: no game under {}", game.display());
        return;
    };
    let atlas = Atlas::from_resources(&resources, "rainWorld").unwrap();
    // sprite-atlases.md: 620 frames, 464 x 512.
    assert_eq!(atlas.frames.len(), 620);
    assert_eq!((atlas.size.w, atlas.size.h), (464, 512));
    // (frame w, h, source w, h)
    for (name, expected) in [
        ("BodyA", (14, 19, 14, 19)),
        ("HipsA", (14, 20, 14, 20)),
        ("HeadA15", (24, 15, 24, 15)),
        ("FaceA0", (12, 7, 16, 18)),
        ("FaceStunned", (12, 6, 16, 18)),
        ("LegsAVerticalPole", (7, 10, 7, 10)),
        ("OnTopOfTerrainHand2", (4, 3, 4, 3)),
    ] {
        let f = atlas.get(name).unwrap_or_else(|| panic!("{name} missing"));
        let got = (f.frame.w, f.frame.h, f.source_size.w, f.source_size.h);
        assert_eq!(got, expected, "{name}");
    }
    assert_eq!(atlas.get("PlayerArm0").unwrap().sprite_source_size.x, 23);
    // Every element name the slugcat code can request is present.
    let names = elements::HEAD_A
        .iter()
        .chain(&elements::FACE_A)
        .chain(&elements::FACE_B)
        .chain(&elements::LEGS_A)
        .chain(&elements::LEGS_A_CRAWLING)
        .chain(&elements::LEGS_A_ON_POLE)
        .chain(&elements::LEGS_A_CLIMBING)
        .chain(&elements::PLAYER_ARM)
        .chain(&[
            elements::BODY_A,
            elements::HIPS_A,
            elements::FACE_DEAD,
            elements::FACE_STUNNED,
            elements::LEGS_A_AIR,
            elements::LEGS_A_WALL,
            elements::ON_TOP_OF_TERRAIN_HAND,
            elements::ON_TOP_OF_TERRAIN_HAND_2,
        ]);
    for name in names {
        assert!(atlas.get(name).is_some(), "{name} missing");
    }
    // The lizard cosmetics' elements, and `graphicHeight` as the constructors read it.
    for (i, name) in elements::LIZARD_SCALE_A.iter().enumerate() {
        let f = atlas.get(name).unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(
            f.source_size.h as f32,
            elements::LIZARD_SCALE_A_HEIGHT[i],
            "{name}"
        );
    }
    for name in elements::LIZARD_SCALE_B
        .iter()
        .chain(&[elements::CIRCLE_20, elements::PIXEL])
    {
        assert!(atlas.get(name).is_some(), "{name} missing");
    }
}
