//! `SlideShow`: a timed playlist of menu scenes faded through the manager's fade, kept
//! in time with its song (the intro before a new Survivor or Monk game). Scenes are
//! shown flat (D13). Behaviour: `rainworld-re/notes/slideshow.md`.

use rw_sim::FloatExt;

use crate::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use crate::process::{MainLoopProcess, ProcessId, ProcessRequests, TimeStacker};
use crate::shared::{MenuContext, MenuShared, Slugcat};

/// `SlideShow.SlideShowID` (the base game's intros; the outros need the endings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideShowId {
    WhiteIntro,
    YellowIntro,
}

/// `SlideShow.Scene`: a playlist entry, in seconds of slideshow time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlideScene {
    pub scene: SceneId,
    pub start_at: f32,
    pub fade_in_done_at: f32,
    pub fade_out_start_at: f32,
}

impl SlideScene {
    fn new(scene: SceneId, start_at: f32, fade_in_done_at: f32, fade_out_start_at: f32) -> Self {
        Self {
            scene,
            start_at,
            fade_in_done_at,
            fade_out_start_at,
        }
    }

    /// `ConvertTime(minutes, seconds, pps)`: hundredths after the seconds.
    #[must_use]
    pub fn convert_time(minutes: i32, seconds: i32, pps: i32) -> f32 {
        minutes as f32 * 60.0 + seconds as f32 + pps as f32 / 100.0
    }

    fn at(
        scene: SceneId,
        start: (i32, i32, i32),
        fade_in: (i32, i32, i32),
        fade_out: (i32, i32, i32),
    ) -> Self {
        Self::new(
            scene,
            Self::convert_time(start.0, start.1, start.2),
            Self::convert_time(fade_in.0, fade_in.1, fade_in.2),
            Self::convert_time(fade_out.0, fade_out.1, fade_out.2),
        )
    }
}

impl SlideShowId {
    /// The intro `SlugcatSelectMenu.StartGame` plays before a new game of `slugcat`, if
    /// any (More Slugcats adds the Saint's, the Watcher its dream).
    #[must_use]
    pub fn intro_for(slugcat: Slugcat) -> Option<Self> {
        match slugcat {
            Slugcat::WHITE => Some(Self::WhiteIntro),
            Slugcat::YELLOW => Some(Self::YellowIntro),
            _ => None,
        }
    }

    /// The song the slideshow waits for and follows: name, priority, fade-in ticks.
    #[must_use]
    pub fn song(self) -> (&'static str, f32, f32) {
        ("RW_Intro_Theme", 1.5, 40.0)
    }

    /// `processAfterSlideShow`.
    #[must_use]
    pub fn process_after(self) -> ProcessId {
        ProcessId::Game
    }

    /// The constructor's playlist, every entry after the first moved 0.6 s later.
    #[must_use]
    pub fn playlist(self) -> Vec<SlideScene> {
        use SceneId as S;
        let mut list = vec![
            SlideScene::new(S::Empty, 0.0, 0.0, 0.0),
            SlideScene::at(S::Intro1Tree, (0, 0, 20), (0, 3, 26), (0, 8, 6)),
            SlideScene::new(S::Empty, SlideScene::convert_time(0, 9, 6), 0.0, 0.0),
            SlideScene::at(S::Intro2Branch, (0, 9, 19), (0, 10, 19), (0, 16, 2)),
            SlideScene::at(S::Intro3InTree, (0, 17, 21), (0, 18, 10), (0, 24, 3)),
            SlideScene::at(S::Intro4Walking, (0, 24, 26), (0, 25, 19), (0, 32, 2)),
            SlideScene::new(S::Empty, SlideScene::convert_time(0, 34, 6), 0.0, 0.0),
            SlideScene::at(S::Intro5Hunting, (0, 35, 50), (0, 36, 54), (0, 42, 15)),
            SlideScene::at(S::Intro67RainDrop, (0, 43, 0), (0, 44, 0), (0, 49, 29)),
            SlideScene::at(S::Intro8Climbing, (0, 50, 19), (0, 51, 9), (0, 55, 21)),
            SlideScene::new(S::Empty, SlideScene::convert_time(0, 56, 24), 0.0, 0.0),
            SlideScene::at(S::Intro9RainyClimb, (0, 57, 2), (0, 57, 80), (1, 1, 1)),
            SlideScene::at(S::Intro10Fall, (1, 1, 1), (1, 1, 1), (1, 4, 0)),
            SlideScene::at(S::Intro105Separation, (1, 4, 29), (1, 5, 18), (1, 11, 10)),
        ];
        match self {
            Self::WhiteIntro => list.extend([
                SlideScene::at(S::Intro11Drowning, (1, 11, 25), (1, 12, 10), (1, 17, 28)),
                SlideScene::at(S::Intro12Waking, (1, 19, 2), (1, 20, 6), (1, 21, 29)),
                SlideScene::at(S::Intro13Alone, (1, 22, 22), (1, 23, 15), (1, 26, 24)),
            ]),
            Self::YellowIntro => list.extend([
                SlideScene::at(S::YellowIntroA, (1, 11, 25), (1, 14, 0), (1, 21, 80)),
                SlideScene::at(S::YellowIntroB, (1, 22, 60), (1, 23, 0), (1, 26, 80)),
            ]),
        }
        list.push(SlideScene::at(
            S::Intro14Title,
            (1, 27, 24),
            (1, 31, 34),
            (1, 33, 60),
        ));
        for s in list.iter_mut().skip(1) {
            s.start_at += 0.6;
            s.fade_in_done_at += 0.6;
            s.fade_out_start_at += 0.6;
        }
        list
    }
}

/// The `SlideShow` subclass's state.
struct SlideShowScreen {
    id: SlideShowId,
    play_list: Vec<SlideScene>,
    page: ObjectId,
    /// Index into the playlist (`current`, -1 before the first scene).
    current: Option<usize>,
    /// `preloadedScenes[current]`.
    scene: Option<ObjectId>,
    time: f32,
    in_scene_time: f32,
    wait_for_music: Option<&'static str>,
    stall: bool,
    exited_slideshow: bool,
    /// Declared and read but never written by the game: the pause button skips while
    /// held, not only when newly pressed.
    last_pause_button: bool,
    skip: bool,
    playback_speed_mult: f32,
}

/// The slideshow process.
pub struct SlideShow {
    menu: Menu,
    screen: SlideShowScreen,
}

impl SlideShow {
    /// `new SlideShow(manager, id)` for an intro: the song requested (when there is a
    /// music player), the playlist, then the first scene.
    #[must_use]
    pub fn new(shared: &mut MenuShared, id: SlideShowId) -> Self {
        let mut menu = Menu::new(ProcessId::SlideShow, shared);
        let page = menu.add_page(shared, "main", 0);
        let mut wait_for_music = None;
        if let Some(music) = &mut shared.music {
            let (name, priority, fade_in) = id.song();
            wait_for_music = Some(name);
            music.menu_requests_song(name, priority, fade_in);
        }
        let mut screen = SlideShowScreen {
            id,
            play_list: id.playlist(),
            page,
            current: None,
            scene: None,
            time: 0.0,
            in_scene_time: 0.0,
            stall: wait_for_music.is_some(),
            wait_for_music,
            exited_slideshow: false,
            last_pause_button: false,
            skip: false,
            playback_speed_mult: 0.0,
        };
        screen.next_scene(&mut menu, shared);
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
    pub fn id(&self) -> SlideShowId {
        self.screen.id
    }

    #[must_use]
    pub fn play_list(&self) -> &[SlideScene] {
        &self.screen.play_list
    }

    /// The playlist index shown.
    #[must_use]
    pub fn current(&self) -> Option<usize> {
        self.screen.current
    }

    /// The scene shown.
    #[must_use]
    pub fn current_scene(&self) -> Option<SceneId> {
        self.screen
            .current
            .and_then(|i| self.screen.play_list.get(i))
            .map(|s| s.scene)
    }

    /// Seconds of slideshow time.
    #[must_use]
    pub fn time(&self) -> f32 {
        self.screen.time
    }

    #[must_use]
    pub fn in_scene_time(&self) -> f32 {
        self.screen.in_scene_time
    }

    /// Waiting for the song to become the music player's current one.
    #[must_use]
    pub fn stall(&self) -> bool {
        self.screen.stall
    }

    #[must_use]
    pub fn playback_speed_mult(&self) -> f32 {
        self.screen.playback_speed_mult
    }

    /// Skipped with the pause button.
    #[must_use]
    pub fn skipped(&self) -> bool {
        self.screen.skip
    }

    #[must_use]
    pub fn exited(&self) -> bool {
        self.screen.exited_slideshow
    }

    /// The current scene's flat illustrations' alphas.
    #[must_use]
    pub fn illustration_alphas(&self) -> Vec<f32> {
        let Some(scene) = self.screen.scene else {
            return Vec::new();
        };
        let Kind::Scene(s) = &self.menu.object(scene).kind else {
            return Vec::new();
        };
        s.flat_illustrations
            .iter()
            .filter_map(|&id| match &self.menu.object(id).kind {
                Kind::Illustration(i) => Some(i.alpha),
                _ => None,
            })
            .collect()
    }
}

impl SlideShowScreen {
    /// `SlideShow.Scene` shown, and the end of its time: the next scene's start, or a
    /// second after the last one starts fading out.
    fn scene_end(&self, current: usize) -> f32 {
        match self.play_list.get(current + 1) {
            Some(next) => next.start_at,
            None => self.play_list[current].fade_out_start_at + 1.0,
        }
    }

    /// `NextScene`: the shown scene is removed and its images freed, then the next one
    /// is built and shown.
    fn next_scene(&mut self, menu: &mut Menu, shared: &mut MenuShared) {
        self.in_scene_time = 0.0;
        if let Some(old) = self.scene.take() {
            menu.remove_scene(old, shared);
        }
        let next = self.current.map_or(0, |c| c + 1);
        self.current = Some(next);
        let Some(entry) = self.play_list.get(next) else {
            return;
        };
        let scene = menu.add_slide_show_scene(shared, self.page, entry.scene);
        // The constructor applies the scene's alphas at display time 0.
        Self::apply_scene_alphas(menu, scene);
        menu.add_sub(self.page, scene);
        menu.scene = Some(scene);
        self.scene = Some(scene);
    }

    /// `SlideShowMenuScene.ApplySceneSpecificAlphas` in flat mode: the second layer of
    /// the rain drop and the Monk's first scene fades in, the title fades out.
    fn apply_scene_alphas(menu: &mut Menu, scene: ObjectId) {
        let Kind::Scene(s) = &menu.object(scene).kind else {
            return;
        };
        let t = s.display_time;
        let last = s.flat_illustrations.len().saturating_sub(1);
        let (layer, alpha) = match s.id {
            SceneId::Intro67RainDrop => (1, t.inverse_lerp(0.52, 0.58)),
            SceneId::YellowIntroA => (1, t.inverse_lerp(0.51, 0.55)),
            SceneId::Intro14Title => (last, t.inverse_lerp(1.0, 0.7)),
            _ => return,
        };
        let Some(&id) = s.flat_illustrations.get(layer) else {
            return;
        };
        if let Some(i) = menu.illustration_mut(id) {
            i.set_alpha = Some(alpha);
        }
    }

    /// `SlideShow.RawUpdate` after the base's ticks and drawing: the pause button skips;
    /// the clock waits for the song, then follows its source; the manager's fade and
    /// the scene change come from the clock.
    fn after_frame(
        &mut self,
        menu: &mut Menu,
        dt: f32,
        ctx: &mut dyn MenuContext,
        requests: &mut ProcessRequests,
    ) {
        let pause = ctx.menu().input.pause_action();
        if pause && !self.last_pause_button {
            self.skip = true;
            if !self.exited_slideshow {
                requests.switch_to(self.id.process_after());
                if let Some(music) = ctx.menu().music.as_mut().filter(|m| m.song.is_some()) {
                    music.fade_out_all_songs(40.0);
                }
                self.exited_slideshow = true;
            }
            return;
        }
        let song = ctx.menu().music.as_ref().and_then(|m| m.song.as_ref());
        if self.stall
            && let (Some(wait), Some(song)) = (self.wait_for_music, song)
            && song.name == wait
        {
            self.stall = false;
        }
        if !self.stall {
            self.advance_time(dt, ctx);
        }
        let Some(current) = self.current else { return };
        let entry = self.play_list[current];
        let end = self.scene_end(current);
        self.in_scene_time = self.time.inverse_lerp(entry.start_at, end);
        if !self.skip {
            requests.set_fade_to_black(
                self.time
                    .inverse_lerp(entry.fade_in_done_at, entry.start_at)
                    .max(self.time.inverse_lerp(entry.fade_out_start_at, end)),
            );
        }
        if self.time < end {
            return;
        }
        if current + 1 < self.play_list.len() {
            self.next_scene(menu, ctx.menu());
        } else if !self.exited_slideshow {
            requests.switch_to(self.id.process_after());
            self.exited_slideshow = true;
        }
    }

    /// The clock: while the song's first source plays inside its clip, `time` runs at
    /// `playbackSpeedMult` (1 to 2), which eases toward catching up with the source;
    /// otherwise it runs with real time.
    fn advance_time(&mut self, dt: f32, ctx: &mut dyn MenuContext) {
        let track = ctx
            .menu()
            .music
            .as_ref()
            .and_then(|m| m.song.as_ref())
            .filter(|s| s.started_playing)
            .and_then(|s| s.sub_tracks.first())
            .map(rw_audio::music::SubTrack::id);
        let source = track
            .and_then(|t| ctx.music_playback(t))
            .filter(|p| p.is_playing && p.clip_length.is_some_and(|len| p.time < len));
        let Some(source) = source else {
            self.time += dt;
            return;
        };
        let mut goal = 1.0;
        self.playback_speed_mult = self.playback_speed_mult.clamp(1.0, 2.0);
        self.time += dt * self.playback_speed_mult;
        if self.time < source.time {
            goal = ((source.time - self.time) * 0.5 + 1.0).clamp(1.0, 2.0);
        }
        let rate = if self.playback_speed_mult < goal {
            0.04
        } else {
            0.08
        };
        self.playback_speed_mult = self.playback_speed_mult.lerp(goal, rate);
    }
}

impl MenuScreen for SlideShowScreen {
    fn signal(&mut self, _menu: &mut Menu, _cx: &mut MenuCx<'_>, _sender: ObjectId, _m: &str) {}

    /// `ForceNoMouseMode`.
    fn shows_cursor(&self) -> bool {
        false
    }

    /// `SlideShowMenuScene.Update` after its sub-objects: the display time is the
    /// slideshow's time in the scene, then the scene's alphas.
    fn scene_update(&mut self, menu: &mut Menu, scene: ObjectId) {
        if let Some(s) = menu.scene_mut(scene) {
            s.display_time = self.in_scene_time;
        }
        Self::apply_scene_alphas(menu, scene);
    }
}

impl<C: MenuContext> MainLoopProcess<C> for SlideShow {
    fn id(&self) -> ProcessId {
        ProcessId::SlideShow
    }

    fn removes_loading_label(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        let mut cx = MenuCx {
            shared: ctx.menu(),
            requests,
        };
        self.menu.update(&mut self.screen, &mut cx);
    }

    fn graf_update(&mut self, time_stacker: f32, _ctx: &mut C) {
        self.menu.graf_update(&self.screen, time_stacker);
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
        self.screen.after_frame(&mut self.menu, dt, ctx, requests);
    }

    fn shut_down(&mut self, ctx: &mut C) {
        self.menu.shut_down(ctx.menu());
    }
}
