//! `OverseerTutorialBehavior.InputInstruction`: the pictures of the controls the guide
//! projects during the tutorial and for the in-world pick-up trigger (the in-world super
//! jump and scavenger trade ones are not ported), with their controllers.

use super::parts::{ButtonState, HologramPart, PartKind, SymbolPart};
use super::{ControlPreset, HologramMessage, HologramUpdate, OverseerHologram};
use crate::{FloatExt, Vec2};

/// Which picture of the controls.
#[derive(Debug, Clone, PartialEq)]
pub enum InstructionDevice {
    /// `KeyBoardInstruction`: `keys` are the part indices of Left, Up, Right, Down,
    /// PickUp, Jump, Throw.
    Keyboard { keys: [usize; 7] },
    /// `GamePadInstruction`: `parts` are socket, PickUp, Jump, Throw.
    GamePad {
        silhouette: usize,
        symbol: Option<usize>,
        socket: usize,
        buttons: [usize; 3],
        parts_remain_visible: [i32; 4],
        direction_always_visible: bool,
    },
    /// `SwitchInstruction`: `buttons` are Unused, Throw, Jump, PickUp.
    Switch {
        socket: usize,
        buttons: [usize; 4],
        parts_offset: [Vec2; 5],
    },
}

/// `InputInstruction.InputInstructionController`.
#[derive(Debug, Clone, PartialEq)]
pub enum Controller {
    GetUpOnFirstBox,
    ClimbPole,
    /// `SuperJump` (the in-tutorial one when `in_tutorial_section`).
    SuperJump {
        in_tutorial_section: bool,
        in_position_counter: i32,
        move_keys_up: bool,
        jump_direction: i32,
    },
    /// `Eat`: `food_arrows` are part indices.
    Eat {
        food_arrows: [usize; 4],
        still_counter: i32,
        moving_counter: i32,
        tell_player_to_stay_still_counter: i32,
        show_slow_button_press: bool,
    },
    /// `PickupObjectInputInstructionController`.
    PickupObject {
        arrow: usize,
        text_shown: bool,
    },
    /// `ScavTradeInputInstructionController` (its trigger is not ported).
    ScavengerTrade,
}

/// `WaitMeter`: 16 boxes round the player that fill up.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitMeter {
    pub filled: f32,
    /// Part indices of the 16 `MeterBox`es.
    pub parts: Vec<usize>,
    pub offset: Vec2,
    pub blink_counter: i32,
    pub blink: bool,
    pub visible: bool,
    pub visibility: f32,
    pub blink_when_full: bool,
}

/// What kind of instruction picture (`KeyBoard`/`GamePad`/`Switch`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionKind {
    Keyboard,
    GamePad,
    Switch,
}

/// `InputInstruction`'s state.
#[derive(Debug, Clone, PartialEq)]
pub struct InputInstruction {
    pub device: InstructionDevice,
    /// `None` when the instruction came without a tutorial (then it is not relevant).
    pub controller: Option<Controller>,
    pub direction: (i32, i32),
    pub show_pickup: bool,
    pub show_jump: bool,
    pub show_throw: bool,
    pub direction_only: bool,
    pub hide_inputs: bool,
    pub close_to_pos: Vec2,
    pub player_ghost: Option<usize>,
    pub wait_meter: Option<WaitMeter>,
    /// `dontTouchInputCross`.
    pub red_cross: Option<usize>,
}

impl InputInstruction {
    #[must_use]
    pub fn kind(&self) -> InstructionKind {
        match self.device {
            InstructionDevice::Keyboard { .. } => InstructionKind::Keyboard,
            InstructionDevice::GamePad { .. } => InstructionKind::GamePad,
            InstructionDevice::Switch { .. } => InstructionKind::Switch,
        }
    }
}

impl OverseerHologram {
    /// `KeyBoardInstruction`, `GamePadInstruction` or `SwitchInstruction` for `message`:
    /// the `InputInstruction` constructor's parts (player ghost, wait meter, food
    /// arrows), then the device's, then the red cross. `overseer_pos` places the
    /// in-world super jump's ghost.
    pub(crate) fn new_instruction(
        id: u64,
        message: HologramMessage,
        importance: f32,
        room: i32,
        preset: ControlPreset,
        tutorial: bool,
        overseer_pos: Vec2,
    ) -> Self {
        let mut h = Self::new(id, message, importance, room);
        let mut ghost = None;
        let mut wait_meter = None;
        let mut add_red_cross = false;
        let in_world = matches!(
            message,
            HologramMessage::InWorldSuperJump
                | HologramMessage::PickupObject
                | HologramMessage::ScavengerTrade
        );
        let controller = if !tutorial && !in_world {
            h.still_relevant = false;
            None
        } else {
            match message {
                HologramMessage::GetUpOnFirstBox => Some(Controller::GetUpOnFirstBox),
                HologramMessage::ClimbPole => {
                    ghost =
                        Some(h.add_ghost("playerStandingSymbol", Some(Vec2::new(524.0, 160.0))));
                    Some(Controller::ClimbPole)
                }
                HologramMessage::SuperJump | HologramMessage::InWorldSuperJump => {
                    let in_tutorial_section = message == HologramMessage::SuperJump;
                    let at = if in_tutorial_section {
                        Vec2::new(316.0, 460.0)
                    } else {
                        overseer_pos
                    };
                    ghost = Some(h.add_ghost("playerCrouchingSymbol", Some(at)));
                    wait_meter = Some(h.add_wait_meter());
                    Some(Controller::SuperJump {
                        in_tutorial_section,
                        in_position_counter: 0,
                        move_keys_up: false,
                        jump_direction: i32::from(in_tutorial_section),
                    })
                }
                HologramMessage::EatInstruction => {
                    let food_arrows = [0; 4].map(|_| {
                        let mut arrow = HologramPart::new(PartKind::FoodArrow);
                        arrow.visible = false;
                        h.add_part(arrow)
                    });
                    wait_meter = Some(h.add_wait_meter());
                    add_red_cross = true;
                    Some(Controller::Eat {
                        food_arrows,
                        still_counter: 0,
                        moving_counter: 0,
                        tell_player_to_stay_still_counter: 0,
                        show_slow_button_press: false,
                    })
                }
                HologramMessage::PickupObject => {
                    let mut arrow = HologramPart::new(PartKind::FoodArrow);
                    arrow.visible = false;
                    Some(Controller::PickupObject {
                        arrow: h.add_part(arrow),
                        text_shown: false,
                    })
                }
                HologramMessage::ScavengerTrade => Some(Controller::ScavengerTrade),
                _ => None,
            }
        };
        let device = match preset {
            ControlPreset::Keyboard => h.add_keyboard(),
            ControlPreset::XBox | ControlPreset::Ps4DualShock => h.add_game_pad(preset),
            ControlPreset::Ps5DualSense | ControlPreset::Switch => h.add_switch(),
        };
        let red_cross = add_red_cross.then(|| {
            let small = matches!(device, InstructionDevice::GamePad { .. });
            let mut cross = HologramPart::new(PartKind::RedCross { small });
            cross.visible = false;
            h.add_part(cross)
        });
        h.kind = super::HologramKind::Instruction(Box::new(InputInstruction {
            device,
            controller,
            direction: (0, 0),
            show_pickup: false,
            show_jump: false,
            show_throw: false,
            direction_only: false,
            hide_inputs: false,
            close_to_pos: Vec2::ZERO,
            player_ghost: ghost,
            wait_meter,
            red_cross,
        }));
        h
    }

    fn add_ghost(&mut self, element: &'static str, room_position: Option<Vec2>) -> usize {
        let symbol = SymbolPart {
            anchor_y: 0.0,
            ..SymbolPart::new(element)
        };
        self.add_part(HologramPart::new(PartKind::PlayerGhost {
            symbol,
            room_position,
        }))
    }

    /// `new WaitMeter(this)`.
    fn add_wait_meter(&mut self) -> WaitMeter {
        let parts = (0..16)
            .map(|i| {
                self.add_part(HologramPart::new(PartKind::MeterBox {
                    f: i as f32 / 16.0,
                    lit: false,
                    fill: [1.0; 4],
                }))
            })
            .collect();
        WaitMeter {
            filled: 0.0,
            parts,
            offset: Vec2::ZERO,
            blink_counter: 0,
            blink: false,
            visible: false,
            visibility: 0.0,
            blink_when_full: true,
        }
    }

    /// `KeyBoardInstruction`'s keys.
    fn add_keyboard(&mut self) -> InstructionDevice {
        let layout: [(&'static str, f32, Vec2, f32); 7] = [
            ("Arrow", -90.0, Vec2::ZERO, 0.0),
            ("Arrow", 0.0, Vec2::new(35.0, 35.0), 0.0),
            ("Arrow", 90.0, Vec2::new(70.0, 0.0), 0.0),
            ("Arrow", 180.0, Vec2::new(35.0, 0.0), 0.0),
            ("Shift", 0.0, Vec2::new(-125.0, 0.0), 10.0),
            ("Z", 0.0, Vec2::new(-80.0, 0.0), 0.0),
            ("X", 0.0, Vec2::new(-45.0, 0.0), 0.0),
        ];
        let keys = layout.map(|(symbol, rotation, offset, wider)| {
            let button = ButtonState {
                symbol_rotation: rotation,
                ..ButtonState::new(symbol)
            };
            self.add_part(HologramPart::at(PartKind::Key { button, wider }, offset))
        });
        InstructionDevice::Keyboard { keys }
    }

    /// `GamePadInstruction`'s parts (the PlayStation pictures' `ps*controllertiny`
    /// symbol is an illustration file).
    fn add_game_pad(&mut self, preset: ControlPreset) -> InstructionDevice {
        let socket = self.add_part(HologramPart::new(PartKind::GamePadStickSocket {
            scale: 0.6,
        }));
        self.add_part(HologramPart::new(PartKind::GamePadStick {
            socket,
            counter: 0,
            out_to_side: false,
            stick_pos: Vec2::ZERO,
            stick_vel: Vec2::ZERO,
            switch: false,
        }));
        let ps = preset == ControlPreset::Ps4DualShock;
        let symbol = ps.then(|| {
            self.add_part(HologramPart::at(
                PartKind::Symbol(SymbolPart::new("ps4controllertiny")),
                Vec2::new(0.0, -40.0),
            ))
        });
        let base_pos = if ps {
            Vec2::new(15.0, 6.0)
        } else {
            Vec2::new(12.0, 5.0)
        };
        let silhouette = self.add_part(HologramPart::at(
            PartKind::GamePadSilhouette {
                xbox: preset == ControlPreset::XBox,
                ps,
                base_pos,
            },
            Vec2::new(0.0, -40.0),
        ));
        let looks: [(&'static str, [f32; 3]); 3] = if ps {
            [
                ("Square", [0.9, 0.3, 1.0]),
                ("Cross", [0.5, 0.5, 1.0]),
                ("Circle", [1.0, 0.3, 0.3]),
            ]
        } else {
            [
                ("X", [0.2, 0.6, 1.0]),
                ("A", [0.4, 1.0, 0.2]),
                ("B", [1.0, 0.2, 0.2]),
            ]
        };
        let buttons = looks.map(|(symbol, color)| {
            self.add_part(HologramPart::new(PartKind::GamePadButton {
                button: ButtonState::new(symbol),
                round: ps,
                color,
            }))
        });
        InstructionDevice::GamePad {
            silhouette,
            symbol,
            socket,
            buttons,
            parts_remain_visible: [0; 4],
            direction_always_visible: false,
        }
    }

    /// `SwitchInstruction`'s parts.
    fn add_switch(&mut self) -> InstructionDevice {
        let socket = self.add_part(HologramPart::new(PartKind::GamePadStickSocket {
            scale: 0.6,
        }));
        self.add_part(HologramPart::new(PartKind::GamePadStick {
            socket,
            counter: 0,
            out_to_side: false,
            stick_pos: Vec2::ZERO,
            stick_vel: Vec2::ZERO,
            switch: true,
        }));
        let buttons = [0; 4].map(|_| {
            self.add_part(HologramPart::new(PartKind::SwitchButton(ButtonState::new(
                "",
            ))))
        });
        let mut parts_offset = [Vec2::new(-30.0, 0.0); 5];
        for (j, p) in parts_offset.iter_mut().skip(1).enumerate() {
            *p = Vec2::new(30.0, 0.0) + Vec2::from_degrees(j as f32 * 90.0) * 16.0;
        }
        InstructionDevice::Switch {
            socket,
            buttons,
            parts_offset,
        }
    }
}

impl WaitMeter {
    /// `WaitMeter.Update`.
    fn update(&self, parts: &mut [HologramPart], blink_counter: &mut i32, blink: &mut bool) {
        if self.filled == 1.0 && self.blink_when_full {
            *blink_counter -= 1;
            if *blink_counter < 1 {
                *blink = !*blink;
                *blink_counter = 10;
                for &p in &self.parts {
                    if let PartKind::MeterBox { lit, .. } = &mut parts[p].kind {
                        *lit = *blink && self.visible;
                    }
                }
            }
        } else {
            for (j, &p) in self.parts.iter().enumerate() {
                if let PartKind::MeterBox { lit, .. } = &mut parts[p].kind {
                    *lit = (j as f32 / 16.0) < self.filled && self.visible;
                }
            }
            *blink_counter = 0;
        }
        for (k, &p) in self.parts.iter().enumerate() {
            parts[p].visible = (k as f32 / 16.0) < self.visibility;
            parts[p].offset = self.offset;
        }
    }
}

impl OverseerHologram {
    /// `WaitMeter.Update` of the instruction's meter.
    fn wait_meter_update(&mut self) {
        let super::HologramKind::Instruction(i) = &mut self.kind else {
            return;
        };
        let Some(meter) = i.wait_meter.as_mut() else {
            return;
        };
        let (mut counter, mut blink) = (meter.blink_counter, meter.blink);
        meter.update(&mut self.parts, &mut counter, &mut blink);
        meter.blink_counter = counter;
        meter.blink = blink;
        let to = if meter.visible { 1.0 } else { 0.0 };
        meter.visibility = meter.visibility.lerp_and_tick(to, 0.1, 0.2);
    }

    fn instruction_mut(&mut self) -> Option<&mut InputInstruction> {
        match &mut self.kind {
            super::HologramKind::Instruction(i) => Some(i),
            _ => None,
        }
    }

    /// The device's `Jump` (or `PickUp`) key or button.
    fn device_button(&mut self, pick_up: bool) -> Option<&mut ButtonState> {
        let i = self.instruction()?;
        let index = match &i.device {
            InstructionDevice::Keyboard { keys } => keys[if pick_up { 4 } else { 5 }],
            InstructionDevice::GamePad { buttons, .. } => buttons[usize::from(!pick_up)],
            InstructionDevice::Switch { buttons, .. } => buttons[if pick_up { 3 } else { 2 }],
        };
        self.parts[index].button_mut()
    }

    /// The device part of `KeyBoardInstruction`/`GamePadInstruction`/`SwitchInstruction.Update`.
    fn device_update(&mut self) {
        let Some(i) = self.instruction().cloned() else {
            return;
        };
        match i.device {
            InstructionDevice::Keyboard { keys } => {
                let buttons_shown = !i.direction_only && !i.hide_inputs;
                let pulses = [
                    i.direction.0 < 0,
                    i.direction.1 > 0,
                    i.direction.0 > 0,
                    i.direction.1 < 0,
                    i.show_pickup,
                    i.show_jump,
                    i.show_throw,
                ];
                for (k, (&key, pulse)) in keys.iter().zip(pulses).enumerate() {
                    let part = &mut self.parts[key];
                    part.visible = if k >= 4 {
                        buttons_shown
                    } else {
                        !i.hide_inputs
                    };
                    if let Some(b) = part.button_mut() {
                        b.pulsate = pulse;
                    }
                }
            }
            InstructionDevice::GamePad {
                silhouette,
                symbol,
                socket,
                buttons,
                mut parts_remain_visible,
                direction_always_visible,
            } => {
                let order = [socket, buttons[0], buttons[1], buttons[2]];
                self.parts[silhouette].visible = !i.hide_inputs;
                if let Some(s) = symbol {
                    self.parts[s].visible = !i.hide_inputs;
                }
                if i.hide_inputs {
                    parts_remain_visible = [0; 4];
                } else {
                    for r in &mut parts_remain_visible {
                        *r = (*r - 1).max(0);
                    }
                    if direction_always_visible || i.direction != (0, 0) {
                        parts_remain_visible[0] = parts_remain_visible[0].max(30);
                    }
                    if i.red_cross.is_some_and(|c| self.parts[c].visible) {
                        parts_remain_visible[0] = parts_remain_visible[0].max(2);
                    }
                    for (k, show) in [i.show_pickup, i.show_jump, i.show_throw]
                        .into_iter()
                        .enumerate()
                    {
                        if show {
                            parts_remain_visible[k + 1] = parts_remain_visible[k + 1].max(30);
                        }
                    }
                }
                for (k, &p) in order.iter().enumerate() {
                    self.parts[p].visible = parts_remain_visible[k] > 0;
                }
                if let Some(c) = i.red_cross {
                    self.parts[c].offset = self.parts[socket].offset;
                }
                let shown: Vec<usize> = order
                    .iter()
                    .copied()
                    .filter(|&p| self.parts[p].visible)
                    .collect();
                if shown.len() == 1 {
                    let p = &mut self.parts[shown[0]];
                    p.offset = p.offset.move_towards(Vec2::ZERO, 2.0).lerp(Vec2::ZERO, 0.1);
                } else {
                    let span = (shown.len() as f32 - 1.0) * 25.0;
                    for (n, &p) in shown.iter().enumerate() {
                        let t = n as f32 / (shown.len() as f32 - 1.0);
                        let to = Vec2::new((-span).lerp(span, t), 0.0);
                        let part = &mut self.parts[p];
                        part.offset = part.offset.move_towards(to, 2.0).lerp(to, 0.1);
                    }
                }
                for (k, show) in [i.show_pickup, i.show_jump, i.show_throw]
                    .into_iter()
                    .enumerate()
                {
                    if let Some(b) = self.parts[buttons[k]].button_mut() {
                        b.pulsate = show;
                    }
                }
                if let Some(InputInstruction {
                    device:
                        InstructionDevice::GamePad {
                            parts_remain_visible: r,
                            ..
                        },
                    ..
                }) = self.instruction_mut()
                {
                    *r = parts_remain_visible;
                }
            }
            InstructionDevice::Switch {
                socket,
                buttons,
                parts_offset,
            } => {
                if let Some(c) = i.red_cross {
                    self.parts[c].offset = self.parts[socket].offset;
                }
                let s = &mut self.parts[socket];
                s.offset = s
                    .offset
                    .move_towards(parts_offset[0], 2.0)
                    .lerp(parts_offset[0], 0.1);
                s.visible = !i.hide_inputs;
                for (k, &b) in buttons.iter().enumerate() {
                    let to = parts_offset[k + 1];
                    let part = &mut self.parts[b];
                    part.offset = part.offset.move_towards(to, 2.0).lerp(to, 0.1);
                    part.visible = !i.hide_inputs;
                }
                for (b, show) in [
                    (buttons[3], i.show_pickup),
                    (buttons[2], i.show_jump),
                    (buttons[1], i.show_throw),
                ] {
                    if let Some(button) = self.parts[b].button_mut() {
                        button.pulsate = show;
                    }
                }
            }
        }
    }
}

impl HologramUpdate<'_, '_> {
    /// `InputInstruction.Update` (the base, the controller, the wait meter) then the
    /// device's part.
    pub(super) fn instruction_update(&mut self) {
        self.controller_update();
        self.holo.wait_meter_update();
        self.holo.device_update();
    }

    fn controller_update(&mut self) {
        let Some(controller) = self.holo.instruction().and_then(|i| i.controller.clone()) else {
            return;
        };
        if let Controller::PickupObject { arrow, text_shown } = controller {
            self.pickup_object(arrow, text_shown);
            return;
        }
        let Some(p) = self.cx.player else {
            return;
        };
        match controller {
            Controller::GetUpOnFirstBox => self.get_up_on_first_box(p),
            Controller::ClimbPole => self.climb_pole(p),
            Controller::SuperJump { .. } => self.super_jump(p),
            Controller::Eat { .. } => self.eat(p),
            Controller::PickupObject { .. } | Controller::ScavengerTrade => {
                // `ScavengerTradeInstructionTrigger` is not ported.
                self.holo.still_relevant = false;
            }
        }
    }

    /// `PickupObjectInputInstructionController.Update`: points at the guide's
    /// `PickupObjectInstruction` (SU_A23's text the first time), and counts it once the
    /// player carries a rock or spear.
    fn pickup_object(&mut self, arrow: usize, mut text_shown: bool) {
        let id = self
            .ai
            .communication
            .as_ref()
            .and_then(|c| c.input_instruction);
        let Some(view) = id
            .and_then(|id| self.cx.instructions.iter().find(|i| i.id == id))
            .copied()
            .filter(|i| !i.slated)
        else {
            self.holo.still_relevant = false;
            return;
        };
        let in_su_a23 = self
            .cx
            .world
            .abstract_room(self.cx.room_index)
            .is_some_and(|r| r.name == "SU_A23");
        if !text_shown && self.cx.hud && in_su_a23 {
            self.cx
                .effects
                .push(crate::creature::overseer::OverseerEffect::Message(
                    crate::creature::overseer::TutorialMessage {
                        text: "Objects can be picked up, carried, and tossed",
                        wait: 0,
                        time: 160,
                        hide_hud: true,
                    },
                ));
            text_shown = true;
        }
        let picked = view.player_has_picked_up_an_item;
        let pos = self.holo.pos;
        self.holo.parts[arrow].visible = !picked;
        self.holo.parts[arrow].offset = view.instruction_pos + Vec2::new(0.0, 20.0) - pos;
        if let Some(i) = self.holo.instruction_mut() {
            i.close_to_pos = view.instruction_pos + Vec2::new(0.0, 60.0);
            i.show_pickup = !picked;
            i.show_throw = picked;
            i.controller = Some(Controller::PickupObject { arrow, text_shown });
        }
        if view.mark_instruction_as_followed {
            // `InWorldInputInstructionCompleted`.
            log::info!("input instruction completed");
            self.cx.guide.pickup_objects_shown += 1;
            self.cx
                .effects
                .push(crate::creature::overseer::OverseerEffect::InstructionFollowed(view.id));
            if let Some(i) = self.cx.instructions.iter_mut().find(|i| i.id == view.id) {
                i.mark_instruction_as_followed = false;
            }
        }
    }

    /// `GetUpOnFirstBox.Update`.
    fn get_up_on_first_box(&mut self, p: crate::creature::overseer::PlayerView) {
        let jumped = self
            .ai
            .tutorial
            .as_ref()
            .is_some_and(|t| t.player_has_jumped_over_box);
        let Some(i) = self.holo.instruction_mut() else {
            return;
        };
        i.close_to_pos = Vec2::new(1800.0, 300.0);
        if p.standing {
            i.direction = (0, 0);
            i.show_jump = true;
        } else {
            i.direction = (0, 1);
            i.show_jump = false;
        }
        self.holo.still_relevant =
            !jumped && crate::creature::overseer::TutorialBehavior::in_box_zone(p.hips.x);
        if p.hips.x > 1836.0 && p.hips.y > 174.0 && p.hips_contact_y < 0 {
            log::info!("Yay! Box cleared!");
            if let Some(t) = self.ai.tutorial.as_mut() {
                t.player_has_jumped_over_box = true;
            }
        }
    }

    /// `ClimbPole.Update`.
    fn climb_pole(&mut self, p: crate::creature::overseer::PlayerView) {
        let climbed = self
            .ai
            .tutorial
            .as_ref()
            .is_some_and(|t| t.player_has_climbed_pole);
        let Some(i) = self.holo.instruction_mut() else {
            return;
        };
        i.close_to_pos = Vec2::new(570.0, 400.0);
        i.direction = (0, 1);
        let ghost = i.player_ghost;
        self.holo.still_relevant = !climbed && p.hips.x > 360.0 && p.hips.x < 660.0;
        let on_beam = p.animation == crate::Animation::ClimbOnBeam;
        if p.hips.y > 280.0 && on_beam {
            log::info!("Yay! Pole climbed!");
            if let Some(t) = self.ai.tutorial.as_mut() {
                t.player_has_climbed_pole = true;
            }
        }
        if let Some(g) = ghost {
            self.holo.parts[g].visible = p.hips.y < 200.0 && !on_beam;
        }
    }

    /// `SuperJump.Update` (in the tutorial; the in-world trigger is not ported).
    fn super_jump(&mut self, p: crate::creature::overseer::PlayerView) {
        let Some(Controller::SuperJump {
            in_tutorial_section,
            mut in_position_counter,
            move_keys_up,
            jump_direction,
        }) = self.holo.instruction().and_then(|i| i.controller.clone())
        else {
            return;
        };
        if in_tutorial_section
            && let Some(t) = self.ai.tutorial.as_mut()
            && t.super_jump_attempt_time > 0
            && in_position_counter < 10
        {
            t.super_jump_attempt_time += 1;
        }
        if let Some(b) = self.holo.device_button(false) {
            b.time_up = 4;
            b.time_down = 40;
        }
        if let Some(InputInstruction {
            device:
                InstructionDevice::GamePad {
                    parts_remain_visible,
                    ..
                },
            ..
        }) = self.holo.instruction_mut()
        {
            parts_remain_visible[2] = parts_remain_visible[2].max(5);
        }
        if !move_keys_up
            && let Some(InstructionDevice::Keyboard { keys }) =
                self.holo.instruction().map(|i| i.device.clone())
        {
            for k in keys {
                self.holo.parts[k].offset.y += 50.0;
            }
        }
        let made = self
            .ai
            .tutorial
            .as_ref()
            .is_some_and(|t| t.player_has_made_super_jump);
        if in_tutorial_section {
            if let Some(i) = self.holo.instruction_mut() {
                i.close_to_pos = Vec2::new(480.0, 540.0);
            }
            self.holo.still_relevant = self.holo.still_relevant && !made && p.hips.x < 505.0;
            if p.hips.x > 430.0 && p.hips.y > 415.0 && p.hips_contact_y < 0 {
                log::info!("Yay! Superjump done!");
                if let Some(t) = self.ai.tutorial.as_mut() {
                    t.player_has_made_super_jump = true;
                }
            }
        } else {
            // `communication.inputInstruction` (a `SuperJumpInstruction`) is never set.
            self.holo.still_relevant = false;
        }
        let ghost = self.holo.instruction().and_then(|i| i.player_ghost);
        let ghost_at = ghost.and_then(|g| match &self.holo.parts[g].kind {
            PartKind::PlayerGhost { room_position, .. } => *room_position,
            _ => None,
        });
        let middle = (p.pos + p.hips) / 2.0;
        let mut hold = false;
        let mut direction = (0, 0);
        let mut show_jump = false;
        let mut meter_visible = None;
        let near = ghost_at.is_some_and(|at| middle.dist_less(at, 30.0));
        if near {
            if let Some(g) = ghost {
                self.holo.parts[g].visible = false;
            }
            if in_tutorial_section && let Some(t) = self.ai.tutorial.as_mut() {
                if t.super_jump_attempt_time == 0 {
                    t.super_jump_attempt_time += 1;
                } else if t.super_jump_attempt_time > 1400 {
                    t.super_jump_attempt_time = -1;
                    self.tutorial_text("Briefly tap DOWN to crouch", 0, 200);
                    self.tutorial_text(
                        "When crouching, hold the JUMP button without giving any other input",
                        0,
                        280,
                    );
                }
            }
            if p.standing {
                direction = (0, -1);
            } else if (p.pos.x < p.hips.x) != (jump_direction < 0) {
                direction = (jump_direction, 0);
            } else {
                hold = p.input_x == 0 && p.input_y == 0 && p.jump[0] && !p.pickup && !p.throw;
                meter_visible = Some(true);
                show_jump = in_position_counter < 80;
            }
        } else {
            if let Some(g) = ghost {
                self.holo.parts[g].visible = true;
            }
            meter_visible = Some(false);
        }
        in_position_counter = if hold { in_position_counter + 1 } else { 0 };
        if in_tutorial_section && in_position_counter < 60 {
            self.cx
                .effects
                .push(crate::creature::overseer::OverseerEffect::ResetSuperLaunchJump);
        }
        let pos = self.holo.pos;
        let Some(i) = self.holo.instruction_mut() else {
            return;
        };
        i.direction = direction;
        i.show_jump = show_jump;
        i.hide_inputs = !near;
        if let Some(m) = i.wait_meter.as_mut() {
            if let Some(v) = meter_visible {
                m.visible = v;
            }
            m.offset = middle - pos;
            m.filled = (in_position_counter as f32).inverse_lerp(0.0, 80.0);
            i.direction_only = !m.visible;
        }
        i.controller = Some(Controller::SuperJump {
            in_tutorial_section,
            in_position_counter,
            move_keys_up: true,
            jump_direction,
        });
    }

    /// `Eat.Update`: arrows over the free edibles; once the player holds one, the wait
    /// meter fills with `eatCounter` while it stands still, and the red cross shows when
    /// it keeps moving.
    fn eat(&mut self, p: crate::creature::overseer::PlayerView) {
        let Some(Controller::Eat {
            food_arrows,
            mut still_counter,
            mut moving_counter,
            mut tell_player_to_stay_still_counter,
            mut show_slow_button_press,
        }) = self.holo.instruction().and_then(|i| i.controller.clone())
        else {
            return;
        };
        if let Some(m) = self
            .holo
            .instruction_mut()
            .and_then(|i| i.wait_meter.as_mut())
        {
            m.blink_when_full = false;
        }
        if let Some(b) = self.holo.device_button(true) {
            b.time_up = if show_slow_button_press { 4 } else { 10 };
            b.time_down = if show_slow_button_press { 40 } else { 10 };
        }
        let world = self.cx.world;
        let name = world
            .abstract_room(self.cx.room_index)
            .map_or("", |r| r.name.as_str());
        let a44 = name == "SU_A44";
        if let Some(i) = self.holo.instruction_mut() {
            i.close_to_pos = if a44 {
                Vec2::new(480.0, 330.0)
            } else if p.pos.x > 500.0 {
                Vec2::new(330.0, 250.0)
            } else {
                Vec2::new(850.0, 250.0)
            };
        }
        let edibles = self.cx.edibles.len();
        let held = self.cx.edibles.iter().any(|e| e.held_by_player);
        if let Some(t) = self.ai.tutorial.as_mut() {
            let count = i32::try_from(edibles).unwrap_or(i32::MAX);
            if a44 {
                t.available_fruit_in_fruit_room = count - 1;
            } else if name == "SU_A42" {
                t.bats_in_bat_room = count;
            }
        }
        let free: Vec<Vec2> = self
            .cx
            .edibles
            .iter()
            .filter(|e| e.free && !held)
            .map(|e| e.pos)
            .collect();
        let pos = self.holo.pos;
        for (k, a) in food_arrows.into_iter().enumerate() {
            let part = &mut self.holo.parts[a];
            match free.get(k).copied() {
                Some(at) => {
                    part.visible = true;
                    part.offset = at + Vec2::new(0.0, 20.0) - pos;
                }
                None => part.visible = false,
            }
        }
        let s = self.cx.story;
        self.holo.still_relevant = held
            || self.ai.tutorial.as_ref().is_some_and(|t| {
                t.eat_instruction_relevant(name, p.food, s.food_to_hibernate, s.max_food, edibles)
            });
        if !self.holo.still_relevant {
            log::info!("eat no longer relevant!");
        }
        self.holo.wait_meter_update();
        let mut hide_inputs = true;
        let mut show_pickup = false;
        let mut show_jump = false;
        if p.input_x == 0 && p.input_y == 0 && !p.jump[0] {
            still_counter += 1;
            moving_counter -= 1;
        } else {
            moving_counter += 1;
            still_counter -= 1;
        }
        still_counter = still_counter.clamp(0, 60);
        moving_counter = moving_counter.clamp(0, 60);
        let mut tell_to_stay_still = false;
        let cross = self.holo.instruction().and_then(|i| i.red_cross);
        let mut cross_visible = cross.is_some_and(|c| self.holo.parts[c].visible);
        let mut meter = None;
        if held && self.ai.tutorial.is_some() {
            self.holo.overseer_sit_still = true;
            hide_inputs = false;
            if still_counter > 30 {
                let filled = (p.eat_counter as f32).inverse_lerp(40.0, 10.0);
                meter = Some((true, Some((p.pos - pos, filled))));
                show_pickup = !cross_visible;
                show_slow_button_press = true;
                if still_counter > 50
                    && moving_counter < 10
                    && tell_player_to_stay_still_counter < 10
                {
                    cross_visible = false;
                }
            } else if moving_counter > 30 {
                meter = Some((false, None));
                if moving_counter > 55 {
                    tell_to_stay_still = true;
                    if tell_player_to_stay_still_counter >= 40 {
                        cross_visible = true;
                    }
                }
            }
        } else {
            meter = Some((false, None));
            if a44 && !held {
                show_slow_button_press = false;
                hide_inputs = false;
                show_pickup = true;
                show_jump = true;
                let pick = self.holo.device_button(true).map_or(0, |b| b.counter);
                if let Some(jump) = self.holo.device_button(false)
                    && (pick - jump.counter).abs() < 5
                {
                    jump.counter = 0;
                }
            }
            self.holo.overseer_sit_still = false;
            cross_visible = false;
        }
        tell_player_to_stay_still_counter += if tell_to_stay_still { 1 } else { -1 };
        tell_player_to_stay_still_counter = tell_player_to_stay_still_counter.clamp(0, 40);
        if let Some(c) = cross {
            self.holo.parts[c].visible = cross_visible;
        }
        let Some(i) = self.holo.instruction_mut() else {
            return;
        };
        i.hide_inputs = hide_inputs;
        i.show_pickup = show_pickup;
        i.show_jump = show_jump;
        if let (Some(m), Some((visible, fill))) = (i.wait_meter.as_mut(), meter) {
            m.visible = visible;
            if let Some((offset, filled)) = fill {
                m.offset = offset;
                m.filled = filled;
            }
        }
        i.controller = Some(Controller::Eat {
            food_arrows,
            still_counter,
            moving_counter,
            tell_player_to_stay_still_counter,
            show_slow_button_press,
        });
    }

    /// `TutorialText`, shown only with a HUD.
    fn tutorial_text(&mut self, text: &'static str, wait: i32, time: i32) {
        if !self.cx.hud {
            return;
        }
        self.cx
            .effects
            .push(crate::creature::overseer::OverseerEffect::Message(
                crate::creature::overseer::TutorialMessage {
                    text,
                    wait,
                    time,
                    hide_hud: false,
                },
            ));
    }
}
