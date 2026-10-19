//! `HUD.TextPrompt`: the black bars at the top and bottom of the screen with a line of
//! text, for messages (tutorials, subregion names), the game-over prompt and the
//! pause text, and the `SubregionTracker` that announces subregions. Symbol and music
//! messages and the Hunter's cycle count-up are not ported.
//!
//! Mirrors: HUD/TextPrompt.cs, HUD/SubregionTracker.cs

use std::f32::consts::PI;
use std::sync::Arc;

use rw_menu::{
    Alignment, AtlasManager, Font, Label, MenuColor, NodeId, Sprite, StageColor, StageTree,
    Translator,
};
use rw_sim::{FloatExt, SoundId};

use crate::{CustomFloat, HudFx, StartCondition};

/// `TextPrompt.Message`.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub text: String,
    pub wait: i32,
    pub time: i32,
    pub fade_out: bool,
    pub darken: bool,
    pub hide_hud: bool,
}

/// `TextPrompt.InfoID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoId {
    Nothing,
    Message,
    Paused,
    GameOver,
}

/// A region's subregions (`Region.subRegions` from `properties.txt`, index 0 the
/// unnamed one) and each room's (`subregionName` from the map file).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Subregions {
    /// The region's acronym (`world.region.name`).
    pub region: String,
    pub names: Vec<String>,
    /// Room name (upper case) to subregion name.
    pub rooms: Vec<(String, String)>,
}

impl Subregions {
    /// The subregion index of `room` (`SubregionTracker.Update`'s search: 0 when the room
    /// has none or names one the region does not list).
    #[must_use]
    pub fn index_of(&self, room: &str) -> usize {
        let Some(name) = self.room_subregion(room) else {
            return 0;
        };
        self.names
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, n)| *n == name)
            .map_or(0, |(i, _)| i)
    }

    /// `room`'s `subregionName`.
    #[must_use]
    pub fn room_subregion(&self, room: &str) -> Option<&str> {
        self.rooms
            .iter()
            .find(|(r, _)| r.eq_ignore_ascii_case(room))
            .map(|(_, name)| name.as_str())
    }

    /// Reads `world/<region>/properties.txt` (`Subregion:` lines) and the room lines of
    /// `world/<region>/map_<region>.txt` (the sixth `><` field), preferring the
    /// timeline's `-<timeline>` variants of both as the game does.
    #[must_use]
    pub fn load(assets: &rw_assets::StreamingAssets, region: &str, timeline: &str) -> Self {
        let lower = region.to_lowercase();
        let mut names = vec![String::new()];
        let properties = Self::region_file(assets, &lower, "properties", timeline);
        if let Ok(text) = std::fs::read_to_string(properties) {
            names.extend(
                text.lines()
                    .filter_map(|l| l.trim_end().strip_prefix("Subregion: "))
                    .map(str::to_owned),
            );
        }
        let mut rooms = Vec::new();
        let map = Self::region_file(assets, &lower, &format!("map_{lower}"), timeline);
        if let Ok(text) = std::fs::read_to_string(map) {
            for line in text.lines() {
                let Some((room, rest)) = line.split_once(": ") else {
                    continue;
                };
                let fields: Vec<&str> = rest.split("><").collect();
                if let Some(sub) = fields.get(5).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    rooms.push((room.to_uppercase(), sub.to_owned()));
                }
            }
        }
        Self {
            region: region.to_uppercase(),
            names,
            rooms,
        }
    }

    /// `world/<region>/<base>-<timeline>.txt` if it exists, else `<base>.txt`.
    fn region_file(
        assets: &rw_assets::StreamingAssets,
        region: &str,
        base: &str,
        timeline: &str,
    ) -> std::path::PathBuf {
        let variant = assets.resolve_file_path(&format!("world/{region}/{base}-{timeline}.txt"));
        if variant.is_file() {
            variant
        } else {
            assets.resolve_file_path(&format!("world/{region}/{base}.txt"))
        }
    }
}

/// `HUD.SubregionTracker`.
#[derive(Debug, Clone)]
pub struct SubregionTracker {
    pub subregions: Subregions,
    pub last_region: usize,
    pub last_shown_region: usize,
    pub counter: i32,
    pub show_cycle_number: bool,
    dev_bool: bool,
}

impl SubregionTracker {
    #[must_use]
    pub fn new(subregions: Subregions) -> Self {
        Self {
            subregions,
            last_region: 0,
            last_shown_region: 0,
            counter: 0,
            show_cycle_number: true,
            dev_bool: false,
        }
    }

    /// `PreRegionBump`, read by event triggers.
    #[must_use]
    pub fn pre_region_bump(&self) -> bool {
        self.counter == 1
    }

    /// `RegionBump`.
    #[must_use]
    pub fn region_bump(&self) -> bool {
        self.counter == 75
    }

    /// `Update`: returns the message to add, if any. `room` is the owner's room
    /// (`None` in a shortcut); `show` is the prompt's.
    fn update(
        &mut self,
        room: Option<&str>,
        show: f32,
        start: StartCondition,
        cycle_number: i32,
        translator: &Translator,
    ) -> Option<String> {
        let num = room.map_or(0, |r| self.subregions.index_of(r));
        if !self.dev_bool && num != 0 && start == StartCondition::Dev {
            self.last_shown_region = num;
            self.dev_bool = true;
        }
        let mut message = None;
        if num != self.last_shown_region
            && room.is_some()
            && num != 0
            && self.last_region == num
            && show == 0.0
        {
            // No warp points in the base game.
            self.counter += 1;
            if self.counter > 80 {
                let region = self.subregions.region.as_str();
                if (num > 1 || self.last_shown_region == 0 || (region != "SS" && region != "DM"))
                    && num < self.subregions.names.len()
                {
                    let name = translator.translate(&self.subregions.names[num]);
                    message = Some(if self.show_cycle_number && start == StartCondition::Load {
                        format!("{} {cycle_number} ~ {name}", translator.translate("Cycle"))
                    } else {
                        name.to_owned()
                    });
                }
                self.show_cycle_number = false;
                self.last_shown_region = num;
            }
        } else {
            self.counter = 0;
        }
        self.last_region = num;
        message
    }
}

/// `HUD.TextPrompt`.
#[derive(Debug, Clone)]
pub struct TextPrompt {
    /// `None` without the display font.
    label: Option<NodeId>,
    fullscreen_fade: NodeId,
    sprites: [NodeId; 2],
    last_any_key: bool,
    restart_not_allowed: i32,
    pub game_over_mode: bool,
    pub play_game_over_sound: bool,
    /// `foodInStomach`, `deathRoom` and `deathPos`, set on entering game-over mode (0
    /// and zero before).
    pub food_in_stomach: i32,
    pub death_room: i32,
    pub death_pos: [f32; 2],
    pub paused_mode: bool,
    /// `pausedWarningText`: the pause text warns that quitting counts as a loss.
    pub paused_warning_text: bool,
    /// The game's `RedIsOutOfCycles` when the pause menu set the warning.
    pub red_is_out_of_cycles: bool,
    /// The game's `pauseMenu != null`, as the prompt reads it each update.
    pub pause_menu: bool,
    pub messages: Vec<Message>,
    pub show: f32,
    pub last_show: f32,
    pub pause_mode_height: f32,
    pub last_pause_mode_height: f32,
    pub hide_hud: bool,
    pub subregion_tracker: Option<SubregionTracker>,
    pub food_visible_mode: f32,
    pub last_food_visible_mode: f32,
    pub sin: f32,
    pub last_sin: f32,
    game_over_string: String,
    message_string: String,
    pub currently_showing: InfoId,
    /// `GoToDeathScreen` was asked for.
    pub go_to_death_screen: bool,
    screen_size: [f32; 2],
}

/// What the prompt reads of the food and karma meters.
#[derive(Debug, Clone, Copy)]
pub struct PromptNeighbours {
    pub food_fade: f32,
    pub food_pos_y: f32,
    pub karma_rad: f32,
    pub karma_fade: f32,
}

impl TextPrompt {
    /// The constructor: two black bars, the darkening sprite and the label, on
    /// `container`. The default keyboard bindings leave pause and map on their default
    /// keys, so the game-over text names Space.
    pub fn new(
        stage: &mut StageTree,
        container: NodeId,
        display_font: Option<&Arc<Font>>,
        screen_size: [f32; 2],
        subregion_tracker: Option<SubregionTracker>,
    ) -> Self {
        let sprites = [0, 1].map(|i| {
            let mut s = Sprite::new("pixel");
            s.color = StageColor::BLACK;
            s.transform.scale_x = 1400.0;
            s.anchor = [0.5, if i == 0 { 0.0 } else { 1.0 }];
            s.transform.x = screen_size[0] / 2.0;
            s.transform.y = if i == 0 { screen_size[1] + 1.0 } else { -1.0 };
            s.transform.scale_y = 100.0;
            stage.add_sprite(container, s)
        });
        let mut fade = Sprite::new(AtlasManager::WHITE);
        fade.color = StageColor::BLACK;
        fade.transform.scale_x = 87.5;
        fade.transform.scale_y = 50.0;
        fade.transform.x = screen_size[0] / 2.0;
        fade.transform.y = screen_size[1] / 2.0;
        let fullscreen_fade = stage.add_sprite(container, fade);
        let label =
            display_font.map(|font| stage.add_label(container, Label::new(Arc::clone(font), "")));
        Self {
            label,
            fullscreen_fade,
            sprites,
            last_any_key: true,
            restart_not_allowed: 40,
            game_over_mode: false,
            play_game_over_sound: false,
            food_in_stomach: 0,
            death_room: 0,
            death_pos: [0.0; 2],
            paused_mode: false,
            paused_warning_text: false,
            red_is_out_of_cycles: false,
            pause_menu: false,
            messages: Vec::new(),
            show: 0.0,
            last_show: 0.0,
            pause_mode_height: 0.0,
            last_pause_mode_height: 0.0,
            hide_hud: false,
            subregion_tracker,
            food_visible_mode: 0.0,
            last_food_visible_mode: 0.0,
            sin: 0.0,
            last_sin: 0.0,
            game_over_string: "Game Over - Press SPACE to restart".to_owned(),
            message_string: String::new(),
            currently_showing: InfoId::Nothing,
            go_to_death_screen: false,
            screen_size,
        }
    }

    /// `HUD.HideGeneralHud`: a message that hides the HUD is showing.
    #[must_use]
    pub fn hide_general_hud(&self) -> bool {
        self.hide_hud && self.show > 0.0
    }

    /// `AddMessage`.
    pub fn add_message(&mut self, text: &str, wait: i32, time: i32, darken: bool, hide_hud: bool) {
        self.messages.push(Message {
            text: text.to_owned(),
            wait,
            time,
            fade_out: false,
            darken,
            hide_hud,
        });
        if self.messages.len() == 1 {
            self.init_next_message();
        }
    }

    /// `EnterGameOverMode` with no grasp to depend on (no creature grasps the player in
    /// the port).
    pub fn enter_game_over_mode(
        &mut self,
        food_in_stomach: i32,
        death_room: i32,
        death_pos: [f32; 2],
    ) {
        self.food_in_stomach = food_in_stomach;
        self.death_room = death_room;
        self.death_pos = death_pos;
        self.restart_not_allowed = 40;
        self.game_over_mode = true;
        self.play_game_over_sound = true;
    }

    fn init_next_message(&mut self) {
        self.message_string.clone_from(&self.messages[0].text);
        self.hide_hud = self.messages[0].hide_hud;
    }

    /// The label's text.
    #[must_use]
    pub fn text<'a>(&self, stage: &'a StageTree) -> &'a str {
        self.label.map_or("", |l| stage.label(l).text())
    }

    /// `Update`. `map_button` is the map button as the game-over prompt reads it.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        n: &PromptNeighbours,
        room: Option<&str>,
        reveal_map: bool,
        map_button: bool,
        start: StartCondition,
        cycle_number: i32,
        translator: &Translator,
        fx: &mut HudFx<'_>,
    ) {
        self.last_show = self.show;
        self.last_pause_mode_height = self.pause_mode_height;
        let pause_target = if self.paused_mode { 1.0 } else { 0.0 };
        self.pause_mode_height =
            self.pause_mode_height
                .lerp_and_tick(pause_target, 0.01, 1.0 / 30.0);
        self.last_sin = self.sin;
        if self.game_over_mode || self.messages.first().is_some_and(|m| m.darken) {
            self.sin += 1.0;
        }
        self.last_food_visible_mode = self.food_visible_mode;
        let mut food_visible = false;
        let mut info = InfoId::Nothing;
        if self.game_over_mode {
            info = InfoId::GameOver;
            if self.currently_showing == InfoId::GameOver
                && self.play_game_over_sound
                && self.restart_not_allowed < 20
            {
                fx.play_sound(SoundId::HudGameOverPrompt);
                self.play_game_over_sound = false;
            }
            if self.restart_not_allowed > 0 {
                self.restart_not_allowed -= 1;
            } else {
                if map_button && !self.last_any_key {
                    self.go_to_death_screen = true;
                }
                self.last_any_key = map_button;
            }
        } else if self.paused_mode {
            if self.pause_menu {
                info = InfoId::Paused;
            } else {
                self.paused_mode = false;
            }
        } else {
            if let Some(tracker) = &mut self.subregion_tracker
                && let Some(text) = tracker.update(room, self.show, start, cycle_number, translator)
            {
                self.add_message(&text, 0, 160, false, true);
            }
            if self
                .messages
                .first()
                .is_some_and(|m| !m.fade_out && m.wait <= 0)
            {
                info = InfoId::Message;
            }
            self.update_messages(n, reveal_map, &mut food_visible);
        }
        if info == self.currently_showing {
            self.show = if self.currently_showing == InfoId::Nothing {
                0.0
            } else {
                (self.show + 1.0 / 30.0).min(1.0)
            };
        } else {
            self.show = (self.show - 1.0 / 30.0).max(0.0);
            if self.show == 0.0 && self.last_show == 0.0 {
                self.currently_showing = info;
                let text = match info {
                    InfoId::Message => self.message_string.clone(),
                    InfoId::Paused => translator.translate(self.paused_text()).to_owned(),
                    InfoId::GameOver => translator.translate(&self.game_over_string).to_owned(),
                    InfoId::Nothing => String::new(),
                };
                if let Some(label) = self.label_mut(fx.stage) {
                    label.set_text(&text);
                }
            }
        }
        let target = if food_visible { 1.0 } else { 0.0 };
        self.food_visible_mode = self.food_visible_mode.lerp_and_tick(target, 0.05, 0.05);
    }

    /// The pause text (Expedition's karma-0 warning aside).
    fn paused_text(&self) -> &'static str {
        if !self.paused_warning_text {
            "Paused"
        } else if self.red_is_out_of_cycles {
            "Paused - Warning! Quitting without saving in a shelter will permanently end your game"
        } else {
            "Paused - Warning! Quitting after 30 seconds into a cycle counts as a loss"
        }
    }

    /// The message queue's countdowns.
    fn update_messages(&mut self, n: &PromptNeighbours, reveal_map: bool, food_visible: &mut bool) {
        let count = self.messages.len();
        let show = self.show;
        let Some(first) = self.messages.first_mut() else {
            return;
        };
        if !first.hide_hud && n.food_fade > 0.0 {
            *food_visible = true;
        }
        if first.wait > 0 {
            first.wait -= 1;
            if count > 1 {
                first.wait = 0;
            }
        } else if show == 1.0 {
            first.time -= 1;
            if reveal_map {
                first.time = (first.time - 4).clamp(0, 20);
            }
            if count > 1 {
                first.time -= 1;
            }
            if first.time < 1 {
                first.fade_out = true;
            }
        } else if first.fade_out && show == 0.0 {
            self.messages.remove(0);
            if !self.messages.is_empty() {
                self.init_next_message();
            }
        }
    }

    fn label_mut<'a>(&self, stage: &'a mut StageTree) -> Option<&'a mut Label> {
        self.label.map(|l| stage.label_mut(l))
    }

    /// `LowerBorderHeight`: the top of the bottom bar.
    #[must_use]
    pub fn lower_border_height(&self, n: &PromptNeighbours, t: f32) -> f32 {
        let pause = self.last_pause_mode_height.lerp(self.pause_mode_height, t);
        let show = self.last_show.lerp(self.show, t);
        let num = 30.0_f32.lerp(60.0, pause) * show.inverse_lerp(0.0, 0.5).s_curve(0.5);
        let food = self.last_food_visible_mode.lerp(self.food_visible_mode, t);
        let above_meters = n.food_pos_y + 30.0 + (n.karma_rad * n.karma_fade - 2.5).max(0.0);
        num.lerp(above_meters, food.s_curve(0.7))
    }

    /// `Draw`; `food_label_x` is `foodMeter.pos.x + TotalWidth(t)`.
    pub fn draw(&self, stage: &mut StageTree, n: &PromptNeighbours, food_label_x: f32, t: f32) {
        let value = self.last_show.lerp(self.show, t);
        let darken = !self.paused_mode
            && !self.game_over_mode
            && self.messages.first().is_some_and(|m| m.darken);
        let fade = stage.sprite_mut(self.fullscreen_fade);
        fade.transform.visible = darken;
        if darken {
            fade.transform.alpha = value.inverse_lerp(0.0, 0.5).s_curve(0.5) * 0.25;
        }
        let grey = MenuColor::MediumGrey.rgb();
        let pulsing = self.game_over_mode || self.messages.first().is_some_and(|m| m.darken);
        let color = if pulsing {
            let s = self.last_sin.lerp(self.sin, t);
            grey.lerp(
                MenuColor::White.rgb(),
                0.5 + 0.5 * (s / 60.0 * PI * 2.0).sin(),
            )
        } else {
            grey
        };
        let food = self.last_food_visible_mode.lerp(self.food_visible_mode, t);
        let x = if self.last_food_visible_mode > 0.0 || self.food_visible_mode > 0.0 {
            20.0_f32.lerp(food_label_x + 25.0, food.s_curve(0.7)) + 0.01
        } else {
            20.01
        };
        if let Some(label) = self.label_mut(stage) {
            label.color = color;
            label.set_alignment(Alignment::Left);
            label.transform.x = x;
            label.transform.y = 15.01;
            label.transform.alpha = value.inverse_lerp(0.5, 1.0).s_curve(0.8);
        }
        let top = stage.sprite_mut(self.sprites[0]);
        top.transform.y = self.screen_size[1] - 30.0 * value.inverse_lerp(0.0, 0.5).s_curve(0.5);
        let lower = self.lower_border_height(n, t);
        stage.sprite_mut(self.sprites[1]).transform.y = lower;
    }
}
