//! `FoodMeter` (the constructor's sleep and death branches, `SleepUpdate`,
//! `DeathUpdate`), `HUD.InitSleepHud` and
//! `SleepAndDeathScreen.FoodMeterXPos`.

use super::*;

struct Rig {
    hud: SleepHud,
    stage: StageTree,
    mic: MenuMic,
    rng: UnityRandom,
    discovery: MapDiscovery,
    tick: ScreenTick,
}

impl Rig {
    fn new(owner: HudOwnerType, max_food: i32, hibernate: i32, food: i32) -> Self {
        let mut stage = StageTree::new();
        let containers = [
            stage.add_container(StageTree::ROOT),
            stage.add_container(StageTree::ROOT),
        ];
        let setup = SleepHudSetup {
            owner,
            containers,
            screen_size: [1366.0, 768.0],
            max_food,
            food_to_hibernate: hibernate,
            current_food: food,
            map: None,
        };
        let hud = SleepHud::new(setup, &mut stage, &mut AtlasManager::new());
        let tick = ScreenTick {
            owner,
            current_food: food,
            allow_food_meter_tick: true,
            map_button: false,
            start_malnourished: false,
            goal_malnourished: false,
            starve_label_alpha: 0.0,
            food_meter_x: hud.food_meter_x(),
        };
        Self {
            hud,
            stage,
            mic: MenuMic::default(),
            rng: UnityRandom::new(3),
            discovery: MapDiscovery::default(),
            tick,
        }
    }

    fn step(&mut self) -> bool {
        self.hud.update(
            &mut self.stage,
            &self.tick,
            false,
            &MapOwner::default(),
            &mut self.mic,
            &mut self.rng,
            &mut self.discovery,
        )
    }

    /// Ticks `n` times: the ticks (from 1) the shown count dropped on, and the tick
    /// the meter called `FoodCountDownDone`.
    fn run(&mut self, n: usize) -> (Vec<usize>, Option<usize>) {
        let mut eaten = Vec::new();
        let mut done = None;
        for t in 1..=n {
            let before = self.hud.food_meter.show_count;
            if self.step() {
                assert!(done.is_none(), "FoodCountDownDone twice");
                done = Some(t);
            }
            if self.hud.food_meter.show_count < before {
                eaten.push(t);
            }
        }
        (eaten, done)
    }
}

#[test]
fn food_meter_x_by_screen_width() {
    // `LerpMap(ScreenSize.x, 1024, 1366, ScreenSize.x / 2 - 110, 540)`.
    assert_eq!(SleepHud::food_meter_x_for([1366.0, 768.0]), 540.0);
    assert_eq!(SleepHud::food_meter_x_for([1024.0, 768.0]), 402.0);
}

#[test]
fn sleep_screen_eats_the_hibernation_food_then_counts_down() {
    // The Survivor (7, 4) slept with 5: the package's food is the 1 left.
    let mut rig = Rig::new(HudOwnerType::SleepScreen, 7, 4, 1);
    let m = &rig.hud.food_meter;
    // `lastCount = Min(CurrentFood + survivalLimit, maxFood)`, delay 65, at
    // (FoodMeterXPos, 0); no quarter pips.
    assert_eq!(
        (m.show_count, m.eat_circle_delay, m.eat_circles),
        (5, 65, 4)
    );
    assert_eq!(m.pos, Vec2::new(540.0, 0.0));
    assert!(m.quarter_pip_shower.is_none());
    let state = rig.rng.state();
    let (eaten, done) = rig.run(400);
    // A pip at delay 0 (tick 65), then every 40; phase 3 at tick 186 with delay 80,
    // done when it reaches 0.
    assert_eq!(eaten, [65, 105, 145, 185]);
    assert_eq!(done, Some(266));
    assert_eq!(rig.hud.food_meter.sleep_screen_phase, 3);
    // At y 450 until the delay passes -30, at the bottom (33) from -60.
    assert_eq!(rig.hud.food_meter.pos, Vec2::new(540.0, 33.0));
    // No draws but the sounds' (none with an empty sound table).
    assert_eq!(rig.rng.state(), state);
}

#[test]
fn sleep_screen_drops_the_meter_after_the_count() {
    let mut rig = Rig::new(HudOwnerType::SleepScreen, 7, 4, 0);
    rig.run(296);
    // Delay -30 at tick 296.
    assert_eq!(rig.hud.food_meter.pos.y, 450.0);
    rig.run(30);
    assert_eq!(rig.hud.food_meter.pos.y, 33.0);
}

#[test]
fn more_than_seven_pips_eat_every_twenty_ticks() {
    // The Hunter (9, 6) with nothing left: the first after 65 ticks, then every 20.
    let mut rig = Rig::new(HudOwnerType::SleepScreen, 9, 6, 0);
    let (eaten, done) = rig.run(300);
    assert_eq!(eaten, [65, 85, 105, 125, 145, 165]);
    assert_eq!(done, Some(166 + 80));
}

#[test]
fn map_button_speeds_the_count_up() {
    let mut rig = Rig::new(HudOwnerType::SleepScreen, 7, 4, 0);
    rig.tick.map_button = true;
    // Three a tick: delay 65 is below 1 at tick 22; 40 more take 14 ticks.
    let (eaten, _) = rig.run(70);
    assert_eq!(eaten, [22, 36, 50, 64]);
}

#[test]
fn malnourished_goal_moves_the_line_before_the_count() {
    let mut rig = Rig::new(HudOwnerType::SleepScreen, 7, 4, 0);
    rig.tick.goal_malnourished = true;
    rig.hud.food_meter.eat_circles = 0;
    rig.step();
    // Nothing to eat: phase 1 with delay 80.
    assert_eq!(rig.hud.food_meter.sleep_screen_phase, 1);
    assert_eq!(rig.hud.food_meter.eat_circle_delay, 80);
    rig.run(80);
    // Delay 0 at tick 81: the line slides to maxFood; the delay waits for it.
    assert_eq!(rig.hud.food_meter.sleep_screen_phase, 2);
    let delay = rig.hud.food_meter.eat_circle_delay;
    rig.run(50);
    assert_eq!(rig.hud.food_meter.eat_circle_delay, delay);
    let (_, done) = rig.run(200);
    assert_eq!(rig.hud.food_meter.sleep_screen_phase, 3);
    assert_eq!(rig.hud.food_meter.show_survival_limit(), 7);
    assert!(done.is_some());
}

#[test]
fn death_screen_counts_down_once_from_eighty() {
    let mut rig = Rig::new(HudOwnerType::DeathScreen, 7, 4, 3);
    let m = &rig.hud.food_meter;
    assert_eq!((m.show_count, m.eat_circle_delay), (3, 80));
    let (eaten, done) = rig.run(200);
    assert!(eaten.is_empty());
    assert_eq!(done, Some(81));
    assert_eq!(rig.hud.food_meter.pos, Vec2::new(540.0, 33.0));
    let mut rig = Rig::new(HudOwnerType::DeathScreen, 7, 4, 0);
    rig.tick.map_button = true;
    // 80 - 3 * 27 < 0.
    assert_eq!(rig.run(60).1, Some(27));
}

#[test]
fn death_screen_waits_for_the_kills_count() {
    let mut rig = Rig::new(HudOwnerType::DeathScreen, 7, 4, 0);
    rig.tick.allow_food_meter_tick = false;
    rig.run(100);
    // `DeathUpdate` does nothing until allowed: no fade, still at y 0.
    assert_eq!(rig.hud.food_meter.fade, 0.0);
    assert_eq!(rig.hud.food_meter.pos.y, 0.0);
}
