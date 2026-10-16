//! `MainLoopProcess` and `ProcessManager`: one main process at a time (the game, a
//! menu, a slideshow ...), switched behind a fade to black. Behaviour:
//! `rainworld-re/notes/menu.md` ("Processes and the fade").
//!
//! The side processes (the music player and the menu microphone) run through
//! [`ProcessFactory::update_side_processes`]; the switch queue only holds requests
//! while a dialog runs, so with none it is always empty. Mod finalization between the two halves of a switch finishes at once,
//! as with no mods to enable or disable.

use std::collections::VecDeque;

use rw_audio::music::MusicProcess;

use crate::atlas::AtlasManager;
use crate::font::Font;
use crate::label::Label;
use crate::stage::{Container, Node, NodeTransform, Sprite, StageColor};
use crate::translator::Translator;
use rw_graphics::Mathf;
use rw_render::StageShader;

/// `ProcessManager.ProcessID` (base game), plus the port's own test screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessId {
    MainMenu,
    Game,
    SleepScreen,
    DeathScreen,
    StarveScreen,
    RegionSelect,
    OptionsMenu,
    MusicPlayer,
    GhostScreen,
    KarmaToMaxScreen,
    SlideShow,
    MenuMic,
    PauseMenu,
    FastTravelScreen,
    WarpFastTravelScreen,
    RegionsOverviewScreen,
    CustomEndGameScreen,
    InputSelect,
    TutorialControlsPage,
    SlugcatSelect,
    IntroRoll,
    Credits,
    ConsoleOptionsMenu,
    Dream,
    RainWorldSteamManager,
    MultiplayerMenu,
    MultiplayerResults,
    InputOptions,
    Statistics,
    Dialog,
    ModdingMenu,
    DemoModeEnd,
    Initialization,
    MonkActivity,
    BackupManager,
    /// Port only: a screen of labels in both fonts and a flat illustration, for
    /// comparing text and image drawing with the game by eye.
    FontTest,
}

impl ProcessId {
    /// The process as `MusicPlayer.UpdateMusicContext` tells them apart (the port's game
    /// is a story session).
    #[must_use]
    pub fn music_process(self) -> MusicProcess {
        match self {
            Self::Game => MusicProcess::StoryGame,
            Self::SlugcatSelect => MusicProcess::SlugcatSelect,
            Self::RegionsOverviewScreen => MusicProcess::RegionsOverviewScreen,
            Self::MultiplayerMenu => MusicProcess::MultiplayerMenu,
            Self::MainMenu => MusicProcess::MainMenu,
            _ => MusicProcess::Other,
        }
    }
}

/// `MainLoopProcess.myTimeStacker`: real time in ticks, run by `RawUpdate`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimeStacker(pub f32);

impl TimeStacker {
    /// The ticks `RawUpdate` runs for a frame of `dt` seconds at `fps`: one per whole
    /// tick strictly above 1 accumulated; after the third the remainder is dropped.
    pub fn advance(&mut self, dt: f32, fps: i32) -> u32 {
        self.0 += dt * fps as f32;
        let mut ticks = 0;
        while self.0 > 1.0 {
            self.0 -= 1.0;
            ticks += 1;
            if ticks > 2 {
                self.0 = 0.0;
            }
        }
        ticks
    }
}

/// What a process asks of the manager during its update (`manager.RequestMainProcessSwitch`),
/// and the manager's state it reads (`upcomingProcess`, `fadeToBlack`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessRequests {
    switches: Vec<(ProcessId, f32)>,
    /// `upcomingProcess`, including a switch requested during this update.
    upcoming: Option<ProcessId>,
    /// A switch is between its two halves (`processAfterModFinalization`).
    switching: bool,
    fade_to_black: f32,
    /// `FadeDelayInProgress`: `blackDelay > 0`.
    fade_delay_in_progress: bool,
    /// `manager.fadeToBlack = x` written by the process.
    set_fade: Option<f32>,
}

impl ProcessRequests {
    /// The default fade-out time of a switch, seconds.
    pub const FADE_OUT_SECONDS: f32 = 0.45;

    /// The manager's state at the start of a process update.
    #[must_use]
    pub fn new(upcoming: Option<ProcessId>, switching: bool, fade_to_black: f32) -> Self {
        Self {
            switches: Vec::new(),
            upcoming,
            switching,
            fade_to_black,
            fade_delay_in_progress: false,
            set_fade: None,
        }
    }

    /// `RequestMainProcessSwitch(id)`.
    pub fn switch_to(&mut self, id: ProcessId) {
        self.switch_to_with_fade(id, Self::FADE_OUT_SECONDS);
    }

    /// `RequestMainProcessSwitch(id, fadeOutSeconds)`: taken at once unless a switch
    /// is already under way.
    pub fn switch_to_with_fade(&mut self, id: ProcessId, fade_out_seconds: f32) {
        if self.upcoming.is_none() && !self.switching {
            self.upcoming = Some(id);
        }
        self.switches.push((id, fade_out_seconds));
    }

    /// `manager.upcomingProcess`.
    #[must_use]
    pub fn upcoming(&self) -> Option<ProcessId> {
        self.upcoming
    }

    /// `manager.fadeToBlack`.
    #[must_use]
    pub fn fade_to_black(&self) -> f32 {
        self.fade_to_black
    }

    /// `manager.fadeToBlack = value` (the slideshow drives the fade itself).
    pub fn set_fade_to_black(&mut self, value: f32) {
        self.fade_to_black = value;
        self.set_fade = Some(value);
    }

    /// `manager.FadeDelayInProgress`.
    #[must_use]
    pub fn fade_delay_in_progress(&self) -> bool {
        self.fade_delay_in_progress
    }
}

/// A main process, updated with the app's context `C`.
pub trait MainLoopProcess<C> {
    fn id(&self) -> ProcessId;

    /// `framesPerSecond`: ticks per second of real time.
    fn frames_per_second(&self) -> i32 {
        40
    }

    /// `TimeSpeedFac`: how fast the shaders' time runs.
    fn time_speed_fac(&self) -> f32 {
        self.frames_per_second() as f32 / 40.0
    }

    /// `FadeInTime`: seconds of the fade from black once this process starts.
    fn fade_in_time(&self) -> f32 {
        0.45
    }

    /// `InitialBlackSeconds`: seconds of black before that fade.
    fn initial_black_seconds(&self) -> f32 {
        0.0
    }

    /// The constructor called `manager.RemoveLoadingLabel()`.
    fn removes_loading_label(&self) -> bool {
        false
    }

    /// One tick (`Update`).
    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests);

    /// Once per frame after the ticks, with the fraction of a tick accumulated.
    fn graf_update(&mut self, time_stacker: f32, ctx: &mut C);

    /// `RawUpdate`: the frame's ticks, then `graf_update`.
    fn raw_update(
        &mut self,
        dt: f32,
        stacker: &mut TimeStacker,
        ctx: &mut C,
        requests: &mut ProcessRequests,
    ) {
        for _ in 0..stacker.advance(dt, self.frames_per_second()) {
            self.update(ctx, requests);
        }
        self.graf_update(stacker.0, ctx);
    }

    /// `ShutDownProcess`.
    fn shut_down(&mut self, _ctx: &mut C) {}

    /// `CommunicateWithUpcomingProcess`.
    fn communicate_with_upcoming(&mut self, _next: &mut Self, _ctx: &mut C)
    where
        Self: Sized,
    {
    }
}

/// Builds processes by ID, as `PostSwitchMainProcess` does, and runs the side
/// processes.
pub trait ProcessFactory<P> {
    /// The process `id`; `old` is the one that just ended, if any.
    fn create_process(&mut self, id: ProcessId, old: Option<&P>) -> P;

    /// The side processes' frame (`sideProcesses[i].RawUpdate`) after the main
    /// process's; `current` is `None` between the halves of a switch.
    fn update_side_processes(&mut self, _current: Option<&mut P>, _dt: f32) {}

    /// The start of `PreSwitchMainProcess(next)`, before the current process ends.
    fn pre_switch(&mut self, _next: ProcessId) {}

    /// The end of `PostSwitchMainProcess`, with the new process.
    fn post_switch(&mut self, _current: &P) {}

    /// `ProcessManager.Update` before the main process's frame (the music player's
    /// creation).
    fn frame_start(&mut self, _current: Option<&P>) {}
}

/// `ProcessManager.fadeSprite`: a black `Futile_White` quad over the screen drawn
/// with `EdgeFade`, sized and faded by `UpdateFade`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FadeSprite {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub alpha: f32,
}

/// The process manager.
pub struct ProcessManager<P> {
    screen_size: [f32; 2],
    current: Option<P>,
    stacker: TimeStacker,
    /// `oldProcess`: the process ended by the last switch.
    old: Option<P>,
    upcoming: Option<ProcessId>,
    after_mod_finalization: Option<ProcessId>,
    mod_finalization_done: bool,
    queue: VecDeque<(ProcessId, f32)>,
    black_delay: f32,
    black_fade_time: f32,
    fade_to_black: f32,
    pause_fade_update: bool,
    fade_sprite: Option<FadeSprite>,
    /// The "Loading..." label's alpha while it is shown.
    loading_label: Option<f32>,
    /// Seconds of shader time (`shadersTime`, `_RAIN = shadersTime / 5`).
    shaders_time: f32,
}

impl<P> ProcessManager<P> {
    /// `new ProcessManager`: black (`fadeToBlack` 1), then the `Initialization`
    /// process. The fade sprite starts transparent until the first `UpdateFade`.
    pub fn new<C>(screen_size: [f32; 2], ctx: &mut C) -> Self
    where
        P: MainLoopProcess<C>,
        C: ProcessFactory<P>,
    {
        let mut manager = Self {
            screen_size,
            current: None,
            stacker: TimeStacker::default(),
            old: None,
            upcoming: None,
            after_mod_finalization: None,
            mod_finalization_done: false,
            queue: VecDeque::new(),
            black_delay: 0.0,
            black_fade_time: ProcessRequests::FADE_OUT_SECONDS,
            fade_to_black: 0.0,
            pause_fade_update: false,
            fade_sprite: None,
            loading_label: None,
            shaders_time: 0.0,
        };
        manager.init_fade_sprite();
        manager.fade_to_black = 1.0;
        manager.pre_switch(ProcessId::Initialization, ctx);
        manager.post_switch(ProcessId::Initialization, ctx);
        manager
    }

    #[must_use]
    pub fn current(&self) -> Option<&P> {
        self.current.as_ref()
    }

    pub fn current_mut(&mut self) -> Option<&mut P> {
        self.current.as_mut()
    }

    /// The fraction of a tick the current process has accumulated.
    #[must_use]
    pub fn time_stacker(&self) -> f32 {
        self.stacker.0
    }

    #[must_use]
    pub fn fade_to_black(&self) -> f32 {
        self.fade_to_black
    }

    #[must_use]
    pub fn fade_sprite(&self) -> Option<FadeSprite> {
        self.fade_sprite
    }

    #[must_use]
    pub fn loading_label_alpha(&self) -> Option<f32> {
        self.loading_label
    }

    #[must_use]
    pub fn upcoming(&self) -> Option<ProcessId> {
        self.upcoming
    }

    #[must_use]
    pub fn shaders_time(&self) -> f32 {
        self.shaders_time
    }

    /// `FadeDelayInProgress`: `blackDelay > 0`.
    #[must_use]
    pub fn fade_delay_in_progress(&self) -> bool {
        self.black_delay > 0.0
    }

    /// `RequestMainProcessSwitch` from outside a process update.
    pub fn request_switch(&mut self, id: ProcessId, fade_out_seconds: f32) {
        self.actual_process_switch(id, fade_out_seconds);
    }

    /// `ProcessManager.Update(deltaTime)`: the current process's frame, then the fade
    /// and the switch it drives.
    pub fn update<C>(&mut self, dt: f32, ctx: &mut C)
    where
        P: MainLoopProcess<C>,
        C: ProcessFactory<P>,
    {
        if let Some((id, fade)) = self.queue.pop_front() {
            self.actual_process_switch(id, fade);
        }
        ctx.frame_start(self.current.as_ref());
        let mut requests = ProcessRequests::new(
            self.upcoming,
            self.after_mod_finalization.is_some(),
            self.fade_to_black,
        );
        requests.fade_delay_in_progress = self.black_delay > 0.0;
        if let Some(current) = &mut self.current {
            self.shaders_time += dt * current.time_speed_fac();
            current.raw_update(dt, &mut self.stacker, ctx, &mut requests);
        }
        if let Some(fade) = requests.set_fade {
            self.fade_to_black = fade;
        }
        for (id, fade) in requests.switches {
            self.actual_process_switch(id, fade);
        }
        ctx.update_side_processes(self.current.as_mut(), dt);
        if self.after_mod_finalization.is_some() && !self.mod_finalization_done {
            self.mod_finalization_done = true;
        }
        let slideshow = self
            .current
            .as_ref()
            .is_some_and(|p| matches!(p.id(), ProcessId::SlideShow | ProcessId::Dream));
        if slideshow && self.upcoming.is_none() && self.after_mod_finalization.is_none() {
            self.update_fade();
        } else if self.pause_fade_update {
            self.pause_fade_update = false;
        } else if let Some(upcoming) = self.upcoming {
            self.fade_to_black += dt / self.black_fade_time;
            self.update_fade();
            if self.fade_to_black > 1.0 {
                self.fade_to_black = 1.0;
                self.pre_switch(upcoming, ctx);
                self.mod_finalization_done = false;
                self.after_mod_finalization = Some(upcoming);
                self.upcoming = None;
            }
        } else if let Some(id) = self.after_mod_finalization {
            self.update_fade();
            if self.mod_finalization_done {
                self.post_switch(id, ctx);
            }
        } else if self.fade_to_black > 0.0 {
            if self.black_delay > 0.0 {
                self.black_delay -= dt;
            } else {
                self.fade_to_black -= dt / self.black_fade_time;
            }
            self.update_fade();
            if self.fade_to_black < 0.0 {
                self.fade_to_black = 0.0;
                self.fade_sprite = None;
                self.loading_label = None;
            }
        }
    }

    /// `ActualProcessSwitch`: ignored while a switch is under way.
    fn actual_process_switch(&mut self, id: ProcessId, fade_out_seconds: f32) {
        if self.upcoming.is_some() || self.after_mod_finalization.is_some() {
            return;
        }
        self.black_fade_time = fade_out_seconds;
        self.upcoming = Some(id);
        self.init_fade_sprite();
    }

    /// `InitFadeSprite`: a fresh transparent fade sprite, and the "Loading..." label
    /// when the next process is the game or a slideshow.
    fn init_fade_sprite(&mut self) {
        self.fade_sprite = Some(FadeSprite {
            x: self.screen_size[0] / 2.0,
            y: self.screen_size[1] / 2.0,
            scale_x: 1.0,
            scale_y: 1.0,
            alpha: 0.0,
        });
        self.loading_label =
            matches!(self.upcoming, Some(ProcessId::Game | ProcessId::SlideShow)).then_some(1.0);
    }

    /// `UpdateFade`.
    fn update_fade(&mut self) {
        if self.fade_sprite.is_none() {
            self.init_fade_sprite();
        }
        let f = self.fade_to_black;
        let [w, h] = self.screen_size;
        if let Some(sprite) = &mut self.fade_sprite {
            sprite.scale_x = (w * 1.5_f32.lerp_to(1.0, f) + 2.0) / 16.0;
            sprite.scale_y = (h * 2.5_f32.lerp_to(1.5, f) + 2.0) / 16.0;
            sprite.alpha = f.inverse_lerp(0.0, 0.9);
        }
        if let Some(alpha) = &mut self.loading_label {
            *alpha = f.inverse_lerp(0.5, 0.9);
        }
    }

    /// `PreSwitchMainProcess`: ends the current process.
    fn pre_switch<C>(&mut self, next: ProcessId, ctx: &mut C)
    where
        P: MainLoopProcess<C>,
        C: ProcessFactory<P>,
    {
        self.shaders_time = 0.0;
        ctx.pre_switch(next);
        if let Some(mut current) = self.current.take() {
            current.shut_down(ctx);
            self.old = Some(current);
        }
    }

    /// `PostSwitchMainProcess`: starts process `id` and the fade from black.
    fn post_switch<C>(&mut self, id: ProcessId, ctx: &mut C)
    where
        P: MainLoopProcess<C>,
        C: ProcessFactory<P>,
    {
        let mut next = ctx.create_process(id, self.old.as_ref());
        if let Some(old) = &mut self.old {
            old.communicate_with_upcoming(&mut next, ctx);
        }
        self.black_fade_time = next.fade_in_time();
        self.black_delay = next.initial_black_seconds();
        if next.removes_loading_label() {
            self.loading_label = None;
        }
        ctx.post_switch(&next);
        self.current = Some(next);
        self.stacker = TimeStacker::default();
        self.pause_fade_update = true;
        self.mod_finalization_done = true;
        self.after_mod_finalization = None;
    }

    /// The fade sprite and "Loading..." label over everything (`Futile.stage`'s last
    /// children), when shown.
    #[must_use]
    pub fn fade_overlay(&self, atlases: &AtlasManager, translator: &Translator) -> Container {
        let mut stage = Container::default();
        if let Some(fade) = self.fade_sprite {
            let mut sprite = Sprite::new(AtlasManager::WHITE);
            sprite.color = StageColor::BLACK;
            sprite.shader = StageShader::EdgeFade;
            sprite.transform = NodeTransform {
                x: fade.x,
                y: fade.y,
                scale_x: fade.scale_x,
                scale_y: fade.scale_y,
                alpha: fade.alpha,
                ..NodeTransform::default()
            };
            stage.add(Node::Sprite(sprite));
        }
        if let (Some(alpha), Some(font)) = (self.loading_label, atlases.font(Font::FONT)) {
            let mut label = Label::new(font.clone(), translator.translate("Loading..."));
            label.transform = NodeTransform {
                alpha,
                ..NodeTransform::at(100.2, 50.2)
            };
            stage.add(Node::Label(label));
        }
        stage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::initialization::Initialization;

    /// Initialization, then plain processes that only count their ticks.
    enum Proc {
        Init(Initialization),
        Plain(ProcessId, u32),
        /// A new story game: 3 s of black, then a 2 s fade-in; counts the ticks that
        /// saw `FadeDelayInProgress`.
        NewGame(u32),
    }

    #[derive(Default)]
    struct Ctx {
        created: Vec<(ProcessId, Option<ProcessId>)>,
        shut_down: Vec<ProcessId>,
        new_game: bool,
    }

    impl ProcessFactory<Proc> for Ctx {
        fn create_process(&mut self, id: ProcessId, old: Option<&Proc>) -> Proc {
            let old = old.map(MainLoopProcess::<Ctx>::id);
            self.created.push((id, old));
            match id {
                ProcessId::Initialization => Proc::Init(Initialization::new(ProcessId::Game)),
                ProcessId::Game if self.new_game => Proc::NewGame(0),
                _ => Proc::Plain(id, 0),
            }
        }
    }

    impl MainLoopProcess<Ctx> for Proc {
        fn id(&self) -> ProcessId {
            match self {
                Self::Init(_) => ProcessId::Initialization,
                Self::Plain(id, _) => *id,
                Self::NewGame(_) => ProcessId::Game,
            }
        }

        fn update(&mut self, ctx: &mut Ctx, requests: &mut ProcessRequests) {
            match self {
                Self::Init(p) => p.update(ctx, requests),
                Self::Plain(_, ticks) => *ticks += 1,
                Self::NewGame(delayed) => *delayed += u32::from(requests.fade_delay_in_progress()),
            }
        }

        fn fade_in_time(&self) -> f32 {
            match self {
                Self::NewGame(_) => 2.0,
                _ => ProcessRequests::FADE_OUT_SECONDS,
            }
        }

        fn initial_black_seconds(&self) -> f32 {
            match self {
                Self::NewGame(_) => 3.0,
                _ => 0.0,
            }
        }

        fn graf_update(&mut self, _time_stacker: f32, _ctx: &mut Ctx) {}

        fn shut_down(&mut self, ctx: &mut Ctx) {
            ctx.shut_down.push(MainLoopProcess::<Ctx>::id(self));
        }
    }

    const DT: f32 = 0.025;

    fn id(m: &ProcessManager<Proc>) -> Option<ProcessId> {
        m.current().map(MainLoopProcess::<Ctx>::id)
    }

    #[test]
    fn time_stacker_runs_ticks_above_one_and_drops_after_three() {
        let mut s = TimeStacker::default();
        // 0.025 s at 40 fps is exactly 1.0: not above 1, so no tick yet.
        assert_eq!(s.advance(DT, 40), 0);
        assert_eq!(s.advance(DT, 40), 1);
        assert_eq!(s.0, 1.0);
        // A long frame: three ticks, then the rest is dropped.
        assert_eq!(s.advance(1.0, 40), 3);
        assert_eq!(s.0, 0.0);
    }

    /// Frame by frame from boot at 40 frames per second: Initialization requests the
    /// game on its first tick (frame 2), the fade-out completes at once (fadeToBlack
    /// is still 1), the switch takes two frames, the fade-in waits a frame
    /// (`pauseFadeUpdate`) and then falls by dt / 0.45 per frame.
    #[test]
    fn boot_fade_frame_by_frame() {
        let mut ctx = Ctx::default();
        let mut m = ProcessManager::<Proc>::new([1366.0, 768.0], &mut ctx);
        assert_eq!(ctx.created, [(ProcessId::Initialization, None)]);
        assert_eq!(m.fade_to_black(), 1.0);
        // InitFadeSprite leaves it transparent until the first UpdateFade.
        assert_eq!(m.fade_sprite().unwrap().alpha, 0.0);

        m.update(DT, &mut ctx); // frame 1: no tick yet; the paused fade resumes.
        assert_eq!(m.fade_sprite().unwrap().alpha, 0.0);
        assert_eq!(m.upcoming(), None);

        m.update(DT, &mut ctx); // frame 2: the request, fade 1 + dt/0.45 > 1.
        assert_eq!(id(&m), None, "pre-switch ended Initialization");
        assert_eq!(ctx.shut_down, [ProcessId::Initialization]);
        let s = m.fade_sprite().unwrap();
        assert_eq!((s.alpha, s.scale_x, s.scale_y), (1.0, 85.5, 72.125));
        assert_eq!(m.loading_label_alpha(), Some(1.0));

        m.update(DT, &mut ctx); // frame 3: post-switch.
        assert_eq!(id(&m), Some(ProcessId::Game));
        assert_eq!(
            ctx.created[1],
            (ProcessId::Game, Some(ProcessId::Initialization))
        );
        assert_eq!(m.fade_to_black(), 1.0);

        m.update(DT, &mut ctx); // frame 4: pauseFadeUpdate.
        assert_eq!(m.fade_to_black(), 1.0);

        let mut want = 1.0_f32;
        let mut frames = 0;
        while m.fade_sprite().is_some() {
            m.update(DT, &mut ctx);
            want -= DT / 0.45;
            frames += 1;
            if want >= 0.0 {
                assert_eq!(m.fade_to_black(), want);
                let s = m.fade_sprite().unwrap();
                assert_eq!(s.alpha, (want / 0.9).clamp(0.0, 1.0));
                assert_eq!(s.scale_x, (1366.0 * (1.5 - 0.5 * want) + 2.0) / 16.0);
                assert_eq!(s.scale_y, (768.0 * (2.5 - want) + 2.0) / 16.0);
                assert_eq!(
                    m.loading_label_alpha(),
                    Some(((want - 0.5) / (0.9 - 0.5)).clamp(0.0, 1.0))
                );
            }
        }
        // In f32, dt / 0.45 rounds up, so the 18th step already goes below 0 and ends
        // the fade (0.45 s exactly would be 18 steps to 0).
        assert_eq!(frames, 18);
        assert_eq!(m.fade_to_black(), 0.0);
        assert_eq!(m.loading_label_alpha(), None);
        // The game ticked from frame 5 on (frame 4 only filled the stacker).
        assert!(matches!(m.current(), Some(Proc::Plain(_, 18))));
    }

    #[test]
    fn a_second_request_during_a_switch_is_ignored() {
        let mut ctx = Ctx::default();
        let mut m = ProcessManager::<Proc>::new([1366.0, 768.0], &mut ctx);
        for _ in 0..40 {
            m.update(DT, &mut ctx);
        }
        assert_eq!(m.fade_to_black(), 0.0);
        m.request_switch(ProcessId::MainMenu, 0.45);
        m.request_switch(ProcessId::OptionsMenu, 0.45);
        assert_eq!(m.upcoming(), Some(ProcessId::MainMenu));
        // No loading label for a menu.
        assert_eq!(m.loading_label_alpha(), None);
        // Fade out: `fadeToBlack += dt / 0.45` per frame until above 1.
        let mut fade = 0.0_f32;
        let mut frames = 0;
        while fade <= 1.0 {
            fade += DT / 0.45;
            frames += 1;
        }
        // f32 rounding takes it above 1 on the 18th frame.
        assert_eq!(frames, 18);
        for _ in 0..frames - 1 {
            m.update(DT, &mut ctx);
        }
        assert_eq!(id(&m), Some(ProcessId::Game));
        m.update(DT, &mut ctx);
        assert_eq!(id(&m), None);
        m.update(DT, &mut ctx);
        assert_eq!(id(&m), Some(ProcessId::MainMenu));
    }

    /// `ProcessManager.Update`'s fade-in after a switch (108396): while `blackDelay > 0`
    /// it counts down by dt with `fadeToBlack` held at 1 (`FadeDelayInProgress`), then
    /// `fadeToBlack` falls by dt / `FadeInTime`.
    #[test]
    fn initial_black_then_fade_in() {
        let mut ctx = Ctx {
            new_game: true,
            ..Ctx::default()
        };
        let mut m = ProcessManager::<Proc>::new([1366.0, 768.0], &mut ctx);
        for _ in 0..4 {
            m.update(DT, &mut ctx); // boot, switch, then `pauseFadeUpdate`
        }
        assert_eq!(id(&m), Some(ProcessId::Game));
        let mut delay = 3.0_f32;
        let mut delay_frames = 0;
        while delay > 0.0 {
            assert!(m.fade_delay_in_progress());
            m.update(DT, &mut ctx);
            delay -= DT;
            delay_frames += 1;
            assert_eq!(m.fade_to_black(), 1.0);
        }
        assert!(!m.fade_delay_in_progress());
        assert_eq!(delay_frames, 120);
        let mut fade = 1.0_f32;
        let mut fade_frames = 0;
        while m.fade_to_black() > 0.0 {
            m.update(DT, &mut ctx);
            fade -= DT / 2.0;
            fade_frames += 1;
            assert_eq!(m.fade_to_black(), fade.max(0.0));
        }
        assert!((80..=81).contains(&fade_frames), "{fade_frames}");
        // The game ticked under the delay from its first tick (frame 5) to the last
        // frame of the delay.
        assert!(matches!(m.current(), Some(Proc::NewGame(119 | 120))));
    }
}
