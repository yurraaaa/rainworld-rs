//! `ShelterDoor` (262186) for an intact, non-ancient shelter, base game, Remix off: the
//! door that opens at cycle start, closes when the slugcat sleeps and ends the cycle.
//! Its graphics are `rw_graphics::ShelterDoorSprites`. Left out: the ancient (More
//! Slugcats) doors, broken shelters, `PrecycleModule`, the tutorials and dream ghost,
//! `GetStuffInWalls` and `ScreenMovement`.
//!
//! Mirrors: ShelterDoor.cs

use rw_assets::shortcut::{FOUR_DIRECTIONS, ShortcutType};
use rw_assets::{ActiveMods, PlacedObjectType, Room, RoomSettings, Terrain};
use rw_random::UnityRandom;

use crate::room_ext::RoomExt;
use crate::{FloatExt, SoundId, Sounds, StaticSoundLoop, Vec2};

/// How a story cycle ended, for the process the game switches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEnd {
    /// `RainWorldGame.Win(malnourished)`: the shelter closed on a living player.
    Win { malnourished: bool },
    /// `GoToStarveScreen`.
    Starve,
    /// `GoToDeathScreen`.
    Death,
}

/// `ShelterDoor`: the door's state in a realized shelter.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelterDoor {
    /// Global index of its room.
    pub room: i32,
    pub closed_fac: f32,
    /// Below 0 opening (or open); `0.003125` once `Close` ran.
    pub close_speed: f32,
    /// Per part: `[current, last, delay]`.
    pub segment_pairs: [[f32; 3]; 5],
    pub pistons: [[f32; 3]; 2],
    pub covers: [[f32; 3]; 4],
    pub pumps: [[f32; 3]; 8],
    /// `pZero`: 60 px from the entrance into the room, where the parts meet.
    pub p_zero: Vec2,
    pub dir: Vec2,
    pub perp: Vec2,
    pub close_tiles: [(i32, i32); 4],
    pub player_spawn_pos: (i32, i32),
    last_closed: bool,
    pub working_loop: StaticSoundLoop,
    pub gasket_mover_loop: StaticSoundLoop,
    pub kill_hostiles: bool,
    pub open_time: i32,
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `aiMapper`: the room's AI map built again once the door has opened.
    pub ai_mapper: Option<crate::world::AiMapper>,
}

/// What one `ShelterDoor.Update` asks of the room and the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DoorStep {
    /// `openTime < 400`: stun the creatures that are not the slugcat's friends.
    pub stun_creatures: bool,
    /// `DoorClosed` ran.
    pub closed: bool,
    /// `closedFac` passed 0.87 while closing: fed players fall asleep when still.
    pub sleep_when_still: bool,
    /// `closedFac` passed 0.4 while closing: players without food starve.
    pub starve_check: bool,
    /// The close tiles become Solid (`Some(true)`) or Air.
    pub close_tiles: Option<bool>,
}

impl ShelterDoor {
    /// `closedFac` reached 0 while opening: a new `AImapper` starts.
    fn start_ai_mapper(&mut self, last: f32) {
        if self.close_speed < 0.0 && last > 0.0 && self.closed_fac == 0.0 {
            self.ai_mapper = Some(crate::world::AiMapper::default());
        }
    }
}

impl ShelterDoor {
    const OPEN_UP_TICKS: f32 = 350.0;
    const INITIAL_WAIT: f32 = 80.0;
    /// `closeSpeed` set by `Close`.
    pub const CLOSE_SPEED: f32 = 0.003_125;

    /// `new ShelterDoor(room)` as `Room.Loaded` makes it: closed while the cycle's timer
    /// is under `initialWait` (`cycleStartUp`), opening over `openUpTicks`; then
    /// `Reset`'s 19 draws. `slugcat_in_room`: the room's creatures include a slugcat
    /// (`killHostiles`).
    #[must_use]
    pub fn new(
        index: i32,
        room: &Room,
        settings: &RoomSettings,
        mods: ActiveMods,
        rain_timer: i32,
        slugcat_in_room: bool,
        rng: &mut UnityRandom,
    ) -> Self {
        let closed_fac = (rain_timer as f32)
            .inverse_lerp(Self::INITIAL_WAIT + Self::OPEN_UP_TICKS, Self::INITIAL_WAIT);
        let mut door = Self {
            room: index,
            closed_fac,
            close_speed: -1.0,
            segment_pairs: [[closed_fac, closed_fac, 0.0]; 5],
            pistons: [[closed_fac, closed_fac, 0.0]; 2],
            covers: [[closed_fac, closed_fac, 0.0]; 4],
            pumps: [[closed_fac, closed_fac, 0.0]; 8],
            p_zero: Vec2::ZERO,
            dir: Vec2::new(0.0, -1.0),
            perp: Vec2::ZERO,
            close_tiles: [(0, 0); 4],
            player_spawn_pos: (0, 0),
            last_closed: false,
            working_loop: StaticSoundLoop::new(
                SoundId::ShelterWorkingBackgroundLoop,
                Vec2::ZERO,
                0.0,
                1.0,
            ),
            gasket_mover_loop: StaticSoundLoop::new(
                SoundId::ShelterGasketMoverLoop,
                Vec2::ZERO,
                0.0,
                1.0,
            ),
            kill_hostiles: slugcat_in_room,
            open_time: 0,
            update_order: 0,
            ai_mapper: None,
        };
        door.find_entrance(room);
        if room.name().eq_ignore_ascii_case("sl_scrushed") {
            // `ShelterEntranceOverrides`.
            door.dir = Vec2::new(1.0, 0.0);
            let tile = (22, 15);
            door.p_zero = Vec2::middle_of_tile(tile) + door.dir * 60.0;
            door.set_close_tiles(tile, (1, 0));
        }
        if let Some(o) = Self::spawn_point(settings, mods) {
            door.player_spawn_pos = o;
        }
        door.perp = door.dir.perpendicular();
        door.working_loop.pos = door.p_zero;
        door.gasket_mover_loop.pos = door.p_zero;
        door.reset(rng);
        door
    }

    fn spawn_point(settings: &RoomSettings, mods: ActiveMods) -> Option<(i32, i32)> {
        settings
            .placed_objects
            .iter()
            .find(|o| o.object_type(mods) == Some(PlacedObjectType::ShelterSpawnPoint))
            .map(|o| rw_assets::tile_position(o.pos))
    }

    /// The constructor's scan: in each column, the first room-exit entrance sets
    /// `pZero`, `dir`, the close tiles and `playerSpawnPos` (the last column's wins).
    fn find_entrance(&mut self, room: &Room) {
        for l in 0..room.width() {
            for m in 0..room.height() {
                if room.tile(l, m).terrain != Terrain::ShortcutEntrance
                    || room.shortcut_map().type_at((l, m)) != ShortcutType::RoomExit
                {
                    continue;
                }
                self.p_zero = Vec2::middle_of_tile((l, m));
                self.dir = Vec2::new(0.0, -1.0);
                if let Some(&(dx, dy)) = FOUR_DIRECTIONS
                    .iter()
                    .find(|(dx, dy)| room.tile(l + dx, m + dy).terrain != Terrain::Solid)
                {
                    self.dir = Vec2::new(dx as f32, dy as f32);
                    self.set_close_tiles((l, m), (dx, dy));
                    let mut p = (l, m);
                    while !room.is_solid_tile(p.0 + dx, p.1 + dy) {
                        p = (p.0 + dx, p.1 + dy);
                    }
                    if dy == 1 {
                        while !room.is_solid_tile(p.0 - 1, p.1) {
                            p.0 -= 1;
                        }
                    }
                    while !room.is_solid_tile(p.0, p.1 - 1) {
                        p.1 -= 1;
                    }
                    self.player_spawn_pos = p;
                }
                self.p_zero += self.dir * 60.0;
                break;
            }
        }
    }

    fn set_close_tiles(&mut self, entrance: (i32, i32), (dx, dy): (i32, i32)) {
        for (n, t) in self.close_tiles.iter_mut().enumerate() {
            let k = n as i32 + 2;
            *t = (entrance.0 + dx * k, entrance.1 + dy * k);
        }
    }

    /// The constructor's `playerSpawnPos`: from the last column's room exit, out along
    /// its first open side to the wall, (when that side is up) left to the wall, then
    /// down to the floor; (20, 13) in an ancient shelter; a `ShelterSpawnPoint` object
    /// overrides either.
    #[must_use]
    pub fn player_spawn_pos(
        room: &Room,
        settings: &RoomSettings,
        ancient: bool,
        mods: ActiveMods,
    ) -> (i32, i32) {
        let mut spawn = if ancient {
            (20, 13)
        } else {
            let mut door = Self::layout_only();
            door.find_entrance(room);
            door.player_spawn_pos
        };
        if let Some(o) = Self::spawn_point(settings, mods) {
            spawn = o;
        }
        spawn
    }

    /// A door with only its layout fields, for [`Self::player_spawn_pos`].
    fn layout_only() -> Self {
        Self {
            room: -1,
            closed_fac: 0.0,
            close_speed: -1.0,
            segment_pairs: [[0.0; 3]; 5],
            pistons: [[0.0; 3]; 2],
            covers: [[0.0; 3]; 4],
            pumps: [[0.0; 3]; 8],
            p_zero: Vec2::ZERO,
            dir: Vec2::new(0.0, -1.0),
            perp: Vec2::ZERO,
            close_tiles: [(0, 0); 4],
            player_spawn_pos: (0, 0),
            last_closed: false,
            working_loop: StaticSoundLoop::new(
                SoundId::ShelterWorkingBackgroundLoop,
                Vec2::ZERO,
                0.0,
                1.0,
            ),
            gasket_mover_loop: StaticSoundLoop::new(
                SoundId::ShelterGasketMoverLoop,
                Vec2::ZERO,
                0.0,
                1.0,
            ),
            kill_hostiles: false,
            open_time: 0,
            update_order: 0,
            ai_mapper: None,
        }
    }

    /// `Reset`: new random delays for every part (5 + 2 + 4 + 8 draws).
    fn reset(&mut self, rng: &mut UnityRandom) {
        for s in &mut self.segment_pairs {
            s[2] = rng.value() * 0.5;
        }
        for p in &mut self.pistons {
            p[2] = rng.value() * 0.8;
        }
        for c in &mut self.covers {
            c[2] = rng.value() * 0.5;
        }
        for p in &mut self.pumps {
            p[2] = rng.value() * 0.5;
        }
    }

    /// `IsClosing`.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        self.close_speed > 0.0
    }

    /// `IsOpening`: not fully open (`closedFac != 0`).
    #[must_use]
    pub fn is_opening(&self) -> bool {
        self.closed_fac != 0.0
    }

    /// `Close`: start closing (the More Slugcats and Jolly Co-op checks left out).
    pub fn close(&mut self) {
        self.close_speed = Self::CLOSE_SPEED;
    }

    /// `Closed` (an intact shelter's): `closedFac` in 0..1.
    #[must_use]
    pub fn closed(&self) -> f32 {
        self.closed_fac.clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn flaps_open(&self) -> f32 {
        self.closed().inverse_lerp(0.04, 0.0)
    }

    #[must_use]
    pub fn pistons_closed(&self) -> f32 {
        self.closed().inverse_lerp(0.2, 0.1)
    }

    #[must_use]
    pub fn segments(&self) -> f32 {
        self.closed().inverse_lerp(0.2, 0.38)
    }

    #[must_use]
    pub fn pistons_fac(&self) -> f32 {
        self.closed().inverse_lerp(0.38, 0.41)
    }

    #[must_use]
    pub fn covers_fac(&self) -> f32 {
        self.closed().inverse_lerp(0.41, 0.51)
    }

    #[must_use]
    pub fn cylinders(&self) -> f32 {
        self.closed().inverse_lerp(0.53, 0.61)
    }

    #[must_use]
    pub fn pumps_enter(&self) -> f32 {
        self.closed().inverse_lerp(0.59, 0.7)
    }

    #[must_use]
    pub fn pumps_exit(&self) -> f32 {
        self.closed().inverse_lerp(0.75, 1.0)
    }

    /// `Custom.Decimal`.
    fn decimal(f: f32) -> f32 {
        f - f.floor()
    }

    /// The let-go loop emitters' updates. They joined the room after the door (added in
    /// `Room.Loaded`, first in the update list), so `Room.Update`'s backward walk runs
    /// them before it.
    pub fn update_loop_emitters(&mut self, sounds: &mut Sounds) {
        self.working_loop.update_released(sounds);
        self.gasket_mover_loop.update_released(sounds);
    }

    /// `Update` from `openTime` on: the timer-driven opening at cycle start (or the
    /// close speed), the bolt, hatch, segment, piston, cover and gasket sounds and loops,
    /// and the parts' progress. `rain_timer` is `rainCycle.timer`.
    pub fn update(
        &mut self,
        rain_timer: i32,
        sounds: &mut Sounds,
        rng: &mut UnityRandom,
    ) -> DoorStep {
        let mut step = DoorStep::default();
        if self.closed() == 0.0 {
            self.open_time += 1;
        } else {
            self.open_time = 0;
        }
        step.stun_creatures = self.open_time < 400;
        let last = self.closed_fac;
        let timer = rain_timer as f32;
        if timer < Self::OPEN_UP_TICKS + Self::INITIAL_WAIT {
            self.closed_fac =
                timer.inverse_lerp(Self::INITIAL_WAIT + Self::OPEN_UP_TICKS, Self::INITIAL_WAIT);
        } else {
            self.closed_fac = (self.closed_fac + self.close_speed).clamp(0.0, 1.0);
            if self.closed_fac == 1.0 && self.close_speed > 0.0 {
                step.closed = true;
            }
        }
        self.start_ai_mapper(last);
        let p_zero = self.p_zero;
        let last_cylinders = last.inverse_lerp(0.53, 0.61);
        if self.close_speed < 0.0
            && Self::decimal(last_cylinders * 4.0) < Self::decimal(self.cylinders() * 4.0)
        {
            sounds.play_at(SoundId::ShelterBoltOpen, p_zero, 1.0, 1.0, rng);
        } else if self.close_speed > 0.0
            && Self::decimal(last_cylinders * 4.0) > Self::decimal(self.cylinders() * 4.0)
        {
            sounds.play_at(SoundId::ShelterBoltClose, p_zero, 1.0, 1.0, rng);
        }
        if self.close_speed > 0.0 && self.closed_fac > 0.87 && last <= 0.87 {
            step.sleep_when_still = true;
        }
        if self.close_speed > 0.0 && self.closed_fac > 0.4 && last <= 0.4 {
            step.starve_check = true;
        }
        self.working_loop.update(sounds, rng);
        let closed = self.closed();
        self.working_loop.volume = if closed > 0.1 && closed < 1.0 {
            self.working_loop.volume.lerp(1.0, 0.1)
        } else {
            (self.working_loop.volume - 0.05).max(0.0)
        };
        if (last >= 0.04 && self.closed_fac < 0.04) || (last < 0.04 && self.closed_fac >= 0.04) {
            sounds.play_at(SoundId::ShelterLittleHatchOpen, p_zero, 1.0, 1.0, rng);
        }
        self.update_parts(sounds, rng);
        self.gasket_mover_loop.update(sounds, rng);
        let (enter, exit) = (self.pumps_enter(), self.pumps_exit());
        if (enter > 0.0 && enter < 1.0) || (exit > 0.0 && exit < 1.0) {
            let l = &mut self.gasket_mover_loop;
            l.volume = l.volume.lerp(1.0, 0.5);
            let to = if enter > 0.0 && enter < 1.0 { 1.2 } else { 0.8 };
            l.pitch = l.pitch.lerp(to, 0.2);
        } else {
            let l = &mut self.gasket_mover_loop;
            l.volume = (l.volume - 0.05).max(0.0);
        }
        let now_closed = self.closed() > 0.0;
        if now_closed != self.last_closed {
            self.reset(rng);
            step.close_tiles = Some(now_closed);
        }
        self.last_closed = now_closed;
        step
    }

    /// The segment pairs, pistons, covers and pumps follow their phases, with their
    /// sounds.
    fn update_parts(&mut self, sounds: &mut Sounds, rng: &mut UnityRandom) {
        let p = self.p_zero;
        let segments = self.segments();
        for s in &mut self.segment_pairs {
            s[1] = s[0];
            s[0] = segments.inverse_lerp(s[2], s[2] + 0.5);
            if s[0] == 1.0 && s[1] < 1.0 {
                sounds.play_at(SoundId::ShelterSegmentPairCollide, p, 1.0, 1.0, rng);
            }
            if s[1] <= 0.1 && s[0] > 0.1 {
                sounds.play_at(SoundId::ShelterSegmentPairMoveIn, p, 1.0, 1.0, rng);
            } else if s[1] == 1.0 && s[0] < 1.0 {
                sounds.play_at(SoundId::ShelterSegmentPairMoveOut, p, 1.0, 1.0, rng);
            }
        }
        let pistons = self.pistons_fac().max(self.pistons_closed());
        let pistons_closed = self.pistons_closed();
        for s in &mut self.pistons {
            s[1] = s[0];
            s[0] = pistons.inverse_lerp(s[2], 1.0);
            if s[0] >= 0.95 && s[1] < 0.95 {
                let id = if pistons_closed < 0.5 {
                    SoundId::ShelterPistonInHard
                } else {
                    SoundId::ShelterPistonInSoft
                };
                sounds.play_at(id, p, 1.0, 1.0, rng);
            } else if s[0] < 1.0 && s[1] == 1.0 {
                sounds.play_at(SoundId::ShelterPistonOut, p, 1.0, 1.0, rng);
            }
        }
        let covers = self.covers_fac();
        for s in &mut self.covers {
            s[1] = s[0];
            s[0] = covers.inverse_lerp(s[2], s[2] + 0.5);
            let id = if s[1] == 1.0 && s[0] < 1.0 {
                SoundId::ShelterProtectiveCoverMoveOut
            } else if s[1] == 0.0 && s[0] > 0.0 {
                SoundId::ShelterProtectiveCoverMoveIn
            } else if s[1] < 1.0 && s[0] == 1.0 {
                SoundId::ShelterProtectiveCoverClickIntoPlace
            } else {
                continue;
            };
            sounds.play_at(id, p, 1.0, 1.0, rng);
        }
        let enter = self.pumps_enter();
        for s in &mut self.pumps {
            s[1] = s[0];
            s[0] = enter.inverse_lerp(s[2], 1.0);
        }
        if self.pumps[0][0] == 1.0 && self.pumps[0][1] < 1.0 {
            sounds.play_at(SoundId::ShelterGasketsSeal, p, 1.0, 1.0, rng);
        } else if self.pumps[0][0] < 1.0 && self.pumps[0][1] == 1.0 {
            sounds.play_at(SoundId::ShelterGasketsUnseal, p, 1.0, 1.0, rng);
        }
    }

    /// `IsTileInsideShelterRange` / `CoordInsideShelterRange`: anywhere in a regular
    /// shelter; in an ancient one x 14..=29 above y 9.
    #[must_use]
    pub fn coord_inside_shelter_range(tile: (i32, i32), ancient: bool) -> bool {
        if ancient {
            tile.0 > 13 && tile.0 < 33 && tile.1 > 9 && tile.0 < 30
        } else {
            true
        }
    }

    /// `IsThisBigCreatureForShelter` (base game).
    #[must_use]
    pub fn big_for_shelter(kind: rw_assets::CreatureType) -> bool {
        use rw_assets::CreatureType as T;
        matches!(
            kind,
            T::Deer
                | T::BrotherLongLegs
                | T::DaddyLongLegs
                | T::RedCentipede
                | T::MirosBird
                | T::PoleMimic
                | T::TentaclePlant
                | T::Vulture
                | T::KingVulture
        )
    }

    /// `IsThisHostileCreatureForShelter` (base game).
    #[must_use]
    pub fn hostile_for_shelter(kind: rw_assets::CreatureType) -> bool {
        use rw_assets::CreatureType as T;
        matches!(
            kind,
            T::Deer
                | T::BigNeedleWorm
                | T::RedCentipede
                | T::Centipede
                | T::Centiwing
                | T::BigSpider
                | T::BrotherLongLegs
                | T::DaddyLongLegs
                | T::MirosBird
                | T::PoleMimic
                | T::TentaclePlant
                | T::Vulture
                | T::KingVulture
                | T::Spider
                | T::SpitterSpider
                | T::DropBug
        )
    }
}

#[cfg(test)]
#[path = "shelter_tests.rs"]
mod tests;
