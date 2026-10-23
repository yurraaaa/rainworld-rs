
use super::*;

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
