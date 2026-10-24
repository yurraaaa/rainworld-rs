//! `FlyGraphics`: the bat fly's lower body, wing beats and its
//! four sprites. Its `Update` draws from the game's shared random stream; `DrawSprites`
//! makes no draws.
//!
//! Mirrors: FlyGraphics.cs

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::creature::fly::{FlyBehavior, FlyView, MovementMode};
use rw_sim::{CameraView, FloatExt, Vec2};

use crate::body::body_part::{BodyPart, PointConnection};
use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::core::math::VecToDeg;

/// `GraphicsModule.cullRange`.
const CULL_RANGE: f32 = 200.0;

/// A fly's graphics module.
#[derive(Debug, Clone, PartialEq)]
pub struct FlyGraphics {
    pub id: rw_sim::creature::EntityId,
    pub culled: bool,
    pub last_culled: bool,
    /// `lowerBody`: a `GenericBodyPart` (rad 5, surface friction 0.5, air friction 1)
    /// on the fly's chunk.
    pub lower_body: BodyPart,
    /// `wings[i, 0]` (now) and `wings[i, 1]` (last).
    pub wings: [[f32; 2]; 2],
    pub horizontal_steering_compensation: f32,
    pub last_horizontal_steering_compensation: f32,
    pub insect_flight_wing_sync: f32,
    pub death_wing_positions: [f32; 2],
}

impl FlyGraphics {
    /// The constructor: the lower body's `Reset` on the chunk (a draw), the wing sync and
    /// the two death wing positions.
    #[must_use]
    pub fn new(view: &FlyView<'_>, rng: &mut UnityRandom) -> Self {
        let mut lower_body = BodyPart::new(5.0, 0.5, 1.0);
        lower_body.reset(view.fly.pos(), rng);
        let insect_flight_wing_sync = rng.value();
        let death_wing_positions = [rng.value(), rng.value()];
        Self {
            id: view.id,
            culled: true,
            last_culled: true,
            lower_body,
            wings: [[0.0; 2]; 2],
            horizontal_steering_compensation: 0.0,
            last_horizontal_steering_compensation: 0.0,
            insect_flight_wing_sync,
            death_wing_positions,
        }
    }

    /// `Reset`: the lower body takes the chunk's position and velocity (no body parts are
    /// registered, so no draws).
    pub fn reset(&mut self, view: &FlyView<'_>) {
        let c = view.fly.chunk();
        self.lower_body.pos = c.pos;
        self.lower_body.last_pos = c.last_pos;
        self.lower_body.vel = c.vel;
    }

    /// `GraphicsModule.ShouldBeCulled`: `room` is the fly's room (its camera count),
    /// `camera_room` the room camera 0 shows.
    #[must_use]
    pub fn should_be_culled(
        &self,
        view: &FlyView<'_>,
        cameras: usize,
        camera_room: &Room,
        camera: &CameraView,
    ) -> bool {
        let pos = view.fly.pos();
        let margin = CULL_RANGE + if self.culled { 0.0 } else { 100.0 };
        if cameras > 2 && !camera.position_currently_visible(camera_room, pos, margin, true) {
            let next = if self.culled { 50.0 } else { 100.0 };
            return !camera.position_visible_in_next_screen(camera_room, pos, next, true);
        }
        false
    }

    /// `Update`, right after the fly's.
    pub fn update(
        &mut self,
        view: &FlyView<'_>,
        room: &Room,
        cameras: usize,
        camera_room: &Room,
        camera: &CameraView,
        rng: &mut UnityRandom,
    ) {
        self.last_culled = self.culled;
        self.culled = self.should_be_culled(view, cameras, camera_room, camera);
        if !self.culled && self.last_culled {
            self.reset(view);
        }
        if self.culled {
            return;
        }
        let fly = view.fly;
        let chunk = fly.chunk();
        let behavior = fly.ai.behavior;
        let dropping = matches!(behavior, Some(FlyBehavior::Drop | FlyBehavior::Burrow));
        self.lower_body.update_free(room);
        if view.submersion > 0.0 || dropping {
            self.lower_body.vel *= 0.9;
        } else {
            self.lower_body.vel.y -= fly.body.params.gravity() * 2.0;
        }
        self.lower_body.push_out_of_terrain(room);
        self.lower_body.connect_to_point(PointConnection {
            point: chunk.pos,
            rad: 10.0,
            push: true,
            elastic: 0.0,
            host_vel: chunk.vel,
            adapt_vel: 0.0,
            exaggerate_vel: 0.0,
        });
        if behavior == Some(FlyBehavior::Chain) {
            self.lower_body.vel *= 0.0;
            if let Some(p) = view.grasp_pos {
                self.lower_body.pos = p;
            } else if let Some(spot) = fly.burrow_or_hang_spot {
                self.lower_body.pos = spot;
            }
        } else if let Some((grabber, _)) = view.grabber {
            let mut v = (grabber - chunk.pos).normalized().perpendicular() * 4.0;
            v.y = -v.y.abs();
            self.lower_body.vel += v;
        }
        if self.lower_body.pos.x == chunk.pos.x {
            self.lower_body.vel.x += (-0.1f32).lerp(0.1, rng.value());
        }
        let held_by_other = view.grabber.is_some_and(|(_, by_fly)| !by_fly);
        if !fly.dead && held_by_other {
            for w in &mut self.wings {
                w[1] = w[0];
            }
            self.wings[0][0] = rng.value();
            self.wings[1][0] = rng.value();
        } else if fly.conscious() {
            self.conscious_wings(view);
            self.insect_flight_wing_sync += rng.value() / 40.0;
            if self.insect_flight_wing_sync > 1.0 {
                self.insect_flight_wing_sync -= 1.0;
            }
        } else {
            self.last_horizontal_steering_compensation = self.horizontal_steering_compensation;
            self.horizontal_steering_compensation *= 0.999;
            let aim = (self.lower_body.pos - chunk.pos).vec_to_deg();
            let y = Self::rotate_around_origo(chunk.pos - chunk.last_pos, aim).y;
            for (w, death) in self.wings.iter_mut().zip(self.death_wing_positions) {
                w[1] = w[0];
                w[0] = w[0].lerp(death - y * 0.1, 0.3).clamp(0.0, 1.0);
            }
        }
    }

    /// The conscious branch of the wing update.
    fn conscious_wings(&mut self, view: &FlyView<'_>) {
        let fly = view.fly;
        let behavior = fly.ai.behavior;
        self.last_horizontal_steering_compensation = self.horizontal_steering_compensation;
        self.horizontal_steering_compensation =
            self.lower_body.pos.direction_to(fly.pos()).x - fly.dir.x;
        for w in &mut self.wings {
            if fly.mov_mode == Some(MovementMode::SwarmFlight) {
                w[0] = 0.0;
                w[1] = 1.0;
            } else if matches!(behavior, Some(FlyBehavior::Drop | FlyBehavior::Burrow)) {
                w[1] = w[0];
                w[0] = 1.0;
                self.horizontal_steering_compensation = 0.0;
            } else if behavior == Some(FlyBehavior::Chain) {
                match fly.chain_behavior_variation {
                    1 | 2 => {
                        w[1] = w[0];
                        w[0] = fly.flap;
                    }
                    3 => {
                        w[1] = 0.0;
                        w[0] = 1.0;
                    }
                    _ => {
                        w[1] = w[0];
                        w[0] = 1.0;
                        self.horizontal_steering_compensation = 0.0;
                    }
                }
            } else {
                w[1] = w[0];
                w[0] = fly.flap;
            }
        }
    }

    /// `Custom.RotateAroundOrigo` (the angle in single precision, the trigonometry in
    /// double, as `Mathf`).
    fn rotate_around_origo(v: Vec2, deg: f32) -> Vec2 {
        let rad = deg * (-std::f32::consts::PI / 180.0);
        let (cos, sin) = (f64::from(rad).cos() as f32, f64::from(rad).sin() as f32);
        Vec2::new(cos * v.x - sin * v.y, sin * v.x + cos * v.y)
    }

    /// `DrawSprites` at `t` of the way from the last update to this one: `FlyBody` and
    /// the two `FlyWing`s in the palette's `black`, `FlyEyes` white, all in the
    /// midground. Nothing while culled or out of a room.
    #[must_use]
    pub fn draw(&self, view: &FlyView<'_>, black: Color, t: f32) -> DrawList {
        let fly = view.fly;
        let mut items = Vec::new();
        if self.culled || fly.room.is_none() {
            return DrawList { items };
        }
        let chunk = fly.chunk();
        let pos = chunk.last_pos.lerp(chunk.pos, t);
        let lower = self.lower_body.last_pos.lerp(self.lower_body.pos, t);
        let aim = (pos - lower).vec_to_deg();
        let centre = Vec2::new(0.5, 0.5);
        items.push(Self::sprite(
            0,
            "FlyBody",
            pos,
            aim,
            Vec2::new(1.0, 1.0),
            centre,
            black,
        ));
        let flap_depth = fly.last_flap_depth.lerp(fly.flap_depth, t);
        for i in 0..2 {
            let side = if i == 0 { -1.0 } else { 1.0 };
            let beat = self.wings[i][1].lerp(self.wings[i][0], t);
            let mut a = beat.lerp(0.5, 0.3f32.lerp(0.0, flap_depth));
            let steer = self
                .last_horizontal_steering_compensation
                .lerp(self.horizontal_steering_compensation, t);
            let steer = if (steer < 0.0) == (i == 0) {
                0.0
            } else {
                ((steer * 0.85).abs() - 0.1).clamp(0.0, 1.0)
            };
            a = a.lerp(0.5, steer);
            a = a.mathf_pow(2.0).inverse_lerp(0.01, 0.99);
            let rotation = side * (40.0 + 150.0 * a) + aim;
            let scale_x = side
                * if fly.flap_speed < 0.0 {
                    1.0
                } else {
                    1.0 - 0.6 * ((beat * std::f32::consts::PI).sin() * (1.0 - steer))
                };
            let visible = if i == 0 {
                fly.bites == 3
            } else {
                fly.bites > 1
            };
            if visible {
                items.push(Self::sprite(
                    1 + i,
                    "FlyWing",
                    pos,
                    rotation,
                    Vec2::new(scale_x, 1.0),
                    Vec2::new(0.5, 0.0),
                    black,
                ));
            }
        }
        let white = Color::WHITE;
        items.push(Self::sprite(
            3,
            "FlyEyes",
            pos,
            aim,
            Vec2::new(1.0, 1.0),
            centre,
            white,
        ));
        DrawList { items }
    }

    /// One of the fly's midground sprites.
    fn sprite(
        index: usize,
        element: &'static str,
        pos: Vec2,
        rotation: f32,
        scale: Vec2,
        anchor: Vec2,
        color: Color,
    ) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element,
            pos,
            rotation,
            scale,
            anchor,
            color,
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }
}
