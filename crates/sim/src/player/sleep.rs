//! Sleeping in a shelter: `Player.SleepUpdate` (88734), the hibernation checks of
//! `Player.Update` (87518, 87603) and the counters the HUD's food meter reads.

use rw_assets::Room;
use rw_random::UnityRandom;

use super::{Animation, HEAD, HIPS, Input, Player};
use crate::{FloatExt, Vec2};

/// What the shelter block of `Player.Update` reads of the room and the session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShelterCheck {
    /// In a story shelter with an intact door (`room.shelterDoor != null && !Broken`).
    pub in_shelter: bool,
    /// `FoodInRoom(room, false)`.
    pub food_in_room: i32,
    pub food_to_hibernate: i32,
    pub max_food: i32,
    /// `saveState.malnourished`.
    pub malnourished: bool,
    /// `rainCycle.timer > rainCycle.cycleLength`.
    pub rain_over: bool,
    /// The abstract position's tile is more than 6 tiles (Manhattan) from node 0's.
    pub far_from_entrance: bool,
    /// `IsTileInsideShelterRange` of the abstract position's tile.
    pub inside_range: bool,
}

/// The first player tick in a shelter (`sleepCounter` 100) lies the body down.
const WAKE_UP_START: i32 = 100;

impl Player {
    /// `Sleeping`.
    #[must_use]
    pub fn sleeping(&self) -> bool {
        self.sleep_counter != 0
    }

    #[must_use]
    pub fn sleep_counter(&self) -> i32 {
        self.sleep_counter
    }

    #[must_use]
    pub fn sleep_curl_up(&self) -> f32 {
        self.sleep_curl_up
    }

    #[must_use]
    pub fn sleep_when_still(&self) -> bool {
        self.sleep_when_still
    }

    /// The shelter door's 0.87 mark: fed players lie down once still.
    pub fn set_sleep_when_still(&mut self) {
        self.sleep_when_still = true;
    }

    #[must_use]
    pub fn force_sleep_counter(&self) -> i32 {
        self.force_sleep_counter
    }

    #[must_use]
    pub fn ready_for_win(&self) -> bool {
        self.ready_for_win
    }

    #[must_use]
    pub fn still_in_start_shelter(&self) -> bool {
        self.still_in_start_shelter
    }

    #[cfg(test)]
    pub(crate) fn leave_start_shelter(&mut self) {
        self.still_in_start_shelter = false;
    }

    #[must_use]
    pub fn touched_no_input_counter(&self) -> i32 {
        self.touched_no_input_counter
    }

    /// The constructor in a shelter with no creature but slugcats: wakes up lying down.
    pub fn start_asleep(&mut self) {
        self.sleep_counter = WAKE_UP_START;
    }

    /// `SleepUpdate`, run while conscious at the start of `Player.Update`. `raw` is the
    /// controller's input this tick (`RWInput.PlayerInput`), `tile` the abstract
    /// position's tile.
    pub fn sleep_update(
        &mut self,
        room: &Room,
        raw: Input,
        tile: (i32, i32),
        rng: &mut UnityRandom,
    ) {
        if !self.is_conscious() {
            return;
        }
        if self.sleep_counter == 0 {
            if self.force_sleep_counter > 0 {
                let to = (self.force_sleep_counter as f32).inverse_lerp(10.0, 210.0);
                self.sleep_curl_up = self.sleep_curl_up.lerp_and_tick(to, 0.04, 0.05);
            } else {
                self.sleep_curl_up = (self.sleep_curl_up - 0.1).max(0.0);
                if self.sleep_when_still
                    && self.touched_no_input_counter > 2
                    && self.body.chunks[HIPS].contact().y < 0
                {
                    self.sleep_counter = -1;
                }
            }
        } else if self.sleep_counter > 0 {
            self.wake_up(room, raw, tile, rng);
            if self.malnourished() {
                self.aerobic_level = 0.75 * (1.0 - self.sleep_curl_up);
            } else if self.sleep_counter == 0 && !self.standing {
                self.animation = Animation::StandUp;
                self.standing = true;
            }
        } else {
            self.sleep_counter -= 1;
            if self.sleep_counter == -6 && self.standing {
                self.animation = Animation::DownOnFours;
                self.standing = false;
            }
            self.sleep_curl_up = (self.sleep_counter as f32).inverse_lerp(-14.0, -24.0);
        }
    }

    /// The `sleepCounter > 0` branch: lie down at 100, wait at 99 for input (or lose the
    /// ground), then uncurl over 17 ticks (40 malnourished).
    fn wake_up(&mut self, room: &Room, raw: Input, tile: (i32, i32), rng: &mut UnityRandom) {
        let uncurl = if self.malnourished() { 40 } else { 17 };
        if self.sleep_counter == WAKE_UP_START {
            self.standing = false;
            let middle = Vec2::middle_of_tile(tile);
            let side = if rng.value() < 0.5 { -1.0 } else { 1.0 };
            let distance = self.body.connections[0].distance;
            for (i, sign) in [(HEAD, -0.5), (HIPS, 0.5)] {
                let c = &mut self.body.chunks[i];
                let rad = c.rad;
                c.hard_set_position(middle + Vec2::new(distance * sign * side, -10.0 + rad));
            }
            self.sleep_counter = WAKE_UP_START - 1;
        } else if self.sleep_counter == WAKE_UP_START - 1 {
            let pressed = raw.x != 0 || raw.y != 0 || raw.jump || raw.pickup || raw.throw;
            if !raw.map && pressed {
                self.sleep_counter = uncurl;
            }
            let grounded = self.is_tile_solid(room, HEAD, 0, -1)
                && self.is_tile_solid(room, HIPS, 0, -1)
                && self.body.chunks[HEAD].contact().y <= -1
                && self.body.chunks[HIPS].contact().y <= -1;
            if grounded {
                self.sleep_curl_up = 1.0;
            } else {
                self.sleep_curl_up -= 0.05;
                if self.sleep_curl_up < -0.2 {
                    self.sleep_curl_up = 0.0;
                    self.sleep_counter = 0;
                }
            }
        } else {
            self.sleep_counter -= 1;
            self.sleep_curl_up = (self.sleep_counter as f32).inverse_lerp(10.0, uncurl as f32);
        }
    }

    /// After the bat-fly catch: `touchedNoInputCounter` and `readyForWin = false`.
    pub fn count_untouched_input(&mut self) {
        let i = self.input[0];
        if i.x == 0 && i.y == 0 && !i.jump && !i.throw && !i.pickup {
            self.touched_no_input_counter += 1;
        } else {
            self.touched_no_input_counter = 0;
        }
        self.ready_for_win = false;
    }

    /// The shelter block of `Player.Update`: ready to win when fed enough (or the rain
    /// came), the forced sleep of a hungry slugcat holding down, then whether the door
    /// should close (`room.shelterDoor.Close()`).
    pub fn shelter_check(&mut self, room: &Room, cx: &ShelterCheck) -> bool {
        if !cx.in_shelter || self.dead || self.sleeping() {
            return false;
        }
        let needed = if cx.malnourished {
            cx.max_food
        } else {
            cx.food_to_hibernate
        };
        let i = self.input[0];
        if (!self.still_in_start_shelter && cx.food_in_room >= needed) || cx.rain_over {
            self.ready_for_win = true;
            self.force_sleep_counter = 0;
        } else if i.y < 0
            && !i.jump
            && !i.throw
            && !i.pickup
            && self.is_tile_solid(room, HIPS, 0, -1)
            && (i.x == 0
                || ((!self.is_tile_solid(room, HIPS, -1, -1)
                    || !self.is_tile_solid(room, HIPS, 1, -1))
                    && self.is_tile_solid(room, HIPS, i32::from(i.x), 0)))
        {
            let starving =
                !cx.malnourished && cx.food_in_room > 0 && cx.food_in_room < cx.food_to_hibernate;
            if starving {
                self.force_sleep_counter += 1;
            } else {
                self.force_sleep_counter = 0;
            }
        } else {
            self.force_sleep_counter = 0;
        }
        if self.is_stunned() {
            self.ready_for_win = false;
        }
        if !(cx.far_from_entrance && cx.inside_range) {
            return false;
        }
        if self.ready_for_win && self.touched_no_input_counter > 20 {
            true
        } else if self.force_sleep_counter > 260 {
            self.sleep_counter = -24;
            true
        } else {
            false
        }
    }
}
