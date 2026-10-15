//! A reactive input program through the Outskirts tutorial (SU_C04, SU_A41, SU_A43,
//! SU_A22 into the shelter SU_S01, optionally by SU_A42 to catch and eat four bat
//! flies first) for a new Survivor game, pausing where the guide
//! overseer's holograms trigger (`OverseerTutorialBehavior`). Each tick's input is
//! chosen from the game state, not from a tick table, so the route survives changes in
//! when the guide follows the player.

use rw_sim::Animation;
use rw_sim::creature::fly::MovementMode;

use super::Session;
use super::params::{InputParam, NewGameParams, StepParams};
use super::reply::State;

/// The slice of [`State`] the route reads each tick.
#[derive(Debug, Clone)]
pub(super) struct RouteView {
    tick: u64,
    room: String,
    /// `bodyChunks[0]` (mainBodyChunk) and `[1]` (hips), room pixels.
    main: [f32; 2],
    hips: [f32; 2],
    hips_contact_y: i8,
    animation: Animation,
    in_shortcut: bool,
    /// The shortcut tile the body travels, while in one.
    shortcut_tile: Option<[i32; 2]>,
    /// The room the player's vessel waits to enter, if it does.
    lobby_room: Option<String>,
    guide_room: Option<String>,
    /// `SU_C04StartUp` still holds the body.
    held: bool,
    food: i32,
    /// A hand holds something (a bat fly).
    holding: bool,
    /// The bat flies in the player's room that nothing holds and that are not
    /// burrowing.
    flies: Vec<[f32; 2]>,
}

impl RouteView {
    pub(super) fn of(s: &State) -> Self {
        let p = s.player.as_ref();
        Self {
            tick: s.tick,
            room: s.player_room.clone(),
            main: p.map_or([0.0; 2], |p| p.chunks[0].pos),
            hips: p.map_or([0.0; 2], |p| p.chunks[1].pos),
            hips_contact_y: p.map_or(0, |p| p.chunks[1].contact[1]),
            animation: p
                .and_then(|p| Animation::from_value(&p.animation))
                .unwrap_or_default(),
            in_shortcut: p.is_none_or(|p| p.in_shortcut) || s.lobby.is_some(),
            shortcut_tile: p.and_then(|p| p.vessel_tile),
            lobby_room: s.lobby.as_ref().map(|l| l.room.clone()),
            guide_room: s
                .creatures
                .iter()
                .find(|c| c.overseer.as_ref().is_some_and(|o| o.player_guide))
                .map(|c| c.room.clone()),
            held: s
                .start_up
                .as_ref()
                .is_some_and(|u| u.push_right || !u.stood_up),
            food: p.map_or(0, |p| p.food),
            holding: p.is_some_and(|p| p.grasps.iter().any(Option::is_some)),
            flies: s
                .creatures
                .iter()
                .filter_map(|c| c.fly.as_ref())
                .filter(|f| {
                    f.room.as_deref() == Some(s.player_room.as_str())
                        && f.grabbed_by.is_empty()
                        && f.mov_mode.as_deref().and_then(MovementMode::from_value)
                            != Some(MovementMode::Burrow)
                })
                .map(|f| f.pos)
                .collect(),
        }
    }

    fn guide_here(&self) -> bool {
        self.guide_room.as_deref() == Some(self.room.as_str())
    }

    /// `OverseerTutorialBehavior`'s box zone in SU_C04.
    fn hips_in_box_zone(&self) -> bool {
        self.hips[0] > 1680.0 && self.hips[0] < 1836.0
    }

    /// The tutorial's "jumped over the box": hips past it, high and standing.
    fn over_box(&self) -> bool {
        self.hips[0] > 1836.0 && self.hips[1] > 174.0 && self.hips_contact_y < 0
    }

    /// The tutorial's pole climbed in SU_A41.
    fn climbed_pole(&self) -> bool {
        self.animation == Animation::ClimbOnBeam && self.hips[1] > 280.0 && self.main[1] > 300.0
    }
}

/// The route's legs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leg {
    /// SU_C04: the start-up script holds and stands the slugcat up.
    StartUp,
    WalkToBox,
    BoxStand,
    BoxCrouch,
    BoxStandAgain,
    ClimbBox,
    ToA41,
    /// SU_A41: to the low area under the pole.
    ToPole,
    PoleWait,
    ClimbPole,
    OnPole,
    /// Up the left pole to the tunnel of the exit to SU_A42, and through it.
    ClimbToA42,
    ToA42,
    /// SU_A42: catch and eat bat flies until the stomach holds enough to hibernate.
    A42Wait,
    Hunt,
    Eat,
    /// Back down SU_A42's exit shaft to SU_A41, and down the left pole.
    OutOfA42,
    DownPole,
    DropOffPole,
    ToRightPole,
    ClimbRightPole,
    ToA43,
    /// SU_A43.
    A43Wait,
    ToJumpSpot,
    Crouch,
    Charge,
    Leap,
    /// Fell short into the left pit: its shortcut leads back up.
    PitShortcut,
    /// The shortcut at the top right leads down into the chamber of the exit to SU_A22.
    ToChamber,
    ChamberShortcut,
    ToA22Exit,
    DownA22Exit,
    /// SU_A22.
    A22Wait,
    ToShelter,
    IntoShelter,
    Done,
}

/// Survivor's `foodToHibernate`.
const FOOD_TO_HIBERNATE: i32 = 4;
/// Ticks to stand in SU_A42 with the guide before hunting (the Eat instruction needs
/// `batTrouble` 100).
const A42_WAIT: u64 = 200;
/// Where to wait in SU_A42 while every fly is in a hive: more than 20 tiles from the
/// right hives, so flies come out of them (`FlyEmergeFromHive`).
const LURE_SPOT: [f32; 2] = [150.0, 190.0];
/// Ticks to keep the hips in the box zone (the hologram shows from 800).
const BOX_TICKS: u64 = 1000;
const BOX_CROUCH_FROM: u64 = 880;
const BOX_CROUCH_TICKS: u32 = 60;
/// Ticks to give up waiting for the guide to follow into a room.
const GUIDE_PATIENCE: u32 = 1500;

/// A notable moment of the run.
#[derive(Debug, Clone)]
pub(super) struct RouteEvent {
    pub tick: u64,
    pub what: String,
    pub view: RouteView,
}

pub(super) struct TutorialRoute {
    leg: Leg,
    leg_ticks: u32,
    /// Tick the hips first entered the box zone.
    box_entered: Option<u64>,
    /// Tick the guide was first seen in the player's current room (or given up on).
    guide_ready: Option<u64>,
    /// Stuck detection of [`Self::walk`]: best progress and ticks without it.
    best: f32,
    stalled: u32,
    jump_hold: u32,
    room: String,
    pub events: Vec<RouteEvent>,
    /// Walk into SU_A43's pit before the leap (the guide's super-jump lesson).
    pit_first: bool,
    fell: bool,
    /// Go by SU_A42 and eat bat flies before SU_A43.
    eat: bool,
}

impl TutorialRoute {
    pub(super) fn new() -> Self {
        Self {
            leg: Leg::StartUp,
            leg_ticks: 0,
            box_entered: None,
            guide_ready: None,
            best: f32::MIN,
            stalled: 0,
            jump_hold: 0,
            room: String::new(),
            events: Vec::new(),
            pit_first: false,
            fell: false,
            eat: false,
        }
    }

    /// The route that first drops into SU_A43's pit and climbs back out by its shortcut.
    pub(super) fn pit_first() -> Self {
        Self {
            pit_first: true,
            ..Self::new()
        }
    }

    /// The route that first eats four bat flies in SU_A42, then drops into SU_A43's
    /// pit (where the guide's super-jump lesson needs a fed slugcat).
    pub(super) fn eating() -> Self {
        Self {
            pit_first: true,
            eat: true,
            ..Self::new()
        }
    }

    pub(super) fn done(&self) -> bool {
        self.leg == Leg::Done
    }

    fn note(&mut self, v: &RouteView, what: impl Into<String>) {
        self.events.push(RouteEvent {
            tick: v.tick,
            what: what.into(),
            view: v.clone(),
        });
    }

    fn go(&mut self, v: &RouteView, leg: Leg) {
        self.note(v, format!("{:?} -> {leg:?}", self.leg));
        self.leg = leg;
        self.leg_ticks = 0;
        self.best = f32::MIN;
        self.stalled = 0;
        self.jump_hold = 0;
    }

    /// The input for the next tick.
    pub(super) fn input(&mut self, v: &RouteView) -> InputParam {
        self.track(v);
        let input = self.leg_input(v);
        self.leg_ticks += 1;
        input
    }

    /// Room changes, the guide's arrival and the box zone, whatever the leg.
    fn track(&mut self, v: &RouteView) {
        if v.room != self.room && !v.in_shortcut {
            self.note(v, format!("player in {}", v.room));
            self.room.clone_from(&v.room);
            self.guide_ready = None;
        }
        if self.guide_ready.is_none() && v.guide_here() && !v.in_shortcut {
            self.note(v, format!("guide in {}", v.room));
            self.guide_ready = Some(v.tick);
        }
        if self.box_entered.is_none() && v.room == "SU_C04" && v.hips_in_box_zone() {
            self.note(v, "hips enter the box zone");
            self.box_entered = Some(v.tick);
        }
        if self.box_entered.is_some() && self.since_box(v) == 800 {
            self.note(v, "boxTrouble passes 800 (the route's count)");
        }
    }

    fn since_box(&self, v: &RouteView) -> u64 {
        self.box_entered.map_or(0, |t| v.tick - t)
    }

    /// Ticks since the guide arrived in this room, waiting at most [`GUIDE_PATIENCE`]
    /// ticks of the leg for it.
    fn since_guide(&mut self, v: &RouteView) -> Option<u64> {
        if self.guide_ready.is_none() && self.leg_ticks >= GUIDE_PATIENCE {
            self.note(v, format!("gave up waiting for the guide in {}", v.room));
            self.guide_ready = Some(v.tick);
        }
        self.guide_ready.map(|t| v.tick - t)
    }

    fn leg_input(&mut self, v: &RouteView) -> InputParam {
        match self.leg {
            Leg::StartUp => {
                if !v.held && v.hips_contact_y < 0 {
                    self.go(v, Leg::WalkToBox);
                }
                InputParam::default()
            }
            Leg::WalkToBox => {
                if v.hips[0] > 1740.0 {
                    self.go(v, Leg::BoxStand);
                    return InputParam::default();
                }
                self.walk(v, 1)
            }
            Leg::BoxStand => {
                if self.since_box(v) >= BOX_CROUCH_FROM {
                    self.go(v, Leg::BoxCrouch);
                }
                Self::stand(self.leg_ticks)
            }
            Leg::BoxCrouch => {
                if self.leg_ticks >= BOX_CROUCH_TICKS {
                    self.go(v, Leg::BoxStandAgain);
                }
                Self::y(-1)
            }
            Leg::BoxStandAgain => {
                if self.since_box(v) >= BOX_TICKS && v.hips_in_box_zone() {
                    self.go(v, Leg::ClimbBox);
                }
                Self::stand(self.leg_ticks)
            }
            Leg::ClimbBox => {
                if v.over_box() {
                    self.note(v, "over the box");
                    self.go(v, Leg::ToA41);
                }
                self.walk(v, 1)
            }
            Leg::ToA41 => {
                if v.room == "SU_A41" && !v.in_shortcut {
                    self.go(v, Leg::ToPole);
                }
                self.walk(v, 1)
            }
            Leg::ToPole => {
                if v.hips[0] > 500.0 {
                    self.go(v, Leg::PoleWait);
                    return InputParam::default();
                }
                self.walk(v, 1)
            }
            Leg::PoleWait => {
                if self.since_guide(v).is_some_and(|t| t >= 400) {
                    self.go(v, Leg::ClimbPole);
                }
                InputParam::default()
            }
            Leg::ClimbPole => {
                if v.climbed_pole() && v.main[1] > 340.0 {
                    self.note(v, "climbed the pole");
                    self.go(v, Leg::OnPole);
                }
                Self::y(1)
            }
            Leg::OnPole => {
                if self.leg_ticks >= 200 {
                    let next = if self.eat && v.food < FOOD_TO_HIBERNATE {
                        Leg::ClimbToA42
                    } else {
                        Leg::DropOffPole
                    };
                    self.go(v, next);
                }
                InputParam::default()
            }
            Leg::ClimbToA42 => {
                if v.main[1] > 580.0 {
                    self.go(v, Leg::ToA42);
                }
                Self::y(1)
            }
            Leg::ToA42 => {
                if v.room == "SU_A42" && !v.in_shortcut {
                    self.go(v, Leg::A42Wait);
                }
                // Off the pole into the tunnel with a jump to the left.
                InputParam {
                    x: -1,
                    jump: v.animation == Animation::ClimbOnBeam && self.leg_ticks % 20 < 5,
                    ..InputParam::default()
                }
            }
            Leg::A42Wait => {
                if self.since_guide(v).is_some_and(|t| t >= A42_WAIT) {
                    self.go(v, Leg::Hunt);
                }
                InputParam::default()
            }
            Leg::Hunt => {
                if v.holding {
                    self.go(v, Leg::Eat);
                    return InputParam::default();
                }
                self.hunt(v)
            }
            Leg::Eat => {
                if !v.holding {
                    self.note(v, format!("ate a bat fly: food {}", v.food));
                    let next = if v.food >= FOOD_TO_HIBERNATE {
                        Leg::OutOfA42
                    } else {
                        Leg::Hunt
                    };
                    self.go(v, next);
                }
                InputParam {
                    pickup: true,
                    ..InputParam::default()
                }
            }
            Leg::OutOfA42 => {
                if v.room == "SU_A41" && !v.in_shortcut {
                    self.go(v, Leg::DownPole);
                }
                self.out_of_a42(v)
            }
            Leg::DownPole => {
                if v.animation != Animation::ClimbOnBeam {
                    return InputParam {
                        x: i8::from(v.main[0] < 525.0),
                        y: 1,
                        ..InputParam::default()
                    };
                }
                if v.main[1] < 420.0 {
                    self.go(v, Leg::DropOffPole);
                }
                Self::y(-1)
            }
            Leg::DropOffPole => {
                if v.animation != Animation::ClimbOnBeam && v.hips_contact_y < 0 {
                    self.go(v, Leg::ToRightPole);
                }
                InputParam {
                    x: 1,
                    jump: self.leg_ticks % 20 < 5,
                    ..InputParam::default()
                }
            }
            Leg::ToRightPole => {
                if v.hips[0] > 815.0 {
                    self.go(v, Leg::ClimbRightPole);
                }
                self.walk(v, 1)
            }
            Leg::ClimbRightPole => {
                if v.main[1] > 475.0 {
                    self.go(v, Leg::ToA43);
                }
                Self::y(1)
            }
            Leg::ToA43 => {
                if v.room == "SU_A43" && !v.in_shortcut {
                    self.go(v, Leg::A43Wait);
                }
                self.walk(v, 1)
            }
            Leg::A43Wait => {
                if self.since_guide(v).is_some_and(|t| t >= 250) {
                    self.go(v, Leg::ToJumpSpot);
                }
                InputParam::default()
            }
            Leg::ToJumpSpot if self.pit_first && !self.fell => {
                if v.hips[1] < 420.0 && v.hips_contact_y < 0 {
                    self.fell = true;
                    self.note(v, "fell into the left pit");
                    self.go(v, Leg::PitShortcut);
                    return InputParam::default();
                }
                self.walk(v, 1)
            }
            Leg::ToJumpSpot => {
                if v.hips[0] > 300.0 && v.hips[1] > 455.0 && v.hips_contact_y < 0 {
                    self.go(v, Leg::Crouch);
                    return InputParam::default();
                }
                self.walk(v, 1)
            }
            Leg::Crouch => {
                if self.leg_ticks >= 20 {
                    self.go(v, Leg::Charge);
                }
                Self::y(-1)
            }
            Leg::Charge => {
                // The guide's super-jump lesson holds the charge down until the jump has
                // been held 60 ticks in place (`SuperJump` controller).
                let hold = if self.fell { 100 } else { 30 };
                if self.leg_ticks >= hold {
                    self.go(v, Leg::Leap);
                    return InputParam::default();
                }
                InputParam {
                    jump: true,
                    ..InputParam::default()
                }
            }
            Leg::Leap => {
                if self.leg_ticks > 10 && v.hips_contact_y < 0 {
                    if v.hips[0] > 460.0 {
                        self.note(v, "across the gap");
                        self.go(v, Leg::ToChamber);
                    } else if v.hips[1] < 420.0 {
                        self.note(v, "fell into the left pit");
                        self.go(v, Leg::PitShortcut);
                    } else {
                        self.go(v, Leg::ToJumpSpot);
                    }
                }
                InputParam::default()
            }
            Leg::PitShortcut => {
                if v.in_shortcut {
                    self.go(v, Leg::ToJumpSpot);
                }
                self.walk(v, -1)
            }
            Leg::ToChamber => {
                if v.in_shortcut {
                    self.go(v, Leg::ChamberShortcut);
                }
                self.walk(v, 1)
            }
            Leg::ChamberShortcut => {
                if !v.in_shortcut {
                    self.go(v, Leg::ToA22Exit);
                }
                InputParam::default()
            }
            Leg::ToA22Exit => {
                if (v.hips[0] - 690.0).abs() < 6.0 && v.hips_contact_y < 0 {
                    self.go(v, Leg::DownA22Exit);
                }
                self.walk(v, if v.hips[0] < 690.0 { 1 } else { -1 })
            }
            Leg::DownA22Exit => {
                if v.room == "SU_A22" && !v.in_shortcut {
                    self.go(v, Leg::A22Wait);
                }
                Self::y(-1)
            }
            Leg::A22Wait => {
                if self.since_guide(v).is_some_and(|t| t >= 150) {
                    self.go(v, Leg::ToShelter);
                }
                InputParam::default()
            }
            Leg::ToShelter => {
                if v.in_shortcut {
                    self.note(
                        v,
                        format!("into the shortcut at tile {:?}", v.shortcut_tile),
                    );
                    self.go(v, Leg::IntoShelter);
                    return InputParam::default();
                }
                self.walk(v, -1)
            }
            Leg::IntoShelter => {
                if v.room == "SU_S01" || v.lobby_room.as_deref() == Some("SU_S01") {
                    self.note(v, "on the way into SU_S01");
                    self.go(v, Leg::Done);
                }
                InputParam::default()
            }
            Leg::Done => InputParam::default(),
        }
    }

    /// Toward the nearest free bat fly, jumping when it is above or when no progress
    /// was made for a while (out of the exit shaft, onto ledges), tapping pick-up when
    /// it is in reach. With every fly in its hive, back off to the left end, where the
    /// flies come out of the far hives.
    fn hunt(&mut self, v: &RouteView) -> InputParam {
        let target = v.flies.iter().min_by(|a, b| {
            let da = (a[0] - v.main[0]).hypot(a[1] - v.main[1]);
            let db = (b[0] - v.main[0]).hypot(b[1] - v.main[1]);
            da.total_cmp(&db)
        });
        let to = target.copied().unwrap_or(LURE_SPOT);
        let dx = to[0] - v.main[0];
        let dy = to[1] - v.main[1];
        let x = if dx.abs() < 10.0 {
            0
        } else {
            dx.signum() as i8
        };
        let moved = (v.main[0] - self.best).abs() > 2.0;
        if moved || x == 0 {
            self.best = v.main[0];
            self.stalled = 0;
        } else {
            self.stalled += 1;
        }
        let above = target.is_some() && dy > 15.0 && dx.abs() < 60.0;
        if self.jump_hold == 0 && ((above && v.hips_contact_y < 0) || self.stalled > 20) {
            self.jump_hold = 20;
            self.stalled = 0;
        }
        let jump = self.jump_hold > 0;
        self.jump_hold = self.jump_hold.saturating_sub(1);
        let near = target.is_some() && dx.hypot(dy) < 50.0;
        InputParam {
            x,
            jump,
            pickup: near && self.leg_ticks.is_multiple_of(2),
            ..InputParam::default()
        }
    }

    /// Along SU_A42's floor to its exit shaft (tile 10) and down it.
    fn out_of_a42(&mut self, v: &RouteView) -> InputParam {
        if v.in_shortcut || v.room != "SU_A42" {
            return InputParam::default();
        }
        let tx = (v.main[0] / 20.0).floor();
        if tx > 11.0 {
            self.walk(v, -1)
        } else if tx < 10.0 {
            self.walk(v, 1)
        } else {
            Self::y(-1)
        }
    }

    /// Stand up (hold up briefly) and then stand still.
    fn stand(ticks: u32) -> InputParam {
        Self::y(i8::from(ticks < 10))
    }

    fn y(y: i8) -> InputParam {
        InputParam {
            y,
            ..InputParam::default()
        }
    }

    /// Walk in direction `dir`, jumping (holding the direction) when no progress was
    /// made for a while.
    fn walk(&mut self, v: &RouteView, dir: i8) -> InputParam {
        let progress = v.hips[0] * f32::from(dir);
        if progress > self.best + 2.0 {
            self.best = progress;
            self.stalled = 0;
        } else {
            self.stalled += 1;
        }
        if self.stalled > 20 && self.jump_hold == 0 {
            self.jump_hold = 30;
            self.stalled = 0;
        }
        let jump = self.jump_hold > 15;
        self.jump_hold = self.jump_hold.saturating_sub(1);
        InputParam {
            x: dir,
            jump,
            ..InputParam::default()
        }
    }
}

/// The guide's `OverseerTutorialBehavior` counters after a tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct GuideCounters {
    box_trouble: i32,
    jumped_over_box: bool,
    pole_trouble: i32,
    climbed_pole: bool,
}

/// One tick of a run: the view the route chose from, its input, and the guide's
/// counters after the tick.
struct RouteTick {
    view: RouteView,
    input: InputParam,
    guide: Option<GuideCounters>,
}

impl RouteTick {
    /// The first tick after which the guide's counters satisfy `cond`.
    fn first(ticks: &[Self], cond: fn(&GuideCounters) -> bool) -> Option<u64> {
        ticks
            .iter()
            .find(|t| t.guide.as_ref().is_some_and(cond))
            .map(|t| t.view.tick)
    }
}

impl Session {
    fn route_survivor() -> Option<Self> {
        let mut s = Self::installed()?;
        s.new_game(&NewGameParams {
            slugcat: "White".to_owned(),
            seed: Some(1),
        })
        .expect("new game");
        Some(s)
    }

    fn guide_counters(&mut self) -> Option<GuideCounters> {
        let scene = self.scene_mut()?;
        let world = scene.game().sim().world();
        let t = world
            .creatures()
            .filter_map(|(_, c)| c.overseer_ai())
            .find(|o| o.is_player_guide)?
            .real_ai
            .as_deref()?
            .tutorial
            .as_ref()?;
        Some(GuideCounters {
            box_trouble: t.box_trouble,
            jumped_over_box: t.player_has_jumped_over_box,
            pole_trouble: t.pole_trouble,
            climbed_pole: t.player_has_climbed_pole,
        })
    }

    /// Runs the route until it is done or `max_ticks` passed.
    fn run_route(&mut self, route: &mut TutorialRoute, max_ticks: u64) -> Vec<RouteTick> {
        let mut ticks = Vec::new();
        let mut state = self.state().expect("state");
        while !route.done() && state.tick < max_ticks {
            let view = RouteView::of(&state);
            let input = route.input(&view);
            state = self
                .step(&StepParams {
                    ticks: Some(1),
                    input: Some(input),
                })
                .expect("steps");
            ticks.push(RouteTick {
                view,
                input,
                guide: self.guide_counters(),
            });
        }
        ticks
    }
}

const MAX_TICKS: u64 = 20_000;

/// Prints every tick (or every `ROUTE_EVERY`th) of the route: the input, the player's
/// room and chunks, the guide's room and its tutorial counters; then the route's events
/// and the ticks the guide's counters crossed the hologram thresholds.
#[test]
#[ignore = "prints the route's run"]
fn print_tutorial_route() {
    let Some(mut s) = Session::route_survivor() else {
        return;
    };
    let mut route = if std::env::var_os("ROUTE_EAT").is_some() {
        TutorialRoute::eating()
    } else if std::env::var_os("ROUTE_PIT").is_some() {
        TutorialRoute::pit_first()
    } else {
        TutorialRoute::new()
    };
    let ticks = s.run_route(&mut route, MAX_TICKS);
    let every: u64 = std::env::var("ROUTE_EVERY")
        .ok()
        .and_then(|e| e.parse().ok())
        .unwrap_or(1);
    for t in ticks.iter().filter(|t| t.view.tick % every == 0) {
        let v = &t.view;
        println!(
            "{:>5} in x{} y{} j{} p{} | {} main {:?} hips {:?} cy {} {} sc {} {:?} lobby {:?} | food {} hold {} flies {:?} | guide {:?} {:?}",
            v.tick,
            t.input.x,
            t.input.y,
            u8::from(t.input.jump),
            u8::from(t.input.pickup),
            v.room,
            v.main,
            v.hips,
            v.hips_contact_y,
            v.animation.value(),
            v.in_shortcut,
            v.shortcut_tile,
            v.lobby_room,
            v.food,
            v.holding,
            v.flies,
            v.guide_room,
            t.guide,
        );
    }
    for e in &route.events {
        println!(
            "EVENT {:>5} {}: {} main {:?} hips {:?} guide in {:?}",
            e.tick, e.what, e.view.room, e.view.main, e.view.hips, e.view.guide_room
        );
    }
    println!(
        "GUIDE boxTrouble > 800 after tick {:?}; jumped over the box after {:?}; \
         poleTrouble > 200 after {:?}; climbed the pole after {:?}",
        RouteTick::first(&ticks, |g| g.box_trouble > 800),
        RouteTick::first(&ticks, |g| g.jumped_over_box),
        RouteTick::first(&ticks, |g| g.pole_trouble > 200),
        RouteTick::first(&ticks, |g| g.climbed_pole),
    );
    println!(
        "done {} at tick {}",
        route.done(),
        ticks.last().map_or(0, |t| t.view.tick)
    );
}

/// The route gets through the tutorial rooms into the shelter.
#[test]
fn tutorial_route_reaches_the_shelter() {
    let Some(mut s) = Session::route_survivor() else {
        return;
    };
    let mut route = TutorialRoute::new();
    s.run_route(&mut route, MAX_TICKS);
    let events: Vec<&str> = route.events.iter().map(|e| e.what.as_str()).collect();
    assert!(route.done(), "stuck in {:?}; events {events:?}", route.leg);
    for want in [
        "over the box",
        "climbed the pole",
        "across the gap",
        "on the way into SU_S01",
    ] {
        assert!(events.contains(&want), "no {want:?} in {events:?}");
    }
    let shortcut = route
        .events
        .iter()
        .find(|e| e.what.starts_with("into the shortcut"))
        .and_then(|e| e.view.shortcut_tile);
    assert_eq!(shortcut, Some([12, 16]), "SU_A22's exit to SU_S01");
}
