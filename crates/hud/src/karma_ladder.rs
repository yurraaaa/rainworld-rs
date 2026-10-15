//! `Menu.KarmaLadder` (536573-537967) and its `KarmaSymbol`s, as the sleep, death and
//! starve screens build them: the base game's ladder of karma symbols up to the cap,
//! its phases, sounds, sprites and random draws. The ripple ladder (Watcher), the
//! ghost screen's cap increase and More Slugcats' vengeance and karma-to-minimum modes
//! are not ported. Its end-game meters are [`EndgameMeter`]s.

use std::f32::consts::PI;

use rw_menu::{
    AtlasManager, LoopId, MenuColor, MenuMic, MenuSoundId, NodeId, Sprite, StageColor, StageTree,
    TriangleStrip,
};
use rw_random::UnityRandom;
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

use crate::{
    CustomFloat, CustomVec2, EndgameMeter, FadeCircle, MeterFrame, MeterSource, karma_symbol_sprite,
};

/// `KarmaLadder.Phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderPhase {
    Resting,
    Deselecting,
    MovingA,
    MovingB,
    Settling,
    Bump,
    CappedMovement,
    CapBump,
    ReinforceSave,
}

impl LadderPhase {
    pub const ALL: [Self; 9] = [
        Self::Resting,
        Self::Deselecting,
        Self::MovingA,
        Self::MovingB,
        Self::Settling,
        Self::Bump,
        Self::CappedMovement,
        Self::CapBump,
        Self::ReinforceSave,
    ];

    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Resting => "Resting",
            Self::Deselecting => "Deselecting",
            Self::MovingA => "MovingA",
            Self::MovingB => "MovingB",
            Self::Settling => "Settling",
            Self::Bump => "Bump",
            Self::CappedMovement => "CappedMovement",
            Self::CapBump => "CapBump",
            Self::ReinforceSave => "ReinforceSave",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.value() == value)
    }
}

/// Where the ladder makes its sounds, random draws and fade circles.
pub struct LadderIo<'a> {
    pub stage: &'a mut StageTree,
    pub mic: &'a mut MenuMic,
    /// `manager.menuMic != null`.
    pub mic_attached: bool,
    pub rng: &'a mut UnityRandom,
    /// The screen HUD's `fadeCircles`.
    pub fade_circles: &'a mut Vec<FadeCircle>,
}

impl LadderIo<'_> {
    /// `Menu.PlaySound`.
    pub(crate) fn play(&mut self, id: MenuSoundId) {
        self.mic.play_sound(id, [0.0, 1.0, 1.0], self.rng);
    }
}

/// What a symbol's `GrafUpdate` reads of the ladder and the screen's map.
#[derive(Debug, Clone, Copy)]
struct SymbolFrame {
    draw_pos: Vec2,
    flat: f32,
    scroll: f32,
    lerped_scroll: f32,
    selection_glow: f32,
    reinforced: bool,
    reinforcement_cycle: f32,
    reinforcement_death: f32,
    /// `hud.map.fade`, when the screen's HUD has a map.
    map_fade: Option<f32>,
}

/// `KarmaLadder.KarmaSymbol`.
#[derive(Debug, Clone)]
pub struct KarmaSymbol {
    /// The symbol, `karmaRing`, the line to the next symbol and two glows.
    sprites: [NodeId; 5],
    /// Karma and the cap the symbol is drawn for.
    pub display_karma: (i32, i32),
    pub line_width: f32,
    pub last_line_width: f32,
    pub flicker_counter: i32,
    pub energy: f32,
    pub last_energy: f32,
    pub last_draw_y: f32,
}

impl KarmaSymbol {
    const KARMA: usize = 0;
    const RING: usize = 1;
    const LINE: usize = 2;
    const GLOW: usize = 3;

    /// The constructor (not in ripple mode): the symbol, ring and line in `container`,
    /// the glows in `foreground`.
    fn new(
        stage: &mut StageTree,
        container: NodeId,
        foreground: NodeId,
        display_karma: (i32, i32),
    ) -> Self {
        let element = karma_symbol_sprite(false, display_karma.0, display_karma.1);
        let karma = stage.add_sprite(container, Sprite::new(&element));
        let ring = stage.add_sprite(container, Sprite::new("karmaRing"));
        let line = stage.add_sprite(container, Sprite::new("pixel"));
        let glows = [0, 1].map(|_| {
            let mut glow = Sprite::new(AtlasManager::WHITE);
            glow.shader = StageShader::FlatLight;
            stage.add_sprite(foreground, glow)
        });
        Self {
            sprites: [karma, ring, line, glows[0], glows[1]],
            display_karma,
            line_width: 0.0,
            last_line_width: 0.0,
            flicker_counter: 0,
            energy: 0.0,
            last_energy: 0.0,
            last_draw_y: 0.0,
        }
    }

    /// `displayKarmaCap`.
    fn display_karma_cap(&self) -> i32 {
        if self.display_karma.1 < 100 {
            self.display_karma.1
        } else {
            8
        }
    }

    /// `Update`: `moving_a` and `scroll` are the ladder's, `next_energy` the energy of
    /// the symbol above (read only below the cap). One draw, two while flickering, and a
    /// third when the flicker dims it.
    fn update(&mut self, moving_a: bool, scroll: f32, next_energy: f32, rng: &mut UnityRandom) {
        self.last_line_width = self.line_width;
        self.last_energy = self.energy;
        let x = self.display_karma.0 as f32;
        let to = if moving_a {
            1.0
        } else {
            (x - scroll)
                .abs()
                .min((x + 1.0 - scroll).abs())
                .clamp(0.0, 1.0)
        };
        if self.display_karma.0 < self.display_karma_cap() {
            self.line_width *= (self.energy + next_energy) / 2.0;
        }
        self.line_width = self.line_width.lerp_and_tick(to, 0.2, 0.1);
        if rng.value() > (self.flicker_counter as f32).inverse_lerp(5.0, 40.0) {
            self.energy = self.energy.lerp_and_tick(1.0, 0.1, 1.0 / 70.0);
        }
        if self.flicker_counter > 0 {
            self.flicker_counter -= 1;
            let flicker = self.flicker_counter as f32;
            if rng.value() < flicker.inverse_lerp(5.0, 19.0) {
                let dim = rng.value().mathf_pow(flicker.lerp_map(50.0, 2.0, 3.0, 0.2));
                self.energy = self.energy.min(self.energy.lerp(dim, 0.5));
            }
        }
    }

    /// `GrafUpdate`: the symbol on the ladder's wheel (flat when `flat` is 1), its glow
    /// near the selection and the line up to the next symbol.
    fn draw(&mut self, stage: &mut StageTree, f: &SymbolFrame, t: f32) {
        let flat = f.flat.s_curve(0.09);
        let rad = 250.0_f32.lerp(1000.0, flat);
        let circumference = 2.0 * rad * PI;
        let x = self.display_karma.0 as f32;
        let ang = (x - f.scroll) * (360.0 / circumference) * 100.0;
        let [karma, ring, line, glow0, glow1] = self.sprites;
        if ang.abs() > 90.0 {
            for s in [karma, ring, glow0, glow1] {
                stage.sprite_mut(s).transform.visible = false;
            }
        } else {
            let half = 63.0 / circumference * 180.0;
            let top = rad * (ang + half).to_radians().sin();
            let bottom = rad * (ang - half).to_radians().sin();
            let mut pos = f.draw_pos + Vec2::new(0.0, (top + bottom) * 0.5);
            pos.y = pos.y.lerp(f.draw_pos.y + (x - f.scroll) * 100.0, flat);
            self.last_draw_y = pos.y;
            let away = ((x - f.lerped_scroll).abs() / 0.75).clamp(0.0, 1.0);
            let scale_y = ((top - bottom).abs() / 63.0).lerp(1.0, flat);
            let energy = self.last_energy.lerp(self.energy, t);
            let color = MenuColor::Black
                .hsl()
                .lerp(
                    MenuColor::White.hsl().lerp(MenuColor::DarkGrey.hsl(), away),
                    energy,
                )
                .rgb();
            for s in [karma, ring] {
                let sprite = stage.sprite_mut(s);
                sprite.transform.visible = true;
                sprite.transform.x = pos.x;
                sprite.transform.y = pos.y;
                sprite.transform.scale_y = scale_y;
                sprite.color = color;
            }
            if away < 0.2 {
                self.draw_glows(stage, f, pos, away);
            } else {
                stage.sprite_mut(glow0).transform.visible = false;
                stage.sprite_mut(glow1).transform.visible = false;
            }
        }
        let line_ang = (x + 0.5 - f.scroll) * (360.0 / circumference) * 100.0;
        if ang.abs() > 90.0 || self.display_karma.0 == self.display_karma_cap() {
            stage.sprite_mut(line).transform.visible = false;
            return;
        }
        let y = (rad * line_ang.to_radians().sin()).lerp((x + 0.5 - f.scroll) * 100.0, flat);
        let width = self.last_line_width.lerp(self.line_width, t).max(0.0);
        let away = ((x + 0.5 - f.lerped_scroll).abs() / 0.75).clamp(0.0, 1.0);
        let sprite = stage.sprite_mut(line);
        sprite.transform.visible = true;
        sprite.transform.x = f.draw_pos.x;
        sprite.transform.y = f.draw_pos.y + y;
        sprite.transform.scale_x = 80.0 * width.mathf_pow(0.6);
        sprite.color = MenuColor::Black
            .hsl()
            .lerp(
                MenuColor::MediumGrey
                    .hsl()
                    .lerp(MenuColor::DarkGrey.hsl(), away),
                width.mathf_pow(2.0),
            )
            .rgb();
    }

    /// The two glows of a symbol at the selection (`away` below 0.2).
    fn draw_glows(&self, stage: &mut StageTree, f: &SymbolFrame, pos: Vec2, away: f32) {
        let mut near = away.inverse_lerp(0.2, 0.1) * (0.3 + 0.7 * f.selection_glow.clamp(0.0, 1.0));
        let mut over = f.selection_glow.inverse_lerp(1.0, 2.0);
        let mut reinforced = 0.0;
        if f.reinforced {
            reinforced = 0.7 + 0.3 * (PI * 2.0 * f.reinforcement_cycle).sin();
            reinforced = reinforced.max(over);
            reinforced *= 1.0 - away;
            reinforced *= 1.0 - f.reinforcement_death;
        }
        if let Some(fade) = f.map_fade {
            let shown = 1.0 - fade;
            near *= shown;
            over *= shown;
            reinforced *= shown;
        }
        let glow0 = stage.sprite_mut(self.sprites[Self::GLOW]);
        glow0.transform.visible = true;
        glow0.transform.x = pos.x;
        glow0.transform.y = pos.y;
        glow0.transform.set_scale(
            (70.0_f32.lerp(150.0, near.mathf_pow(0.5)) + over * 50.0 + reinforced * 40.0) / 8.0,
        );
        glow0.transform.alpha = (0.2 * near.mathf_pow(1.5)).lerp(0.4, over.max(reinforced * 0.5));
        let glow1 = stage.sprite_mut(self.sprites[Self::GLOW + 1]);
        glow1.transform.visible = true;
        glow1.transform.x = pos.x;
        glow1.transform.y = pos.y;
        glow1
            .transform
            .set_scale((50.0_f32.lerp(90.0, near) + over * 100.0 + 10.0 * reinforced) / 8.0);
        glow1.transform.alpha = (0.4 * near.mathf_pow(0.5))
            .lerp(0.8 + 0.1 * reinforced * (1.0 - over), over.max(reinforced));
    }

    /// The symbol's sprites: the symbol, the ring and the line.
    #[must_use]
    pub fn sprites(&self) -> [NodeId; 3] {
        [
            self.sprites[Self::KARMA],
            self.sprites[Self::RING],
            self.sprites[Self::LINE],
        ]
    }
}

/// `KarmaLadder`'s own sprites, in the order they are added to its main container.
#[derive(Debug, Clone)]
struct LadderSprites {
    background_circle: NodeId,
    black_ribbon: NodeId,
    /// `SymbolGradientSprite(side, part)`: a `LinearGradient200` and a `pixel`.
    symbol_gradients: [[NodeId; 2]; 2],
    /// `LineSprite(line, part)`: a `pixel` and a `LinearGradient200`.
    lines: [[NodeId; 2]; 4],
    /// `CustomFSprite`s: quads with a colour per corner.
    reinforcement_lines: [NodeId; 4],
    circle: NodeId,
    reinforcement: NodeId,
    black_fade: NodeId,
    local_bloom: NodeId,
}

/// The ladder at the screen's `(350, 450)`.
#[derive(Debug, Clone)]
pub struct KarmaLadder {
    /// Relative to the page.
    pub pos: Vec2,
    pub last_pos: Vec2,
    /// `ladderContainers`: behind the meters, the main one, and the fade circles' (the
    /// symbols' glows), which draws above the screen's HUD.
    pub containers: [NodeId; 3],
    /// The end-game meters' place in the main container, behind the symbols.
    pub meter_container: NodeId,
    sprites: LadderSprites,
    pub symbols: Vec<KarmaSymbol>,
    /// `endGameMeters`.
    pub meters: Vec<EndgameMeter>,
    pub scroll: f32,
    pub last_scroll: f32,
    pub scroll_vel: f32,
    pub movement_shown: bool,
    pub reinforced: bool,
    pub display_karma: (i32, i32),
    pub move_to_karma: i32,
    pub capped_movement: bool,
    pub flat: f32,
    pub last_flat: f32,
    pub selection_glow: f32,
    pub last_selection_glow: f32,
    pub circle_rad: f32,
    pub last_circle_rad: f32,
    pub circle_thickness: f32,
    pub last_circle_thickness: f32,
    pub selector_white: f32,
    pub last_selector_white: f32,
    pub reinforcement_white: f32,
    pub last_reinforcement_white: f32,
    pub reinforcement_cycle: f32,
    pub last_reinforcement_cycle: f32,
    pub reinforcement_clamp: f32,
    pub last_reinforcement_clamp: f32,
    pub reinforcement_death: f32,
    pub last_reinforcement_death: f32,
    pub reinforcement_lines: f32,
    pub last_reinforcement_lines: f32,
    pub local_bloom: f32,
    pub last_local_bloom: f32,
    pub phase: LadderPhase,
    pub ticks_in_phase: i32,
    pub karma_increasing: bool,
    pub movement_sound_loop: Option<LoopId>,
    pub started_animating: bool,
    pub display_meters_on_rest: bool,
    pub end_game_meter_anim_counter: i32,
    pub next_meter_to_animate: usize,
    pub show_end_game_meters_counter: i32,
    /// `options.ScreenSize.x`.
    screen_width: f32,
}

impl KarmaLadder {
    /// `new KarmaLadder(menu, owner, pos, hud, displayKarma, reinforced)`: its sprites
    /// in `containers` (made by the screen's `InitContainers`), the meters of the
    /// trackers with progress to show (`AddEndgameMeters`, after a death's
    /// `PlayerDied`), a symbol for each karma up to the cap, at rest.
    #[must_use]
    pub fn new(
        stage: &mut StageTree,
        containers: [NodeId; 3],
        pos: Vec2,
        display_karma: (i32, i32),
        reinforced: bool,
        screen_width: f32,
        meters: Option<&MeterSource<'_>>,
    ) -> Self {
        let main = containers[1];
        let background_circle = Self::white_sprite(stage, main, StageShader::VectorCircle);
        let meter_container = stage.add_container(main);
        let meters = meters.map_or_else(Vec::new, |source| {
            let shown: Vec<_> = source
                .trackers
                .iter()
                .filter(|t| t.any_progress_to_show())
                .collect();
            Self::endgame_meter_positions(shown.len())
                .into_iter()
                .zip(shown)
                .map(|(p, t)| {
                    EndgameMeter::new(stage, [containers[0], meter_container], p, t, source)
                })
                .collect()
        });
        let black_ribbon = stage.add_sprite(main, Sprite::new("pixel"));
        let symbols = (0..=display_karma.1)
            .map(|i| KarmaSymbol::new(stage, main, containers[2], (i, display_karma.1)))
            .collect();
        let symbol_gradients = [0, 1].map(|_| {
            [
                Self::anchored_black(stage, main, "LinearGradient200"),
                Self::anchored_black(stage, main, "pixel"),
            ]
        });
        let lines = [0, 1, 2, 3].map(|_| {
            let mut line = Sprite::new("pixel");
            line.anchor[1] = 0.0;
            line.transform.scale_x = 3.0;
            line.color = MenuColor::MediumGrey.rgb();
            let line = stage.add_sprite(main, line);
            let gradient = Self::anchored_black(stage, main, "LinearGradient200");
            stage.sprite_mut(gradient).transform.scale_x = 3.0;
            [line, gradient]
        });
        let reinforcement_lines =
            [0, 1, 2, 3].map(|_| stage.add_strip(main, TriangleStrip::long_mesh(1, "pixel")));
        let circle = Self::white_sprite(stage, main, StageShader::VectorCircle);
        let reinforcement = Self::white_sprite(stage, main, StageShader::VectorCircle);
        let black_fade = Self::white_sprite(stage, main, StageShader::FlatLight);
        let local_bloom = Self::white_sprite(stage, main, StageShader::LocalBloom);
        let x = display_karma.0 as f32;
        Self {
            pos,
            last_pos: pos,
            containers,
            meter_container,
            sprites: LadderSprites {
                background_circle,
                black_ribbon,
                symbol_gradients,
                lines,
                reinforcement_lines,
                circle,
                reinforcement,
                black_fade,
                local_bloom,
            },
            symbols,
            meters,
            scroll: x,
            last_scroll: x,
            scroll_vel: 0.0,
            movement_shown: false,
            reinforced,
            display_karma,
            move_to_karma: display_karma.0,
            capped_movement: false,
            flat: 1.0,
            last_flat: 1.0,
            selection_glow: 0.0,
            last_selection_glow: 0.0,
            circle_rad: 53.0,
            last_circle_rad: 53.0,
            circle_thickness: 0.0,
            last_circle_thickness: 0.0,
            selector_white: 0.0,
            last_selector_white: 0.0,
            reinforcement_white: 0.0,
            last_reinforcement_white: 0.0,
            reinforcement_cycle: 0.0,
            last_reinforcement_cycle: 0.0,
            reinforcement_clamp: 0.0,
            last_reinforcement_clamp: 0.0,
            reinforcement_death: 0.0,
            last_reinforcement_death: 0.0,
            reinforcement_lines: 0.0,
            last_reinforcement_lines: 0.0,
            local_bloom: 0.0,
            last_local_bloom: 0.0,
            phase: LadderPhase::Resting,
            ticks_in_phase: 0,
            karma_increasing: false,
            movement_sound_loop: None,
            started_animating: false,
            display_meters_on_rest: false,
            end_game_meter_anim_counter: -1,
            next_meter_to_animate: 0,
            show_end_game_meters_counter: -1,
            screen_width,
        }
    }

    /// A `Futile_White` sprite drawn with `shader`.
    fn white_sprite(stage: &mut StageTree, container: NodeId, shader: StageShader) -> NodeId {
        let mut s = Sprite::new(AtlasManager::WHITE);
        s.shader = shader;
        stage.add_sprite(container, s)
    }

    /// A black sprite anchored at its bottom.
    fn anchored_black(stage: &mut StageTree, container: NodeId, element: &str) -> NodeId {
        let mut s = Sprite::new(element);
        s.color = StageColor::BLACK;
        s.anchor[1] = 0.0;
        stage.add_sprite(container, s)
    }

    /// `AddEndgameMeters`' layout: `count` meters split in two arcs of the 225 px circle
    /// (the second one longer when odd), each at `180 / (n + 1)` degree steps, the second
    /// arc turned half a circle.
    #[must_use]
    pub fn endgame_meter_positions(count: usize) -> Vec<Vec2> {
        let mut out = Vec::with_capacity(count);
        for side in 0..2 {
            let mut n = count / 2;
            if count % 2 == 1 && side == 1 {
                n += 1;
            }
            let step = 180.0 / (n as f32 + 1.0);
            for k in 0..n {
                let ang = step * (k as f32 + 1.0) + if side == 0 { 0.0 } else { 180.0 };
                out.push(Vec2::from_degrees(ang) * 225.0);
            }
        }
        out
    }

    /// `displayKarmaCap`.
    #[must_use]
    pub fn display_karma_cap(&self) -> i32 {
        self.display_karma.1
    }

    /// `AllAnimationDone`.
    #[must_use]
    pub fn all_animation_done(&self) -> bool {
        if self
            .meters
            .iter()
            .any(|m| m.powered_on && m.meter_animation < 1.0)
        {
            return false;
        }
        self.started_animating
            && self.display_karma.0 == self.move_to_karma
            && self.next_meter_to_animate >= self.meters.len()
    }

    /// `GoToKarma`.
    pub fn go_to_karma(&mut self, new_goal_karma: i32, display_meters_on_rest: bool) {
        self.display_meters_on_rest = display_meters_on_rest;
        self.move_to_karma = new_goal_karma;
        self.karma_increasing = self.move_to_karma > self.display_karma.0;
        self.capped_movement =
            self.move_to_karma < 0 || self.move_to_karma > self.display_karma_cap();
        if display_meters_on_rest && self.display_karma.0 == self.move_to_karma {
            self.movement_shown = true;
            self.show_end_game_meters_counter = 125;
        }
        self.started_animating = true;
    }

    /// `Update`, after the symbols' (`base.Update`). `owner_pos` is the page's screen
    /// position (`DrawPos(1)` is it plus `pos`); `fade_out_illustration` the screen's,
    /// as of its last tick.
    pub fn update(&mut self, io: &mut LadderIo<'_>, owner_pos: Vec2, fade_out_illustration: f32) {
        for m in &mut self.meters {
            m.update(io);
        }
        let moving_a = self.phase == LadderPhase::MovingA;
        let top = self.symbols.len() as i32 - 2;
        for i in 0..self.symbols.len() {
            let above = (self.symbols[i].display_karma.0.min(top) + 1).max(0) as usize;
            let next_energy = self.symbols[above].energy;
            self.symbols[i].update(moving_a, self.scroll, next_energy, io.rng);
        }
        self.last_pos = self.pos;
        if self.show_end_game_meters_counter > -1 && !self.meters.is_empty() {
            let shift = (self.show_end_game_meters_counter as f32)
                .inverse_lerp(85.0, 20.0)
                .s_curve(0.5);
            let x = self.screen_width.lerp_map(1024.0, 1366.0, 500.0, 350.0);
            self.pos.x = 350.0_f32.lerp(x, shift);
        }
        self.update_movement_loop(io);
        self.update_meter_counters(io);
        self.last_scroll = self.scroll;
        self.last_flat = self.flat;
        self.last_circle_rad = self.circle_rad;
        self.last_circle_thickness = self.circle_thickness;
        self.last_selection_glow = self.selection_glow;
        self.last_selector_white = self.selector_white;
        self.last_reinforcement_white = self.reinforcement_white;
        self.last_reinforcement_cycle = self.reinforcement_cycle;
        self.last_reinforcement_clamp = self.reinforcement_clamp;
        self.last_reinforcement_death = self.reinforcement_death;
        self.last_reinforcement_lines = self.reinforcement_lines;
        self.last_local_bloom = self.local_bloom;
        self.update_bloom(fade_out_illustration);
        self.scroll += self.scroll_vel;
        self.selection_glow = self.selection_glow.lerp_and_tick(1.0, 0.2, 0.0125);
        self.update_reinforcement();
        self.circle_thickness = 3.0;
        self.selector_white = 0.0;
        if self.phase != LadderPhase::ReinforceSave {
            self.reinforcement_clamp = 0.0;
        }
        self.ticks_in_phase += 1;
        let draw_pos = owner_pos + self.pos;
        match self.phase {
            LadderPhase::Resting => self.resting(io),
            LadderPhase::Deselecting => self.deselecting(io),
            LadderPhase::MovingA | LadderPhase::MovingB => self.moving(io),
            LadderPhase::Settling => self.settling(io, draw_pos),
            LadderPhase::Bump => self.bump(io, draw_pos),
            LadderPhase::CappedMovement => self.capped(io),
            LadderPhase::CapBump => self.cap_bump(io, draw_pos),
            LadderPhase::ReinforceSave => self.reinforce_save(io),
        }
    }

    /// The movement loop: started once there is a menu microphone; its volume and pitch
    /// follow the scroll's speed.
    fn update_movement_loop(&mut self, io: &mut LadderIo<'_>) {
        if self.movement_sound_loop.is_none() && io.mic_attached {
            self.movement_sound_loop = io.mic.play_loop(
                MenuSoundId::KarmaLadderMovementLoop,
                [0.0, 1.0, 1.0],
                false,
                io.rng,
            );
        }
        let Some(handle) = self.movement_sound_loop else {
            return;
        };
        let speed = (self.last_scroll - self.scroll).abs();
        match io.mic.loop_mut(handle) {
            Some(l) => {
                l.loop_volume = speed.inverse_lerp(0.0, 0.01);
                l.loop_pitch = speed.lerp_map_pow(0.0, 0.03, 0.5, 1.0, 0.8);
                if l.destroyed {
                    self.movement_sound_loop = None;
                }
            }
            None => self.movement_sound_loop = None,
        }
    }

    /// The end-game meters' power-on (`showEndGameMetersCounter`) and the start of each
    /// meter's animation in turn.
    fn update_meter_counters(&mut self, io: &mut LadderIo<'_>) {
        if self.show_end_game_meters_counter > 0 {
            self.show_end_game_meters_counter -= 1;
            if self.show_end_game_meters_counter == 1 {
                self.end_game_meter_anim_counter = 1;
            } else if self.show_end_game_meters_counter == 80 {
                io.play(MenuSoundId::EndgameMetersPowerOn);
                for m in &mut self.meters {
                    m.powered_on = true;
                    let high = io.rng.range_i32(20, 60);
                    m.flicker_counter = m.flicker_counter.max(io.rng.range_i32(12, high));
                }
            }
        }
        if self.end_game_meter_anim_counter > -1 && self.next_meter_to_animate < self.meters.len() {
            self.end_game_meter_anim_counter -= 1;
            if self.end_game_meter_anim_counter == 0 {
                let count = self.meters.len() as i32;
                let m = &mut self.meters[self.next_meter_to_animate];
                m.meter_animation = 0.01;
                if m.any_change_to_show() {
                    if let Some(sound) = m.change_sound() {
                        io.play(sound);
                    }
                    self.end_game_meter_anim_counter = 10 + 20 / count;
                } else {
                    self.end_game_meter_anim_counter = 1;
                }
                self.next_meter_to_animate += 1;
            }
        }
    }

    /// `localBloom`: rises at once to the selection's glow, falls slowly; dimmed while
    /// the screen's illustration fades for the map.
    fn update_bloom(&mut self, fade_out_illustration: f32) {
        let mut target = 0.0_f32;
        for s in &self.symbols {
            let away = ((s.display_karma.0 as f32 - self.scroll).abs() / 0.75).clamp(0.0, 1.0);
            let v = away.inverse_lerp(0.2, 0.1) * (0.3 + 0.7 * self.selection_glow.clamp(0.0, 1.0));
            target = target.max(v);
        }
        target = (target * 0.5 * self.selection_glow).mathf_pow(2.0);
        if self.local_bloom < target {
            self.local_bloom = target;
        } else {
            self.local_bloom =
                target.max(self.local_bloom - 1.0 / 40.0_f32.lerp(220.0, self.local_bloom));
        }
        self.local_bloom *= 1.0 - fade_out_illustration;
    }

    fn update_reinforcement(&mut self) {
        if !self.reinforced {
            self.reinforcement_white = 0.0;
            self.reinforcement_lines = 0.0;
            return;
        }
        let moving = matches!(
            self.phase,
            LadderPhase::MovingA
                | LadderPhase::MovingB
                | LadderPhase::Deselecting
                | LadderPhase::CappedMovement
        );
        self.reinforcement_white = if moving {
            self.reinforcement_white
                .lerp_and_tick(0.0, 0.07, 1.0 / 52.0)
        } else {
            self.reinforcement_white
                .lerp_and_tick(1.0, 0.11, 1.0 / 52.0)
        };
        self.reinforcement_white = self.reinforcement_white.max(self.selector_white);
        if self.phase != LadderPhase::ReinforceSave {
            self.reinforcement_cycle += 1.0 / 90.0;
        }
        if matches!(self.phase, LadderPhase::MovingA | LadderPhase::MovingB) {
            self.reinforcement_lines = self.reinforcement_lines.lerp_and_tick(0.0, 0.1, 0.05);
        } else {
            self.reinforcement_lines = self
                .reinforcement_lines
                .lerp_and_tick(1.0, 0.1, 0.05)
                .min(1.0 - self.reinforcement_death);
        }
    }

    fn resting(&mut self, io: &mut LadderIo<'_>) {
        self.circle_rad = self.circle_rad.lerp_and_tick(51.5, 0.08, 0.01);
        self.flat = 1.0;
        self.scroll_vel *= 0.3;
        self.scroll = self.scroll.lerp(self.display_karma.0 as f32, 0.2);
        if self.display_karma.0 != self.move_to_karma && self.started_animating {
            self.new_phase(LadderPhase::Deselecting, io);
        }
    }

    /// One draw a tick, a second when it flickers the glow.
    fn deselecting(&mut self, io: &mut LadderIo<'_>) {
        self.circle_rad = self.circle_rad.lerp_and_tick(55.0, 0.08, 0.01);
        let p = (self.ticks_in_phase as f32).inverse_lerp(0.0, 30.0);
        if io.rng.value() < p {
            self.selection_glow = io.rng.value().lerp(0.0, p);
        }
        self.flat = 1.0 - p;
        if p < 1.0 {
            return;
        }
        if self.move_to_karma < self.display_karma.0 && self.reinforced {
            if self.move_to_karma < 0 {
                self.reinforcement_death = 0.01;
                self.new_phase(LadderPhase::CappedMovement, io);
            } else {
                self.new_phase(LadderPhase::ReinforceSave, io);
            }
        } else if self.capped_movement {
            self.new_phase(LadderPhase::CappedMovement, io);
        } else {
            self.new_phase(LadderPhase::MovingA, io);
            self.scroll_vel +=
                ((self.display_karma.0 - self.move_to_karma) as f32).mathf_sign() * 0.01;
        }
    }

    fn moving(&mut self, io: &mut LadderIo<'_>) {
        let a = self.phase == LadderPhase::MovingA;
        self.circle_rad = self.circle_rad.lerp_and_tick(50.0, 0.1, 0.1);
        self.selection_glow = 0.0;
        let goal = self.move_to_karma as f32
            + ((self.move_to_karma - self.display_karma.0) as f32).mathf_sign()
                * -0.049
                * if a { 1.0 } else { 0.0 };
        self.flat = 0.0;
        self.scroll_vel += (goal - self.scroll) / if a { 400.0 } else { 3.0 };
        self.scroll_vel += (goal - self.scroll).mathf_sign() * 0.001;
        self.scroll_vel *= if a { 0.95 } else { 0.9 };
        let passed = ((self.scroll < goal) == (self.move_to_karma < self.display_karma.0)) == a;
        if passed {
            self.new_phase(
                if a {
                    LadderPhase::MovingB
                } else {
                    LadderPhase::Settling
                },
                io,
            );
        }
    }

    fn settling(&mut self, io: &mut LadderIo<'_>, draw_pos: Vec2) {
        let p = (self.ticks_in_phase as f32).inverse_lerp(0.0, 40.0);
        self.scroll_vel *= 0.6;
        self.scroll = self.scroll.lerp(self.display_karma.0 as f32, 0.2);
        if self.karma_increasing {
            self.circle_rad = 51.5 + (p * PI * 1.5).sin() * 3.0 + (p * PI).sin() * 3.0;
            if !self.capped_movement {
                self.circle_thickness = 3.0 + 3.0 * (p.mathf_pow(5.0) * PI).sin();
                self.selector_white = p.inverse_lerp(0.3, 0.6);
            }
            self.selection_glow = (p.mathf_pow(5.0) * PI).sin() * 0.4;
            self.flat = 0.0;
        } else {
            self.circle_rad = 51.5 + (p * PI).sin() * 3.0;
            self.flat = self.flat.lerp(1.0, p.mathf_pow(4.0));
            self.selection_glow *= 0.9 - self.reinforcement_death * 0.3;
        }
        if self.reinforced && self.reinforcement_death > 0.0 {
            let wave = (self.reinforcement_death.mathf_pow(3.0) * PI).sin();
            self.selection_glow += 0.3 * wave;
            self.circle_rad += 10.0 * wave;
            self.reinforcement_death = self.reinforcement_death.max(p.inverse_lerp(0.2, 0.8));
            if self.reinforcement_death >= 1.0 {
                self.reinforced = false;
                io.fade_circles.push(FadeCircle::new(
                    io.stage,
                    self.containers[2],
                    31.5,
                    16.0,
                    0.82,
                    40.0,
                    8.0,
                    draw_pos,
                ));
                io.play(MenuSoundId::KarmaLadderReinforcementDissipateB);
            }
        }
        if p >= 1.0 {
            let next = if self.karma_increasing && self.capped_movement {
                LadderPhase::CapBump
            } else if self.karma_increasing {
                LadderPhase::Bump
            } else {
                LadderPhase::Resting
            };
            self.new_phase(next, io);
        }
    }

    /// Tick 6 makes two draws for each other symbol and a third for its neighbours;
    /// tick 13 one for each meter, which flickers its energy.
    fn bump(&mut self, io: &mut LadderIo<'_>, draw_pos: Vec2) {
        self.scroll_vel *= 0.4;
        self.scroll = self.scroll.lerp(self.display_karma.0 as f32, 0.2);
        let p = (self.ticks_in_phase as f32).inverse_lerp(0.0, 45.0);
        self.circle_rad =
            53.0 + 50.0_f32.lerp(0.0, p.mathf_pow(0.13)) - 2.0 * p.inverse_lerp(0.8, 1.0);
        self.selection_glow = (1.0 + (p.mathf_pow(0.2) * PI).sin()) * (1.0 - p).mathf_pow(0.4);
        self.flat = p.mathf_pow(0.3);
        let current = self.display_karma.0 as usize;
        match self.ticks_in_phase {
            4 => {
                io.fade_circles.push(FadeCircle::new(
                    io.stage,
                    self.containers[2],
                    self.circle_rad,
                    26.0,
                    0.88,
                    60.0,
                    6.0,
                    draw_pos,
                ));
                io.play(MenuSoundId::KarmaLadderIncreaseBump);
            }
            5 => {
                for (k, s) in self.symbols.iter_mut().enumerate() {
                    if k != current {
                        s.energy = 0.0;
                    }
                }
            }
            6 => {
                for (k, s) in self.symbols.iter_mut().enumerate() {
                    if k == current {
                        continue;
                    }
                    let high = io.rng.range_i32(40, 80);
                    s.flicker_counter = s.flicker_counter.max(io.rng.range_i32(12, high));
                    if k.abs_diff(current) < 2 {
                        s.flicker_counter += io.rng.range_i32(20, 30);
                    }
                }
            }
            13 => {
                for m in &mut self.meters {
                    m.energy =
                        0.5_f32.lerp(1.0, io.rng.value()) * 1.0_f32.lerp(0.15, m.show_as_fulfilled);
                }
            }
            _ => {}
        }
        if p >= 1.0 {
            self.new_phase(LadderPhase::Resting, io);
        }
    }

    fn capped(&mut self, io: &mut LadderIo<'_>) {
        let p = (self.ticks_in_phase as f32).inverse_lerp(0.0, 15.0);
        self.circle_rad = self.circle_rad.lerp_and_tick(50.0, 0.1, 0.1);
        self.selection_glow = 0.0;
        self.flat = 0.0;
        self.scroll_vel *= 1.0_f32.lerp(0.92, p.mathf_pow(6.0));
        if self.ticks_in_phase == 9 {
            io.play(if self.move_to_karma > self.display_karma.0 {
                MenuSoundId::KarmaLadderHitUpperCap
            } else {
                MenuSoundId::KarmaLadderHitLowerCap
            });
        }
        let sign = ((self.display_karma.0 - self.move_to_karma) as f32).mathf_sign();
        if p < 0.5 {
            self.scroll_vel -= sign * 0.001;
        } else {
            self.scroll_vel += sign * 0.001;
        }
        if p >= 1.0 {
            self.move_to_karma = self.display_karma.0;
            self.new_phase(LadderPhase::Settling, io);
        }
    }

    fn cap_bump(&mut self, io: &mut LadderIo<'_>, draw_pos: Vec2) {
        self.scroll_vel *= 0.4;
        self.scroll = self.scroll.lerp(self.display_karma.0 as f32, 0.2);
        let p = (self.ticks_in_phase as f32).inverse_lerp(0.0, 45.0);
        self.circle_rad = 53.0 + 20.0_f32.lerp(0.0, p.mathf_pow(0.13)) - p.inverse_lerp(0.8, 1.0);
        self.selection_glow =
            (1.0 + 0.5 * (p.mathf_pow(0.2) * PI).sin()) * (1.0 - p).mathf_pow(0.4);
        self.flat = p.mathf_pow(0.3);
        if self.ticks_in_phase == 4 {
            io.fade_circles.push(FadeCircle::new(
                io.stage,
                self.containers[2],
                self.circle_rad,
                16.0,
                0.82,
                40.0,
                6.0,
                draw_pos,
            ));
            io.play(MenuSoundId::KarmaLadderUpperCapBump);
        }
        if p >= 1.0 {
            self.new_phase(LadderPhase::Resting, io);
        }
    }

    fn reinforce_save(&mut self, io: &mut LadderIo<'_>) {
        let p = (self.ticks_in_phase as f32).inverse_lerp(0.0, 35.0);
        self.circle_rad = self.circle_rad.lerp_and_tick(
            50.0 + 20.0 * self.reinforcement_clamp.mathf_pow(0.2),
            0.1,
            0.1,
        );
        self.selection_glow = 0.0;
        self.flat = 0.0;
        self.scroll_vel *= 0.95;
        let x = self.display_karma.0 as f32;
        if p < 0.25 {
            self.scroll_vel += (self.move_to_karma as f32 - self.scroll) / 400.0;
            self.scroll_vel += (self.move_to_karma as f32 - self.scroll).mathf_sign() * 0.001;
        } else {
            self.scroll_vel +=
                ((self.display_karma.0 - self.move_to_karma) as f32).mathf_sign() * 0.001;
        }
        self.scroll = p.lerp_map_pow(0.6, 1.0, self.scroll, x, 8.0);
        match self.ticks_in_phase {
            17 => io.play(MenuSoundId::KarmaLadderReinforceSaveGrab),
            35 => io.play(MenuSoundId::KarmaLadderReinforceSavePull),
            50 => io.play(MenuSoundId::KarmaLadderReinforceSaveSecure),
            _ => {}
        }
        if p > 0.2 {
            self.reinforcement_cycle = self.reinforcement_cycle.floor();
            if (self.scroll - x).abs() < 0.1 {
                self.reinforcement_clamp = (self.reinforcement_clamp - 0.05).max(0.0);
                if self.reinforcement_clamp <= 0.0 {
                    self.move_to_karma = self.display_karma.0;
                    self.reinforcement_death = 0.01;
                    self.new_phase(LadderPhase::Settling, io);
                }
            } else {
                self.reinforcement_clamp = p.inverse_lerp(0.2, 0.5);
            }
        }
    }

    /// `NewPhase`: its sound; settling moves the displayed karma to the goal, and
    /// resting shows the meters when asked.
    fn new_phase(&mut self, next: LadderPhase, io: &mut LadderIo<'_>) {
        self.ticks_in_phase = -1;
        match next {
            LadderPhase::Deselecting => io.play(MenuSoundId::KarmaLadderDeselect),
            LadderPhase::MovingA | LadderPhase::CappedMovement | LadderPhase::ReinforceSave => {
                io.play(MenuSoundId::KarmaLadderStartMoving);
            }
            LadderPhase::Resting => {
                if self.display_meters_on_rest {
                    self.movement_shown = true;
                    self.show_end_game_meters_counter = 85;
                }
            }
            LadderPhase::Settling => {
                if self.karma_increasing && !self.capped_movement {
                    io.play(MenuSoundId::KarmaLadderIncreaseBumpCharge);
                } else if self.reinforcement_death > 0.0 {
                    io.play(MenuSoundId::KarmaLadderReinforcementDissipateA);
                } else {
                    io.play(MenuSoundId::KarmaLadderReselect);
                }
                self.display_karma.0 = self.move_to_karma;
            }
            _ => {}
        }
        self.phase = next;
    }

    /// `GrafUpdate`: the symbols, then the ladder's sprites. `owner_pos` is the page's
    /// draw position, `map_fade` the screen HUD's map fade (if it has a map) and
    /// `screen_height` Futile's.
    pub fn draw(
        &mut self,
        stage: &mut StageTree,
        owner_pos: Vec2,
        t: f32,
        map_fade: Option<f32>,
        screen_height: f32,
    ) {
        let draw_pos = owner_pos + self.last_pos.lerp(self.pos, t);
        let frame = SymbolFrame {
            draw_pos,
            flat: self.last_flat.lerp(self.flat, t),
            scroll: self.scroll,
            lerped_scroll: self.last_scroll.lerp(self.scroll, t),
            selection_glow: self.last_selection_glow.lerp(self.selection_glow, t),
            reinforced: self.reinforced,
            reinforcement_cycle: self
                .last_reinforcement_cycle
                .lerp(self.reinforcement_cycle, t),
            reinforcement_death: self
                .last_reinforcement_death
                .lerp(self.reinforcement_death, t),
            map_fade,
        };
        let meter_frame = MeterFrame {
            ladder: draw_pos,
            t,
            map_fade,
        };
        for m in &mut self.meters {
            m.draw(stage, &meter_frame);
        }
        for s in &mut self.symbols {
            s.draw(stage, &frame, t);
        }
        let sp = self.sprites.clone();
        let thickness = self.last_circle_thickness.lerp(self.circle_thickness, t);
        let rad = self.last_circle_rad.lerp(self.circle_rad, t) + thickness * 0.5;
        let circle = stage.sprite_mut(sp.circle);
        circle.transform.x = draw_pos.x;
        circle.transform.y = draw_pos.y;
        circle.color = MenuColor::MediumGrey
            .hsl()
            .lerp(
                MenuColor::White.hsl(),
                self.last_selector_white.lerp(self.selector_white, t),
            )
            .rgb();
        circle.transform.set_scale(rad / 8.0);
        circle.transform.alpha = thickness / rad;
        let background = stage.sprite_mut(sp.background_circle);
        background.transform.x = draw_pos.x;
        background.transform.y = draw_pos.y;
        background.transform.set_scale(150.0 / 8.0);
        background.transform.alpha = 3.0 / 150.0;
        background.color = MenuColor::DarkGrey.rgb();
        let fade = stage.sprite_mut(sp.black_fade);
        fade.transform.x = draw_pos.x;
        fade.transform.y = draw_pos.y;
        fade.transform.set_scale(50.0);
        fade.color = MenuColor::Black.rgb();
        fade.transform.alpha = map_fade.map_or(0.0, |f| 0.7 * f);
        let bloom = self.last_local_bloom.lerp(self.local_bloom, t);
        let local_bloom = stage.sprite_mut(sp.local_bloom);
        local_bloom.transform.x = draw_pos.x;
        local_bloom.transform.y = draw_pos.y;
        local_bloom.transform.set_scale(
            250.0_f32.lerp(400.0, bloom.max(0.0).mathf_pow(3.0))
                * if self.reinforced { 1.5 } else { 1.0 }
                / 8.0,
        );
        local_bloom.transform.alpha = bloom;
        if self.reinforced {
            self.draw_reinforcement(stage, draw_pos, rad, thickness, t);
        } else {
            stage.sprite_mut(sp.reinforcement).transform.visible = false;
            for l in sp.reinforcement_lines {
                stage.transform_mut(l).visible = false;
            }
        }
        self.draw_frame(stage, draw_pos, rad, screen_height);
    }

    /// The reinforced karma's ring around the selection and its four lines to the
    /// symbol, which it clamps onto while saving the karma.
    fn draw_reinforcement(
        &self,
        stage: &mut StageTree,
        draw_pos: Vec2,
        rad: f32,
        thickness: f32,
        t: f32,
    ) {
        let clamp = self
            .last_reinforcement_clamp
            .lerp(self.reinforcement_clamp, t);
        let death = self
            .last_reinforcement_death
            .lerp(self.reinforcement_death, t);
        let white = self
            .last_reinforcement_white
            .lerp(self.reinforcement_white, t)
            * (1.0 - death)
            * (0.5
                + 0.5
                    * (PI
                        * 2.0
                        * self
                            .last_reinforcement_cycle
                            .lerp(self.reinforcement_cycle, t))
                    .sin())
            .max(
                self.last_selection_glow
                    .lerp(self.selection_glow, t)
                    .inverse_lerp(1.0, 1.5),
            )
            .max(self.last_selector_white.lerp(self.selector_white, t))
            .max(clamp);
        let color = MenuColor::MediumGrey
            .hsl()
            .lerp(MenuColor::White.hsl(), white)
            .rgb();
        let width = 4.0 + 4.0 * clamp + (death.mathf_pow(4.0) * PI).sin() * 4.0;
        let mut ring = 31.5_f32.lerp(rad - thickness * 0.5, 0.5) + width * 0.5;
        ring = ring.lerp(31.5 + width / 2.0, clamp);
        ring -= 4.0 * (death.mathf_pow(1.5) * PI).sin();
        let symbol_y = self
            .symbols
            .get(self.display_karma.0.max(0) as usize)
            .map_or(draw_pos.y, |s| s.last_draw_y);
        let y = draw_pos.y.lerp(symbol_y, clamp);
        let sprite = stage.sprite_mut(self.sprites.reinforcement);
        sprite.transform.visible = true;
        sprite.color = color;
        sprite.transform.x = draw_pos.x;
        sprite.transform.y = y;
        sprite.transform.set_scale(ring / 8.0);
        sprite.transform.alpha = width / ring;
        let lines = self
            .last_reinforcement_lines
            .lerp(self.reinforcement_lines, t);
        let half_width = 2.0 * lines.mathf_pow(0.5);
        for (i, &node) in self.sprites.reinforcement_lines.iter().enumerate() {
            let from = Vec2::new(draw_pos.x, y)
                + Vec2::from_degrees(-45.0 + 90.0 * i as f32) * (ring - 2.0);
            let mut to = Vec2::new(draw_pos.x, symbol_y);
            let hit = from.circles_collision_time(to, to - from, 0.0, 30.75);
            if !hit.is_nan() {
                to = from.lerp(to, hit.clamp(0.0, 1.0));
            }
            let side = (from - to).perpendicular();
            let strip = stage.strip_mut(node);
            strip.transform.visible = true;
            // Corners 0 1 2 3 of the quad as a strip: 0, 1, 3, 2.
            strip.vertices[0] = (from - side * half_width).to_array();
            strip.vertices[1] = (from + side * half_width).to_array();
            strip.vertices[2] = (to - side * half_width).to_array();
            strip.vertices[3] = (to + side * half_width).to_array();
            let near = color.with_alpha_multiplied(lines);
            let far = MenuColor::White.rgb().with_alpha_multiplied(lines);
            strip.colors = vec![near, near, far, far];
        }
    }

    /// The black column the symbols turn on, its fades top and bottom, and the four grey
    /// lines from the column's corners to the selection circle.
    fn draw_frame(&self, stage: &mut StageTree, draw_pos: Vec2, rad: f32, screen_height: f32) {
        let half_width = 50.0;
        let half_height = 250.0;
        let ribbon = stage.sprite_mut(self.sprites.black_ribbon);
        ribbon.transform.x = draw_pos.x;
        ribbon.transform.y = draw_pos.y;
        ribbon.transform.scale_x = half_width * 2.0;
        ribbon.transform.scale_y = 900.0;
        ribbon.color = MenuColor::Black.rgb();
        for (k, line) in self.sprites.lines.iter().enumerate() {
            let x = if k % 2 == 0 { -half_width } else { half_width };
            let from = draw_pos + Vec2::new(x, if k < 2 { -half_height } else { half_height });
            let mut to = draw_pos + Vec2::new(x, 0.0);
            let hit = from.circles_collision_time(draw_pos, to - from, 0.0, rad);
            if !hit.is_nan() {
                to = from.lerp(to, hit.clamp(0.0, 1.0));
            }
            let length = (from.distance(to) + rad.lerp_map(51.0, 63.0, 8.0, 2.0))
                * if k < 2 { 1.0 } else { -1.0 };
            let pixel = stage.sprite_mut(line[0]);
            pixel.transform.x = from.x;
            pixel.transform.y = from.y;
            pixel.transform.scale_y = length;
            let gradient = stage.sprite_mut(line[1]);
            gradient.transform.x = from.x;
            gradient.transform.y = from.y;
            gradient.transform.scale_y = length / 200.0;
        }
        for (l, [gradient, pixel]) in self.sprites.symbol_gradients.iter().enumerate() {
            let below = l == 0;
            let y = draw_pos.y + half_height * if below { -1.0 } else { 1.0 };
            let g = stage.sprite_mut(*gradient);
            g.transform.x = draw_pos.x;
            g.transform.y = y;
            g.transform.scale_x = half_width * 2.0;
            g.transform.scale_y = half_height * 0.75 * if below { 1.0 } else { -1.0 } / 200.0;
            let p = stage.sprite_mut(*pixel);
            p.transform.x = draw_pos.x;
            p.transform.y = y;
            p.transform.scale_x = half_width * 2.0;
            p.transform.scale_y = draw_pos.y - if below { screen_height } else { 0.0 };
        }
    }

    /// `LocalBloomSprite`.
    #[must_use]
    pub fn local_bloom_sprite(&self) -> NodeId {
        self.sprites.local_bloom
    }
}

#[cfg(test)]
mod tests;
