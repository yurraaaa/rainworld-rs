//! `RoomSpecificScript`: the objects `Room.Loaded` adds to particular rooms
//! (`AddRoomSpecificScript`), of which the start of a new Survivor or Monk game in
//! SU_C04 is ported. Behaviour: `rainworld-re/notes/intro-overseer-survey.md` section
//! 2.3 and `slideshow.md`.

use rw_assets::{ActiveMods, SlugcatName};

use crate::game::PlayerObserver;
use crate::{Player, PlayerController, Vec2};

/// What `AddRoomSpecificScript` reads of the story session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorySession {
    /// `StoryCharacter`.
    pub character: SlugcatName,
    /// `saveState.cycleNumber`.
    pub cycle_number: i32,
    /// `saveState.denPosition`.
    pub den_position: String,
}

/// What the scripts read of the game each tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptTick {
    /// `room.game.manager.FadeDelayInProgress`.
    pub fade_delay_in_progress: bool,
    /// `cameras[0].hud != null` with an active control preset (the keyboard's, always).
    pub hud_with_control_preset: bool,
}

/// A room's script.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomScript {
    SuC04StartUp(SuC04StartUp),
}

impl RoomScript {
    /// `RoomSpecificScript.AddRoomSpecificScript` for a room whose settings ask for a
    /// script. Only SU_C04's is ported; the others the base game adds are `SS_E08`
    /// (gradient gravity), `SU_A22` (first-cycle message), `SL_C12` (jetfish), `SB_A14`
    /// (karma), `SB_D03` (shortcut lock), `SU_A43` (super-jump only), `deathPit`,
    /// `LF_H01` (the Hunter's `HardmodeStart`), `LF_A03` and `SS_D02`; More Slugcats and
    /// the Watcher add their own lists after these.
    #[must_use]
    pub fn for_room(
        name: &str,
        session: Option<&StorySession>,
        mods: ActiveMods,
        _first_time_realized: bool,
    ) -> Vec<Self> {
        let early_cycle = !(mods.msc || mods.watcher)
            || session.is_some_and(|s| {
                s.cycle_number < 2 && !(mods.msc && s.character == SlugcatName::Spear)
            });
        match name {
            "SU_C04" if early_cycle => vec![Self::SuC04StartUp(SuC04StartUp::new())],
            // "LF_H01": `HardmodeStart` when the Hunter's first cycle starts in its den
            // and the room is realized for the first time (needs creatures, the spear,
            // the map and the Hunter's stats).
            _ => Vec::new(),
        }
    }

    /// `Update`; `player` is the player when its body is in the room.
    pub fn update(
        &mut self,
        player: Option<&mut Player>,
        tick: ScriptTick,
        observer: &mut dyn PlayerObserver,
    ) {
        match self {
            Self::SuC04StartUp(s) => s.update(player, tick, observer),
        }
    }
}

/// `SU_C04StartUp`: a new game's slugcat lies in the tunnel with no control and is
/// pushed out to the right, then stands up when it drops below the tunnel.
#[derive(Debug, Clone, PartialEq)]
pub struct SuC04StartUp {
    /// Per player number: the furthest x the body's middle reached.
    right_most: [f32; 4],
    stood_up: bool,
    push_right: [bool; 4],
    counter: i32,
    showed_controls: bool,
    show_controls_counter: i32,
    /// `tutCntrlPgOwner.controlsPage.wantToContinue`, set by this script.
    controls_page_continue: bool,
}

impl Default for SuC04StartUp {
    fn default() -> Self {
        Self::new()
    }
}

impl SuC04StartUp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            right_most: [0.0; 4],
            stood_up: false,
            push_right: [true; 4],
            counter: 0,
            showed_controls: false,
            show_controls_counter: 0,
            controls_page_continue: false,
        }
    }

    #[must_use]
    pub fn stood_up(&self) -> bool {
        self.stood_up
    }

    /// `pushRight[0]`.
    #[must_use]
    pub fn push_right(&self) -> bool {
        self.push_right[0]
    }

    #[must_use]
    pub fn counter(&self) -> i32 {
        self.counter
    }

    #[must_use]
    pub fn show_controls_counter(&self) -> i32 {
        self.show_controls_counter
    }

    /// Whether the `TutorialControlsPageOwner` is due but not made yet (it also needs
    /// the HUD and a control preset).
    #[must_use]
    pub fn controls_page_due(&self) -> bool {
        !self.showed_controls && self.show_controls_counter > 100
    }

    /// `showedControls`: the script made its `TutorialControlsPageOwner` (it keeps
    /// `tutCntrlPgOwner` from then on).
    #[must_use]
    pub fn showed_controls(&self) -> bool {
        self.showed_controls
    }

    /// `wantToContinue` of the controls page this script made: the player went past
    /// x 800.
    #[must_use]
    pub fn controls_page_continue(&self) -> bool {
        self.controls_page_continue
    }

    /// `Update` for player 0 (Jolly Co-op's other players, pushed from tick `5 * n`,
    /// are not ported).
    fn update(
        &mut self,
        mut player: Option<&mut Player>,
        tick: ScriptTick,
        observer: &mut dyn PlayerObserver,
    ) {
        self.counter += 1;
        if let Some(p) = player.as_deref_mut() {
            if self.push_right[0] && p.controller().is_none() {
                p.set_controller(Some(PlayerController::Null));
            }
            if !self.push_right[0] && p.controller() == Some(PlayerController::Null) {
                p.set_controller(None);
            }
        }
        if tick.fade_delay_in_progress {
            return;
        }
        if self.show_controls_counter > 0 {
            self.show_controls_counter += 1;
        }
        let Some(p) = player else { return };
        let head = p.head().pos;
        if self.push_right[0] && head.y > 550.0 && head.x < 350.0 {
            let [c0, c1] = &mut p.body.chunks[..] else {
                return;
            };
            let mut middle = (c0.pos + c1.pos) / 2.0;
            self.right_most[0] = middle.x.max(self.right_most[0]);
            middle.x = self.right_most[0];
            c0.hard_set_position(middle + Vec2::new(9.0, 0.0));
            c1.hard_set_position(middle + Vec2::new(-5.0, 0.0));
            observer.nudge_tail_x(-1.0);
        } else {
            self.push_right[0] = false;
        }
        if p.head().pos.y < 350.0 && !self.stood_up {
            self.stood_up = true;
            p.set_standing(true);
            self.show_controls_counter = 1;
        }
        if self.controls_page_due() && tick.hud_with_control_preset {
            self.showed_controls = true;
        }
        if self.showed_controls && p.head().pos.x > 800.0 {
            self.controls_page_continue = true;
        }
    }
}
