//! Room settings triggers in a realized room: `ActiveTriggerChecker` (made by every
//! `Room.Loaded` for the triggers the save lets through), the room objects their events
//! add (`PickupObjectInstruction`, `RoomSpecificTextMessage`), the guide's placed
//! triggers (`SuperJumpInstruction`, `ScavengerTradeInstructionTrigger`,
//! `ReliableIggyDirection`), and the song requests the
//! sim hands the music player (`rw-audio`). Behaviour: `rainworld-re/notes/music.md`
//! section 4.2; the game side is `game_triggers.rs`.

use rw_assets::{
    EventTrigger, IggySymbol, MusicEvent, QuadData, ReliableIggyDirectionData, StopMusicEvent,
    TriggerType,
};
use rw_random::UnityRandom;

use crate::Vec2;
use crate::item::ObjectId;

/// What the game asks of `manager.musicPlayer` during a tick, in order. The app applies
/// them before the music player's update, which runs after the game's.
#[derive(Debug, Clone, PartialEq)]
pub enum MusicRequest {
    /// `GameRequestsSong` from a `MusicEvent` trigger.
    Song(MusicEvent),
    /// `GameRequestsSongStop` from a `StopMusicEvent` trigger.
    Stop(StopMusicEvent),
    /// `RainCycle.Update`'s `RainRequestStopSong`: music is not allowed and camera 0's
    /// room has rain danger (the player only acts with a song or a next song).
    RainStop,
}

/// What a checker reads of `Players[0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriggerPlayer {
    /// `Players[0].Room == room.abstractRoom`.
    pub in_room: bool,
    /// `realizedCreature != null`.
    pub realized: bool,
    /// `pos.abstractNode`.
    pub node: i32,
    pub still_in_start_shelter: bool,
    pub in_shortcut: bool,
    /// `Player.Karma` (0 outside a story).
    pub karma: i32,
    /// `mainBodyChunk.pos`.
    pub pos: Vec2,
}

/// The HUD's state the checkers read (camera 0 follows `Players[0]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TriggerHud {
    /// `GameOverModeActive`.
    pub game_over_mode: bool,
    /// The text prompt's subregion tracker's `PreRegionBump` and `RegionBump`; `None`
    /// without a HUD or tracker.
    pub region_bumps: Option<(bool, bool)>,
}

/// `ActiveTriggerChecker` for one of the room settings' triggers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTriggerChecker {
    /// Index into the room settings' `triggers`.
    pub trigger: usize,
    pub counter: i32,
    pub wait: i32,
}

impl ActiveTriggerChecker {
    #[must_use]
    pub fn new(trigger: usize) -> Self {
        Self {
            trigger,
            counter: -1,
            wait: 10,
        }
    }

    /// `Update` up to `FireEvent`: whether the event fires this tick (Watcher's rot
    /// presence branch is out). `SeeCreature` and unknown types never fire.
    pub fn update(
        &mut self,
        trigger: &EventTrigger,
        player: &TriggerPlayer,
        hud: TriggerHud,
    ) -> bool {
        if self.counter >= 0 {
            self.counter += 1;
            if self.counter >= trigger.delay {
                self.counter = -1;
                return true;
            }
            return false;
        }
        match trigger.trigger_type {
            Some(TriggerType::Spot) => {
                if !player.in_room {
                    return false;
                }
                if self.wait > 0 {
                    self.wait -= 1;
                    return false;
                }
                let spot = trigger.spot.unwrap_or_default();
                if Self::conditions(trigger, player, hud)
                    && player
                        .pos
                        .dist_less(Vec2::new(spot.pos[0], spot.pos[1]), spot.rad)
                {
                    return self.positive(trigger);
                }
                false
            }
            Some(kind @ (TriggerType::PreRegionBump | TriggerType::RegionBump)) => {
                let Some((pre, bump)) = hud.region_bumps else {
                    return false;
                };
                if !player.in_room || !Self::conditions(trigger, player, hud) {
                    return false;
                }
                let flag = if kind == TriggerType::PreRegionBump {
                    pre
                } else {
                    bump
                };
                flag && self.positive(trigger)
            }
            Some(TriggerType::SeeCreature) | None => false,
        }
    }

    /// `TriggerConditions`.
    fn conditions(trigger: &EventTrigger, player: &TriggerPlayer, hud: TriggerHud) -> bool {
        (trigger.entrance < 0 || player.node == trigger.entrance)
            && player.realized
            && !player.still_in_start_shelter
            && !player.in_shortcut
            && player.karma >= trigger.karma
            && !hud.game_over_mode
    }

    /// `Positive`: fires now without a delay, else starts the counter.
    fn positive(&mut self, trigger: &EventTrigger) -> bool {
        if trigger.delay < 1 {
            return true;
        }
        if self.counter < 0 {
            self.counter = 0;
        }
        false
    }

    /// `FireEvent`'s chance: `None` without an event (nothing happens and the checker
    /// stays), else whether the event goes off. A chance of exactly 1 draws nothing.
    pub fn roll(trigger: &EventTrigger, rng: &mut UnityRandom) -> Option<bool> {
        trigger.event.as_ref()?;
        Some(trigger.fire_chance == 1.0 || rng.value() < trigger.fire_chance)
    }
}

/// `InputInstructionTrigger`'s own fields.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InstructionTrigger {
    pub instruction_pos: Vec2,
    pub activated: bool,
    pub completed: bool,
    pub time_since_activated: i32,
}

/// `PickupObjectInstruction` (an `InputInstructionTrigger`): points the guide at the
/// nearest free rock or spear until the player has carried one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PickupObjectInstruction {
    pub trigger: InstructionTrigger,
    pub item: Option<ObjectId>,
    pub player_has_picked_up_an_item: bool,
    pub mark_instruction_as_followed: bool,
}

impl PickupObjectInstruction {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// `SuperJumpInstruction`, made by its placed object: the guide shows the super jump
/// once the player drops through the line between handles 2 and 1.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperJumpInstruction {
    pub trigger: InstructionTrigger,
    /// The placed object's position and handles (`QuadObjectData`).
    pub pos: Vec2,
    pub handles: [Vec2; 3],
}

impl SuperJumpInstruction {
    #[must_use]
    pub fn new(pos: Vec2, quad: QuadData) -> Self {
        Self {
            trigger: InstructionTrigger {
                instruction_pos: pos,
                ..InstructionTrigger::default()
            },
            pos,
            handles: quad.handles.map(Vec2::from),
        }
    }

    #[must_use]
    pub fn trg_a(&self) -> Vec2 {
        self.pos + self.handles[2]
    }

    #[must_use]
    pub fn trg_b(&self) -> Vec2 {
        self.pos + self.handles[1]
    }

    /// `CheckTrigger`'s test on the player's main chunk: not rising, and its last move
    /// crosses the trigger line.
    #[must_use]
    pub fn crossed(&self, last_pos: Vec2, pos: Vec2) -> bool {
        if last_pos.y < pos.y {
            return false;
        }
        let (a, b) = (self.trg_a(), self.trg_b());
        let v = last_pos.line_intersection(pos, a, b);
        v.is_between_points(a, b) && v.is_between_points(last_pos, pos)
    }
}

/// `ScavengerTradeInstructionTrigger` (namespace `ScavTradeInstruction`), made by its
/// placed object: activated by a realized data pearl that came from this room.
#[derive(Debug, Clone, PartialEq)]
pub struct ScavengerTradeInstructionTrigger {
    pub trigger: InstructionTrigger,
    pub pos: Vec2,
    pub pearl: Option<ObjectId>,
}

impl ScavengerTradeInstructionTrigger {
    #[must_use]
    pub fn new(pos: Vec2) -> Self {
        Self {
            trigger: InstructionTrigger {
                instruction_pos: pos,
                ..InstructionTrigger::default()
            },
            pos,
            pearl: None,
        }
    }
}

/// `ReliableIggyDirection`, made by its placed object: the guide points at an exit
/// while the player is within the radius.
#[derive(Debug, Clone, PartialEq)]
pub struct ReliableIggyDirection {
    pub pos: Vec2,
    pub data: ReliableIggyDirectionData,
    pub has_been_activated: bool,
    pub first_update: bool,
}

impl ReliableIggyDirection {
    #[must_use]
    pub fn new(pos: Vec2, data: ReliableIggyDirectionData) -> Self {
        Self {
            pos,
            data,
            has_been_activated: false,
            first_update: true,
        }
    }
}

/// What a trigger room object is.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerObjectKind {
    Checker(ActiveTriggerChecker),
    PickupInstruction(PickupObjectInstruction),
    SuperJumpInstruction(SuperJumpInstruction),
    ScavTradeInstruction(ScavengerTradeInstructionTrigger),
    ReliableIggyDirection(Box<ReliableIggyDirection>),
    /// `RoomSpecificTextMessage`.
    TextMessage,
}

/// A trigger room object in a realized room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerObject {
    /// Unique; also the object's place in the update list (`Room.AddObject`).
    pub id: u64,
    pub room: i32,
    /// `slatedForDeletetion`.
    pub slated: bool,
    pub kind: TriggerObjectKind,
}

/// What the guide's communication reads of an `InputInstructionTrigger`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputInstructionView {
    pub id: u64,
    pub room: i32,
    pub slated: bool,
    pub trigger: InstructionTrigger,
    pub kind: InstructionViewKind,
}

/// The input instruction's class and what its controller reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstructionViewKind {
    Pickup {
        player_has_picked_up_an_item: bool,
        mark_instruction_as_followed: bool,
    },
    SuperJump {
        pos: Vec2,
        handles: [Vec2; 3],
    },
    ScavengerTrade {
        pos: Vec2,
        pearl: Option<ObjectId>,
    },
}

/// What the guide reads of a `ReliableIggyDirection` (`forcedDirectionToGive`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForcedDirectionView {
    pub id: u64,
    pub room: i32,
    pub exit: i32,
    pub symbol: Option<IggySymbol>,
}

impl TriggerObject {
    /// The view of an input instruction object.
    #[must_use]
    pub fn instruction_view(&self) -> Option<InputInstructionView> {
        let (trigger, kind) = match &self.kind {
            TriggerObjectKind::PickupInstruction(p) => (
                p.trigger,
                InstructionViewKind::Pickup {
                    player_has_picked_up_an_item: p.player_has_picked_up_an_item,
                    mark_instruction_as_followed: p.mark_instruction_as_followed,
                },
            ),
            TriggerObjectKind::SuperJumpInstruction(s) => (
                s.trigger,
                InstructionViewKind::SuperJump {
                    pos: s.pos,
                    handles: s.handles,
                },
            ),
            TriggerObjectKind::ScavTradeInstruction(s) => (
                s.trigger,
                InstructionViewKind::ScavengerTrade {
                    pos: s.pos,
                    pearl: s.pearl,
                },
            ),
            _ => return None,
        };
        Some(InputInstructionView {
            id: self.id,
            room: self.room,
            slated: self.slated,
            trigger,
            kind,
        })
    }

    /// The view of a `ReliableIggyDirection`.
    #[must_use]
    pub fn forced_direction_view(&self) -> Option<ForcedDirectionView> {
        let TriggerObjectKind::ReliableIggyDirection(d) = &self.kind else {
            return None;
        };
        Some(ForcedDirectionView {
            id: self.id,
            room: self.room,
            exit: d.data.exit,
            symbol: d.data.symbol,
        })
    }

    /// The `InputInstructionTrigger` fields of an input instruction object.
    pub fn instruction_trigger_mut(&mut self) -> Option<&mut InstructionTrigger> {
        match &mut self.kind {
            TriggerObjectKind::PickupInstruction(p) => Some(&mut p.trigger),
            TriggerObjectKind::SuperJumpInstruction(s) => Some(&mut s.trigger),
            TriggerObjectKind::ScavTradeInstruction(s) => Some(&mut s.trigger),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A07: &str = "Spot<tA>event<tB>MusicEvent<eA>songName<eB>NA_01 - Proxima<eA>vol<eB>0.3108693<tA>multiUse<tB>0<tA>toCycle<tB>-1<tA>fromCycle<tB>0<tA>delay<tB>0<tA>fireChance<tB>1<tA>entrance<tB>-1<tA>karma<tB>0<tA>slugcats<tB><tA>pos<tB>710.0718<tB>289.2014<tA>handlePos<tB>-103.3553<tB>-215.3553";

    impl TriggerPlayer {
        fn at(pos: Vec2) -> Self {
            TriggerPlayer {
                in_room: true,
                realized: true,
                node: 1,
                still_in_start_shelter: false,
                in_shortcut: false,
                karma: 0,
                pos,
            }
        }
    }

    /// `wait` 10: the first ten ticks with the player in the room count down; the
    /// eleventh inside the circle (|handlePos| = 238.9 px) fires.
    #[test]
    fn spot_waits_ten_ticks_then_fires_inside_the_circle() {
        let t = EventTrigger::parse(A07).unwrap();
        let mut c = ActiveTriggerChecker::new(0);
        let inside = TriggerPlayer::at(Vec2::new(710.0, 100.0));
        for _ in 0..10 {
            assert!(!c.update(&t, &inside, TriggerHud::default()));
        }
        assert_eq!(c.wait, 0);
        let outside = TriggerPlayer::at(Vec2::new(710.0, 30.0));
        assert!(!c.update(&t, &outside, TriggerHud::default()));
        assert!(c.update(&t, &inside, TriggerHud::default()));
    }

    #[test]
    fn conditions_block_and_absence_keeps_the_wait() {
        let t = EventTrigger::parse(&A07.replace("entrance<tB>-1", "entrance<tB>3")).unwrap();
        let mut c = ActiveTriggerChecker::new(0);
        c.wait = 0;
        let mut p = TriggerPlayer::at(Vec2::new(710.0, 289.0));
        assert!(!c.update(&t, &p, TriggerHud::default()));
        p.node = 3;
        let over = TriggerHud {
            game_over_mode: true,
            region_bumps: None,
        };
        assert!(!c.update(&t, &p, over));
        p.in_shortcut = true;
        assert!(!c.update(&t, &p, TriggerHud::default()));
        p.in_shortcut = false;
        assert!(c.update(&t, &p, TriggerHud::default()));
        let mut d = ActiveTriggerChecker::new(0);
        p.in_room = false;
        assert!(!d.update(&t, &p, TriggerHud::default()));
        assert_eq!(d.wait, 10);
    }

    /// A delay starts the counter on the first positive tick; the event fires once the
    /// counter reaches it, wherever the player is by then.
    #[test]
    fn delay_counts_from_the_first_positive_tick() {
        let t = EventTrigger::parse(&A07.replace("delay<tB>0", "delay<tB>3")).unwrap();
        let mut c = ActiveTriggerChecker::new(0);
        c.wait = 0;
        let inside = TriggerPlayer::at(Vec2::new(710.0, 289.0));
        assert!(!c.update(&t, &inside, TriggerHud::default()));
        assert_eq!(c.counter, 0);
        let mut away = inside;
        away.in_room = false;
        assert!(!c.update(&t, &away, TriggerHud::default()));
        assert!(!c.update(&t, &away, TriggerHud::default()));
        assert!(c.update(&t, &away, TriggerHud::default()));
        assert_eq!(c.counter, -1);
    }

    #[test]
    fn region_bump_triggers_read_the_tracker() {
        let t = EventTrigger::parse("RegionBump<tA>event<tB>StopMusicEvent").unwrap();
        let mut c = ActiveTriggerChecker::new(0);
        let p = TriggerPlayer::at(Vec2::ZERO);
        assert!(!c.update(&t, &p, TriggerHud::default()));
        let pre = TriggerHud {
            game_over_mode: false,
            region_bumps: Some((true, false)),
        };
        assert!(!c.update(&t, &p, pre));
        let bump = TriggerHud {
            game_over_mode: false,
            region_bumps: Some((false, true)),
        };
        assert!(c.update(&t, &p, bump));
        let see = EventTrigger::parse("SeeCreature<tA>event<tB>StopMusicEvent").unwrap();
        assert!(!c.update(&see, &p, bump));
    }

    /// SU_B04's 0.4565217 chance draws one `Random.value` per fire; a chance of 1 and a
    /// trigger without an event draw nothing.
    /// SU_A13's object: the line from handle 2 (468.6972, 297.9665) to handle 1
    /// (210.6972, 203.9665). A main chunk falling across it fires; rising across it, or
    /// falling straight down (outside `IsPointBetweenPoints`' strict box), does not.
    #[test]
    fn super_jump_trigger_line() {
        let quad = QuadData::parse("-271~-102~-265~-126~-6~-32").unwrap();
        let s = SuperJumpInstruction::new(Vec2::new(475.6972, 329.9665), quad);
        assert_eq!(s.trigger.instruction_pos, Vec2::new(475.6972, 329.9665));
        let mid = (s.trg_a() + s.trg_b()) / 2.0;
        let above = mid + Vec2::new(-1.0, 8.0);
        let below = mid + Vec2::new(1.0, -8.0);
        assert!(s.crossed(above, below));
        assert!(!s.crossed(below, above));
        assert!(!s.crossed(mid + Vec2::new(0.0, 8.0), mid - Vec2::new(0.0, 8.0)));
        assert!(!s.crossed(above + Vec2::new(0.0, 30.0), above + Vec2::new(2.0, 20.0)));
    }

    #[test]
    fn fire_chance_draws_only_below_one() {
        let b04 = EventTrigger::parse(
            "Spot<tA>event<tB>MusicEvent<eA>songName<eB>RW_1 - Urban Jungle<tA>fireChance<tB>0.4565217",
        )
        .unwrap();
        let mut rng = UnityRandom::new(5);
        let mut copy = rng.clone();
        let first = copy.value();
        let passed = ActiveTriggerChecker::roll(&b04, &mut rng);
        assert_eq!(passed, Some(first < 0.456_521_7));
        assert_eq!(rng.state(), copy.state());
        let a07 = EventTrigger::parse(A07).unwrap();
        assert_eq!(ActiveTriggerChecker::roll(&a07, &mut rng), Some(true));
        let none = EventTrigger::parse("Spot<tA>event<tB>NONE<tA>fireChance<tB>0.5").unwrap();
        assert_eq!(ActiveTriggerChecker::roll(&none, &mut rng), None);
        assert_eq!(rng.state(), copy.state());
    }
}
