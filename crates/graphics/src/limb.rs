//! `Limb`: a body part that hunts a target point at a capped speed, and the terrain
//! grip search (`FindGrip`).

use rw_assets::{Room, Slope, Terrain, tile_centre};
use rw_sim::{BodyChunk, Vec2};

use crate::body_part::{BodyPart, EIGHT_DIRECTIONS_AND_ZERO};
use crate::math::CustomVec;

/// `Limb.Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimbMode {
    /// Hunt `relative_hunt_pos`, rotated with the body and moved with the connection.
    HuntRelativePosition,
    HuntAbsolutePosition,
    /// Pulled into the connection chunk; a slugcat hand is then hidden.
    Retracted,
    Dangle,
}

/// Arguments of [`Limb::find_grip`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GripSearch {
    /// Where the limb is attached; grips farther than `max_radius` are ignored.
    pub attached: Vec2,
    /// The 3x3 tiles around this point are searched.
    pub search_from: Vec2,
    pub max_radius: f32,
    /// The grip nearest this point wins.
    pub goal: Vec2,
    /// Neighbour offsets with this x (or y) are skipped.
    pub forbidden_x: i32,
    pub forbidden_y: i32,
    /// Air tiles with a wall behind also count.
    pub behind_walls: bool,
}

/// One limb.
#[derive(Debug, Clone, PartialEq)]
pub struct Limb {
    pub part: BodyPart,
    pub mode: LimbMode,
    /// 0 or 1.
    pub limb_number: usize,
    default_hunt_speed: f32,
    pub hunt_speed: f32,
    default_quickness: f32,
    /// Lerp factor of the velocity toward the hunt direction.
    pub quickness: f32,
    pub relative_hunt_pos: Vec2,
    pub absolute_hunt_pos: Vec2,
    /// Within `hunt_speed` of the target this tick (or retracted).
    pub reached_snap_position: bool,
    /// Pulled back to the connection until it gets there (`retract`).
    pub retract: bool,
}

/// No grip found yet (the game's `(-100000, -100000)` sentinel).
const NO_GRIP: Vec2 = Vec2::new(-100_000.0, -100_000.0);

impl Limb {
    /// A limb in relative mode at the origin; place it with [`Limb::reset`].
    #[must_use]
    pub fn new(limb_number: usize, part: BodyPart, hunt_speed: f32, quickness: f32) -> Self {
        Self {
            part,
            mode: LimbMode::HuntRelativePosition,
            limb_number,
            default_hunt_speed: hunt_speed,
            hunt_speed,
            default_quickness: quickness,
            quickness,
            relative_hunt_pos: Vec2::ZERO,
            absolute_hunt_pos: Vec2::ZERO,
            reached_snap_position: false,
            retract: false,
        }
    }

    /// `Limb.Update`. `connection` is the chunk the limb hangs on, `rotation_chunk` the
    /// chunk that orients relative targets.
    pub fn update(&mut self, room: &Room, connection: &BodyChunk, rotation_chunk: Vec2) {
        self.part.last_pos = self.part.pos;
        if self.retract && self.mode != LimbMode::Retracted {
            self.mode = LimbMode::HuntAbsolutePosition;
            self.absolute_hunt_pos = connection.pos;
            if self.absolute_hunt_pos.distance(self.part.pos) < self.hunt_speed {
                self.mode = LimbMode::Retracted;
            }
        }
        if self.mode == LimbMode::HuntRelativePosition {
            self.absolute_hunt_pos = connection.pos
                + self
                    .relative_hunt_pos
                    .rotated(rotation_chunk.aim_to(connection.pos));
        }
        match self.mode {
            LimbMode::HuntRelativePosition | LimbMode::HuntAbsolutePosition => {
                if self.absolute_hunt_pos.distance(self.part.pos) < self.hunt_speed {
                    self.part.vel = self.absolute_hunt_pos - self.part.pos;
                    self.reached_snap_position = true;
                } else {
                    let want = self.part.pos.direction_to(self.absolute_hunt_pos) * self.hunt_speed;
                    self.part.vel = self.part.vel.lerp_clamped(want, self.quickness);
                    self.reached_snap_position = false;
                }
            }
            LimbMode::Retracted => {
                self.part.vel = connection.vel;
                self.part.pos = connection.pos;
                self.reached_snap_position = true;
            }
            LimbMode::Dangle => self.reached_snap_position = false,
        }
        self.quickness = self.default_quickness;
        self.hunt_speed = self.default_hunt_speed;
        if self.mode != LimbMode::Retracted {
            self.part.pos += self.part.vel;
            if self.mode == LimbMode::HuntRelativePosition {
                self.part.pos += connection.vel;
            }
            self.part.vel *= self.part.air_friction;
            self.part.push_out_of_terrain(room);
        }
    }

    /// `Limb.FindGrip`: among the 3x3 tiles around `search_from`, the terrain point
    /// nearest `goal` within `max_radius` of `attached` (Solid tiles at their free
    /// edges, Floor tops, slope lines, beam centre lines, optionally walls behind);
    /// if one is found the limb hunts it in absolute mode and true is returned
    /// (`GrabbedTerrain`). Without burrow terrain.
    pub fn find_grip(&mut self, room: &Room, s: GripSearch) -> bool {
        let mut search_from = s.search_from;
        let mut goal = s.goal;
        if s.attached.distance(search_from) >= s.max_radius {
            search_from = s.attached + s.attached.direction_to(search_from) * (s.max_radius - 1.0);
        }
        if s.attached.distance(goal) >= s.max_radius {
            goal = s.attached + s.attached.direction_to(goal) * s.max_radius;
        }
        let (sx, sy) = search_from.tile();
        let mut best = NO_GRIP;
        for (dx, dy) in EIGHT_DIRECTIONS_AND_ZERO {
            if dx == s.forbidden_x || dy == s.forbidden_y {
                continue;
            }
            let (tx, ty) = (sx + dx, sy + dy);
            let mid = Vec2::from(tile_centre(tx, ty));
            let mut cand = Vec2::new(
                goal.x.clamp(mid.x - 10.0, mid.x + 10.0),
                goal.y.clamp(mid.y - 10.0, mid.y + 10.0),
            );
            let tile = room.tile(tx, ty);
            match tile.terrain {
                Terrain::Solid => {
                    if dx != 0 && !room.tile(tx - dx, ty).is_solid() {
                        cand.x = mid.x - dx as f32 * 10.0;
                    }
                    if dy != 0 && !room.tile(tx, ty - dy).is_solid() {
                        cand.y = mid.y - dy as f32 * 10.0;
                    }
                    Self::consider(&mut best, cand, goal, &s);
                }
                Terrain::Floor => {
                    cand.y = mid.y + 10.0;
                    Self::consider(&mut best, cand, goal, &s);
                }
                Terrain::Slope => {
                    match room.identify_slope(tx, ty) {
                        Some(Slope::DownLeft | Slope::UpRight) => {
                            cand.y = mid.y + 10.0 - (cand.x - (mid.x - 10.0));
                        }
                        Some(Slope::DownRight | Slope::UpLeft) => {
                            cand.y = mid.y - 10.0 + (cand.x - (mid.x - 10.0));
                        }
                        None => {}
                    }
                    Self::consider(&mut best, cand, goal, &s);
                }
                Terrain::Air
                    if s.behind_walls
                        && tile.wall_behind()
                        && best == NO_GRIP
                        && s.attached.distance(cand) < s.max_radius =>
                {
                    best = cand;
                }
                _ => {}
            }
            if tile.horizontal_pole() {
                let on_beam = Vec2::new(goal.x.clamp(mid.x - 10.0, mid.x + 10.0), mid.y);
                Self::consider(&mut best, on_beam, goal, &s);
            }
            if tile.vertical_pole() {
                let on_beam = Vec2::new(mid.x, goal.y.clamp(mid.y - 10.0, mid.y + 10.0));
                Self::consider(&mut best, on_beam, goal, &s);
            }
        }
        if best.x != NO_GRIP.x && best.y != NO_GRIP.y {
            self.mode = LimbMode::HuntAbsolutePosition;
            self.absolute_hunt_pos = best;
            return true;
        }
        false
    }

    /// `OverLappingHuntPos`.
    #[must_use]
    pub fn overlapping_hunt_pos(&self) -> bool {
        self.reached_snap_position
            || self.part.pos.distance(self.absolute_hunt_pos) < self.part.rad + 1.0
    }

    /// `defaultHuntSpeed`.
    #[must_use]
    pub fn default_hunt_speed(&self) -> f32 {
        self.default_hunt_speed
    }

    /// Takes `cand` as the best grip if it is nearer `goal` and within reach.
    fn consider(best: &mut Vec2, cand: Vec2, goal: Vec2, s: &GripSearch) {
        let nearer = goal.dist_no_sqrt(cand) < goal.dist_no_sqrt(*best);
        if nearer && s.attached.distance(cand) < s.max_radius {
            *best = cand;
        }
    }
}
