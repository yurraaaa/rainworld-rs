//! `PlayerGraphics`: the slugcat's cosmetic simulation, ticked after the player.
//!
//! Behaviour: `rainworld-re/notes/slugcat-graphics.md` sections 1-3 (base game,
//! Remix off, Survivor stats), including the vertical-pole, ledge-grab, roll/flip,
//! corridor and swimming (`water.md`) branches. Not ported (docs/DIVERGENCES.md D8):
//! fear (no creature the slugcat fears is ported), the rain look point, the throw
//! reach (only weapons and frogs), zero gravity, pups, the Mark, DLC. `aerobicLevel` is not in the port and is 0.

use std::f32::consts::TAU;

use rw_assets::{Room, Terrain, tile_centre};
use rw_random::UnityRandom;
use rw_sim::creature::CreatureId;
use rw_sim::{Animation, BodyMode, Player, PlayerSurroundings, RoomExt, Vec2, Water};

use crate::body_part::{BodyPart, PointConnection};
use crate::draw::{Color, SlugcatName};
use crate::hand::{HandContext, HandGrasp, SlugcatHand};
use crate::looker::ObjectLooker;
use crate::math::{CustomVec, Mathf};
use crate::tail::{TailAnchor, TailSegment};

/// `Player.aerobicLevel`: not ported, so breathing stays at its resting pace.
pub(crate) const AEROBIC_LEVEL: f32 = 0.0;

/// `(rad, connectionRad, affectPrevious)` of the four base-game tail segments.
const TAIL_SEGMENTS: [(f32, f32, f32); 4] = [
    (6.0, 4.0, 1.0),
    (4.0, 7.0, 0.5),
    (2.5, 7.0, 0.5),
    (1.0, 7.0, 0.5),
];

/// `Custom.eightDirections`: W, SW, S, SE, E, NE, N, NW.
const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// Sprite values Futile keeps from frame to frame because the game only assigns them
/// in some states.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpriteMemory {
    pub legs_scale_x: f32,
    pub face_scale_x: f32,
    pub arm_scale_y: [f32; 2],
    pub hand_cap_element: [&'static str; 2],
}

impl Default for SpriteMemory {
    /// As `InitiateSprites` leaves them.
    fn default() -> Self {
        Self {
            legs_scale_x: 1.0,
            face_scale_x: 1.0,
            arm_scale_y: [-1.0, 1.0],
            hand_cap_element: [crate::elements::ON_TOP_OF_TERRAIN_HAND; 2],
        }
    }
}

/// The slugcat's graphics module.
///
/// Create it with the player, call [`PlayerGraphics::update`] once per tick right
/// after `Player::update` (and only when the player was updated), and
/// [`PlayerGraphics::draw`] once per rendered frame. Make a new one when the player
/// is replaced (respawn).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerGraphics {
    pub tail: [TailSegment; 4],
    pub hands: [SlugcatHand; 2],
    pub head: BodyPart,
    pub legs: BodyPart,
    /// `drawPositions[chunk][0]` this tick, `[chunk][1]` last tick: the chunk
    /// positions plus cosmetic offsets, anchors of the body and hips sprites.
    pub draw_positions: [[Vec2; 2]; 2],
    /// Unit vector the feet point along.
    pub legs_direction: Vec2,
    /// Beam balance wobble amount, 0..=120.
    pub disbalance_amount: f32,
    /// Beam balance phase, 0..300.
    pub balance_counter: f32,
    /// Distance fallen in the current airborne stretch (px).
    pub airborne_counter: f32,
    pub look_direction: Vec2,
    pub last_look_dir: Vec2,
    /// Eyes closed while above 0; counts down every tick.
    pub blink: i32,
    /// Breathing phase in cycles.
    pub breath: f32,
    pub last_breath: f32,
    /// Raised by input changes (0..=1).
    pub flail: f32,
    pub(crate) body_color: Color,
    pub(crate) sprites: SpriteMemory,
    /// `GraphicsModule.culled`: true until the first update, which then resets.
    pub(crate) culled: bool,
    /// The entrance the player was being pulled into at the end of the last tick.
    entering_shortcut: Option<(i32, i32)>,
    /// The player's corridor turn direction at the last update.
    corridor_turn_dir: Option<(i32, i32)>,
    pub looker: ObjectLooker,
    /// `spearDir`: leans the held objects forward while walking (-1..=1).
    pub spear_dir: f32,
}

impl PlayerGraphics {
    /// The `PlayerGraphics` constructor for a base-game slugcat: parts at the chunks
    /// with a random 1 px offset each.
    pub fn new(player: &Player, slugcat: SlugcatName, rng: &mut UnityRandom) -> Self {
        let (head_pos, hips_pos) = (player.head().pos, player.hips().pos);
        let tail = TAIL_SEGMENTS.map(|(rad, connection_rad, affect)| {
            let mut seg = TailSegment::new(rad, connection_rad, affect);
            seg.part.reset(hips_pos, rng);
            seg
        });
        let hands = [0, 1].map(|i| {
            let mut hand = SlugcatHand::new(i);
            hand.reset(head_pos, rng);
            hand.limb.absolute_hunt_pos = hand.limb.part.pos;
            hand
        });
        let mut head = BodyPart::new(4.0, 0.8, 0.99);
        head.reset(head_pos, rng);
        let mut legs = BodyPart::new(1.0, 0.8, 0.99);
        legs.reset(hips_pos, rng);
        let chunks = player.chunks();
        Self {
            tail,
            hands,
            head,
            legs,
            draw_positions: [
                [chunks[0].pos, chunks[0].last_pos],
                [chunks[1].pos, chunks[1].last_pos],
            ],
            legs_direction: Vec2::new(0.0, -1.0),
            disbalance_amount: 0.0,
            balance_counter: 0.0,
            airborne_counter: 0.0,
            look_direction: Vec2::ZERO,
            last_look_dir: Vec2::ZERO,
            blink: 0,
            breath: 0.0,
            last_breath: 0.0,
            flail: 0.0,
            body_color: Self::default_slugcat_color(slugcat),
            sprites: SpriteMemory::default(),
            culled: true,
            entering_shortcut: None,
            corridor_turn_dir: player.corridor_turn_dir(),
            looker: ObjectLooker::default(),
            spear_dir: 0.0,
        }
    }

    /// `PlayerGraphics.DefaultSlugcatColor` (Remix custom and Jolly colours off).
    #[must_use]
    pub fn default_slugcat_color(name: SlugcatName) -> Color {
        if name == SlugcatName::Yellow {
            Color::rgb(1.0, 1.0, 115.0 / 255.0)
        } else if name == SlugcatName::Red {
            Color::rgb(1.0, 115.0 / 255.0, 115.0 / 255.0)
        } else {
            Color::WHITE
        }
    }

    /// `PlayerGraphics.Reset`: draw positions at the chunks, every part at its chunk
    /// with a random 1 px offset, at rest.
    pub fn reset(&mut self, player: &Player, rng: &mut UnityRandom) {
        let (head_pos, hips_pos) = (player.head().pos, player.hips().pos);
        self.draw_positions = [[head_pos; 2], [hips_pos; 2]];
        for seg in &mut self.tail {
            seg.part.reset(hips_pos, rng);
        }
        self.head.reset(head_pos, rng);
        self.legs.reset(hips_pos, rng);
        for hand in &mut self.hands {
            hand.reset(head_pos, rng);
        }
    }

    /// One tick, after `Player::update`, following the game's calls around it.
    ///
    /// While the body is pulled into a shortcut the game runs an extra update from
    /// `Creature.Update` and drags the parts toward the hole; inside the tube nothing
    /// runs; leaving it resets (`SpitOutOfShortCut`) and starts a new sprite leaser.
    /// `water` is the room's water, as passed to `Player::update`.
    pub fn update(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    ) {
        if player.is_destroyed() {
            return;
        }
        self.player_pokes(player);
        if let Some(entrance) = self.entering_shortcut.take() {
            let seen = player.before_pull().unwrap_or(player);
            self.tick(seen, room, water, around, rng);
            let hole = room.shortcut_hole_direction(entrance.0, entrance.1);
            let target = Vec2::from(tile_centre(entrance.0, entrance.1))
                + Vec2::new(hole.0 as f32, hole.1 as f32) * -5.0;
            self.sucked_into_shortcut(target);
        }
        if player.in_shortcut() {
            return;
        }
        self.tick(player, room, water, around, rng);
        self.entering_shortcut = player.entering_shortcut();
    }

    /// Leaving a shortcut: the inner `Player.SpitOutOfShortCut` resets the module and
    /// `NewRoom` (always called, the creature having no room) resets it again; the
    /// sprites start afresh (a new sprite leaser). The outer `SpitOutOfShortCut`'s reset,
    /// after the carried objects come out, is [`Self::reset`].
    pub fn spat_out_of_shortcut(&mut self, player: &Player, rng: &mut UnityRandom) {
        for _ in 0..2 {
            self.reset(player, rng);
        }
        self.sprites = SpriteMemory::default();
    }

    /// `PlayerGraphics.LookAtObject`, or `LookAtNothing` for `None`.
    pub fn look_at(&mut self, target: Option<CreatureId>) {
        match target {
            Some(id) => self.looker.look_at_object(id),
            None => self.looker.look_at_nothing(),
        }
    }

    /// `PlayerGraphics.BiteFly(hand)` but its drips, which the sim makes: the head jerks
    /// toward the hand, the hand goes to the mouth, the eyes close. Returns `head.pos`,
    /// where the drips start.
    pub fn bite_fly(&mut self, hand: usize, fly: CreatureId) -> Vec2 {
        self.head.vel += self.head.pos.direction_to(self.hands[hand].limb.part.pos) * 2.0;
        self.draw_positions[0][0] += Vec2::new(0.0, 1.0);
        self.hands[hand].limb.part.pos = self.draw_positions[0][0];
        self.looker.look_at_object(fly);
        self.blink = self.blink.max(5);
        self.head.pos
    }

    /// The hands' (pos, vel) for `Player.GraphicsModuleUpdated`.
    #[must_use]
    pub fn hand_motion(&self) -> [(Vec2, Vec2); 2] {
        self.hands
            .each_ref()
            .map(|h| (h.limb.part.pos, h.limb.part.vel))
    }

    /// What `Player.Update` does to its graphics module directly: `Player.Blink`, and
    /// at the end of a corridor turn every body part's velocity is pushed 10 px/tick
    /// against the new direction.
    pub(crate) fn player_pokes(&mut self, player: &Player) {
        self.blink = self.blink.max(player.blink_request());
        if let Some((dx, dy)) = self.corridor_turn_dir
            && player.corridor_turn_dir().is_none()
        {
            let push = Vec2::new(dx as f32, dy as f32) * 10.0;
            for part in self.body_parts_mut() {
                part.vel -= push;
            }
        }
        self.corridor_turn_dir = player.corridor_turn_dir();
    }

    /// `bodyParts`: tail, hands, head, legs.
    pub(crate) fn body_parts_mut(&mut self) -> impl Iterator<Item = &mut BodyPart> {
        self.tail
            .iter_mut()
            .map(|seg| &mut seg.part)
            .chain(self.hands.iter_mut().map(|hand| &mut hand.limb.part))
            .chain([&mut self.head, &mut self.legs])
    }

    /// `PlayerGraphics.SuckedIntoShortCut`: tail, head and legs dragged toward `target`.
    pub fn sucked_into_shortcut(&mut self, target: Vec2) {
        for seg in &mut self.tail {
            let p = &mut seg.part;
            p.last_pos = p.pos;
            p.vel *= 0.5;
            p.pos = (p.pos * 5.0 + target) * (1.0 / 6.0);
        }
        for part in [&mut self.head, &mut self.legs] {
            part.vel *= 0.5;
            part.last_pos = part.pos;
            part.pos = part.pos.lerp(target, 1.0 / 3.0);
        }
    }

    /// `PlayerGraphics.Update` for the ported states.
    fn tick(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    ) {
        // `GraphicsModule.Update`: `ShouldBeCulled` is false for the player, so the
        // first update un-culls and resets.
        if self.culled {
            self.culled = false;
            self.reset(player, rng);
        }
        let input = player.input_history();
        if input[1].x != input[0].x || input[1].y != input[0].y {
            self.flail = (self.flail + 1.0 / 3.0).min(1.0);
        } else {
            self.flail = (self.flail - 0.0125).max(0.0);
        }
        self.last_breath = self.breath;
        if !player.is_dead() {
            self.breath += if player.sleeping() {
                0.0125
            } else {
                1.0 / 60.0_f32.lerp_to(15.0, AEROBIC_LEVEL.powf(1.5))
            };
        }
        self.update_spear_dir(player);
        if !player.is_conscious() {
            self.looker.look_at_nothing();
            self.blink = 10;
        }

        let (head, hips) = (player.head(), player.hips());
        for dp in &mut self.draw_positions {
            dp[1] = dp[0];
        }
        self.draw_positions[0][0] = head.pos;
        self.draw_positions[1][0] = hips.pos;
        let (fast, tail_drag) = self.body_mode_offsets(player);
        match player.animation() {
            Animation::Roll | Animation::Flip => self.fling_tail(head.pos.direction_to(hips.pos)),
            Animation::CorridorTurn => self.corridor_turn_jitter(rng),
            _ => {}
        }

        let no_contact = [head, hips]
            .iter()
            .all(|c| c.contact().x == 0 && c.contact().y == 0);
        if player.body_mode() == BodyMode::Default
            && player.animation() == Animation::None
            && no_contact
        {
            self.airborne_counter += head.vel.length();
        } else {
            self.airborne_counter = 0.0;
        }
        if player.body_mode() == BodyMode::ClimbingOnBeam
            && matches!(
                player.animation(),
                Animation::StandOnBeam | Animation::BeamTip
            )
        {
            self.beam_balance(player, rng);
        }
        self.update_tail(player, room, water, fast, tail_drag);

        self.blink -= 1;
        if self.blink < -rng.range_i32(2, 1800) {
            let max = rng.range_i32(3, 10);
            self.blink = rng.range_i32(3, max);
        }
        // Short of breath after a dive: the head bobs with the swim cycle, eyes shut.
        if !player.is_dead() && player.lungs_exhausted() {
            self.looker.look_at_nothing();
            let bob = (player.swim_cycle() * TAU).sin();
            self.head.vel.y += bob;
            self.draw_positions[0][0].y += bob * 2.5;
            self.blink = 1;
        }
        if rng.value() < 0.1 {
            self.update_looker(player, around, rng);
        }
        if rng.value() < 0.0025 {
            self.looker.look_at_nothing();
        }
        self.last_look_dir = self.look_direction;
        self.look_direction = if player.is_conscious() && self.looker.current.is_some() {
            let point = self.look_point(around);
            self.head.pos.direction_to(point)
        } else {
            self.look_direction * 0.0
        };
        if player.is_standing() {
            if player.input_history()[0].x == 0 {
                self.head.vel -= self.look_direction * 0.5;
            }
            self.draw_positions[0][0] -= self.look_direction * 2.0;
        } else {
            self.head.vel += self.look_direction;
        }

        self.update_head(player, room);
        self.update_legs(player, room);
        if player.is_conscious() {
            self.reach_for_fly(player, around);
        }
        self.update_hands(player, room, water, around, rng);
        if player.sleep_curl_up() > 0.0 {
            self.curl_up(player, rng);
        }
    }

    /// Curled up asleep (239358): body and hips drawn together and up, the tail wrapped
    /// round, the head tucked in, the eyes now and then shut (`PlayerBlink`), the hands
    /// in front.
    fn curl_up(&mut self, player: &Player, rng: &mut UnityRandom) {
        let curl = player.sleep_curl_up();
        let (head, hips) = (player.head().pos, player.hips().pos);
        let side = (head.x - hips.x).unity_sign();
        let middle = (head + hips) / 2.0;
        for dp in &mut self.draw_positions {
            dp[0] = dp[0].lerp_clamped(middle, curl * 0.2);
            dp[0].y += 2.0 * curl;
        }
        self.draw_positions[1][0].x -= 3.0 * side * curl;
        let hips_draw = self.draw_positions[1][0];
        let last = (self.tail.len() - 1) as f32;
        for (i, seg) in self.tail.iter_mut().enumerate() {
            let f = i as f32 / last;
            let part = &mut seg.part;
            part.vel *= 1.0 - 0.2 * curl;
            let wrap = Vec2::new(
                ((f * std::f32::consts::PI).mathf_sin() * 25.0 - f * 10.0) * -side,
                5.0_f32.lerp_to(-15.0, f),
            );
            part.pos = part.pos.lerp_clamped(hips_draw + wrap, 0.1 * curl);
        }
        self.head.vel *= 1.0 - 0.4 * curl;
        self.head.pos = self
            .head
            .pos
            .lerp_clamped(middle + Vec2::new(side * 5.0, -3.0), 0.5 * curl);
        if rng.value() < 1.0 / 30.0 {
            self.blink = self.blink.max(2);
        }
        for hand in &mut self.hands {
            hand.limb.absolute_hunt_pos = middle + Vec2::new(side * 10.0, -20.0);
        }
    }

    /// `spearDir` leans toward the held direction while standing, else back to 0.
    fn update_spear_dir(&mut self, player: &Player) {
        let x = player.input_history()[0].x;
        if player.body_mode() == BodyMode::Stand && x != 0 {
            self.spear_dir = (self.spear_dir + f32::from(x) * 0.1).clamp(-1.0, 1.0);
        } else if self.spear_dir < 0.0 {
            self.spear_dir = (self.spear_dir + 0.05).min(0.0);
        } else if self.spear_dir > 0.0 {
            self.spear_dir = (self.spear_dir - 0.05).max(0.0);
        }
    }

    /// `objectLooker.mostInterestingLookPoint`.
    fn look_point(&self, around: &PlayerSurroundings<'_>) -> Vec2 {
        self.looker
            .look_point(around, self.head.pos, self.look_direction)
    }

    /// The hands' block for food (239320-239348): hungry, close to a fly it could
    /// auto-grab and in sight, a hand whose other hand has reached its place reaches for
    /// it (not while moving away from it), the head and body leaning in.
    fn reach_for_fly(&mut self, player: &Player, around: &PlayerSurroundings<'_>) {
        let Some(fly) = self.looker.current else {
            return;
        };
        let climbing = player.body_mode() == BodyMode::ClimbingOnBeam
            && player.animation() != Animation::StandOnBeam;
        if !around.hungry || climbing || !around.world.fly_auto_grabable(fly) {
            return;
        }
        let point = self.look_point(around);
        let main = player.head().pos;
        let in_sight = around
            .world
            .realized(around.room)
            .is_some_and(|r| r.room.visual_contact(main, point));
        if !main.dist_less(point, 80.0) || !in_sight {
            return;
        }
        let mut hand = None;
        for i in 0..2 {
            if around.grasps[i].is_none() && self.hands[1 - i].limb.reached_snap_position {
                hand = Some(i);
            }
        }
        let x = player.input_history()[0].x;
        if x != 0 && (point.x < main.x) == (x > 0) {
            hand = None;
        }
        let Some(i) = hand else { return };
        self.hands[i].reaching_for_object = true;
        self.hands[i].limb.absolute_hunt_pos = point;
        // `num3` (fear) is 0: the head and body lean in.
        self.draw_positions[0][0] += self.draw_positions[0][0].direction_to(point) * 5.0;
        self.head.vel += self.draw_positions[0][0].direction_to(point) * 2.0;
    }

    /// The body-mode block of `PlayerGraphics.Update`: draw offsets and pokes at the
    /// head and tail. Returns `(fast, tail drag)`, the running flag and the tail's
    /// starting drag factor (`num5`).
    fn body_mode_offsets(&mut self, player: &Player) -> (bool, f32) {
        let (head, hips) = (player.head(), player.hips());
        let flip = player.flip_direction() as f32;
        let frame = player.animation_frame();
        let mut fast = false;
        let mut tail_drag = 1.0;
        match player.body_mode() {
            BodyMode::Stand => {
                let [d0, d1] = Self::stand_offsets(flip, frame, hips.vel.x);
                self.draw_positions[0][0] += d0;
                self.draw_positions[1][0] += d1;
                fast = head.vel.x.abs() > 2.0 && hips.vel.x.abs() > 2.0;
                tail_drag = 1.0 - ((hips.vel.x.abs() - 1.0) * 0.5).clamp(0.0, 1.0);
            }
            BodyMode::Crawl => {
                let [d0, d1] = Self::crawl_offsets(flip, frame, player.super_launch_jump());
                self.draw_positions[0][0] += d0;
                self.draw_positions[1][0] += d1;
                let s = (frame as f32 / 21.0 * TAU).sin();
                self.head.vel.y -= s * -0.5 - 0.5;
                self.head.vel.x += if head.pos.x < hips.pos.x { -1.0 } else { 1.0 };
            }
            BodyMode::ClimbingOnBeam => match player.animation() {
                Animation::GetUpOnBeam => self.disbalance_amount = 70.0,
                Animation::GetUpToBeamTip => self.disbalance_amount = 120.0,
                Animation::StandOnBeam => {
                    self.draw_positions[1][0].y += 3.0;
                    fast = head.vel.x.abs() > 2.0 && hips.vel.x.abs() > 2.0;
                    tail_drag = 1.0 - ((hips.vel.x.abs() - 1.0) * 0.3).clamp(0.0, 1.0);
                    if fast {
                        self.tail[0].part.vel.x -= head.vel.x * 2.0;
                        self.tail[0].part.vel.y += 1.5;
                        self.tail[1].part.vel.x -= head.vel.x * 0.2;
                        self.tail[1].part.vel.y += 0.5;
                    }
                }
                Animation::ClimbOnBeam => {
                    let phase = frame as f32 / 20.0 * TAU;
                    self.draw_positions[0][0].x += flip * 2.5 + flip * 0.5 * phase.sin();
                    self.draw_positions[1][0].x += flip * 2.5 * phase.cos();
                }
                _ => {}
            },
            BodyMode::WallClimb => {
                self.legs_direction.y -= 1.0;
                self.draw_positions[0][0].y += 2.0;
                let push = if hips.contact().y < 0 { 3.0 } else { 5.0 };
                self.draw_positions[0][0].x -= flip * push;
                self.head.vel.y -= flip * 5.0;
            }
            BodyMode::Default if player.animation() == Animation::LedgeGrab => {
                self.legs_direction.y -= 1.0;
                self.draw_positions[0][0].x -= flip * 5.0;
            }
            // Other Default animations: the tail hangs loose.
            BodyMode::Default => tail_drag = 0.0,
            BodyMode::Swimming
                if player.animation() == Animation::DeepSwim
                    || player.input_history()[0].x != 0 =>
            {
                let side = head.pos.direction_to(hips.pos).perpendicular();
                self.draw_positions[1][0] += side * ((player.swim_cycle() * TAU).sin() * 5.0);
            }
            _ => {}
        }
        (fast, tail_drag)
    }

    /// Rolls and flips: each tail segment is pushed along `dir` (head to hips), 6 px/tick
    /// at the root, divided by 1.7 per segment.
    pub fn fling_tail(&mut self, dir: Vec2) {
        let mut push = 6.0;
        for seg in &mut self.tail {
            seg.part.vel += dir * push;
            push /= 1.7;
        }
    }

    fn update_looker(
        &mut self,
        player: &Player,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    ) {
        self.looker.update(around, player.head().pos, rng);
    }

    /// CorridorTurn: the body and hips draw positions jump up to 3 and 2 px in random
    /// directions every tick, eyes shut.
    fn corridor_turn_jitter(&mut self, rng: &mut UnityRandom) {
        for (dp, reach) in self.draw_positions.iter_mut().zip([3.0, 2.0]) {
            let dir = Vec2::random_unit(rng);
            dp[0] += dir * (reach * rng.value());
        }
        self.blink = 5;
    }

    /// Standing gait offsets of `[body, hips]` draw positions for facing `flip`,
    /// `animationFrame` `frame` (0..=6) and hips speed `hips_vel_x`.
    #[must_use]
    pub fn stand_offsets(flip: f32, frame: i32, hips_vel_x: f32) -> [Vec2; 2] {
        let phase = frame as f32 / 6.0 * TAU;
        [
            Vec2::new(
                flip * 6.0 * (hips_vel_x.abs() - 0.2).clamp(0.0, 1.0),
                phase.cos() * 2.0,
            ),
            Vec2::new(-flip * (1.5 - frame as f32 / 6.0), 2.0 + phase.sin() * 4.0),
        ]
    }

    /// Crawling gait offsets of `[body, hips]` draw positions; a charged super jump
    /// (`super_launch_jump > 19`) lowers the hips 3 px less.
    #[must_use]
    pub fn crawl_offsets(flip: f32, frame: i32, super_launch_jump: i32) -> [Vec2; 2] {
        let s = (frame as f32 / 21.0 * TAU).sin();
        let c = (frame as f32 / 14.0 * TAU).cos();
        let boost = if super_launch_jump > 19 { 0.0 } else { 1.0 };
        [
            Vec2::new(c * flip * 2.0, -(s * -1.5 - 3.0)),
            Vec2::new(-3.0 * s * flip, -(c * 1.5 - 7.0 + 3.0 * boost)),
        ]
    }

    /// Wobble while standing on a beam or pole tip: disbalance grows when moving fast
    /// (much faster on a tip) and decays otherwise; the balance counter advances faster
    /// when disbalanced.
    fn beam_balance(&mut self, player: &Player, rng: &mut UnityRandom) {
        let (head, hips) = (player.head(), player.hips());
        if head.vel.x.abs() > 2.0 {
            self.disbalance_amount += if player.animation() == Animation::BeamTip {
                17.0
            } else {
                3.0
            };
        } else {
            self.disbalance_amount -= 1.0;
        }
        self.disbalance_amount = self.disbalance_amount.clamp(0.0, 120.0);
        self.balance_counter += 1.0 + self.disbalance_amount / 40.0 * (1.0 + rng.value());
        if self.balance_counter > 300.0 {
            self.balance_counter -= 300.0;
        }
        let wobble = (self.balance_counter / 300.0 * TAU).sin() / (hips.vel.x.abs() + 1.0);
        let d = self.disbalance_amount;
        self.draw_positions[0][0].x += wobble * (d + 20.0) * 0.08;
        self.draw_positions[0][0].y += wobble * d * 0.02;
        self.tail[0].part.vel.x += wobble * (d + 20.0) * 0.1;
        self.tail[1].part.vel.x += wobble * (d + 20.0) * 0.04;
    }

    /// The tail loop: each segment updates against its anchor, is damped and pulled
    /// down by `tail_drag` (both fading as the hips sink in water), kept within
    /// `9 * (l + 1)` px of the hips, and pushed away from the body by a force halving
    /// per segment.
    fn update_tail(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        fast: bool,
        mut tail_drag: f32,
    ) {
        let hips = player.hips();
        let flip = player.flip_direction() as f32;
        let mut push_from = if fast {
            hips.pos + Vec2::new(flip * 16.0 * (hips.vel.x.abs() - 0.2).clamp(0.0, 1.0), -4.0)
        } else {
            player.head().pos
        };
        let mut next_push_from = hips.pos;
        let mut push = 28.0;
        let dry = 1.0 - hips.submersion(water);
        // Room gravity is 1.
        for l in 0..self.tail.len() {
            let (before, rest) = self.tail.split_at_mut(l);
            let anchor = match before.last_mut() {
                Some(prev) => TailAnchor::Segment(&mut prev.part),
                None => TailAnchor::Point(self.draw_positions[1][0]),
            };
            rest[0].update(room, anchor);
            let p = &mut rest[0].part;
            p.vel *= 0.75_f32.lerp_to(0.95, tail_drag * dry);
            p.vel.y -= 0.1_f32.lerp_to(0.5, tail_drag) * dry;
            tail_drag = (tail_drag * 10.0 + 1.0) / 11.0;
            let reach = 9.0 * (l + 1) as f32;
            if p.pos.distance(hips.pos) >= reach {
                p.pos = hips.pos + hips.pos.direction_to(p.pos) * reach;
            }
            p.vel += push_from.direction_to(p.pos) * push * (1.0 / push_from.distance(p.pos));
            push *= 0.5;
            push_from = next_push_from;
            next_push_from = p.pos;
        }
    }

    /// The head hangs between the draw positions, offset toward the body's front.
    fn update_head(&mut self, player: &Player, room: &Room) {
        let [d0, d1] = [self.draw_positions[0][0], self.draw_positions[1][0]];
        let lead = Self::head_lead(player.body_mode(), d0, d1);
        self.head.update_free(room);
        let hanging = player.animation() == Animation::HangFromBeam;
        self.head.connect_to_point(PointConnection {
            point: d0.lerp(d1, 0.2) + lead,
            rad: if hanging { 0.0 } else { 3.0 },
            push: false,
            elastic: 0.2,
            host_vel: player.head().vel,
            adapt_vel: 0.7,
            exaggerate_vel: 0.1,
        });
    }

    /// How far the head's goal leads the body draw position `d0` away from the hips
    /// `d0`..`d1`: 3 px along the body, sideways 2.5x while crawling, downward 2x in a
    /// corridor.
    #[must_use]
    pub fn head_lead(body_mode: BodyMode, d0: Vec2, d1: Vec2) -> Vec2 {
        let mut lead = d1.direction_to(d0) * 3.0;
        if body_mode == BodyMode::Crawl {
            lead.x *= 2.5;
        } else if body_mode == BodyMode::CorridorClimb && lead.y < 0.0 {
            lead.y *= 2.0;
        }
        lead
    }

    /// The legs stand on the ground under the hips, hang below a pole tip, grip the
    /// pole below the head while climbing it, or dangle and swing with the hips'
    /// velocity; `legs_direction` follows.
    fn update_legs(&mut self, player: &Player, room: &Room) {
        let (head, hips) = (player.head(), player.hips());
        self.legs.update_free(room);
        let animation = player.animation();
        let connection = PointConnection {
            point: hips.pos,
            rad: 4.0,
            push: false,
            elastic: 0.25,
            host_vel: Vec2::new(hips.vel.x, -10.0),
            adapt_vel: 0.5,
            exaggerate_vel: 0.1,
        };
        if player.body_mode() == BodyMode::CorridorClimb {
            self.legs.connect_to_point(PointConnection {
                point: hips.pos + head.pos.direction_to(hips.pos) * 4.0,
                rad: 2.0,
                host_vel: hips.vel,
                ..connection
            });
            let turn = Self::corridor_legs_turn(room, player);
            self.legs_direction += Vec2::from_degrees(head.pos.aim_to(hips.pos) + turn);
        } else if hips.contact().y == -1 || animation == Animation::StandOnBeam {
            self.legs.connect_to_point(PointConnection {
                point: hips.pos + Vec2::new(self.legs_direction.x * 8.0, 1.0),
                rad: 5.0,
                ..connection
            });
            self.legs_direction.x -= f32::from(hips.on_slope());
            self.legs_direction.y -= 1.0;
        } else if animation == Animation::BeamTip {
            self.legs.connect_to_point(PointConnection {
                point: hips.pos + Vec2::new(0.0, -8.0),
                rad: 0.0,
                host_vel: Vec2::new(0.0, -10.0),
                ..connection
            });
            let below = hips.pos.tile_middle() + Vec2::new(0.0, -10.0);
            self.legs_direction += self.draw_positions[0][0].direction_to(below);
        } else if animation == Animation::ClimbOnBeam {
            let flip = player.flip_direction() as f32;
            let phase = player.animation_frame() as f32 / 20.0 * TAU;
            let grip = Vec2::new(-flip * (5.0 - phase.sin()), -16.0 - 5.0 * phase.cos());
            self.legs.connect_to_point(PointConnection {
                point: head.pos + grip,
                rad: 0.0,
                host_vel: Vec2::ZERO,
                ..connection
            });
            self.legs_direction.y -= 1.0;
        } else {
            let offset_y = if animation == Animation::HangFromBeam {
                -5.0
            } else {
                -2.0
            };
            self.legs.connect_to_point(PointConnection {
                point: hips.pos + Vec2::new(self.legs_direction.x * 8.0, offset_y),
                ..connection
            });
            self.legs_direction += hips.vel * 0.01;
            self.legs_direction.y -= 0.05;
        }
        self.legs_direction = self.legs_direction.normalized();
    }

    /// The feet's angle (degrees) from the body's head-to-hips axis in a corridor: the
    /// first of four neighbour pairs around the hips tile, counted from the pair across
    /// the body, that is solid on both sides, preferring straight along (0), then the
    /// diagonal on the facing side (45 or -45), the other diagonal, then across (+-90).
    #[must_use]
    pub fn corridor_legs_turn(room: &Room, player: &Player) -> f32 {
        let (head, hips) = (player.head().pos, player.hips().pos);
        let flip = player.flip_direction();
        let base = ((270.0 - hips.aim_to(head)) / 45.0).round_to_int();
        let (tx, ty) = hips.tile();
        let mut best = 10;
        let mut turn = 0.0;
        for m in 0..4 {
            let solid = [10, 14].into_iter().all(|k| {
                let (dx, dy) = EIGHT_DIRECTIONS[(m + base + k).rem_euclid(8) as usize];
                room.tile(tx + dx, ty + dy).terrain == Terrain::Solid
            });
            if !solid {
                continue;
            }
            let rank = match m {
                1 => {
                    if flip == -1 {
                        1
                    } else {
                        2
                    }
                }
                3 => {
                    if flip == 1 {
                        1
                    } else {
                        2
                    }
                }
                2 => 3,
                _ => 0,
            };
            if rank < best {
                best = rank;
                turn = match m {
                    1 => 45.0,
                    2 => {
                        if flip == -1 {
                            -90.0
                        } else {
                            90.0
                        }
                    }
                    3 => -45.0,
                    _ => 0.0,
                };
            }
        }
        turn
    }

    /// Both hands, hand 0 first (hand 1 reads where hand 0 went).
    fn update_hands(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    ) {
        let grasps = HandGrasp::of(around);
        for i in 0..self.hands.len() {
            let hand0 = &self.hands[0].limb;
            let mut cx = HandContext {
                player,
                room,
                water,
                balance_counter: self.balance_counter,
                disbalance: &mut self.disbalance_amount,
                head_vel: &mut self.head.vel,
                airborne_counter: self.airborne_counter,
                hand0: (hand0.part.pos, hand0.reached_snap_position),
                rng: &mut *rng,
                grasps,
                spear_dir: self.spear_dir,
                draw_pos0: &mut self.draw_positions[0][0],
            };
            let hand = &mut self.hands[i];
            hand.update(&mut cx);
            let pos = hand.limb.part.pos;
            if pos.x.is_nan() || pos.y.is_nan() {
                hand.limb.part.pos = player.head().pos;
            }
        }
    }
}
