//! Room settings triggers in a realized room: `ActiveTriggerChecker` (made by every
//! `Room.Loaded` for the triggers the save lets through), the room objects their events
//! add (`PickupObjectInstruction`, `RoomSpecificTextMessage`), and the song requests the
//! sim hands the music player (`rw-audio`). Behaviour: `rainworld-re/notes/music.md`
//! section 4.2; the game side is `game_triggers.rs`.

use rw_assets::{EventTrigger, MusicEvent, StopMusicEvent, TriggerType};
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

/// `PickupObjectInstruction` (an `InputInstructionTrigger`): points the guide at the
/// nearest free rock or spear until the player has carried one.
#[derive(Debug, Clone, PartialEq)]
pub struct PickupObjectInstruction {
    pub instruction_pos: Vec2,
    pub activated: bool,
    pub completed: bool,
    pub time_since_activated: i32,
    pub item: Option<ObjectId>,
    pub player_has_picked_up_an_item: bool,
    pub mark_instruction_as_followed: bool,
}

impl PickupObjectInstruction {
    #[must_use]
    pub fn new() -> Self {
        Self {
            instruction_pos: Vec2::ZERO,
            activated: false,
            completed: false,
            time_since_activated: 0,
            item: None,
            player_has_picked_up_an_item: false,
            mark_instruction_as_followed: false,
        }
    }
}

impl Default for PickupObjectInstruction {
    fn default() -> Self {
        Self::new()
    }
}

/// What a trigger room object is.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerObjectKind {
    Checker(ActiveTriggerChecker),
    PickupInstruction(PickupObjectInstruction),
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
    pub completed: bool,
    pub slated: bool,
    pub instruction_pos: Vec2,
    pub player_has_picked_up_an_item: bool,
    pub mark_instruction_as_followed: bool,
}

impl TriggerObject {
    /// The view of an input instruction object.
    #[must_use]
    pub fn instruction_view(&self) -> Option<InputInstructionView> {
        let TriggerObjectKind::PickupInstruction(p) = &self.kind else {
            return None;
        };
        Some(InputInstructionView {
            id: self.id,
            room: self.room,
            completed: p.completed,
            slated: self.slated,
            instruction_pos: p.instruction_pos,
            player_has_picked_up_an_item: p.player_has_picked_up_an_item,
            mark_instruction_as_followed: p.mark_instruction_as_followed,
        })
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
