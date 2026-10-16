//! The intro roll driven through the process manager with the installed game's
//! illustrations and `sounds.txt` (read-only), the music player running on silent timed
//! sources as the app runs it. Set `RW_DIR` to override the default install; tests skip
//! when it is absent.
//!
//! Expected numbers come from the decompiled game (`Assembly-CSharp`, 1.11.8):
//! `IntroRoll` (536124: constructor, `Update` 536230, `RawUpdate` 536257, `GoToMenu`
//! 536345, `CheckTimeStamp` 536370), `RainEffect` (536379), `Custom.SCurve` (468521),
//! `MusicPlayer.UpdateMusicContext` (414132) and `ProcessManager.Update` (108108).

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::unity::AssetBundle;
use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicContext, SongBank, SongKind, TimedSources};
use rw_menu::{
    Initialization, IntroRoll, KeyboardKey, MainLoopProcess, MenuContext, MenuShared,
    MenuSoundEvent, MenuSoundId, ProcessFactory, ProcessId, ProcessManager, ProcessRequests,
    TimeStacker,
};
use rw_random::UnityRandom;
use rw_sim::Vec2;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const DT: f32 = 0.025;
const SEED: i32 = 1;

enum Proc {
    Init(Initialization),
    Intro(Box<IntroRoll>),
    Other(ProcessId),
}

struct Ctx {
    shared: MenuShared,
    sources: TimedSources,
    music_stacker: TimeStacker,
    /// Every one-shot the menu microphone started, by frame.
    sounds: Vec<(u64, MenuSoundId, f32)>,
    frame: u64,
}

impl MenuContext for Ctx {
    fn menu(&mut self) -> &mut MenuShared {
        &mut self.shared
    }
}

impl ProcessFactory<Proc> for Ctx {
    fn create_process(&mut self, id: ProcessId, _old: Option<&Proc>) -> Proc {
        match id {
            ProcessId::Initialization => Proc::Init(Initialization::new(ProcessId::IntroRoll)),
            ProcessId::IntroRoll => Proc::Intro(Box::new(IntroRoll::new(&mut self.shared))),
            other => Proc::Other(other),
        }
    }

    fn update_side_processes(&mut self, current: Option<&mut Proc>, dt: f32) {
        if let Some(player) = &mut self.shared.music {
            for _ in 0..self.music_stacker.advance(dt, 40) {
                player.update(&mut self.sources, &mut self.shared.random);
                self.sources.advance();
            }
        }
        self.shared
            .mic
            .graf_update(matches!(current, Some(Proc::Intro(_))));
        let frame = self.frame;
        for e in self.shared.mic.take_events() {
            if let MenuSoundEvent::Play(s) = e {
                self.sounds.push((frame, s.id, s.call_volume));
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
            Self::Intro(_) => ProcessId::IntroRoll,
            Self::Other(id) => *id,
        }
    }

    fn update(&mut self, ctx: &mut Ctx, requests: &mut ProcessRequests) {
        match self {
            Self::Init(p) => p.update(ctx, requests),
            Self::Intro(p) => p.update(ctx, requests),
            Self::Other(_) => {}
        }
    }

    fn graf_update(&mut self, t: f32, ctx: &mut Ctx) {
        if let Self::Intro(p) = self {
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
        if let Self::Intro(p) = self {
            p.raw_update(dt, stacker, ctx, requests);
            return;
        }
        for _ in 0..stacker.advance(dt, 40) {
            self.update(ctx, requests);
        }
    }

    fn shut_down(&mut self, ctx: &mut Ctx) {
        if let Self::Intro(p) = self {
            p.shut_down(ctx);
        }
    }
}

/// The decompiled formulas the expectations are computed with.
struct Decompiled;

impl Decompiled {
    /// `Custom.SCurve` (468521).
    fn s_curve(x: f32, k: f32) -> f32 {
        let x = x * 2.0 - 1.0;
        if x < 0.0 {
            let x = (1.0 + x).abs();
            return k * x / (k - x + 1.0) * 0.5;
        }
        let k = -1.0 - k;
        0.5 + k * x / (k - x + 1.0) * 0.5
    }

    /// `Mathf.InverseLerp(a, b, v)`.
    fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
        if a == b {
            0.0
        } else {
            ((v - a) / (b - a)).clamp(0.0, 1.0)
        }
    }

    /// `IntroRoll.RawUpdate`'s illustration alphas at `time` / `delayedTime`.
    fn alphas(time: f32, delayed: f32) -> [f32; 3] {
        [
            Self::inverse_lerp(1.0, 2.0, time).min(Self::inverse_lerp(4.0, 3.0, time)),
            Self::inverse_lerp(4.0, 5.0, time).min(Self::inverse_lerp(7.0, 6.0, time)),
            Self::inverse_lerp(10.0, 11.0, delayed),
        ]
        .map(|a| Self::s_curve(a, 0.65))
    }

    /// `CheckTimeStamp`.
    fn passed(last: f32, current: f32, check: f32) -> bool {
        current >= check && last < check
    }
}

struct Harness {
    manager: ProcessManager<Proc>,
    ctx: Ctx,
}

impl Harness {
    /// Boots: frame 1 makes the music player, frame 2 Initialization asks for the intro
    /// roll and the switch's first half runs (the fade is still 1), frame 3 creates it.
    fn boot() -> Option<Self> {
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
        shared.random = UnityRandom::new(SEED);
        shared.set_sound_table(table);
        let mut ctx = Ctx {
            shared,
            sources: TimedSources::new(bank),
            music_stacker: TimeStacker::default(),
            sounds: Vec::new(),
            frame: 0,
        };
        let manager = ProcessManager::new([1366.0, 768.0], &mut ctx);
        let mut h = Self { manager, ctx };
        for _ in 0..3 {
            h.frame(&[]);
        }
        assert!(h.intro().is_some(), "frame 3 starts the intro roll");
        Some(h)
    }

    fn frame(&mut self, keys: &[KeyboardKey]) {
        for &k in keys {
            self.ctx.shared.input.set_key(k, true);
        }
        self.ctx.frame += 1;
        self.manager.update(DT, &mut self.ctx);
        for &k in keys {
            self.ctx.shared.input.set_key(k, false);
        }
    }

    fn intro(&self) -> Option<&IntroRoll> {
        match self.manager.current() {
            Some(Proc::Intro(p)) => Some(p),
            _ => None,
        }
    }

    fn roll(&self) -> &IntroRoll {
        self.intro().expect("the intro roll runs")
    }

    fn music_started(&self) -> bool {
        let song = self.ctx.shared.music.as_ref().and_then(|m| m.song.as_ref());
        matches!(song.map(|s| s.kind), Some(SongKind::IntroRoll(i)) if i.music_track_started)
    }

    fn rain_fading(&self) -> bool {
        let song = self.ctx.shared.music.as_ref().and_then(|m| m.song.as_ref());
        matches!(song.map(|s| s.kind), Some(SongKind::IntroRoll(i)) if i.fade_out_rain)
    }
}

/// The whole roll untouched, frame by frame at 40 fps (one tick per frame from the
/// intro's second frame): `time` and `delayedTime` gain dt after each frame's ticks
/// (the music bundles are loaded by the music player's second update, frame 3, so
/// `delayedTime` never waits); A fades in over 1-2 s and out over 3-4 s, B over 4-5 and
/// 6-7, C over `delayedTime` 10-11, the label over 14-15, all through `SCurve(_, 0.65)`;
/// `rainFade = SCurve(InverseLerp(4, 10, time), 0.8)`; spikes (0.4, 30) at 1.5 s,
/// (0.6, 40) at 3, (0.3, 1600) at 5, (1, 40) at `delayedTime` 10.5; `Thunder_Close`
/// (vol 0.3) then `Thunder` (0.4) at 10.3; `StartMusic` at 8.5; and once `delayedTime`
/// passes 40, `GoToMenu`: `StartMusic`, `fadeOutRain`, the switch to the main menu.
#[test]
fn intro_roll_timeline() {
    let Some(mut h) = Harness::boot() else { return };
    assert_eq!(h.roll().third_illustration(), "Intro_Roll_C");
    assert_eq!(
        h.roll().illustration_alphas(),
        [1.0; 3],
        "alpha 1 until the first frame"
    );
    let mut time = 0.0_f32;
    let mut frame = 3;
    let mut music_frame = None;
    let mut thunder_frame = None;
    let mut spikes = Vec::new();
    let mut switch_frame = None;
    while switch_frame.is_none() {
        let last = time;
        h.frame(&[]);
        frame += 1;
        time += DT;
        let roll = h.roll();
        assert_eq!(
            (roll.time(), roll.delayed_time()),
            (time, time),
            "frame {frame}"
        );
        assert_eq!(roll.illustration_alphas(), Decompiled::alphas(time, time));
        assert_eq!(
            roll.label_alpha(),
            Decompiled::s_curve(Decompiled::inverse_lerp(14.0, 15.0, time), 0.65)
        );
        let rain = roll.rain_effect();
        assert_eq!(
            rain.rain_fade,
            Decompiled::s_curve(Decompiled::inverse_lerp(4.0, 10.0, time), 0.8)
        );
        for (at, intensity, drop_off) in [
            (1.5, 0.4, 30.0),
            (3.0, 0.6, 40.0),
            (5.0, 0.3, 1600.0),
            (10.5, 1.0, 40.0),
        ] {
            if Decompiled::passed(last, time, at) {
                assert_eq!(rain.li_drop_off, drop_off, "spike at {at}");
                assert_eq!(rain.extra_lightning_chance, 1.0);
                assert!(rain.lightning_intensity >= intensity);
                spikes.push(frame);
            }
        }
        if Decompiled::passed(last, time, 10.3) {
            thunder_frame = Some(frame);
        }
        if h.music_started() && music_frame.is_none() {
            music_frame = Some(frame);
            assert!(Decompiled::passed(last, time, 8.5), "StartMusic at 8.5 s");
        }
        if h.manager.upcoming() == Some(ProcessId::MainMenu) {
            switch_frame = Some(frame);
            assert!(time > 40.0 && last <= 40.0, "the time-out at 40 s");
            assert!(h.roll().continue_to_menu() && h.rain_fading());
        }
        assert!(frame < 4000);
    }
    eprintln!(
        "spikes at frames {spikes:?}, music {music_frame:?}, thunder {thunder_frame:?}, menu {switch_frame:?}"
    );
    assert_eq!(spikes.len(), 4);
    let thunder = thunder_frame.expect("thunder");
    let played: Vec<(MenuSoundId, f32)> = h
        .ctx
        .sounds
        .iter()
        .filter(|(f, _, _)| *f == thunder)
        .map(|(_, id, v)| (*id, *v))
        .collect();
    assert_eq!(
        played,
        [
            (MenuSoundId::ThunderClose, 0.3),
            (MenuSoundId::Thunder, 0.4)
        ]
    );
    assert!(
        h.ctx.sounds.iter().all(|(f, _, _)| *f == thunder),
        "no other sounds"
    );
}

/// `IntroRoll.Update`: a button newly pressed (`flag && !lastAnyButton`) while `time > 1`
/// goes to the menu. The tick runs before `RawUpdate` adds the frame's dt, so it sees
/// the previous frame's `time`. A key already held when `time` passes 1 never counts
/// until released and pressed again.
#[test]
fn any_button_after_one_second_goes_to_the_menu() {
    let Some(mut h) = Harness::boot() else { return };
    // Held from the start through 1.5 s: no new press after 1 s.
    while h.roll().time() < 1.5 {
        h.frame(&[KeyboardKey::Z]);
        assert_eq!(h.manager.upcoming(), None);
    }
    h.frame(&[]);
    assert_eq!(h.manager.upcoming(), None);
    assert!(!h.music_started());
    h.frame(&[KeyboardKey::LeftShift]);
    assert_eq!(h.manager.upcoming(), Some(ProcessId::MainMenu));
    assert!(h.roll().continue_to_menu());
    // GoToMenu starts the title music early and fades the rain.
    assert!(h.music_started() && h.rain_fading());

    // A fresh press at exactly the first tick that sees time > 1.
    let Some(mut h) = Harness::boot() else { return };
    while h.roll().time() <= 1.0 {
        h.frame(&[]);
    }
    h.frame(&[KeyboardKey::Escape]);
    assert_eq!(h.manager.upcoming(), Some(ProcessId::MainMenu));

    // The tick that still sees time <= 1 ignores the press.
    let Some(mut h) = Harness::boot() else { return };
    while h.roll().time() + DT <= 1.0 {
        h.frame(&[]);
    }
    h.frame(&[KeyboardKey::Return]);
    assert_eq!(h.manager.upcoming(), None);
}

/// `RainEffect`'s constructor for seed 1 (nothing draws before the intro roll): per drop
/// `Reset` (x `Lerp(-100, 1466, v)`, y `770 + 100 v`, depth `Lerp(0.5, 7, v^0.5)`) then
/// `pos = (1366 v, 900 v)`, `lastPos = pos + (0, 300) / depth`, `lastLastPos` 300 / depth
/// above that; then `rainDirs[j, 0] = RNV * v * 20`, `rainDirs[j, 1] = RNV * v * 10`. No
/// slugcat illustration draw (no DLC). Its first `Update` draws two per drop (no drop
/// resets yet), four per wind pair, and one for the lightning roll: 1009.
#[test]
fn rain_effect_draws_for_a_fixed_seed() {
    let Some(mut h) = Harness::boot() else { return };
    let mut r = UnityRandom::new(SEED);
    let first = {
        let _x = -100.0 + (1466.0 + 100.0) * r.value();
        let _y = 770.0 + r.value() * 100.0;
        let depth = 0.5 + 6.5 * (f64::from(r.value()).powf(0.5) as f32);
        let pos = Vec2::new(1366.0 * r.value(), 900.0 * r.value());
        (pos, depth)
    };
    let rain = h.roll().rain_effect();
    assert_eq!(rain.drops.len(), 500);
    assert_eq!((rain.drops[0].pos, rain.drops[0].depth), first);
    assert_eq!(
        rain.drops[0].last_pos,
        first.0 + Vec2::new(0.0, 300.0) / first.1
    );
    for _ in 5..2500 {
        r.value();
    }
    let mut dirs = [[Vec2::ZERO; 2]; 2];
    for d in &mut dirs {
        d[0] = Vec2::from_degrees(r.value() * 360.0) * r.value() * 20.0;
        d[1] = Vec2::from_degrees(r.value() * 360.0) * r.value() * 10.0;
    }
    assert_eq!(rain.rain_dirs, dirs);
    assert_eq!(h.ctx.shared.random, r, "2508 draws in the constructor");
    let drop = rain.drops[0];
    // Frame 4 fills the stacker; frame 5 runs the first tick.
    h.frame(&[]);
    assert_eq!(h.ctx.shared.random, r);
    h.frame(&[]);
    let dp = Decompiled::inverse_lerp(0.5, 7.0, drop.depth);
    let dir = Vec2::new(0.0, -300.0) + dirs[0][0].lerp(dirs[1][0], dp);
    let jitter = Vec2::from_degrees(r.value() * 360.0) * r.value() * 14.0;
    assert_eq!(
        h.roll().rain_effect().drops[0].pos,
        drop.pos + (dir + jitter) / drop.depth
    );
    for _ in 2..1009 {
        r.value();
    }
    assert_eq!(h.ctx.shared.random, r, "1009 draws in the first update");
}

/// `UpdateMusicContext` from the music player's creation (frame 1, `Initialization`) to
/// the intro roll: `Other` processes keep the `Menu` context; the intro roll asks for
/// `IntroRollMusic`, whose rain sub-track starts on the player's third update.
#[test]
fn intro_roll_music_context() {
    let Some(h) = Harness::boot() else { return };
    let music = h.ctx.shared.music.as_ref().expect("music on by default");
    assert_eq!(music.music_context, MusicContext::Menu);
    assert!(music.song.as_ref().is_some_and(|s| s.is_intro_roll()));
    assert!(music.asset_bundles_loaded());
}
