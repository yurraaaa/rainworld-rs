//! `PlayerGraphics.DrawSprites`, `InitiateSprites` and `AddToContainer` (base game):
//! the slugcat's 12 sprites as a [`DrawList`].
//!
//! Behaviour: `rainworld-re/notes/slugcat-graphics.md` sections 4-7.
//!
//! Mirrors: PlayerGraphics.cs (DrawSprites, InitiateSprites, AddToContainer)

use std::borrow::Cow;
use std::f32::consts::TAU;

use rw_sim::{Animation, BodyMode, Player, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::elements::{
    BODY_A, FACE_A, FACE_B, FACE_DEAD, FACE_STUNNED, HEAD_A, HIPS_A, LEGS_A, LEGS_A_AIR,
    LEGS_A_CLIMBING, LEGS_A_CRAWLING, LEGS_A_ON_POLE, LEGS_A_POLE, LEGS_A_VERTICAL_POLE,
    LEGS_A_WALL, ON_TOP_OF_TERRAIN_HAND, ON_TOP_OF_TERRAIN_HAND_2, PLAYER_ARM,
};
use crate::limb::LimbMode;
use crate::math::{CustomVec, Mathf};
use crate::player_graphics::PlayerGraphics;

/// Number of base-game slugcat sprites.
pub const SPRITE_COUNT: usize = 12;

/// `AddToContainer` (base game): hand caps (7, 8) and the Mark (10, 11) go to
/// Foreground, the rest to Midground; each container keeps index order.
pub const SPRITE_LAYERS: [Layer; SPRITE_COUNT] = [
    Layer::Midground,
    Layer::Midground,
    Layer::Midground,
    Layer::Midground,
    Layer::Midground,
    Layer::Midground,
    Layer::Midground,
    Layer::Foreground,
    Layer::Foreground,
    Layer::Midground,
    Layer::Foreground,
    Layer::Foreground,
];

/// The tail mesh's 13 triangles over its 15 vertices.
pub const TAIL_TRIANGLES: [[u16; 3]; 13] = [
    [0, 1, 2],
    [1, 2, 3],
    [4, 5, 6],
    [5, 6, 7],
    [8, 9, 10],
    [9, 10, 11],
    [12, 13, 14],
    [2, 3, 4],
    [3, 4, 5],
    [6, 7, 8],
    [7, 8, 9],
    [10, 11, 12],
    [11, 12, 13],
];

/// Default sprite anchor (centre).
const CENTRE: Vec2 = Vec2::new(0.5, 0.5);

/// Interpolated positions shared by the sprites of one frame.
struct Frame {
    /// Body draw position (`drawPositions[0]`).
    body: Vec2,
    /// Hips draw position (`drawPositions[1]`).
    hips: Vec2,
    head: Vec2,
    /// `0.5 + 0.5 sin(breath)`.
    breath: f32,
    /// `player.sleepCurlUp`.
    curl: f32,
    /// `malnourished`.
    malnourished: f32,
}

impl PlayerGraphics {
    /// `DrawSprites` at `time_stacker` (fraction of the tick since the last update):
    /// the visible sprites in container order, in room pixels. `black_color` and
    /// `fog_color` are the room palette's (the eyes' colour, and the malnourished eyes'
    /// tint). Empty before the first update, inside a shortcut and once the body is
    /// destroyed.
    pub fn draw(
        &mut self,
        player: &Player,
        time_stacker: f32,
        black_color: Color,
        fog_color: Color,
    ) -> DrawList {
        if self.culled || player.in_shortcut() || player.is_destroyed() {
            return DrawList::default();
        }
        // `ApplyPalette` again while the malnourishment fades.
        if self.malnourished > 0.0 && !player.malnourished() {
            self.apply_palette(player);
        }
        let face_color = self.face_color(player, black_color, fog_color);
        let t = time_stacker;
        let aerobic = player.aerobic_level();
        let mut f = Frame {
            body: self.draw_positions[0][1].lerp_clamped(self.draw_positions[0][0], t),
            hips: self.draw_positions[1][1].lerp_clamped(self.draw_positions[1][0], t),
            head: self.head.last_pos.lerp_clamped(self.head.pos, t),
            breath: 0.5 + 0.5 * (self.last_breath.lerp_to(self.breath, t) * TAU).sin(),
            curl: player.sleep_curl_up(),
            malnourished: self.malnourished,
        };
        if aerobic > 0.5 {
            let dir = f.hips.direction_to(f.body);
            let pant = (-1.0_f32).lerp_to(1.0, f.breath);
            let level = aerobic.inverse_lerp(0.5, 1.0);
            f.body += dir * (pant * level * 0.5);
            f.head -= f.hips.direction_to(f.body) * (pant * level.powf(1.5) * 0.75);
        }

        let mut slots: [Option<DrawItem>; SPRITE_COUNT] = Default::default();
        let [body, hips] = self.body_sprites(&f, t, aerobic);
        slots[0] = Some(DrawItem::Sprite(body));
        slots[1] = Some(DrawItem::Sprite(hips));
        slots[2] = Some(DrawItem::Mesh(self.tail_mesh(&f, t)));
        let [head, face] = self.head_sprites(player, &f, t, face_color);
        slots[3] = Some(DrawItem::Sprite(head));
        slots[9] = Some(DrawItem::Sprite(face));
        // Hidden while swimming under water.
        if player.animation() != Animation::DeepSwim {
            slots[4] = Some(DrawItem::Sprite(self.legs_sprite(player, &f, t)));
        }
        for j in 0..2 {
            let (arm, cap) = self.hand_sprites(player, &f, t, j);
            slots[5 + j] = arm.map(DrawItem::Sprite);
            slots[7 + j] = cap.map(DrawItem::Sprite);
        }
        // Sprites 10 and 11 (the Mark) have alpha 0 without the Mark: not drawn.

        let mut list = DrawList::default();
        for layer in [Layer::Midground, Layer::Foreground] {
            for (i, slot) in slots.iter_mut().enumerate() {
                if SPRITE_LAYERS[i] == layer
                    && let Some(item) = slot.take()
                {
                    list.items.push(item);
                }
            }
        }
        list
    }

    /// `HeadA` frame for a head angle (degrees from upright): `round(|angle| / 360 * 34)`.
    #[must_use]
    pub fn head_frame(angle: f32) -> usize {
        ((angle / 360.0 * 34.0).abs().round_to_int() as usize).min(HEAD_A.len() - 1)
    }

    /// `Face` frame for the eyes' direction: `round(|angle| / 22.5)`.
    #[must_use]
    pub fn face_frame(dir: Vec2) -> usize {
        ((Vec2::ZERO.aim_to(dir) / 22.5).abs().round_to_int() as usize).min(FACE_A.len() - 1)
    }

    /// `PlayerArm` frame for a hand `dist` px from its shoulder.
    #[must_use]
    pub fn arm_frame(dist: f32) -> usize {
        (dist / 2.0).clamp(0.0, 12.0).round_to_int() as usize
    }

    /// Body scale x for breathing phase `breath` (0..1), uprightness `vert`,
    /// `sleepCurlUp` and `malnourished`:
    /// `1 + lerp(lerp(lerp(-0.05, -0.15, malnourished), 0.05, breath) * vert, 0.15, curl)`.
    #[must_use]
    pub fn body_scale_x(breath: f32, vert: f32, sleep_curl_up: f32, malnourished: f32) -> f32 {
        let thin = (-0.05_f32).lerp_to(-0.15, malnourished);
        1.0 + (thin.lerp_to(0.05, breath) * vert).lerp_to(0.15, sleep_curl_up)
    }

    /// Hips scale x: `1 + 0.2 sleepCurlUp + 0.05 breath - 0.05 malnourished`.
    #[must_use]
    pub fn hips_scale_x(breath: f32, sleep_curl_up: f32, malnourished: f32) -> f32 {
        1.0 + sleep_curl_up * 0.2 + 0.05 * breath - 0.05 * malnourished
    }

    /// The face's colour (sprite 9, base game): `blackColor`, lerped
    /// `0.2 * malnourished^2` toward `lerp(white, fogColor, 0.5)`.
    #[must_use]
    pub fn face_color(&self, player: &Player, black_color: Color, fog_color: Color) -> Color {
        if self.malnourished <= 0.0 {
            return black_color;
        }
        let m = self.malnourished_tint(player);
        black_color.lerp(Color::WHITE.lerp(fog_color, 0.5), 0.2 * m * m)
    }

    /// Body (0) and hips (1).
    fn body_sprites(&self, f: &Frame, t: f32, aerobic: f32) -> [Sprite; 2] {
        let vert = f.hips.direction_to(f.body).y.abs().inverse_lerp(0.3, 0.5);
        let rise = 0.5_f32.lerp_to(1.0, aerobic) * f.breath * (1.0 - vert);
        let tail0 = self.tail[0]
            .part
            .last_pos
            .lerp_clamped(self.tail[0].part.pos, t);
        [
            Sprite {
                index: 0,
                element: BODY_A,
                pos: Vec2::new(f.body.x, f.body.y - f.curl * 4.0 + rise),
                rotation: f.hips.aim_to(f.body),
                scale: Vec2::new(
                    Self::body_scale_x(f.breath, vert, f.curl, f.malnourished),
                    1.0,
                ),
                anchor: Vec2::new(0.5, 0.789_473_7),
                color: self.body_color,
                shader: Shader::Basic,
                layer: SPRITE_LAYERS[0],
            },
            Sprite {
                index: 1,
                element: HIPS_A,
                pos: (f.hips * 2.0 + f.body) * (1.0 / 3.0) - Vec2::new(0.0, f.curl * 3.0),
                rotation: f.body.aim_to(tail0),
                scale: Vec2::new(
                    Self::hips_scale_x(f.breath, f.curl, f.malnourished),
                    1.0 + f.curl * 0.2,
                ),
                anchor: CENTRE,
                color: self.body_color,
                shader: Shader::Basic,
                layer: SPRITE_LAYERS[1],
            },
        ]
    }

    /// The tail (2): four quads from the hips through the segments, 6 px half-width
    /// at the root, each segment's stretched radius after it, the tip a point; the
    /// widths scaled by `1 - 0.2 malnourished`.
    fn tail_mesh(&self, f: &Frame, t: f32) -> Mesh {
        let mut vertices = vec![Vec2::ZERO; 15];
        let mut from = (f.hips * 3.0 + f.body) * 0.25;
        let mut half_width = 6.0;
        let thin = 1.0 - 0.2 * f.malnourished;
        for (i, seg) in self.tail.iter().enumerate() {
            let to = seg.part.last_pos.lerp_clamped(seg.part.pos, t);
            let dir = (to - from).normalized();
            let perp = dir.perpendicular();
            let overlap = if i == 0 { 0.0 } else { to.distance(from) / 5.0 };
            vertices[i * 4] = from - perp * (half_width * thin) + dir * overlap;
            vertices[i * 4 + 1] = from + perp * (half_width * thin) + dir * overlap;
            if i < 3 {
                let r = perp * (seg.stretched_rad() * thin);
                vertices[i * 4 + 2] = to - r - dir * overlap;
                vertices[i * 4 + 3] = to + r - dir * overlap;
            } else {
                vertices[i * 4 + 2] = to;
            }
            half_width = seg.stretched_rad();
            from = to;
        }
        Mesh {
            index: 2,
            vertices,
            uvs: Vec::new(),
            triangles: Cow::Borrowed(&TAIL_TRIANGLES),
            color: self.body_color,
            vertex_colors: Vec::new(),
            shader: Shader::Basic,
            layer: SPRITE_LAYERS[2],
        }
    }

    /// Head (3) and face (9): the head frame follows the angle from the body's middle
    /// to the head; the face frame looks along the head, closed while blinking. Curled
    /// up asleep, the head turns sideways and tucks in, eyes down.
    fn head_sprites(
        &mut self,
        player: &Player,
        f: &Frame,
        t: f32,
        face_color: Color,
    ) -> [Sprite; 2] {
        let mut angle = f.body.lerp(f.hips, 0.5).aim_to(f.head);
        let facing = if angle < 0.0 { -1.0 } else { 1.0 };
        let mut head_frame = Self::head_frame(angle);
        let mut head = f.head;
        let mut look =
            self.last_look_dir.lerp_clamped(self.look_direction, t) * (3.0 * (1.0 - f.curl));
        let faces = if self.blink > 0 { &FACE_B } else { &FACE_A };
        let (face_element, face_rotation);
        if f.curl > 0.0 {
            head_frame = (7.0_f32.lerp_to(4.0, f.curl) as usize).min(8);
            let side = (f.body.x - f.hips.x).unity_sign();
            self.sprites.face_scale_x = side;
            let frame = (head_frame as f32).lerp_to(1.0, f.curl) as usize;
            face_element = faces[frame.min(faces.len() - 1)];
            face_rotation = angle * (1.0 - f.curl);
            angle = angle.lerp_to(45.0 * side, f.curl);
            head += Vec2::new(side * 2.0 * f.curl, f.curl);
            look -= Vec2::new(4.0 * side * f.curl, 2.0 * f.curl);
        } else if player.effective_room_gravity() == 0.0 {
            head_frame = 0;
            face_rotation = angle;
            face_element = if player.is_conscious() {
                faces[0]
            } else if player.is_dead() {
                FACE_DEAD
            } else {
                FACE_STUNNED
            };
        } else if player.is_conscious() {
            let sideways =
                player.body_mode() == BodyMode::Stand && player.input_history()[0].x != 0;
            if sideways || player.body_mode() == BodyMode::Crawl {
                if player.body_mode() == BodyMode::Crawl {
                    head_frame = 7;
                    self.sprites.face_scale_x = (f.body.x - f.hips.x).unity_sign();
                } else {
                    head_frame = 6;
                    self.sprites.face_scale_x = facing;
                }
                look.x = 0.0;
                face_element = faces[4];
            } else {
                let mut v = f.head - f.hips;
                v.x *= 1.0 - look.length() / 3.0;
                self.sprites.face_scale_x = if look.x.abs() < 0.1 {
                    facing
                } else {
                    look.x.unity_sign()
                };
                face_element = faces[Self::face_frame(v.normalized())];
            }
            face_rotation = 0.0;
        } else {
            look = Vec2::ZERO;
            head_frame = 0;
            face_element = if player.is_dead() {
                FACE_DEAD
            } else {
                FACE_STUNNED
            };
            face_rotation = angle;
        }
        [
            Sprite {
                index: 3,
                element: HEAD_A[head_frame],
                pos: head,
                rotation: angle,
                scale: Vec2::new(if angle < 0.0 { -1.0 } else { 1.0 }, 1.0),
                anchor: CENTRE,
                color: self.body_color,
                shader: Shader::Basic,
                layer: SPRITE_LAYERS[3],
            },
            Sprite {
                index: 9,
                element: face_element,
                pos: head + look + Vec2::new(0.0, -2.0),
                rotation: face_rotation,
                scale: Vec2::new(self.sprites.face_scale_x, 1.0),
                anchor: CENTRE,
                color: face_color,
                shader: Shader::Basic,
                layer: SPRITE_LAYERS[9],
            },
        ]
    }

    /// Legs (4): the gait frame for the body mode, rotated against `legs_direction`;
    /// on a vertical pole kept within 6 px below to 4 px above the hips.
    fn legs_sprite(&mut self, player: &Player, f: &Frame, t: f32) -> Sprite {
        let frame = usize::try_from(player.animation_frame()).unwrap_or(0);
        let by_flip = if player.flip_direction() > 0 {
            1.0
        } else {
            -1.0
        };
        let mut element = LEGS_A_AIR;
        let mut pos = self.legs.last_pos.lerp_clamped(self.legs.pos, t);
        match player.body_mode() {
            BodyMode::Stand => {
                element = LEGS_A[frame.min(LEGS_A.len() - 1)];
                self.sprites.legs_scale_x = by_flip;
            }
            BodyMode::Crawl => {
                element = LEGS_A_CRAWLING[(frame / 2).min(LEGS_A_CRAWLING.len() - 1)];
                self.sprites.legs_scale_x = by_flip;
            }
            // Frames 7..=12 replay 1..=6 mirrored.
            BodyMode::CorridorClimb => {
                let mirrored = frame > 6;
                element = LEGS_A_CLIMBING[if mirrored { frame % 6 } else { frame }.min(6)];
                self.sprites.legs_scale_x = if mirrored { -1.0 } else { 1.0 };
            }
            BodyMode::ClimbingOnBeam => match player.animation() {
                Animation::StandOnBeam => {
                    element = LEGS_A_ON_POLE[if frame < 7 { frame } else { 0 }];
                    self.sprites.legs_scale_x = by_flip;
                }
                Animation::BeamTip => element = LEGS_A_POLE,
                Animation::ClimbOnBeam => {
                    element = LEGS_A_VERTICAL_POLE;
                    self.sprites.legs_scale_x = by_flip;
                    pos.y = pos.y.clamp(f.hips.y - 6.0, f.hips.y + 4.0);
                    self.sprites.hand_cap_element = [ON_TOP_OF_TERRAIN_HAND; 2];
                }
                Animation::HangFromBeam => {
                    self.sprites.hand_cap_element = [ON_TOP_OF_TERRAIN_HAND_2; 2];
                }
                _ => {}
            },
            BodyMode::ZeroG if player.animation() == Animation::ZeroGPoleGrab => {
                self.sprites.hand_cap_element = [ON_TOP_OF_TERRAIN_HAND; 2];
            }
            BodyMode::WallClimb => {
                element = LEGS_A_WALL;
                self.sprites.legs_scale_x = by_flip;
            }
            BodyMode::Default if player.animation() == Animation::LedgeGrab => {
                element = LEGS_A_WALL;
                self.sprites.legs_scale_x = by_flip;
            }
            _ => {}
        }
        Sprite {
            index: 4,
            element,
            pos,
            rotation: self.legs_direction.aim_to(Vec2::ZERO),
            scale: Vec2::new(self.sprites.legs_scale_x, 1.0),
            anchor: Vec2::new(0.5, 0.25),
            color: self.body_color,
            shader: Shader::Basic,
            layer: SPRITE_LAYERS[4],
        }
    }

    /// Arm `5 + j` (hidden while retracted, and while gripping a vertical pole) and hand
    /// cap `7 + j` (only while gripping a beam or pole).
    fn hand_sprites(
        &mut self,
        player: &Player,
        f: &Frame,
        t: f32,
        j: usize,
    ) -> (Option<Sprite>, Option<Sprite>) {
        let hand = &self.hands[j];
        let pos = hand.limb.part.last_pos.lerp_clamped(hand.limb.part.pos, t);
        let animation = player.animation();
        let mut arm = None;
        let climbing = matches!(animation, Animation::ClimbOnBeam | Animation::ZeroGPoleGrab);
        if hand.limb.mode != LimbMode::Retracted {
            let body_angle = f.hips.aim_to(f.body);
            let mut spread = 4.5 / (hand.retract_counter as f32 + 1.0);
            if matches!(animation, Animation::StandOnBeam | Animation::BeamTip)
                && self.disbalance_amount <= 40.0
                && hand.limb.mode == LimbMode::HuntRelativePosition
            {
                spread *= self.disbalance_amount / 40.0;
            }
            if animation == Animation::HangFromBeam {
                spread *= 0.5;
            }
            spread *= (body_angle / 360.0 * TAU).cos().abs();
            let side = -1.0 + 2.0 * j as f32;
            let shoulder = f.body + Vec2::new(side * spread, -3.5).rotated(body_angle);
            self.sprites.arm_scale_y[j] = match player.body_mode() {
                BodyMode::Crawl => {
                    if f.body.x < f.hips.x {
                        -1.0
                    } else {
                        1.0
                    }
                }
                BodyMode::WallClimb => {
                    if player.flip_direction() == -1 {
                        -1.0
                    } else {
                        1.0
                    }
                }
                _ => pos.distance_to_line(f.body, f.hips).unity_sign(),
            };
            if animation == Animation::HangUnderVerticalBeam {
                self.sprites.arm_scale_y[j] = if j == 0 { 1.0 } else { -1.0 };
            }
            let sprite = Sprite {
                index: 5 + j,
                element: PLAYER_ARM[Self::arm_frame(pos.distance(shoulder))],
                pos,
                rotation: pos.aim_to(shoulder) + 90.0,
                scale: Vec2::new(1.0, self.sprites.arm_scale_y[j]),
                anchor: Vec2::new(0.9, 0.5),
                color: self.body_color,
                shader: Shader::Basic,
                layer: SPRITE_LAYERS[5 + j],
            };
            // Futile keeps the hidden sprite's values; only visibility changes.
            arm = (!climbing || !hand.limb.reached_snap_position).then_some(sprite);
        }
        let gripping = matches!(
            animation,
            Animation::ClimbOnBeam
                | Animation::HangFromBeam
                | Animation::GetUpOnBeam
                | Animation::ZeroGPoleGrab
        ) && self.hands[j].limb.reached_snap_position;
        let cap = gripping.then(|| Sprite {
            index: 7 + j,
            element: self.sprites.hand_cap_element[j],
            pos: pos + Vec2::new(0.0, if climbing { 0.0 } else { 3.0 }),
            rotation: 0.0,
            scale: Vec2::new(if j == 0 { 1.0 } else { -1.0 }, 1.0),
            anchor: CENTRE,
            color: self.body_color,
            shader: Shader::Basic,
            layer: SPRITE_LAYERS[7 + j],
        });
        (arm, cap)
    }
}
