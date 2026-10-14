//! `SlugcatHand`: a hand on the head chunk that reaches for terrain and beams while
//! the body mode needs it and retracts (hidden) otherwise.

use rw_assets::{Room, Terrain};
use rw_random::UnityRandom;
use rw_sim::{Animation, BodyMode, HEAD, HIPS, Player, Vec2, Water};

use crate::body_part::{BodyPart, PointConnection};
use crate::limb::{GripSearch, Limb, LimbMode};
use crate::math::{CustomVec, Mathf};
use std::f32::consts::TAU;

/// One slugcat hand.
#[derive(Debug, Clone, PartialEq)]
pub struct SlugcatHand {
    pub limb: Limb,
    /// Ticks of wanting to retract; lowers the shoulder spread while drawn.
    pub retract_counter: i32,
}

/// Graphics state a hand reads or pokes while it updates.
pub(crate) struct HandContext<'a> {
    pub player: &'a Player,
    pub room: &'a Room,
    pub water: Option<&'a Water>,
    pub balance_counter: f32,
    pub disbalance: &'a mut f32,
    pub head_vel: &'a mut Vec2,
    pub airborne_counter: f32,
    /// Hand 0's position and `reached_snap_position`, read by hand 1 while crawling.
    pub hand0: (Vec2, bool),
    pub rng: &'a mut UnityRandom,
}

impl SlugcatHand {
    /// Hand `limb_number` (rad 3, surface friction 0.8, hunt speed 7, quickness 0.5).
    #[must_use]
    pub fn new(limb_number: usize) -> Self {
        Self {
            limb: Limb::new(limb_number, BodyPart::new(3.0, 0.8, 1.0), 7.0, 0.5),
            retract_counter: 0,
        }
    }

    /// `Limb.Reset` at `point`; the hunt target keeps its old value, as in the game.
    pub fn reset(&mut self, point: Vec2, rng: &mut UnityRandom) {
        self.limb.part.reset(point, rng);
    }

    /// `SlugcatHand.Update` with empty grasps, base game: hunt, stay within 20 px of
    /// the head chunk, then follow the body mode or retract after 5 idle ticks.
    pub(crate) fn update(&mut self, cx: &mut HandContext<'_>) {
        let head = cx.player.head();
        self.limb.update(cx.room, head, cx.player.hips().pos);
        self.limb.part.connect_to_point(PointConnection {
            point: head.pos,
            rad: 20.0,
            push: false,
            elastic: 0.0,
            host_vel: head.vel,
            adapt_vel: 0.0,
            exaggerate_vel: 0.0,
        });
        let idle = self.engage_in_movement(cx);
        if idle && self.limb.mode != LimbMode::Retracted {
            self.retract_counter += 1;
            if self.retract_counter > 5 {
                let head_pos = cx.player.head().pos;
                self.limb.mode = LimbMode::HuntAbsolutePosition;
                let pull = ((self.retract_counter as f32 - 5.0) * 0.05).clamp(0.0, 1.0);
                self.limb.part.pos = self.limb.part.pos.lerp(head_pos, pull);
                if self.limb.part.pos.distance(head_pos) < 2.0 && self.limb.reached_snap_position {
                    self.limb.mode = LimbMode::Retracted;
                }
                self.limb.absolute_hunt_pos = head_pos;
                self.limb.hunt_speed = 1.0 + self.retract_counter as f32 * 0.2;
                self.limb.quickness = 1.0;
            }
        } else {
            self.retract_counter = (self.retract_counter - 10).max(0);
        }
    }

    /// `SlugcatHand.EngageInMovement` for the ported body modes and animations: sets
    /// the hand's target and returns whether the hand is free (true = retract).
    fn engage_in_movement(&mut self, cx: &mut HandContext<'_>) -> bool {
        let player = cx.player;
        let head = player.head();
        let flip = player.flip_direction();
        let limb = self.limb.limb_number;
        let sign = if limb == 0 { -1.0 } else { 1.0 };
        let mut free = true;
        match player.body_mode() {
            BodyMode::ClimbingOnBeam => {
                free = false;
                match player.animation() {
                    Animation::ClimbOnBeam => self.climb_pole_target(player),
                    Animation::HangFromBeam | Animation::GetUpOnBeam => {
                        self.hang_from_beam_target(cx.room, player);
                    }
                    Animation::StandOnBeam | Animation::BeamTip => {
                        free = self.stand_on_beam_target(cx);
                    }
                    Animation::HangUnderVerticalBeam => {
                        self.limb.mode = LimbMode::HuntAbsolutePosition;
                        let (dx, dy) = if limb == 0 { (-0.5, 20.0) } else { (0.5, 25.0) };
                        self.limb.absolute_hunt_pos = head.pos.tile_middle() + Vec2::new(dx, dy);
                        self.limb.hunt_speed = 10.0;
                        self.limb.quickness = 1.0;
                    }
                    _ => {}
                }
            }
            BodyMode::Crawl => {
                free = false;
                self.limb.mode = LimbMode::HuntAbsolutePosition;
                self.limb.hunt_speed = 12.0;
                self.limb.quickness = 0.7;
                let (hand0_pos, hand0_reached) = cx.hand0;
                let leading =
                    limb == 0 || ((hand0_pos.x - head.pos.x).abs() < 10.0 && hand0_reached);
                if leading && head.pos.distance(self.limb.absolute_hunt_pos) >= 29.0 {
                    let reach_from = head.pos + Vec2::new(flip as f32 * 20.0, 0.0);
                    self.limb.find_grip(
                        cx.room,
                        GripSearch {
                            attached: reach_from,
                            search_from: reach_from,
                            max_radius: 100.0,
                            goal: Vec2::new(
                                head.pos.x + flip as f32 * 28.0,
                                head.pos.tile_middle().y - 10.0,
                            ),
                            forbidden_x: 2,
                            forbidden_y: 1,
                            behind_walls: false,
                        },
                    );
                }
            }
            BodyMode::CorridorClimb => {
                free = false;
                self.limb.mode = LimbMode::HuntAbsolutePosition;
                if self.limb.part.pos.dist_no_sqrt(head.pos) >= 20.0 * 20.0 {
                    self.corridor_grip(cx.room, player);
                }
            }
            BodyMode::WallClimb => {
                free = false;
                self.limb.mode = LimbMode::HuntAbsolutePosition;
                self.limb.absolute_hunt_pos.x = head.pos.tile_middle().x + flip as f32 * 10.0;
                self.limb.absolute_hunt_pos.y = if (limb == 0) == (flip == -1) {
                    head.pos.y - 7.0
                } else {
                    head.pos.y + 3.0
                };
            }
            BodyMode::Swimming => {
                free = false;
                self.swim_target(player, cx.water, sign);
            }
            BodyMode::Default
                if cx.airborne_counter > 180.0 && head.vel.distance(Vec2::ZERO) >= 4.0 =>
            {
                free = false;
                self.retract_counter = 0;
                self.flail_target(cx, sign);
            }
            _ => {}
        }
        if matches!(
            player.animation(),
            Animation::DownOnFours | Animation::CrawlTurn
        ) {
            free = false;
            self.limb.mode = LimbMode::HuntAbsolutePosition;
            let dir = head.vel.normalized();
            self.limb.absolute_hunt_pos = head.pos
                + Vec2::new(
                    -6.0 + 12.0 * limb as f32 + dir.x * 20.0,
                    dir.y.abs() * -20.0,
                );
        } else if player.animation() == Animation::LedgeGrab {
            // The hunt mode is left as it was.
            free = false;
            let mut target = head.pos.tile_middle();
            if (limb == 0) == (flip == -1) {
                target += Vec2::new(flip as f32 * 10.0, -10.0);
            } else {
                target += Vec2::new(flip as f32 * 15.0, 10.0);
            }
            self.limb.absolute_hunt_pos = target;
        }
        free
    }

    /// CorridorClimb, hand stretched 20 px from the head: grip the corridor wall about
    /// 20 px ahead (along the body, bent toward the held direction), hand 0 to the
    /// body's left, hand 1 to its right.
    fn corridor_grip(&mut self, room: &Room, player: &Player) {
        let head = player.head().pos;
        let along = player.hips().pos.direction_to(head);
        let input = player.input_history()[0];
        let held = Vec2::new(input.x as f32, input.y as f32).normalized();
        let side = 6.0 - 12.0 * self.limb.limb_number as f32;
        let goal = head + (along + held * 1.5).normalized() * 20.0 + along.perpendicular() * side;
        self.limb.find_grip(
            room,
            GripSearch {
                attached: head,
                search_from: head,
                max_radius: 100.0,
                goal,
                forbidden_x: 2,
                forbidden_y: 2,
                behind_walls: false,
            },
        );
    }

    /// Swimming: under water the hands sweep from forward to back along the stroke
    /// (relative to the body); at the surface they paddle around a point 7 px under it.
    fn swim_target(&mut self, player: &Player, water: Option<&Water>, sign: f32) {
        let cycle = player.swim_cycle();
        let stroke = if cycle < 3.0 {
            cycle / 3.0
        } else {
            1.0 - (cycle - 3.0)
        };
        if player.animation() == Animation::DeepSwim {
            self.limb.mode = LimbMode::HuntRelativePosition;
            let reach = (1.0_f32 - stroke.inverse_lerp(0.5, 1.0)).powf(1.5);
            let mut target = Vec2::from_degrees((20.0 + reach * 140.0) * sign) * 20.0;
            if cycle < 3.0 {
                target.x *= 0.5;
            }
            self.limb.relative_hunt_pos = target;
        } else {
            self.limb.mode = LimbMode::HuntAbsolutePosition;
            let x = f32::from(player.input_history()[0].x);
            let head = player.head().pos;
            let sweep = x * 10.0 * -(stroke * TAU).sin();
            let mut target = self.limb.absolute_hunt_pos;
            target.x = head.x + (sign * 30.0).lerp_to(sweep, 0.5);
            target.y = water.map_or(f32::MIN, |w| w.level_at(target.x)) - 7.0;
            let (turn, radius) = if x == 0.0 { (1.0, 5.0) } else { (-x, 10.0) };
            target += Vec2::from_degrees(360.0 * stroke * turn) * (sign * radius);
            self.limb.absolute_hunt_pos = target;
        }
        self.limb.hunt_speed = 5.0;
        self.limb.quickness = 0.5;
    }

    /// ClimbOnBeam: the hands grip the pole above and below the head in turn, by the
    /// animation frame.
    fn climb_pole_target(&mut self, player: &Player) {
        let head = player.head().pos;
        let flip = player.flip_direction();
        let near_side = (self.limb.limb_number == 1) == (flip == 1);
        let phase = player.animation_frame() as f32 / 20.0 * std::f32::consts::TAU;
        let sway = if near_side { phase.sin() } else { phase.cos() };
        self.limb.mode = LimbMode::HuntAbsolutePosition;
        let (dy, dx) = if near_side {
            (-3.0, -flip as f32)
        } else {
            (3.0, flip as f32)
        };
        self.limb.absolute_hunt_pos =
            Vec2::new(head.tile_middle().x + dx, head.y + dy + 6.0 * sway);
        self.retract_counter = 40;
    }

    /// HangFromBeam / GetUpOnBeam: the hands grip the beam either side of the head,
    /// swaying with the animation frame; inside a wall or off the beam they snap to
    /// the tile edges.
    fn hang_from_beam_target(&mut self, room: &Room, player: &Player) {
        let head = player.head().pos;
        let side = if self.limb.limb_number == 0 {
            -1.0
        } else {
            1.0
        };
        let sway = (std::f32::consts::TAU * player.animation_frame() as f32 / 20.0).sin();
        self.limb.mode = LimbMode::HuntAbsolutePosition;
        let mut target = Vec2::new(head.x, head.tile_middle().y - 1.0);
        target.x += side * (10.0 + 3.0 * sway);
        let tile = room.tile_at(target.to_array());
        if tile.terrain == Terrain::Solid || !tile.horizontal_pole() {
            target.x = head.tile_middle().x + side * 10.0;
        }
        self.limb.absolute_hunt_pos = target;
    }

    /// StandOnBeam: arms out for balance (relative target swaying with the balance
    /// counter), gripping the beam when one is in reach; folded in while balanced.
    /// Returns whether the hand is free.
    fn stand_on_beam_target(&mut self, cx: &mut HandContext<'_>) -> bool {
        let player = cx.player;
        let head = player.head().pos;
        let limb = self.limb.limb_number;
        let flip = player.flip_direction();
        self.limb.mode = LimbMode::HuntRelativePosition;
        let sway = (std::f32::consts::TAU * cx.balance_counter / 300.0).sin();
        let side = if limb == 0 { -1.0 } else { 1.0 };
        self.limb.relative_hunt_pos =
            Vec2::new(-20.0 + 40.0 * limb as f32, -4.0 - 6.0 * sway * side);
        let goal_y = if (limb == 0) == (flip == -1) {
            0.0
        } else {
            -5.0
        };
        self.limb.find_grip(
            cx.room,
            GripSearch {
                attached: head,
                search_from: head,
                max_radius: 100.0,
                goal: head + Vec2::new(-10.0 + 20.0 * limb as f32, goal_y),
                forbidden_x: if limb == 0 { 1 } else { -1 },
                forbidden_y: -1,
                behind_walls: false,
            },
        );
        let mut free = false;
        if self.limb.mode == LimbMode::HuntAbsolutePosition {
            if self.limb.part.pos.y > head.y + 5.0 {
                cx.head_vel.x += 2.0 - 4.0 * limb as f32;
            }
            if (head.x - player.hips().pos.x).abs() < 10.0 {
                *cx.disbalance -= 8.0;
            }
        } else if *cx.disbalance < 40.0 {
            let fold = (40.0 - *cx.disbalance) / 40.0;
            let rel = &mut self.limb.relative_hunt_pos;
            rel.y *= 1.0 - fold;
            rel.y -= fold * 15.0;
            rel.x *= 1.0 - fold;
            if fold >= 1.0 {
                free = true;
            }
        }
        self.limb.hunt_speed = 5.0;
        self.limb.quickness = 0.2;
        free
    }

    /// A long fall: the hands grab at random terrain ahead, or flail relative to the
    /// body scaled by the speed.
    fn flail_target(&mut self, cx: &mut HandContext<'_>, sign: f32) {
        let (head, hips) = (&cx.player.chunks()[HEAD], &cx.player.chunks()[HIPS]);
        let jitter = Vec2::random_unit(cx.rng) * (40.0 * cx.rng.value());
        let ahead = head.pos + head.vel * 4.0 + jitter;
        if cx.room.tile_at(ahead.to_array()).terrain != Terrain::Air {
            self.limb.mode = LimbMode::HuntAbsolutePosition;
            self.limb.absolute_hunt_pos = ahead;
            self.limb.hunt_speed = 18.0;
            self.limb.quickness = 1.0;
            return;
        }
        self.limb.mode = LimbMode::HuntRelativePosition;
        let upright = head.pos.y > hips.pos.y + 5.0;
        let rel = &mut self.limb.relative_hunt_pos;
        rel.x = ((head.vel.x * if upright { 2.1 } else { 1.0 }).abs() + 4.0) * sign;
        rel.y = head.vel.y * if upright { -3.0 } else { -0.9 } + (head.vel.x * 0.6).abs() + 1.0;
        let speed = head.vel.length();
        if speed > 10.0 {
            let a = Vec2::random_unit(cx.rng);
            *rel += a * (cx.rng.value() * cx.rng.value() * speed);
        }
        self.limb.hunt_speed = 8.0;
        self.limb.quickness = 0.6;
    }
}
