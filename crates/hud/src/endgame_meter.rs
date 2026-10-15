//! `Menu.EndgameMeter` (530838-531649): a passage's symbol on the karma ladder's arcs,
//! its label and glow, and its meter towards the ladder: a `FloatMeter` (a bar) or a
//! `NotchMeter` (a notch per step), with the power-on flicker, the fill animation, the
//! fulfilment and their sounds and random draws. Base game with Remix off: no custom
//! passages, the Traveller's region colours are Remix's, the statistics screen's
//! `ScoreFlash` is not used here.

use std::f32::consts::PI;
use std::sync::Arc;

use rw_menu::{
    Alignment, AtlasManager, Font, Label, MenuColor, MenuSoundId, NodeId, Sprite, StageColor,
    StageTree, TextParams, Translator,
};
use rw_render::StageShader;
use rw_save::win_state::{EndgameId, EndgameTracker, Progress};
use rw_sim::{FloatExt, Vec2};

use crate::{CustomFloat, CustomVec2, LadderIo};

/// What the ladder builds its meters from: the screen's win state and the label font.
pub struct MeterSource<'a> {
    pub trackers: &'a [EndgameTracker],
    /// `Custom.GetFont()`.
    pub font: Option<Arc<Font>>,
    pub translator: &'a Translator,
}

/// What a meter's `GrafUpdate` reads of its owners.
#[derive(Debug, Clone, Copy)]
pub struct MeterFrame {
    /// The ladder's `DrawPos`.
    pub ladder: Vec2,
    pub t: f32,
    /// The screen HUD's map fade, when it has a map.
    pub map_fade: Option<f32>,
}

/// `EndgameMeter.Meter`'s two kinds.
#[derive(Debug, Clone)]
pub enum MeterKind {
    Float(FloatMeter),
    Notch(NotchMeter),
}

/// `FloatMeter`: a bar filling from the last shown progress to the new one.
#[derive(Debug, Clone)]
pub struct FloatMeter {
    side_bar: NodeId,
    tip: NodeId,
    sprites: [NodeId; 2],
    fill: f32,
    last_fill: f32,
    /// -1 for a loss, 1 for a gain, 0 for none.
    pub change_direction: i32,
}

/// `NotchMeter`: an eye per step (`JetFishEyeA`), a ring on it when filled and a glow
/// when newly filled.
#[derive(Debug, Clone)]
pub struct NotchMeter {
    notches: Vec<[NodeId; 4]>,
    custom_colors: Option<Vec<StageColor>>,
    /// Each notch's pop and its last value.
    pub pop: Vec<[f32; 2]>,
}

/// `EndgameMeter.Meter`: the stick from the ladder towards the symbol and its kind.
#[derive(Debug, Clone)]
pub struct Meter {
    stick: NodeId,
    pub extended: f32,
    pub last_extended: f32,
    /// Set by `GrafUpdate`.
    pub pulse: f32,
    pub kind: MeterKind,
}

/// One passage's meter.
#[derive(Debug, Clone)]
pub struct EndgameMeter {
    /// The tracker as the screen got it (bool-array and list progress parsed).
    pub tracker: EndgameTracker,
    /// Relative to the ladder.
    pub pos: Vec2,
    symbol: NodeId,
    circle: NodeId,
    glow: NodeId,
    /// None without the font.
    label: Option<NodeId>,
    pub pulse: f32,
    pub last_pulse: f32,
    pub energy: f32,
    pub last_energy: f32,
    pub flicker_counter: i32,
    pub powered_on: bool,
    pub meter_animation: f32,
    pub last_meter_animation: f32,
    pub animation_light_up: f32,
    pub last_animation_light_up: f32,
    pub show_as_fulfilled: f32,
    pub last_show_as_fulfilled: f32,
    pub fulfilled_now: bool,
    show_positive_change: bool,
    pub meter: Meter,
}

/// The meter colours' inputs (`EmptyColor`, `FilledColor`, `GainColor`, `LossColor`
/// and `AllColorsViaThis`) at one frame.
#[derive(Debug, Clone, Copy)]
struct MeterColors {
    pulse: f32,
    show_as_fulfilled: f32,
    fulfilled_now: bool,
    energy: f32,
    alpha: f32,
}

impl MeterColors {
    fn all_via_this(self, mut color: StageColor) -> StageColor {
        if self.show_as_fulfilled > 0.0 && self.fulfilled_now {
            let bright = MenuColor::MediumGrey
                .rgb()
                .lerp(MenuColor::White.rgb(), 0.5);
            color = color.lerp(bright, self.pulse * self.show_as_fulfilled);
        }
        if self.energy < 1.0 {
            color = MenuColor::Black
                .rgb()
                .lerp(color, self.alpha.mathf_pow(0.5));
        }
        color
    }

    fn empty(self) -> StageColor {
        self.all_via_this(MenuColor::VeryDarkGrey.rgb())
    }

    fn filled(self) -> StageColor {
        self.all_via_this(MenuColor::DarkGrey.rgb())
    }

    fn gain(self, cue: f32) -> StageColor {
        let fade = if self.fulfilled_now {
            1.0 - self.show_as_fulfilled
        } else {
            1.0
        };
        self.all_via_this(
            self.filled()
                .lerp(MenuColor::MediumGrey.rgb(), self.pulse * cue * fade),
        )
    }

    fn loss(self, cue: f32) -> StageColor {
        self.all_via_this(self.empty().lerp(
            StageColor::rgb(1.0, 0.0, 0.0),
            (1.0 - self.pulse) * 0.5 * cue,
        ))
    }
}

/// Where a meter's stick runs this frame.
#[derive(Debug, Clone, Copy)]
struct Stick {
    start: Vec2,
    tip: Vec2,
}

impl Stick {
    fn length(self) -> f32 {
        self.start.distance(self.tip)
    }

    fn rotation(self) -> f32 {
        (self.tip - self.start).to_degrees()
    }
}

impl EndgameMeter {
    /// The constructor: its symbol, label, circle and glow in `main`, its meter's stick
    /// (and a float meter's side bar) in `bkg`.
    pub fn new(
        stage: &mut StageTree,
        [bkg, main]: [NodeId; 2],
        pos: Vec2,
        tracker: &EndgameTracker,
        source: &MeterSource<'_>,
    ) -> Self {
        let tracker = EndgameTracker {
            progress: tracker.parsed(),
            ..tracker.clone()
        };
        let fulfilled = tracker.goal_fulfilled();
        let already = tracker.goal_already_fulfilled();
        let fulfilled_now = fulfilled && !already;
        let show_as_fulfilled = if fulfilled && already { 1.0 } else { 0.0 };
        let id = tracker.id;
        let symbol = stage.add_sprite(
            main,
            Sprite::new(&format!(
                "{}{}",
                id.value(),
                if show_as_fulfilled > 0.5 { "A" } else { "B" }
            )),
        );
        let label = Self::label(stage, main, id, source);
        let circle = stage.add_sprite(main, Sprite::new("EndGameCircle"));
        let mut glow = Sprite::new(AtlasManager::WHITE);
        glow.shader = StageShader::FlatLight;
        let glow = stage.add_sprite(main, glow);
        let notched = match &tracker.progress {
            _ if id == EndgameId::Survivor => true,
            Progress::Bools { progress, .. } => progress.len() < 15,
            Progress::List { .. } => EndgameId::LIST_MAX < 15,
            _ => false,
        };
        let stick = Meter::stick(stage, bkg);
        let kind = if notched {
            MeterKind::Notch(NotchMeter::new(stage, main, &tracker))
        } else {
            MeterKind::Float(FloatMeter::new(stage, [bkg, main], &tracker))
        };
        let mut meter = Self {
            tracker,
            pos,
            symbol,
            circle,
            glow,
            label,
            pulse: 0.0,
            last_pulse: 0.0,
            energy: 0.0,
            last_energy: 0.0,
            flicker_counter: 0,
            powered_on: false,
            meter_animation: 0.0,
            last_meter_animation: 0.0,
            animation_light_up: 0.0,
            last_animation_light_up: 0.0,
            show_as_fulfilled,
            last_show_as_fulfilled: show_as_fulfilled,
            fulfilled_now,
            show_positive_change: false,
            meter: Meter {
                stick,
                extended: 0.0,
                last_extended: 0.0,
                pulse: 0.0,
                kind,
            },
        };
        if meter.any_change_to_show() {
            meter.show_positive_change = match &meter.tracker.progress {
                Progress::Bools { .. } => true,
                Progress::Integer {
                    progress,
                    last_shown,
                } => last_shown < progress,
                Progress::Float {
                    progress,
                    last_shown,
                    ..
                } => last_shown < progress,
                _ => false,
            };
        }
        meter
    }

    /// The passage's name (`<name>-linebreaks` when the strings have it), lines split.
    fn label(
        stage: &mut StageTree,
        main: NodeId,
        id: EndgameId,
        source: &MeterSource<'_>,
    ) -> Option<NodeId> {
        let name = id.display_name();
        let broken = format!("{name}-linebreaks");
        let text = if source.translator.strings().entries.contains_key(&broken) {
            source.translator.translate(&broken)
        } else {
            source.translator.translate(name)
        };
        let text = text.replace("<LINE>", "\r\n").replace("<WWLINE>", "");
        let font = Arc::clone(source.font.as_ref()?);
        let params = TextParams {
            line_height_offset: -3.0,
            ..TextParams::default()
        };
        Some(stage.add_label(main, Label::with_params(font, &text, params)))
    }

    /// `AnyChangeToShow` (list trackers only with Remix).
    #[must_use]
    pub fn any_change_to_show(&self) -> bool {
        match &self.tracker.progress {
            Progress::Bools {
                progress,
                last_shown,
            } => progress.iter().zip(last_shown).any(|(p, l)| p != l),
            Progress::Integer {
                progress,
                last_shown,
            } => progress != last_shown,
            Progress::Float {
                progress,
                last_shown,
                ..
            } => progress != last_shown,
            _ => false,
        }
    }

    /// The sound the ladder starts this meter's animation with.
    #[must_use]
    pub fn change_sound(&self) -> Option<MenuSoundId> {
        match &self.meter.kind {
            MeterKind::Float(f) if f.change_direction > 0 => {
                Some(MenuSoundId::EndgameFloatMeterStartAnimationUp)
            }
            MeterKind::Float(f) if f.change_direction < 0 => {
                Some(MenuSoundId::EndgameFloatMeterStartAnimationDown)
            }
            MeterKind::Float(_) => None,
            MeterKind::Notch(_) => Some(MenuSoundId::EndgameNotchMeterStartAnimation),
        }
    }

    /// `Update`: the pulse; powered on, the energy rises unless flickering (one draw, a
    /// second while flickering, a third when that dims it); the meter's animation once
    /// fully lit, the fulfilment and the light-up; then the meter's own update.
    pub fn update(&mut self, io: &mut LadderIo<'_>) {
        self.last_pulse = self.pulse;
        self.pulse += 1.0;
        self.last_energy = self.energy;
        if self.powered_on {
            if io.rng.value() > (self.flicker_counter as f32).inverse_lerp(5.0, 40.0) {
                self.energy = self.energy.lerp_and_tick(1.0, 0.1, 1.0 / 70.0);
            }
            if self.flicker_counter > 0 {
                self.flicker_counter -= 1;
                let flicker = self.flicker_counter as f32;
                if io.rng.value() < flicker.inverse_lerp(5.0, 19.0) {
                    let dim = io
                        .rng
                        .value()
                        .mathf_pow(flicker.lerp_map(50.0, 2.0, 3.0, 0.2));
                    self.energy = self.energy.min(self.energy.lerp(dim, 0.5));
                }
            }
        } else {
            self.energy = 0.0;
        }
        self.last_meter_animation = self.meter_animation;
        if self.meter_animation > 0.0 && self.energy > 0.99 {
            self.meter_animation = (self.meter_animation + 1.0 / 62.0).min(1.0);
        }
        self.last_show_as_fulfilled = self.show_as_fulfilled;
        if self.meter_animation == 1.0 && self.fulfilled_now {
            if self.show_as_fulfilled == 0.0 {
                io.play(MenuSoundId::EndgameMeterFullfilled);
                io.stage.sprite_mut(self.symbol).element = format!("{}A", self.tracker.id.value());
                // `CueAchievement`: the port has no achievements.
            }
            self.show_as_fulfilled = (self.show_as_fulfilled + 1.0 / 24.0).min(1.0);
        }
        self.last_animation_light_up = self.animation_light_up;
        let animating = self.meter_animation > 0.0 && self.meter_animation < 1.0;
        if (self.show_positive_change && animating)
            || (self.meter_animation > 0.0 && self.fulfilled_now)
        {
            self.animation_light_up = self.animation_light_up.lerp_and_tick(1.0, 0.05, 0.0125);
        } else {
            self.animation_light_up =
                self.animation_light_up
                    .lerp_and_tick(0.0, 0.001, 0.004_166_667);
        }
        self.meter.last_extended = self.meter.extended;
        let speed = 1.0 / 600.0_f32.lerp(10.0, self.energy);
        let to = if self.powered_on { 1.0 } else { 0.0 };
        self.meter.extended = self.meter.extended.lerp_and_tick(to, 0.05, speed);
        if let MeterKind::Notch(n) = &mut self.meter.kind {
            n.update(
                &self.tracker,
                self.last_meter_animation,
                self.meter_animation,
                io,
            );
        }
    }

    /// `GrafUpdate`: the symbol, circle, label and glow, then the meter.
    pub fn draw(&mut self, stage: &mut StageTree, f: &MeterFrame) {
        let t = f.t;
        let pos = f.ladder + self.pos;
        let energy = self.last_energy.lerp(self.energy, t);
        let shown = self.last_show_as_fulfilled.lerp(self.show_as_fulfilled, t);
        let light = self
            .last_animation_light_up
            .lerp(self.animation_light_up, t);
        let bright = if self.fulfilled_now {
            MenuColor::White
        } else {
            MenuColor::MediumGrey
        };
        let color = MenuColor::VeryDarkGrey
            .rgb()
            .lerp(bright.rgb(), shown.mathf_pow(0.2).max(light));
        let symbol = stage.sprite_mut(self.symbol);
        symbol.transform.x = pos.x;
        symbol.transform.y = pos.y;
        symbol.color = color;
        symbol.transform.alpha = energy;
        let circle = stage.sprite_mut(self.circle);
        circle.transform.x = pos.x;
        circle.transform.y = pos.y;
        circle.color = color;
        circle.transform.alpha = shown.mathf_pow(0.1) * energy;
        if let Some(label) = self.label {
            let label = stage.label_mut(label);
            label.transform.y = pos.y;
            label.color = color;
            label.transform.alpha = energy * light;
            if pos.x > f.ladder.x {
                label.transform.x = pos.x + 25.0;
                label.set_alignment(Alignment::Left);
            } else {
                label.transform.x = pos.x - 25.0;
                label.set_alignment(Alignment::Right);
            }
        }
        let wave = (shown.mathf_pow(0.3) * PI).sin();
        let glow = stage.sprite_mut(self.glow);
        glow.transform.x = pos.x;
        glow.transform.y = pos.y;
        let now = if self.fulfilled_now { 0.1 * shown } else { 0.0 };
        glow.transform.alpha = (0.2 * self.show_as_fulfilled + 0.2 * wave + now) * energy;
        let now = if self.fulfilled_now {
            5.0 + self.meter.pulse * 20.0 * shown
        } else {
            0.0
        };
        glow.transform.set_scale((90.0 + 20.0 * wave + now) / 16.0);
        self.draw_meter(stage, pos, f);
    }

    /// `Meter.GrafUpdate` and its kind's.
    fn draw_meter(&mut self, stage: &mut StageTree, symbol: Vec2, f: &MeterFrame) {
        let t = f.t;
        let tip = symbol + symbol.direction_to(f.ladder) * 35.0;
        let side = (tip.x - f.ladder.x).mathf_sign();
        let start = f.ladder.vertical_cross_point(tip, f.ladder.x + side * 50.0);
        let extended = self.meter.last_extended.lerp(self.meter.extended, t);
        let stick = Stick {
            start,
            tip: start.lerp(tip, extended.s_curve(0.6).clamp(0.0, 1.0)),
        };
        self.meter.pulse =
            0.5 + 0.5 * (self.last_pulse.lerp(self.pulse, t) / 60.0 * PI * 2.0).sin();
        let alpha = self
            .last_energy
            .lerp(self.energy, t)
            .inverse_lerp(0.5, 1.0)
            .mathf_pow(2.0)
            * extended;
        let colors = MeterColors {
            pulse: self.meter.pulse,
            show_as_fulfilled: self.show_as_fulfilled,
            fulfilled_now: self.fulfilled_now,
            energy: self.energy,
            alpha,
        };
        let s = stage.sprite_mut(self.meter.stick);
        s.transform.x = stick.start.x;
        s.transform.y = stick.start.y;
        s.transform.scale_y = stick.length();
        s.transform.rotation = stick.rotation();
        s.color = colors.empty();
        let animation = self.last_meter_animation.lerp(self.meter_animation, t);
        match &mut self.meter.kind {
            MeterKind::Float(m) => m.draw(stage, stick, colors, animation, f.map_fade),
            MeterKind::Notch(m) => m.draw(stage, stick, colors, &self.tracker, t),
        }
    }

    /// The symbol's sprite.
    #[must_use]
    pub fn symbol_sprite(&self) -> NodeId {
        self.symbol
    }
}

impl Meter {
    /// The stick: a 2 px `pixel` anchored at its bottom, behind the symbols.
    fn stick(stage: &mut StageTree, bkg: NodeId) -> NodeId {
        let mut stick = Sprite::new("pixel");
        stick.transform.scale_x = 2.0;
        stick.anchor[1] = 0.0;
        stage.add_sprite(bkg, stick)
    }
}

impl FloatMeter {
    fn new(stage: &mut StageTree, [bkg, main]: [NodeId; 2], tracker: &EndgameTracker) -> Self {
        let mut side_bar = Sprite::new("pixel");
        side_bar.transform.scale_x = 8.0;
        side_bar.transform.scale_y = 2.0;
        side_bar.anchor[1] = 1.0;
        let side_bar = stage.add_sprite(bkg, side_bar);
        let mut tip = Sprite::new("pixel");
        tip.transform.scale_x = 4.0;
        tip.transform.scale_y = 2.0;
        tip.anchor[1] = 1.0;
        let tip = stage.add_sprite(main, tip);
        let show_from = tracker.id.show_from();
        let (fill, last_fill) = match &tracker.progress {
            Progress::Bools {
                progress,
                last_shown,
            } => {
                let n = progress.len() as f32;
                (
                    progress.iter().filter(|&&b| b).count() as f32 / n,
                    last_shown.iter().filter(|&&b| b).count() as f32 / n,
                )
            }
            Progress::List { progress, last } => {
                let n = EndgameId::LIST_MAX as f32;
                (progress.len() as f32 / n, last.len() as f32 / n)
            }
            Progress::Float {
                progress,
                last_shown,
                ..
            } => (
                progress.inverse_lerp(show_from as f32, 1.0),
                last_shown.inverse_lerp(show_from as f32, 1.0),
            ),
            Progress::Integer {
                progress,
                last_shown,
            } => {
                let max = tracker.id.integer_max().unwrap_or(0) as f32;
                (
                    (*progress as f32).inverse_lerp(show_from as f32, max),
                    (*last_shown as f32).inverse_lerp(show_from as f32, max),
                )
            }
            Progress::Raw(_) => (0.0, 0.0),
        };
        let change_direction = if fill < last_fill {
            -1
        } else {
            i32::from(fill > last_fill)
        };
        let mut sprite = Sprite::new("pixel");
        sprite.transform.scale_x = 2.0;
        sprite.anchor[1] = 0.0;
        let first = stage.add_sprite(
            if change_direction > -1 { main } else { bkg },
            sprite.clone(),
        );
        let second = stage.add_sprite(main, sprite);
        Self {
            side_bar,
            tip,
            sprites: [first, second],
            fill,
            last_fill,
            change_direction,
        }
    }

    fn draw(
        &self,
        stage: &mut StageTree,
        stick: Stick,
        colors: MeterColors,
        animation: f32,
        map_fade: Option<f32>,
    ) {
        let rotation = stick.rotation();
        let length = stick.length();
        let side = stage.sprite_mut(self.side_bar);
        side.transform.x = stick.tip.x;
        side.transform.y = stick.tip.y;
        side.transform.rotation = rotation;
        side.color = colors.empty();
        let mut cue = animation.inverse_lerp(0.5, 1.0).mathf_pow(3.0);
        if let Some(fade) = map_fade {
            cue *= 1.0 - fade;
        }
        let fill = self
            .last_fill
            .lerp(self.fill, animation.mathf_pow(0.65).s_curve(0.4));
        let tip = stage.sprite_mut(self.tip);
        tip.transform.x = stick.start.x.lerp(stick.tip.x, fill);
        tip.transform.y = stick.start.y.lerp(stick.tip.y, fill);
        tip.transform.rotation = rotation;
        tip.transform.alpha = colors.alpha;
        tip.color = match self.change_direction {
            d if d > 0 => colors.gain(cue),
            _ => colors.filled(),
        };
        stage.sprite_mut(self.sprites[1]).color = colors.filled();
        for s in self.sprites {
            let s = stage.sprite_mut(s);
            s.transform.x = stick.start.x;
            s.transform.y = stick.start.y;
            s.transform.rotation = rotation;
            s.transform.alpha = colors.alpha;
        }
        let [first, second] = self.sprites;
        match self.change_direction {
            d if d < 0 => {
                let s = stage.sprite_mut(first);
                s.color = colors.loss(cue);
                s.transform.scale_y = length * self.last_fill;
                stage.sprite_mut(second).transform.scale_y = length * fill;
            }
            d if d > 0 => {
                let s = stage.sprite_mut(first);
                s.color = colors.gain(cue);
                s.transform.scale_y = length * fill;
                stage.sprite_mut(second).transform.scale_y = length * self.last_fill;
            }
            _ => stage.sprite_mut(second).transform.scale_y = length * fill,
        }
    }
}

impl NotchMeter {
    /// The base game's `lizardsOrder` colours (`LizardBreedParams.standardColor`).
    const LIZARD_COLORS: [StageColor; 6] = [
        StageColor::rgb(0.2, 1.0, 0.0),
        StageColor::rgb(1.0, 0.0, 1.0),
        StageColor::rgb(0.0, 0.5, 1.0),
        StageColor::rgb(1.0, 1.0, 1.0),
        StageColor::rgb(1.0, 0.6, 0.0),
        StageColor::rgb(0.1, 0.1, 0.1),
    ];

    fn new(stage: &mut StageTree, main: NodeId, tracker: &EndgameTracker) -> Self {
        let count = Self::count(tracker);
        let mut notches = Vec::with_capacity(count);
        for _ in 0..count {
            let eye = stage.add_sprite(main, Sprite::new("JetFishEyeA"));
            let ring = stage.add_sprite(main, Sprite::new("haloGlyph-1"));
            notches.push([eye, ring, eye, eye]);
        }
        for n in &mut notches {
            n[2] = stage.add_sprite(main, Sprite::new("haloGlyph-1"));
            let mut glow = Sprite::new(AtlasManager::WHITE);
            glow.shader = StageShader::FlatLight;
            n[3] = stage.add_sprite(main, glow);
        }
        let custom_colors = match (&tracker.id, &tracker.progress) {
            (EndgameId::DragonSlayer, _) => Some(Self::LIZARD_COLORS.to_vec()),
            (EndgameId::Scholar, Progress::List { progress, .. }) => {
                let mut colors = vec![StageColor::rgb(0.0, 0.0, 0.0); EndgameId::LIST_MAX];
                for c in &mut colors {
                    c.a = 0.0;
                }
                for (c, &pearl) in colors.iter_mut().zip(progress) {
                    *c = PearlColor::of_index(pearl).notch_color();
                }
                Some(colors)
            }
            _ => None,
        };
        Self {
            notches,
            custom_colors,
            pop: vec![[0.0; 2]; count],
        }
    }

    /// The number of notches: the list's or array's length, or the integer's steps above
    /// `showFrom`.
    fn count(tracker: &EndgameTracker) -> usize {
        match &tracker.progress {
            Progress::List { .. } => EndgameId::LIST_MAX,
            Progress::Bools { progress, .. } => progress.len(),
            _ => {
                let max = tracker.id.integer_max().unwrap_or(0);
                usize::try_from(max - tracker.id.show_from()).unwrap_or(0)
            }
        }
    }

    /// `NotchFilled(i)` (`last` false) and `NotchLastFilled(i)` (true).
    fn filled(tracker: &EndgameTracker, i: usize, last: bool) -> bool {
        match &tracker.progress {
            Progress::List { progress, last: l } => i < if last { l.len() } else { progress.len() },
            Progress::Bools {
                progress,
                last_shown,
            } => if last { last_shown } else { progress }
                .get(i)
                .copied()
                .unwrap_or(false),
            Progress::Integer {
                progress,
                last_shown,
            } => {
                let p = if last { *last_shown } else { *progress };
                i64::from(p - tracker.id.show_from()) > i as i64
            }
            _ => false,
        }
    }

    /// `Update`: a notch pops (with a sound) as the animation passes it, if it fills
    /// now; one emptied now drops.
    fn update(
        &mut self,
        tracker: &EndgameTracker,
        last_animation: f32,
        animation: f32,
        io: &mut LadderIo<'_>,
    ) {
        let n = self.notches.len();
        for i in 0..n {
            self.pop[i][1] = self.pop[i][0];
            let at = (i + 1) as f32 / n as f32;
            if last_animation < at && animation >= at {
                let filled = Self::filled(tracker, i, false);
                let last = Self::filled(tracker, i, true);
                if filled && !last {
                    self.pop[i][0] = 0.01;
                    io.play(MenuSoundId::EndgameNotchMeterSlotFill);
                } else if !filled && last {
                    self.pop[i][0] = -1.0;
                }
            }
            if self.pop[i][0] > 0.0 {
                self.pop[i][0] = (self.pop[i][0] + 0.1).min(1.0);
            }
        }
    }

    fn draw(
        &self,
        stage: &mut StageTree,
        stick: Stick,
        colors: MeterColors,
        tracker: &EndgameTracker,
        t: f32,
    ) {
        let n = self.notches.len();
        for (i, &[eye, ring, filled_ring, glow]) in self.notches.iter().enumerate() {
            let pop = self.pop[i][1].lerp(self.pop[i][0], t);
            let mut at = stick.start.lerp(stick.tip, (i + 1) as f32 / n as f32);
            at.x = at.x.floor() + 0.5;
            at.y = at.y.floor() + 0.5;
            for s in [eye, ring, filled_ring, glow] {
                let s = stage.sprite_mut(s);
                s.transform.x = at.x;
                s.transform.y = at.y;
                s.transform.alpha = colors.alpha;
            }
            let filled = Self::filled(tracker, i, false);
            let last = Self::filled(tracker, i, true);
            if last && !filled {
                stage.sprite_mut(ring).transform.visible = true;
                let s = stage.sprite_mut(filled_ring);
                s.color = colors.loss(1.0);
                s.transform.alpha = (1.0 - colors.pulse) * 0.5;
                s.transform.visible = true;
                stage.sprite_mut(glow).transform.visible = false;
            } else if (filled && (pop > 0.0 || last)) || (last && (pop != -1.0 || filled)) {
                let custom = self.custom_colors.as_ref().and_then(|c| c.get(i)).copied();
                let s = stage.sprite_mut(filled_ring);
                s.transform.visible = true;
                s.color = match custom {
                    _ if !last => colors.gain(1.0),
                    None => colors.filled(),
                    Some(c) => colors.all_via_this(c).lerp(colors.filled(), 0.5),
                };
                let s = stage.sprite_mut(glow);
                if last {
                    s.transform.visible = false;
                } else {
                    let wave = (pop.mathf_pow(0.5) * PI).sin();
                    s.transform.visible = true;
                    s.transform
                        .set_scale(0.7 + 0.5 * colors.pulse.lerp(2.0, wave));
                    s.transform.alpha =
                        0.15_f32.lerp(0.3, colors.pulse).lerp(0.7, wave) * colors.alpha;
                    if let Some(c) = custom {
                        s.color = colors.all_via_this(c).lerp(StageColor::WHITE, 0.5);
                    }
                }
            } else {
                stage.sprite_mut(filled_ring).transform.visible = false;
                stage.sprite_mut(glow).transform.visible = false;
            }
            stage.sprite_mut(eye).color = colors.empty();
            stage.sprite_mut(ring).color = MenuColor::Black.rgb();
        }
    }
}

/// `DataPearl.UniquePearlMainColor` and `UniquePearlHighLightColor` of a base-game
/// `DataPearlType`, for the Scholar's notches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PearlColor {
    Misc,
    Misc2,
    Cc,
    SiWest,
    SiTop,
    LfWest,
    LfBottom,
    Hi,
    Sh,
    Ds,
    SbFiltration,
    SbRavine,
    Gw,
    SlBridge,
    SlMoon,
    Su,
    Uw,
    PebblesPearl,
    SlChimney,
    RedStomach,
}

impl PearlColor {
    /// `DataPearlType`'s entries with every DLC off, in index order.
    pub const ALL: [Self; 20] = [
        Self::Misc,
        Self::Misc2,
        Self::Cc,
        Self::SiWest,
        Self::SiTop,
        Self::LfWest,
        Self::LfBottom,
        Self::Hi,
        Self::Sh,
        Self::Ds,
        Self::SbFiltration,
        Self::SbRavine,
        Self::Gw,
        Self::SlBridge,
        Self::SlMoon,
        Self::Su,
        Self::Uw,
        Self::PebblesPearl,
        Self::SlChimney,
        Self::RedStomach,
    ];

    /// The pearl of a Scholar list entry: `values.GetEntry(index)`, `Misc` when not
    /// registered.
    #[must_use]
    pub fn of_index(index: i32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i))
            .copied()
            .unwrap_or(Self::Misc)
    }

    #[must_use]
    pub fn main(self) -> StageColor {
        let (r, g, b) = match self {
            Self::Cc => (0.9, 0.6, 0.1),
            Self::Ds => (0.0, 0.7, 0.1),
            Self::Gw => (0.0, 0.7, 0.5),
            Self::Hi => (2.0 / 255.0, 50.0 / 255.0, 1.0),
            Self::LfBottom => (1.0, 0.1, 0.1),
            Self::LfWest => (1.0, 0.0, 0.3),
            Self::SbFiltration => (0.1, 0.5, 0.5),
            Self::Sh => (0.2, 0.0, 0.1),
            Self::SiTop | Self::SiWest | Self::SbRavine => (0.01, 0.01, 0.01),
            Self::SlBridge => (0.4, 0.1, 0.9),
            Self::SlMoon => (0.9, 0.95, 0.2),
            Self::Su => (0.5, 0.6, 0.9),
            Self::Uw => (0.4, 0.6, 0.4),
            Self::SlChimney => (1.0, 0.0, 0.55),
            Self::RedStomach => (0.6, 1.0, 0.9),
            Self::Misc | Self::Misc2 | Self::PebblesPearl => (0.0, 0.0, 0.0),
        };
        StageColor::rgb(r, g, b)
    }

    #[must_use]
    pub fn highlight(self) -> Option<StageColor> {
        let (r, g, b) = match self {
            Self::Cc => (1.0, 1.0, 0.0),
            Self::Gw => (0.5, 1.0, 0.5),
            Self::Hi => (0.5, 0.8, 1.0),
            Self::Sh => (1.0, 0.2, 0.6),
            Self::SiTop => (0.1, 0.4, 0.6),
            Self::SiWest => (0.1, 0.6, 0.4),
            Self::SlBridge => (1.0, 0.4, 1.0),
            Self::SbRavine => (0.6, 0.1, 0.4),
            Self::Uw => (1.0, 0.7, 1.0),
            Self::SlChimney => (0.8, 0.3, 1.0),
            Self::RedStomach => (1.0, 1.0, 1.0),
            _ => return None,
        };
        Some(StageColor::rgb(r, g, b))
    }

    /// The Scholar notch's colour: the main colour 40% to the highlight, then
    /// `Custom.Saturate(0.2)`.
    #[must_use]
    pub fn notch_color(self) -> StageColor {
        let main = self.main();
        let c = match self.highlight() {
            Some(h) => main.lerp(h, 0.4),
            None => main,
        };
        let lo = c.r.min(c.g).min(c.b);
        let hi = c.r.max(c.g).max(c.b);
        let full = StageColor::rgb(
            c.r.inverse_lerp(lo, hi),
            c.g.inverse_lerp(lo, hi),
            c.b.inverse_lerp(lo, hi),
        );
        c.lerp(full, 0.2)
    }
}

#[cfg(test)]
mod tests;
