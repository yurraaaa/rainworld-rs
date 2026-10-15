//! The line geometry of the overseer holograms' parts: what each `HologramPart`
//! constructor builds with `AddLine` and its variants (`OverseerHolograms` and
//! `OverseerTutorialBehavior` in the decompile). Behaviour:
//! `rainworld-re/notes/overseer.md` §7.

use rw_sim::creature::overseer::hologram::{HologramPart, PartKind};
use rw_sim::{Vec2, Vec3};

use crate::math::UnityVector3;

/// One `HologramPart.Line`: drawn from `a` to `b` at transform 0 and from `a2` to `b2`
/// at transform 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HologramLine {
    pub a: Vec3,
    pub b: Vec3,
    pub a2: Vec3,
    pub b2: Vec3,
}

/// A part's sprites: `own_sprites` of its own first, then one `pixel` per line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartShape {
    pub own_sprites: usize,
    pub lines: Vec<HologramLine>,
}

impl HologramLine {
    fn new(a: Vec3, b: Vec3) -> Self {
        Self { a, b, a2: a, b2: b }
    }

    /// The keys' and buttons' press: the back ends (`z < 0`) shrink by `back` and their
    /// pressed ends move to the front face `depth`; the front ends' pressed ends grow by
    /// `front`.
    fn press(&mut self, back: f32, front: f32, depth: f32) {
        for (end, end2) in [(&mut self.a, &mut self.a2), (&mut self.b, &mut self.b2)] {
            if end.z < 0.0 {
                *end = Vec3::from_xy(end.xy() * back).with_z(end.z);
                *end2 = Vec3::from_xy(end2.xy() * back).with_z(depth);
            } else {
                *end2 = Vec3::from_xy(end2.xy() * front).with_z(end2.z);
            }
        }
    }

    /// `KeyBoardKey.MakeWider`: every x moves `add` further from the middle.
    fn widen(&mut self, add: f32) {
        for p in [&mut self.a, &mut self.a2, &mut self.b, &mut self.b2] {
            p.x += if p.x < 0.0 { -add } else { add };
        }
    }
}

/// `Custom.Vec3FromVec2`.
trait WithZ {
    fn with_z(self, z: f32) -> Self;
}

impl WithZ for Vec3 {
    fn with_z(self, z: f32) -> Self {
        Self::new(self.x, self.y, z)
    }
}

impl PartShape {
    /// The geometry `part`'s constructor builds. `parts` are the hologram's parts (a
    /// stick reads its socket's scale).
    #[must_use]
    pub fn of(part: &HologramPart, parts: &[HologramPart]) -> Self {
        let mut s = Self::default();
        match &part.kind {
            PartKind::Arrow { .. } => s.add_closed_3d_polygon(
                &[
                    Vec2::new(-3.5, 20.0),
                    Vec2::new(0.0, 27.0),
                    Vec2::new(3.5, 20.0),
                ],
                3.0,
            ),
            PartKind::Pentagon => s.ring_3d(5, 15.0, 0.0, 5.0),
            PartKind::Symbol(_) | PartKind::CyclingSymbol { .. } | PartKind::PlayerGhost { .. } => {
                s.own_sprites = 1;
            }
            PartKind::LightEffect {
                light_alpha,
                dark_alpha,
                ..
            } => {
                s.own_sprites = usize::from(*dark_alpha > 0.0) + usize::from(*light_alpha > 0.0);
            }
            PartKind::RedCross { small } => s.red_cross(*small),
            PartKind::FoodArrow => s.food_arrow(),
            PartKind::Key { wider, .. } => s.key(*wider),
            PartKind::GamePadButton { round, .. } => s.game_pad_button(*round),
            PartKind::GamePadSilhouette { xbox, ps, .. } => s.silhouette(*xbox, *ps),
            PartKind::GamePadStickSocket { scale } => s.ring_3d(8, 35.0 * scale, 0.0, 5.0),
            PartKind::GamePadStick { socket, switch, .. } => {
                let scale = match parts.get(*socket).map(|p| &p.kind) {
                    Some(PartKind::GamePadStickSocket { scale }) => *scale,
                    _ => 0.6,
                };
                s.stick(scale, *switch);
            }
            PartKind::SwitchButton(_) => s.switch_button(),
            PartKind::MeterBox { f, .. } => s.meter_box(*f),
            PartKind::HoloImage(_) => s.own_sprites = 2,
            PartKind::Frame => s.ring_3d(8, 50.0, 0.5, 5.0),
        }
        s
    }

    /// `AddLine(Vector3, Vector3)`.
    fn add_line(&mut self, a: Vec3, b: Vec3) {
        self.lines.push(HologramLine::new(a, b));
    }

    /// `AddLine(Vector2, Vector2)`: at depth 0.
    fn add_flat_line(&mut self, a: Vec2, b: Vec2) {
        self.add_line(Vec3::from_xy(a), Vec3::from_xy(b));
    }

    /// `AddDoubleLine`: the line at `-depth` and at `depth`.
    fn add_double_line(&mut self, a: Vec2, b: Vec2, depth: f32) {
        self.add_line(
            Vec3::from_xy(a).with_z(-depth),
            Vec3::from_xy(b).with_z(-depth),
        );
        self.add_line(
            Vec3::from_xy(a).with_z(depth),
            Vec3::from_xy(b).with_z(depth),
        );
    }

    /// `Add3DLine`: the double line and the edge joining its `a` ends.
    fn add_3d_line(&mut self, a: Vec2, b: Vec2, depth: f32) {
        self.add_double_line(a, b, depth);
        self.add_line(
            Vec3::from_xy(a).with_z(-depth),
            Vec3::from_xy(a).with_z(depth),
        );
    }

    fn add_closed_polygon(&mut self, points: &[Vec2]) {
        for w in points.windows(2) {
            self.add_flat_line(w[0], w[1]);
        }
        self.add_flat_line(points[points.len() - 1], points[0]);
    }

    fn add_closed_3d_polygon(&mut self, points: &[Vec2], depth: f32) {
        for w in points.windows(2) {
            self.add_3d_line(w[0], w[1], depth);
        }
        self.add_3d_line(points[points.len() - 1], points[0], depth);
    }

    /// `n` 3D lines round a circle of radius `r`, corner `i` at `(i + shift) / n` turns
    /// (`Pentagon`, `GamePadStickSocket`, `OverseerImage.Frame`).
    fn ring_3d(&mut self, n: usize, r: f32, shift: f32, depth: f32) {
        let count = n as f32;
        for i in 0..n {
            let from = (i as f32 + shift) / count;
            let to = ((i + 1) as f32 + shift) / count;
            self.add_3d_line(
                Vec2::from_degrees(from * 360.0) * r,
                Vec2::from_degrees(to * 360.0) * r,
                depth,
            );
        }
    }

    /// `InputInstruction.RedCross`: four arms 60 long and 10 wide.
    fn red_cross(&mut self, small: bool) {
        let mut points = Vec::with_capacity(12);
        for i in 0..4 {
            let v = Vec2::from_degrees(45.0 + 90.0 * i as f32);
            let side = v.perpendicular() * 5.0;
            points.push(v * 5.0 + side);
            points.push(v * 60.0 + side);
            points.push(v * 60.0 - side);
        }
        let scale = if small { 0.4 } else { 1.0 };
        for p in &mut points {
            *p *= scale;
        }
        self.add_closed_3d_polygon(&points, 5.0);
    }

    /// `Eat.FoodArrow`: a square at y 10 and lines from its corners (last first) to
    /// the tip at y -10.
    fn food_arrow(&mut self) {
        let (w, top, d) = (5.0, 10.0, 5.0);
        self.add_line(Vec3::new(-w, top, -d), Vec3::new(w, top, -d));
        self.add_line(Vec3::new(w, top, -d), Vec3::new(w, top, d));
        self.add_line(Vec3::new(w, top, d), Vec3::new(-w, top, d));
        self.add_line(Vec3::new(-w, top, d), Vec3::new(-w, top, -d));
        for k in (0..4).rev() {
            let a = self.lines[k].a;
            self.add_line(a, Vec3::new(0.0, -top, 0.0));
        }
    }

    /// `KeyBoardKey` and `MakeWider(wider)`.
    fn key(&mut self, wider: f32) {
        self.own_sprites = 1;
        let (r, corner, depth) = (15.0, 3.0, 5.0);
        self.add_closed_3d_polygon(
            &[
                Vec2::new(-r + corner, -r),
                Vec2::new(-r, -r + corner),
                Vec2::new(-r, r - corner),
                Vec2::new(-r + corner, r),
                Vec2::new(r - corner, r),
                Vec2::new(r, r - corner),
                Vec2::new(r, -r + corner),
                Vec2::new(r - corner, -r),
            ],
            depth,
        );
        for line in &mut self.lines {
            line.press(0.9, 1.1, depth);
            line.widen(wider);
        }
    }

    /// `GamePadButton`: 8 3D lines, or 24 double lines round for the PlayStation.
    fn game_pad_button(&mut self, round: bool) {
        self.own_sprites = 1;
        let (r, depth) = (18.0, 5.0);
        let n = if round { 24 } else { 8 };
        let count = n as f32;
        for i in 0..n {
            let from = i as f32 / count;
            let to = (i + 1) as f32 / count;
            let shift = 360.0 / (count / 2.0);
            let a = Vec2::from_degrees(from * 360.0 + shift) * r;
            let b = Vec2::from_degrees(to * 360.0 + shift) * r;
            if round {
                self.add_double_line(a, b, depth);
            } else {
                self.add_3d_line(a, b, depth);
            }
        }
        for line in &mut self.lines {
            line.press(0.9, 1.1, depth);
        }
    }

    /// `GamePadSilhouette`: four dots, and the pad's outline except for the PlayStation.
    fn silhouette(&mut self, xbox: bool, ps: bool) {
        self.own_sprites = 4;
        if ps {
            return;
        }
        let half: [(f32, f32); 13] = if xbox {
            [
                (22.0, 7.25),
                (20.0, 7.25),
                (15.0, 6.0),
                (14.0, 5.5),
                (11.0, 3.0),
                (8.0, 0.0),
                (2.5, -0.5),
                (1.0, 3.0),
                (0.5, 6.0),
                (5.0, 25.0),
                (9.0, 30.0),
                (14.0, 30.0),
                (19.5, 27.5),
            ]
        } else {
            [
                (22.0, 11.0),
                (20.0, 9.0),
                (15.0, 10.0),
                (14.0, 10.0),
                (11.0, 3.0),
                (8.0, 0.0),
                (5.0, 0.0),
                (1.0, 3.0),
                (0.0, 6.0),
                (5.0, 25.0),
                (9.0, 30.0),
                (14.0, 30.0),
                (16.0, 28.0),
            ]
        };
        let mut points = [Vec2::ZERO; 26];
        for (j, &(x, y)) in half.iter().enumerate() {
            points[j] = Vec2::new(x, y) + Vec2::new(-25.0, -15.0);
            points[25 - j] = Vec2::new(25.0 - x, y - 15.0);
        }
        self.add_closed_polygon(&points);
    }

    /// `GamePadStick`: a ring at depths -10 and 0, a wider one at -5 between them, and
    /// the slanted edges.
    fn stick(&mut self, scale: f32, switch: bool) {
        let r = if switch { 12.0 + 6.0 } else { 12.0 } * scale;
        let mid = -5.0;
        let rim = 5.0 * scale;
        for i in 0..8 {
            let from = Vec2::from_degrees(i as f32 / 8.0 * 360.0);
            let to = Vec2::from_degrees((i + 1) as f32 / 8.0 * 360.0);
            let (back, front) = (-5.0 + mid, 5.0 + mid);
            self.add_line(Self::at(from, r, back), Self::at(to, r, back));
            self.add_line(Self::at(from, r + rim, mid), Self::at(to, r + rim, mid));
            self.add_line(Self::at(from, r, front), Self::at(to, r, front));
            self.add_line(Self::at(from, r, back), Self::at(from, r + rim, mid));
            self.add_line(Self::at(from, r, front), Self::at(from, r + rim, mid));
        }
    }

    /// `Custom.Vec3FromVec2(dir * radius, z)`.
    fn at(dir: Vec2, radius: f32, z: f32) -> Vec3 {
        Vec3::from_xy(dir * radius).with_z(z)
    }

    /// `SwitchInstruction.SwitchButton`: an octagon whose back shrinks to the front
    /// when pressed.
    fn switch_button(&mut self) {
        self.own_sprites = 1;
        let (r, depth) = (9.0, 2.0);
        for i in 0..8 {
            let from = i as f32 / 8.0;
            let to = (i + 1) as f32 / 8.0;
            self.add_3d_line(
                Vec2::from_degrees(from * 360.0 + 22.5) * r,
                Vec2::from_degrees(to * 360.0 + 22.5) * r,
                depth,
            );
        }
        for line in &mut self.lines {
            line.press(0.8, 1.0, depth);
        }
    }

    /// `WaitMeter.MeterBox` at fraction `f` of the circle: a box from radius 50 to 60
    /// that swells when lit.
    fn meter_box(&mut self, f: f32) {
        let (inner, outer, half) = (50.0, 60.0, 2.5);
        let left = Vec2::from_degrees(f * 360.0 - half);
        let right = Vec2::from_degrees(f * 360.0 + half);
        self.add_3d_line(left * inner, right * inner, 5.0);
        self.add_3d_line(right * inner, right * outer, 5.0);
        self.add_3d_line(right * outer, left * outer, 5.0);
        self.add_3d_line(left * outer, left * inner, 5.0);
        let mut sum = Vec3::new(0.0, 0.0, 0.0);
        for line in &self.lines {
            sum = sum + line.a;
        }
        let n = self.lines.len() as f32;
        let centre = Vec3::new(sum.x / n, sum.y / n, sum.z / n);
        for line in &mut self.lines {
            line.a = line.a.minus(line.a2.minus(centre).unit());
            line.b = line.b.minus(line.b2.minus(centre).unit());
            line.a2 = line.a2 + line.a2.minus(centre).unit().scale(2.0);
            line.b2 = line.b2 + line.b2.minus(centre).unit().scale(2.0);
        }
    }
}
