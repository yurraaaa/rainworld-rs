//! `HUD.RainMeter`: a ring of dots around the karma symbol, one per `timePerCircle`
//! ticks of the rain cycle, emptying as the cycle runs out, and its blink at half time.
//! It reads the cycle through [`RainCycleView`]; the Remix tick-tock, the hidden-timer
//! regions and the Saint's and void sea's rules are not ported.

use rw_menu::{NodeId, StageTree};
use rw_sim::{FloatExt, Vec2};

use crate::CustomFloat;
use crate::circle::{HudCircle, SnapGraphic};

/// What the rain meter reads of `world.rainCycle` and `setupValues.disableRain`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainCycleView {
    /// `cycleLength` in ticks.
    pub cycle_length: i32,
    /// `timer`: ticks into the cycle.
    pub timer: i32,
    pub disable_rain: bool,
}

impl RainCycleView {
    /// A cycle of `setup.cycleTimeMax` (800 s) at its start, for tests and tools.
    pub const STAND_IN: Self = Self {
        cycle_length: 32_000,
        timer: 0,
        disable_rain: false,
    };

    /// `AmountLeft`.
    #[must_use]
    pub fn amount_left(&self) -> f32 {
        (self.cycle_length - self.timer) as f32 / self.cycle_length as f32
    }
}

/// `HUD.RainMeter`.
#[derive(Debug, Clone)]
pub struct RainMeter {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub remain_visible_counter: i32,
    pub fade: f32,
    pub last_fade: f32,
    plop: f32,
    pub circles: Vec<HudCircle>,
    f_rain: f32,
    pub half_time_blink: i32,
    half_time_shown: bool,
    pub time_per_circle: i32,
}

/// What the rain meter reads of the rest of the HUD and the owner.
#[derive(Debug, Clone, Copy)]
pub struct RainNeighbours {
    pub karma_pos: Vec2,
    pub karma_fade: f32,
    pub karma_radius: f32,
    pub reveal_map: bool,
    pub show_karma_food_rain: bool,
    pub hide_general_hud: bool,
    /// The owner is in a room whose `DangerType` is not `None`.
    pub room_rain_danger: bool,
}

impl RainMeter {
    /// The constructor: `cycleLength / 1200` circles, at most 30.
    pub fn new(stage: &mut StageTree, container: NodeId, cycle: &RainCycleView) -> Self {
        let mut time_per_circle = 1200;
        let mut count = cycle.cycle_length / time_per_circle;
        if count > 30 {
            count = 30;
            time_per_circle = cycle.cycle_length / count;
        }
        let circles = (0..count)
            .map(|_| HudCircle::new(stage, container, SnapGraphic::SmallEmptyCircle, 0))
            .collect();
        Self {
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            remain_visible_counter: 0,
            fade: 0.0,
            last_fade: 0.0,
            plop: 0.0,
            circles,
            f_rain: 0.0,
            half_time_blink: 0,
            half_time_shown: false,
            time_per_circle,
        }
    }

    fn show(&self, n: &RainNeighbours) -> bool {
        self.half_time_blink > 0 || n.show_karma_food_rain || n.reveal_map
    }

    /// `Update`; may raise the karma meter's `forceVisibleCounter`.
    pub fn update(
        &mut self,
        cycle: &RainCycleView,
        n: &RainNeighbours,
        karma_force_visible: &mut i32,
    ) {
        self.last_pos = self.pos;
        self.pos = n.karma_pos;
        if !self.half_time_shown
            && !cycle.disable_rain
            && cycle.amount_left() < 0.5
            && n.room_rain_danger
        {
            self.half_time_blink = 220;
            self.half_time_shown = true;
        }
        self.last_fade = self.fade;
        if self.remain_visible_counter > 0 {
            self.remain_visible_counter -= 1;
        }
        if self.half_time_blink > 0 {
            self.half_time_blink -= 1;
            *karma_force_visible = (*karma_force_visible).max(10);
        }
        if (n.karma_fade > 0.0 && self.show(n)) || self.remain_visible_counter > 0 {
            self.fade = (self.fade + 1.0 / 30.0).min(1.0);
        } else {
            self.fade = (self.fade - 0.1).max(0.0);
        }
        if n.hide_general_hud {
            self.fade = 0.0;
        }
        if self.fade >= 0.7 {
            self.plop = (self.plop + 0.05).min(1.0);
        } else {
            self.plop = 0.0;
        }
        self.f_rain = if cycle.disable_rain {
            1.0
        } else {
            cycle.amount_left()
        };
        let count = self.circles.len() as f32;
        for (i, circle) in self.circles.iter_mut().enumerate() {
            circle.update();
            if !(self.fade > 0.0 || self.last_fade > 0.0) {
                circle.rad = 0.0;
                continue;
            }
            let fi = i as f32;
            let num = fi / (count - 1.0);
            let value = self.f_rain.inverse_lerp(fi / count, (fi + 1.0) / count);
            let mut num2 = (0.5 - value.inverse_lerp(1.0 / 30.0, 1.0))
                .abs()
                .inverse_lerp(0.5, 0.475);
            if self.half_time_blink > 0 {
                let blink = if self.half_time_blink % 15 < 7 {
                    0.0
                } else {
                    1.0
                };
                num2 = num2.max(blink);
            }
            let sweep =
                ((1.0 - self.f_rain) * self.fade - 0.075).lerp(1.075, self.plop.mathf_pow(0.85));
            let bump = (1.0 - num - sweep).abs().inverse_lerp(0.075, 0.0) * 2.0 * self.fade;
            circle.rad = ((2.0 + num2) * self.fade.mathf_pow(2.0) + bump)
                * value.inverse_lerp(0.0, 1.0 / 30.0);
            if num2 == 0.0 {
                circle.thickness = -1.0;
                circle.snap_graphic = SnapGraphic::Circle4;
                circle.snap_rad = 2.0;
                circle.snap_thickness = -1.0;
            } else {
                circle.thickness = 3.5_f32.lerp(1.0, num2);
                circle.snap_graphic = SnapGraphic::SmallEmptyCircle;
                circle.snap_rad = 3.0;
                circle.snap_thickness = 1.0;
            }
            let angle = (1.0 - fi / count) * 360.0 * self.fade.mathf_pow(1.5 - num).s_curve(0.6);
            circle.pos = self.pos + Vec2::from_degrees(angle) * (n.karma_radius + 8.5 + num2);
        }
    }

    #[must_use]
    pub fn draw_pos(&self, t: f32) -> Vec2 {
        self.last_pos.lerp(self.pos, t)
    }

    pub fn draw(&self, stage: &mut StageTree, t: f32) {
        for circle in &self.circles {
            circle.draw(stage, t);
        }
    }

    /// `ResetHalfTime`.
    pub fn reset_half_time(&mut self) {
        self.half_time_shown = false;
        self.half_time_blink = 0;
    }
}
