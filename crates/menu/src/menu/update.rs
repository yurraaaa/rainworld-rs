//! `Menu.Update` and the objects' `Update`s: selection, pressing and holding.

use rw_sim::{FloatExt, Vec2};

use super::object::{ButtonVariant, Kind, ObjectId};
use super::{Menu, MenuCx, MenuScreen};
use crate::sound::MenuSoundId;

/// What a button does when its behaviour reports a click.
enum Click {
    Signal(String),
    CheckBox,
}

impl Menu {
    /// `Menu.Update` for one tick; the screen's own update follows it.
    pub fn update(&mut self, screen: &mut dyn MenuScreen, cx: &mut MenuCx<'_>) {
        if !self.init {
            self.init_menu(cx);
            screen.init(self);
            self.init = true;
        }
        self.update_scroll_wheel(cx);
        self.allow_select_move = true;
        self.last_mouse_pos = self.mouse_pos;
        self.mouse_pos = Vec2::from(cx.shared.input.mouse.pos);
        self.last_input = self.input;
        self.input = cx.shared.input.take_tick_input();
        self.last_info_label_fade = self.info_label_fade;
        if self.selected.is_some() {
            self.info_label_fade = 1.0;
        } else {
            self.info_label_fade =
                (self.info_label_fade - 1.0 / 1.0_f32.lerp(100.0, self.info_label_fade)).max(0.0);
        }
        self.info_label_sin += self.info_label_fade;
        self.frozen = self.freeze(screen, cx);
        for i in 0..self.pages.len() {
            let page = self.pages[i];
            self.update_object(page, screen, cx);
        }
        self.last_hold_button = self.hold_button;
        self.last_mouse_down = self.mouse_down;
        self.mouse_down = cx.shared.input.mouse.buttons[0] && cx.requests.upcoming().is_none();
        if self.freeze(screen, cx) {
            if self.mouse_down && !self.last_mouse_down {
                cx.shared.mouse_mode = true;
                self.mode_switch = true;
                self.hold_button = true;
            }
            return;
        }
        if cx.shared.mouse_mode {
            self.update_mouse_mode(cx);
        } else {
            self.update_keyboard_mode(cx);
        }
        if let Some(selected) = self.selected {
            let page = self.pages[self.current_page];
            if self.page_data(page).last_selected != Some(selected) || self.info_label_dirty {
                self.info_label_dirty = false;
                self.info_label_sin = 0.0;
            }
            self.page_data_mut(page).last_selected = Some(selected);
        }
        if self.mode_switch {
            if !self.hold_button {
                self.mode_switch = false;
            }
            self.hold_button = false;
            self.press_button = false;
            self.selected = None;
        } else {
            self.press_button = self.hold_button && !self.last_hold_button;
        }
        if self.press_button
            && let Some(selected) = self.selected
            && let Some(b) = self.button_mut(selected)
            && b.behav.greyed_out
        {
            b.behav.extra_size_bump = 0.0;
            cx.play_sound(MenuSoundId::GreyedOutButtonClicked);
        }
        if self.hold_button != self.last_hold_button {
            self.info_label_dirty = true;
        }
        self.update_sound_loop(cx);
    }

    /// `FreezeMenuFunctions`: while a switch is under way or the screen is more than
    /// half black.
    fn freeze(&self, screen: &dyn MenuScreen, cx: &MenuCx<'_>) -> bool {
        cx.requests.upcoming().is_some() || cx.requests.fade_to_black() > 0.5 || screen.freeze()
    }

    /// `Menu.Init`, on the first update.
    fn init_menu(&mut self, cx: &mut MenuCx<'_>) {
        let shift = self.horizontal_move_to_get_centered();
        for i in 0..self.pages.len() {
            let page = self.pages[i];
            let first = self.page_data(page).selectables.first().copied();
            if first.is_some() {
                self.page_data_mut(page).last_selected = first;
            }
            self.objects[page.0].pos.x -= shift;
        }
        if let Some(scene) = self.scene {
            self.horizontal_displace(scene, -shift);
            if let Some(gradients) = self.gradients_container {
                self.create_button_gradients(gradients);
            }
        }
        let first_page = self.pages[0];
        if self.back_object.is_some() && self.selected.is_none() {
            self.selected = self.back_object;
        } else if !cx.shared.mouse_mode
            && let Some(last) = self.page_data(first_page).last_selected
        {
            self.selected = Some(last);
        }
        self.stage
            .add_child(crate::StageTree::ROOT, self.cursor_container);
    }

    /// `MenuScene.HorizontalDisplace` (flat illustrations only).
    fn horizontal_displace(&mut self, scene: ObjectId, dp: f32) {
        let flats = match &self.objects[scene.0].kind {
            Kind::Scene(s) => s.flat_illustrations.clone(),
            _ => return,
        };
        for f in flats {
            self.objects[f.0].pos.x += dp;
        }
    }

    /// The mouse wheel in mouse mode (`mouseScrollWheelMovement`).
    fn update_scroll_wheel(&mut self, cx: &mut MenuCx<'_>) {
        if cx.shared.mouse_mode {
            self.float_scroll_wheel += std::mem::take(&mut cx.shared.input.mouse.scroll);
            if (self.float_scroll_wheel * 15.0).round() as i32 != self.last_scroll_wheel {
                let now = (self.float_scroll_wheel * 10.0).round() as i32;
                self.mouse_scroll_wheel_movement = self.last_scroll_wheel - now;
                self.last_scroll_wheel = now;
            } else {
                self.mouse_scroll_wheel_movement = 0;
            }
        } else {
            self.float_scroll_wheel = 0.0;
            self.last_scroll_wheel = 0;
            self.mouse_scroll_wheel_movement = 0;
        }
    }

    /// Mouse mode: the selection is the first selectable under the cursor; keyboard
    /// input leaves the mode.
    fn update_mouse_mode(&mut self, cx: &mut MenuCx<'_>) {
        self.selected = None;
        let page = self.pages[self.current_page];
        let selectables = self.page_data(page).selectables.clone();
        for s in selectables {
            if self.currently_selectable_mouse(s) && self.mouse_over(s) {
                self.selected = Some(s);
                break;
            }
        }
        self.hold_button = self.mouse_down;
        let any_mouse_button = cx.shared.input.mouse.buttons.iter().any(|b| *b);
        if (self.input.x != 0 || self.input.y != 0 || self.input.jmp) && !any_mouse_button {
            cx.shared.mouse_mode = false;
            self.mode_switch = true;
            self.hold_button = self.input.jmp;
        }
    }

    /// Keyboard mode: a new direction moves the selection, held it repeats after 20
    /// ticks every 7; cancel jumps to the back object.
    fn update_keyboard_mode(&mut self, cx: &mut MenuCx<'_>) {
        let (input, last) = (self.input, self.last_input);
        self.hold_button = input.jmp;
        if self.mouse_down && !self.last_mouse_down {
            cx.shared.mouse_mode = true;
            self.mode_switch = true;
            self.hold_button = true;
        }
        if input.y != 0 && last.y != input.y {
            self.select_new_object([0, input.y], cx);
        } else if input.x != 0 && last.x != input.x {
            self.select_new_object([input.x, 0], cx);
        }
        let held_y = input.y != 0 && last.y == input.y && input.x == 0;
        let held_x = input.x != 0 && last.x == input.x && input.y == 0;
        if held_y || held_x {
            self.scroll_init_delay += 1;
        } else {
            self.scroll_init_delay = 0;
        }
        if self.scroll_init_delay > 20 {
            self.scroll_delay += 1;
            if self.scroll_delay > 6 {
                self.scroll_delay = 0;
                if input.y != 0 && last.y == input.y {
                    self.select_new_object([0, input.y], cx);
                } else if input.x != 0 && last.x == input.x {
                    self.select_new_object([input.x, 0], cx);
                }
            }
        } else {
            self.scroll_delay = 0;
        }
        if self.allow_select_move
            && self.back_object.is_some()
            && input.thrw
            && !last.thrw
            && !input.jmp
            && self.selected != self.back_object
        {
            self.selected = self.back_object;
        }
    }

    /// `SelectNewObject`: moves and plays the select sound (greyed or not).
    fn select_new_object(&mut self, direction: [i8; 2], cx: &mut MenuCx<'_>) {
        if !self.allow_select_move {
            return;
        }
        let Some(candidate) = self.select_candidate(direction) else {
            return;
        };
        if Some(candidate) == self.selected {
            return;
        }
        self.selected = Some(candidate);
        let greyed = self.behavior(candidate).is_none_or(|b| b.greyed_out);
        cx.play_sound(if greyed {
            MenuSoundId::GreyedOutButtonSelectGamepadOrKeyboard
        } else {
            MenuSoundId::ButtonSelectGamepadOrKeyboard
        });
    }

    /// `SelectCandidate`: the neighbour link for the direction, else the best scored
    /// selectable of the page (distance times angle penalty; candidates behind wrap
    /// round the screen).
    #[must_use]
    pub fn select_candidate(&self, direction: [i8; 2]) -> Option<ObjectId> {
        let page = self.pages[self.current_page];
        let Some(selected) = self.selected else {
            return self.page_data(page).last_selected;
        };
        let links = self.objects[selected.0].next_selectable;
        let [dx, dy] = direction;
        if dx < 0 && links[0].is_some() {
            return links[0];
        }
        if dy > 0 && links[1].is_some() {
            return links[1];
        }
        if dx > 0 && links[2].is_some() {
            return links[2];
        }
        if dy < 0 && links[3].is_some() {
            return links[3];
        }
        if !self.objects[selected.0].kind.positioned() {
            return self.page_data(page).last_selected;
        }
        let mut result = self.page_data(page).last_selected;
        let from = self.center_position(selected);
        let dir = Vec2::new(f32::from(dx), f32::from(dy));
        let dir_n = dir.normalized();
        let mut best = f32::MAX;
        for &candidate in &self.page_data(page).selectables {
            if !self.objects[candidate.0].kind.positioned()
                || !self.currently_selectable_non_mouse(candidate)
                || candidate == selected
            {
                continue;
            }
            let to = self.center_position(candidate);
            let mut dist = from.distance(to);
            let to_dir = from.direction_to(to);
            let mut angle = 0.5 - 0.5 * to_dir.dot(dir_n);
            if angle > 0.5 {
                if angle > 0.8 {
                    angle = 0.5 - 0.5 * (-to_dir).dot(dir_n);
                    let wrap = if dx != 0 { 2400.0 } else { 1800.0 };
                    dist = from.distance(to + dir * wrap);
                } else {
                    dist += 10000.0;
                }
            }
            angle *= 50.0;
            let score = (1.0 + dist) * (1.0 + angle);
            if score < best {
                result = Some(candidate);
                best = score;
            }
        }
        result
    }

    /// `CenterPositionOfObject`: a rectangle's screen centre, else its `pos`.
    fn center_position(&self, id: ObjectId) -> Vec2 {
        let o = &self.objects[id.0];
        if o.kind.rectangular() {
            self.screen_pos(id) + o.size / 2.0
        } else {
            o.pos
        }
    }

    /// `CurrentlySelectableMouse`.
    #[must_use]
    pub fn currently_selectable_mouse(&self, id: ObjectId) -> bool {
        match &self.objects[id.0].kind {
            Kind::Button(b) => {
                let selectable = match b.variant {
                    ButtonVariant::CheckBox { selectable, .. } => selectable,
                    _ => true,
                };
                !b.behav.greyed_out && selectable
            }
            Kind::HoldButton(h) => !h.behav.greyed_out && !h.has_signalled,
            _ => false,
        }
    }

    /// `CurrentlySelectableNonMouse`.
    #[must_use]
    pub fn currently_selectable_non_mouse(&self, id: ObjectId) -> bool {
        match &self.objects[id.0].kind {
            Kind::Button(b) => match b.variant {
                ButtonVariant::CheckBox { selectable, .. } => selectable,
                _ => true,
            },
            Kind::HoldButton(_) => true,
            _ => false,
        }
    }

    /// `IsMouseOverMe`: inside a rectangle (strictly) or a circle.
    #[must_use]
    pub fn mouse_over(&self, id: ObjectId) -> bool {
        let o = &self.objects[id.0];
        let p = self.screen_pos(id);
        let m = self.mouse_pos;
        match o.kind {
            Kind::HoldButton(_) => (p - m).length() < o.rad,
            _ => m.x > p.x && m.y > p.y && m.x < p.x + o.size.x && m.y < p.y + o.size.y,
        }
    }

    /// The background loop: found among the microphone's loops or started, brought to
    /// full volume; other menus' background loops fade out.
    fn update_sound_loop(&mut self, cx: &mut MenuCx<'_>) {
        if let Some(handle) = self.sound_loop {
            if let Some(l) = cx.shared.mic.loop_mut(handle) {
                l.loop_volume = l.loop_volume.lerp_and_tick(1.0, 0.02, 0.05);
            }
        } else if let Some(id) = self.my_sound_loop {
            let found = cx
                .shared
                .mic
                .loops()
                .iter()
                .find(|l| l.is_bkg && l.sound.id == id)
                .map(|l| l.handle);
            self.sound_loop = found.or_else(|| cx.play_loop(id, [0.0, 1.0, 1.0], true));
        }
        if self.all_alien_loops_gone {
            return;
        }
        self.all_alien_loops_gone = true;
        let mine = self.my_sound_loop;
        let handles: Vec<_> = cx
            .shared
            .mic
            .loops()
            .iter()
            .filter(|l| l.is_bkg && Some(l.sound.id) != mine)
            .map(|l| l.handle)
            .collect();
        for handle in handles {
            if let Some(l) = cx.shared.mic.loop_mut(handle) {
                l.loop_volume = (l.loop_volume - 0.025).max(0.0);
                if l.loop_volume <= 0.0 {
                    l.destroyed = true;
                }
            }
            self.all_alien_loops_gone = false;
        }
    }

    /// `MenuObject.Update`: the sub-objects first, then the object's own update.
    pub fn update_object(
        &mut self,
        id: ObjectId,
        screen: &mut dyn MenuScreen,
        cx: &mut MenuCx<'_>,
    ) {
        if let Kind::Scene(s) = &self.objects[id.0].kind {
            if s.hidden {
                return;
            }
            self.update_subs(id, screen, cx);
            self.update_scene(id, cx);
            return;
        }
        self.update_subs(id, screen, cx);
        let mouse_pos = self.mouse_pos;
        let o = &mut self.objects[id.0];
        o.last_pos = o.pos;
        if o.kind.rectangular() {
            o.last_size = o.size;
        }
        if matches!(o.kind, Kind::HoldButton(_)) {
            o.last_rad = o.rad;
        }
        match &mut o.kind {
            Kind::RoundedRect(r) => {
                r.last_fill_alpha = r.fill_alpha;
                r.last_add_size = r.add_size;
            }
            Kind::Illustration(i) => {
                i.last_alpha = i.alpha;
                if let Some(a) = i.set_alpha.take() {
                    i.alpha = a;
                }
            }
            Kind::MouseCursor(c) => {
                o.pos = mouse_pos;
                c.last_fade = c.fade;
            }
            _ => {}
        }
        match self.objects[id.0].kind {
            Kind::Page(_) => screen.page_update(self, id, cx),
            Kind::Button(_) => self.update_button(id, screen, cx),
            Kind::HoldButton(_) => self.update_hold_button(id, screen, cx),
            Kind::MouseCursor(_) => self.update_cursor(id, screen, cx),
            Kind::RainEffect(_) => {
                let random = &mut cx.shared.random;
                if let Kind::RainEffect(r) = &mut self.objects[id.0].kind {
                    r.update(random);
                }
            }
            _ => {}
        }
    }

    /// `MouseCursor.Update`: fades in on the current page in mouse mode when the
    /// cursor is shown (`ShowCursor`: full screen).
    fn update_cursor(&mut self, id: ObjectId, screen: &dyn MenuScreen, cx: &MenuCx<'_>) {
        let page = self.page_of(id);
        let on_page = self.page_data(page).index == self.current_page;
        let show = screen.shows_cursor()
            && cx.shared.full_screen
            && !self.frozen
            && cx.shared.mouse_mode
            && on_page;
        if let Kind::MouseCursor(c) = &mut self.objects[id.0].kind {
            c.fade = c
                .fade
                .lerp_and_tick(if show { 1.0 } else { 0.0 }, 0.01, 1.0 / 30.0);
        }
    }

    fn update_subs(&mut self, id: ObjectId, screen: &mut dyn MenuScreen, cx: &mut MenuCx<'_>) {
        let mut i = 0;
        while let Some(&sub) = self.objects[id.0].sub_objects.get(i) {
            self.update_object(sub, screen, cx);
            i += 1;
        }
    }

    /// `RoundedRect.Update` after construction (its own `Update()` call).
    pub(super) fn update_rounded_rect_bookkeeping(&mut self, id: ObjectId) {
        let o = &mut self.objects[id.0];
        o.last_pos = o.pos;
        o.last_size = o.size;
        if let Kind::RoundedRect(r) = &mut o.kind {
            r.last_fill_alpha = r.fill_alpha;
            r.last_add_size = r.add_size;
        }
    }

    /// `ButtonBehavior.Update`; returns what to do if it clicked.
    fn update_behavior(&mut self, id: ObjectId, cx: &mut MenuCx<'_>) -> Option<()> {
        let selected = self.is_selected(id);
        let (press, hold, mouse_mode) = (self.press_button, self.hold_button, cx.shared.mouse_mode);
        let toggled = self.objects[id.0].toggled;
        let b = self.behavior_mut(id)?;
        b.last_col = b.col;
        b.last_flash = b.flash;
        b.last_sin = b.sin;
        b.flash = b.flash.lerp_and_tick(0.0, 0.03, 1.0 / 6.0);
        let mut press_sound = false;
        let mut clicked = false;
        if selected && (!b.greyed_out || !mouse_mode) {
            b.bump = true;
            b.size_bump = b.size_bump.lerp_and_tick(1.0, 0.1, 0.1);
            b.sin += 1.0;
            if !b.flash_bool {
                b.flash_bool = true;
                b.flash = 1.0;
            }
            if !b.greyed_out {
                if press {
                    if !b.clicked {
                        press_sound = true;
                    }
                    b.clicked = true;
                }
                if !hold {
                    if b.clicked {
                        clicked = true;
                    }
                    b.clicked = false;
                }
                b.col = (b.col + 0.1).min(1.0);
            }
        } else {
            b.clicked = false;
            b.bump = false;
            b.flash_bool = false;
            b.size_bump = b.size_bump.lerp_and_tick(0.0, 0.1, 0.05);
            b.col = (b.col - 1.0 / 30.0).max(0.0);
        }
        if toggled {
            b.size_bump = b.size_bump.lerp_and_tick(1.0, 0.1, 0.1);
            b.sin = 7.5;
            b.bump = true;
            if b.flash < 0.75 {
                b.flash = 0.75;
            }
        }
        b.last_extra_size_bump = b.extra_size_bump;
        if b.bump {
            b.extra_size_bump = (b.extra_size_bump + 0.1).min(1.0);
        } else {
            b.extra_size_bump = 0.0;
        }
        if press_sound {
            cx.play_sound(MenuSoundId::ButtonPressInit);
        }
        clicked.then_some(())
    }

    /// The behaviour's click, in the middle of its update as the game: `Clicked()` runs
    /// before `clicked` is cleared and `col` rises.
    fn behavior_tick(&mut self, id: ObjectId, screen: &mut dyn MenuScreen, cx: &mut MenuCx<'_>) {
        if self.update_behavior(id, cx).is_some() {
            self.clicked(id, screen, cx);
        }
    }

    fn clicked(&mut self, id: ObjectId, screen: &mut dyn MenuScreen, cx: &mut MenuCx<'_>) {
        let click = match &self.objects[id.0].kind {
            Kind::Button(b) => match b.variant {
                ButtonVariant::CheckBox { .. } => Click::CheckBox,
                _ => Click::Signal(b.signal_text.clone()),
            },
            _ => return,
        };
        match click {
            Click::Signal(text) => screen.signal(self, cx, id, &text),
            Click::CheckBox => {
                if self.behavior(id).is_some_and(|b| b.greyed_out) {
                    return;
                }
                let checked = !screen.checked(self, id);
                screen.set_checked(self, id, checked);
                cx.play_sound(if screen.checked(self, id) {
                    MenuSoundId::CheckboxCheck
                } else {
                    MenuSoundId::CheckboxUncheck
                });
                if let Some(b) = self.button_mut(id)
                    && let ButtonVariant::CheckBox {
                        symbol_half_visible,
                        last_symbol_half_visible,
                        ..
                    } = &mut b.variant
                {
                    *symbol_half_visible = 0.0;
                    *last_symbol_half_visible = 0.0;
                }
            }
        }
    }

    /// `ButtonTemplate.Update` and the subclass's: simple and arrow buttons and check
    /// boxes update their behaviour twice per tick, as the game.
    fn update_button(&mut self, id: ObjectId, screen: &mut dyn MenuScreen, cx: &mut MenuCx<'_>) {
        self.behavior_tick(id, screen, cx);
        self.behavior_tick(id, screen, cx);
        let selected = self.is_selected(id);
        let Some(b) = self.button(id) else { return };
        let behav = b.behav.clone();
        let bump = behav.size_bump + 0.5 * (behav.extra_size_bump * std::f32::consts::PI).sin();
        let not_clicked = if behav.clicked { 0.0 } else { 1.0 };
        let fill = 0.3_f32.lerp(0.6, behav.col);
        match b.variant.clone() {
            ButtonVariant::Simple {
                rounded_rect,
                select_rect,
                fade_alpha,
                ..
            } => {
                self.set_rect(
                    rounded_rect,
                    fill * fade_alpha,
                    Vec2::new(10.0, 6.0) * bump * not_clicked,
                );
                self.set_rect_add_size(select_rect, Vec2::new(2.0, -2.0) * bump * not_clicked);
            }
            ButtonVariant::BigArrow {
                rounded_rect,
                select_rect,
                ..
            } => {
                self.set_rect(
                    rounded_rect,
                    fill,
                    Vec2::new(10.0, 6.0) * bump * not_clicked,
                );
                self.set_rect_add_size(select_rect, Vec2::new(2.0, -2.0) * bump * not_clicked);
            }
            ButtonVariant::CheckBox { rounded_rect, .. } => {
                if let Some(b) = self.button_mut(id)
                    && let ButtonVariant::CheckBox {
                        symbol_half_visible,
                        last_symbol_half_visible,
                        ..
                    } = &mut b.variant
                {
                    *last_symbol_half_visible = *symbol_half_visible;
                    *symbol_half_visible = if selected {
                        symbol_half_visible.lerp_and_tick(1.0, 0.07, 1.0 / 60.0)
                    } else {
                        0.0
                    };
                }
                self.set_rect(rounded_rect, fill, Vec2::new(4.0, 4.0) * bump * not_clicked);
            }
        }
    }

    fn set_rect(&mut self, rect: ObjectId, fill_alpha: f32, add_size: Vec2) {
        if let Kind::RoundedRect(r) = &mut self.objects[rect.0].kind {
            r.fill_alpha = fill_alpha;
            r.add_size = add_size;
        }
    }

    fn set_rect_add_size(&mut self, rect: ObjectId, add_size: Vec2) {
        if let Kind::RoundedRect(r) = &mut self.objects[rect.0].kind {
            r.add_size = add_size;
        }
    }

    /// `HoldButton.Update`: its loop sound, the pulse, filling while held and the
    /// signal when full.
    fn update_hold_button(
        &mut self,
        id: ObjectId,
        screen: &mut dyn MenuScreen,
        cx: &mut MenuCx<'_>,
    ) {
        self.behavior_tick(id, screen, cx);
        let selected = self.is_selected(id);
        let hold = self.hold_button;
        let Some(h) = self.hold_button_mut(id) else {
            return;
        };
        h.last_held = h.held;
        if h.held {
            if h.sound_loop.is_none() {
                h.sound_loop =
                    cx.play_loop(MenuSoundId::SecurityButtonLoop, [0.0, 0.0, 1.0], false);
            }
            let (filled, pulse) = (h.filled, h.pulse);
            if let Some(l) = h.sound_loop.and_then(|s| cx.shared.mic.loop_mut(s)) {
                l.loop_volume = l.loop_volume.lerp(1.0, 0.85);
                l.loop_pitch =
                    0.3_f32.lerp(1.5, filled) - 0.15 * (pulse * std::f32::consts::PI * 2.0).sin();
            }
        } else if let Some(handle) = h.sound_loop {
            match cx.shared.mic.loop_mut(handle) {
                Some(l) => {
                    l.loop_volume = (l.loop_volume - 0.125).max(0.0);
                    if l.loop_volume <= 0.0 {
                        l.destroyed = true;
                        h.sound_loop = None;
                    }
                }
                None => h.sound_loop = None,
            }
        }
        let pulsing = if h.controlled_from_outside {
            h.held
        } else {
            h.behav.clicked
        };
        if pulsing {
            h.last_pulse = h.pulse;
            h.pulse += h.filled / 20.0;
        } else {
            h.pulse = 0.0;
            h.last_pulse = 0.0;
        }
        h.last_filled = h.filled;
        if h.controlled_from_outside {
            h.behav.size_bump = if h.held { 1.0 } else { 0.0 };
        } else {
            h.held = selected && !h.behav.greyed_out && !h.has_signalled && hold;
        }
        if h.held {
            h.behav.sin = h.pulse;
            let rate = if h.warning_mode {
                1.0 / (h.fill_time * 4.0)
            } else {
                1.0 / h.fill_time
            };
            h.filled = h.filled.lerp_and_tick(1.0, 0.007, rate);
            let signal = (h.filled >= 1.0 && !h.has_signalled).then(|| h.signal_text.clone());
            h.released_counter = 0;
            let init_sound = !h.last_held;
            if let Some(text) = signal {
                screen.signal(self, cx, id, &text);
                if let Some(h) = self.hold_button_mut(id) {
                    h.has_signalled = true;
                }
                self.reset_selection();
            }
            if init_sound {
                cx.play_sound(MenuSoundId::SecurityButtonInit);
            }
            return;
        }
        if h.last_held && !h.has_signalled {
            cx.play_sound(MenuSoundId::SecurityButtonRelease);
        }
        let Some(h) = self.hold_button_mut(id) else {
            return;
        };
        if h.has_signalled {
            h.released_counter += 1;
            if h.released_counter > 30 {
                h.filled = h.filled.lerp_and_tick(0.0, 0.04, 0.025);
                if h.filled < 0.5 {
                    h.has_signalled = false;
                }
            } else {
                h.filled = 1.0;
            }
        } else {
            h.filled = h.filled.lerp_and_tick(0.0, 0.04, 0.025);
        }
    }
}
