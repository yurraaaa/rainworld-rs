//! `ShelterDoor.InitiateSprites` / `DrawSprites` (263972) of a regular shelter: 42
//! `shelterGate` sprites in the `Items` container, all turned to face the entrance,
//! drawn with `ColoredSprite3` (palette colours by the texels' depth).

use rw_sim::Vec2;
use rw_sim::shelter::ShelterDoor;

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::math::{CustomVec, Mathf};

const COG: &str = "ShelterGate_cog";
const PISTONS: [&str; 2] = ["ShelterGate_piston1", "ShelterGate_piston2"];
const PLUGS: [&str; 8] = [
    "ShelterGate_plug1",
    "ShelterGate_plug2",
    "ShelterGate_plug3",
    "ShelterGate_plug4",
    "ShelterGate_plug5",
    "ShelterGate_plug6",
    "ShelterGate_plug7",
    "ShelterGate_plug8",
];
const SEGMENTS: [&str; 10] = [
    "ShelterGate_segment1",
    "ShelterGate_segment2",
    "ShelterGate_segment3",
    "ShelterGate_segment4",
    "ShelterGate_segment5",
    "ShelterGate_segment6",
    "ShelterGate_segment7",
    "ShelterGate_segment8",
    "ShelterGate_segment9",
    "ShelterGate_segment10",
];
const CYLINDERS: [&str; 4] = [
    "ShelterGate_cylinder1",
    "ShelterGate_cylinder2",
    "ShelterGate_cylinder3",
    "ShelterGate_cylinder4",
];
const COVERS: [&str; 4] = [
    "ShelterGate_cover1",
    "ShelterGate_cover2",
    "ShelterGate_cover3",
    "ShelterGate_cover4",
];
const PUMPS: [&str; 8] = [
    "ShelterGate_pump1",
    "ShelterGate_pump2",
    "ShelterGate_pump3",
    "ShelterGate_pump4",
    "ShelterGate_pump5",
    "ShelterGate_pump6",
    "ShelterGate_pump7",
    "ShelterGate_pump8",
];
const HATCH: &str = "ShelterGate_Hatch";

/// The door's sprites.
pub trait ShelterDoorSprites {
    /// `DrawSprites` at `t` between ticks, in sprite order (hidden cylinders left out).
    fn draw(&self, t: f32) -> DrawList;
}

/// One sprite in the door's style, at `pos` in the room.
struct DoorSprite {
    index: usize,
    element: &'static str,
    pos: Vec2,
    rotation: f32,
    alpha: f32,
}

impl DoorSprite {
    fn item(self, scale_x: f32, anchor: Vec2) -> DrawItem {
        // `camPos += 0.25`: the sprites sit a quarter pixel down and left.
        DrawItem::Sprite(Sprite {
            index: self.index,
            element: self.element,
            pos: self.pos - Vec2::new(0.25, 0.25),
            rotation: self.rotation,
            scale: Vec2::new(scale_x, 1.0),
            anchor,
            color: Color::WHITE.with_alpha(self.alpha),
            shader: Shader::ColoredSprite3,
            layer: Layer::Items,
        })
    }
}

impl ShelterDoorSprites for ShelterDoor {
    fn draw(&self, t: f32) -> DrawList {
        let centre = Vec2::new(0.5, 0.5);
        let closed = self.closed();
        let (p, dir, perp) = (self.p_zero, self.dir, self.perp);
        let facing = dir.aim_to(Vec2::ZERO);
        // `(i % 2 == 0) ? -1 : 1`.
        let side = [-1.0, 1.0];
        let mut items: Vec<(usize, DrawItem)> = Vec::new();
        for j in 0..4 {
            let mut v = perp * (side[j % 2] * if j >= 2 { 35.0 } else { 55.0 });
            v -= dir * if j >= 2 { 50.0 } else { 15.0 };
            let spin = if j >= 2 { 400.0 } else { -150.0 };
            let s = DoorSprite {
                index: j,
                element: COG,
                pos: p + v,
                rotation: closed * spin * side[j % 2],
                alpha: 1.0 - (if j < 2 { 6.0 } else { 5.0 }) / 30.0,
            };
            items.push((j, s.item(1.0, centre)));
        }
        for (k, element) in SEGMENTS.iter().enumerate() {
            let pair = self.segment_pairs[k / 2];
            let out = (1.0 - pair[1].lerp_to(pair[0], t)).powf(0.75) * 55.0;
            let s = DoorSprite {
                index: 14 + k,
                element,
                pos: p + perp * (out * side[k % 2]),
                rotation: facing,
                alpha: 1.0 - 4.0 * closed.inverse_lerp(0.78, 0.61) / 30.0,
            };
            items.push((14 + k, s.item(1.0, centre)));
        }
        for (l, element) in PISTONS.iter().enumerate() {
            let piston = self.pistons[l];
            let mut v = dir * ((1.0 - piston[1].lerp_to(piston[0], t)) * -120.0);
            if self.pistons_closed() > 0.0 {
                v += perp * (5.0 * if l == 0 { -1.0 } else { 1.0 });
            }
            let s = DoorSprite {
                index: 4 + l,
                element,
                pos: p + v,
                rotation: facing,
                alpha: 0.9,
            };
            items.push((4 + l, s.item(1.0, centre)));
        }
        for (m, element) in COVERS.iter().enumerate() {
            let cover = self.covers[m];
            let out = (1.0 - cover[1].lerp_to(cover[0], t)).powf(2.5) * 65.0;
            let s = DoorSprite {
                index: 28 + m,
                element,
                pos: p + perp * (out * if m >= 2 { -1.0 } else { 1.0 }),
                rotation: facing,
                alpha: 1.0,
            };
            items.push((28 + m, s.item(1.0, centre)));
        }
        let cylinders = self.cylinders();
        for (n, element) in CYLINDERS.iter().enumerate() {
            if (n as f32) / 4.0 < cylinders {
                let s = DoorSprite {
                    index: 24 + n,
                    element,
                    pos: p,
                    rotation: facing,
                    alpha: 1.0,
                };
                items.push((24 + n, s.item(1.0, centre)));
            }
        }
        let exit = self.pumps_exit();
        for (k, (pump_el, plug_el)) in PUMPS.iter().zip(PLUGS.iter()).enumerate() {
            let pump = self.pumps[k];
            let at = pump[1].lerp_to(pump[0], t);
            let v = perp * (at * -42.0 * side[k % 2]) + -dir * (exit * 80.0);
            let s = DoorSprite {
                index: 32 + k,
                element: pump_el,
                pos: p + v,
                rotation: facing,
                alpha: 1.0,
            };
            items.push((32 + k, s.item(1.0, centre)));
            let v = perp * ((1.0 - at - 0.35).clamp(0.0, 1.0) * 60.0 * side[k % 2]);
            let s = DoorSprite {
                index: 6 + k,
                element: plug_el,
                pos: p + v,
                rotation: facing,
                alpha: 1.0,
            };
            items.push((6 + k, s.item(1.0, centre)));
        }
        let flaps = self.flaps_open();
        for k in 0..2 {
            let sign = if k == 0 { 1.0 } else { -1.0 };
            let pos = p + dir * 46.0 + perp * (15.0f32.lerp_to(25.0, flaps) * sign);
            let s = DoorSprite {
                index: 40 + k,
                element: HATCH,
                pos,
                rotation: (-dir).aim_to(dir) - 90.0 * -sign * flaps,
                alpha: 1.0,
            };
            items.push((
                40 + k,
                s.item(if k == 1 { -1.0 } else { 1.0 }, Vec2::new(0.2, 0.43)),
            ));
        }
        items.sort_by_key(|&(i, _)| i);
        DrawList {
            items: items.into_iter().map(|(_, item)| item).collect(),
        }
    }
}
