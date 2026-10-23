
use super::*;

/// `BodyChunk.VisibilityBonus` (`BodyChunk.cs` 162-165) for a green lizard's
/// `movementBasedVision` 0.7: a still chunk of a body without its own bonus is -0.49,
/// one moving 10 px a tick 0.7, and the owner's bonus counts at 0.65.
#[test]
fn chunk_visibility_bonus() {
    let still = ChunkVisibility {
        pos: Vec2::new(100.0, 100.0),
        last_pos: Vec2::new(100.0, 100.0),
        owner_bonus: 0.0,
    };
    assert!((still.bonus(0.7) + 0.49).abs() < 1e-6);
    let fast = ChunkVisibility {
        last_pos: Vec2::new(90.0, 100.0),
        ..still
    };
    assert!((fast.bonus(0.7) - 0.7).abs() < 1e-6);
    let survivor_still = ChunkVisibility {
        owner_bonus: 0.2,
        ..still
    };
    assert!((survivor_still.bonus(0.7) - (-0.49 + 0.2 * 0.65)).abs() < 1e-6);
}

/// `Player.VisibilityBonus` and `Sneak` (`Player.cs` 2162, 2208-2233; the Survivor's
/// stats are 0 and 0.5, the Monk's -0.1 and 0.6, `SlugcatStats.cs` 87, 189-190).
#[test]
fn player_visibility() {
    let survivor = PlayerVisibility {
        general: 0.0,
        stealth_in_sneak_mode: 0.5,
        priv_sneak: 0.0,
        submersion: 0.0,
    };
    assert_eq!(survivor.bonus(), 0.0);
    let crawling = PlayerVisibility {
        priv_sneak: 1.0,
        ..survivor
    };
    assert_eq!(crawling.sneak(), 1.0);
    assert_eq!(crawling.bonus(), -0.5);
    let half = PlayerVisibility {
        priv_sneak: 0.125,
        ..survivor
    };
    assert!((half.sneak() - 0.5).abs() < 1e-6);
    let monk_swimming = PlayerVisibility {
        general: -0.1,
        stealth_in_sneak_mode: 0.6,
        priv_sneak: 0.0,
        submersion: 1.0,
    };
    assert!((monk_swimming.bonus() - (-0.1 - 0.3)).abs() < 1e-6);
}
