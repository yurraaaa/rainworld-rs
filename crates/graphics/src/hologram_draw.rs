//! `OverseerHologram.InitiateSprites` / `DrawSprites` and the parts' sprite code: every
//! part's own sprites (symbols, keys, lights, the projected image), then its lines,
//! turned in 3D, given depth on the screen and flickering as they fade. All of a
//! hologram's sprites go to the `Bloom` container. Behaviour:
//! `rainworld-re/notes/overseer.md` §7.

use rw_random::UnityRandom;
use rw_sim::creature::overseer::hologram::{
    ButtonState, HoloColor, HoloImage, HologramKind, HologramPart, ImageId, InstructionDevice,
    OverseerHologram, PartColor, PartKind, SymbolPart,
};
use rw_sim::{FloatExt, Vec2, Vec3};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::hologram_shape::{HologramLine, PartShape};
use crate::math::{Custom, CustomVec, Mathf, UnityVector3};

/// One Futile sprite as it stays between frames (Futile keeps whatever was last set).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SpriteState {
    visible: bool,
    element: &'static str,
    /// Screen position (room minus `camPos`).
    pos: Vec2,
    rotation: f32,
    scale: Vec2,
    anchor: Vec2,
    color: Color,
    alpha: f32,
    shader: Shader,
}

/// What a frame of drawing reads besides the hologram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HologramFrame {
    /// `timeStacker`.
    pub t: f32,
    /// `camPos`: the room position of the screen's bottom-left corner.
    pub cam_pos: Vec2,
    /// `rainWorld.screenSize`.
    pub screen_size: Vec2,
    /// The overseer's eye (`DrawPosOfSegment(0, t)`) while it has a graphics module and
    /// is in the room, else its chunk between ticks; the hologram pops out of it.
    pub head: Vec2,
}

/// The arguments `OverseerHologram.DrawSprites` hands a part.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PartDraw {
    pos: Vec2,
    /// `useFade`.
    fade: f32,
    /// `popOut`.
    pop_out: f32,
    color: Color,
    transform: f32,
}

/// A part's lines and colour.
#[derive(Debug, Clone, PartialEq)]
struct PartGraphics {
    shape: PartShape,
    first_sprite: usize,
    /// `color`, lerped toward `GetToColor` every update.
    color: Color,
    last_color: Color,
}

/// One hologram's sprites and its parts' colours.
#[derive(Debug, Clone, PartialEq)]
pub struct HologramGraphics {
    pub id: u64,
    parts: Vec<PartGraphics>,
    sprites: Vec<SpriteState>,
}

impl SpriteState {
    /// `new FSprite(element)`.
    fn new(element: &'static str, shader: Shader) -> Self {
        Self {
            visible: true,
            element,
            pos: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::new(1.0, 1.0),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::WHITE,
            alpha: 1.0,
            shader,
        }
    }

    /// `new FSprite(element)` on the `Hologram` shader.
    fn holo(element: &'static str) -> Self {
        Self::new(element, Shader::Hologram)
    }

    /// A line's `pixel`, anchored at its bottom.
    fn line() -> Self {
        Self {
            anchor: Vec2::new(0.5, 0.0),
            ..Self::new("pixel", Shader::Hologram)
        }
    }

    /// Futile multiplies `alpha` into the colour's.
    fn sprite(&self, index: usize, cam_pos: Vec2) -> Sprite {
        Sprite {
            index,
            element: self.element,
            pos: self.pos + cam_pos,
            rotation: self.rotation,
            scale: self.scale,
            anchor: self.anchor,
            color: self.color.with_alpha(self.color.a * self.alpha),
            shader: self.shader,
            layer: Layer::Bloom,
        }
    }

    /// The symbols' `pixel` stand-in: a line from `from` to `to` (room positions).
    fn pixel_line(&mut self, from: Vec2, to: Vec2) {
        self.element = "pixel";
        self.anchor.y = 0.0;
        self.rotation = from.aim_to(to);
        self.scale.y = from.distance(to);
    }
}

impl HologramFrame {
    /// `ApplyDepthOnVector`: the screen position of `p`, pushed out from (half the
    /// screen width, two thirds of its height) by `-z * 0.0015`.
    #[must_use]
    pub fn apply_depth(&self, p: Vec3) -> Vec2 {
        let (cx, cy) = (self.screen_size.x / 2.0, self.screen_size.y * (2.0 / 3.0));
        let mut x = p.x - self.cam_pos.x;
        let mut y = p.y - self.cam_pos.y;
        x -= cx;
        x *= 1.0 - p.z * 0.0015;
        x += cx;
        y -= cy;
        y *= 1.0 - p.z * 0.0015;
        y += cy;
        Vec2::new(x, y)
    }
}

impl HologramGraphics {
    /// `"key"` + symbol + `"A"`/`"B"` (keyboard keys).
    const KEY_ELEMENTS: [(&'static str, &'static str, &'static str); 4] = [
        ("Arrow", "keyArrowA", "keyArrowB"),
        ("Shift", "keyShiftA", "keyShiftB"),
        ("Z", "keyZA", "keyZB"),
        ("X", "keyXA", "keyXB"),
    ];

    /// `"button"` + symbol + `"A"`/`"B"` (game pad buttons).
    const BUTTON_ELEMENTS: [(&'static str, &'static str, &'static str); 6] = [
        ("X", "buttonXA", "buttonXB"),
        ("A", "buttonAA", "buttonAB"),
        ("B", "buttonBA", "buttonBB"),
        ("Square", "buttonSquareA", "buttonSquareB"),
        ("Cross", "buttonCrossA", "buttonCrossB"),
        ("Circle", "buttonCircleA", "buttonCircleB"),
    ];

    /// `InitiateSprites`: every part's sprites, the lines' `pixel`s after its own, all
    /// on the `Hologram` shader but the light effect's.
    #[must_use]
    pub fn new(h: &OverseerHologram) -> Self {
        let mut parts = Vec::with_capacity(h.parts.len());
        let mut sprites = Vec::new();
        for part in &h.parts {
            let shape = PartShape::of(part, &h.parts);
            let first_sprite = sprites.len();
            sprites.extend(Self::own_sprites(part, &shape));
            sprites.extend(shape.lines.iter().map(|_| SpriteState::line()));
            parts.push(PartGraphics {
                shape,
                first_sprite,
                color: Color::default(),
                last_color: Color::default(),
            });
        }
        Self {
            id: h.id,
            parts,
            sprites,
        }
    }

    /// A part's own sprites as its `InitiateSprites` makes them.
    fn own_sprites(part: &HologramPart, shape: &PartShape) -> Vec<SpriteState> {
        match &part.kind {
            PartKind::Symbol(s)
            | PartKind::CyclingSymbol { symbol: s, .. }
            | PartKind::PlayerGhost { symbol: s, .. } => vec![SpriteState::holo(s.element)],
            PartKind::LightEffect { dark_alpha, .. } => (0..shape.own_sprites)
                .map(|i| {
                    if i == 0 && *dark_alpha > 0.0 {
                        SpriteState {
                            color: Color::BLACK,
                            ..SpriteState::new("Futile_White", Shader::FlatLight)
                        }
                    } else {
                        SpriteState::new("Futile_White", Shader::LightSource)
                    }
                })
                .collect(),
            PartKind::Key { button, .. } => vec![Self::button_sprite(true, button)],
            PartKind::GamePadButton { button, .. } => vec![Self::button_sprite(false, button)],
            PartKind::GamePadSilhouette { .. } => vec![SpriteState::holo("Circle4"); 4],
            PartKind::SwitchButton(_) => vec![SpriteState::holo("LizardBubble0")],
            PartKind::HoloImage(_) => vec![
                SpriteState::holo("RND_PROJ"),
                SpriteState::holo("Futile_White"),
            ],
            _ => Vec::new(),
        }
    }

    /// A key's or button's symbol sprite, turned by its `symbolRotation`.
    fn button_sprite(key: bool, button: &ButtonState) -> SpriteState {
        SpriteState {
            rotation: button.symbol_rotation,
            ..SpriteState::new(
                Self::button_element(key, button.symbol, false).unwrap_or("pixel"),
                Shader::Hologram,
            )
        }
    }

    /// `(key ? "key" : "button") + symbol + (pressed ? "B" : "A")`.
    fn button_element(key: bool, symbol: &str, pressed: bool) -> Option<&'static str> {
        let table: &[(&str, &'static str, &'static str)] = if key {
            &Self::KEY_ELEMENTS
        } else {
            &Self::BUTTON_ELEMENTS
        };
        table
            .iter()
            .find(|(s, ..)| *s == symbol)
            .map(|&(_, up, down)| if pressed { down } else { up })
    }

    /// The colour a part's `GetToColor` gave, `base` being `hologram.color`'s overseer
    /// colour.
    #[must_use]
    pub fn to_color(color: PartColor, base: Color) -> Color {
        match color {
            PartColor::Hologram => base,
            PartColor::Holo(c) => Self::holo_color(c, base),
            PartColor::Rgba([r, g, b, a]) => Color { r, g, b, a },
            PartColor::Dimmed(c) => Self::holo_color(c, base).scale(0.75).with_alpha(1.0),
        }
    }

    /// `hologram.color`: the overseer's `base`, or the colour a flashing pointer swaps in.
    #[must_use]
    pub fn holo_color(color: HoloColor, base: Color) -> Color {
        match color {
            HoloColor::Base => base,
            HoloColor::White => Color::WHITE,
            HoloColor::Red => Color::rgb(1.0, 0.0, 0.0),
            HoloColor::Mint => Color::rgb(0.0, 1.0, 0.6),
        }
    }

    /// The end of every part's `Update`: its colour goes half way to the colour its
    /// update chose. `base` is `hologram.color` now: `Color.Lerp(MainColor,
    /// ColorOfSegment(0, 0), 0.5)` with a graphics module, white without.
    pub fn update(&mut self, h: &OverseerHologram, base: Color) {
        if self.parts.len() != h.parts.len() {
            *self = Self::new(h);
        }
        for (g, part) in self.parts.iter_mut().zip(&h.parts) {
            g.last_color = g.color;
            g.color = g.color.lerp(Self::to_color(part.to_color, base), 0.5);
        }
    }

    /// A part's colour (`color`, as of the last update).
    #[must_use]
    pub fn part_color(&self, part: usize) -> Option<Color> {
        self.parts.get(part).map(|p| p.color)
    }

    /// `DrawSprites` at `f.t` between ticks: each part's own sprites, then its lines.
    /// `rng` is a cosmetic stream (the game draws from the global one every frame).
    pub fn draw(
        &mut self,
        h: &OverseerHologram,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) -> DrawList {
        if self.parts.len() != h.parts.len() {
            *self = Self::new(h);
        }
        let fade = h.last_fade.lerp(h.fade, f.t);
        let pop_out = fade.max(0.0).mathf_pow(0.2);
        let pos = h.last_pos.lerp(h.pos, f.t);
        for (i, part) in h.parts.iter().enumerate() {
            let g = &self.parts[i];
            let d = PartDraw {
                pos: pos + part.last_offset.lerp(part.offset, f.t),
                fade: (part.last_part_fade.lerp(part.part_fade, f.t) * fade)
                    .max(0.0)
                    .mathf_pow(part.fade_exponent),
                pop_out,
                color: g.last_color.lerp(g.color, f.t),
                transform: part.last_transform.lerp(part.transform, f.t),
            };
            self.draw_own(i, h, &d, f, rng);
            self.draw_lines(i, part, &d, f, rng);
        }
        let items = self
            .sprites
            .iter()
            .enumerate()
            .filter(|(_, s)| s.visible)
            .map(|(k, s)| DrawItem::Sprite(s.sprite(k, f.cam_pos)))
            .collect();
        DrawList { items }
    }

    /// The part's own `DrawSprites`.
    fn draw_own(
        &mut self,
        i: usize,
        h: &OverseerHologram,
        d: &PartDraw,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) {
        let part = &h.parts[i];
        let first = self.parts[i].first_sprite;
        match &part.kind {
            PartKind::Symbol(s)
            | PartKind::CyclingSymbol { symbol: s, .. }
            | PartKind::PlayerGhost { symbol: s, .. } => self.draw_symbol(first, s, d, f, rng),
            PartKind::LightEffect {
                light_alpha,
                dark_alpha,
                light_rad,
                dark_rad,
            } => {
                let own = self.parts[i].shape.own_sprites;
                let dark = (*dark_alpha > 0.0).then_some(0);
                let light = (*light_alpha > 0.0).then_some(usize::from(dark.is_some()));
                let mut pos = d.pos;
                for k in 0..own {
                    if rng.value() > FloatExt::inverse_lerp(d.fade, 0.0, 0.5) {
                        // The game breaks out: later sprites keep their last state.
                        self.sprites[first + k].visible = false;
                        break;
                    }
                    let s = &mut self.sprites[first + k];
                    s.visible = true;
                    pos = f.head.lerp(pos, d.pop_out);
                    s.pos = pos - f.cam_pos;
                    let grow = 0.5 + 0.5 * d.fade;
                    if Some(k) == light {
                        s.color = d.color;
                        s.alpha = light_alpha * d.fade.mathf_pow(3.0);
                        s.scale = Vec2::new(1.0, 1.0) * (light_rad / 8.0 * grow);
                    } else if Some(k) == dark {
                        s.alpha = dark_alpha * d.fade.mathf_pow(3.0);
                        s.scale = Vec2::new(1.0, 1.0) * (dark_rad / 8.0 * grow);
                    }
                }
            }
            PartKind::Key { button, .. } => self.draw_button(first, true, button, part, d, f, rng),
            PartKind::GamePadButton { button, .. } => {
                self.draw_button(first, false, button, part, d, f, rng);
            }
            PartKind::GamePadSilhouette { base_pos, .. } => {
                self.draw_silhouette(first, h, *base_pos, d, f, rng);
            }
            PartKind::SwitchButton(_) => self.draw_switch_button(first, d, f, rng),
            PartKind::HoloImage(img) => {
                let image = Self::current_image(h);
                self.draw_image(first, img, image, part.part_fade, d, f);
            }
            _ => {}
        }
    }

    /// `Symbol.DrawSprites`: the element (narrowed as it fades), or at random a line
    /// back to the overseer's eye; hidden at random below half fade.
    fn draw_symbol(
        &mut self,
        first: usize,
        sym: &SymbolPart,
        d: &PartDraw,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) {
        let s = &mut self.sprites[first];
        let shown = FloatExt::inverse_lerp(d.fade, 0.5, 1.0);
        if rng.value() > shown {
            s.visible = false;
            return;
        }
        s.visible = true;
        let pos = f.head.lerp(d.pos, d.pop_out);
        s.pos = pos - f.cam_pos;
        s.color = d.color;
        if rng.value() > d.fade {
            s.pixel_line(pos, f.head);
            s.scale.x = 1.0;
        } else {
            s.element = sym.element;
            s.rotation = 0.0;
            s.scale = Vec2::new(sym.scale_x * shown, 1.0);
            s.anchor.y = sym.anchor_y;
        }
    }

    /// `ButtonOrKey.DrawSprites`: the key's symbol at the depth of its press, `B` while
    /// pressed.
    #[allow(clippy::too_many_arguments)]
    fn draw_button(
        &mut self,
        first: usize,
        key: bool,
        button: &ButtonState,
        part: &HologramPart,
        d: &PartDraw,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) {
        let s = &mut self.sprites[first];
        if rng.value() > FloatExt::inverse_lerp(d.fade, 0.5, 1.0) {
            s.visible = false;
            return;
        }
        s.visible = true;
        let p = Vec3::new(d.pos.x, d.pos.y, -5.0 + 10.0 * d.transform);
        let p = Vec3::from_xy(f.head).lerp_to(p, d.pop_out);
        s.pos = f.apply_depth(p);
        let pressed = button.down || part.transform > 0.5;
        if let Some(element) = Self::button_element(key, button.symbol, pressed) {
            s.element = element;
        }
        s.color = d.color;
    }

    /// `SwitchButton.DrawSprites`: a bubble that shows while pressed.
    fn draw_switch_button(
        &mut self,
        first: usize,
        d: &PartDraw,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) {
        let s = &mut self.sprites[first];
        if rng.value() > FloatExt::inverse_lerp(d.fade, 0.5, 1.0) || d.transform < 0.05 {
            s.visible = false;
            s.scale = Vec2::ZERO;
            return;
        }
        s.visible = true;
        s.scale = Vec2::new(0.85, 0.85);
        let p = Vec3::new(d.pos.x, d.pos.y, -2.0 + 4.0 * d.transform);
        let p = Vec3::from_xy(f.head).lerp_to(p, d.pop_out);
        s.alpha = d.transform;
        s.pos = f.apply_depth(p);
        s.color = d.color;
    }

    /// `GamePadSilhouette.DrawSprites`: four dots (the pulsating buttons' in their
    /// colours), each at random a line back to the eye.
    fn draw_silhouette(
        &mut self,
        first: usize,
        h: &OverseerHologram,
        base_pos: Vec2,
        d: &PartDraw,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) {
        if rng.value() > FloatExt::inverse_lerp(d.fade, 0.5, 1.0) {
            for s in &mut self.sprites[first..first + 4] {
                s.visible = false;
            }
            return;
        }
        let buttons = match h.instruction().map(|i| &i.device) {
            Some(InstructionDevice::GamePad { buttons, .. }) => Some(*buttons),
            _ => None,
        };
        for j in 0..4 {
            let pulsing = buttons
                .and_then(|b| b.get(j).copied())
                .filter(|&b| h.parts[b].button().is_some_and(|s| s.pulsate));
            let color = pulsing.and_then(|b| self.part_color(b)).unwrap_or(d.color);
            let s = &mut self.sprites[first + j];
            s.visible = true;
            let v = d.pos + base_pos + Vec2::from_degrees(-90.0 - 90.0 * j as f32) * 4.0;
            s.pos = v - f.cam_pos;
            s.color = color;
            if rng.value() > d.fade {
                s.pixel_line(d.pos, f.head);
            } else {
                s.element = "Circle4";
                s.rotation = 0.0;
                s.scale.y = 1.0;
                s.anchor.y = 0.5;
            }
        }
    }

    /// The picture an image hologram shows (`imageOwner.CurrImage`).
    fn current_image(h: &OverseerHologram) -> ImageId {
        match &h.kind {
            HologramKind::Image(img) => img
                .images
                .get(img.curr_image)
                .copied()
                .unwrap_or(ImageId::Undefined),
            HologramKind::Angry { counter, .. } if counter % 60 >= 30 => ImageId::DeadSlugcatB,
            _ => ImageId::DeadSlugcatA,
        }
    }

    /// `HoloImage.DrawSprites`: the projection (the colour carries the pan and the
    /// picture to the `HologramImage` shader) and a flat light under it.
    fn draw_image(
        &mut self,
        first: usize,
        img: &HoloImage,
        image: ImageId,
        part_fade: f32,
        d: &PartDraw,
        f: &HologramFrame,
    ) {
        if d.fade == 0.0 {
            self.sprites[first].visible = false;
            self.sprites[first + 1].visible = false;
            return;
        }
        let pos = f.head.lerp(d.pos, d.pop_out) - f.cam_pos;
        let size = 0.1 * 0.5.lerp(1.0, d.fade);
        let num = if img.show_random_flicker_image {
            img.random_image
        } else {
            image.index()
        };
        let pan = img.last_pan_pos.lerp(img.pan_pos, f.t);
        let alpha =
            (d.fade.mathf_pow(2.0) * img.last_my_alpha.lerp(img.my_alpha, f.t)).s_curve(0.4);
        let s = &mut self.sprites[first];
        s.visible = true;
        s.pos = pos;
        s.shader = Shader::HologramImage;
        s.element = if img.show_random_flicker_image {
            "RND_PROJ"
        } else {
            "STR_PROJ"
        };
        s.rotation = 0.0;
        s.scale = Vec2::new(size, size);
        if num >= 0 {
            s.color = Color::rgb(0.5 + 0.5 * pan.x, 0.5 + 0.5 * pan.y, num as f32 / 25.0);
        }
        s.alpha = alpha;
        let glow = alpha.min(part_fade).lerp(alpha, 0.5);
        let s = &mut self.sprites[first + 1];
        s.visible = true;
        s.shader = Shader::FlatLight;
        s.pos = pos;
        let scale = 15.0 * 0.5.lerp(1.0, glow);
        s.scale = Vec2::new(scale, scale);
        s.alpha = glow * 0.5;
        s.color = d.color;
    }

    /// The base `DrawSprites`' lines: each shown by chance (more as it fades in), turned,
    /// popped out of the eye, its ends snapping toward the eye at random while fading,
    /// then given depth.
    fn draw_lines(
        &mut self,
        i: usize,
        part: &HologramPart,
        d: &PartDraw,
        f: &HologramFrame,
        rng: &mut UnityRandom,
    ) {
        let g = &self.parts[i];
        let first = g.first_sprite + g.shape.own_sprites;
        let rz = part.last_rotation.lerp_angle(part.rotation, f.t) / 360.0;
        // `rotation.x` and `.y` stay 0 for every base-game part.
        let (rx, ry) = (0.0, 0.0);
        let zero = Vec3::from_xy(f.head);
        let at = Vec3::from_xy(d.pos);
        for (j, line) in g.shape.lines.iter().enumerate() {
            let s = &mut self.sprites[first + j];
            if rng.value() >= d.fade.mathf_pow(2.0) {
                s.visible = false;
                continue;
            }
            s.visible = true;
            let (mut a, mut b) = Self::line_ends(line, d.transform, rx, ry, rz, at);
            a = zero.lerp_to(a, d.pop_out);
            b = zero.lerp_to(b, d.pop_out);
            if rng.value() > d.fade {
                a = Self::flicker(a, zero, d.fade, rng);
                b = Self::flicker(b, zero, d.fade, rng);
            }
            let (a, b) = (f.apply_depth(a), f.apply_depth(b));
            s.pos = a;
            s.rotation = a.aim_to(b);
            s.scale.y = a.distance(b);
            s.color = d.color;
            s.alpha = d.fade.mathf_pow(0.5);
        }
    }

    /// A line's ends at transform `t`, turned and moved to `at`.
    fn line_ends(line: &HologramLine, t: f32, rx: f32, ry: f32, rz: f32, at: Vec3) -> (Vec3, Vec3) {
        (
            line.a.lerp_to(line.a2, t).rotate_3d(rx, ry, rz) + at,
            line.b.lerp_to(line.b2, t).rotate_3d(rx, ry, rz) + at,
        )
    }

    /// A fading line's end: at the eye, part way to it, or kept.
    fn flicker(p: Vec3, zero: Vec3, fade: f32, rng: &mut UnityRandom) -> Vec3 {
        if rng.value() > fade.mathf_pow(0.05) {
            zero
        } else if rng.value() > fade {
            p.lerp_to(zero, rng.value())
        } else {
            p
        }
    }
}
