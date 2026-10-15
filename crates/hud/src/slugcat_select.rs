//! `SlugcatSelectMenu` (553691) in the base game (Remix, Jolly Co-op and More Slugcats
//! off): a page per slugcat that scrolls sideways, the hold-to-start button (new game,
//! or continue a saved one), back, the arrows and the restart check box, which rises
//! on a page with a save and makes the start a new game. A slugcat without a save gets
//! a `SlugcatPageNewGame` (name and blurb), one with a save a `SlugcatPageContinue`:
//! the region and cycle, and a HUD ([`SelectHud`]) with the saved karma and food. In
//! depth mode the menu has the rain effect behind the pages, and a continue page the
//! mark and glow its save has ([`PageLights`]); flat mode has neither.

mod lights;

use rw_assets::StreamingAssets;
use rw_menu::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use rw_menu::{
    KeyboardKey, MainLoopProcess, MenuColor, MenuContext, MenuShared, MenuSoundId, ProcessId,
    ProcessRequests, SlideShowId, Slugcat, StoryGameInitCondition,
};
use rw_random::UnityRandom;
use rw_save::{MinerTarget, SaveState, SaveStateMiner};
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::{FloatExt, Vec2};

use crate::{SelectHud, SelectHudSetup};

pub use lights::{LightsFrame, PageLights};

/// A `SlugcatPage`.
pub struct SlugcatPage {
    pub page: ObjectId,
    pub slugcat: Slugcat,
    pub scene: ObjectId,
    pub image_pos: Vec2,
    pub scene_offset: Vec2,
    pub in_position: f32,
    pub last_in_position: f32,
    pub lights: PageLights,
    pub kind: PageKind,
}

/// The page's subclass.
pub enum PageKind {
    /// `SlugcatPageNewGame`.
    NewGame {
        difficulty_label: ObjectId,
        info_label: ObjectId,
    },
    /// `SlugcatPageContinue`.
    Continue(Box<ContinuePage>),
}

/// `SlugcatPageContinue`'s own parts.
pub struct ContinuePage {
    pub hud: SelectHud,
    pub region_label: ObjectId,
}

/// `SaveGameData`: what `MineForSaveData` reads of a stored game for its page. The
/// More Slugcats and Remix fields are not read.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveGameData {
    pub shelter_name: String,
    pub cycle: i32,
    pub food: i32,
    pub karma: i32,
    pub karma_cap: i32,
    pub karma_reinforced: bool,
    /// The Watcher's; 0 in a base-game save.
    pub ripple_level: f32,
    pub has_glow: bool,
    pub has_mark: bool,
    pub reds_extra_cycles: bool,
    pub reds_death: bool,
    pub ascended: bool,
}

/// The values `MineForSaveData` mines from a stored save state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MinedKey {
    DenPos,
    LastVDenPos,
    CycleNum,
    Food,
    HasTheGlow,
    ReinforcedKarma,
    Karma,
    KarmaCap,
    RippleLevel,
    HasTheMark,
    RedExtraCycles,
    RedsDeath,
    Ascended,
}

/// The start button's labels, translated once.
#[derive(Debug, Clone)]
struct StartTexts {
    new_game: String,
    delete_save: String,
    statistics: String,
    continue_game: String,
}

/// The `SlugcatSelectMenu` subclass's state.
pub struct SlugcatSelectScreen {
    pub pages: Vec<SlugcatPage>,
    pub slugcat_page_index: usize,
    pub scroll: f32,
    pub last_scroll: f32,
    pub qued_side_input: i32,
    pub start_button: ObjectId,
    pub back_button: ObjectId,
    pub prev_button: ObjectId,
    pub next_button: ObjectId,
    pub restart_checkbox: ObjectId,
    restart_checked: bool,
    restart_available: bool,
    restart_up: f32,
    last_pause_button: bool,
    screen_width: f32,
    /// `saveGameData`, per page.
    pub save_game_data: Vec<Option<SaveGameData>>,
    /// `redIsDead`: the Hunter's save ended (its last cycle's death, or ascended).
    pub red_is_dead: bool,
    /// `rainEffect`: depth mode only.
    pub rain_effect: Option<ObjectId>,
    texts: StartTexts,
}

/// The slugcat select process.
pub struct SlugcatSelectMenu {
    menu: Menu,
    screen: SlugcatSelectScreen,
    /// The pages' per-frame draws (D1).
    cosmetic: UnityRandom,
}

impl MinedKey {
    /// `MineForSaveData`'s targets in its order; `>REDEXTRACYCLES` is looked for in
    /// the save state's and in the death-persistent section, `>REDSDEATH` for the
    /// Hunter only.
    fn targets(slugcat: Slugcat) -> Vec<(Self, MinerTarget)> {
        let sv = Some("<svB>");
        let dp = Some("<dpB>");
        let mut targets = vec![
            (Self::DenPos, MinerTarget::new(">DENPOS", sv, "<svA>", 35)),
            (
                Self::LastVDenPos,
                MinerTarget::new(">LASTVDENPOS", sv, "<svA>", 20),
            ),
            (
                Self::CycleNum,
                MinerTarget::new(">CYCLENUM", sv, "<svA>", 50),
            ),
            (Self::Food, MinerTarget::new(">FOOD", sv, "<svA>", 20)),
            (
                Self::HasTheGlow,
                MinerTarget::new(">HASTHEGLOW", None, "<svA>", 20),
            ),
            (
                Self::ReinforcedKarma,
                MinerTarget::new(">REINFORCEDKARMA", dp, "<dpA>", 20),
            ),
            (Self::Karma, MinerTarget::new(">KARMA", dp, "<dpA>", 20)),
            (
                Self::KarmaCap,
                MinerTarget::new(">KARMACAP", dp, "<dpA>", 20),
            ),
            (
                Self::RippleLevel,
                MinerTarget::new(">RIPPLELEVEL", dp, "<dpA>", 20),
            ),
            (
                Self::HasTheMark,
                MinerTarget::new(">HASTHEMARK", None, "<dpA>", 20),
            ),
            (
                Self::RedExtraCycles,
                MinerTarget::new(">REDEXTRACYCLES", None, "<svA>", 20),
            ),
            (
                Self::RedExtraCycles,
                MinerTarget::new(">REDEXTRACYCLES", None, "<dpA>", 20),
            ),
        ];
        if slugcat == Slugcat::Red {
            targets.push((
                Self::RedsDeath,
                MinerTarget::new(">REDSDEATH", None, "<dpA>", 20),
            ));
        }
        targets.push((
            Self::Ascended,
            MinerTarget::new(">ASCENDED", None, "<dpA>", 20),
        ));
        targets
    }
}

impl SaveGameData {
    /// `MineForSaveData`'s first branch: the current save state of the slugcat.
    #[must_use]
    pub fn from_state(state: &SaveState, room_exists: impl Fn(&str) -> bool) -> Self {
        let d = &state.death_persistent;
        Self {
            shelter_name: state.den_to_use(room_exists),
            cycle: state.cycle_number,
            food: state.food,
            karma: d.karma,
            karma_cap: d.karma_cap,
            karma_reinforced: d.reinforced_karma,
            ripple_level: 0.0,
            has_glow: state.the_glow,
            has_mark: d.the_mark,
            reds_extra_cycles: state.red_extra_cycles(),
            reds_death: d.reds_death,
            ascended: d.ascended,
        }
    }

    /// `MineForSaveData`'s second branch: `SaveStateMiner` over the stored state. A
    /// number that does not parse is left 0, as the game logs and skips it. The den is
    /// `DENPOS`, else `LASTVDENPOS`, else the final fallback shelter, whichever names
    /// a room.
    #[must_use]
    pub fn from_stored(
        slugcat: Slugcat,
        save_state: &str,
        room_exists: impl Fn(&str) -> bool,
    ) -> Self {
        let targets = MinedKey::targets(slugcat);
        let list: Vec<MinerTarget> = targets.iter().map(|(_, t)| *t).collect();
        let mut data = Self {
            shelter_name: String::new(),
            cycle: 0,
            food: 0,
            karma: 0,
            karma_cap: 0,
            karma_reinforced: false,
            ripple_level: 0.0,
            has_glow: false,
            has_mark: false,
            reds_extra_cycles: false,
            reds_death: false,
            ascended: false,
        };
        let mut shelter: Option<String> = None;
        for result in SaveStateMiner::mine(save_state, &list) {
            let Some((key, _)) = targets.iter().find(|(_, t)| t.data_id == result.name) else {
                continue;
            };
            let value = result.data.unwrap_or_default();
            match key {
                MinedKey::DenPos => shelter = Some(value),
                MinedKey::LastVDenPos => {
                    if shelter.as_deref().is_none_or(|s| !room_exists(s)) {
                        shelter = Some(value);
                    }
                }
                MinedKey::CycleNum => data.cycle = Self::parse_int(&value, "cycle num"),
                MinedKey::Food => data.food = Self::parse_int(&value, "food"),
                MinedKey::HasTheGlow => data.has_glow = true,
                MinedKey::ReinforcedKarma => data.karma_reinforced = value == "1",
                MinedKey::Karma => data.karma = Self::parse_int(&value, "karma"),
                MinedKey::KarmaCap => data.karma_cap = Self::parse_int(&value, "karma cap"),
                MinedKey::RippleLevel => match value.trim().parse() {
                    Ok(r) => data.ripple_level = r,
                    Err(_) => log::warn!("failed to assign ripple level. Data: {value}"),
                },
                MinedKey::HasTheMark => data.has_mark = true,
                MinedKey::RedExtraCycles => data.reds_extra_cycles = true,
                MinedKey::RedsDeath => data.reds_death = true,
                MinedKey::Ascended => data.ascended = true,
            }
        }
        data.shelter_name = shelter
            .filter(|s| room_exists(s))
            .unwrap_or_else(|| SaveState::final_fallback_shelter(slugcat).to_owned());
        data
    }

    fn parse_int(value: &str, what: &str) -> i32 {
        value.trim().parse().unwrap_or_else(|_| {
            log::warn!("failed to assign {what}. Data: {value}");
            0
        })
    }

    /// `RainWorld.roomNameToIndex.ContainsKey`: a room of the installed world.
    #[must_use]
    pub fn room_exists(assets: &StreamingAssets, room: &str) -> bool {
        assets
            .find_room_file(&room.to_lowercase(), ".txt")
            .is_some()
    }

    /// The region label's text: the shelter's region's display name and the cycle (the
    /// Hunter's cycles left), or nothing for a shelter name of two characters or
    /// fewer. The speedrun timer's text is Remix's.
    #[must_use]
    pub fn region_text(&self, slugcat: Slugcat, shared: &MenuShared) -> String {
        if self.shelter_name.chars().count() <= 2 {
            return String::new();
        }
        // The game takes the name up to its first `_` (a name without one throws).
        let acronym = self.shelter_name.split('_').next().unwrap_or_default();
        let name = shared
            .assets
            .region_full_name(acronym, Some(slugcat.value()));
        if name.is_empty() {
            return String::new();
        }
        let cycle = if slugcat == Slugcat::Red {
            SaveState::reds_cycles(self.reds_extra_cycles) - self.cycle
        } else {
            self.cycle
        };
        let t = &shared.translator;
        format!("{} - {} {cycle}", t.translate(&name), t.translate("Cycle"))
    }
}

impl SlugcatPage {
    /// `ScrollMagnitude`.
    pub const SCROLL_MAGNITUDE: f32 = 500.0;

    /// The game's English text for a new-game page: the name and the blurb with its
    /// line delimiter replaced (`Custom.ReplaceLineDelimeters`).
    #[must_use]
    pub fn texts(slugcat: Slugcat, shared: &MenuShared) -> (String, String) {
        let t = &shared.translator;
        let (name, blurb) = match slugcat {
            Slugcat::White => (
                t.translate("THE SURVIVOR"),
                t.translate(
                    "A nimble omnivore, both predator and prey. Lost in a harsh and indifferent<LINE>land you must make your own way, with wit and caution as your greatest assets.",
                ),
            ),
            Slugcat::Yellow => (
                t.translate("THE MONK"),
                t.translate(
                    "Weak of body but strong of spirit. In tune with the mysteries of the world and<LINE>empathetic to its creatures, your journey will be a significantly more peaceful one.",
                ),
            ),
            // The Hunter. In English the game replaces the translation with a fixed text.
            _ => (
                t.translate("THE HUNTER"),
                "Strong and quick, with a fierce metabolism requiring a steady diet of meat. But the<LINE>stomach won't be your only concern, as the path of the hunter is one of extreme peril.",
            ),
        };
        (
            name.to_owned(),
            blurb.replace("<LINE>", "\r\n").replace("<WWLINE>", ""),
        )
    }

    /// A new-game page's name and blurb labels.
    #[must_use]
    pub fn new_game_labels(&self) -> Option<(ObjectId, ObjectId)> {
        match &self.kind {
            PageKind::NewGame {
                difficulty_label,
                info_label,
            } => Some((*difficulty_label, *info_label)),
            PageKind::Continue(_) => None,
        }
    }

    /// A continue page's own parts.
    #[must_use]
    pub fn continue_page(&self) -> Option<&ContinuePage> {
        match &self.kind {
            PageKind::Continue(c) => Some(c),
            PageKind::NewGame { .. } => None,
        }
    }

    /// `AddImage(ascended)`: the scene and its offset (the depth, mark and glow offsets
    /// are [`PageLights::new`]'s).
    #[must_use]
    pub fn image(slugcat: Slugcat, ascended: bool, red_is_dead: bool) -> (SceneId, Vec2) {
        match slugcat {
            Slugcat::White if ascended => (SceneId::GhostWhite, Vec2::new(-10.0, 100.0)),
            Slugcat::White => (SceneId::SlugcatWhite, Vec2::new(-10.0, 100.0)),
            Slugcat::Yellow if ascended => (SceneId::GhostYellow, Vec2::new(10.0, 75.0)),
            Slugcat::Yellow => (SceneId::SlugcatYellow, Vec2::new(10.0, 75.0)),
            // The Hunter.
            _ if ascended => (SceneId::GhostRed, Vec2::new(10.0, 45.0)),
            _ if red_is_dead => (SceneId::SlugcatDeadRed, Vec2::new(10.0, 45.0)),
            _ => (SceneId::SlugcatRed, Vec2::new(10.0, 45.0)),
        }
    }
}

impl SlugcatSelectScreen {
    /// `slugcatColorOrder` of the base game.
    pub const ORDER: [Slugcat; 3] = [Slugcat::Yellow, Slugcat::White, Slugcat::Red];

    /// `NextScroll`.
    #[must_use]
    pub fn next_scroll(&self) -> f32 {
        self.scroll
            .lerp_and_tick(0.0, 0.07, (self.qued_side_input.abs() + 1) as f32 / 30.0)
    }

    /// Page `i`'s offset from the centre in pages, from the menu's scroll `scroll`,
    /// wrapped into -1..1.
    fn wrap_page_offset(&self, i: usize, scroll: f32) -> f32 {
        let count = self.pages.len() as f32;
        let mut num = (i as f32 - self.slugcat_page_index as f32) - scroll;
        if num < -1.0 {
            num += count;
        } else if num > 1.0 {
            num -= count;
        }
        num
    }

    /// `SlugcatPage.Scroll(timeStacker)` of page `i`.
    #[must_use]
    pub fn page_scroll(&self, i: usize, t: f32) -> f32 {
        self.wrap_page_offset(i, self.last_scroll.lerp(self.scroll, t))
    }

    /// `SlugcatPage.NextScroll(1)` of page `i`.
    #[must_use]
    pub fn page_next_scroll(&self, i: usize) -> f32 {
        self.wrap_page_offset(i, self.next_scroll())
    }

    /// `UseAlpha`.
    #[must_use]
    pub fn use_alpha(&self, i: usize, t: f32) -> f32 {
        Menu::s_curve((1.0 - self.page_scroll(i, t).abs()).clamp(0.0, 1.0), 0.5)
    }

    /// `SlugcatPageContinue.KarmaSymbolPos` of page `i`, with its HUD.
    #[must_use]
    pub fn karma_symbol_pos(&self, i: usize, hud: &SelectHud) -> Vec2 {
        let x = self.screen_width / 2.0
            - hud.food_meter.total_width(1.0) * 0.5
            - hud.karma_radius() * 0.5
            + self.page_next_scroll(i) * SlugcatPage::SCROLL_MAGNITUDE;
        Vec2::new(x, self.pages[i].image_pos.y - 290.0)
    }

    /// `SlugcatSelectMenu.SlugcatUnlocked`: a slugcat with a save, else
    /// `SlugcatStats.SlugcatUnlocked`.
    #[must_use]
    pub fn slugcat_unlocked(slugcat: Slugcat, shared: &MenuShared, saved: bool) -> bool {
        saved || slugcat != Slugcat::Red || shared.progression.red_unlocked
    }

    /// `MineForSaveData` of `slugcat`'s game, or `None` without one: the current save
    /// state if it is the slugcat's, else the stored one mined.
    #[must_use]
    pub fn mine_for_save_data(slugcat: Slugcat, shared: &mut MenuShared) -> Option<SaveGameData> {
        if !shared.saves.is_there_a_saved_game(slugcat) {
            return None;
        }
        let assets = &shared.assets;
        if let Some(state) = shared
            .saves
            .current_save_state
            .as_ref()
            .filter(|s| s.save_state_number == slugcat)
        {
            return Some(SaveGameData::from_state(state, |room| {
                SaveGameData::room_exists(assets, room)
            }));
        }
        let line = shared.saves.stored_state(slugcat)?;
        let assets = &shared.assets;
        Some(SaveGameData::from_stored(slugcat, &line, |room| {
            SaveGameData::room_exists(assets, room)
        }))
    }

    fn saved(&self, i: usize) -> Option<&SaveGameData> {
        self.save_game_data.get(i).and_then(Option::as_ref)
    }

    fn page_index(&self, page: ObjectId) -> Option<usize> {
        self.pages.iter().position(|p| p.page == page)
    }

    /// `UpdateStartButtonText`: held for 40 ticks (120 with restart checked).
    fn update_start_button_text(&self, menu: &mut Menu) {
        if let Some(h) = menu.hold_button_mut(self.start_button) {
            h.fill_time = if self.restart_checked { 120.0 } else { 40.0 };
        }
        let slugcat = self.pages[self.slugcat_page_index].slugcat;
        let text = if self.saved(self.slugcat_page_index).is_none() {
            self.texts.new_game.clone()
        } else if self.restart_checked {
            self.texts.delete_save.replace(' ', "\r\n")
        } else if slugcat == Slugcat::Red && self.red_is_dead {
            self.texts.statistics.clone()
        } else {
            self.texts.continue_game.clone()
        };
        let label = menu.hold_button(self.start_button).map(|h| h.menu_label);
        if let Some(label) = label {
            menu.set_label_text(label, &text);
        }
    }

    /// `UpdateSelectedSlugcatInMiscProg`.
    fn update_selected_slugcat(&self, shared: &mut MenuShared) {
        if let Some(page) = self.pages.get(self.slugcat_page_index) {
            shared.select_slugcat(page.slugcat);
        }
    }

    /// `StartGame`: the Hunter needs unlocking. A saved game continues unless restart is
    /// checked (the Hunter's ended one goes to its statistics); otherwise the save is
    /// wiped and a new game starts, the Survivor and the Monk with their intro slideshow
    /// unless S or the map button is held.
    fn start_game(&mut self, slugcat: Slugcat, cx: &mut MenuCx<'_>) {
        if slugcat == Slugcat::Red && !cx.shared.progression.red_unlocked {
            return;
        }
        cx.shared.in_game_slugcat = Some(slugcat);
        cx.shared.saves.current_save_state = None;
        cx.shared.select_slugcat(slugcat);
        if !self.restart_checked && cx.shared.saves.is_there_a_saved_game(slugcat) {
            if slugcat == Slugcat::Red && self.red_is_dead {
                cx.requests.switch_to(ProcessId::Statistics);
                cx.play_sound(MenuSoundId::SwitchPageOut);
            } else {
                cx.shared.start_game_condition = Some(StoryGameInitCondition::Load);
                cx.requests.switch_to(ProcessId::Game);
                cx.play_sound(MenuSoundId::ContinueGame);
            }
            if let Some(song) = cx.shared.intro_roll_song() {
                song.fade_out(20.0);
            }
            return;
        }
        if let Err(e) = cx.shared.saves.wipe_save_state(slugcat) {
            log::error!("wiping the {slugcat} save: {e}");
        }
        cx.shared.start_game_condition = Some(StoryGameInitCondition::New);
        let input = &cx.shared.input;
        let skip_intro = input.key(KeyboardKey::S) || input.map_action();
        match SlideShowId::intro_for(slugcat).filter(|_| !skip_intro) {
            Some(intro) => {
                cx.shared.next_slideshow = Some(intro);
                cx.requests.switch_to(ProcessId::SlideShow);
            }
            None => cx.requests.switch_to(ProcessId::Game),
        }
        cx.play_sound(MenuSoundId::StartNewGame);
        if let Some(song) = cx.shared.intro_roll_song() {
            song.fade_out(20.0);
        }
    }

    /// `SlugcatSelectMenu.Update` after the base.
    fn update(&mut self, menu: &mut Menu, cx: &mut MenuCx<'_>) {
        if let Some(rain) = self.rain_effect.and_then(|id| menu.rain_effect_mut(id)) {
            rain.rain_fade = (rain.rain_fade + 0.006).min(0.3);
        }
        if self.restart_available {
            if let Some(b) = menu.behavior_mut(self.restart_checkbox) {
                b.greyed_out = false;
            }
            menu.set_check_box_selectable(self.restart_checkbox, true);
        } else {
            if let Some(b) = menu.behavior_mut(self.restart_checkbox) {
                b.greyed_out = true;
            }
            menu.set_check_box_selectable(self.restart_checkbox, false);
            // `restartCheckbox.Checked = false` reports to `SetChecked`.
            self.set_restart_checked(menu, false);
            self.restart_checked = false;
            let saved = self.saved(self.slugcat_page_index).is_some();
            self.restart_up = if saved { 1.0 } else { 0.0 };
            self.restart_available = saved;
        }
        if let Some(h) = menu.hold_button_mut(self.start_button) {
            h.warning_mode = self.restart_checked;
        }
        menu.object_mut(self.restart_checkbox).pos.y = (-50.0_f32).lerp(30.0, self.restart_up);
        let pause = cx.shared.input.pause_button();
        if pause && !self.last_pause_button {
            cx.requests.switch_to(ProcessId::MainMenu);
            cx.play_sound(MenuSoundId::SwitchPageOut);
        }
        self.last_pause_button = pause;
        self.last_scroll = self.scroll;
        self.scroll = self.next_scroll();
        let slugcat = Self::ORDER[self.slugcat_page_index];
        let saved = self.saved(self.slugcat_page_index).is_some();
        let greyed = self.scroll.abs() > 0.1 || !Self::slugcat_unlocked(slugcat, cx.shared, saved);
        if let Some(b) = menu.behavior_mut(self.start_button) {
            b.greyed_out = greyed;
        }
        if self.last_scroll.abs() > 0.5 && self.scroll.abs() <= 0.5 {
            self.update_start_button_text(menu);
        }
        if self.scroll != 0.0 || self.last_scroll != 0.0 {
            return;
        }
        let count = self.pages.len();
        if self.qued_side_input < 0 {
            self.qued_side_input += 1;
            self.slugcat_page_index = (self.slugcat_page_index + count - 1) % count;
            self.update_selected_slugcat(cx.shared);
            self.scroll = 1.0;
            self.last_scroll = 1.0;
            self.restart_available = false;
        } else if self.qued_side_input > 0 {
            self.qued_side_input -= 1;
            self.slugcat_page_index = (self.slugcat_page_index + 1) % count;
            self.update_selected_slugcat(cx.shared);
            self.scroll = -1.0;
            self.last_scroll = -1.0;
            self.restart_available = false;
        }
    }

    fn set_restart_checked(&mut self, menu: &mut Menu, checked: bool) {
        self.restart_checked = checked;
        self.update_start_button_text(menu);
    }

    /// `SlugcatPageContinue.Update` after `SlugcatPage.Update`: the HUD's update, then
    /// both meters faded in within half a page of the centre and placed under the
    /// region label.
    fn continue_page_update(&mut self, menu: &mut Menu, i: usize, cx: &mut MenuCx<'_>) {
        let PageKind::Continue(c) = &mut self.pages[i].kind else {
            return;
        };
        let shared = &mut *cx.shared;
        c.hud
            .update(&mut menu.stage, &mut shared.mic, &mut shared.random);
        let fade = self.page_next_scroll(i).abs().inverse_lerp(0.5, 0.0);
        let PageKind::Continue(c) = &self.pages[i].kind else {
            return;
        };
        let pos = self.karma_symbol_pos(i, &c.hud);
        if let PageKind::Continue(c) = &mut self.pages[i].kind {
            c.hud.place(fade, pos);
        }
    }

    /// `SlugcatPage.GrafUpdate`'s mark and glow, page by page.
    fn draw_lights(&self, menu: &mut Menu, t: f32, cosmetic: &mut UnityRandom) {
        for (i, p) in self.pages.iter().enumerate() {
            let cam_pos = match &menu.object(p.scene).kind {
                Kind::Scene(s) => s.cam_pos_at(t),
                _ => Vec2::ZERO,
            };
            let mark_fade = match self.saved(i) {
                Some(d) if p.slugcat == Slugcat::Red => {
                    (d.cycle as f32).inverse_lerp(4.0, 14.0).mathf_pow(3.5)
                }
                _ if p.slugcat == Slugcat::Red => 0.0,
                _ => 1.0,
            };
            let frame = LightsFrame {
                t,
                x: self.screen_width / 2.0 + self.page_scroll(i, t) * SlugcatPage::SCROLL_MAGNITUDE,
                image_y: p.image_pos.y,
                alpha: self.use_alpha(i, t),
                cam_pos,
                in_position: p.last_in_position.lerp(p.in_position, t),
                mark_fade,
            };
            p.lights.draw(&mut menu.stage, &frame, cosmetic);
        }
    }

    /// `SlugcatPageContinue.GrafUpdate`'s `hud.Draw` for every continue page.
    fn draw_huds(&mut self, menu: &mut Menu, t: f32) {
        for page in &mut self.pages {
            if let PageKind::Continue(c) = &mut page.kind {
                c.hud.draw(&mut menu.stage, t);
            }
        }
    }
}

impl MenuScreen for SlugcatSelectScreen {
    fn signal(&mut self, _menu: &mut Menu, cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        match message {
            "BACK" => {
                cx.requests.switch_to(ProcessId::MainMenu);
                cx.play_sound(MenuSoundId::SwitchPageOut);
            }
            "START" => {
                let slugcat = self.pages[self.slugcat_page_index].slugcat;
                self.start_game(slugcat, cx);
            }
            "PREV" => {
                self.qued_side_input = (self.qued_side_input - 1).max(-3);
                cx.play_sound(MenuSoundId::NextSlugcat);
            }
            "NEXT" => {
                self.qued_side_input = (self.qued_side_input + 1).min(3);
                cx.play_sound(MenuSoundId::NextSlugcat);
            }
            _ => {}
        }
    }

    fn init(&mut self, menu: &mut Menu) {
        menu.selected = Some(self.start_button);
    }

    /// `SlugcatPage.Update` (the mark and glow draws), then a continue page's own.
    fn page_update(&mut self, menu: &mut Menu, page: ObjectId, cx: &mut MenuCx<'_>) {
        let Some(i) = self.page_index(page) else {
            return;
        };
        let alpha = self.use_alpha(i, 1.0);
        let scroll = self.page_scroll(i, 1.0);
        if let Some(s) = menu.scene_mut(self.pages[i].scene) {
            s.camera_range = 0.5 * alpha;
        }
        let target = (1.0 - scroll.abs()).inverse_lerp(0.5, 1.0).powi(5);
        let p = &mut self.pages[i];
        p.last_in_position = p.in_position;
        if target < p.in_position {
            p.in_position = target;
        } else {
            p.in_position = p.in_position.lerp_and_tick(target, 0.01, 1.0 / 300.0);
        }
        let in_position = p.in_position;
        p.lights.update(alpha, in_position, &mut cx.shared.random);
        self.continue_page_update(menu, i, cx);
    }

    /// `SlugcatPage.GrafUpdate` and the subclass's: the scene's layers (less the
    /// camera over their depth) or flat image and the labels slide with the scroll and
    /// fade away from the centre (the region label is medium grey). The mark, the glow
    /// and the continue pages' HUDs are drawn after the menu (see
    /// [`SlugcatSelectMenu::graf_update`]).
    fn page_graf_update(&self, menu: &mut Menu, page: ObjectId, t: f32) {
        let Some(i) = self.page_index(page) else {
            return;
        };
        let p = &self.pages[i];
        let scroll = self.page_scroll(i, t);
        let alpha = self.use_alpha(i, t);
        let (layers, flats, cam_pos) = match &menu.object(p.scene).kind {
            Kind::Scene(s) => (
                s.depth_illustrations.clone(),
                s.flat_illustrations.clone(),
                s.cam_pos_at(t),
            ),
            _ => (Vec::new(), Vec::new(), Vec2::ZERO),
        };
        for layer in layers {
            let pos = menu.object(layer).pos;
            let Kind::Illustration(il) = &menu.object(layer).kind else {
                continue;
            };
            let depth = il.depth.map_or(1.0, |d| d.depth);
            let (node, il_alpha) = (il.sprite, il.alpha);
            let pos = pos - cam_pos * 80.0 / depth + p.scene_offset;
            let tr = menu.stage.transform_mut(node);
            tr.x = pos.x + scroll * SlugcatPage::SCROLL_MAGNITUDE;
            tr.y = pos.y;
            tr.alpha = il_alpha * alpha;
        }
        for flat in flats {
            let pos = menu.object(flat).pos + p.scene_offset;
            let Kind::Illustration(il) = &menu.object(flat).kind else {
                continue;
            };
            let (node, il_alpha) = (il.sprite, il.alpha);
            let tr = menu.stage.transform_mut(node);
            tr.x = pos.x + scroll * SlugcatPage::SCROLL_MAGNITUDE;
            tr.y = pos.y;
            tr.alpha = il_alpha * alpha;
        }
        let x = self.screen_width / 2.0 + scroll * SlugcatPage::SCROLL_MAGNITUDE + 0.01;
        let (labels, color) = match &p.kind {
            PageKind::NewGame {
                difficulty_label,
                info_label,
            } => (vec![*difficulty_label, *info_label], None),
            PageKind::Continue(c) => (vec![c.region_label], Some(MenuColor::MediumGrey)),
        };
        for label in labels {
            if let Kind::Label(l) = &menu.object(label).kind {
                let node = l.label;
                let stage_label = menu.stage.label_mut(node);
                stage_label.transform.alpha = alpha;
                stage_label.transform.x = x;
                if let Some(color) = color {
                    stage_label.color = color.rgb();
                }
            }
        }
    }

    fn checked(&self, _menu: &Menu, _check_box: ObjectId) -> bool {
        self.restart_checked
    }

    fn set_checked(&mut self, menu: &mut Menu, _check_box: ObjectId, checked: bool) {
        self.set_restart_checked(menu, checked);
    }
}

impl SlugcatSelectMenu {
    /// `new SlugcatSelectMenu(manager)`.
    #[must_use]
    pub fn new(shared: &mut MenuShared) -> Self {
        let mut menu = Menu::new(ProcessId::SlugcatSelect, shared);
        let page0 = menu.add_page(shared, "main", 0);
        let rain_effect = (!shared.flat_illustrations).then(|| {
            let rain = menu.add_rain_effect(page0, &mut shared.random);
            menu.add_sub(page0, rain);
            rain
        });
        if shared.unlock_red_file() {
            shared.unlock_red();
        }
        let order = SlugcatSelectScreen::ORDER;
        let mut save_game_data: Vec<Option<SaveGameData>> = order
            .iter()
            .map(|&s| SlugcatSelectScreen::mine_for_save_data(s, shared))
            .collect();
        let red_is_dead = order
            .iter()
            .zip(&save_game_data)
            .find(|(s, _)| **s == Slugcat::Red)
            .and_then(|(_, d)| d.as_ref())
            .is_some_and(|d| {
                (d.reds_death && d.cycle >= SaveState::reds_cycles(d.reds_extra_cycles))
                    || d.ascended
            });
        // `SetSlugcatColorOrder`: the selected slugcat's page, else the Survivor's.
        let mut slugcat_page_index = 0;
        for (i, slugcat) in order.iter().enumerate() {
            if *slugcat == shared.progression.selected_slugcat {
                slugcat_page_index = i;
                break;
            }
            if *slugcat == Slugcat::White {
                slugcat_page_index = i;
            }
        }
        // With a single save, its page.
        let saved: Vec<usize> = (0..order.len())
            .filter(|&i| save_game_data[i].is_some())
            .collect();
        if let [only] = saved[..] {
            slugcat_page_index = only;
        }
        let screen_width = shared.screen_size[0];
        let pages = order
            .iter()
            .enumerate()
            .map(|(j, &slugcat)| match &mut save_game_data[j] {
                Some(data) => {
                    Self::add_continue_page(&mut menu, shared, 1 + j, slugcat, data, red_is_dead)
                }
                None => Self::add_new_game_page(&mut menu, shared, 1 + j, slugcat, false),
            })
            .collect();
        let start_button =
            menu.add_hold_button(shared, page0, "", "START", Vec2::new(683.0, 85.0), 40.0);
        menu.add_sub(page0, start_button);
        let back_text = shared.translator.translate("BACK").to_owned();
        let back_button = menu.add_simple_button(
            shared,
            page0,
            &back_text,
            "BACK",
            Vec2::new(200.0, 668.0),
            Vec2::new(110.0, 30.0),
        );
        menu.add_sub(page0, back_button);
        menu.back_object = Some(back_button);
        let prev_button = menu.add_big_arrow_button(page0, "PREV", Vec2::new(200.0, 50.0), -1);
        menu.add_sub(page0, prev_button);
        let next_button = menu.add_big_arrow_button(page0, "NEXT", Vec2::new(1116.0, 50.0), 1);
        menu.add_sub(page0, next_button);
        // `GetRestartTextWidth` / `GetRestartTextOffset` for English; the safe screen
        // offset is zero.
        let (text_width, text_offset) = (85.0, 0.0);
        let restart_text = shared.translator.translate("Restart game").to_owned();
        let restart_checkbox = menu.add_check_box(
            shared,
            page0,
            Vec2::new(683.0 + 200.0 + text_offset, 30.0_f32.max(0.0)),
            text_width,
            &restart_text,
            "RESTART",
            false,
        );
        let label = menu.check_box_label(restart_checkbox);
        if let Some(label) = label {
            let width = menu.label_text_width(label);
            menu.object_mut(label).pos.x += text_width - width - 5.0;
        }
        menu.add_sub(page0, restart_checkbox);
        let screen = SlugcatSelectScreen {
            pages,
            slugcat_page_index,
            scroll: 0.0,
            last_scroll: 0.0,
            qued_side_input: 0,
            start_button,
            back_button,
            prev_button,
            next_button,
            restart_checkbox,
            restart_checked: false,
            restart_available: false,
            restart_up: 0.0,
            last_pause_button: false,
            screen_width,
            save_game_data,
            red_is_dead,
            rain_effect,
            texts: StartTexts {
                new_game: shared.translator.translate("NEW GAME").to_owned(),
                delete_save: shared.translator.translate("DELETE SAVE").to_owned(),
                statistics: shared.translator.translate("STATISTICS").to_owned(),
                continue_game: shared.translator.translate("CONTINUE").to_owned(),
            },
        };
        screen.update_start_button_text(&mut menu);
        screen.update_selected_slugcat(shared);
        menu.my_sound_loop = Some(MenuSoundId::MainMenuLoop);
        Self {
            menu,
            screen,
            cosmetic: UnityRandom::new(0),
        }
    }

    /// `new SlugcatPage(...)` and `AddImage`: the page, its scene (with `AddGlow`'s
    /// sprites among the layers) and the mark's sprites.
    fn add_page_and_image(
        menu: &mut Menu,
        shared: &mut MenuShared,
        index: usize,
        slugcat: Slugcat,
        ascended: bool,
        red_is_dead: bool,
        mut lights: PageLights,
    ) -> (ObjectId, ObjectId, Vec2, PageLights) {
        let name = format!("Slugcat_Page_{}", slugcat.value());
        let page = menu.add_page(shared, &name, index);
        let (scene_id, mut scene_offset) = SlugcatPage::image(slugcat, ascended, red_is_dead);
        scene_offset.x -= (1366.0 - shared.screen_size[0]) / 2.0;
        let (scene, glow) = menu.add_page_scene(shared, page, scene_id, lights.has_glow);
        lights.glow = glow;
        menu.add_sub(page, scene);
        let container = menu.container_of(page);
        lights.add_mark(&mut menu.stage, container);
        (page, scene, scene_offset, lights)
    }

    /// `new SlugcatPageNewGame(menu, null, pageIndex, slugcat)`.
    fn add_new_game_page(
        menu: &mut Menu,
        shared: &mut MenuShared,
        index: usize,
        slugcat: Slugcat,
        saved: bool,
    ) -> SlugcatPage {
        // `AddImage(ascended: false)`; no save, so the Hunter is not dead, and no mark
        // or glow.
        let lights = PageLights::new(slugcat, false, false);
        let (page, scene, mut scene_offset, lights) =
            Self::add_page_and_image(menu, shared, index, slugcat, false, false, lights);
        let mut image_pos = Vec2::new(683.0, 484.0);
        let (text, info) = SlugcatPage::texts(slugcat, shared);
        let lines = info.matches('\n').count();
        let extra = if lines > 1 { 30.0 } else { 0.0 };
        let difficulty_label = menu.add_label(
            shared,
            page,
            &text,
            Vec2::new(-1000.0, image_pos.y - 249.0 + extra),
            Vec2::new(200.0, 30.0),
            true,
        );
        menu.add_sub(page, difficulty_label);
        let info_label = menu.add_label(
            shared,
            page,
            &info,
            Vec2::new(-1000.0, image_pos.y - 249.0 - 60.0 + extra / 2.0),
            Vec2::new(400.0, 60.0),
            true,
        );
        menu.add_sub(page, info_label);
        if lines > 1 {
            image_pos.y += 30.0;
            scene_offset.y += 30.0;
        }
        let (name_color, info_color) =
            if SlugcatSelectScreen::slugcat_unlocked(slugcat, shared, saved) {
                (MenuColor::MediumGrey, MenuColor::DarkGrey)
            } else {
                (MenuColor::VeryDarkGrey, MenuColor::VeryDarkGrey)
            };
        for (label, color) in [(difficulty_label, name_color), (info_label, info_color)] {
            if let Kind::Label(l) = &menu.object(label).kind {
                let node = l.label;
                menu.stage.label_mut(node).color = color.rgb();
            }
        }
        SlugcatPage {
            page,
            slugcat,
            scene,
            image_pos,
            scene_offset,
            in_position: 0.0,
            last_in_position: 0.0,
            lights,
            kind: PageKind::NewGame {
                difficulty_label,
                info_label,
            },
        }
    }

    /// `new SlugcatPageContinue(menu, null, pageIndex, slugcat)`: the image (a ghost
    /// after ascending, the dead Hunter's when its save ended), two HUD containers, the
    /// HUD with the karma (clamped to the cap) and the food (clamped to the meter),
    /// then the region label. The clamps write back to the save data, as the game's
    /// `saveGameData` is the menu's.
    fn add_continue_page(
        menu: &mut Menu,
        shared: &mut MenuShared,
        index: usize,
        slugcat: Slugcat,
        data: &mut SaveGameData,
        red_is_dead: bool,
    ) -> SlugcatPage {
        // `HasMark` and `HasGlow`: not after ascending, in flat mode or for the dead
        // Hunter.
        let lit = !data.ascended
            && !shared.flat_illustrations
            && !(slugcat == Slugcat::Red && red_is_dead);
        let lights = PageLights::new(slugcat, lit && data.has_mark, lit && data.has_glow);
        let (page, scene, scene_offset, lights) = Self::add_page_and_image(
            menu,
            shared,
            index,
            slugcat,
            data.ascended,
            red_is_dead,
            lights,
        );
        let image_pos = Vec2::new(683.0, 484.0);
        let page_container = menu.container_of(page);
        let containers = [0, 1].map(|_| menu.stage.add_container(page_container));
        data.karma = data.karma.clamp(0, data.karma_cap.max(0));
        let stats = SlugcatCampaign::of(slugcat, rw_assets::ActiveMods::NONE)
            .map_or((7, 4), |c| (c.stats.max_food, c.stats.food_to_hibernate));
        data.food = data.food.clamp(0, stats.0);
        let karma = if data.ripple_level >= 1.0 {
            (((data.ripple_level - 1.0) * 2.0) as i32, 100)
        } else {
            (data.karma, data.karma_cap)
        };
        let hud = SelectHud::new(
            SelectHudSetup {
                container: containers[1],
                karma,
                karma_reinforced: data.karma_reinforced,
                food_meter: stats,
                current_food: data.food,
            },
            &mut menu.stage,
        );
        let text = data.region_text(slugcat, shared);
        let region_label = menu.add_label(
            shared,
            page,
            &text,
            Vec2::new(-1000.0, image_pos.y - 249.0),
            Vec2::new(200.0, 30.0),
            true,
        );
        menu.add_sub(page, region_label);
        SlugcatPage {
            page,
            slugcat,
            scene,
            image_pos,
            scene_offset,
            in_position: 0.0,
            last_in_position: 0.0,
            lights,
            kind: PageKind::Continue(Box::new(ContinuePage { hud, region_label })),
        }
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    #[must_use]
    pub fn screen(&self) -> &SlugcatSelectScreen {
        &self.screen
    }
}

impl<C: MenuContext> MainLoopProcess<C> for SlugcatSelectMenu {
    fn id(&self) -> ProcessId {
        ProcessId::SlugcatSelect
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        let mut cx = MenuCx {
            shared: ctx.menu(),
            requests,
        };
        self.menu.update(&mut self.screen, &mut cx);
        self.screen.update(&mut self.menu, &mut cx);
    }

    /// `GrafUpdate`; the pages' mark and glow and the continue pages' `hud.Draw` come
    /// after the menu's objects, which is the game's result as they write only their
    /// own sprites (the mark and glow draw from the cosmetic stream, D1).
    fn graf_update(&mut self, time_stacker: f32, ctx: &mut C) {
        let mouse = Vec2::from(ctx.menu().input.mouse.pos);
        self.menu.graf_update(&self.screen, time_stacker, mouse);
        self.screen
            .draw_lights(&mut self.menu, time_stacker, &mut self.cosmetic);
        self.screen.draw_huds(&mut self.menu, time_stacker);
    }

    fn shut_down(&mut self, ctx: &mut C) {
        self.menu.shut_down(ctx.menu());
    }
}

#[cfg(test)]
mod tests;
