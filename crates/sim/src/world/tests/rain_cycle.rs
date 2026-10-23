use super::*;
use crate::world::DeathRainMode;

impl RainCycle {
    /// A 4800-tick cycle at `timer`.
    fn test(story_mode: bool, start_up_ticks: i32, timer: i32) -> Self {
        Self {
            cycle_length: 4800,
            base_cycle_length: 4800,
            timer,
            story_mode,
            start_up_ticks,
            ..Self::default()
        }
    }
}

trait Close {
    fn close(self, to: f32) -> bool;
}

impl Close for f32 {
    fn close(self, to: f32) -> bool {
        (self - to).abs() < 1e-5
    }
}

#[test]
fn story_shader_light() {
    // Cycle start without a start-up: -1 + lerp(1, 1 - 1, 0.2) * 2 = 0.6.
    assert!(RainCycle::test(true, 0, 0).shader_light().close(0.6));
    // Mid-cycle: full light.
    assert!(RainCycle::test(true, 0, 2400).shader_light().close(1.0));
    // A quarter into the start-up, 600 ticks in: startUp 0.25, proximity 0.75.
    assert!(RainCycle::test(true, 2400, 600).shader_light().close(-0.5));
    // 1200 ticks before rain: RainApproaching 0.5, LightChange 1/6, InverseLerp
    // (0.4, 1, 1/6) = 0 -> -1.
    assert!(RainCycle::test(true, 0, 3600).shader_light().close(-1.0));
    // After the rain: GlobalRain's -1.
    assert!(RainCycle::test(true, 0, 4800).shader_light().close(-1.0));
}

#[test]
fn story_dark_palette() {
    assert_eq!(RainCycle::test(true, 0, 2400).rain_dark_palette(), 0.0);
    assert!(
        RainCycle::test(true, 0, 3600)
            .rain_dark_palette()
            .close(5.0 / 6.0)
    );
    assert_eq!(RainCycle::test(true, 0, 4800).rain_dark_palette(), 1.0);
}

#[test]
fn non_story_light_and_dark() {
    // TimeUntilRain 540: LerpMap(540, 200, 880, -1, 1) = 0; InverseLerp(1000, 400,
    // 540) = 460 / 600.
    let c = RainCycle::test(false, 0, 4260);
    assert!(c.shader_light().close(0.0));
    assert!(c.rain_dark_palette().close(460.0 / 600.0));
}

/// `ScreenShake` = `Pow(1 - InverseLerp(0, 0.2, RainApproaching), 2)` and `MicroScreenShake`
/// = `Pow(1 - InverseLerp(0, 0.6, RainApproaching), 1.5)` before the rain.
#[test]
fn the_shake_before_the_rain_rises_with_the_approach() {
    // 2400 ticks left: RainApproaching 1.
    let c = RainCycle::test(true, 0, 2400);
    assert_eq!((c.screen_shake(), c.micro_screen_shake()), (0.0, 0.0));
    // 240 ticks left: RainApproaching 0.1.
    let c = RainCycle::test(true, 0, 4560);
    assert!(c.screen_shake().close(0.25));
    assert!(
        c.micro_screen_shake()
            .close((1.0_f32 - 0.1 / 0.6).powf(1.5))
    );
    // 1440 ticks left: the micro shake is about to start (0.6).
    let c = RainCycle::test(true, 0, 3360);
    assert_eq!(c.screen_shake(), 0.0);
    assert_eq!(c.micro_screen_shake(), 0.0);
    // Not a story: 400 ticks of approach.
    let c = RainCycle::test(false, 0, 4720);
    assert!(c.screen_shake().close((1.0_f32 - 0.2 / 0.2).powf(2.0)));
    let c = RainCycle::test(false, 0, 4760);
    assert!(c.screen_shake().close((1.0_f32 - 0.1 / 0.2).powf(2.0)));
}

/// From the cycle's end the three getters return `GlobalRain`'s.
#[test]
fn after_the_rain_the_getters_return_the_global_rains() {
    let mut c = RainCycle::test(true, 0, 4800);
    c.global_rain.shader_light = -0.7;
    c.global_rain.screen_shake = 1.7;
    c.global_rain.micro_screen_shake = 2.3;
    assert_eq!(c.shader_light(), -0.7);
    assert_eq!(c.screen_shake(), 1.7);
    assert_eq!(c.micro_screen_shake(), 2.3);
    c.timer = 4799;
    assert!(c.shader_light() != -0.7);
    assert!(c.screen_shake() != 1.7);
}

/// `RainHit` runs once, when the timer reaches the length: the first `NextDeathRainMode`'s
/// draws (two on the 70 % path, three or four on the 30 % one), none afterwards.
#[test]
fn the_rain_hits_once_with_the_games_draws() {
    let mut paths = [false, false];
    for seed in 0..40 {
        let mut rng = UnityRandom::new(seed);
        let mut mirror = rng.clone();
        let below = mirror.value() < 0.7;
        let mut c = RainCycle::test(true, 0, 4799);
        c.rain_hit(&mut rng);
        assert!(!c.death_rain_has_hit && c.global_rain.death_rain.is_none());
        assert_eq!(rng, UnityRandom::new(seed), "no draw before the length");
        c.timer = 4800;
        c.rain_hit(&mut rng);
        assert!(c.death_rain_has_hit);
        let mode = c.global_rain.death_rain.unwrap().mode;
        if below {
            assert_eq!(mode, DeathRainMode::AlternateBuildUp);
            mirror.value();
        } else {
            assert_eq!(mode, DeathRainMode::CalmBeforeStorm);
            mirror.value();
            if mirror.value() >= 0.5 {
                mirror.value();
            }
        }
        assert_eq!(rng, mirror);
        paths[usize::from(below)] = true;
        let after = rng.clone();
        c.rain_hit(&mut rng);
        assert_eq!(rng, after, "only once");
    }
    assert_eq!(paths, [true, true]);
}
