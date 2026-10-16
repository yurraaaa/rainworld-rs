//! `Menu.SleepAndDeathScreen` (550413) on `KarmaLadderScreen` (537968): the sleep,
//! death and starve screens of the base game with Remix off. The scene, the karma
//! ladder with its end-game meters, the passage tokens, the food meter and region map
//! of its HUD ([`SleepHud`]), CONTINUE and EXIT, the starving warning, the
//! watched-screen counters, the meters ever shown and the map toggle. PASSAGE shows as
//! the game shows it but stays greyed out: the custom end-game and fast travel screens
//! are not ported (D13). The kills display is [`SleepScreenKills`]. Not ported: the
//! Expedition, Watcher and More Slugcats branches.
//! Behaviour: `rainworld-re/notes/menu.md` ("Sleep and death screens").

use rw_assets::SlugcatName;
use rw_menu::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use rw_menu::{Font, MenuColor, MenuShared, MenuSoundId, ProcessId, StoryGameInitCondition};
use rw_random::UnityRandom;
use rw_save::WatchedScreen;
use rw_save::session::KillRecord;
use rw_sim::{FloatExt, Input, MapDiscovery, Vec2};

use crate::{
    EndgameTokens, HudOwnerType, KarmaLadder, LadderIo, MapOwner, MeterSource, PauseMenu,
    ScreenTick, SleepDeathScreenDataPackage, SleepHud, SleepHudSetup, SleepScreenKills,
};

/// The screen's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenButton {
    Continue,
    Exit,
    Passage,
}

impl ScreenButton {
    pub const ALL: [Self; 3] = [Self::Continue, Self::Exit, Self::Passage];

    #[must_use]
    pub const fn signal(self) -> &'static str {
        match self {
            Self::Continue => "CONTINUE",
            Self::Exit => "EXIT",
            Self::Passage => "PASSAGE",
        }
    }

    #[must_use]
    pub fn from_signal(signal: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.signal() == signal)
    }
}

/// The `SleepAndDeathScreen` state the base menu calls back into.
struct ScreenState {
    id: ProcessId,
    page: ObjectId,
    scene: ObjectId,
    /// The ladder's start: one below the saved karma on the sleep screen.
    karma: (i32, i32),
    karma_reinforced: bool,
    continue_button: ObjectId,
    exit_button: ObjectId,
    /// `passageButton`, once a token asks for it.
    passage_button: Option<ObjectId>,
    slugcat: SlugcatName,
    ladder: KarmaLadder,
    /// `endgameTokens` (not for the Hunter).
    tokens: Option<EndgameTokens>,
    /// The tokens' per-frame draws (D1).
    cosmetic: UnityRandom,
    /// `killsDisplay`: on the sleep screen after a cycle with kills.
    kills: Option<SleepScreenKills>,
    hud: SleepHud,
    food: i32,
    player_room: i32,
    player_pos: Vec2,
    start_malnourished: bool,
    goal_malnourished: bool,
    end_game_scene_counter: i32,
    force_watch_animation: bool,
    show_flower: bool,
    fade_out_illustration: f32,
    starved_label: Option<ObjectId>,
    starved_warning_counter: i32,
    map_toggle: bool,
    last_map_button_down: bool,
    last_animations_done: bool,
    /// The map's discovery textures, handed over by the game.
    discovery: MapDiscovery,
    /// The region's `roomsVisited` in the save state.
    rooms_visited: Vec<String>,
    /// `RWInput.PlayerInput(0)` this tick.
    input: Input,
    frames_per_second: i32,
}

/// A sleep, death or starve screen.
pub struct SleepAndDeathScreen {
    menu: Menu,
    screen: ScreenState,
}

impl SleepAndDeathScreen {
    /// `new SleepAndDeathScreen(manager, ID)` and, with the game's package,
    /// `GetDataFromGame` (`CommunicateWithUpcomingProcess` runs it before the first
    /// update).
    pub fn new(
        shared: &mut MenuShared,
        id: ProcessId,
        package: SleepDeathScreenDataPackage,
    ) -> Self {
        let sleep = id == ProcessId::SleepScreen;
        let mut menu = Menu::new(id, shared);
        let page = menu.add_page(shared, "main", 0);
        menu.selected = None;
        menu.set_info_label_y(shared.screen_size[1] - 30.0);
        menu.my_sound_loop = Some(if sleep {
            MenuSoundId::SleepScreenLoop
        } else {
            MenuSoundId::DeathScreenLoop
        });
        shared.play_menu_sound(
            if sleep {
                MenuSoundId::EnterSleepScreen
            } else {
                MenuSoundId::EnterDeathScreen
            },
            [0.0, 1.0, 1.0],
        );
        // `forceWatchAnimation` with `options.validation` is cleared again in
        // `GetDataFromGame`.
        let screen = ScreenState::from_game(&mut menu, shared, id, page, package);
        Self { menu, screen }
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    #[must_use]
    pub fn id(&self) -> ProcessId {
        self.screen.id
    }

    #[must_use]
    pub fn ladder(&self) -> &KarmaLadder {
        &self.screen.ladder
    }

    #[must_use]
    pub fn hud(&self) -> &SleepHud {
        &self.screen.hud
    }

    pub fn hud_mut(&mut self) -> &mut SleepHud {
        &mut self.screen.hud
    }

    /// `framesPerSecond`: 200 while the map button is held over an unfinished ladder.
    #[must_use]
    pub fn frames_per_second(&self) -> i32 {
        self.screen.frames_per_second
    }

    #[must_use]
    pub fn force_watch_animation(&self) -> bool {
        self.screen.force_watch_animation
    }

    #[must_use]
    pub fn fade_out_illustration(&self) -> f32 {
        self.screen.fade_out_illustration
    }

    #[must_use]
    pub fn map_toggle(&self) -> bool {
        self.screen.map_toggle
    }

    /// `RevealMap`.
    #[must_use]
    pub fn reveal_map(&self) -> bool {
        self.screen.reveal_map()
    }

    #[must_use]
    pub fn starved_label(&self) -> Option<ObjectId> {
        self.screen.starved_label
    }

    /// CONTINUE, EXIT and PASSAGE once added.
    #[must_use]
    pub fn buttons(&self) -> Vec<ObjectId> {
        let s = &self.screen;
        let mut buttons = vec![s.continue_button, s.exit_button];
        buttons.extend(s.passage_button);
        buttons
    }

    #[must_use]
    pub fn tokens(&self) -> Option<&EndgameTokens> {
        self.screen.tokens.as_ref()
    }

    /// `killsDisplay`.
    #[must_use]
    pub fn kills(&self) -> Option<&SleepScreenKills> {
        self.screen.kills.as_ref()
    }

    #[must_use]
    pub fn scene(&self) -> ObjectId {
        self.screen.scene
    }

    /// `SleepAndDeathScreen.Update` with this tick's player input.
    pub fn update(&mut self, cx: &mut MenuCx<'_>, input: Input) {
        let s = &mut self.screen;
        s.input = input;
        if s.starved_warning_counter >= 0 {
            s.starved_warning_counter += 1;
        }
        self.menu.update(&mut self.screen, cx);
        let s = &mut self.screen;
        if let Some(kills) = &mut s.kills {
            kills.update(&mut self.menu, cx.shared, s.page, input.map);
        }
        self.karma_ladder_screen_update(cx);
        let s = &mut self.screen;
        let greyed = s.buttons_greyed_out(cx);
        if let Some(b) = self.menu.behavior_mut(s.exit_button) {
            b.greyed_out = greyed;
        }
        // Always greyed out: the custom end-game screen is not ported (D13).
        if let Some(b) = s.passage_button.and_then(|p| self.menu.button_mut(p)) {
            b.behav.greyed_out = true;
            b.black = (b.black - 0.0125).max(0.0);
        }
        let done = s.ladder.all_animation_done()
            && s.end_game_scene_counter < 0
            && s.tokens
                .as_ref()
                .is_none_or(EndgameTokens::all_animations_done);
        if !s.last_animations_done && done && !s.map_toggle {
            s.map_toggle = true;
        }
        s.last_animations_done = done;
        if !s.last_map_button_down && input.map {
            s.map_toggle = !s.map_toggle;
            let text = s.info_text(&self.menu, cx);
            self.menu.refresh_info_label(&text);
        }
        let s = &mut self.screen;
        s.last_map_button_down = input.map;
        if s.end_game_scene_counter >= 0 {
            s.end_game_scene_counter += 1;
            if s.end_game_scene_counter > 140 {
                cx.requests.switch_to(ProcessId::CustomEndGameScreen);
            }
        }
        let target = if s.reveal_map() { 1.0 } else { 0.0 };
        s.fade_out_illustration = s.fade_out_illustration.lerp_and_tick(target, 0.02, 0.025);
        if !cx.shared.flat_illustrations {
            self.fade_depth_layers();
        }
    }

    /// The end of `Update` in depth mode: the scene's back layers dim as the map shows
    /// (`setAlpha = Lerp(from, to, fadeOutIllustration)` by layer index).
    fn fade_depth_layers(&mut self) {
        let s = &self.screen;
        let Kind::Scene(scene) = &self.menu.object(s.scene).kind else {
            return;
        };
        let alphas: Vec<(ObjectId, f32)> = Self::depth_fades(s.id)
            .iter()
            .filter_map(|&(layer, from, to)| {
                scene
                    .depth_illustrations
                    .get(layer)
                    .map(|&id| (id, from.lerp(to, s.fade_out_illustration)))
            })
            .collect();
        for (id, alpha) in alphas {
            if let Some(i) = self.menu.illustration_mut(id) {
                i.set_alpha = Some(alpha);
            }
        }
    }

    /// Per screen: (layer index, alpha with the map hidden, alpha with it shown).
    #[must_use]
    pub fn depth_fades(id: ProcessId) -> &'static [(usize, f32, f32)] {
        match id {
            ProcessId::SleepScreen => &[(1, 0.24, 0.1), (0, 1.0, 0.2), (2, 1.0, 0.35)],
            ProcessId::StarveScreen => &[(0, 0.85, 0.4)],
            ProcessId::DeathScreen => &[(0, 1.0, 0.1), (2, 1.0, 0.25), (3, 1.0, 0.5)],
            _ => &[],
        }
    }

    /// `KarmaLadderScreen.Update` after `Menu.Update`: the frame rate, CONTINUE's greying
    /// and fade from black, then the HUD.
    fn karma_ladder_screen_update(&mut self, cx: &mut MenuCx<'_>) {
        let s = &mut self.screen;
        let map_closed = s.hud.map.as_ref().is_none_or(|m| m.fade < 0.5);
        s.frames_per_second = if !s.ladder.all_animation_done() && map_closed && s.input.map {
            200
        } else {
            40
        };
        let greyed = s.buttons_greyed_out(cx);
        if let Some(b) = self.menu.button_mut(s.continue_button) {
            b.behav.greyed_out = greyed;
            b.black = (b.black - 0.025).max(0.0);
        }
        let tick = ScreenTick {
            owner: s.hud.owner(),
            current_food: s.food,
            allow_food_meter_tick: s.kills.as_ref().is_none_or(|k| k.counted_and_done),
            map_button: s.input.map,
            start_malnourished: s.start_malnourished,
            goal_malnourished: s.goal_malnourished,
            starve_label_alpha: s.starve_label_alpha(1.0),
            food_meter_x: s.hud.food_meter_x(),
        };
        let owner = MapOwner {
            discovery_active: false,
            in_room_pos: s.player_pos,
            room: s.player_room,
            complete_darkness: false,
            input: s.input,
            rooms_visited: s.rooms_visited.clone(),
            now_ticks: MapOwner::now_ticks(),
        };
        let reveal = s.reveal_map();
        let shared = &mut *cx.shared;
        let done = s.hud.update(
            &mut self.menu.stage,
            &tick,
            reveal,
            &owner,
            &mut shared.mic,
            &mut shared.random,
            &mut s.discovery,
        );
        if done {
            self.food_count_down_done();
        }
    }

    /// `FoodCountDownDone`: the ladder moves a notch up after sleeping, down after a
    /// death (the flower fades in when the karma was reinforced, or for the Monk).
    fn food_count_down_done(&mut self) {
        log::info!("Karma ladder MOVE!");
        let s = &mut self.screen;
        if s.id == ProcessId::SleepScreen {
            s.ladder.go_to_karma(s.karma.0 + 1, true);
        } else {
            s.ladder.go_to_karma(s.karma.0 - 1, true);
            if s.show_flower {
                self.menu.start_scene_timer(s.scene);
            }
        }
        if s.starved_label.is_some() {
            s.starved_warning_counter = 0;
        }
    }

    /// `GrafUpdate`: the menu's objects, the ladder, the HUD, then the starving label's
    /// red pulse and fade-in.
    pub fn graf_update(&mut self, t: f32, mouse: Vec2, screen_height: f32) {
        self.menu.graf_update(&self.screen, t, mouse);
        let s = &mut self.screen;
        let owner = self.menu.draw_pos(s.page, t);
        let map_fade = s.hud.map.as_ref().map(|m| m.last_fade.lerp(m.fade, t));
        s.ladder
            .draw(&mut self.menu.stage, owner, t, map_fade, screen_height);
        if let Some(tokens) = &s.tokens {
            tokens.draw(
                &mut self.menu.stage,
                owner,
                t,
                &s.ladder.meters,
                &mut s.cosmetic,
            );
        }
        s.hud.draw(&mut self.menu.stage, t);
        if let Some(kills) = &s.kills {
            kills.draw(&mut self.menu, owner, t);
        }
        if let Some(label) = s.starved_label
            && let Kind::Label(l) = &self.menu.object(label).kind
        {
            let node = l.label;
            let pulse = 0.5
                - 0.5
                    * ((t + s.starved_warning_counter as f32) / 30.0 * std::f32::consts::PI * 2.0)
                        .sin();
            let alpha = s.starve_label_alpha(t);
            let label = self.menu.stage.label_mut(node);
            label.color = MenuColor::MediumGrey
                .rgb()
                .lerp(rw_menu::StageColor::rgb(1.0, 0.0, 0.0), pulse);
            label.transform.alpha = alpha;
        }
    }

    /// `ShutDownProcess`.
    pub fn shut_down(&mut self, shared: &mut MenuShared) {
        self.menu.shut_down(shared);
    }
}

impl ScreenState {
    /// `GetDataFromGame`.
    fn from_game(
        menu: &mut Menu,
        shared: &mut MenuShared,
        id: ProcessId,
        page: ObjectId,
        package: SleepDeathScreenDataPackage,
    ) -> Self {
        let sleep = id == ProcessId::SleepScreen;
        let any_death = matches!(id, ProcessId::DeathScreen | ProcessId::StarveScreen);
        log::info!(
            "{id:?} screen get data from game! karma: {:?} reinf: {} sMal:{} gMal:{}",
            package.karma,
            package.karma_reinforced,
            package.start_malnourished,
            package.goal_malnourished
        );
        let (k, cap) = package.karma;
        let down = if sleep && cap < 100 { -1 } else { 0 };
        let karma = ((k + down).clamp(0, cap.max(0)), cap);
        // `AddBkgIllustration`.
        let scene_id = match id {
            ProcessId::SleepScreen => SceneId::SleepScreen,
            ProcessId::DeathScreen => SceneId::NewDeath,
            _ => SceneId::StarveScreen,
        };
        let scene = menu.add_scene(shared, page, scene_id);
        menu.add_sub(page, scene);
        menu.scene = Some(scene);
        // `InitContainers`: two ladder containers, the HUD's two, then the third.
        let page_container = menu.container_of(page);
        let behind = menu.stage.add_container(page_container);
        let main = menu.stage.add_container(page_container);
        let hud_containers = [0, 1].map(|_| {
            let c = menu.add_menu_container(page, Vec2::ZERO, None);
            menu.add_sub(page, c);
            c
        });
        let front = menu.stage.add_container(page_container);
        // `AddSubObjects`: CONTINUE, EXIT, then the ladder (on a death after
        // `PlayerDied`, as its `AddEndgameMeters` calls it).
        let x = PauseMenu::continue_and_exit_buttons_x(shared.screen_size);
        let size = Vec2::new(110.0, 30.0);
        let text = shared.translator.translate("CONTINUE").to_owned();
        let continue_button = menu.add_simple_button(
            shared,
            page,
            &text,
            ScreenButton::Continue.signal(),
            Vec2::new(x - 180.0, 15.0),
            size,
        );
        menu.add_sub(page, continue_button);
        menu.page_data_mut(page).last_selected = Some(continue_button);
        let text = shared.translator.translate("EXIT").to_owned();
        let exit_button = menu.add_simple_button(
            shared,
            page,
            &text,
            ScreenButton::Exit.signal(),
            Vec2::new(x - 320.0, 15.0),
            size,
        );
        menu.add_sub(page, exit_button);
        let slugcat = package.slugcat;
        // `winState`: the package's save state's (none without one).
        let trackers = shared.saves.state_in_memory_mut(slugcat).map(|state| {
            let win_state = &mut state.death_persistent.win_state;
            if any_death {
                win_state.player_died();
            }
            win_state.trackers.clone()
        });
        let source = trackers.as_ref().map(|trackers| MeterSource {
            trackers,
            font: shared.atlases.font(Font::FONT).cloned(),
            translator: &shared.translator,
        });
        let ladder = KarmaLadder::new(
            &mut menu.stage,
            [behind, main, front],
            Vec2::new(350.0, 450.0),
            karma,
            package.karma_reinforced,
            shared.screen_size[0],
            source.as_ref(),
        );
        // `InitSleepHud`: the HUD's containers as `fContainers` = (second, first).
        let hud_nodes = hud_containers.map(|c| {
            menu.object(c)
                .my_container
                .expect("a menu container has its own")
        });
        let region = package.map_data.as_ref().map(|m| m.region.clone());
        let hud = SleepHud::new(
            SleepHudSetup {
                owner: if sleep {
                    HudOwnerType::SleepScreen
                } else {
                    HudOwnerType::DeathScreen
                },
                containers: [hud_nodes[1], hud_nodes[0]],
                screen_size: shared.screen_size,
                max_food: package.max_food,
                food_to_hibernate: package.food_to_hibernate,
                current_food: package.food,
                map: package.map_data,
            },
            &mut menu.stage,
            &mut shared.atlases,
        );
        let rooms_visited = region
            .and_then(|r| {
                shared
                    .saves
                    .state_in_memory(slugcat)
                    .map(|s| s.rooms_visited(&r))
            })
            .unwrap_or_default();
        let mut state = Self {
            id,
            page,
            scene,
            karma,
            karma_reinforced: package.karma_reinforced,
            continue_button,
            exit_button,
            passage_button: None,
            slugcat,
            ladder,
            tokens: None,
            cosmetic: UnityRandom::new(0),
            kills: None,
            hud,
            food: package.food,
            player_room: package.player_room,
            player_pos: package.player_pos,
            start_malnourished: package.start_malnourished,
            goal_malnourished: package.goal_malnourished,
            end_game_scene_counter: -1,
            force_watch_animation: false,
            show_flower: false,
            fade_out_illustration: 0.0,
            starved_label: None,
            starved_warning_counter: -1,
            map_toggle: false,
            last_map_button_down: false,
            last_animations_done: false,
            discovery: package.discovery,
            rooms_visited,
            input: Input::NONE,
            frames_per_second: 40,
        };
        state.watch_and_warn(menu, shared, slugcat, &package.session_record.kills);
        let fulfilled_now = state.passages(menu, shared, trackers.is_some());
        // No dev tools and no Remix: only validation skips the forced watch; a passage
        // fulfilled on the sleep screen forces it again.
        if shared.options.validation {
            state.force_watch_animation = false;
        }
        if fulfilled_now {
            state.force_watch_animation = true;
        }
        state
    }

    /// `LeftHandButtonsPosXAdd`.
    fn left_hand_buttons_x(screen: [f32; 2]) -> f32 {
        screen[0].lerp_map(1024.0, 1280.0, 222.0, 70.0)
    }

    /// The end of `GetDataFromGame`: the tokens (not for the Hunter), the first meter
    /// never shown before (on the sleep screen: forces the watch, recorded and saved),
    /// and whether a meter's passage was fulfilled on this sleep screen.
    fn passages(&mut self, menu: &mut Menu, shared: &mut MenuShared, has_save: bool) -> bool {
        if self.slugcat != SlugcatName::Red {
            let x = Self::left_hand_buttons_x(shared.screen_size) + 140.0;
            let (tokens, add_now) = EndgameTokens::new(
                &mut menu.stage,
                menu.container,
                Vec2::new(x, 15.0),
                &self.ladder.meters,
            );
            self.tokens = Some(tokens);
            if add_now {
                self.add_passage_button(menu, shared, false);
            }
        }
        if self.id != ProcessId::SleepScreen {
            return false;
        }
        if has_save {
            let ids: Vec<_> = self.ladder.meters.iter().map(|m| m.tracker.id).collect();
            let state = shared.saves.state_in_memory_mut(self.slugcat);
            if let Some(dp) = state.map(|s| &mut s.death_persistent) {
                let shown = dp.meters_ever_shown();
                if let Some(&id) = ids.iter().find(|id| !shown.iter().any(|s| s == id.value())) {
                    self.force_watch_animation = true;
                    dp.add_meter_shown(id);
                    if let Err(e) = shared
                        .saves
                        .save_death_persistent_data_of_current_state(false, false)
                    {
                        log::error!("saving the meters shown: {e}");
                    }
                }
            }
        }
        self.ladder.meters.iter().any(|m| m.fulfilled_now)
    }

    /// `AddPassageButton(buttonBlack)`: PASSAGE at the bottom left, never for the
    /// Hunter; greyed out for good (D13).
    fn add_passage_button(&mut self, menu: &mut Menu, shared: &MenuShared, black: bool) {
        if self.slugcat == SlugcatName::Red || self.passage_button.is_some() {
            return;
        }
        log::info!("Add passage button");
        let text = shared.translator.translate("PASSAGE").to_owned();
        let button = menu.add_simple_button(
            shared,
            self.page,
            &text,
            ScreenButton::Passage.signal(),
            Vec2::new(Self::left_hand_buttons_x(shared.screen_size), 15.0),
            Vec2::new(110.0, 30.0),
        );
        menu.add_sub(self.page, button);
        if let Some(b) = menu.button_mut(button) {
            b.behav.greyed_out = true;
            if black {
                b.black = 1.0;
            }
        }
        self.passage_button = Some(button);
    }

    /// The rest of `GetDataFromGame`: the watched-screen counters (the first viewings
    /// wait for the ladder), the flower, the starving warning and the malnourished food
    /// meter.
    fn watch_and_warn(
        &mut self,
        menu: &mut Menu,
        shared: &mut MenuShared,
        slugcat: SlugcatName,
        kills: &[KillRecord],
    ) {
        let misc = &mut shared.saves.misc;
        if self.id == ProcessId::SleepScreen {
            let watched = misc.watched(WatchedScreen::Sleep);
            self.force_watch_animation |= watched < 2;
            misc.set_watched(WatchedScreen::Sleep, watched + 1);
            if !kills.is_empty() {
                let pos = Vec2::new(Self::left_hand_buttons_x(shared.screen_size), 728.0);
                let buttons = PauseMenu::continue_and_exit_buttons_x(shared.screen_size);
                let mut display = SleepScreenKills::new(&shared.atlases, pos, buttons, kills);
                display.started = true;
                self.kills = Some(display);
            }
            let hud = &mut self.hud;
            if self.goal_malnourished {
                let text = shared
                    .translator
                    .translate("You are starving. Your game has not been saved.")
                    .to_owned();
                let label = menu.add_label(
                    shared,
                    self.page,
                    &text,
                    Vec2::new(0.0, 24.0),
                    Vec2::new(1366.0, 20.0),
                    true,
                );
                menu.add_sub(self.page, label);
                self.starved_label = Some(label);
                let misc = &mut shared.saves.misc;
                let watched = misc.watched(WatchedScreen::Malnourish);
                self.force_watch_animation |= watched < 6;
                misc.set_watched(WatchedScreen::Malnourish, watched + 1);
            }
            let misc = &mut shared.saves.misc;
            if self.start_malnourished {
                hud.food_meter.new_show_count(hud.food_meter.max_food);
                let watched = misc.watched(WatchedScreen::Malnourish);
                self.force_watch_animation |= watched < 4;
                misc.set_watched(WatchedScreen::Malnourish, watched + 1);
            }
            if self.goal_malnourished || self.start_malnourished {
                let count = hud.food_meter.show_count as f32;
                hud.food_meter.move_survival_limit(count, false);
                hud.food_meter.eat_circles = hud.food_meter.max_food;
            }
        } else {
            let watched = misc.watched(WatchedScreen::Death);
            self.force_watch_animation |= watched < 2;
            misc.set_watched(WatchedScreen::Death, watched + 1);
            if self.karma_reinforced {
                let watched = misc.watched(WatchedScreen::DeathWithFlower);
                self.force_watch_animation |= watched < 2;
                misc.set_watched(WatchedScreen::DeathWithFlower, watched + 1);
            }
            self.show_flower = self.karma_reinforced || slugcat == SlugcatName::Yellow;
        }
        if self.start_malnourished {
            let hud = &mut self.hud;
            let max = hud.food_meter.max_food as f32;
            hud.food_meter.move_survival_limit(max, false);
        }
    }

    /// `RevealMap`: the map button, once the ladder has shown its movement.
    fn reveal_map(&self) -> bool {
        self.end_game_scene_counter < 0 && self.input.map && self.ladder.movement_shown
    }

    /// `FreezeMenuFunctions`: the map button or the map, or the base menu's.
    fn freeze_menu_functions(&self, cx: &MenuCx<'_>) -> bool {
        self.input.map
            || self.reveal_map()
            || cx.requests.upcoming().is_some()
            || cx.requests.fade_to_black() > 0.5
    }

    /// `ButtonsGreyedOut`.
    fn buttons_greyed_out(&self, cx: &MenuCx<'_>) -> bool {
        self.freeze_menu_functions(cx)
            || (self.force_watch_animation && !self.ladder.all_animation_done())
            || self.end_game_scene_counter >= 0
            || self
                .tokens
                .as_ref()
                .is_some_and(|t| t.force_show_token_add && !t.all_animations_done())
    }

    /// `StarveLabelAlpha`.
    fn starve_label_alpha(&self, t: f32) -> f32 {
        (self.starved_warning_counter as f32 + t).inverse_lerp(40.0, 60.0)
    }
}

impl MenuScreen for ScreenState {
    /// `Singal`: CONTINUE starts the next cycle (`startGameCondition = Load`), EXIT goes
    /// to the main menu.
    fn signal(&mut self, _menu: &mut Menu, cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        match ScreenButton::from_signal(message) {
            Some(ScreenButton::Continue) => {
                cx.shared.start_game_condition = Some(StoryGameInitCondition::Load);
                cx.requests.switch_to(ProcessId::Game);
                cx.play_sound(MenuSoundId::ContinueFromSleepDeathScreen);
            }
            Some(ScreenButton::Exit) => {
                cx.requests.switch_to(ProcessId::MainMenu);
                cx.play_sound(MenuSoundId::SwitchPageOut);
            }
            // Greyed out for good, so never sent (D13).
            Some(ScreenButton::Passage) | None => {}
        }
    }

    fn freeze(&self) -> bool {
        self.input.map || self.reveal_map()
    }

    /// The page's sub-objects ran: the ladder is the last of them that updates.
    fn page_update(&mut self, menu: &mut Menu, page: ObjectId, cx: &mut MenuCx<'_>) {
        let owner = menu.screen_pos(page);
        let shared = &mut *cx.shared;
        let mut io = LadderIo {
            stage: &mut menu.stage,
            mic: &mut shared.mic,
            mic_attached: shared.mic_attached,
            rng: &mut shared.random,
            fade_circles: &mut self.hud.fade_circles,
        };
        self.ladder
            .update(&mut io, owner, self.fade_out_illustration);
        let add = self
            .tokens
            .as_mut()
            .is_some_and(|t| t.update(&self.ladder.meters));
        if add {
            self.add_passage_button(menu, cx.shared, true);
        }
    }

    /// `UpdateInfoText`: the buttons' explanations; nothing while the map shows.
    fn info_text(&self, menu: &Menu, cx: &MenuCx<'_>) -> String {
        if self.reveal_map() {
            return String::new();
        }
        let text = menu
            .selected
            .and_then(|s| menu.button(s))
            .filter(|_| menu.selected.and_then(|s| menu.simple_kind(s)).is_some())
            .and_then(|b| ScreenButton::from_signal(&b.signal_text))
            .map(|b| match b {
                ScreenButton::Exit => "Exit to title screen",
                ScreenButton::Passage => {
                    "(One time use) Fast travel to any discovered shelter, recovering all karma"
                }
                ScreenButton::Continue => "Continue to the next cycle",
            });
        text.map_or_else(String::new, |t| {
            cx.shared.translator.translate(t).to_owned()
        })
    }
}
