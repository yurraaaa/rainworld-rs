//! `SlugcatHand`: a hand on the head chunk that reaches for terrain and beams while
//! the body mode needs it and retracts (hidden) otherwise.

use rand::Rng;
use rw_assets::{Room, Terrain};
use rw_sim::{Animation, BodyMode, HEAD, HIPS, Player, Vec2};

use crate::body_part::{BodyPart, PointConnection};
use crate::limb::{GripSearch, Limb, LimbMode};
use crate::math::{CustomVec, UnityRandom};

/// One slugcat hand.
#[derive(Debug, Clone, PartialEq)]
pub struct SlugcatHand {
    pub limb: Limb,
    /// Ticks of wanting to retract; lowers the shoulder spread while drawn.
    pub retract_counter: i32,
}

/// Graphics state a hand reads or pokes while it updates.
pub(crate) struct HandContext<'a, R: Rng + ?Sized> {
    pub player: &'a Player,
    pub room: &'a Room,
    pub balance_counter: f32,
    pub disbalance: &'a mut f32,
    pub head_vel: &'a mut Vec2,
    pub airborne_counter: f32,
    /// Hand 0's position and `reached_snap_position`, read by hand 1 while crawling.
    pub hand0: (Vec2, bool),
    pub rng: &'a mut R,
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
    pub fn reset<R: Rng + ?Sized>(&mut self, point: Vec2, rng: &mut R) {
        self.limb.part.reset(point, rng);
    }

    /// `SlugcatHand.Update` with empty grasps, base game: hunt, stay within 20 px of
    /// the head chunk, then follow the body mode or retract after 5 idle ticks.
    pub(crate) fn update<R: Rng + ?Sized>(&mut self, cx: &mut HandContext<'_, R>) {
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
    fn engage_in_movement<R: Rng + ?Sized>(&mut self, cx: &mut HandContext<'_, R>) -> bool {
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
                    Animation::HangFromBeam | Animation::GetUpOnBeam => {
                        self.hang_from_beam_target(cx.room, player);
                    }
                    Animation::StandOnBeam => free = self.stand_on_beam_target(cx),
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
        }
        free
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
    fn stand_on_beam_target<R: Rng + ?Sized>(&mut self, cx: &mut HandContext<'_, R>) -> bool {
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
    fn flail_target<R: Rng + ?Sized>(&mut self, cx: &mut HandContext<'_, R>, sign: f32) {
        let (head, hips) = (&cx.player.chunks()[HEAD], &cx.player.chunks()[HIPS]);
        let jitter = Vec2::from_deg(cx.rng.value() * 360.0) * (40.0 * cx.rng.value());
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
            let a = Vec2::from_deg(cx.rng.value() * 360.0);
            *rel += a * (cx.rng.value() * cx.rng.value() * speed);
        }
        self.limb.hunt_speed = 8.0;
        self.limb.quickness = 0.6;
    }
}
