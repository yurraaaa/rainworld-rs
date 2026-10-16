//! `LizardGraphics` (decompile `LizardGraphics.cs`): the lizard's legs, tail and head
//! as cosmetic body parts. Its legs' grip is the body's `LegsGripping`, so its `Update`
//! is gameplay: it runs after the lizard's in its room's update list and draws on the
//! shared stream (culling's `Reset`, the leg rolls and sounds, the eyes, the blink, the
//! look focus, bubbles). The constructor draws on a stream seeded by the creature's ID.
//! Drawing is in `lizard_draw.rs`.

use rw_assets::{CreatureType, Room};
use rw_random::UnityRandom;
use rw_sim::color::RandomVariation;
use rw_sim::creature::lizard::{
    BubbleHead, LightUpkeep, LizardBubble, LizardGraphicsLink, LizardGraphicsTick, LizardHead,
    LizardSpawn, LizardView, LizardVoice, LizardVoiceView,
};
use rw_sim::creature::{CreatureId, EntityId, LizardAnimation};
use rw_sim::{CameraView, FloatExt, RoomExt, SoundId, Vec2, WaterDrip};

use crate::body_part::{BodyPart, PointConnection};
use crate::draw::Color;
use crate::lizard_cosmetics::{CosmeticCx, LizardCosmetic};
use crate::lizard_draw::LizardReading;
use crate::lizard_limb::{LimbTick, LizardLimb};
use crate::math::CustomVec;
use crate::tail::{TailAnchor, TailSegment};

/// `GraphicsModule.cullRange`.
const CULL_RANGE: f32 = 200.0;

/// `LizardGraphics.IndividualVariations`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndividualVariations {
    pub head_size: f32,
    pub fatness: f32,
    pub tail_length: f32,
    pub tail_fatness: f32,
    pub tail_color: f32,
}

/// `CreatureLooker` over the lizard AI's tracker, which tracks no creature yet (lizard
/// step 7), so it never finds one to look at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureLooker {
    pub look_focus_delay: i32,
    pub chance_to_look_at_nothing: f32,
    pub frames_between_focus_change: i32,
}

impl CreatureLooker {
    /// `Update`: with the focus delay run out, a `1 / framesBetweenFocusChange` chance to
    /// look again: then over `chanceToLookAtNothing` a reevaluation, else at nothing.
    fn update(&mut self, g: &mut LizardGraphics, rng: &mut UnityRandom) {
        if self.look_focus_delay > 0 {
            self.look_focus_delay -= 1;
        }
        if self.look_focus_delay < 1 && rng.value() < 1.0 / self.frames_between_focus_change as f32
        {
            self.look_focus_delay = self.frames_between_focus_change;
            if rng.value() > self.chance_to_look_at_nothing {
                Self::reevaluate(rng);
                return;
            }
            g.look_at_nothing(rng);
        }
    }

    /// `ReevaluateLookObject` with no creature to favour: its coin for random interest
    /// (no tracked creatures to score).
    fn reevaluate(rng: &mut UnityRandom) {
        let _ = rng.value() < 0.5;
    }
}

/// A lizard's graphics module.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardGraphics {
    pub id: EntityId,
    pub creature: CreatureId,
    /// Its serial (`LizardGraphicsLink::module`).
    pub module: u64,
    /// The lizard as the module last read it (a dropped module's bubbles read this once
    /// the body is gone).
    pub lizard: LizardReading,
    /// `palette.blackColor`, once its sprites were in the camera's room.
    pub palette_black: Option<Color>,
    pub kind: CreatureType,
    pub culled: bool,
    pub last_culled: bool,
    pub ivars: IndividualVariations,
    pub limbs: Vec<LizardLimb>,
    /// `drawPositions[chunk, 0]` (now) and `[chunk, 1]` (last).
    pub draw_positions: [[Vec2; 2]; 3],
    pub tail: Vec<TailSegment>,
    pub head: BodyPart,
    pub legs_grabbing: i32,
    pub front_legs_grabbing: i32,
    pub hind_legs_grabbing: i32,
    pub no_grip_counter: i32,
    pub front_bob: f32,
    pub hind_bob: f32,
    pub depth_rotation: f32,
    pub last_depth_rotation: f32,
    pub head_depth_rotation: f32,
    pub last_head_depth_rotation: f32,
    pub stunned_last_update: bool,
    pub rotate_while_stunned: bool,
    pub blink: f32,
    pub last_blink: f32,
    pub flicker: i32,
    pub flicker_color: f32,
    pub white_flicker: i32,
    /// The link's `white_flicker_calls` last seen.
    pub white_flicker_calls: u32,
    pub every_second_draw: bool,
    pub breath: f32,
    pub last_breath: f32,
    pub tail_direction: Vec2,
    pub looker: Option<CreatureLooker>,
    pub look_pos: Vec2,
    pub scanning: f32,
    pub head_color_setter: f32,
    /// `eyes[i, 0..3]`.
    pub eyes: [[Vec2; 3]; 2],
    pub eye_beams_active: f32,
    pub black_salamander: bool,
    pub cosmetics: Vec<LizardCosmetic>,
    pub body_length: f32,
    pub tail_length: f32,
    pub head_connection_rad: f32,
    pub show_dominance: f32,
    pub voice_visualization_intensity: f32,
    pub last_voice_visualization_intensity: f32,
    pub voice_visualization: f32,
    pub last_voice_visualization: f32,
    pub belly_drag_volume: f32,
    /// `whiteCamoColorAmount` (-1 for the breeds that never camouflage).
    pub white_camo_color_amount: f32,
    pub effect_color: [f32; 3],
}

/// The sprites before the cosmetics: body circles, body, tail, limbs, head, limb colours.
pub const LIZARD_BASE_SPRITES: usize = 20;

impl LizardGraphics {
    /// The constructor, under `Random.InitState(ID.RandomSeed)` with the stream restored
    /// after (so `rng` is untouched): the individual variations, limbs, tail and head
    /// (each placed with a draw), the salamander roll, the cosmetics, blink, breath and
    /// tail direction.
    #[must_use]
    pub fn new(view: &LizardView<'_>) -> Self {
        let lizard = view.lizard;
        let params = lizard.params;
        let mut rng = UnityRandom::new(view.id.random_seed());
        let rng = &mut rng;
        let ivars = Self::generate_ivars(view.kind, rng);
        let limbs: Vec<LizardLimb> = (0..4).map(|i| LizardLimb::new(view, i, rng)).collect();
        let n = params.tail_segments;
        let mut tail = Vec::new();
        for j in 0..n {
            let rad = 8.0 * params.body_size_fac * (n - j) as f32 / n as f32;
            let conn = (if j > 0 { 8.0 } else { 16.0 } + rad) / 2.0
                * params.tail_length_factor
                * ivars.tail_length;
            let mut seg = TailSegment::new(rad, conn, 0.4);
            seg.pull_in_previous_position = false;
            seg.part.reset(lizard.body.chunks[1].pos, rng);
            tail.push(seg);
        }
        let tail_length = tail.iter().map(|t| t.connection_rad).sum();
        let mut head = BodyPart::new(6.0 * params.head_size, 0.5, 0.99);
        head.reset(lizard.body.chunks[0].pos, rng);
        let head_connection_rad = 11.0 * params.head_size;
        let body_length = head_connection_rad * 0.5
            + lizard
                .body
                .connections
                .iter()
                .map(|c| c.distance)
                .sum::<f32>();
        // `regionParams.blackSalamanderChance` defaults to a third; only salamanders read it.
        let black_salamander = rng.value() < 1.0 / 3.0;
        let cx = CosmeticCx {
            kind: view.kind,
            body_length,
            body_and_tail_length: body_length + tail_length,
            head_size: params.head_size,
            tail_color: ivars.tail_color,
        };
        let cosmetics = if view.kind == CreatureType::GreenLizard {
            LizardCosmetic::choose_green(cx, LIZARD_BASE_SPRITES, rng)
        } else {
            Vec::new()
        };
        let blink = rng.value();
        let breath = rng.value();
        let deg = rng.value() * 360.0;
        let tail_direction = Vec2::from_degrees(deg) * rng.value();
        Self {
            id: view.id,
            creature: view.creature,
            module: 0,
            lizard: LizardReading::of(view),
            palette_black: None,
            kind: view.kind,
            culled: true,
            last_culled: true,
            ivars,
            limbs,
            draw_positions: [[Vec2::ZERO; 2]; 3],
            tail,
            head,
            legs_grabbing: 0,
            front_legs_grabbing: 0,
            hind_legs_grabbing: 0,
            no_grip_counter: 0,
            front_bob: 0.0,
            hind_bob: 0.0,
            depth_rotation: 0.0,
            last_depth_rotation: 0.0,
            head_depth_rotation: 0.0,
            last_head_depth_rotation: 0.0,
            stunned_last_update: false,
            rotate_while_stunned: false,
            blink,
            last_blink: blink,
            flicker: 0,
            flicker_color: 0.0,
            white_flicker: 0,
            white_flicker_calls: 0,
            every_second_draw: false,
            breath,
            last_breath: 0.0,
            tail_direction,
            looker: None,
            look_pos: Vec2::ZERO,
            scanning: 0.0,
            head_color_setter: 0.0,
            eyes: [[Vec2::ZERO; 3]; 2],
            eye_beams_active: 0.0,
            black_salamander,
            cosmetics,
            body_length,
            tail_length,
            head_connection_rad,
            show_dominance: 0.0,
            voice_visualization_intensity: 0.0,
            last_voice_visualization_intensity: 0.0,
            voice_visualization: 0.0,
            last_voice_visualization: 0.0,
            belly_drag_volume: 0.0,
            white_camo_color_amount: -1.0,
            effect_color: lizard.effect_color,
        }
    }

    /// `GenerateIvars` (the DLC breeds' caps left out).
    fn generate_ivars(kind: CreatureType, rng: &mut UnityRandom) -> IndividualVariations {
        let mut head_size = rng.clamped_random_variation(0.5, 0.07, 0.5) * 2.0;
        if rng.value() < 0.5 {
            head_size = 1.0;
        }
        let mut fatness = rng.clamped_random_variation(0.5, 0.12, 0.5) * 2.0;
        let tail_length = rng.clamped_random_variation(0.5, 0.2, 0.3) * 2.0;
        let mut tail_fatness = rng.clamped_random_variation(0.45, 0.1, 0.3) * 2.0;
        let mut tail_color = 0.0;
        if kind != CreatureType::WhiteLizard && rng.value() > 0.5 {
            tail_color = rng.value();
        }
        if kind == CreatureType::RedLizard {
            fatness = fatness.min(1.0);
            tail_fatness = tail_fatness.min(1.0);
        } else if kind == CreatureType::BlackLizard {
            fatness = rng.clamped_random_variation(0.45, 0.06, 0.5) * 2.0;
        }
        IndividualVariations {
            head_size,
            fatness,
            tail_length,
            tail_fatness,
            tail_color,
        }
    }

    /// `bodyParts`: limbs, tail, head.
    fn part_mut(&mut self, i: usize) -> Option<&mut BodyPart> {
        let limbs = self.limbs.len();
        let tail = self.tail.len();
        if i < limbs {
            Some(&mut self.limbs[i].limb.part)
        } else if i < limbs + tail {
            Some(&mut self.tail[i - limbs].part)
        } else if i == limbs + tail {
            Some(&mut self.head)
        } else {
            None
        }
    }

    /// `Reset`: the draw positions on the chunks, then the tail, the limbs (both on the
    /// middle chunk) and the head placed again, a draw each.
    pub fn reset(&mut self, view: &LizardView<'_>, rng: &mut UnityRandom) {
        let chunks = &view.lizard.body.chunks;
        for (i, c) in chunks.iter().enumerate().take(3) {
            self.draw_positions[i] = [c.pos, c.pos];
        }
        for t in &mut self.tail {
            t.part.reset(chunks[1].pos, rng);
        }
        for l in &mut self.limbs {
            l.reset(chunks[1].pos, rng);
        }
        self.head.reset(chunks[0].pos, rng);
    }

    /// `SuckedIntoShortCut`: tail and limbs pulled a sixth of the way to `pos`.
    pub fn sucked_into_shortcut(&mut self, pos: Vec2) {
        let parts = self
            .tail
            .iter_mut()
            .map(|t| &mut t.part)
            .chain(self.limbs.iter_mut().map(|l| &mut l.limb.part));
        for p in parts {
            p.last_pos = p.pos;
            p.vel *= 0.5;
            p.pos = (p.pos * 5.0 + pos) / 6.0;
        }
    }

    /// `LookAtNothing`: ahead of the head, randomly off by up to 100 px.
    fn look_at_nothing(&mut self, rng: &mut UnityRandom) {
        let [dp0, dp1] = [self.draw_positions[0][0], self.draw_positions[1][0]];
        let deg = rng.value() * 360.0;
        self.look_pos =
            dp0 + dp1.direction_to(dp0) * 100.0 + Vec2::from_degrees(deg) * (100.0 * rng.value());
    }

    /// `GraphicsModule.ShouldBeCulled`.
    fn should_be_culled(
        &self,
        view: &LizardView<'_>,
        cameras: usize,
        camera_room: &Room,
        camera: &CameraView,
    ) -> bool {
        let pos = view.lizard.body.chunks[0].pos;
        let margin = CULL_RANGE + if self.culled { 0.0 } else { 100.0 };
        if cameras > 2 && !camera.position_currently_visible(camera_room, pos, margin, true) {
            let next = if self.culled { 50.0 } else { 100.0 };
            return !camera.position_visible_in_next_screen(camera_room, pos, next, true);
        }
        false
    }

    /// `BodyChunkDisplayRad`.
    #[must_use]
    pub fn body_chunk_display_rad(view: &LizardView<'_>, i: usize) -> f32 {
        view.lizard.body.chunks[i].rad * (1.0 / view.lizard.params.body_rad_fac)
    }

    /// `HeadRotation(1)` as the graphics are now.
    fn head_now(&self) -> LizardHead {
        LizardHead {
            pos: self.head.pos,
            vel: self.head.vel,
            aim: self.draw_positions[0][0].aim_to(self.head.pos),
            depth_rotation: self.head_depth_rotation,
        }
    }

    /// `BubbleIntensity`.
    #[must_use]
    pub fn bubble_intensity(&self, view: &LizardView<'_>) -> f32 {
        view.lizard.bubble_intensity * 0.5 + self.voice_visualization_intensity
    }

    /// `new LizardBubble(this, intensity, stickiness, extraSpeed)`.
    fn bubble(
        &self,
        view: &LizardView<'_>,
        params: [f32; 3],
        rng: &mut UnityRandom,
    ) -> LizardBubble {
        let head: BubbleHead = self
            .head_now()
            .for_bubble(view.lizard.params, view.jaw_open());
        LizardBubble::new(
            (view.creature, self.module),
            view.lizard.body.chunks[0].pos,
            head,
            params,
            rng,
        )
    }

    /// `Update`, after the lizard's; `link` carries the body's pushes and flickers in
    /// and the grip, head and parts out.
    pub fn update(
        &mut self,
        view: &LizardView<'_>,
        link: &mut LizardGraphicsLink,
        cx: &mut LizardGraphicsTick<'_>,
    ) {
        self.lizard = LizardReading::of(view);
        let voice = view.lizard.voice.as_ref();
        self.last_voice_visualization_intensity = self.voice_visualization_intensity;
        self.voice_visualization_intensity = if voice.is_some_and(LizardVoice::making_a_sound) {
            1.0
        } else {
            0.0
        };
        self.last_voice_visualization = self.voice_visualization;
        self.voice_visualization = voice.map_or(0.0, LizardVoice::visualized_intensity);
        for (i, dv) in std::mem::take(&mut link.pushes) {
            if let Some(p) = self.part_mut(i) {
                p.vel += dv;
            }
        }
        self.white_flicker = link.white_flicker;
        self.flicker = link.flicker;
        if link.white_flicker_calls != self.white_flicker_calls {
            self.white_flicker_calls = link.white_flicker_calls;
            self.every_second_draw = false;
        }
        self.last_culled = self.culled;
        self.culled = self.should_be_culled(view, cx.cameras, cx.camera_room, cx.camera);
        if !self.culled && self.last_culled {
            self.reset(view, cx.rng);
        }
        self.update_sound_loop(view, link, cx);
        let lizard = view.lizard;
        let params = lizard.params;
        self.show_dominance =
            (self.show_dominance - 1.0 / 60.0f32.lerp(120.0, cx.rng.value())).clamp(0.0, 1.0);
        if !self.culled {
            self.update_cosmetics(view, cx);
        }
        if self.white_flicker > 0 {
            self.white_flicker -= 1;
        }
        match self.looker {
            None => {
                self.looker = Some(CreatureLooker {
                    look_focus_delay: 0,
                    chance_to_look_at_nothing: 0.3,
                    frames_between_focus_change: params.frames_between_look_focus_change,
                });
            }
            Some(mut looker) => {
                looker.update(self, cx.rng);
                self.looker = Some(looker);
            }
        }
        if matches!(
            lizard.animation,
            LizardAnimation::PreySpotted
                | LizardAnimation::PreyReSpotted
                | LizardAnimation::ThreatSpotted
                | LizardAnimation::ThreatReSpotted
        ) {
            CreatureLooker::reevaluate(cx.rng);
        }
        self.eye_beams_active = (self.eye_beams_active - 0.05).clamp(0.0, 1.0);
        self.update_eyes(view, cx.rng);
        self.update_head_color_setter(view, cx);
        self.scanning += 0.05;
        if self.scanning > 1.0 {
            self.scanning -= 1.0;
        }
        let stunned = lizard.stunned();
        if stunned && !self.stunned_last_update {
            self.rotate_while_stunned = true;
        }
        self.stunned_last_update = stunned;
        for (i, c) in lizard.body.chunks.iter().enumerate().take(3) {
            self.draw_positions[i][1] = self.draw_positions[i][0];
            self.draw_positions[i][0] = c.pos;
        }
        let bob = (4.0 + 7.0 / params.walk_bob) / 2.0;
        self.front_bob =
            (self.front_bob * bob + (self.front_legs_grabbing - 1) as f32) / (bob + 1.0);
        self.hind_bob = (self.hind_bob * bob + (self.hind_legs_grabbing - 1) as f32) / (bob + 1.0);
        self.draw_positions[0][0].y += self.front_bob * params.walk_bob;
        self.draw_positions[1][0].y += self.front_bob + self.hind_bob * params.walk_bob * 0.5;
        self.draw_positions[2][0].y += self.hind_bob * params.walk_bob;
        if self.flicker > 0 {
            self.flicker -= 1;
            self.blink = cx.rng.value();
            self.last_blink = cx.rng.value();
        } else {
            self.last_blink = self.blink;
            if lizard.conscious() {
                let ex = lizard.ai.excitement.max(self.show_dominance);
                self.blink += 0.0025f32.lerp(0.07, ex) + cx.rng.value() * 0.001;
            } else {
                self.blink = (self.blink - self.blink.floor()).lerp(0.25, 0.02);
            }
        }
        self.update_light(link, cx);
        self.last_breath = self.breath;
        if !lizard.dead {
            self.breath += 0.0125;
        }
        if lizard.conscious() && cx.rng.value() < 0.05 {
            let deg = cx.rng.value() * 360.0;
            self.tail_direction = Vec2::from_degrees(deg) * cx.rng.value();
        }
        self.update_limbs(view, cx);
        self.update_tail(view, cx.room, cx);
        self.update_head(view, cx);
        if self.voice_visualization > 0.2 {
            let shake = 8.0
                * self
                    .voice_visualization
                    .inverse_lerp(0.2, 1.0)
                    .mathf_pow(0.5);
            for (i, dp) in self.draw_positions.iter_mut().enumerate() {
                dp[0] += Vec2::random_unit(cx.rng) * shake / (1 + i) as f32;
            }
        }
        link.legs_grabbing = self.legs_grabbing;
        link.no_grip_counter = self.no_grip_counter;
        link.white_flicker = self.white_flicker;
        link.flicker = self.flicker;
        self.write_link(link);
    }

    /// The head and body parts as the body and the room's objects read them.
    pub fn write_link(&self, link: &mut LizardGraphicsLink) {
        link.head = self.head_now();
        link.voice = LizardVoiceView {
            intensity: self.voice_visualization_intensity,
            visualization: self.voice_visualization,
        };
        link.parts = self
            .limbs
            .iter()
            .map(|l| l.limb.part.pos)
            .chain(self.tail.iter().map(|t| t.part.pos))
            .chain(std::iter::once(self.head.pos))
            .collect();
    }

    /// The belly-drag loop (the lounge's loop while lunging): louder the more of the
    /// body drags along terrain, higher the faster.
    fn update_sound_loop(
        &mut self,
        view: &LizardView<'_>,
        link: &mut LizardGraphicsLink,
        cx: &mut LizardGraphicsTick<'_>,
    ) {
        let lizard = view.lizard;
        let sl = &mut link.sound_loop;
        if lizard.animation == LizardAnimation::Lounge {
            sl.sound = Some(SoundId::LizardLoungeAttackLoop);
            sl.set_pitch(1.0);
            sl.set_volume(1.0);
        } else {
            sl.sound = Some(SoundId::LizardBellyDragLoop);
            sl.set_pitch(1.0);
            sl.set_volume(1.0);
            let (mut drag, mut vy, mut vx) = (0.0f32, 0.0f32, 0.0f32);
            for (i, c) in lizard.body.chunks.iter().enumerate().take(3) {
                let r = Self::body_chunk_display_rad(view, i) + 6.0;
                let contact = c.contact();
                let room = cx.room;
                if contact.x != 0
                    || room.is_solid_at(c.pos - Vec2::new(r, 0.0))
                    || room.is_solid_at(c.pos + Vec2::new(r, 0.0))
                {
                    drag += if contact.x != 0 { 1.0 } else { 0.5 } / 3.0;
                    vy += c.vel.y.abs() / 3.0;
                }
                if contact.y != 0
                    || room.is_solid_at(c.pos - Vec2::new(0.0, r))
                    || room.is_solid_at(c.pos + Vec2::new(0.0, r))
                {
                    drag += if contact.y != 0 { 1.0 } else { 0.5 } / 3.0;
                    vx += c.vel.x.abs() / 3.0;
                }
            }
            for t in &self.tail {
                if drag > 1.0 {
                    break;
                }
                if t.part.terrain_contact {
                    drag += 0.5 / self.tail.len() as f32;
                }
            }
            self.belly_drag_volume = self.belly_drag_volume.lerp(drag.clamp(0.0, 1.0), 0.2);
            let speed = vy.max(vx);
            sl.set_pitch(0.3f32.lerp(2.2, speed.inverse_lerp(0.2, 8.0).lerp(0.5, 0.3)));
            sl.set_volume(self.belly_drag_volume * speed.inverse_lerp(0.2, 2.0));
        }
        let pos = lizard.body.chunks[0].pos;
        sl.update_in(lizard.room, pos, &mut cx.loop_cx());
    }

    /// The eyes' wandering (two draws each).
    fn update_eyes(&mut self, view: &LizardView<'_>, rng: &mut UnityRandom) {
        let chunks = &view.lizard.body.chunks;
        let reach = 180.0f32;
        let ahead = chunks[0].pos + chunks[1].pos.direction_to(chunks[0].pos) * 300.0;
        for l in 0..2 {
            let eye = &mut self.eyes[l];
            eye[1] = eye[0];
            let deg = rng.value() * 360.0;
            let wander = Vec2::from_degrees(deg) * (reach * 0.5 * rng.value());
            eye[2] = eye[2].lerp((eye[2] + wander).clamp_magnitude(reach), 0.5);
            if l == 0 {
                eye[0] = eye[0].lerp(self.look_pos + eye[2], 0.6);
            } else {
                let to = self.look_pos.lerp(ahead, reach.inverse_lerp(25.0, 180.0));
                eye[0] = eye[0].lerp(to + eye[2], 0.3);
            }
            let back = (chunks[0].pos - chunks[1].pos).normalized();
            if back.dot((eye[0] - chunks[1].pos).normalized())
                < view.lizard.params.periferal_vision_angle
            {
                eye[0] = eye[0].lerp(ahead, 0.1);
                eye[2] *= 0.85;
            }
            if !eye[0].dist_less(ahead, 300.0) {
                eye[0] = eye[0].lerp(ahead, 0.1);
            }
            if eye[0].dist_less(chunks[0].pos, 100.0) {
                eye[0] = chunks[0].pos + chunks[0].pos.direction_to(eye[0]) * 100.0;
            }
        }
    }

    /// `headColorSetter` by the animation, and the lunge's bubbles and the green
    /// lizard's splashes, and the threat display's bubbles.
    fn update_head_color_setter(&mut self, view: &LizardView<'_>, cx: &mut LizardGraphicsTick<'_>) {
        let lizard = view.lizard;
        match lizard.animation {
            LizardAnimation::PrepareToLounge => {
                let d = 1.0 / (lizard.params.pre_lounge_crouch as f32 / 2.0);
                self.head_color_setter = (self.head_color_setter - d).clamp(-1.0, 1.0);
            }
            LizardAnimation::Lounge => {
                self.head_color_setter = 1.0;
                let b = self.bubble(view, [0.0, 1.0, self.voice_visualization * 10.0], cx.rng);
                cx.add(LizardSpawn::Bubble(b));
                if view.kind == CreatureType::GreenLizard {
                    for (m, c) in lizard.body.chunks.iter().enumerate() {
                        if c.contact().y < 0 && c.vel.length() > 7.0 {
                            let at = c.pos
                                + Vec2::new(0.0, -1.0)
                                    * (Self::body_chunk_display_rad(view, m) * 0.9);
                            for _ in 0..3 {
                                let deg = cx.rng.value() * 360.0;
                                let mut vel = Vec2::from_degrees(deg) * (cx.rng.value() * 10.0);
                                vel.y = vel.y.abs();
                                vel.x += c.vel.x;
                                let drip = WaterDrip::new(at, vel, false, cx.rng);
                                cx.add(LizardSpawn::Drip(drip));
                            }
                        }
                    }
                }
            }
            LizardAnimation::ThreatSpotted | LizardAnimation::ThreatReSpotted => {
                if lizard.time_in_animation < 10 || cx.rng.value() < 1.0 / 6.0 {
                    let extra = self.voice_visualization.mathf_pow(2.0) * 20.0;
                    let mut b = self.bubble(view, [0.0, 0.5, extra], cx.rng);
                    let deg = cx.rng.value() * 360.0;
                    b.vel = Vec2::from_degrees(deg) * 17.0f32.lerp(21.0, cx.rng.value());
                    b.life_time = 10.0f32.lerp(15.0, cx.rng.value()) as i32;
                    cx.add(LizardSpawn::Bubble(b));
                }
            }
            _ => self.head_color_setter = self.head_color_setter.lerp(0.0, 0.2),
        }
        if lizard.post_lounge_stun > 0 {
            self.head_color_setter = self.head_color_setter.lerp(-1.0, 0.8);
        }
    }

    /// The head light in a dark room: kept up each update, let go when the room is lit
    /// or the light went.
    fn update_light(&mut self, link: &mut LizardGraphicsLink, cx: &mut LizardGraphicsTick<'_>) {
        if let Some(id) = link.light {
            // Not a breed whose head lights up from noise: `0.6 * (1 - whiteCamoColorAmount)`.
            cx.light_upkeep = Some(LightUpkeep {
                id,
                pos: self.head.pos,
                rad: 100.0,
                alpha: 0.6 * (1.0 - self.white_camo_color_amount),
                color: self.effect_color,
            });
            if cx.light_slated || cx.darkness == 0.0 {
                link.light = None;
            }
        } else if cx.darkness > 0.0 && self.kind != CreatureType::Salamander {
            cx.add(LizardSpawn::Light {
                pos: self.head.pos,
                color: self.effect_color,
            });
        }
    }

    /// The legs: their updates, the grip counts and the depth rotation they give.
    fn update_limbs(&mut self, view: &LizardView<'_>, cx: &mut LizardGraphicsTick<'_>) {
        let lizard = view.lizard;
        let params = lizard.params;
        let mut turn = 0.0f32;
        self.legs_grabbing = 0;
        self.front_legs_grabbing = 0;
        self.hind_legs_grabbing = 0;
        let mut no_grip = true;
        let count = self.limbs.len();
        for i in 0..count {
            if lizard.conscious() && lizard.swim > 0.5 {
                let smooth = if params.smoothen_leg_movement {
                    1.0
                } else {
                    0.1
                };
                let phase = (i as f32 / count as f32 * smooth + self.breath * 2.0)
                    * std::f32::consts::PI
                    * 2.0;
                self.limbs[i].limb.part.vel.x += phase.sin() * lizard.swim * 0.5;
            }
            let mut tick = LimbTick {
                view,
                room: cx.room,
                rng: &mut *cx.rng,
                sounds: &mut *cx.sounds,
            };
            LizardLimb::update(&mut self.limbs, i, &mut tick);
            let l = &mut self.limbs[i];
            if l.grip_counter >= params.limb_grip_delay {
                self.legs_grabbing += 1;
                if i < 2 {
                    self.front_legs_grabbing += 1;
                } else if i >= count - 2 {
                    self.hind_legs_grabbing += 1;
                }
            }
            if l.grip_counter > 0 {
                no_grip = false;
            }
            let c = &lizard.body.chunks[l.chunk];
            let rot = &lizard.body.chunks[if l.chunk == 2 { 0 } else { 2 }];
            let mut side = l.limb.part.pos.distance_to_line(c.pos, rot.pos);
            side *= if i > 1 { 1.0 } else { -1.0 };
            if side.abs() > 10.0 {
                turn += if side >= 0.0 { 1.0 } else { -1.0 };
            }
            l.flip = l.flip.lerp(if side < 0.0 { 1.0 } else { -1.0 }, 0.3);
        }
        turn = turn.clamp(-1.0, 1.0);
        if lizard.conscious() {
            let facing = if self.draw_positions[0][0].x > self.draw_positions[1][0].x {
                -1.0
            } else {
                1.0
            };
            turn = turn.lerp(facing, lizard.swim);
        }
        self.last_depth_rotation = self.depth_rotation;
        if !lizard.stunned() || self.rotate_while_stunned {
            self.depth_rotation = self.depth_rotation.lerp(turn, 0.1);
        }
        self.last_head_depth_rotation = self.head_depth_rotation;
        let dp0 = self.draw_positions[0][0];
        let f = (self.look_pos - dp0)
            .normalized()
            .dot((self.head.pos - dp0).normalized());
        let f = f.abs().inverse_lerp(0.0, 0.6);
        self.head_depth_rotation = self.head_depth_rotation.lerp(self.depth_rotation * f, 0.5);
        if no_grip {
            self.no_grip_counter += 1;
        } else {
            self.no_grip_counter = 0;
        }
    }

    /// The tail, from the tip unless swimming.
    fn update_tail(&mut self, view: &LizardView<'_>, room: &Room, cx: &LizardGraphicsTick<'_>) {
        let lizard = view.lizard;
        let stiffness = lizard
            .params
            .tail_stiffness
            .lerp(900.0, self.show_dominance);
        let n = self.tail.len();
        if lizard.conscious() && lizard.swim > 0.5 {
            for i in 0..n {
                self.update_tail_segment(view, room, cx, i, stiffness);
            }
        } else {
            for i in (0..n).rev() {
                self.update_tail_segment(view, room, cx, i, stiffness);
            }
        }
    }

    /// `UpdateTailSegment` (its dominance shake, the only draws, never runs: no base-game
    /// lizard shows dominance).
    fn update_tail_segment(
        &mut self,
        view: &LizardView<'_>,
        room: &Room,
        cx: &LizardGraphicsTick<'_>,
        i: usize,
        stiffness: f32,
    ) {
        let lizard = view.lizard;
        let n = self.tail.len();
        let dp1 = self.draw_positions[1][0];
        let dp2 = self.draw_positions[2][0];
        if i == 0 {
            self.tail[0].update(room, TailAnchor::Point(dp2));
        } else {
            let (prev, rest) = self.tail.split_at_mut(i);
            rest[0].update(room, TailAnchor::Segment(&mut prev[i - 1].part));
        }
        let a = match i {
            0 => dp1,
            1 => dp2,
            _ => self.tail[i - 2].part.pos,
        }
        .lerp(dp1, 0.2);
        let seg = &mut self.tail[i].part;
        if cx.point_submerged(seg.pos) {
            seg.vel *= 0.8;
        } else {
            seg.vel.y -= 0.9 * (i as f32 / (n - 1) as f32).mathf_pow(3.0);
        }
        let reach = 15.0 * (i + 1) as f32;
        if !seg.pos.dist_less(dp2, reach) {
            seg.pos = lizard.body.chunks[2].pos + dp2.direction_to(seg.pos) * reach;
        }
        let decline = lizard.params.tail_stiffness_decline.mathf_pow(i as f32);
        seg.vel += a.direction_to(seg.pos) * (stiffness * decline) / a.distance(seg.pos);
        if i == 0 {
            seg.vel += self.tail_direction;
        }
        if lizard.conscious() && lizard.swim > 0.5 {
            let w = lizard.swim.inverse_lerp(0.5, 1.0);
            seg.vel *= 1.0 - 0.5 * w;
            let wave =
                ((self.breath * 5.0 - i as f32 / n as f32) * 0.3 * std::f32::consts::PI).sin();
            let push = dp1.direction_to(dp2) + (seg.pos - a).normalized().perpendicular() * wave;
            seg.vel += push * (w * 0.85f32.mathf_pow(i as f32) * 7.0);
        }
    }

    /// The head: follows the jaws' prey, or turns toward where the lizard looks or goes,
    /// tied to the neck; a jump of over 40 px blows bubbles, as do `bubble` ticks.
    fn update_head(&mut self, view: &LizardView<'_>, cx: &mut LizardGraphicsTick<'_>) {
        let lizard = view.lizard;
        let params = lizard.params;
        let dp0 = self.draw_positions[0][0];
        let dp1 = self.draw_positions[1][0];
        let moved = self.head.pos - self.head.last_pos;
        self.head.update_free(cx.room);
        if let Some(prey) = view.grabbed_creature {
            self.head.pos = prey;
        } else if lizard.conscious() {
            let mut dir = dp0.direction_to(self.look_pos);
            if let Some(last) = lizard.upcoming_connections.last() {
                let to = Vec2::middle_of_tile(last.dest.tile);
                dir = dir.slerp(dp0.direction_to(to), 0.15 + params.neck_stiffness * 0.15);
            }
            let neck = dp1.direction_to(dp0);
            let mut bend = (-dir.dot(neck)).clamp(0.0, 1.0);
            bend = 1.0 - bend;
            bend = bend.mathf_pow(3.0);
            bend = 1.0 - bend;
            bend *= 1.0 - params.neck_stiffness;
            if bend > 0.0 {
                dir = dir.slerp(neck, bend);
            }
            self.head.vel += dir * 6.0f32.lerp(2.0, params.neck_stiffness);
        }
        self.head.connect_to_point(PointConnection {
            point: dp0 + dp1.direction_to(dp0) * (12.0 * params.head_size),
            rad: self.head_connection_rad,
            push: false,
            elastic: 0.2,
            host_vel: lizard.body.chunks[0].vel,
            adapt_vel: 0.5,
            exaggerate_vel: 0.1,
        });
        if !self.head.pos.dist_less(self.head.last_pos + moved, 40.0) {
            let n = 1 + (moved.length().inverse_lerp(10.0, 180.0) * 4.0) as i32;
            for _ in 0..n {
                let stickiness = cx.rng.value();
                let extra = 3.0 + self.voice_visualization * 10.0;
                let b = self.bubble(view, [1.0, stickiness, extra], cx.rng);
                cx.add(LizardSpawn::Bubble(b));
            }
        }
        let intensity = self.bubble_intensity(view);
        if lizard.bubble > 0 {
            let deg = cx.rng.value() * 360.0;
            self.head.pos += Vec2::from_degrees(deg) * (5.0 * intensity);
        }
        if lizard.bubble > 0 || self.voice_visualization > 0.5 {
            let extra = self.voice_visualization.mathf_pow(2.0) * 20.0;
            let mut k = 0;
            loop {
                let limit = cx.rng.range_i32(1, 1 + (intensity * 3.0) as i32);
                if k >= limit {
                    break;
                }
                let b = self.bubble(view, [intensity, 0.0, extra], cx.rng);
                cx.add(LizardSpawn::Bubble(b));
                k += 1;
            }
        }
    }
}
