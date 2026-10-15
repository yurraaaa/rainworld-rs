//! The in-game HUD of a single-player story game (`HUD.HUD.InitSinglePlayerHud`): the
//! text prompt, food meter, karma meter, rain meter, the region map ([`Map`]), the
//! tutorial controls page SU_C04's start-up script shows, and the pause menu over it.
//! It reads the game through [`OwnerTick`] and
//! [`RainCycleView`], makes the game's HUD sounds and random draws through the sim, and
//! draws into its own Futile stage (`rw_menu::StageTree`), which the app draws above
//! the room. Behaviour: `rainworld-re/notes/hud.md`.

mod circle;
mod controls_page;
mod food_meter;
mod karma_ladder;
mod karma_meter;
mod map;
mod pause_menu;
mod rain_meter;
mod screen_package;
mod select_hud;
mod sleep_hud;
mod sleep_screen;
mod slugcat_select;
mod text_prompt;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use rw_menu::{AtlasManager, Font, MenuMic, MenuSoundId, NodeId, StageTree, Translator};
use rw_random::UnityRandom;
use rw_render::StageBatch;
use rw_sim::{FloatExt, MapDiscovery, SoundId, Sounds, Vec2};

pub use circle::{FADABLE_COLORS, FadeCircle, HudCircle, SnapGraphic};
pub use controls_page::{ControlMap, ControlsPage, IllustrationSource, PageText};
pub use food_meter::{CIRCLE_DISTANCE, FoodMeter, MeterCircle, ScreenTick};
pub use karma_ladder::{KarmaLadder, KarmaSymbol, LadderIo, LadderMeter, LadderPhase};
pub use karma_meter::{KarmaMeter, karma_symbol_sprite};
pub use map::{
    ConnectionView, CycleLabel, FadeInMarker, GateData, GateRequirement, Map, MapConnection,
    MapData, MapDataRoom, MapOwner, MarkerKind, ShelterData, SwarmRoomData,
};
pub use pause_menu::{PauseButton, PauseGame, PauseMenu, PauseRequest};
pub use rain_meter::{RainCycleView, RainMeter};
pub use screen_package::{
    GameOverPrompt, PlayerAt, RealizedAt, ScreenPackageSource, SleepDeathScreenDataPackage,
};
pub use select_hud::{SelectHud, SelectHudSetup};
pub use sleep_hud::{SleepHud, SleepHudSetup};
pub use sleep_screen::{ScreenButton, SleepAndDeathScreen};
pub use slugcat_select::{
    ContinuePage, PageKind, SaveGameData, SlugcatPage, SlugcatSelectMenu, SlugcatSelectScreen,
};
pub use text_prompt::{InfoId, Message, SubregionTracker, Subregions, TextPrompt};

use food_meter::MeterNeighbours;
use karma_meter::KarmaNeighbours;
use rain_meter::RainNeighbours;
use text_prompt::PromptNeighbours;

/// `RWCustom.Custom` and `Mathf` helpers the HUD needs beyond `rw_sim::FloatExt`.
pub trait CustomFloat {
    /// `Custom.SCurve`.
    fn s_curve(self, k: f32) -> f32;
    /// `Mathf.RoundToInt` (halves to even).
    fn round_to_int(self) -> i32;
    /// `Mathf.Sign`: 1 for zero (either sign) and up, else -1.
    fn mathf_sign(self) -> f32;
}

impl CustomFloat for f32 {
    fn s_curve(self, k: f32) -> f32 {
        let x = self * 2.0 - 1.0;
        if x < 0.0 {
            let x = (1.0 + x).abs();
            return k * x / (k - x + 1.0) * 0.5;
        }
        let k = -1.0 - k;
        0.5 + k * x / (k - x + 1.0) * 0.5
    }

    fn round_to_int(self) -> i32 {
        self.round_ties_even() as i32
    }

    fn mathf_sign(self) -> f32 {
        if self >= 0.0 { 1.0 } else { -1.0 }
    }
}

/// `RWCustom.Custom` helpers on positions.
pub trait CustomVec2 {
    /// `Custom.CirclesCollisionTime(x1, y1, x2, y2, vx1, vy1, r1, r2)` with `self` the
    /// first circle's position: the fraction of `vel` the first circle moves before it
    /// touches the second at `other` (NaN when it never does).
    fn circles_collision_time(self, other: Vec2, vel: Vec2, r1: f32, r2: f32) -> f32;
}

impl CustomVec2 for Vec2 {
    fn circles_collision_time(self, other: Vec2, vel: Vec2, r1: f32, r2: f32) -> f32 {
        let (x1, y1, x2, y2, vx, vy) = (self.x, self.y, other.x, other.y, vel.x, vel.y);
        let (e, f) = (0.001_f32, 0.001_f32);
        let b = -x1 * vx - y1 * vy + vx * x2 + vy * y2 + x1 * e - x2 * e + y1 * f - y2 * f;
        let a = vx.mathf_pow(2.0) + vy.mathf_pow(2.0) - 2.0 * vx * e + e.mathf_pow(2.0)
            - 2.0 * vy * f
            + f.mathf_pow(2.0);
        let c = x1.mathf_pow(2.0) + y1.mathf_pow(2.0) - r1.mathf_pow(2.0) - 2.0 * x1 * x2
            + x2.mathf_pow(2.0)
            - 2.0 * y1 * y2
            + y2.mathf_pow(2.0)
            - 2.0 * r1 * r2
            - r2.mathf_pow(2.0);
        (2.0 * b - ((-2.0 * b).mathf_pow(2.0) - 4.0 * a * c).sqrt()) / (2.0 * a)
    }
}

/// `Mathf.Clamp` and `Custom.IntClamp` on ints: the lower bound wins over the upper
/// one instead of panicking when they cross.
pub trait CustomInt {
    fn int_clamp(self, min: i32, max: i32) -> i32;
}

impl CustomInt for i32 {
    fn int_clamp(self, min: i32, max: i32) -> i32 {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

/// `manager.menuSetup.startGameCondition` as the subregion tracker reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartCondition {
    New,
    Load,
    /// The dev start (a game started on a room).
    Dev,
}

/// What the HUD reads of its owner, the player, each tick (`IOwnAHUD` and `Player`).
#[derive(Debug, Clone, Default)]
pub struct OwnerTick {
    /// `RevealMap`: the map button in `input[0]` (`inVoidSea` is More Slugcats).
    pub reveal_map: bool,
    /// `CurrentFood` (`FoodInStomach`).
    pub current_food: i32,
    /// `playerState.quarterFoodPoints`.
    pub quarter_food_points: i32,
    pub show_karma_food_rain_time: i32,
    /// The player's room (`room.abstractRoom.name`); `None` while it has none (in a
    /// shortcut, destroyed).
    pub room: Option<String>,
    /// In a shelter whose `shelterDoor` is not broken.
    pub in_intact_shelter: bool,
    pub still_in_start_shelter: bool,
    pub ready_for_win: bool,
    pub force_sleep_counter: i32,
    /// The pose `FoodMeter.GameUpdate` lowers `forceSleep` faster in: in a shelter, not in
    /// a shortcut, holding down and nothing else on solid ground.
    pub force_sleep_pose: bool,
    /// `room.roomSettings.DangerType != None`.
    pub room_rain_danger: bool,
    pub map: MapOwner,
}

/// `HUD.OwnerType`: what owns a HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudOwnerType {
    Player,
    SleepScreen,
    /// The death and starve screens.
    DeathScreen,
    FastTravelScreen,
    WarpFastTravelScreen,
    WarpSafariMenu,
    RegionOverview,
    ArenaSession,
    CharacterSelect,
    /// More Slugcats.
    SafariOverseer,
}

impl HudOwnerType {
    pub const ALL: [Self; 10] = [
        Self::Player,
        Self::SleepScreen,
        Self::DeathScreen,
        Self::FastTravelScreen,
        Self::WarpFastTravelScreen,
        Self::WarpSafariMenu,
        Self::RegionOverview,
        Self::ArenaSession,
        Self::CharacterSelect,
        Self::SafariOverseer,
    ];

    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::SleepScreen => "SleepScreen",
            Self::DeathScreen => "DeathScreen",
            Self::FastTravelScreen => "FastTravelScreen",
            Self::WarpFastTravelScreen => "WarpFastTravelScreen",
            Self::WarpSafariMenu => "WarpSafariMenu",
            Self::RegionOverview => "RegionOverview",
            Self::ArenaSession => "ArenaSession",
            Self::CharacterSelect => "CharacterSelect",
            Self::SafariOverseer => "SafariOverseer",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.value() == value)
    }
}

/// Where `PlayHUDSound` goes: the player's room microphone, or a menu screen's
/// `PlaySound`.
pub enum HudSounds<'a> {
    Game(&'a mut Sounds),
    Menu(&'a mut MenuMic),
}

/// Where the HUD's parts make sounds, fade circles and random draws while updating.
pub struct HudFx<'a> {
    pub stage: &'a mut StageTree,
    pub container: NodeId,
    pub fade_circles: &'a mut Vec<FadeCircle>,
    pub sounds: HudSounds<'a>,
    pub rng: &'a mut UnityRandom,
}

impl HudFx<'_> {
    /// `HUD.PlaySound`: the owner's `PlayHUDSound`, a sound at pan 0, volume and pitch
    /// 1 (disembodied in a room).
    pub fn play_sound(&mut self, id: SoundId) {
        match &mut self.sounds {
            HudSounds::Game(sounds) => sounds.play_disembodied(id, 0.0, 1.0, 1.0, self.rng),
            HudSounds::Menu(mic) => match MenuSoundId::from_name(id.name()) {
                Some(id) => mic.play_sound(id, [0.0, 1.0, 1.0], self.rng),
                None => log::warn!("{} is not a menu sound", id.name()),
            },
        }
    }
}

/// What the HUD is built from (`InitSinglePlayerHud`, `FireUpSinglePlayerHUD`).
#[derive(Debug, Clone)]
pub struct HudSetup {
    /// `rainWorld.screenSize`.
    pub screen_size: [f32; 2],
    /// The player's `slugcatStats`.
    pub max_food: i32,
    pub food_to_hibernate: i32,
    pub current_food: i32,
    /// The save state's karma, cap and `reinforcedKarma`.
    pub karma: i32,
    pub karma_cap: i32,
    pub karma_reinforced: bool,
    /// `saveState.cycleNumber`: from cycle 1 the meters show for 200 ticks at start.
    pub cycle_number: i32,
    /// The camera's room is a shelter: the meters start fully shown.
    pub camera_room_shelter: bool,
    pub rain: RainCycleView,
    /// The world's region, for the subregion tracker (`None` without a region).
    pub subregions: Option<Subregions>,
    pub start: StartCondition,
    /// `new MapData(world)` with the map's files; `None` without a region.
    pub map: Option<MapData>,
    /// The Hunter's map cycle label: its text and whether the cycles ran out.
    pub cycle_label: Option<(String, bool)>,
}

/// The HUD of the player camera.
pub struct Hud {
    stage: StageTree,
    /// `fContainers`: "HUD" and "HUD2" (all the ported parts draw in the second).
    containers: [NodeId; 2],
    screen_size: [f32; 2],
    start: StartCondition,
    cycle_number: i32,
    save_karma: (i32, i32),
    save_reinforced: bool,
    pub text_prompt: TextPrompt,
    pub food_meter: FoodMeter,
    pub karma_meter: KarmaMeter,
    pub map: Map,
    pub rain_meter: RainMeter,
    pub fade_circles: Vec<FadeCircle>,
    pub show_karma_food_rain: bool,
    pub controls_page: Option<ControlsPage>,
    /// The owner's `forceSleepCounter` last tick, for drawing.
    force_sleep_counter: i32,
}

impl Hud {
    /// `FireUpSinglePlayerHUD`: the parts in `InitSinglePlayerHud`'s order (text
    /// prompt, food meter, karma meter, map, rain meter), shown at once in a shelter and
    /// for 200 ticks after cycle 0.
    #[must_use]
    pub fn new(mut setup: HudSetup, atlases: &mut AtlasManager) -> Self {
        let mut stage = StageTree::new();
        let containers = [
            stage.add_container(StageTree::ROOT),
            stage.add_container(StageTree::ROOT),
        ];
        let c = containers[1];
        let tracker = setup.subregions.clone().map(SubregionTracker::new);
        let display_font: Option<Arc<Font>> = atlases.font(Font::DISPLAY_FONT).cloned();
        let text_prompt = TextPrompt::new(
            &mut stage,
            c,
            display_font.as_ref(),
            setup.screen_size,
            tracker,
        );
        let food_meter = FoodMeter::new(
            &mut stage,
            c,
            setup.max_food,
            setup.food_to_hibernate,
            setup.current_food,
        );
        let karma_meter = KarmaMeter::new(
            &mut stage,
            c,
            setup.karma,
            setup.karma_cap,
            setup.karma_reinforced,
        );
        let in_front = stage.add_container(StageTree::ROOT);
        let label = setup
            .cycle_label
            .take()
            .zip(display_font)
            .map(|((text, red), font)| (font, text, red));
        let map = Map::new(
            setup.map.take().unwrap_or_else(MapData::empty),
            HudOwnerType::Player,
            &mut stage,
            containers[0],
            in_front,
            atlases,
            setup.screen_size,
            label,
        );
        let rain_meter = RainMeter::new(&mut stage, c, &setup.rain);
        let mut hud = Self {
            stage,
            containers,
            screen_size: setup.screen_size,
            start: setup.start,
            cycle_number: setup.cycle_number,
            save_karma: (setup.karma, setup.karma_cap),
            save_reinforced: setup.karma_reinforced,
            text_prompt,
            food_meter,
            karma_meter,
            map,
            rain_meter,
            fade_circles: Vec::new(),
            show_karma_food_rain: false,
            controls_page: None,
            force_sleep_counter: 0,
        };
        if setup.camera_room_shelter {
            hud.karma_meter.fade = 1.0;
            hud.rain_meter.fade = 1.0;
            hud.food_meter.fade = 1.0;
        }
        if setup.cycle_number > 0 {
            hud.food_meter.visible_counter = 200;
            hud.karma_meter.force_visible_counter = 200;
            hud.rain_meter.remain_visible_counter = 200;
        }
        hud
    }

    /// `HUD.HideGeneralHud`.
    #[must_use]
    pub fn hide_general_hud(&self) -> bool {
        self.text_prompt.hide_general_hud()
    }

    fn prompt_neighbours(&self) -> PromptNeighbours {
        PromptNeighbours {
            food_fade: self.food_meter.fade,
            food_pos_y: self.food_meter.pos.y,
            karma_rad: self.karma_meter.rad,
            karma_fade: self.karma_meter.fade,
        }
    }

    /// `HUD.Update`: the parts newest first (rain meter, map, karma meter, food meter,
    /// text prompt), then the fade circles, then `showKarmaFoodRain` for the next tick.
    /// `map_button` is the map button as the game-over prompt reads it.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        owner: &OwnerTick,
        map_button: bool,
        rain: &RainCycleView,
        translator: &Translator,
        sounds: &mut Sounds,
        rng: &mut UnityRandom,
        discovery: &mut MapDiscovery,
    ) {
        self.force_sleep_counter = owner.force_sleep_counter;
        let hide = self.hide_general_hud();
        let skfr = self.show_karma_food_rain;
        let karma_radius = KarmaMeter::radius_of(
            self.karma_meter.rad,
            self.karma_meter.show_as_reinforced,
            self.food_meter.force_sleep,
        );
        let rain_n = RainNeighbours {
            karma_pos: self.karma_meter.pos,
            karma_fade: self.karma_meter.fade,
            karma_radius,
            reveal_map: owner.reveal_map,
            show_karma_food_rain: skfr,
            hide_general_hud: hide,
            room_rain_danger: owner.room_rain_danger,
        };
        self.rain_meter
            .update(rain, &rain_n, &mut self.karma_meter.force_visible_counter);
        self.map.update(
            &owner.map,
            owner.reveal_map,
            hide,
            &mut self.stage,
            discovery,
            rng,
        );
        let prompt = self.prompt_neighbours();
        let karma_n = KarmaNeighbours {
            food_down_in_corner: self.food_meter.down_in_corner,
            food_fade: self.food_meter.fade,
            map_fade: self.map.fade,
            text_food_visible_mode_zero: self.text_prompt.food_visible_mode == 0.0,
            text_lower_border: self.text_prompt.lower_border_height(&prompt, 1.0),
            reveal_map: owner.reveal_map,
            show_karma_food_rain: skfr,
            hide_general_hud: hide,
            character_select: false,
            save_karma: self.save_karma,
            save_reinforced: self.save_reinforced,
        };
        let mut fx = HudFx {
            stage: &mut self.stage,
            container: self.containers[1],
            fade_circles: &mut self.fade_circles,
            sounds: HudSounds::Game(sounds),
            rng,
        };
        self.karma_meter.update(&karma_n, &mut fx);
        let food_n = MeterNeighbours {
            karma_pos: self.karma_meter.pos,
            karma_fade: self.karma_meter.fade,
            karma_rad: self.karma_meter.rad,
            karma_reinforced: self.karma_meter.show_as_reinforced,
            karma_any_visibility: self.karma_meter.any_visibility(owner.reveal_map, skfr),
            rain_fade: self.rain_meter.fade,
            show_karma_food_rain: skfr,
            hide_general_hud: hide,
        };
        self.food_meter.update(owner, &food_n, &mut fx);
        let prompt = PromptNeighbours {
            food_fade: self.food_meter.fade,
            food_pos_y: self.food_meter.pos.y,
            karma_rad: self.karma_meter.rad,
            karma_fade: self.karma_meter.fade,
        };
        self.text_prompt.update(
            &prompt,
            owner.room.as_deref(),
            owner.reveal_map,
            map_button,
            self.start,
            self.cycle_number,
            translator,
            &mut fx,
        );
        for i in (0..self.fade_circles.len()).rev() {
            if self.fade_circles[i].life <= 0.0 {
                let gone = self.fade_circles.remove(i);
                self.stage.remove_from_container(gone.circle.sprite);
            } else {
                self.fade_circles[i].update();
            }
        }
        self.show_karma_food_rain = owner.reveal_map
            || owner.show_karma_food_rain_time > 0
            || (owner.room.is_some() && owner.in_intact_shelter);
    }

    /// `HUD.Draw` (and the controls page's `GrafUpdate`, which its owner runs from
    /// `DrawSprites`): sets every sprite for time stacker `t`.
    pub fn draw(&mut self, t: f32) {
        self.map.draw(&mut self.stage, t);
        let prompt = self.prompt_neighbours();
        let food_label_x = self.food_meter.pos.x + self.food_meter.total_width(t);
        self.text_prompt
            .draw(&mut self.stage, &prompt, food_label_x, t);
        self.food_meter
            .draw(&mut self.stage, self.force_sleep_counter, t);
        self.karma_meter.draw(
            &mut self.stage,
            self.containers[1],
            self.food_meter.force_sleep,
            t,
        );
        self.rain_meter.draw(&mut self.stage, t);
        for c in self.fade_circles.iter().rev() {
            c.circle.draw(&mut self.stage, t);
        }
        if let Some(page) = &self.controls_page {
            page.draw(&mut self.stage, t);
        }
    }

    /// Hands textures the HUD changed (the map's reveal) to `atlases`; call before
    /// syncing them to the renderer.
    pub fn sync_textures(&mut self, atlases: &mut AtlasManager) {
        self.map.sync_textures(atlases);
    }

    /// The map shaders' globals at `t` between ticks.
    #[must_use]
    pub fn map_globals(&self, t: f32) -> rw_render::MapGlobals {
        self.map.globals(t)
    }

    /// Appends the HUD's quads, in draw order, to `batch` (stage pixels).
    pub fn push_quads(&self, atlases: &AtlasManager, batch: &mut StageBatch) {
        self.stage.push_quads(atlases, batch);
    }

    /// `TextPrompt.AddMessage`.
    pub fn add_message(&mut self, text: &str, wait: i32, time: i32, darken: bool, hide_hud: bool) {
        self.text_prompt
            .add_message(text, wait, time, darken, hide_hud);
    }

    /// The text prompt's label text.
    #[must_use]
    pub fn prompt_text(&self) -> &str {
        self.text_prompt.text(&self.stage)
    }

    /// `new TutorialControlsPageOwner(game)`: the page, on the HUD's second container.
    pub fn open_controls_page(&mut self, source: &mut IllustrationSource<'_>, text: &PageText<'_>) {
        let page = ControlsPage::new(
            &mut self.stage,
            self.containers[1],
            self.screen_size,
            source,
            text,
        );
        self.controls_page = Some(page);
    }

    /// The owner's `Update` (a room object, run in the room's update): the page's, with
    /// `want_to_continue` as the start-up script last set it and `pause_menu` whether
    /// the game's pause menu is open. Drops the page once it shut down.
    pub fn update_controls_page(&mut self, want_to_continue: bool, pause_menu: bool) {
        let map_fade = self.map.fade;
        let Some(page) = &mut self.controls_page else {
            return;
        };
        page.want_to_continue |= want_to_continue;
        page.update(&mut self.stage, map_fade, pause_menu);
        if page.done {
            self.controls_page = None;
        }
    }

    /// The controls page's label texts, if it is open.
    #[must_use]
    pub fn controls_page_texts(&self) -> Option<Vec<String>> {
        self.controls_page
            .as_ref()
            .map(|p| p.control_map.texts(&self.stage))
    }

    /// The stage, for tests and tools.
    #[must_use]
    pub fn stage(&self) -> &StageTree {
        &self.stage
    }

    /// The karma meter's symbol position this tick.
    #[must_use]
    pub fn karma_pos(&self) -> Vec2 {
        self.karma_meter.pos
    }
}
