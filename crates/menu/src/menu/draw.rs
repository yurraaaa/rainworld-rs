//! `GrafUpdate`: the objects place and colour their sprites for a frame.
//!
//! Mirrors: Menu/MenuObject.cs, Menu/Menu.cs (GrafUpdate)

use std::f32::consts::PI;

use rw_sim::{FloatExt, Vec2};

use super::object::{ButtonVariant, Kind, ObjectId, RoundedRectData, SimpleKind};
use super::{Menu, MenuScreen};
use crate::color::{HslColor, MenuColor};
use crate::stage::StageColor;

impl Menu {
    /// `Menu.GrafUpdate`: the info label, then every page. `mouse` is the frame's
    /// `Futile.mousePosition`, read live rather than kept from the last tick.
    pub fn graf_update(&mut self, screen: &dyn MenuScreen, t: f32, mouse: Vec2) {
        if let Some(node) = self.info_label {
            let alpha = self.last_info_label_fade.lerp(self.info_label_fade, t);
            let color = MenuColor::White.rgb().lerp(
                MenuColor::MediumGrey.rgb(),
                0.5 + 0.5 * ((self.info_label_sin + t) / 9.0).sin(),
            );
            let label = self.stage.label_mut(node);
            label.transform.alpha = alpha;
            label.color = color;
        }
        for i in 0..self.pages.len() {
            let page = self.pages[i];
            self.graf_object(page, screen, t, mouse);
        }
    }

    /// `MenuObject.GrafUpdate`: the sub-objects first, then the object's own.
    pub fn graf_object(&mut self, id: ObjectId, screen: &dyn MenuScreen, t: f32, mouse: Vec2) {
        if let Kind::Scene(s) = &self.objects[id.0].kind
            && s.hidden
        {
            return;
        }
        if let Kind::MouseCursor(_) = self.objects[id.0].kind {
            self.graf_cursor(id, t, mouse);
        }
        let mut i = 0;
        while let Some(&sub) = self.objects[id.0].sub_objects.get(i) {
            self.graf_object(sub, screen, t, mouse);
            i += 1;
        }
        match &self.objects[id.0].kind {
            Kind::Page(_) => screen.page_graf_update(self, id, t),
            Kind::Label(l) => {
                let node = l.label;
                let pos = self.draw_pos(id, t) + self.objects[id.0].size / 2.0;
                let tr = self.stage.transform_mut(node);
                tr.x = pos.x;
                tr.y = pos.y;
            }
            Kind::Button(_) => self.graf_button(id, screen, t),
            Kind::HoldButton(_) => self.graf_hold_button(id, t),
            Kind::Slider(_) => self.graf_slider(id, t),
            Kind::RoundedRect(_) => self.graf_rounded_rect(id, t),
            Kind::Illustration(i) => {
                let node = i.sprite;
                let alpha = i.last_alpha.lerp(i.alpha, t);
                let depth = i.depth.is_some();
                let pos = self.draw_pos(id, t);
                let tr = self.stage.transform_mut(node);
                tr.x = pos.x;
                tr.y = pos.y;
                tr.alpha = alpha;
                if depth {
                    self.graf_depth_illustration(id, t);
                }
            }
            Kind::DarkGradient(g) => {
                let (node, alpha) = (g.sprite, g.alpha);
                let pos = self.draw_pos(id, t) + self.objects[id.0].size / 2.0;
                let tr = self.stage.transform_mut(node);
                tr.x = pos.x;
                tr.y = pos.y;
                tr.alpha = alpha;
            }
            Kind::Container { .. } => {
                let pos = self.objects[id.0].pos;
                if let Some(node) = self.objects[id.0].my_container {
                    let tr = self.stage.transform_mut(node);
                    tr.x = pos.x;
                    tr.y = pos.y;
                }
            }
            Kind::RainEffect(r) => r.graf_update(&mut self.stage, t),
            // `MenuScene.GrafUpdate`: the globals every scene shader reads.
            Kind::Scene(s) => self.scene_globals = Some(s.shader_globals(t)),
            Kind::MouseCursor(_) => {}
        }
    }

    /// `MouseCursor.GrafUpdate`: at the frame's mouse position `m` (not interpolated
    /// between ticks), faded by an S-curve.
    fn graf_cursor(&mut self, id: ObjectId, t: f32, m: Vec2) {
        let Kind::MouseCursor(c) = &self.objects[id.0].kind else {
            return;
        };
        let (cursor, shadow) = (c.cursor, c.shadow);
        let fade = Self::s_curve(c.last_fade.lerp(c.fade, t), 0.6);
        let tr = self.stage.transform_mut(cursor);
        tr.x = m.x + 0.01;
        tr.y = m.y + 0.01;
        tr.alpha = fade;
        let tr = self.stage.transform_mut(shadow);
        tr.x = m.x + 3.01;
        tr.y = m.y - 8.01;
        tr.alpha = fade.powi(3) * 0.3;
    }

    /// `Custom.SCurve`.
    #[must_use]
    pub fn s_curve(x: f32, k: f32) -> f32 {
        let x = x * 2.0 - 1.0;
        if x < 0.0 {
            let x = (1.0 + x).abs();
            return k * x / (k - x + 1.0) * 0.5;
        }
        let k = -1.0 - k;
        0.5 + k * x / (k - x + 1.0) * 0.5
    }

    /// `ButtonTemplate.InterpColor`.
    fn interp_color(&self, id: ObjectId, t: f32, base: HslColor) -> StageColor {
        let inactive = self.objects[id.0].inactive;
        let Some(b) = self.button(id) else {
            return base.rgb();
        };
        if b.behav.greyed_out || inactive {
            return MenuColor::DarkGrey.hsl().blacken(b.black).rgb();
        }
        let a = b
            .behav
            .last_col
            .lerp(b.behav.col, t)
            .max(b.behav.last_flash.lerp(b.behav.flash, t));
        base.lerp(MenuColor::White.hsl(), a).blacken(b.black).rgb()
    }

    /// `ButtonTemplate.MyColor` (and `CheckBox`'s override).
    fn button_color(&self, id: ObjectId, t: f32) -> StageColor {
        let Some(b) = self.button(id) else {
            return StageColor::WHITE;
        };
        if let ButtonVariant::CheckBox { bright_color, .. } = b.variant {
            if b.behav.greyed_out {
                return MenuColor::VeryDarkGrey.hsl().blacken(b.black).rgb();
            }
            let a = b
                .behav
                .last_col
                .lerp(b.behav.col, t)
                .max(b.behav.last_flash.lerp(b.behav.flash, t));
            let lit = if bright_color {
                MenuColor::MediumGrey.rgb().lerp(MenuColor::White.rgb(), a)
            } else {
                MenuColor::DarkGrey
                    .rgb()
                    .lerp(MenuColor::MediumGrey.rgb(), a)
            };
            return lit.lerp(MenuColor::Black.rgb(), b.black);
        }
        if let ButtonVariant::Simple {
            sub:
                SimpleKind::SelectOne {
                    selected_col,
                    last_selected_col,
                    ..
                },
            ..
        } = b.variant
        {
            return MenuColor::DarkGrey.rgb().lerp(
                self.template_color(id, t),
                last_selected_col.lerp(selected_col, t),
            );
        }
        self.template_color(id, t)
    }

    /// `Slider.MyColor`.
    fn slider_color(&self, id: ObjectId, t: f32) -> StageColor {
        let Some(s) = self.slider(id) else {
            return StageColor::WHITE;
        };
        let b = &s.behav;
        if b.greyed_out {
            return MenuColor::DarkGrey.hsl().blacken(s.black).rgb();
        }
        let a = b
            .last_col
            .lerp(b.col, t)
            .max(b.last_flash.lerp(b.flash, t))
            .max(s.graphic_in_dragged_mode);
        MenuColor::MediumGrey
            .hsl()
            .lerp(MenuColor::White.hsl(), a)
            .blacken(s.black)
            .rgb()
    }

    /// `Slider.GrafUpdate` and `HorizontalSlider.GrafUpdate`: the knob's colours, the
    /// label right of the line, and the line on both sides of the knob.
    fn graf_slider(&mut self, id: ObjectId, t: f32) {
        let color = self.slider_color(id, t);
        let Some(s) = self.slider(id) else { return };
        let s = s.clone();
        let b = &s.behav;
        let owner_pos = self.objects[id.0]
            .owner
            .filter(|o| self.objects[o.0].kind.positioned())
            .map_or(Vec2::ZERO, |o| self.screen_pos(o));
        let anchor = s.anchor_point + owner_pos;
        if let Kind::Label(l) = &self.objects[s.menu_label.0].kind {
            let node = l.label;
            let y = self.draw_pos(id, t).y + 16.0;
            let label = self.stage.label_mut(node);
            label.color = color;
            label.transform.x = anchor.x + s.length + 40.01;
            label.transform.y = y;
        }
        let fill = MenuColor::Black.rgb().lerp(
            MenuColor::White.rgb(),
            b.last_flash.lerp(b.flash, t).max(Self::inverse_lerp(
                0.0,
                0.2,
                s.graphic_in_dragged_mode,
            )),
        );
        if let Kind::RoundedRect(r) = &self.objects[s.rounded_rect.0].kind {
            let sprites = r.sprites.clone();
            for (i, node) in sprites.into_iter().enumerate() {
                self.stage.sprite_mut(node).color = if i < 9 { fill } else { color };
            }
        }
        let alpha = 0.5 + 0.5 * (b.last_sin.lerp(b.sin, t) / 30.0 * PI * 2.0).sin();
        self.set_border(s.select_rect, color, alpha);
        let (rect_add, rect_pos, rect_size) = match &self.objects[s.rounded_rect.0].kind {
            Kind::RoundedRect(r) => (
                r.last_add_size.lerp(r.add_size, t),
                self.draw_pos(s.rounded_rect, t),
                self.draw_size(s.rounded_rect, t),
            ),
            _ => return,
        };
        let v = rect_pos - rect_add / 2.0;
        let size = rect_size + rect_add;
        let (vx, sx) = (v.x.floor() + 0.01, size.x.round() + 0.01);
        let end = anchor.x + s.length + 20.0;
        for node in s.line_sprites {
            self.stage.sprite_mut(node).color = color;
        }
        let y = anchor.y + 15.0;
        let [l0, l1, l2, l3] = s.line_sprites;
        for (node, x, visible) in [
            (l0, anchor.x, vx > anchor.x),
            (l1, anchor.x, vx > anchor.x),
            (l2, end, vx + sx < end),
            (l3, end, vx + sx < end),
        ] {
            let sprite = self.stage.sprite_mut(node);
            sprite.transform.x = x;
            sprite.transform.y = y;
            sprite.transform.visible = visible;
        }
        self.stage.transform_mut(l1).scale_x = vx - anchor.x;
        self.stage.transform_mut(l2).scale_x = s.length + 20.0 - (vx + sx - anchor.x);
    }

    /// The base `ButtonTemplate.MyColor`.
    fn template_color(&self, id: ObjectId, t: f32) -> StageColor {
        let base = self
            .button(id)
            .and_then(|b| b.rect_color)
            .unwrap_or_else(|| MenuColor::MediumGrey.hsl());
        self.interp_color(id, t, base)
    }

    /// The flash colour of a button's filled rectangle.
    fn flash_color(&self, id: ObjectId, t: f32) -> StageColor {
        let flash = self
            .behavior(id)
            .map_or(0.0, |b| b.last_flash.lerp(b.flash, t));
        MenuColor::Black.rgb().lerp(MenuColor::White.rgb(), flash)
    }

    /// `0.5 + 0.5 sin(sin / 30 * 2 pi)` times the size bump.
    fn select_pulse(&self, id: ObjectId, t: f32, sign: f32) -> f32 {
        self.behavior(id).map_or(0.0, |b| {
            let s = b.last_sin.lerp(b.sin, t);
            (0.5 + sign * 0.5 * (s / 30.0 * PI * 2.0).sin()) * b.size_bump
        })
    }

    fn set_fill_color(&mut self, rect: ObjectId, color: StageColor) {
        if let Kind::RoundedRect(r) = &self.objects[rect.0].kind {
            for &node in &r.sprites[..9.min(r.sprites.len())] {
                self.stage.sprite_mut(node).color = color;
            }
        }
    }

    fn set_border(&mut self, rect: ObjectId, color: StageColor, alpha: f32) {
        if let Kind::RoundedRect(r) = &self.objects[rect.0].kind {
            for &node in &r.sprites[..8.min(r.sprites.len())] {
                let sprite = self.stage.sprite_mut(node);
                sprite.color = color;
                sprite.transform.alpha = alpha;
            }
        }
    }

    fn graf_button(&mut self, id: ObjectId, screen: &dyn MenuScreen, t: f32) {
        let Some(b) = self.button(id) else { return };
        let variant = b.variant.clone();
        let greyed = b.behav.greyed_out;
        let flash = self.flash_color(id, t);
        match variant {
            ButtonVariant::Simple {
                rounded_rect,
                select_rect,
                menu_label,
                label_color,
                fade_alpha,
                sub,
            } => {
                let label_rgb = self.interp_color(id, t, label_color);
                if let Kind::Label(l) = &self.objects[menu_label.0].kind {
                    let node = l.label;
                    self.stage.label_mut(node).color = label_rgb;
                }
                self.set_fill_color(rounded_rect, flash);
                let pulse = self.select_pulse(id, t, 1.0);
                let color = self.button_color(id, t);
                self.set_border(select_rect, color, pulse * fade_alpha);
                if let SimpleKind::Controls { device_image } = sub
                    && let Kind::Illustration(i) = &self.objects[device_image.0].kind
                {
                    let node = i.sprite;
                    self.stage.sprite_mut(node).color = color;
                    if let Kind::Label(l) = &self.objects[menu_label.0].kind {
                        let node = l.label;
                        self.stage.label_mut(node).color = color;
                    }
                }
            }
            ButtonVariant::BigArrow {
                rounded_rect,
                select_rect,
                symbol,
            } => {
                self.set_fill_color(rounded_rect, flash);
                let pulse = self.select_pulse(id, t, 1.0);
                let color = self.button_color(id, t);
                self.set_border(select_rect, color, pulse);
                let pos = self.draw_pos(id, t) + self.draw_size(id, t) / 2.0;
                let sprite = self.stage.sprite_mut(symbol);
                sprite.color = color;
                sprite.transform.x = pos.x;
                sprite.transform.y = pos.y;
            }
            ButtonVariant::CheckBox {
                rounded_rect,
                symbol,
                label,
                symbol_half_visible,
                last_symbol_half_visible,
                ..
            } => {
                let pulse = self.select_pulse(id, t, -1.0);
                let color = self.button_color(id, t);
                if let Kind::Label(l) = &self.objects[label.0].kind {
                    let node = l.label;
                    self.stage.label_mut(node).color = color;
                }
                self.set_fill_color(rounded_rect, flash);
                let symbol_color = if greyed {
                    MenuColor::VeryDarkGrey.rgb()
                } else {
                    self.template_color(id, t)
                        .lerp(MenuColor::VeryDarkGrey.rgb(), pulse)
                };
                let pos = self.draw_pos(id, t) + self.draw_size(id, t) / 2.0;
                let alpha = if screen.checked(self, id) {
                    1.0
                } else {
                    last_symbol_half_visible.lerp(symbol_half_visible, t) * 0.25
                };
                let sprite = self.stage.sprite_mut(symbol);
                sprite.color = symbol_color;
                sprite.transform.x = pos.x;
                sprite.transform.y = pos.y;
                sprite.transform.alpha = alpha;
            }
        }
    }

    /// `HoldButton.MyColor`.
    fn hold_color(&self, id: ObjectId, t: f32) -> StageColor {
        let Some(h) = self.hold_button(id) else {
            return StageColor::WHITE;
        };
        if h.behav.greyed_out {
            return MenuColor::DarkGrey.rgb();
        }
        if h.warning_mode && h.filled > 0.0 {
            let w = 0.5 + (h.filled * 50.0 + PI / 2.0).sin() * 0.5;
            return StageColor::rgb(1.0, w, w);
        }
        let lrp = h.behav.last_col.lerp(h.behav.col, t);
        MenuColor::MediumGrey
            .hsl()
            .lerp(MenuColor::White.hsl(), lrp)
            .rgb()
    }

    /// `HoldButton.GrafUpdate`: the five circles around the button's position.
    fn graf_hold_button(&mut self, id: ObjectId, t: f32) {
        let color = self.hold_color(id, t);
        let pos = self.draw_pos(id, t);
        let o = &self.objects[id.0];
        let rad = o.last_rad.lerp(o.rad, t);
        let Kind::HoldButton(h) = &o.kind else { return };
        let b = &h.behav;
        let pulsing = if h.controlled_from_outside {
            h.held
        } else {
            b.clicked
        };
        let pulse_factor = if pulsing {
            0.5 + 0.5 * (h.last_pulse.lerp(h.pulse, t) * PI * 2.0).sin()
        } else {
            1.0
        };
        let mut num =
            rad + 8.0 * (b.size_bump + 0.5 * (b.extra_size_bump * PI).sin()) * pulse_factor;
        num += 0.5;
        let filled = h.last_filled.lerp(h.filled, t);
        let mut select =
            (0.5 + 0.5 * (b.last_sin.lerp(b.sin, t) / 30.0 * PI * 2.0).sin()) * b.size_bump;
        if b.greyed_out {
            select = 0.0;
        }
        let (col, size_bump, circles, label) = (b.col, b.size_bump, h.circles, h.menu_label);
        if let Kind::Label(l) = &self.objects[label.0].kind {
            let node = l.label;
            self.stage.label_mut(node).color = color;
        }
        for node in circles {
            let tr = self.stage.transform_mut(node);
            tr.x = pos.x;
            tr.y = pos.y;
            tr.set_scale(num / 8.0);
        }
        let s = self.stage.sprite_mut(circles[0]);
        s.color = StageColor::rgb(1.0 / 51.0, 0.0, 0.3_f32.lerp(0.6, col));
        let s = self.stage.sprite_mut(circles[1]);
        s.color = color;
        s.transform.alpha = 2.0 / num;
        let s = self.stage.sprite_mut(circles[2]);
        s.transform.set_scale((num + 10.0) / 8.0);
        s.transform.alpha = filled;
        let s = self.stage.sprite_mut(circles[3]);
        s.color = color.lerp(MenuColor::DarkGrey.rgb(), 0.5);
        s.transform.set_scale((num + 15.0) / 8.0);
        s.transform.alpha = 2.0 / (num + 15.0);
        let s = self.stage.sprite_mut(circles[4]);
        s.transform.set_scale((num - 8.0 * size_bump) / 8.0);
        s.transform.alpha = 2.0 / (num - 8.0 * size_bump);
        s.color = StageColor::rgb(0.0, 0.0, select);
    }

    /// `RoundedRect.GrafUpdate`: sides and corners on whole pixels (+0.41), the border
    /// in the owning button's colour.
    pub(super) fn graf_rounded_rect(&mut self, id: ObjectId, t: f32) {
        let border = match self.objects[id.0].owner {
            Some(owner) if self.button(owner).is_some() => {
                let border = match &self.objects[id.0].kind {
                    Kind::RoundedRect(r) => r.border_color,
                    _ => None,
                };
                match border {
                    Some(c) => self.interp_color(owner, t, c),
                    None => self.button_color(owner, t),
                }
            }
            _ => match &self.objects[id.0].kind {
                Kind::RoundedRect(r) => r.border_color.map_or(StageColor::WHITE, HslColor::rgb),
                _ => StageColor::WHITE,
            },
        };
        let draw = self.draw_pos(id, t);
        let size = self.draw_size(id, t);
        let Kind::RoundedRect(r) = &self.objects[id.0].kind else {
            return;
        };
        let add = r.last_add_size.lerp(r.add_size, t);
        let mut v = draw - add / 2.0;
        let s = size + add;
        v = Vec2::new(v.x.floor() + 0.41, v.y.floor() + 0.41);
        let fill_alpha = r.last_fill_alpha.lerp(r.fill_alpha, t);
        let r: RoundedRectData = r.clone();
        self.stage
            .set_xy(r.sprites[r.side(0)], v.x + 1.0, v.y + 7.0);
        self.stage.transform_mut(r.sprites[r.side(0)]).scale_y = s.y - 14.0;
        self.stage
            .set_xy(r.sprites[r.side(1)], v.x + 7.0, v.y + s.y - 1.0);
        self.stage.transform_mut(r.sprites[r.side(1)]).scale_x = s.x - 14.0;
        self.stage
            .set_xy(r.sprites[r.side(2)], v.x + s.x - 1.0, v.y + 7.0);
        self.stage.transform_mut(r.sprites[r.side(2)]).scale_y = s.y - 14.0;
        self.stage
            .set_xy(r.sprites[r.side(3)], v.x + 7.0, v.y + 1.0);
        self.stage.transform_mut(r.sprites[r.side(3)]).scale_x = s.x - 14.0;
        let corners = [
            (v.x + 3.5, v.y + 3.5),
            (v.x + 3.5, v.y + s.y - 3.5),
            (v.x + s.x - 3.5, v.y + s.y - 3.5),
            (v.x + s.x - 3.5, v.y + 3.5),
        ];
        for (i, (x, y)) in corners.into_iter().enumerate() {
            self.stage.set_xy(r.sprites[r.corner(i)], x, y);
        }
        for i in 0..4 {
            self.stage.sprite_mut(r.sprites[r.side(i)]).color = border;
            self.stage.sprite_mut(r.sprites[r.corner(i)]).color = border;
        }
        if r.filled {
            self.stage.set_xy(r.sprites[0], v.x + 4.0, v.y + 7.0);
            self.stage.transform_mut(r.sprites[0]).scale_y = s.y - 14.0;
            self.stage.set_xy(r.sprites[1], v.x + 7.0, v.y + s.y - 4.0);
            self.stage.transform_mut(r.sprites[1]).scale_x = s.x - 14.0;
            self.stage.set_xy(r.sprites[2], v.x + s.x - 4.0, v.y + 7.0);
            self.stage.transform_mut(r.sprites[2]).scale_y = s.y - 14.0;
            self.stage.set_xy(r.sprites[3], v.x + 7.0, v.y + 4.0);
            self.stage.transform_mut(r.sprites[3]).scale_x = s.x - 14.0;
            for (i, (x, y)) in corners.into_iter().enumerate() {
                self.stage.set_xy(r.sprites[4 + i], x, y);
            }
            self.stage
                .set_xy(r.sprites[RoundedRectData::MAIN_FILL], v.x + 7.0, v.y + 7.0);
            let tr = self
                .stage
                .transform_mut(r.sprites[RoundedRectData::MAIN_FILL]);
            tr.scale_x = s.x - 14.0;
            tr.scale_y = s.y - 14.0;
            for &node in &r.sprites[..9] {
                self.stage.transform_mut(node).alpha = fill_alpha;
            }
        }
    }
}
