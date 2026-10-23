//! `ProjectedCircle`'s sprites in `BackgroundShortcuts`: two `Futile_White` discs under the
//! `VectorCircle` shader (a red `1/255` one and a black ring), black `pixel` spokes
//! around the rim and black lines to the connected circles and the off-screen points. A
//! blinking connection draws a cosmetic random value for its width.
//!
//! Mirrors: CoralBrain/ProjectedCircle.cs (InitiateSprites, DrawSprites, Position, Rad)

use rw_random::UnityRandom;
use rw_sim::{CircleOwner, CoralNeuron, CoralStem, ProjectedCircle, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::math::{CustomVec, Mathf};

/// What a circle reads of the room around it: where the circles and their owners are.
pub struct CircleSurroundings<'a> {
    pub neurons: &'a [CoralNeuron],
    pub stems: &'a [CoralStem],
    pub circles: &'a [ProjectedCircle],
    /// `room.lightAngle`.
    pub light_angle: Vec2,
}

impl CircleSurroundings<'_> {
    /// `Position(timeStacker)` of `circle`: its owner's centre shifted against the light by
    /// its depth.
    #[must_use]
    pub fn position(&self, circle: &ProjectedCircle, t: f32) -> Vec2 {
        let centre = match circle.owner {
            CircleOwner::Neuron { order } => self
                .neurons
                .iter()
                .find(|n| n.update_order == order)
                .map_or(Vec2::ZERO, |n| n.geometry.circle_center(circle.index, t)),
            CircleOwner::Stem { stem } => self
                .stems
                .get(stem as usize)
                .and_then(|s| s.circle_center(t))
                .unwrap_or(Vec2::ZERO),
        };
        centre
            - Vec2::new(self.light_angle.x, -self.light_angle.y)
                * (circle.last_depth_offset.lerp_to(circle.depth_offset, t) * 15.0)
    }

    fn circle(&self, order: u64) -> Option<&ProjectedCircle> {
        self.circles.iter().find(|c| c.update_order == order)
    }

    /// A sprite of a black `pixel` standing on its end.
    fn pixel(index: usize, pos: Vec2, rotation: f32, scale: Vec2) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "pixel",
            pos,
            rotation,
            scale,
            anchor: Vec2::new(0.5, 0.0),
            color: Color::BLACK,
            shader: Shader::Basic,
            layer: Layer::BackgroundShortcuts,
        })
    }

    fn disc(index: usize, pos: Vec2, scale: f32, color: Color) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "Futile_White",
            pos,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader: Shader::VectorCircle,
            layer: Layer::BackgroundShortcuts,
        })
    }
}

/// A circle's draw.
pub trait ProjectedCircleSprites {
    /// `DrawSprites` at `t` between ticks (after the depth update); `rng` is the cosmetic
    /// stream.
    fn draw(&self, around: &CircleSurroundings<'_>, t: f32, rng: &mut UnityRandom) -> DrawList;
}

impl ProjectedCircleSprites for ProjectedCircle {
    fn draw(&self, around: &CircleSurroundings<'_>, t: f32, rng: &mut UnityRandom) -> DrawList {
        let rad = self.rad_at(t);
        let here = around.position(self, t);
        let blink = self.last_blink.lerp_to(self.blink, t);
        let spokes = usize::try_from(self.spokes).unwrap_or(0);
        let mut items = vec![
            CircleSurroundings::disc(0, here, rad / 8.0, Color::rgb(1.0 / 255.0, 0.0, 0.0)),
            CircleSurroundings::disc(
                1,
                here,
                rad / 8.0,
                Color::BLACK.with_alpha(
                    4.0 / self
                        .rad
                        .lerp_to(4.0, rw_sim::FloatExt::mathf_pow(blink, 0.5)),
                ),
            ),
        ];
        let turn = self.last_rotation.lerp_to(self.rotation, t);
        for j in 0..spokes {
            let degrees = j as f32 / spokes as f32 * 360.0 + turn;
            let at = here + Vec2::from_degrees(degrees) * (rad - 1.0f32.lerp_to(5.0, blink));
            items.push(CircleSurroundings::pixel(
                j + 2,
                at,
                degrees,
                Vec2::new(
                    4.0f32.lerp_to(
                        rad * std::f32::consts::PI / spokes as f32,
                        rw_sim::FloatExt::mathf_pow(blink, 1.5),
                    ),
                    10.0,
                ),
            ));
        }
        let connected = self.connected_circles.len();
        for k in 0..connected + self.off_screen_connections.len() {
            let (to, other_rad) = if k < connected {
                match around.circle(self.connected_circles[k]) {
                    Some(c) => (around.position(c, t), c.rad_at(t)),
                    None => continue,
                }
            } else {
                (self.off_screen_connections[k - connected], 0.0)
            };
            let direction = here.direction_to(to);
            let wide = if self.connections_blink[k] > 0 && rng.value() < 0.7 {
                4.0
            } else {
                2.0
            };
            items.push(CircleSurroundings::pixel(
                k + spokes + 2,
                here + direction * rad,
                here.aim_to(to),
                Vec2::new(wide, (here.distance(to) - rad - other_rad).max(0.0)),
            ));
        }
        DrawList { items }
    }
}
