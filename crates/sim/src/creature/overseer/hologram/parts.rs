//! `OverseerHologram.HologramPart` and its subclasses: the state their `Update` keeps.
//!
//! Mirrors: OverseerHolograms/OverseerHologram.cs (HologramPart)

use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// `hologram.color` at a part's update: the overseer's colour, or one a flashing
/// pointer swaps in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoloColor {
    /// `Color.Lerp(MainColor, ColorOfSegment(0, 0), 0.5)`, white without a graphics module.
    #[default]
    Base,
    White,
    Red,
    /// The food pointer's (0, 1, 0.6).
    Mint,
}

/// The colour a part's `GetToColor` gave at its last update; the graphics lerp each
/// part's colour half way toward it every tick.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PartColor {
    #[default]
    Hologram,
    /// `hologram.color` resolved at that update.
    Holo(HoloColor),
    /// A fixed colour (RGBA).
    Rgba([f32; 4]),
    /// `MeterBox` unlit: `hologram.color * 0.75` made opaque.
    Dimmed(HoloColor),
}

/// A `Symbol`'s sprite.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPart {
    pub element: &'static str,
    pub anchor_y: f32,
    pub scale_x: f32,
}

impl SymbolPart {
    #[must_use]
    pub fn new(element: &'static str) -> Self {
        Self {
            element,
            anchor_y: 0.5,
            scale_x: 1.0,
        }
    }
}

/// `ButtonOrKey` / `SwitchButton`: a pulsating key or button.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonState {
    pub pulsate: bool,
    pub counter: i32,
    pub down: bool,
    pub time_up: i32,
    pub time_down: i32,
    /// The element after `key`/`button` (`"Arrow"`, `"Shift"`, `"Z"`, ...).
    pub symbol: &'static str,
    pub symbol_rotation: f32,
}

impl ButtonState {
    #[must_use]
    pub fn new(symbol: &'static str) -> Self {
        Self {
            pulsate: false,
            counter: 0,
            down: false,
            time_up: 10,
            time_down: 10,
            symbol,
            symbol_rotation: 0.0,
        }
    }

    /// The press cycle; returns `down` for the transform.
    fn update(&mut self) {
        if self.pulsate {
            self.counter -= 1;
            if self.counter < 1 {
                self.down = !self.down;
                self.counter = if self.down {
                    self.time_down
                } else {
                    self.time_up
                };
            }
        } else {
            self.down = false;
            self.counter = 0;
        }
    }
}

/// `OverseerImage.HoloImage`.
#[derive(Debug, Clone, PartialEq)]
pub struct HoloImage {
    pub last_img: i32,
    pub last_show_img: (i32, i32),
    pub pan_pos: Vec2,
    pub last_pan_pos: Vec2,
    pub pan_vel: Vec2,
    pub pan_center: Vec2,
    pub my_alpha: f32,
    pub last_my_alpha: f32,
    pub random_image: i32,
    pub random_image_time: f32,
    pub random_flicker: f32,
    pub show_random_flicker_image: bool,
    /// The `Overseer_Image_LOOP` emitter exists.
    pub sound: bool,
}

/// The part classes.
#[derive(Debug, Clone, PartialEq)]
pub enum PartKind {
    /// `Arrow`: points along `dir`.
    Arrow {
        dir: Vec2,
    },
    Pentagon,
    Symbol(SymbolPart),
    /// `CyclingSymbol`.
    CyclingSymbol {
        symbol: SymbolPart,
        elements: Vec<&'static str>,
        curr: usize,
        cntr: i32,
        cycle_length: i32,
    },
    /// `HologramLightEffect` (not on the Hologram shader).
    LightEffect {
        light_alpha: f32,
        dark_alpha: f32,
        light_rad: f32,
        dark_rad: f32,
    },
    /// `PlayerGhost`: the slugcat's outline at a room position.
    PlayerGhost {
        symbol: SymbolPart,
        room_position: Option<Vec2>,
    },
    /// `InputInstruction.RedCross`.
    RedCross {
        small: bool,
    },
    /// `Eat.FoodArrow`.
    FoodArrow,
    /// `KeyBoardKey`; `wider` is `MakeWider`'s widening.
    Key {
        button: ButtonState,
        wider: f32,
    },
    /// `GamePadButton` with its preset's colour (RGB).
    GamePadButton {
        button: ButtonState,
        round: bool,
        color: [f32; 3],
    },
    /// `GamePadSilhouette` (`ps` for the PlayStation presets, which draw no outline).
    GamePadSilhouette {
        xbox: bool,
        ps: bool,
        base_pos: Vec2,
    },
    GamePadStickSocket {
        scale: f32,
    },
    /// `GamePadStick`; `socket` is the socket's part index.
    GamePadStick {
        socket: usize,
        counter: i32,
        out_to_side: bool,
        stick_pos: Vec2,
        stick_vel: Vec2,
        switch: bool,
    },
    /// `SwitchInstruction.SwitchButton`.
    SwitchButton(ButtonState),
    /// `WaitMeter.MeterBox` at fraction `f` of the circle.
    MeterBox {
        f: f32,
        lit: bool,
        fill: [f32; 4],
    },
    HoloImage(Box<HoloImage>),
    /// `OverseerImage.Frame`.
    Frame,
}

/// One `HologramPart`.
#[derive(Debug, Clone, PartialEq)]
pub struct HologramPart {
    pub kind: PartKind,
    pub offset: Vec2,
    pub last_offset: Vec2,
    /// `rotation.z` in degrees (x and y stay 0 for every base-game part).
    pub rotation: f32,
    pub last_rotation: f32,
    pub transform: f32,
    pub last_transform: f32,
    pub part_fade: f32,
    pub last_part_fade: f32,
    pub visible: bool,
    pub fade_exponent: f32,
    pub to_color: PartColor,
}

impl HologramPart {
    #[must_use]
    pub fn new(kind: PartKind) -> Self {
        Self {
            kind,
            offset: Vec2::ZERO,
            last_offset: Vec2::ZERO,
            rotation: 0.0,
            last_rotation: 0.0,
            transform: 0.0,
            last_transform: 0.0,
            part_fade: 1.0,
            last_part_fade: 1.0,
            visible: true,
            fade_exponent: 1.0,
            to_color: PartColor::Hologram,
        }
    }

    /// A part with an offset.
    #[must_use]
    pub fn at(kind: PartKind, offset: Vec2) -> Self {
        Self {
            offset,
            ..Self::new(kind)
        }
    }

    /// `HologramPart.Update`'s base: last values, the part fade, a new fade exponent when
    /// the combined fade lands on 0 or 1, and the colour to lerp toward.
    pub(super) fn base_update(
        &mut self,
        fade: f32,
        last_fade: f32,
        color: HoloColor,
        rng: &mut UnityRandom,
    ) {
        self.last_rotation = self.rotation;
        self.last_offset = self.offset;
        self.last_transform = self.transform;
        self.last_part_fade = self.part_fade;
        let target = if self.visible { 1.0 } else { 0.0 };
        self.part_fade = self.part_fade.lerp_and_tick(target, 0.01, 0.05);
        let now = self.part_fade * fade;
        if (now == 0.0 || now == 1.0) && now != self.last_part_fade * last_fade {
            self.fade_exponent = 0.5.lerp(1.5, rng.value());
        }
        self.to_color = self.get_to_color(color);
    }

    /// `GetToColor`.
    fn get_to_color(&self, color: HoloColor) -> PartColor {
        match &self.kind {
            PartKind::RedCross { .. } => PartColor::Rgba([1.0, 0.0, 0.0, 1.0]),
            PartKind::Key { button, .. } | PartKind::SwitchButton(button) if button.down => {
                PartColor::Rgba([1.0; 4])
            }
            PartKind::GamePadButton { button, color, .. } => {
                if button.down {
                    PartColor::Rgba([1.0; 4])
                } else {
                    PartColor::Rgba([color[0], color[1], color[2], 1.0])
                }
            }
            PartKind::GamePadStick {
                out_to_side: true, ..
            } => PartColor::Rgba([1.0; 4]),
            PartKind::MeterBox {
                lit: true, fill, ..
            } => PartColor::Rgba(*fill),
            PartKind::MeterBox { lit: false, .. } => PartColor::Dimmed(color),
            _ => PartColor::Holo(color),
        }
    }

    /// The class-specific part of `Update` that needs nothing but the part (arrow
    /// rotation, key presses, symbol cycling, meter boxes).
    pub(super) fn own_update(&mut self) {
        match &mut self.kind {
            PartKind::Arrow { dir } => {
                self.rotation = Vec2::new(-dir.x, dir.y).to_degrees();
            }
            PartKind::Key { button, .. }
            | PartKind::GamePadButton { button, .. }
            | PartKind::SwitchButton(button) => {
                button.update();
                let to = if button.down { 1.0 } else { 0.0 };
                self.transform = self.transform.lerp_and_tick(to, 0.14, 0.125);
            }
            PartKind::CyclingSymbol {
                symbol,
                elements,
                curr,
                cntr,
                cycle_length,
            } => {
                *cntr += 1;
                if *cntr > *cycle_length {
                    *cntr = 0;
                    *curr += 1;
                    if *curr >= elements.len() {
                        *curr = 0;
                    }
                    symbol.element = elements[*curr];
                }
            }
            PartKind::MeterBox { lit, .. } => {
                let to = if *lit { 1.0 } else { 0.0 };
                self.transform = self.transform.lerp_and_tick(to, 0.05, 0.1);
            }
            _ => {}
        }
    }

    /// The button state of a key or button part.
    pub fn button_mut(&mut self) -> Option<&mut ButtonState> {
        match &mut self.kind {
            PartKind::Key { button, .. }
            | PartKind::GamePadButton { button, .. }
            | PartKind::SwitchButton(button) => Some(button),
            _ => None,
        }
    }

    /// The button state of a key or button part.
    #[must_use]
    pub fn button(&self) -> Option<&ButtonState> {
        match &self.kind {
            PartKind::Key { button, .. }
            | PartKind::GamePadButton { button, .. }
            | PartKind::SwitchButton(button) => Some(button),
            _ => None,
        }
    }
}
