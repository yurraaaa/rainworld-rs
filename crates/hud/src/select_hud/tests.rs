//! `SlugcatPageContinue` (554289: its HUD's parts, `Update` placing them),
//! `FoodMeter` (474428: the `CharacterSelect` constructor branch, `Update`'s plop-in,
//! `CharSelectUpdate` 475424) and `KarmaMeter` (476308: `Show` for `CharacterSelect`,
//! `Radius`).

use super::*;

struct Rig {
    hud: SelectHud,
    stage: StageTree,
    mic: MenuMic,
    rng: UnityRandom,
}

impl Rig {
    fn new(karma: (i32, i32), reinforced: bool, food_meter: (i32, i32), food: i32) -> Self {
        let mut stage = StageTree::new();
        let container = stage.add_container(StageTree::ROOT);
        let setup = SelectHudSetup {
            container,
            karma,
            karma_reinforced: reinforced,
            food_meter,
            current_food: food,
        };
        let hud = SelectHud::new(setup, &mut stage);
        Self {
            hud,
            stage,
            mic: MenuMic::default(),
            rng: UnityRandom::new(5),
        }
    }

    fn update(&mut self) {
        self.hud
            .update(&mut self.stage, &mut self.mic, &mut self.rng);
    }

    /// The page's tick: the HUD's update, then its placing at `fade`.
    fn step(&mut self, fade: f32) {
        self.update();
        self.hud.place(fade, Vec2::new(600.0, 194.0));
    }

    fn plopped(&self) -> usize {
        self.hud
            .food_meter
            .circles
            .iter()
            .filter(|c| c.plopped)
            .count()
    }

    /// Steps at fade 1 over `ticks`: the ticks a pip plopped in on.
    fn plops(&mut self, ticks: std::ops::RangeInclusive<usize>) -> Vec<usize> {
        let mut plops = Vec::new();
        for tick in ticks {
            let before = self.plopped();
            self.step(1.0);
            if self.plopped() > before {
                plops.push(tick);
            }
        }
        plops
    }
}

#[test]
fn the_page_shows_the_saved_food_and_karma() {
    let rig = Rig::new((2, 4), false, (7, 4), 3);
    let m = &rig.hud.food_meter;
    // `lastCount = CurrentFood`, `NewShowCount`; no eating, no quarter pips.
    assert_eq!((m.show_count, m.last_count), (3, 3));
    assert_eq!((m.eat_circle_delay, m.eat_circles), (0, 0));
    assert!(m.quarter_pip_shower.is_none());
    let fed: Vec<bool> = m.circles.iter().map(|c| c.food_plopped).collect();
    assert_eq!(fed, [true, true, true, false, false, false, false]);
    assert_eq!(rig.hud.karma_meter.symbol, "smallKarma2");
    let rig = Rig::new((6, 7), true, (9, 6), 0);
    assert_eq!(rig.hud.karma_meter.symbol, "smallKarma6-7");
    assert_eq!(rig.hud.food_meter.circles.len(), 9);
}

#[test]
fn the_food_meter_sits_right_of_the_karma_symbol() {
    // `pos = KarmaSymbolPos + (Radius + 22 + 0.01, 0.01)`; `Radius` adds 8 for the
    // reinforced ring while `forceSleep` is 0.
    let mut rig = Rig::new((0, 4), false, (7, 4), 0);
    rig.hud.place(1.0, Vec2::new(600.0, 194.0));
    assert_eq!(rig.hud.karma_meter.pos, Vec2::new(600.01, 194.01));
    assert_eq!(
        rig.hud.food_meter.pos,
        Vec2::new(600.0 + 22.5 + 22.01, 194.01)
    );
    let mut rig = Rig::new((0, 4), true, (7, 4), 0);
    rig.hud.place(1.0, Vec2::new(600.0, 194.0));
    assert_eq!(rig.hud.food_meter.pos.x, 600.0 + 30.5 + 22.01);
}

#[test]
fn the_pips_plop_in_every_other_tick_without_draws() {
    let mut rig = Rig::new((1, 4), false, (7, 4), 2);
    let state = rig.rng.state();
    // The page is centred from the start: fade 1 after the first update. From the
    // second, `initPlopCircle` 0 counts its delay below 1 and plops, the next delay
    // `Max(2, circle / 3)` = 2.
    rig.step(1.0);
    assert_eq!(rig.plopped(), 0);
    assert_eq!(rig.plops(2..=20), [2, 4, 6, 8, 10, 12, 14]);
    // `CharSelectUpdate` and the karma meter draw nothing, and nothing sounds while
    // the food stays.
    assert_eq!(rig.rng.state(), state);
}

#[test]
fn a_hidden_page_restarts_the_plop_in() {
    let mut rig = Rig::new((1, 4), false, (7, 4), 2);
    rig.step(0.0);
    assert_eq!(rig.hud.food_meter.init_plop_circle, -1);
    // Faded in on tick 2; `initPlopCircle` -1 spends a delay before the first pip.
    assert_eq!(rig.plops(2..=8), [5, 7]);
}

#[test]
fn the_karma_meter_shows_for_a_character_select_owner() {
    // `Show` is true for `CharacterSelect`: the fade climbs 0.1 a tick to
    // `0.25 + 0.75 * map fade` (no map) before the page overrides it.
    let mut rig = Rig::new((1, 4), false, (7, 4), 0);
    rig.update();
    assert_eq!(rig.hud.karma_meter.fade, 0.1);
    rig.update();
    rig.update();
    assert_eq!(rig.hud.karma_meter.fade, 0.25);
}
