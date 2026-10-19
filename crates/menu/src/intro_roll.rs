//! `IntroRoll`, the title roll at start-up: three full-screen illustrations over the
//! rain effect, lightning and thunder, the title music, then the main menu on any
//! button or after 40 seconds. Behaviour: `rainworld-re/notes/menu.md` ("IntroRoll").
//!
//! Mirrors: Menu/IntroRoll.cs

use rw_sim::{FloatExt, Vec2};

use crate::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId};
use crate::process::{MainLoopProcess, ProcessId, ProcessRequests, TimeStacker};
use crate::shared::{MenuContext, MenuShared};
use crate::sound::MenuSoundId;

/// The `IntroRoll` subclass's state.
struct IntroRollScreen {
    illustrations: [ObjectId; 3],
    rain_effect: ObjectId,
    any_button_label: ObjectId,
    /// Seconds since the process started.
    time: f32,
    /// `time`, held at 4 s until the music bundles are loaded.
    delayed_time: f32,
    continue_to_menu: bool,
    last_any_button: bool,
    force_watch_roll: bool,
}

/// The intro roll process.
pub struct IntroRoll {
    menu: Menu,
    screen: IntroRollScreen,
}

impl IntroRoll {
    /// The Downpour slugcats `Intro_Roll_C_<name>` is picked from.
    pub const DOWNPOUR_C: [&'static str; 5] =
        ["gourmand", "rivulet", "spear", "artificer", "saint"];

    /// `new IntroRoll(manager)` for the base game (no Sofanthiel or Watcher): the rain
    /// effect (its draws), the three illustrations hidden (with Downpour installed the
    /// third is a random slugcat's, one more draw), the "press any button" label at
    /// alpha 0, and the intro roll music requested.
    #[must_use]
    pub fn new(shared: &mut MenuShared) -> Self {
        let mut menu = Menu::new(ProcessId::IntroRoll, shared);
        let page = menu.add_page(shared, "main", 0);
        shared.mouse_mode = false;
        let rain_effect = menu.add_rain_effect(page, &mut shared.random);
        menu.add_sub(page, rain_effect);
        let third = if shared.dlcs.downpour {
            let i = shared.random.range_i32(0, Self::DOWNPOUR_C.len() as i32) as usize;
            format!("Intro_Roll_C_{}", Self::DOWNPOUR_C[i])
        } else {
            "Intro_Roll_C".to_owned()
        };
        let illustrations = ["Intro_Roll_A", "Intro_Roll_B", third.as_str()]
            .map(|file| menu.add_illustration(shared, page, "", file, Vec2::ZERO, true, false));
        for id in illustrations {
            menu.add_sub(page, id);
            if let Kind::Illustration(i) = &menu.object(id).kind {
                let sprite = i.sprite;
                menu.stage.transform_mut(sprite).visible = false;
            }
        }
        let text = shared
            .translator
            .translate("Press any button to continue")
            .to_owned();
        let [w, _] = shared.screen_size;
        let label = menu.add_label(
            shared,
            page,
            &text,
            Vec2::new(w / 2.0 - 50.0, 40.0),
            Vec2::new(100.0, 30.0),
            false,
        );
        menu.add_sub(page, label);
        let shift = menu.horizontal_move_to_get_centered();
        menu.object_mut(label).pos.x += shift;
        IntroRollScreen::set_label_alpha(&mut menu, label, 0.0);
        if let Some(music) = &mut shared.music {
            music.request_intro_roll_music();
        }
        Self {
            menu,
            screen: IntroRollScreen {
                illustrations,
                rain_effect,
                any_button_label: label,
                time: 0.0,
                delayed_time: 0.0,
                continue_to_menu: false,
                last_any_button: false,
                force_watch_roll: false,
            },
        }
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    /// Seconds since the process started (`time`).
    #[must_use]
    pub fn time(&self) -> f32 {
        self.screen.time
    }

    /// `delayedTime`.
    #[must_use]
    pub fn delayed_time(&self) -> f32 {
        self.screen.delayed_time
    }

    /// The illustrations' alphas (A, B, C).
    #[must_use]
    pub fn illustration_alphas(&self) -> [f32; 3] {
        self.screen
            .illustrations
            .map(|id| match &self.menu.object(id).kind {
                Kind::Illustration(i) => i.alpha,
                _ => 0.0,
            })
    }

    /// The third illustration's file.
    #[must_use]
    pub fn third_illustration(&self) -> &str {
        match &self.menu.object(self.screen.illustrations[2]).kind {
            Kind::Illustration(i) => &i.file_name,
            _ => "",
        }
    }

    #[must_use]
    pub fn rain_effect(&self) -> &crate::menu::RainEffectData {
        self.menu
            .rain_effect(self.screen.rain_effect)
            .expect("the intro roll's rain effect")
    }

    /// The "press any button" label's alpha.
    #[must_use]
    pub fn label_alpha(&self) -> f32 {
        match &self.menu.object(self.screen.any_button_label).kind {
            Kind::Label(l) => self.menu.stage.label(l.label).transform.alpha,
            _ => 0.0,
        }
    }

    /// `continueToMenu`.
    #[must_use]
    pub fn continue_to_menu(&self) -> bool {
        self.screen.continue_to_menu
    }
}

impl IntroRollScreen {
    fn set_label_alpha(menu: &mut Menu, label: ObjectId, alpha: f32) {
        if let Kind::Label(l) = &menu.object(label).kind {
            let node = l.label;
            menu.stage.label_mut(node).transform.alpha = alpha;
        }
    }

    /// `CheckTimeStamp`: `check` was passed between `last` and `current`.
    fn passed(last: f32, current: f32, check: f32) -> bool {
        current >= check && last < check
    }

    /// `GoToMenu`: once, when the music bundles are loaded (sound, platform, options and
    /// progression are ready from the start in the port) and no dialog shows: the title
    /// music starts if it has not, the rain fades, and the main menu is requested.
    fn go_to_menu(&mut self, shared: &mut MenuShared, requests: &mut ProcessRequests) {
        if self.continue_to_menu || !shared.music_ready() {
            return;
        }
        self.continue_to_menu = true;
        if let Some(song) = shared.intro_roll_song() {
            song.start_music();
            song.fade_out_rain();
        }
        requests.switch_to(ProcessId::MainMenu);
    }

    /// `IntroRoll.Update` after the base: any button newly pressed after a second goes
    /// to the menu; past 14 s of `delayedTime`, a random lightning spike one tick in 160.
    fn update(&mut self, menu: &mut Menu, any_button: bool, cx: &mut MenuCx<'_>) {
        let earliest = if self.force_watch_roll { 14.5 } else { 1.0 };
        if self.time > earliest && any_button && !self.last_any_button {
            self.go_to_menu(cx.shared, cx.requests);
        }
        self.last_any_button = any_button;
        let random = &mut cx.shared.random;
        if self.delayed_time > 14.0 && random.value() < 1.0 / 160.0 {
            let intensity = random.value().mathf_pow(2.0) * 0.85;
            let drop_off = 20.0_f32.lerp(120.0, random.value());
            if let Some(rain) = menu.rain_effect_mut(self.rain_effect) {
                rain.lightning_spike(intensity, drop_off);
            }
        }
    }

    /// `IntroRoll.RawUpdate` after the base's ticks and drawing: the clocks, the fades,
    /// the scripted lightning, thunder and title music, and the 40 s time-out.
    fn after_frame(
        &mut self,
        menu: &mut Menu,
        dt: f32,
        shared: &mut MenuShared,
        requests: &mut ProcessRequests,
    ) {
        let last_time = self.time;
        self.time += dt;
        let last_delayed = self.delayed_time;
        if self.delayed_time < 4.0 || shared.music_ready() {
            self.delayed_time += dt;
        }
        let (time, delayed) = (self.time, self.delayed_time);
        let rain_fade = Menu::s_curve(time.inverse_lerp(4.0, 10.0), 0.8);
        let alphas = [
            time.inverse_lerp(1.0, 2.0).min(time.inverse_lerp(4.0, 3.0)),
            time.inverse_lerp(4.0, 5.0).min(time.inverse_lerp(7.0, 6.0)),
            delayed.inverse_lerp(10.0, 11.0),
        ]
        .map(|a| Menu::s_curve(a, 0.65));
        for (&id, alpha) in self.illustrations.iter().zip(alphas) {
            if let Kind::Illustration(i) = &mut menu.object_mut(id).kind {
                i.alpha = alpha;
                let sprite = i.sprite;
                menu.stage.transform_mut(sprite).visible = alpha > 0.0;
            }
        }
        Self::set_label_alpha(
            menu,
            self.any_button_label,
            Menu::s_curve(delayed.inverse_lerp(14.0, 15.0), 0.65),
        );
        if let Some(rain) = menu.rain_effect_mut(self.rain_effect) {
            rain.rain_fade = rain_fade;
            for (at, intensity, drop_off) in
                [(1.5, 0.4, 30.0), (3.0, 0.6, 40.0), (5.0, 0.3, 1600.0)]
            {
                if Self::passed(last_time, time, at) {
                    rain.lightning_spike(intensity, drop_off);
                }
            }
        }
        if Self::passed(last_delayed, delayed, 10.3) {
            let random = &mut shared.random;
            shared
                .mic
                .play_sound(MenuSoundId::ThunderClose, [0.0, 0.3, 1.0], random);
            shared
                .mic
                .play_sound(MenuSoundId::Thunder, [0.0, 0.4, 1.0], random);
        }
        if Self::passed(last_delayed, delayed, 10.5)
            && let Some(rain) = menu.rain_effect_mut(self.rain_effect)
        {
            rain.lightning_spike(1.0, 40.0);
        }
        if Self::passed(last_delayed, delayed, 8.5)
            && let Some(song) = shared.intro_roll_song()
        {
            song.start_music();
        }
        if !self.continue_to_menu && delayed > 40.0 {
            self.go_to_menu(shared, requests);
        }
    }
}

impl MenuScreen for IntroRollScreen {
    fn signal(&mut self, _menu: &mut Menu, _cx: &mut MenuCx<'_>, _sender: ObjectId, _m: &str) {}

    fn shows_cursor(&self) -> bool {
        false
    }
}

impl<C: MenuContext> MainLoopProcess<C> for IntroRoll {
    fn id(&self) -> ProcessId {
        ProcessId::IntroRoll
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        // Read before the base update takes the tick's input; nothing changes it between.
        let any_button = ctx.menu().input.any_button();
        let mut cx = MenuCx {
            shared: ctx.menu(),
            requests,
        };
        self.menu.update(&mut self.screen, &mut cx);
        self.screen.update(&mut self.menu, any_button, &mut cx);
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut C) {
        let mouse = Vec2::from(ctx.menu().input.mouse.pos);
        self.menu.graf_update(&self.screen, time_stacker, mouse);
    }

    fn raw_update(
        &mut self,
        dt: f32,
        stacker: &mut TimeStacker,
        ctx: &mut C,
        requests: &mut ProcessRequests,
    ) {
        for _ in 0..stacker.advance(dt, 40) {
            self.update(ctx, requests);
        }
        self.graf_update(stacker.0, ctx);
        self.screen
            .after_frame(&mut self.menu, dt, ctx.menu(), requests);
    }

    fn shut_down(&mut self, ctx: &mut C) {
        self.menu.shut_down(ctx.menu());
    }
}
