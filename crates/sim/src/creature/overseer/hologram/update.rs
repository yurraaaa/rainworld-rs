//! `OverseerHologram.Update` and its subclasses' (`PointingHologram`, `DoorPointer`,
//! `BatPointer`, `ShelterPointer`, `DirectionPointer`, `ObjectPointer`,
//! `CreaturePointer`, `FoodPointer`, `AngryHologram`, `OverseerImage`), with their
//! `DisplayPosScore`s.
//!
//! Mirrors: OverseerHolograms/OverseerHologram.cs, OverseerHolograms/AngryHologram.cs,
//!   OverseerHolograms/OverseerImage.cs

use rw_assets::CreatureType;

use super::parts::{HoloImage, PartKind};
use super::{
    DoorPointer, HologramKind, HologramMessage, ImageHologram, ObjectPointerKind, OverseerHologram,
    PointerKind,
};
use crate::creature::overseer::update::{OverseerTick, PlayerView};
use crate::creature::overseer::{OverseerAbstractAi, OverseerAi, OverseerBody, OverseerMode};
use crate::creature::{AbstractCreatureAi, WorldCoordinate};
use crate::room_ext::RoomExt;
use crate::{FloatExt, SoundId, TileExt, Vec2};

/// A hologram's update, with its overseer's parts (taken out of the world).
pub(crate) struct HologramUpdate<'a, 'b> {
    pub holo: &'a mut OverseerHologram,
    pub body: &'a mut OverseerBody,
    pub ai: &'a mut OverseerAi,
    pub abs: &'a mut OverseerAbstractAi,
    pub base: &'a mut AbstractCreatureAi,
    pub cx: &'a mut OverseerTick<'b>,
}

/// What the scoring reads of the room and the player, copied out so the hologram can
/// be borrowed alongside.
#[derive(Clone, Copy)]
pub(super) struct ScoreView<'r> {
    pub room: &'r rw_assets::Room,
    pub room_index: i32,
    pub ai_map: &'r crate::world::AiMap,
    pub camera: crate::CameraView,
    /// The player's `DangerPos`.
    pub danger: Option<Vec2>,
    pub water_level: i32,
}

impl ScoreView<'_> {
    fn mid(t: (i32, i32)) -> Vec2 {
        Vec2::middle_of_tile(t)
    }

    pub(super) fn viewed(&self, pos: Vec2, margin: f32) -> bool {
        self.camera.viewed(self.room_index, self.room, pos, margin)
    }

    /// `OverseerHologram.DisplayPosScore`.
    pub(super) fn base_score(&self, t: (i32, i32)) -> f32 {
        let mut num = 0.0;
        if self.room.is_solid_tile(t.0, t.1) {
            num += 1000.0;
        }
        if t.1 < self.water_level + 5 {
            num += 500.0;
        }
        num -= self.ai_map.terrain_proximity(t.0, t.1).min(5) as f32 * 50.0;
        if let Some(danger) = self.danger {
            let mid = Self::mid(t);
            if mid.dist_less(danger, 100.0) {
                num += 1000.0;
            }
            if self.viewed(mid, -100.0) {
                num -= 100_000.0;
            }
        }
        num
    }

    /// `PointingHologram.DisplayPosScore`.
    fn pointing_score(&self, t: (i32, i32), point_at: Vec2) -> f32 {
        let danger = self.danger.unwrap_or_default();
        let mid = Self::mid(t);
        let mut num = 0.0;
        if point_at.dist_less(danger, 150.0) {
            num += 100.0;
        }
        let d = point_at.distance(danger);
        if point_at.dist_less(mid, d) && danger.dist_less(mid, d) {
            num += mid.distance_to_line(danger, point_at).abs().min(500.0);
        } else {
            num += 500.0;
        }
        if mid.dist_less(point_at, 100.0) {
            num += 1000.0;
        }
        if self.viewed(mid, -10.0) {
            if self.viewed(point_at, -10.0) {
                num += (100.0 - mid.distance(point_at)).abs().min(1000.0) * 0.5;
            }
        } else {
            num += 10_000.0;
        }
        num
    }

    /// `ShortcutLeadingToNode(door).StartTile` (the first shortcut when none leads there).
    pub(super) fn door_start(&self, door: i32) -> Option<(i32, i32)> {
        let map = self.room.shortcut_map();
        usize::try_from(door)
            .ok()
            .and_then(|d| map.leading_to_node(d))
            .or_else(|| map.shortcuts().first())
            .map(|s| s.start)
    }

    /// `DoorPointer.DisplayPosScore`.
    fn door_score(&self, t: (i32, i32), door: i32, door_pos: Vec2) -> f32 {
        let mut num = self.pointing_score(t, door_pos);
        if door > -1
            && let Some(start) = self.door_start(door)
        {
            let start_mid = Self::mid(start);
            let mid = Self::mid(t);
            if start_mid.dist_less(mid, 400.0) {
                let hole = self.room.shortcut_hole_direction(start.0, start.1);
                let hole = Vec2::new(hole.0 as f32, hole.1 as f32).normalized();
                let val = hole.dot(start_mid.direction_to(mid));
                let facing = 0.0.lerp(1000.0, val.inverse_lerp(-1.0, 1.0).mathf_pow(4.0));
                num -= facing * start_mid.distance(mid).inverse_lerp(400.0, 200.0);
            }
        }
        num
    }

    /// `InputInstruction.DisplayPosScore` and the device's terrain term.
    pub(super) fn instruction_score(
        &self,
        t: (i32, i32),
        close_to: Vec2,
        direction_only: bool,
        terrain_dx: Option<i32>,
    ) -> f32 {
        let mid = Self::mid(t);
        let danger = self.danger.unwrap_or_default();
        let mut num = self.base_score(t)
            + (200.0 - mid.distance(danger)).abs() / 2.0
            + mid.distance(close_to);
        if let Some(dx) = terrain_dx.filter(|_| !direction_only) {
            num -= self.ai_map.terrain_proximity(t.0 + dx, t.1).min(5) as f32 * 50.0;
        }
        num
    }
}

impl HologramUpdate<'_, '_> {
    fn mid(t: (i32, i32)) -> Vec2 {
        Vec2::middle_of_tile(t)
    }

    fn player(&self) -> Option<PlayerView> {
        self.cx.player
    }

    pub(super) fn score_view(&self) -> ScoreView<'_> {
        ScoreView {
            room: self.cx.room,
            room_index: self.cx.room_index,
            ai_map: self.cx.ai_map,
            camera: self.cx.camera,
            danger: self.player().map(|p| p.pos),
            water_level: self.cx.room.water_level().unwrap_or(-1),
        }
    }

    /// `PointAt` of a pointing hologram.
    fn point_at(&self) -> Vec2 {
        match &self.holo.kind {
            HologramKind::Door { door, .. } => door.door_pos,
            _ => self
                .holo
                .pointed_creature_pos(self.cx.world)
                .unwrap_or(self.body.chunk().pos),
        }
    }

    /// The hologram's `DisplayPosScore(testPos)`.
    fn display_pos_score(&mut self, t: (i32, i32)) -> f32 {
        let view = self.score_view();
        match &self.holo.kind {
            HologramKind::Door { door, .. } => view.door_score(t, door.door, door.door_pos),
            HologramKind::Object {
                pointer: ObjectPointerKind::Food { .. },
            } => {
                let point_at = self.body.chunk().pos;
                let danger = view.danger.unwrap_or_default();
                let mid = Self::mid(t);
                let mut num = mid.distance(point_at.lerp(point_at.move_towards(danger, 50.0), 0.5));
                if !view.viewed(mid, -20.0) {
                    num += 10_000.0;
                }
                num
            }
            HologramKind::Object { .. } => view.pointing_score(t, self.point_at()),
            HologramKind::Angry { .. } => match view.danger {
                Some(d) => Self::mid(t).distance(d),
                None => self.cx.rng.value(),
            },
            HologramKind::Image(image) => {
                let gate = self
                    .cx
                    .world
                    .abstract_room(self.cx.room_index)
                    .is_some_and(rw_assets::world::AbstractRoomData::gate);
                if gate {
                    Self::mid(t).distance(Vec2::new(726.0, 250.0))
                } else if let Some(at) = image.show_pos {
                    Self::mid(t).distance(at)
                } else {
                    view.base_score(t)
                }
            }
            HologramKind::Instruction(i) => {
                let dx = match i.kind() {
                    super::InstructionKind::Keyboard => Some(-10),
                    super::InstructionKind::GamePad => Some(10),
                    super::InstructionKind::Switch => None,
                };
                view.instruction_score(t, i.close_to_pos, i.direction_only, dx)
            }
        }
    }

    /// `Update(eu)` of the hologram's class.
    /// The subclasses go on after the base destroyed a hologram whose overseer left.
    pub fn update(&mut self) {
        if let HologramKind::Door {
            pointer: PointerKind::Forced { cycle, .. },
            ..
        } = &mut self.holo.kind
        {
            *cycle += 1;
        }
        let alive = self.base_update();
        match self.holo.kind.clone() {
            HologramKind::Door { pointer, .. } => {
                self.pointing_update();
                self.door_update();
                if alive {
                    self.pointer_update(pointer);
                } else {
                    self.lost_overseer_pointer_update(pointer);
                }
            }
            HologramKind::Object { pointer } => {
                self.pointing_update();
                self.object_update(pointer);
            }
            HologramKind::Angry { .. } => self.angry_update(),
            HologramKind::Image(_) => self.image_update(),
            HologramKind::Instruction(_) => self.instruction_update(),
        }
    }

    /// The pointer classes' `Update` once the overseer has left the room (its `room` is
    /// null): the tutorial's bat pointer and a forced shelter pointer run on; the others
    /// dereference the overseer's room and throw in the game, ending the room's update.
    fn lost_overseer_pointer_update(&mut self, pointer: PointerKind) {
        match pointer {
            PointerKind::Bat { .. } if self.holo.message == HologramMessage::TutorialFood => {
                self.bat_update();
            }
            PointerKind::Bat { .. } => self.holo.still_relevant = false,
            PointerKind::Shelter { forcepoint, .. } if forcepoint > -1 => {
                self.shelter_update(forcepoint);
            }
            PointerKind::Shelter { .. } | PointerKind::Direction { .. } => {
                log::warn!("hologram update after its overseer left (the game throws here)");
            }
            // `direction.room != overseer.room`.
            PointerKind::Forced { .. } => self.holo.still_relevant = false,
        }
    }

    /// `OverseerHologram.Update`. Returns `false` when it destroyed itself for having
    /// lost its overseer.
    fn base_update(&mut self) -> bool {
        if !self.body.in_room || self.cx.room_index != self.holo.room {
            self.holo.slated = true;
            return false;
        }
        let h = &mut *self.holo;
        h.last_pos = h.pos;
        h.look_at_comm_crit_counter -= 1;
        if h.look_at_comm_crit_counter < 1 {
            h.look_at_communication_creature = !h.look_at_communication_creature;
            let max = if h.look_at_communication_creature {
                120
            } else {
                40
            };
            h.look_at_comm_crit_counter = self.cx.rng.range_i32(20, max);
        }
        for i in 0..self.holo.parts.len() {
            self.part_update(i);
        }
        let (w, h) = (self.cx.room.width(), self.cx.room.height());
        for _ in 0..10 {
            let test = if self.cx.rng.value() < 0.5 {
                let dir = Vec2::random_unit(self.cx.rng);
                (self.body.chunk().pos + dir * (self.cx.rng.value() * 400.0)).tile()
            } else {
                let x = self.cx.rng.range_i32(0, w);
                (x, self.cx.rng.range_i32(0, h))
            };
            let temp = self.holo.temp_display_tile;
            if self.display_pos_score(test) < self.display_pos_score(temp) {
                self.holo.temp_display_tile = test;
            }
            let (temp, display) = (self.holo.temp_display_tile, self.holo.display_tile);
            if self.display_pos_score(temp) < self.display_pos_score(display) - 50.0 {
                self.holo.display_tile = temp;
            }
        }
        let h = &mut *self.holo;
        let b = &*self.body;
        h.last_fade = h.fade;
        let display_mid = Self::mid(h.display_tile);
        let near =
            h.display_tile.float_dist(b.hover_tile) < 15.0 && h.pos.dist_less(display_mid, 200.0);
        h.fade = if matches!(b.mode, OverseerMode::Watching | OverseerMode::Projecting)
            && h.still_relevant
            && near
        {
            h.fade.lerp_and_tick(1.0, 0.01, 0.05)
        } else {
            h.fade.lerp_and_tick(0.0, 0.01, 0.1)
        };
        h.push_around_pos *= 0.8;
        if b.extended > 0.0 {
            h.push_around_pos += (b.chunk().pos - b.chunk().last_pos) * b.extended;
        }
        if h.fade == 0.0 && h.last_fade == 0.0 {
            h.pos = display_mid + h.push_around_pos;
            h.last_pos = h.pos;
            if !h.still_relevant {
                h.slated = true;
            }
        } else if h.pos.dist_less(display_mid, 200.0) {
            let to = display_mid + h.push_around_pos;
            h.pos = h.pos.move_towards(to, 30.0);
            h.pos = h.pos.lerp(to, 0.05);
        }
        h.fade = h
            .fade
            .min(0.2 + 0.8 * h.last_pos.distance(h.pos).inverse_lerp(20.0, 2.0));
        if let Some(p) = self.cx.player
            && !p.in_shortcut
            && (!p.in_room || p.dead)
        {
            self.holo.still_relevant = false;
        }
        true
    }

    /// Part `i`'s `Update`.
    fn part_update(&mut self, i: usize) {
        let (fade, last_fade, color) = (self.holo.fade, self.holo.last_fade, self.holo.color());
        let part = &mut self.holo.parts[i];
        part.base_update(fade, last_fade, color, self.cx.rng);
        part.own_update();
        let pos = self.holo.pos;
        match self.holo.parts[i].kind.clone() {
            PartKind::PlayerGhost {
                room_position: Some(at),
                ..
            } => {
                let p = &mut self.holo.parts[i];
                let f = (fade * p.part_fade).mathf_pow(0.8);
                p.offset = (at - pos) * (0.2 + 0.8 * f);
            }
            PartKind::RedCross { .. } => {
                if let Some(offset) = self.red_cross_offset() {
                    self.holo.parts[i].offset = offset;
                }
            }
            PartKind::GamePadStick { .. } => self.stick_update(i),
            PartKind::HoloImage(_) => self.holo_image_update(i),
            _ => {}
        }
    }

    /// `RedCross.Update`'s offset: on the stick's socket, or between the keyboard's Up
    /// and Down keys.
    fn red_cross_offset(&self) -> Option<Vec2> {
        let i = self.holo.instruction()?;
        let parts = &self.holo.parts;
        Some(match &i.device {
            super::InstructionDevice::Keyboard { keys } => {
                (parts[keys[1]].offset + parts[keys[3]].offset) / 2.0
            }
            super::InstructionDevice::GamePad { socket, .. }
            | super::InstructionDevice::Switch { socket, .. } => parts[*socket].offset,
        })
    }

    /// `GamePadStick.Update`.
    fn stick_update(&mut self, i: usize) {
        let direction = self.holo.instruction().map_or((0, 0), |d| d.direction);
        let show = Vec2::new(direction.0 as f32, direction.1 as f32).normalized();
        let PartKind::GamePadStick {
            socket,
            counter,
            out_to_side,
            stick_pos,
            stick_vel,
            switch,
        } = &mut self.holo.parts[i].kind
        else {
            return;
        };
        if direction != (0, 0) {
            *counter -= 1;
            if *counter < 1 {
                *out_to_side = !*out_to_side;
                *counter = 20;
            }
        } else {
            *out_to_side = false;
            *counter = 0;
        }
        if *out_to_side {
            *stick_vel += show * (0.75 * (*counter as f32).inverse_lerp(20.0, 10.0));
        }
        *stick_pos += *stick_vel;
        *stick_vel *= 0.75;
        *stick_vel -= *stick_pos / 1.5;
        let (socket, switch, pos) = (*socket, *switch, *stick_pos);
        if pos.length() > 1.0 {
            let k = if switch { 2.2 } else { 1.0 };
            self.holo.parts[socket].offset += pos * k;
        }
        let pos = pos.clamp_length(1.0);
        if let PartKind::GamePadStick { stick_pos, .. } = &mut self.holo.parts[i].kind {
            *stick_pos = pos;
        }
        let reach = if switch { 20.0 - 6.0 } else { 20.0 };
        let socket_part = &self.holo.parts[socket];
        let scale = match socket_part.kind {
            PartKind::GamePadStickSocket { scale } => scale,
            _ => 0.6,
        };
        let (offset, visible) = (
            socket_part.offset + pos * (reach * scale),
            socket_part.visible,
        );
        let part = &mut self.holo.parts[i];
        part.offset = offset;
        part.visible = visible;
    }

    /// The image owner's `CurrImageIndex`, `CurrImage.Index`, `ShowTime` and
    /// `ImmediatelyToContent`.
    fn image_owner(&self) -> (i32, i32, i32, f32) {
        match &self.holo.kind {
            HologramKind::Angry { counter, .. } => {
                let img = if counter % 60 >= 30 {
                    super::ImageId::DeadSlugcatB
                } else {
                    super::ImageId::DeadSlugcatA
                };
                (0, img.index(), 0, 1.0)
            }
            HologramKind::Image(image) => {
                let img = image.images.get(image.curr_image).copied();
                let index = img.map_or(-1, super::ImageId::index);
                let curr = i32::try_from(image.curr_image).unwrap_or(0);
                (curr, index, image.show_time, 0.0)
            }
            HologramKind::Instruction(i) => match &i.controller {
                Some(super::Controller::ScavengerTrade { show_image, .. }) => {
                    let img = super::instruction::SCAV_TRADE_IMAGES[*show_image];
                    (i32::try_from(*show_image).unwrap_or(0), img.index(), 0, 0.5)
                }
                _ => (0, 0, 0, 0.0),
            },
            _ => (0, 0, 0, 0.0),
        }
    }

    /// `HoloImage.Update`.
    fn holo_image_update(&mut self, i: usize) {
        let (curr_index, curr_image, show_time, immediately) = self.image_owner();
        let (pos, fade) = (self.holo.pos, self.holo.fade);
        let part_fade = self.holo.parts[i].part_fade;
        let PartKind::HoloImage(img) = &mut self.holo.parts[i].kind else {
            return;
        };
        let rng = &mut *self.cx.rng;
        if !img.sound {
            img.sound = true;
            self.cx
                .sounds
                .play_at(SoundId::OverseerImageLoop, pos, 0.0, 1.0, rng);
        }
        let mut changed = false;
        if curr_index != img.last_img {
            img.random_flicker = img.random_flicker.lerp(1.0, rng.value() * 0.2);
            img.last_img = curr_index;
            changed = true;
        }
        let shown = if img.show_random_flicker_image {
            (img.random_image, 0)
        } else {
            (curr_image, 1)
        };
        let shown = (shown.0.max(0), shown.1);
        let level = fade * part_fade * img.my_alpha;
        if shown != img.last_show_img && level > 0.2 {
            let id = if changed {
                SoundId::OverseerImageBigFlicker
            } else {
                SoundId::OverseerImageSmallFlicker
            };
            self.cx.sounds.play_at(id, pos, level, 1.0, rng);
        }
        img.last_show_img = shown;
        img.last_pan_pos = img.pan_pos;
        img.pan_pos += img.pan_vel;
        img.pan_pos = img.pan_pos.clamp_length(1.0).lerp(img.pan_center, 0.05);
        img.pan_vel *= 0.8;
        let dir = Vec2::random_unit(rng);
        let k = 0.1 * rng.value() * rng.value() * rng.value() * rng.value();
        img.pan_vel += dir * k;
        img.pan_vel += (img.pan_center - img.pan_pos) * 0.01;
        img.last_my_alpha = img.my_alpha;
        let num = (part_fade * fade).inverse_lerp(0.5, 1.0);
        img.my_alpha = num.min(img.my_alpha.lerp_and_tick(num, 0.01, 1.0 / 120.0));
        img.my_alpha = img.my_alpha.lerp(num, immediately * 0.5);
        let num = img.my_alpha.inverse_lerp(1.0, 0.5);
        let slow = if num < img.random_flicker {
            20.0.lerp(200.0, img.random_flicker)
        } else {
            80.0
        };
        let rate = (show_time as f32).lerp_map(20.0, 300.0, 10.0, slow);
        img.random_flicker = img.random_flicker.lerp_and_tick(num, 0.0, 1.0 / rate);
        img.random_flicker *= 1.0.lerp(0.2, immediately);
        let counts = img.my_alpha > 0.9 && img.random_flicker < 0.1;
        let was_random = img.show_random_flicker_image;
        img.random_image_time += 1.0 / 15.0.lerp(1.0, img.random_flicker.mathf_pow(0.5));
        if img.random_image_time >= 1.0 {
            img.show_random_flicker_image = rng.value() < img.random_flicker;
            let before = img.random_image;
            img.random_image = rng.range_i32(0, 25);
            img.random_image_time = 0.0;
            if img.show_random_flicker_image {
                let dir = Vec2::random_unit(rng);
                img.pan_center = dir * (0.8 * rng.value());
            } else {
                img.pan_center *= 0.0;
            }
            if was_random != img.show_random_flicker_image
                || (img.show_random_flicker_image && before != img.random_image)
            {
                img.pan_pos = img.pan_center;
                img.last_pan_pos = img.pan_center;
            }
        }
        if counts && let Some(comm) = self.ai.communication.as_mut() {
            comm.showed_image_time += 1;
        }
    }

    /// `PointingHologram.Update` after the base: the arrow turns toward `PointAt`.
    fn pointing_update(&mut self) {
        let to = self.holo.pos.direction_to(self.point_at());
        let t = self.cx.rng.value() * 0.3;
        let symbol_visible = self
            .symbol_part()
            .is_some_and(|s| self.holo.parts[s].visible);
        let arrow = &mut self.holo.parts[0];
        if let PartKind::Arrow { dir } = &mut arrow.kind {
            *dir = dir.slerp(to, t);
        }
        arrow.visible = symbol_visible;
    }

    /// The pointer's `symbol` part.
    fn symbol_part(&self) -> Option<usize> {
        match self.holo.kind {
            HologramKind::Door { .. } => Some(2),
            HologramKind::Object { .. } => Some(3),
            _ => None,
        }
    }

    /// The `door` setter: the door's position, or not relevant.
    fn set_door(&mut self, value: i32) {
        let connections = self.connections();
        let view = self.score_view();
        let start = (value > -1 && value < connections)
            .then(|| view.door_start(value))
            .flatten();
        let HologramKind::Door { door, .. } = &mut self.holo.kind else {
            return;
        };
        door.door = value;
        match start {
            Some(t) => door.door_pos = Self::mid(t),
            None => self.holo.still_relevant = false,
        }
    }

    fn connections(&self) -> i32 {
        self.cx
            .world
            .abstract_room(self.cx.room_index)
            .map_or(0, |r| i32::try_from(r.connections.len()).unwrap_or(0))
    }

    fn connection(&self, door: i32) -> i32 {
        self.cx
            .world
            .abstract_room(self.cx.room_index)
            .and_then(|r| r.connections.get(usize::try_from(door).ok()?).copied())
            .unwrap_or(-1)
    }

    fn door_state(&self) -> Option<DoorPointer> {
        match &self.holo.kind {
            HologramKind::Door { door, .. } => Some(door.clone()),
            _ => None,
        }
    }

    fn door_mut(&mut self) -> Option<&mut DoorPointer> {
        match &mut self.holo.kind {
            HologramKind::Door { door, .. } => Some(door),
            _ => None,
        }
    }

    /// `DoorPointer.Update` after `PointingHologram`'s.
    fn door_update(&mut self) {
        let Some(state) = self.door_state() else {
            return;
        };
        if state.report_player_exit_entrance
            && let Some(p) = self.player().filter(|p| p.in_room)
            && let Some(entrance) = p.entering_shortcut
        {
            let dest = self
                .cx
                .room
                .shortcut_map()
                .data_at(entrance)
                .and_then(|s| s.dest_node)
                .and_then(|n| i32::try_from(n).ok())
                .unwrap_or(-1);
            if dest > -1 && dest < self.connections() {
                if let Some(d) = self.door_mut() {
                    d.report_player_exit_entrance = false;
                }
                self.player_going_into_exit(dest == state.door);
            }
        }
        let necessary = if state.door > -1 && self.holo.still_relevant {
            if self.ai.tutorial.is_some() {
                true
            } else {
                // `CreatureDoorDistanceTracker` needs the Fly exit-distance maps: it is
                // made but never updates, so the pointer keeps pointing.
                if let Some(d) = self.door_mut() {
                    d.tracker = true;
                }
                state.pointer_necessary
            }
        } else {
            false
        };
        if let Some(d) = self.door_mut() {
            d.pointer_necessary = necessary;
        }
        self.holo.parts[2].visible = necessary;
        self.holo.parts[1].visible = necessary;
    }

    /// `PlayerGoingIntoExit` (with the bat and shelter pointers' overrides).
    fn player_going_into_exit(&mut self, following: bool) {
        let message = self.holo.message;
        let necessary = self.door_state().is_some_and(|d| d.pointer_necessary);
        let guide = &mut *self.cx.guide;
        let Some(comm) = self.ai.communication.as_mut() else {
            return;
        };
        if following {
            comm.player_is_following_a_direction(message, guide);
        } else {
            comm.player_not_following_a_direction(message, necessary, guide);
            match self.holo.kind {
                HologramKind::Door {
                    pointer:
                        PointerKind::Bat {
                            currently_pointing_at_swarm_room,
                        },
                    ..
                } => comm.un_wanted_swarm_room = currently_pointing_at_swarm_room,
                HologramKind::Door {
                    pointer:
                        PointerKind::Shelter {
                            currently_showing_to_shelter,
                            ..
                        },
                    ..
                } => comm.un_wanted_shelter = currently_showing_to_shelter,
                _ => {}
            }
        }
    }

    /// The pointer classes' own `Update`.
    fn pointer_update(&mut self, pointer: PointerKind) {
        match pointer {
            PointerKind::Bat { .. } => self.bat_update(),
            PointerKind::Shelter { forcepoint, .. } => self.shelter_update(forcepoint),
            PointerKind::Direction { .. } => self.direction_update(),
            PointerKind::Forced { direction, .. } => self.forced_update(direction),
        }
    }

    /// `ForcedDirectionPointer.Update` after `DoorPointer`'s: points at its
    /// `ReliableIggyDirection`'s exit, always shown, while that is in the guide's room.
    fn forced_update(&mut self, direction: u64) {
        let room = self.cx.room_index;
        let Some(exit) = self
            .cx
            .forced_directions
            .iter()
            .find(|d| d.id == direction && d.room == room)
            .map(|d| d.exit)
        else {
            self.holo.still_relevant = false;
            return;
        };
        self.set_door(exit);
        if let Some(d) = self.door_mut() {
            d.pointer_necessary = true;
        }
        self.holo.parts[2].visible = true;
        self.holo.parts[1].visible = true;
    }

    fn room_name(&self) -> &str {
        self.cx
            .world
            .abstract_room(self.cx.room_index)
            .map_or("", |r| r.name.as_str())
    }

    fn set_symbol(&mut self, element: &'static str) {
        if let PartKind::Symbol(s) = &mut self.holo.parts[2].kind {
            s.element = element;
        }
    }

    /// `BatPointer.Update`.
    fn bat_update(&mut self) {
        if self.holo.message == HologramMessage::TutorialFood {
            let few_bats = self
                .ai
                .tutorial
                .as_ref()
                .is_some_and(|t| t.bats_in_bat_room < 3);
            if self.room_name() == "SU_A41" && few_bats {
                self.set_door(3);
                self.set_symbol("foodSymbol");
            } else {
                self.set_door(0);
                self.set_symbol("batSymbol");
            }
            return;
        }
        let room = self.cx.room_index;
        if self.cx.world.guide_active_swarm_room(room) {
            self.holo.still_relevant = false;
            return;
        }
        if self.door_state().is_some_and(|d| d.last_room == room) {
            return;
        }
        let need = self
            .player()
            .map_or(1, |p| self.cx.story.food_to_hibernate - p.food);
        if let Some(d) = self.door_mut() {
            d.last_room = room;
        }
        let world = self.cx.world;
        let swarm_rooms = world.data().swarm_rooms.clone();
        let (mut best_door, mut best, mut best_swarm) = (-1, f32::MAX, 0);
        let comm_last = self
            .ai
            .communication
            .as_ref()
            .map_or(-1, |c| c.last_player_room);
        let unwanted = self
            .ai
            .communication
            .as_ref()
            .map_or(-1, |c| c.un_wanted_swarm_room);
        for (i, &swarm) in swarm_rooms.iter().enumerate() {
            let name = world.abstract_room(swarm).map_or("", |r| r.name.as_str());
            let su = world.region_name() == Some("SU");
            if !world.guide_active_swarm_room(swarm) || (su && name == "SU_A42") {
                continue;
            }
            let flies = world.flies_in_room(swarm, CreatureType::Fly);
            if flies <= 0 {
                continue;
            }
            let (mut door, mut dist) = (-1, f32::MAX);
            for j in 0..self.connections() {
                if self.connection(j) > -1 {
                    let coord = WorldCoordinate::new(room, -1, -1, j);
                    let d = world.guide_swarm_room_distance(coord, i);
                    if d > -1.0 && d < dist {
                        door = j;
                        dist = d;
                    }
                }
            }
            if door < 0 {
                continue;
            }
            // `CycleStartUp` is 1: the cycle's start-up ticks are not ported.
            let start_up = 1.0;
            dist *= 1.0.lerp((flies as f32).lerp_map(1.0, 5.0, 10.0, 1.0), start_up);
            // `roomsVisited` of a new game holds no swarm room yet.
            let i = i32::try_from(i).unwrap_or(-1);
            if self.connection(door) == comm_last || i == unwanted || flies < need {
                dist = dist * 20.0 + 10_000.0;
            }
            if dist < best {
                best = dist;
                best_door = door;
                best_swarm = i;
            }
        }
        if best_door > -1 {
            self.set_door(best_door);
            if let HologramKind::Door {
                pointer:
                    PointerKind::Bat {
                        currently_pointing_at_swarm_room,
                    },
                ..
            } = &mut self.holo.kind
            {
                *currently_pointing_at_swarm_room = best_swarm;
            }
            return;
        }
        if let Some(comm) = self.ai.communication.as_mut() {
            comm.no_bat_direction_from_room = room;
        }
        self.holo.still_relevant = false;
    }

    /// `ShelterPointer.Update`.
    fn shelter_update(&mut self, forcepoint: i32) {
        let room = self.cx.room_index;
        if forcepoint > -1 {
            self.set_door(forcepoint);
        } else if self.door_state().is_some_and(|d| d.last_room != room) {
            if let Some(d) = self.door_mut() {
                d.last_room = room;
            }
            let world = self.cx.world;
            let comm_last = self
                .ai
                .communication
                .as_ref()
                .map_or(-1, |c| c.last_player_room);
            let unwanted = self
                .ai
                .communication
                .as_ref()
                .map_or(-1, |c| c.un_wanted_shelter);
            let (mut best_door, mut best, mut best_shelter) = (-1, f32::MAX, 0);
            let shelters = world.guide_shelters();
            for i in 0..shelters.len() {
                let (mut door, mut dist) = (-1, f32::MAX);
                for j in 0..self.connections() {
                    if self.connection(j) > -1 {
                        let d =
                            world.guide_shelter_distance(i, WorldCoordinate::new(room, -1, -1, j));
                        if d > -1.0 && d < dist {
                            door = j;
                            dist = d;
                        }
                    }
                }
                if door < 0 {
                    continue;
                }
                // A new game has discovered no shelter (`GetDiscoveredShelterStringsInRegion`).
                let i = i32::try_from(i).unwrap_or(-1);
                if self.connection(door) == comm_last || i == unwanted {
                    dist = dist * 20.0 + 10_000.0;
                }
                if dist < best {
                    best = dist;
                    best_door = door;
                    best_shelter = i;
                }
            }
            if best_door > -1 {
                self.set_door(best_door);
                if let HologramKind::Door {
                    pointer:
                        PointerKind::Shelter {
                            currently_showing_to_shelter,
                            ..
                        },
                    ..
                } = &mut self.holo.kind
                {
                    *currently_showing_to_shelter = best_shelter;
                }
            } else {
                if let Some(comm) = self.ai.communication.as_mut() {
                    comm.no_shelter_direction_from_room = room;
                }
                self.holo.still_relevant = false;
            }
        }
        let door = self.door_state().map_or(-1, |d| d.door);
        let to = self.connection(door);
        let to_shelter = self.holo.still_relevant
            && door > -1
            && door < self.connections()
            && to > -1
            && self
                .cx
                .world
                .abstract_room(to)
                .is_some_and(rw_assets::world::AbstractRoomData::shelter);
        if let HologramKind::Door {
            pointer:
                PointerKind::Shelter {
                    showing_a_shelter_door,
                    ..
                },
            ..
        } = &mut self.holo.kind
        {
            if to_shelter {
                *showing_a_shelter_door = true;
            } else if *showing_a_shelter_door && let Some(comm) = self.ai.communication.as_mut() {
                comm.has_already_showed_player_to_a_shelter = true;
            }
        }
    }

    /// `DirectionPointer.Update`.
    fn direction_update(&mut self) {
        if let HologramKind::Door {
            pointer: PointerKind::Direction { cycle },
            ..
        } = &mut self.holo.kind
        {
            *cycle += 1;
        }
        let room = self.cx.room_index;
        if self.door_state().is_some_and(|d| d.last_room == room) {
            return;
        }
        if let Some(d) = self.door_mut() {
            d.last_room = room;
        }
        let (mut best_door, mut best) = (-1, f32::MAX);
        for i in 0..self.connections() {
            if self.connection(i) > -1 {
                let coord = WorldCoordinate::new(room, -1, -1, i);
                let d = self.cx.world.guide_direction_distance(coord);
                if d > -1.0 && d < best {
                    best_door = i;
                    best = d;
                }
            }
        }
        if best_door > -1 {
            self.set_door(best_door);
            return;
        }
        if let Some(comm) = self.ai.communication.as_mut() {
            comm.no_progression_direction_from_room = room;
        }
        self.holo.still_relevant = false;
    }

    /// `ObjectPointer.Update` and its subclasses': the creature pointer follows the
    /// communication module's most dangerous creature; no food object is ever realized
    /// in the port, so the food pointer falls out of relevance.
    fn object_update(&mut self, pointer: ObjectPointerKind) {
        let point_at = self.point_at();
        let to = self.holo.pos.direction_to(point_at);
        let viewed = self.score_view().viewed(point_at, -10.0);
        let arrow = &mut self.holo.parts[2];
        if let PartKind::Arrow { dir } = &mut arrow.kind {
            *dir = to;
        }
        arrow.visible = !viewed;
        let comm = self.ai.communication.as_ref();
        match pointer {
            ObjectPointerKind::Creature { .. } => self.creature_pointer_update(),
            ObjectPointerKind::Food { .. } => {
                self.holo.importance = comm.map_or(0.0, |c| c.most_delicous_food_delicious);
                self.holo.still_relevant = false;
            }
        }
        if let HologramKind::Object { pointer } = &mut self.holo.kind {
            match pointer {
                ObjectPointerKind::Creature { cycle, .. }
                | ObjectPointerKind::Food { cycle, .. } => {
                    *cycle += 1;
                }
            }
        }
    }

    /// `CreaturePointer.Update`: points at the most dangerous creature's realized body,
    /// and once shown long enough (on screen 10 ticks for a known type, else 40; off
    /// screen 80 to 120 by hand-holding) the player counts as warned of it.
    fn creature_pointer_update(&mut self) {
        let (most, danger) = self.ai.communication.as_ref().map_or((None, 0.0), |c| {
            (
                c.most_dangerous_creature_in_room,
                c.most_dangerous_creature_danger,
            )
        });
        if let Some(c) = most {
            let realized = self.cx.world.creature(c).is_realized().then_some(c);
            self.set_pointed_creature(realized);
        }
        self.holo.importance = danger;
        let point_at = match self.holo.kind {
            HologramKind::Object {
                pointer: ObjectPointerKind::Creature { point_at, .. },
            } => point_at,
            _ => None,
        };
        if self.holo.fade > 0.8
            && let Some(target) = point_at
        {
            let pos = self.point_at();
            let c = self.cx.world.creature(target);
            let (id, kind) = (c.id, c.template.kind);
            let knows = self.cx.guide.knows_creature_type(kind);
            let on_screen = self.score_view().viewed(pos, -10.0) && kind != CreatureType::PoleMimic;
            let off_limit = 80.0.lerp(120.0, self.cx.guide.hand_holding).to_int_cs();
            let mut warned = None;
            if let HologramKind::Object {
                pointer:
                    ObjectPointerKind::Creature {
                        has_warned_counter_visible: visible,
                        has_warned_counter_off_screen: off,
                        ..
                    },
            } = &mut self.holo.kind
            {
                if on_screen {
                    *visible += 1;
                    if *visible > if knows { 10 } else { 40 } {
                        warned = Some(true);
                        *visible = 0;
                    }
                } else {
                    *off += 1;
                    if *off > off_limit {
                        warned = Some(false);
                        *off = 0;
                    }
                }
            }
            if let Some(handled) = warned {
                if let Some(comm) = self.ai.communication.as_mut() {
                    comm.player_has_now_been_warned_of_creature(id, kind, handled, self.cx.guide);
                }
                self.holo.still_relevant = false;
            }
        }
        if self.holo.importance == 0.0 || point_at.is_none() {
            self.holo.still_relevant = false;
        }
    }

    fn set_pointed_creature(&mut self, creature: Option<crate::creature::CreatureId>) {
        if let HologramKind::Object {
            pointer: ObjectPointerKind::Creature { point_at, .. },
        } = &mut self.holo.kind
        {
            *point_at = creature;
        }
    }

    /// `AngryHologram.Update`.
    fn angry_update(&mut self) {
        let HologramKind::Angry { counter, show_time } = &mut self.holo.kind else {
            return;
        };
        *counter += 1;
        let c = *counter;
        let (alpha, flicker) = match &self.holo.parts[1].kind {
            PartKind::HoloImage(img) => (img.my_alpha, img.random_flicker),
            _ => (0.0, 1.0),
        };
        *show_time += 1;
        if alpha > 0.9 && flicker < 0.1 {
            *show_time += 1;
        }
        let shown = *show_time;
        self.holo.parts[0].visible = c % 40 < 20;
        self.abs.go_to_player = true;
        if let Some(p) = self.player() {
            let cross = &mut self.holo.parts[0];
            cross.offset = cross.offset.lerp(p.pos - self.holo.pos, 0.23) * 0.99;
        }
        if shown <= 800 {
            return;
        }
        self.holo.still_relevant = false;
        let guide = &mut *self.cx.guide;
        if self.ai.communication.is_some() {
            if !guide.displayed_anger() {
                guide.influence_like(-2.0);
            }
            guide.set_displayed_anger(true);
        }
        self.abs.player_guide_go_away(2000);
        let den = self.cx.world.offscreen_den_index();
        self.base
            .set_destination_no_pathing(WorldCoordinate::new(den, -1, -1, 0), true);
    }

    /// `OverseerImage.Update`.
    fn image_update(&mut self) {
        let flicker = match &self.holo.parts[0].kind {
            PartKind::HoloImage(img) => img.random_flicker,
            _ => 1.0,
        };
        let room_name = self.room_name().to_owned();
        let HologramKind::Image(image) = &mut self.holo.kind else {
            return;
        };
        if flicker > 0.5 {
            image.curr_image = 0;
        }
        if !image.images.is_empty() {
            if flicker < 0.2 {
                image.counter += 1;
            }
            if image.counter > image.time_on_each_image {
                image.counter = 0;
                image.curr_image += 1;
                if image.curr_image >= image.images.len() {
                    image.curr_image = 0;
                }
            }
        }
        let Some(comm) = self.ai.communication.as_ref() else {
            return;
        };
        let showed = comm.showed_image_time;
        if !image.logged_as_seen && showed > image.show_time / 3 {
            self.cx.guide.image_shown_in_room(&room_name);
            image.logged_as_seen = true;
        }
        if showed > image.show_time {
            self.holo.still_relevant = false;
        }
        // `room.regionGate` is null: gates are not ported.
    }
}

impl OverseerHologram {
    /// A pointing hologram's parts: arrow, light effect (and an `ObjectPointer`'s own
    /// arrow), then the symbol.
    fn pointing_parts(&mut self, object: bool, symbol: super::PartKind) {
        self.add_part(super::HologramPart::new(PartKind::Arrow {
            dir: Vec2::ZERO,
        }));
        self.add_part(super::HologramPart::new(PartKind::LightEffect {
            light_alpha: 0.6,
            dark_alpha: 0.2,
            light_rad: 40.0,
            dark_rad: 80.0,
        }));
        if object {
            self.add_part(super::HologramPart::new(PartKind::Arrow {
                dir: Vec2::ZERO,
            }));
        }
        self.add_part(super::HologramPart::new(symbol));
    }

    /// A `DoorPointer` (`doorPos` starts at the overseer).
    pub(crate) fn new_door_pointer(
        id: u64,
        message: HologramMessage,
        importance: f32,
        room: i32,
        pointer: PointerKind,
        symbol: &'static str,
        overseer_pos: Vec2,
    ) -> Self {
        let mut h = Self::new(id, message, importance, room);
        h.pointing_parts(false, PartKind::Symbol(super::SymbolPart::new(symbol)));
        h.kind = HologramKind::Door {
            pointer,
            door: DoorPointer {
                door: -1,
                door_pos: overseer_pos,
                report_player_exit_entrance: true,
                pointer_necessary: true,
                tracker: false,
                last_room: -1,
            },
        };
        h
    }

    /// A `CreaturePointer` or `FoodPointer`.
    pub(crate) fn new_object_pointer(
        id: u64,
        message: HologramMessage,
        importance: f32,
        room: i32,
        pointer: ObjectPointerKind,
    ) -> Self {
        let mut h = Self::new(id, message, importance, room);
        let symbol = match pointer {
            ObjectPointerKind::Creature { .. } => {
                PartKind::Symbol(super::SymbolPart::new("miscDangerSymbol"))
            }
            ObjectPointerKind::Food { .. } => PartKind::CyclingSymbol {
                symbol: super::SymbolPart::new("foodSymbol"),
                elements: vec!["foodSymbol"],
                curr: 0,
                cntr: 0,
                cycle_length: 15,
            },
        };
        h.pointing_parts(true, symbol);
        h.kind = HologramKind::Object { pointer };
        h
    }

    /// `HoloImage`'s constructor draws its first random image.
    pub(super) fn holo_image(rng: &mut rw_random::UnityRandom) -> super::HologramPart {
        super::HologramPart::new(PartKind::HoloImage(Box::new(HoloImage {
            last_img: 0,
            last_show_img: (0, 0),
            pan_pos: Vec2::ZERO,
            last_pan_pos: Vec2::ZERO,
            pan_vel: Vec2::ZERO,
            pan_center: Vec2::ZERO,
            my_alpha: 0.0,
            last_my_alpha: 0.0,
            random_image: rng.range_i32(0, 25),
            random_image_time: 0.0,
            random_flicker: 1.0,
            show_random_flicker_image: false,
            sound: false,
        })))
    }

    /// `AngryHologram`: red cross, image, frame.
    pub(crate) fn new_angry(
        id: u64,
        importance: f32,
        room: i32,
        rng: &mut rw_random::UnityRandom,
    ) -> Self {
        let mut h = Self::new(id, HologramMessage::Angry, importance, room);
        h.add_part(super::HologramPart::new(PartKind::RedCross {
            small: false,
        }));
        h.add_part(Self::holo_image(rng));
        h.add_part(super::HologramPart::new(PartKind::Frame));
        h.kind = HologramKind::Angry {
            counter: 0,
            show_time: 0,
        };
        h
    }

    /// `OverseerImage` (the guide's `GateScene` message): the room's pictures, the image
    /// and its frame.
    pub(crate) fn new_image(
        id: u64,
        importance: f32,
        room: i32,
        image: ImageHologram,
        rng: &mut rw_random::UnityRandom,
    ) -> Self {
        let mut h = Self::new(id, HologramMessage::GateScene, importance, room);
        h.add_part(Self::holo_image(rng));
        h.add_part(super::HologramPart::new(PartKind::Frame));
        h.kind = HologramKind::Image(image);
        h
    }

    /// `InfluenceHoverScoreOfTile` for a tile whose middle is `mid`: pointers keep their
    /// overseer near what they point at, the angry hologram near the player (drawing).
    /// `object_point_at` is an object pointer's `PointAt`.
    pub(crate) fn influence_hover_score(
        &self,
        mid: Vec2,
        f: f32,
        player_pos: Option<Vec2>,
        object_point_at: Vec2,
        rng: &mut rw_random::UnityRandom,
    ) -> f32 {
        let point_at = match &self.kind {
            HologramKind::Door { door, .. } => door.door_pos,
            HologramKind::Object { .. } => object_point_at,
            HologramKind::Angry { .. } => {
                return match player_pos {
                    Some(p) => f + mid.distance(p) * 0.1.lerp(1.9, rng.value()),
                    None => f,
                };
            }
            _ => return f,
        };
        let danger = player_pos.unwrap_or_default();
        let f = f + mid.distance(point_at) * 0.1;
        if mid.distance(point_at) > danger.distance(point_at) {
            f + 2000.0
        } else {
            f
        }
    }

    /// `lookAt` (the overseer looks there while projecting).
    #[must_use]
    pub fn look_at(
        &self,
        player_pos: Option<Vec2>,
        overseer_pos: Vec2,
        object_point_at: Vec2,
    ) -> Vec2 {
        let pointing = matches!(
            self.kind,
            HologramKind::Door { .. } | HologramKind::Object { .. }
        );
        if let HologramKind::Angry { .. } = self.kind {
            return player_pos.unwrap_or(overseer_pos);
        }
        if self.fade > 0.0 && self.fade < 1.0 {
            return self.pos;
        }
        match player_pos.filter(|_| self.look_at_communication_creature) {
            Some(p) => p,
            None if pointing => match &self.kind {
                HologramKind::Door { door, .. } => door.door_pos,
                _ => object_point_at,
            },
            None => self.pos,
        }
    }

    /// A creature pointer's `PointAt` while its creature is realized: its `DangerPos`.
    #[must_use]
    pub fn pointed_creature_pos(&self, world: &crate::world::World) -> Option<Vec2> {
        let HologramKind::Object {
            pointer:
                ObjectPointerKind::Creature {
                    point_at: Some(c), ..
                },
        } = self.kind
        else {
            return None;
        };
        world.creature(c).realized.as_ref()?.danger_pos()
    }
}
