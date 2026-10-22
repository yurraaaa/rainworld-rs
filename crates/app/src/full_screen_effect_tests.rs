use rw_assets::{RoomEffect, StreamingAssets};

use super::*;
use crate::GameDir;

fn settings(effects: &[(RoomEffectType, f32)]) -> RoomSettings {
    RoomSettings {
        effects: effects
            .iter()
            .map(|&(kind, amount)| RoomEffect {
                kind: kind.value().to_owned(),
                amount,
            })
            .collect(),
        ..RoomSettings::ANCESTOR
    }
}

fn selected(effects: &[(RoomEffectType, f32)]) -> Option<(RoomEffectType, Shader, Layer)> {
    FullScreenEffect::select(&settings(effects)).map(|e| (e.effect, e.shader, e.layer))
}

/// `ApplyPalette`'s else-if chain: the first effect above 0 wins, whatever order the
/// settings list them in; amount 0 counts as absent.
#[test]
fn first_match_wins_in_the_games_order() {
    use RoomEffectType as E;
    assert_eq!(selected(&[]), None);
    assert_eq!(
        selected(&[(E::Bloom, 0.5), (E::SkyBloom, 0.2)]),
        Some((E::SkyBloom, Shader::SkyBloom, Layer::Bloom))
    );
    assert_eq!(
        selected(&[(E::Fog, 0.4), (E::SkyAndLightBloom, 0.1)]),
        Some((E::SkyAndLightBloom, Shader::LightAndSkyBloom, Layer::Bloom))
    );
    assert_eq!(
        selected(&[(E::Fog, 0.5), (E::LightBurn, 0.3)]),
        Some((E::LightBurn, Shader::LightBloom, Layer::Bloom))
    );
    // `Lightning` takes the `LightBloom` branch and is the alpha effect even next to
    // `LightBurn`.
    assert_eq!(
        selected(&[(E::LightBurn, 0.3), (E::Lightning, 0.5)]),
        Some((E::Lightning, Shader::LightBloom, Layer::Bloom))
    );
    assert_eq!(
        selected(&[(E::Bloom, 0.5), (E::Fog, 0.4)]),
        Some((E::Fog, Shader::Fog, Layer::Foreground))
    );
    assert_eq!(
        selected(&[(E::Bloom, 0.5), (E::VoidMelt, 0.4)]),
        Some((E::Bloom, Shader::Bloom, Layer::Bloom))
    );
    assert_eq!(selected(&[(E::Bloom, 0.0), (E::Fog, 0.0)]), None);
    // `VoidMelt` (`LevelMelt2`) is not ported yet.
    assert_eq!(selected(&[(E::VoidMelt, 0.6)]), None);
}

/// `DrawUpdate`: the bloom shaders fade with `num4` (`InverseLerp(-0.7, 0, num4)`),
/// `Fog` keeps its amount, `Lightning` uses `InverseLerp(0, 0.2, amount)`.
#[test]
fn alpha_per_frame() {
    use RoomEffectType as E;
    let bloom = settings(&[(E::Bloom, 0.8)]);
    let effect = FullScreenEffect::select(&bloom).unwrap();
    assert_eq!(effect.alpha(&bloom, 1.0), 0.8);
    assert_eq!(effect.alpha(&bloom, 0.0), 0.8);
    assert!((effect.alpha(&bloom, -0.35) - 0.4).abs() < 1e-6);
    assert_eq!(effect.alpha(&bloom, -0.7), 0.0);
    assert_eq!(effect.alpha(&bloom, -1.0), 0.0);

    let fog = settings(&[(E::Fog, 0.43)]);
    let effect = FullScreenEffect::select(&fog).unwrap();
    assert_eq!(effect.alpha(&fog, -1.0), 0.43);

    let burn = settings(&[(E::LightBurn, 0.64), (E::Fog, 0.5)]);
    let effect = FullScreenEffect::select(&burn).unwrap();
    assert!((effect.alpha(&burn, -0.175) - 0.48).abs() < 1e-6);

    let lightning = settings(&[(E::Lightning, 0.1)]);
    let effect = FullScreenEffect::select(&lightning).unwrap();
    assert!((effect.alpha(&lightning, 1.0) - 0.5).abs() < 1e-6);
    assert!((effect.alpha(&lightning, -0.35) - 0.25).abs() < 1e-6);
    let strong = settings(&[(E::Lightning, 0.85)]);
    assert_eq!(effect.alpha(&strong, 1.0), 1.0);
}

/// `SetUpFullScreenEffect`: `Futile_White` (16 px) scaled `ScreenSize.x / 16` by 48,
/// anchored bottom-left at the stage origin, white with the alpha (clamped as
/// `FNode.alpha` is).
#[test]
fn sprite_covers_the_screen() {
    let effect = FullScreenEffect::select(&settings(&[(RoomEffectType::Fog, 0.5)])).unwrap();
    let list = effect.draw_list(1366.0, Vec2::new(100.0, 50.0), 1.4);
    let [DrawItem::Sprite(s)] = &list.items[..] else {
        panic!("one sprite")
    };
    assert_eq!(s.element, SpriteAtlas::FUTILE_WHITE);
    assert_eq!((s.pos, s.anchor), (Vec2::new(100.0, 50.0), Vec2::ZERO));
    assert_eq!(s.scale, Vec2::new(1366.0 / 16.0, 48.0));
    assert_eq!((s.color.r, s.color.a), (1.0, 1.0));
    assert_eq!((s.shader, s.layer), (Shader::Fog, Layer::Foreground));
}

/// `_waterLevel = InverseLerp(sSize.y, 0, line - targetPoint.y)`: a water line 400 px
/// above the window's bottom is 0.479 of the screen from its top; below the window
/// (or no line: 0) clamps to 1; gates and shelters set 0.
#[test]
fn water_level_from_the_water_line() {
    use crate::visuals::Visuals;
    assert!((Visuals::water_level(Some(500.0), 100.0, 768.0) - 368.0 / 768.0).abs() < 1e-6);
    assert_eq!(Visuals::water_level(Some(0.0), 100.0, 768.0), 1.0);
    assert_eq!(Visuals::water_level(Some(2000.0), 100.0, 768.0), 0.0);
    assert_eq!(Visuals::water_level(None, 100.0, 768.0), 0.0);
}

/// The rooms with `LightBurn` and `Fog` both above 0 draw `LightBloom`; their fog
/// never draws (installed game's settings, templates inherited).
#[test]
fn light_burn_beats_fog_in_the_three_rooms() {
    let dir = GameDir::from_env().streaming_assets();
    if !dir.is_dir() {
        eprintln!("skipping: no installed game");
        return;
    }
    let assets = StreamingAssets::new(dir);
    for (room, region) in [("SH_A05", "sh"), ("SH_E05", "sh"), ("SB_J02", "sb")] {
        let s = RoomSettings::load(&assets, room, Some(region), None).unwrap();
        assert!(s.effect_amount(RoomEffectType::Fog) > 0.0, "{room} has fog");
        let effect = FullScreenEffect::select(&s).unwrap();
        assert_eq!(
            (effect.effect, effect.shader),
            (RoomEffectType::LightBurn, Shader::LightBloom),
            "{room}"
        );
    }
}
