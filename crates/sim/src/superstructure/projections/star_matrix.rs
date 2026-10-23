//! `StarMatrix` (the placed object `ProjectedStars`, 4 superstructure rooms): a cloud of
//! stars in a unit sphere, joined to near neighbours, turned and refocused at random and
//! projected on the plane. The game's side is `superstructure/projections/turn.rs`, the sprites
//! `rw-graphics`' `superstructure/projections/star_matrix.rs`.
//!
//! The z rotation never moves: the game adds `zRotatVelVel` to itself where it means
//! `zRotatVel` (kept).
//!
//! Mirrors: StarMatrix.cs

use rw_random::UnityRandom;

use crate::{FloatExt, Vec2, Vec3};

/// `Mathf.Cos` and `Mathf.Sin` of an angle in turns (times 2 pi).
trait Turns {
    fn cos_sin(self) -> (f32, f32);
}

impl Turns for f32 {
    fn cos_sin(self) -> (f32, f32) {
        let a = f64::from(self * std::f32::consts::PI * 2.0);
        (a.cos() as f32, a.sin() as f32)
    }
}

/// `StarMatrix.Star`.
#[derive(Debug, Clone, PartialEq)]
pub struct Star {
    pub index: usize,
    pub first_sprite: usize,
    pub total_sprites: usize,
    pub position: Vec3,
    pub connections: Vec<usize>,
    /// `circles`: the rings round the star.
    pub circles: usize,
    pub connections_on: Vec<bool>,
}

impl Star {
    /// The constructor: the position (3 draws), the rings (1 to 3), the number of
    /// connections wanted for a star past the tenth, up to a hundred tries per wanted
    /// connection among the earlier stars, and each connection's switch.
    fn new(stars: &[Star], index: usize, first_sprite: usize, rng: &mut UnityRandom) -> Self {
        let [x, y, z] = rng.inside_unit_sphere();
        let position = Vec3::new(x, y, z);
        let circles = if rng.value() < 0.5 {
            0
        } else {
            let top = rng.range_i32(1, 4);
            rng.range_i32(0, top) as usize
        };
        let wanted = if index > 10 { rng.range_i32(1, 3) } else { 0 };
        let mut connections: Vec<usize> = Vec::new();
        for _ in 0..wanted * 100 {
            let other = rng.range_i32(0, index as i32) as usize;
            if stars[other].position.distance(position) < 0.4 && !connections.contains(&other) {
                connections.push(other);
            }
            if connections.len() as i32 >= wanted {
                break;
            }
        }
        let connections_on: Vec<bool> = connections.iter().map(|_| rng.value() < 0.5).collect();
        Self {
            index,
            first_sprite,
            total_sprites: 1 + connections.len() + circles,
            position,
            connections,
            circles,
            connections_on,
        }
    }

    /// `Update`: now and then a connection switches (2 to 3 draws).
    fn update(&mut self, rng: &mut UnityRandom) {
        if rng.value() < 0.05 && !self.connections.is_empty() {
            let at = rng.range_i32(0, self.connections.len() as i32) as usize;
            self.connections_on[at] = rng.value() < 0.5;
        }
    }
}

/// `StarMatrix`.
#[derive(Debug, Clone, PartialEq)]
pub struct StarMatrix {
    pub update_order: u64,
    pub pos: Vec2,
    pub rad: f32,
    pub stars: Vec<Star>,
    pub total_sprites: usize,
    pub last_x_rotat: f32,
    pub x_rotat: f32,
    pub x_rotat_vel: f32,
    pub x_rotat_vel_vel: f32,
    pub last_y_rotat: f32,
    pub y_rotat: f32,
    pub y_rotat_vel: f32,
    pub y_rotat_vel_vel: f32,
    pub last_z_rotat: f32,
    pub z_rotat: f32,
    pub z_rotat_vel: f32,
    pub z_rotat_vel_vel: f32,
    pub focus: Vec3,
    pub last_focus: Vec3,
    pub get_to_focus: Vec3,
}

impl StarMatrix {
    /// The constructor for a placed object at `pos` whose handle makes `placed_rad`
    /// (`Rad`); the stars' draws come in index order.
    pub fn new(update_order: u64, pos: Vec2, placed_rad: f32, rng: &mut UnityRandom) -> Self {
        let rad = placed_rad + 50.0;
        let area = 0.0f32.max(rad.lerp(500.0, 0.3)).mathf_pow(2.0) * std::f32::consts::PI;
        let count = (area / 4000.0).to_int_cs().max(0) as usize;
        let mut stars: Vec<Star> = Vec::with_capacity(count);
        let mut total_sprites = 0;
        for index in 0..count {
            let star = Star::new(&stars, index, total_sprites, rng);
            total_sprites += star.total_sprites;
            stars.push(star);
        }
        Self {
            update_order,
            pos,
            rad,
            stars,
            total_sprites,
            last_x_rotat: 0.0,
            x_rotat: 0.0,
            x_rotat_vel: 0.0,
            x_rotat_vel_vel: 0.0,
            last_y_rotat: 0.0,
            y_rotat: 0.0,
            y_rotat_vel: 0.0,
            y_rotat_vel_vel: 0.0,
            last_z_rotat: 0.0,
            z_rotat: 0.0,
            z_rotat_vel: 0.0,
            z_rotat_vel_vel: 0.0,
            focus: Vec3::default(),
            last_focus: Vec3::default(),
            get_to_focus: Vec3::default(),
        }
    }

    /// A random acceleration of a rotation: the roll (1 draw), then the sign and the
    /// size (2 draws) when it hits.
    fn random_kick(rng: &mut UnityRandom) -> Option<f32> {
        if rng.value() < 0.05 {
            let sign = (-1.0f32).lerp(1.0, rng.value());
            Some(sign * rng.value().mathf_pow(6.0) * 0.005)
        } else {
            None
        }
    }

    /// `Update`.
    pub fn update(&mut self, rng: &mut UnityRandom) {
        self.last_focus = self.focus;
        self.focus = self
            .focus
            .move_towards(self.get_to_focus, 0.002)
            .lerp(self.get_to_focus, 0.007);
        if rng.value() < 0.025 && !self.stars.is_empty() {
            let num = rng.range_i32(0, self.stars.len() as i32) as usize;
            if self.stars[num].position.length() < 0.6 {
                let [x, y, z] = rng.inside_unit_sphere();
                self.get_to_focus = self.stars[num].position + Vec3::new(x, y, z).scale(0.05);
            }
        }
        self.last_x_rotat = self.x_rotat;
        self.last_y_rotat = self.y_rotat;
        self.last_z_rotat = self.z_rotat;
        self.x_rotat += self.x_rotat_vel;
        self.y_rotat += self.y_rotat_vel;
        self.z_rotat += self.z_rotat_vel;
        self.x_rotat_vel = self.x_rotat_vel.clamp(-0.1, 0.1) * 0.9;
        self.y_rotat_vel = self.y_rotat_vel.clamp(-0.1, 0.1) * 0.9;
        self.z_rotat_vel = self.z_rotat_vel.clamp(-0.1, 0.1) * 0.9;
        self.x_rotat_vel += self.x_rotat_vel_vel;
        self.y_rotat_vel += self.y_rotat_vel_vel;
        self.z_rotat_vel_vel += self.z_rotat_vel_vel;
        self.x_rotat_vel_vel = self.x_rotat_vel_vel.clamp(-0.01, 0.01) * 0.9;
        self.y_rotat_vel_vel = self.y_rotat_vel_vel.clamp(-0.01, 0.01) * 0.9;
        self.z_rotat_vel_vel = self.z_rotat_vel_vel.clamp(-0.01, 0.01) * 0.9;
        if let Some(kick) = Self::random_kick(rng) {
            self.x_rotat_vel_vel += kick;
        }
        if let Some(kick) = Self::random_kick(rng) {
            self.y_rotat_vel_vel += kick;
        }
        if let Some(kick) = Self::random_kick(rng) {
            self.z_rotat_vel_vel += kick;
        }
        for star in &mut self.stars {
            star.update(rng);
        }
    }

    /// `Star.OnPlanePos` of star `index` at `time_stacker` between ticks.
    #[must_use]
    pub fn on_plane_pos(&self, index: usize, time_stacker: f32) -> Vec2 {
        let star = &self.stars[index];
        let (cx, sx) = self.last_x_rotat.lerp(self.x_rotat, time_stacker).cos_sin();
        let (cy, sy) = self.last_y_rotat.lerp(self.y_rotat, time_stacker).cos_sin();
        let (cz, sz) = self.last_z_rotat.lerp(self.z_rotat, time_stacker).cos_sin();
        let focus = self.last_focus.lerp(self.focus, time_stacker);
        let (a, b, c) = (
            star.position.x - focus.x,
            star.position.y - focus.y,
            star.position.z - focus.z,
        );
        let (b, c) = (b * cx - c * sx, b * sx + c * cx);
        // The y turn's new z is never read again.
        let a = c * sy + a * cy;
        Vec2::new(a * cz - b * sz, a * sz + b * cz)
    }
}

#[cfg(test)]
#[path = "tests/star_matrix.rs"]
mod tests;
