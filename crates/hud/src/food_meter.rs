//! `HUD.FoodMeter`: a row of pips (one per food point, `maxFood` of them), a gap and
//! the hibernation line before pip `foodToHibernate`, quarter pips on the next pip, the
//! fill and eat animations and when the meter shows. Only the player's meter (owner
//! `Player`) is ported; the sleep, death and character-select screens' updates, the
//! More Slugcats pup meters and the Watcher's camouflage back circle are not
//! (`rainworld-re/notes/hud.md`).

use std::f32::consts::PI;

use rw_menu::{NodeId, Sprite, StageColor, StageTree};
use rw_render::StageShader;
use rw_sim::{FloatExt, SoundId, Vec2};

use crate::circle::{FADABLE_COLORS, FadeCircle, HudCircle, SnapGraphic};
use crate::{CustomFloat, HudFx, OwnerTick};

/// Pixels between pip centres (`CircleDistance` of the player's meter).
pub const CIRCLE_DISTANCE: f32 = 30.0;

/// What the pips read of their meter during its update.
#[derive(Debug, Clone, Copy)]
struct MeterView {
    pos: Vec2,
    fade: f32,
    last_fade: f32,
    show_survival_limit: i32,
    show_count: i32,
    time_counter: i32,
    refuse_counter: i32,
    /// The owner is in an intact shelter, not its start shelter, not ready to win.
    shelter_warning: bool,
}

impl MeterView {
    /// `MeterCircle.XAdd`.
    fn x_add(&self, number: i32) -> f32 {
        let gap = if number >= self.show_survival_limit {
            CIRCLE_DISTANCE / 2.0
        } else {
            0.0
        };
        CIRCLE_DISTANCE * number as f32 + gap
    }
}

/// `FoodMeter.MeterCircle`: one pip, an outer ring (`FoodCircleA`) and a filled disc
/// (`FoodCircleB`) when the food point is there.
#[derive(Debug, Clone)]
pub struct MeterCircle {
    pub gradient: NodeId,
    pub circles: [HudCircle; 2],
    pub number: i32,
    /// Per circle: radius and its velocity.
    pub rads: [[f32; 2]; 2],
    pub plopped: bool,
    pub food_plopped: bool,
    pub eaten: bool,
    pub food_plop_delay: i32,
    pub eat_counter: i32,
    pub slow_x_add: f32,
    pub last_slow_x_add: f32,
}

impl MeterCircle {
    /// `DrawPos` given the meter's draw position.
    fn draw_pos(&self, meter_pos: Vec2, t: f32) -> Vec2 {
        meter_pos + Vec2::new(self.last_slow_x_add.lerp(self.slow_x_add, t), 0.0)
    }

    /// `InitPlop`.
    fn init_plop(&mut self) {
        self.rads[0][0] = self.circles[0].snap_rad - 1.5;
        self.rads[0][1] += 0.5;
        self.rads[1][1] += 0.5;
        self.plopped = true;
    }

    /// `FoodPlop`.
    fn food_plop(&mut self, fx: &mut HudFx<'_>) {
        self.food_plopped = true;
        self.rads[1][1] += 2.0;
        self.food_plop_delay = 16;
        fx.play_sound(SoundId::HudFoodMeterFillPlopA);
    }

    /// `QuarterCirclePlop`.
    fn quarter_circle_plop(&mut self, meter_pos: Vec2, fx: &mut HudFx<'_>) {
        self.rads[0][0] += 1.5;
        fx.play_sound(SoundId::HudFoodMeterFillQuarterPlop);
        let pos = self.draw_pos(meter_pos, 1.0);
        fx.add_fade_circle(10.0, 4.0, 0.82, 14.0, 4.0, pos);
    }

    /// `EatFade`.
    fn eat_fade(&mut self, fx: &mut HudFx<'_>) {
        self.eat_counter = 50;
        self.eaten = true;
        self.rads[0][0] = self.circles[0].snap_rad + 1.5;
        self.rads[0][1] += 0.6;
        fx.play_sound(SoundId::HudFoodMeterDepletePlopA);
    }

    fn update(&mut self, m: &MeterView, fx: &mut HudFx<'_>) {
        self.last_slow_x_add = self.slow_x_add;
        self.slow_x_add = self
            .slow_x_add
            .lerp_and_tick(m.x_add(self.number), 0.06, 2.0);
        for i in 0..2 {
            let snap = self.circles[i].snap_rad;
            let r = &mut self.rads[i];
            if !self.plopped {
                r[0] = snap / 2.0;
                r[1] = 0.0;
                continue;
            }
            r[0] += r[1];
            r[1] *= if r[0] < snap { 0.8 } else { 0.95 };
            r[1] += (snap - r[0]) * 0.2;
            r[0] = r[0].lerp_and_tick(snap, 0.0001, 0.2);
        }
        let mut num = 1.0;
        self.circles[0].color = 0;
        if self.number < m.show_survival_limit
            && self.number >= m.show_count
            && !self.eaten
            && m.shelter_warning
        {
            if m.time_counter % 20 > 10 {
                self.rads[0][0] *= 0.96;
                self.circles[0].color = 1;
            }
            num = 0.65 + 0.35 * (m.time_counter as f32 / 20.0 * PI * 2.0).sin();
        }
        let pos = self.draw_pos(m.pos, 1.0);
        for (circle, rad) in self.circles.iter_mut().zip(self.rads) {
            circle.update();
            circle.pos = pos;
            circle.rad = rad[0];
        }
        self.circles[0].fade = if self.plopped { m.fade * num } else { 0.0 };
        if self.eaten {
            self.eat_counter -= 1;
            if self.eat_counter < 1 {
                self.food_plopped = false;
                self.eaten = false;
            }
            let eat = self.eat_counter as f32;
            self.circles[1].fade = eat.inverse_lerp(0.0, 35.0).mathf_pow(1.2) * m.fade;
            if self.eat_counter > 30 {
                let f = eat.inverse_lerp(30.0, 50.0);
                self.rads[0][0] =
                    self.rads[0][0].lerp_and_tick(13.0 + 0.5 * (f * PI).sin(), 0.02, 1.5 + 0.5 * f);
                self.rads[0][1] *= 0.0;
            } else if self.eat_counter == 29 {
                self.rads[0][1] = -1.5;
                fx.play_sound(SoundId::HudFoodMeterDepletePlopB);
            }
            let snap = self.circles[1].snap_rad;
            self.rads[1][0] = eat.lerp_map(40.0, 0.0, snap, snap / 2.0);
        } else {
            if m.refuse_counter > 0 {
                let rng = &mut *fx.rng;
                self.rads[0][0] += (-1.0_f32).lerp(1.0, rng.value()) * 0.5;
                self.rads[1][0] += (-0.25_f32).lerp(1.0, rng.value());
                self.rads[1][1] += rng.value() * 0.4;
                if self.rads[1][0] + 1.0 > self.rads[0][0] {
                    self.rads[0][0] = self.rads[1][0] + 1.0;
                    self.rads[0][1] += 0.2 + rng.value() * 0.4;
                }
            }
            self.circles[1].fade = if self.plopped && self.food_plopped {
                m.fade
            } else {
                0.0
            };
        }
        if self.food_plop_delay > 0 {
            self.food_plop_delay -= 1;
            if self.food_plop_delay == 12 {
                self.rads[0][0] = self.circles[0].snap_rad + 2.0;
                self.rads[0][1] += 1.0;
                fx.play_sound(SoundId::HudFoodMeterFillPlopB);
            } else if self.food_plop_delay == 0 {
                fx.add_fade_circle(10.0, 10.0, 0.82, 30.0, 4.0, pos);
                fx.play_sound(SoundId::HudFoodMeterFillFadeCircle);
            }
        }
        if m.last_fade == 0.0 {
            self.plopped = false;
        }
        self.circles[0].visible = self.plopped;
        self.circles[1].visible = self.plopped && self.food_plopped;
    }

    fn draw(&self, stage: &mut StageTree, meter_pos: Vec2, fade: f32, t: f32) {
        let pos = self.draw_pos(meter_pos, t);
        let g = stage.sprite_mut(self.gradient);
        g.transform.set_scale(6.25);
        g.transform.alpha = 0.1 * fade.max(0.0).mathf_pow(2.0);
        g.transform.x = pos.x;
        g.transform.y = pos.y;
        for circle in &self.circles {
            circle.draw(stage, t);
            let tr = &mut stage.sprite_mut(circle.sprite).transform;
            tr.x = pos.x;
            tr.y = pos.y;
        }
    }
}

/// `FoodMeter.QuarterPipShower`: the quarter food points drawn on the next empty pip.
#[derive(Debug, Clone)]
pub struct QuarterPipShower {
    pub quarter_pips: NodeId,
    pub display_quarter_food: i32,
    pub quarter_pip_delay: i32,
    pub quarter_pip_sin: f32,
    pub last_quarter_pip_sin: f32,
    pub light_up: f32,
    pub last_light_up: f32,
}

impl QuarterPipShower {
    fn new(stage: &mut StageTree, container: NodeId) -> Self {
        Self {
            quarter_pips: stage.add_sprite(container, Sprite::new("pixel")),
            display_quarter_food: 0,
            quarter_pip_delay: 0,
            quarter_pip_sin: 0.0,
            last_quarter_pip_sin: 0.0,
            light_up: 0.0,
            last_light_up: 0.0,
        }
    }
}

/// `HUD.FoodMeter` of the player.
#[derive(Debug, Clone)]
pub struct FoodMeter {
    pub line_sprite: NodeId,
    pub dark_fade: NodeId,
    pub circles: Vec<MeterCircle>,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub fade: f32,
    pub last_fade: f32,
    pub last_count: i32,
    pub visible_counter: i32,
    pub show_count: i32,
    pub show_count_delay: i32,
    pub init_plop_circle: i32,
    pub init_plop_delay: i32,
    pub refuse_counter: i32,
    pub eat_circle_delay: i32,
    pub time_counter: i32,
    pub down_in_corner: f32,
    pub max_food: i32,
    pub survival_limit: i32,
    pub quarter_pip_shower: QuarterPipShower,
    last_show_surv_lim: f32,
    show_surv_lim: f32,
    surv_lim_from: f32,
    surv_lim_to: f32,
    surv_lim_prog: f32,
    pub force_sleep: f32,
}

/// What the food meter reads of the karma and rain meters during its update.
#[derive(Debug, Clone, Copy)]
pub struct MeterNeighbours {
    pub karma_pos: Vec2,
    pub karma_fade: f32,
    /// `rad` and `showAsReinforced`: `Radius` also reads this meter's `forceSleep`.
    pub karma_rad: f32,
    pub karma_reinforced: bool,
    pub karma_any_visibility: bool,
    pub rain_fade: f32,
    pub show_karma_food_rain: bool,
    pub hide_general_hud: bool,
}

impl FoodMeter {
    /// The meter's resting corner position (`SafeScreenOffset` is zero).
    pub const CORNER: Vec2 = Vec2::new(50.0, 25.0);

    /// The constructor for the player's meter, its sprites added to `container`.
    pub fn new(
        stage: &mut StageTree,
        container: NodeId,
        max_food: i32,
        survival_limit: i32,
        current_food: i32,
    ) -> Self {
        let show_surv_lim = survival_limit as f32;
        let gradients: Vec<NodeId> = (0..max_food)
            .map(|_| {
                let mut g = Sprite::new(rw_menu::AtlasManager::WHITE);
                g.shader = StageShader::FlatLight;
                g.color = StageColor::BLACK;
                stage.add_sprite(container, g)
            })
            .collect();
        let mut dark = Sprite::new(rw_menu::AtlasManager::WHITE);
        dark.shader = StageShader::FlatLight;
        dark.color = StageColor::BLACK;
        let dark_fade = stage.add_sprite(container, dark);
        let mut line = Sprite::new("pixel");
        line.transform.scale_x = 2.0;
        line.transform.scale_y = 34.5;
        let line_sprite = stage.add_sprite(container, line);
        let view = MeterView {
            pos: Self::CORNER,
            fade: 0.0,
            last_fade: 0.0,
            show_survival_limit: show_surv_lim.round_to_int(),
            show_count: 0,
            time_counter: 0,
            refuse_counter: 0,
            shelter_warning: false,
        };
        let circles = gradients
            .into_iter()
            .zip(0..)
            .map(|(gradient, number)| {
                let mut circles = [
                    HudCircle::new(stage, container, SnapGraphic::FoodCircleA, 0),
                    HudCircle::new(stage, container, SnapGraphic::FoodCircleB, 0),
                ];
                for c in &mut circles {
                    c.rad = 0.0;
                    c.last_rad = 0.0;
                }
                let x_add = view.x_add(number);
                MeterCircle {
                    gradient,
                    rads: [[circles[0].snap_rad, 0.0], [circles[1].snap_rad, 0.0]],
                    circles,
                    number,
                    plopped: false,
                    food_plopped: false,
                    eaten: false,
                    food_plop_delay: 0,
                    eat_counter: 0,
                    slow_x_add: x_add,
                    last_slow_x_add: x_add,
                }
            })
            .collect();
        let quarter_pip_shower = QuarterPipShower::new(stage, container);
        let mut meter = Self {
            line_sprite,
            dark_fade,
            circles,
            pos: Self::CORNER,
            last_pos: Self::CORNER,
            fade: 0.0,
            last_fade: 0.0,
            last_count: current_food,
            visible_counter: 0,
            show_count: 0,
            show_count_delay: 0,
            init_plop_circle: 0,
            init_plop_delay: 0,
            refuse_counter: 0,
            eat_circle_delay: 0,
            time_counter: 0,
            down_in_corner: 0.0,
            max_food,
            survival_limit,
            quarter_pip_shower,
            last_show_surv_lim: show_surv_lim,
            show_surv_lim,
            surv_lim_from: show_surv_lim,
            surv_lim_to: show_surv_lim,
            surv_lim_prog: 1.0,
            force_sleep: 0.0,
        };
        meter.new_show_count(current_food);
        meter
    }

    /// `ShowSurvivalLimit`: the pip the hibernation line stands before.
    #[must_use]
    pub fn show_survival_limit(&self) -> i32 {
        self.show_surv_lim.round_to_int()
    }

    #[must_use]
    pub fn draw_pos(&self, t: f32) -> Vec2 {
        self.last_pos.lerp(self.pos, t)
    }

    /// `TotalWidth`: from the first pip's centre to the last's, plus a pip's radius.
    #[must_use]
    pub fn total_width(&self, t: f32) -> f32 {
        let (Some(first), Some(last)) = (self.circles.first(), self.circles.last()) else {
            return 0.0;
        };
        let pos = self.draw_pos(t);
        last.draw_pos(pos, t).x - first.draw_pos(pos, t).x + first.circles[0].snap_rad
    }

    /// `MoveSurvivalLimit`.
    pub fn move_survival_limit(&mut self, to: f32, smooth: bool) {
        if to == self.surv_lim_to {
            return;
        }
        if smooth {
            self.surv_lim_from = self.show_surv_lim;
            self.surv_lim_to = to;
            self.surv_lim_prog = 0.0;
        } else {
            self.show_surv_lim = to;
            self.last_show_surv_lim = to;
            self.surv_lim_to = to;
            self.surv_lim_from = to;
            self.surv_lim_prog = 1.0;
        }
    }

    /// `NewShowCount`.
    pub fn new_show_count(&mut self, count: i32) {
        self.show_count = count;
        self.last_count = count;
        for c in &mut self.circles {
            c.food_plopped = c.number < count;
        }
    }

    /// `RefuseFood`.
    pub fn refuse_food(&mut self) {
        self.refuse_counter = 10;
    }

    fn view(&self, owner: &OwnerTick) -> MeterView {
        MeterView {
            pos: self.pos,
            fade: self.fade,
            last_fade: self.last_fade,
            show_survival_limit: self.show_survival_limit(),
            show_count: self.show_count,
            time_counter: self.time_counter,
            refuse_counter: self.refuse_counter,
            shelter_warning: owner.in_intact_shelter
                && !owner.still_in_start_shelter
                && !owner.ready_for_win,
        }
    }

    pub fn update(&mut self, owner: &OwnerTick, n: &MeterNeighbours, fx: &mut HudFx<'_>) {
        let view = self.view(owner);
        for c in self.circles.iter_mut().rev() {
            c.update(&view, fx);
        }
        if self.last_count != owner.current_food {
            if self.fade == 0.0 && n.karma_fade == 0.0 {
                self.down_in_corner = 1.0;
            }
            self.last_count = owner.current_food;
            self.visible_counter = 200;
        }
        self.last_fade = self.fade;
        self.last_pos = self.pos;
        self.last_show_surv_lim = self.show_surv_lim;
        if self.show_surv_lim != self.surv_lim_to {
            let span = (2.0 + (self.surv_lim_from - self.surv_lim_to).abs()) * 20.0;
            self.surv_lim_prog = (self.surv_lim_prog + 1.0 / span).min(1.0);
            self.show_surv_lim = self
                .surv_lim_from
                .lerp(self.surv_lim_to, self.surv_lim_prog.s_curve(0.65));
        }
        self.game_update(owner, n, fx);
        if self.fade > 0.0 {
            if self.last_fade == 0.0 {
                self.init_plop_circle = -1;
                self.init_plop_delay = 0;
            }
            if self.init_plop_circle < self.circles.len() as i32 {
                self.init_plop_delay -= 1;
                if self.init_plop_delay < 1 {
                    if let Ok(i) = usize::try_from(self.init_plop_circle) {
                        self.circles[i].init_plop();
                    }
                    self.init_plop_circle += 1;
                    self.init_plop_delay = 2.max(self.init_plop_circle / 3);
                }
            }
            self.time_counter += 1;
        }
        if self.refuse_counter > 0 {
            self.refuse_counter -= 1;
        }
        if n.hide_general_hud {
            self.fade = 0.0;
        }
    }

    /// `GameUpdate`.
    fn game_update(&mut self, owner: &OwnerTick, n: &MeterNeighbours, fx: &mut HudFx<'_>) {
        if owner.reveal_map || self.refuse_counter > 0 || n.show_karma_food_rain {
            // `Mathf.Max(visibleCounter > 0 ? 1 : 0, 1)`: always 1.
            let target = 1.0;
            self.fade = if self.fade < target {
                (self.fade + 0.1).min(target)
            } else {
                (self.fade - 0.1).max(target)
            };
            self.update_show_count(owner, fx);
        } else {
            if self.visible_counter > 0 {
                self.visible_counter -= 1;
                self.fade = (self.fade + 0.1).min(1.0);
            } else {
                self.fade = (self.fade - 0.0125).max(0.0);
            }
            if self.fade == 1.0 {
                self.update_show_count(owner, fx);
            } else if self.fade == 0.0 {
                self.show_count_delay = 15;
            }
        }
        self.update_quarter_pips(owner, fx);
        if self.down_in_corner > 0.0 && n.karma_any_visibility {
            self.down_in_corner = (self.down_in_corner - 0.0625).max(0.0);
        } else if self.fade > 0.0 && n.karma_fade == 0.0 && !n.karma_any_visibility {
            self.down_in_corner = (self.down_in_corner + 0.0625).min(1.0);
        }
        if owner.force_sleep_counter > 0 {
            let to = (owner.force_sleep_counter as f32)
                .inverse_lerp(10.0, 260.0)
                .mathf_pow(0.75);
            let tick = 1.0 / 180.0_f32.lerp(4.0, self.force_sleep);
            self.force_sleep = self.force_sleep.lerp_and_tick(to, 0.014, tick);
        } else if owner.force_sleep_pose {
            self.force_sleep = (self.force_sleep - 1.0 / 60.0).max(-1.0);
        } else {
            self.force_sleep = (self.force_sleep - 1.0 / 30.0).max(0.0);
        }
        let lim = (self.survival_limit as f32).lerp(owner.current_food as f32, self.force_sleep);
        self.move_survival_limit(lim, false);
        let karma_radius = crate::karma_meter::KarmaMeter::radius_of(
            n.karma_rad,
            n.karma_reinforced,
            self.force_sleep,
        );
        let ring = Vec2::from_degrees(90.0_f32.lerp(135.0, self.down_in_corner))
            * (karma_radius + 22.0 + n.rain_fade.mathf_pow(0.4).s_curve(0.5) * 8.0);
        let t = (1.0 - self.down_in_corner).s_curve(0.5);
        self.pos = Self::CORNER.lerp(n.karma_pos + ring, t.clamp(0.0, 1.0));
    }

    /// `UpdateShowCount` (Remix's wait for an eaten pip is off).
    fn update_show_count(&mut self, owner: &OwnerTick, fx: &mut HudFx<'_>) {
        let food = owner.current_food;
        if self.show_count < food {
            if self.show_count_delay == 0 {
                self.show_count_delay = 10;
                if let Some(c) = usize::try_from(self.show_count)
                    .ok()
                    .and_then(|i| self.circles.get_mut(i))
                    .filter(|c| !c.food_plopped)
                {
                    c.food_plop(fx);
                }
                self.show_count += 1;
                self.quarter_pip_shower.display_quarter_food = owner.quarter_food_points;
            } else {
                self.show_count_delay -= 1;
            }
        } else if self.show_count > food {
            if self.eat_circle_delay == 0 {
                self.eat_circle_delay = 40;
            }
            self.eat_circle_delay -= 1;
            if self.eat_circle_delay < 1 {
                if let Some(c) = usize::try_from(self.show_count - 1)
                    .ok()
                    .and_then(|i| self.circles.get_mut(i))
                {
                    c.eat_fade(fx);
                }
                self.show_count -= 1;
                self.eat_circle_delay = 0;
            }
        }
    }

    /// `QuarterPipShower.Update`.
    fn update_quarter_pips(&mut self, owner: &OwnerTick, fx: &mut HudFx<'_>) {
        let q = &mut self.quarter_pip_shower;
        q.last_quarter_pip_sin = q.quarter_pip_sin;
        q.last_light_up = q.light_up;
        if q.display_quarter_food > 0 {
            q.quarter_pip_sin += 1.0;
        }
        q.light_up *= 0.95;
        if owner.quarter_food_points <= q.display_quarter_food {
            return;
        }
        self.visible_counter = 80;
        if self.fade < 0.5 {
            q.quarter_pip_delay = 20;
            return;
        }
        if q.quarter_pip_delay > 0 {
            q.quarter_pip_delay -= 1;
            return;
        }
        q.quarter_pip_delay = 20;
        q.display_quarter_food += 1;
        q.light_up = 1.0;
        let pos = self.pos;
        if let Some(c) = usize::try_from(self.show_count)
            .ok()
            .and_then(|i| self.circles.get_mut(i))
        {
            c.quarter_circle_plop(pos, fx);
        }
    }

    /// `Draw`; `force_sleep_counter` is the owner's.
    pub fn draw(&self, stage: &mut StageTree, force_sleep_counter: i32, t: f32) {
        let fade = self.last_fade.lerp(self.fade, t);
        let pos = self.draw_pos(t);
        for c in self.circles.iter().rev() {
            c.draw(stage, pos, fade, t);
        }
        // The game lerps the line by `timeCounter`, not the time stacker.
        let lim = self
            .last_show_surv_lim
            .lerp(self.show_surv_lim, self.time_counter as f32);
        let line = stage.sprite_mut(self.line_sprite);
        line.transform.x = pos.x + CIRCLE_DISTANCE * (lim - 0.25);
        if (force_sleep_counter > 10 && force_sleep_counter < 260) || self.force_sleep < 0.0 {
            let shake = if self.time_counter % 4 < 2 { -1.5 } else { 1.5 };
            line.transform.x += shake * self.force_sleep.abs().inverse_lerp(0.2, 0.9);
        }
        line.transform.y = pos.y;
        line.transform.alpha = fade;
        line.color = if self.force_sleep < 0.0 {
            let pulse = 0.5 + 0.5 * ((self.time_counter as f32 + t) / 30.0 * PI * 2.0).sin();
            StageColor::WHITE.lerp(
                StageColor::rgb(1.0, 0.0, 0.0),
                self.force_sleep.abs() * pulse,
            )
        } else {
            StageColor::WHITE
        };
        let dark = stage.sprite_mut(self.dark_fade);
        dark.transform.x = pos.x + CIRCLE_DISTANCE * self.circles.len() as f32 * 0.35;
        dark.transform.y = pos.y - 70.0;
        dark.transform.scale_x = 37.5;
        dark.transform.scale_y = 18.75;
        dark.transform.alpha = 0.2 * fade.mathf_pow(2.0);
        self.draw_quarter_pips(stage, pos, t);
    }

    /// `QuarterPipShower.Draw`.
    fn draw_quarter_pips(&self, stage: &mut StageTree, meter_pos: Vec2, t: f32) {
        let q = &self.quarter_pip_shower;
        let next = usize::try_from(self.show_count)
            .ok()
            .and_then(|i| self.circles.get(i));
        let sprite = stage.sprite_mut(q.quarter_pips);
        let Some(next) = next.filter(|_| (1..=3).contains(&q.display_quarter_food)) else {
            sprite.transform.visible = false;
            return;
        };
        sprite.transform.visible = true;
        sprite.element = format!("QuarterPips{}", q.display_quarter_food);
        let ring = &next.circles[0];
        let sin = q.last_quarter_pip_sin.lerp(q.quarter_pip_sin, t);
        let glow = (0.5 + 0.5 * (sin / 20.0).sin())
            .clamp(0.0, 1.0)
            .mathf_pow(0.4);
        let light = q.last_light_up.lerp(q.light_up, t).max(0.0).mathf_pow(2.0);
        sprite.transform.alpha =
            ring.last_fade.lerp(ring.fade, t) * 0.2_f32.lerp(0.5, glow).lerp(0.6, light);
        sprite
            .transform
            .set_scale(ring.last_rad.lerp(ring.rad, t) / ring.snap_rad);
        let pos = next.draw_pos(meter_pos, t);
        sprite.transform.x = pos.x;
        sprite.transform.y = pos.y;
        sprite.color = FADABLE_COLORS[ring.color];
    }
}

impl HudFx<'_> {
    pub(crate) fn add_fade_circle(
        &mut self,
        rad: f32,
        rad_speed: f32,
        rad_slow_down: f32,
        life_time: f32,
        thickness: f32,
        pos: Vec2,
    ) {
        let circle = FadeCircle::new(
            self.stage,
            self.container,
            rad,
            rad_speed,
            rad_slow_down,
            life_time,
            thickness,
            pos,
        );
        self.fade_circles.push(circle);
    }
}
