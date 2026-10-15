//! The new-game intro slideshows driven through the process manager with the installed
//! game's illustrations, the music player running on silent timed sources as the app
//! runs it headless. Set `RW_DIR` to override the default install; tests skip when it is
//! absent.
//!
//! Expected numbers come from the decompiled game (`Assembly-CSharp`, 1.11.8): the
//! `SlideShow` constructor (551452), `ConvertTime` (552339), `RawUpdate` (552344),
//! `SlideShowMenuScene.ApplySceneSpecificAlphas` (552967) and
//! `RWInput.CheckPauseButton` (54624).

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::unity::AssetBundle;
use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicClock, SongBank, SourcePlayback, TimedSources, TrackId};
use rw_menu::menu::SceneId;
use rw_menu::{
    Initialization, KeyboardKey, MainLoopProcess, MenuContext, MenuShared, ProcessFactory,
    ProcessId, ProcessManager, ProcessRequests, SlideScene, SlideShow, SlideShowId, TimeStacker,
};
use rw_random::UnityRandom;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const DT: f32 = 0.025;

enum Proc {
    Init(Initialization),
    Slides(Box<SlideShow>),
    Other(ProcessId),
}

struct Ctx {
    shared: MenuShared,
    sources: TimedSources,
    music_stacker: TimeStacker,
    /// Replaces the song source's playback the slideshow reads.
    fake_source: Option<SourcePlayback>,
}

impl MenuContext for Ctx {
    fn menu(&mut self) -> &mut MenuShared {
        &mut self.shared
    }

    fn music_playback(&self, track: TrackId) -> Option<SourcePlayback> {
        self.fake_source.or_else(|| self.sources.playback(track))
    }
}

impl ProcessFactory<Proc> for Ctx {
    fn create_process(&mut self, id: ProcessId, _old: Option<&Proc>) -> Proc {
        match id {
            ProcessId::Initialization => Proc::Init(Initialization::new(ProcessId::SlideShow)),
            ProcessId::SlideShow => {
                let show = self.shared.next_slideshow.expect("next slideshow set");
                Proc::Slides(Box::new(SlideShow::new(&mut self.shared, show)))
            }
            other => Proc::Other(other),
        }
    }

    fn update_side_processes(&mut self, _current: Option<&Proc>, dt: f32) {
        if let Some(player) = &mut self.shared.music {
            for _ in 0..self.music_stacker.advance(dt, 40) {
                player.update(&mut self.sources);
                self.sources.advance();
            }
        }
    }

    fn pre_switch(&mut self, next: ProcessId) {
        self.shared.pre_switch(next);
    }

    fn post_switch(&mut self, current: &Proc) {
        self.shared.update_music_context(current.id());
    }

    fn frame_start(&mut self, current: Option<&Proc>) {
        self.shared
            .create_music_player(current.map(MainLoopProcess::id));
    }
}

impl MainLoopProcess<Ctx> for Proc {
    fn id(&self) -> ProcessId {
        match self {
            Self::Init(_) => ProcessId::Initialization,
            Self::Slides(_) => ProcessId::SlideShow,
            Self::Other(id) => *id,
        }
    }

    fn update(&mut self, ctx: &mut Ctx, requests: &mut ProcessRequests) {
        match self {
            Self::Init(p) => p.update(ctx, requests),
            Self::Slides(p) => p.update(ctx, requests),
            Self::Other(_) => {}
        }
    }

    fn graf_update(&mut self, t: f32, ctx: &mut Ctx) {
        if let Self::Slides(p) = self {
            p.graf_update(t, ctx);
        }
    }

    fn raw_update(
        &mut self,
        dt: f32,
        stacker: &mut TimeStacker,
        ctx: &mut Ctx,
        requests: &mut ProcessRequests,
    ) {
        if let Self::Slides(p) = self {
            p.raw_update(dt, stacker, ctx, requests);
            return;
        }
        for _ in 0..stacker.advance(dt, 40) {
            self.update(ctx, requests);
        }
    }

    fn removes_loading_label(&self) -> bool {
        matches!(self, Self::Slides(_))
    }

    fn shut_down(&mut self, ctx: &mut Ctx) {
        if let Self::Slides(p) = self {
            p.shut_down(ctx);
        }
    }
}

/// The decompiled formulas and constants the expectations are computed with.
struct Decompiled;

impl Decompiled {
    /// The playlists after the +0.6 s offset, from the constructor's `ConvertTime`
    /// arguments worked by hand: scene, start, fade-in done, fade-out start.
    const SHARED: [(SceneId, f32, f32, f32); 14] = [
        (SceneId::Empty, 0.0, 0.0, 0.0),
        (SceneId::Intro1Tree, 0.8, 3.86, 8.66),
        (SceneId::Empty, 9.66, 0.6, 0.6),
        (SceneId::Intro2Branch, 9.79, 10.79, 16.62),
        (SceneId::Intro3InTree, 17.81, 18.70, 24.63),
        (SceneId::Intro4Walking, 24.86, 25.79, 32.62),
        (SceneId::Empty, 34.66, 0.6, 0.6),
        (SceneId::Intro5Hunting, 36.10, 37.14, 42.75),
        (SceneId::Intro67RainDrop, 43.60, 44.60, 49.89),
        (SceneId::Intro8Climbing, 50.79, 51.69, 55.81),
        (SceneId::Empty, 56.84, 0.6, 0.6),
        (SceneId::Intro9RainyClimb, 57.62, 58.40, 61.61),
        (SceneId::Intro10Fall, 61.61, 61.61, 64.60),
        (SceneId::Intro105Separation, 64.89, 65.78, 71.70),
    ];
    const WHITE: [(SceneId, f32, f32, f32); 3] = [
        (SceneId::Intro11Drowning, 71.85, 72.70, 77.88),
        (SceneId::Intro12Waking, 79.62, 80.66, 81.89),
        (SceneId::Intro13Alone, 82.82, 83.75, 86.84),
    ];
    const YELLOW: [(SceneId, f32, f32, f32); 2] = [
        (SceneId::YellowIntroA, 71.85, 74.60, 82.40),
        (SceneId::YellowIntroB, 83.20, 83.60, 87.40),
    ];
    const TITLE: (SceneId, f32, f32, f32) = (SceneId::Intro14Title, 87.84, 91.94, 94.20);

    fn playlist(id: SlideShowId) -> Vec<(SceneId, f32, f32, f32)> {
        let mut list = Self::SHARED.to_vec();
        match id {
            SlideShowId::WhiteIntro => list.extend(Self::WHITE),
            SlideShowId::YellowIntro => list.extend(Self::YELLOW),
        }
        list.push(Self::TITLE);
        list
    }

    /// `Mathf.InverseLerp(a, b, v)`.
    fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
        if a == b {
            0.0
        } else {
            ((v - a) / (b - a)).clamp(0.0, 1.0)
        }
    }

    /// The scene's end: the next start, or a second after the last fade-out starts.
    fn end(list: &[SlideScene], i: usize) -> f32 {
        list.get(i + 1)
            .map_or(list[i].fade_out_start_at + 1.0, |n| n.start_at)
    }

    /// `manager.fadeToBlack` the slideshow sets at `time` in scene `i`.
    fn fade(list: &[SlideScene], i: usize, time: f32) -> f32 {
        let s = list[i];
        Self::inverse_lerp(s.fade_in_done_at, s.start_at, time).max(Self::inverse_lerp(
            s.fade_out_start_at,
            Self::end(list, i),
            time,
        ))
    }

    /// One frame of the clock while the source plays: `(time, playbackSpeedMult)` after.
    fn clock(time: f32, mult: f32, source: f32) -> (f32, f32) {
        let mult = mult.clamp(1.0, 2.0);
        let time = time + DT * mult;
        let goal = if time < source {
            ((source - time) * 0.5 + 1.0).clamp(1.0, 2.0)
        } else {
            1.0
        };
        let rate = if mult < goal { 0.04 } else { 0.08 };
        (time, mult + (goal - mult) * rate)
    }
}

struct Harness {
    manager: ProcessManager<Proc>,
    ctx: Ctx,
}

impl Harness {
    /// Boots straight into `show` (frame 3 creates it), with music unless `music` is
    /// false.
    fn boot(show: SlideShowId, music: bool) -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let data = root.join("RainWorld_Data");
        if !data.join("resources.assets").is_file() {
            eprintln!("skipping: no game under {}", root.display());
            return None;
        }
        let assets = StreamingAssets::new(data.join("StreamingAssets"));
        let resources = UnityResources::open(&data).expect("resources");
        let bundle = AssetBundle::open(&assets.asset_bundle("loadedsoundeffects")).expect("bundle");
        let table = Arc::new(SoundTable::load(&assets, &bundle).expect("sounds.txt"));
        let bank = SongBank::open(&assets).ok();
        let mut shared = MenuShared::load(assets, &resources, [1366.0, 768.0]).expect("fonts");
        shared.random = UnityRandom::new(1);
        shared.set_sound_table(table);
        shared.next_slideshow = Some(show);
        shared.request_play_music = music;
        let mut ctx = Ctx {
            shared,
            sources: TimedSources::new(bank),
            music_stacker: TimeStacker::default(),
            fake_source: None,
        };
        let manager = ProcessManager::new([1366.0, 768.0], &mut ctx);
        let mut h = Self { manager, ctx };
        for _ in 0..3 {
            h.frame(&[]);
        }
        assert!(h.slides_opt().is_some(), "frame 3 starts the slideshow");
        Some(h)
    }

    fn frame(&mut self, keys: &[KeyboardKey]) {
        for &k in keys {
            self.ctx.shared.input.set_key(k, true);
        }
        self.manager.update(DT, &mut self.ctx);
        for &k in keys {
            self.ctx.shared.input.set_key(k, false);
        }
    }

    fn slides_opt(&self) -> Option<&SlideShow> {
        match self.manager.current() {
            Some(Proc::Slides(p)) => Some(p),
            _ => None,
        }
    }

    fn slides(&self) -> &SlideShow {
        self.slides_opt().expect("the slideshow runs")
    }

    /// The song's first source as the slideshow reads it.
    fn source(&self) -> Option<SourcePlayback> {
        let song = self.ctx.shared.music.as_ref()?.song.as_ref()?;
        let track = song.sub_tracks.first()?.id();
        self.ctx.sources.playback(track)
    }
}

/// The constructor's playlists against the constants worked by hand (to 0.005 s), and
/// `ConvertTime`.
#[test]
fn playlists_match_the_constructor() {
    assert_eq!(SlideScene::convert_time(1, 27, 24), 87.24);
    for id in [SlideShowId::WhiteIntro, SlideShowId::YellowIntro] {
        let got = id.playlist();
        let want = Decompiled::playlist(id);
        assert_eq!(got.len(), want.len(), "{id:?}");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.scene, w.0);
            for (a, b) in [
                (g.start_at, w.1),
                (g.fade_in_done_at, w.2),
                (g.fade_out_start_at, w.3),
            ] {
                assert!((a - b).abs() < 0.005, "{id:?} {:?}: {a} vs {b}", w.0);
            }
        }
        assert_eq!(id.process_after(), ProcessId::Game);
        assert_eq!(id.song(), ("RW_Intro_Theme", 1.5, 40.0));
    }
}

/// The whole Survivor intro on timed sources: the slideshow stalls until
/// `RW_Intro_Theme` is the music player's song; then each frame its clock follows
/// `RawUpdate`'s rule against the source's position (read before the frame), the
/// manager's fade is the scene's `max(InverseLerp(fadeInDone, start), InverseLerp(fadeOut,
/// end))`, scenes change when the clock passes their end (Intro 9 to 10 with the fade
/// at 0 both sides: a hard cut), and after the title it switches to the game.
#[test]
fn white_intro_follows_its_song() {
    let Some(mut h) = Harness::boot(SlideShowId::WhiteIntro, true) else {
        return;
    };
    let list = h.slides().play_list().to_vec();
    let mut stalled = 0;
    let mut frames = 0;
    let mut scenes = vec![h.slides().current_scene()];
    let mut cut = None;
    while h.manager.upcoming().is_none() {
        let s = h.slides();
        let (time, mult, stall) = (s.time(), s.playback_speed_mult(), s.stall());
        let source = h.source().filter(|p| {
            h.ctx
                .shared
                .music
                .as_ref()
                .and_then(|m| m.song.as_ref())
                .is_some_and(|s| s.started_playing)
                && p.is_playing
                && p.clip_length.is_some_and(|l| p.time < l)
        });
        let before_scene = s.current();
        let before_fade = h.manager.fade_to_black();
        h.frame(&[]);
        frames += 1;
        let s = h.slides();
        if s.stall() {
            stalled += 1;
            assert_eq!(s.time(), 0.0);
            continue;
        }
        let want = match source {
            Some(p) if !stall => Decompiled::clock(time, mult, p.time),
            _ => (time + DT, mult),
        };
        assert!(
            (s.time() - want.0).abs() < 1e-4 && (s.playback_speed_mult() - want.1).abs() < 1e-5,
            "frame {frames}: {} {} vs {want:?}",
            s.time(),
            s.playback_speed_mult()
        );
        if let Some(p) = source {
            assert!((s.time() - p.time).abs() < 0.2, "in step with the song");
        }
        if s.current() != before_scene {
            scenes.push(s.current_scene());
            let i = before_scene.expect("a scene");
            assert!(time < Decompiled::end(&list, i) && s.time() >= Decompiled::end(&list, i));
            if list[i].scene == SceneId::Intro9RainyClimb {
                cut = Some((before_fade, h.manager.fade_to_black()));
            }
        }
        // Set before `NextScene`, by the scene the frame started in.
        let i = before_scene.expect("a scene");
        if h.manager.upcoming().is_none() {
            assert_eq!(
                h.manager.fade_to_black(),
                Decompiled::fade(&list, i, s.time()),
                "frame {frames}"
            );
        }
        assert!(frames < 5000, "never ended");
    }
    eprintln!(
        "RW_Intro_Theme clip length: {:?}",
        h.source().and_then(|p| p.clip_length)
    );
    assert!(stalled <= 2, "waited {stalled} frames for the song");
    assert_eq!(cut, Some((0.0, 0.0)), "Intro 9 to 10 is a cut");
    let want: Vec<_> = list.iter().map(|s| Some(s.scene)).collect();
    assert_eq!(scenes, want);
    assert_eq!(h.manager.upcoming(), Some(ProcessId::Game));
    let end = h.slides().time();
    assert!((95.2..95.3).contains(&end), "ended at {end}");
}

/// Without a music player there is no stall and no song to follow: the clock is real
/// time, and the Monk's playlist runs to the game.
#[test]
fn yellow_intro_without_music_runs_on_real_time() {
    let Some(mut h) = Harness::boot(SlideShowId::YellowIntro, false) else {
        return;
    };
    assert!(h.ctx.shared.music.is_none());
    assert!(!h.slides().stall());
    let mut alphas = Vec::new();
    while h.manager.upcoming().is_none() {
        let before = h.slides().time();
        h.frame(&[]);
        let s = h.slides();
        assert_eq!(s.time(), before + DT);
        if s.current_scene() == Some(SceneId::YellowIntroA) {
            alphas.push((s.in_scene_time(), s.illustration_alphas()));
        }
    }
    // `flatIllustrations[1]` of Yellow A fades in over `InverseLerp(0.51, 0.55,
    // displayTime)`. A tick sets `alpha` from the `setAlpha` of the tick before, whose
    // display time was the scene time after the frame before that.
    assert_eq!(alphas.len(), 454, "11.35 s of Yellow A");
    for k in 2..alphas.len() {
        let a = &alphas[k].1;
        assert_eq!(a.len(), 2);
        assert_eq!(a[0], 1.0);
        let want = Decompiled::inverse_lerp(0.51, 0.55, alphas[k - 2].0);
        assert!((a[1] - want).abs() < 1e-6, "frame {k}: {} vs {want}", a[1]);
    }
    assert_eq!(alphas.last().map(|a| a.1[1]), Some(1.0));
    assert_eq!(h.manager.upcoming(), Some(ProcessId::Game));
}

/// The song's source ahead by 3 s: `playbackSpeedMult` eases up towards
/// `(source - time) * 0.5 + 1` (capped at 2) by 0.04 a frame, and back towards 1 by 0.08
/// once caught up.
#[test]
fn clock_catches_up_with_the_song() {
    let Some(mut h) = Harness::boot(SlideShowId::WhiteIntro, true) else {
        return;
    };
    while h.slides().stall() || h.source().is_none_or(|p| !p.is_playing) {
        h.frame(&[]);
    }
    let length = h.source().and_then(|p| p.clip_length);
    let mut source = h.slides().time() + 3.0;
    let mut max = 0.0_f32;
    for frame in 0..400 {
        let s = h.slides();
        let (time, mult) = (s.time(), s.playback_speed_mult());
        h.ctx.fake_source = Some(SourcePlayback {
            is_playing: true,
            time: source,
            clip_length: length,
        });
        h.frame(&[]);
        let s = h.slides();
        let want = Decompiled::clock(time, mult, source);
        assert!(
            (s.time() - want.0).abs() < 1e-4 && (s.playback_speed_mult() - want.1).abs() < 1e-5,
            "frame {frame}"
        );
        max = max.max(s.playback_speed_mult());
        source += DT;
    }
    assert!(max > 1.5, "sped up ({max})");
    let s = h.slides();
    assert!((s.time() - source).abs() < 0.1, "caught up");
}

/// `CheckPauseButton` with `lastPauseButton` never written: any frame Escape
/// is down skips; the switch to the game fades from the current fade (the slideshow
/// stops setting it) after one held frame, and every song fades out over 40 ticks.
#[test]
fn escape_skips_at_once() {
    let Some(mut h) = Harness::boot(SlideShowId::WhiteIntro, true) else {
        return;
    };
    for _ in 0..200 {
        h.frame(&[]);
    }
    let fade = h.manager.fade_to_black();
    h.frame(&[KeyboardKey::Escape]);
    let s = h.slides();
    assert!(s.skipped() && s.exited());
    assert_eq!(h.manager.upcoming(), Some(ProcessId::Game));
    let song = h.ctx.shared.music.as_ref().and_then(|m| m.song.as_ref());
    assert_eq!(song.map(|s| s.fade_out_time), Some(40.0));
    // `pauseFadeUpdate`, set by the switch to the slideshow and left set while the
    // slideshow drove the fade, holds the fade for this frame.
    assert_eq!(h.manager.fade_to_black(), fade);
    // Still held: the skip repeats (level-triggered) without a second request.
    h.frame(&[KeyboardKey::Escape]);
    assert!((h.manager.fade_to_black() - (fade + DT / 0.45)).abs() < 1e-5);
    assert!(h.slides().skipped());
}

/// Escape on the slideshow's first frame, before its song is the current one.
#[test]
fn escape_skips_while_waiting_for_the_song() {
    let Some(mut h) = Harness::boot(SlideShowId::YellowIntro, true) else {
        return;
    };
    h.frame(&[KeyboardKey::Escape]);
    assert!(h.slides().skipped());
    assert_eq!(h.manager.upcoming(), Some(ProcessId::Game));
}
