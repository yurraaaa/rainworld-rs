//! The menu objects' constructors: each makes its sprites in the game's order (they
//! are drawn in the order added to their container), and registers selectables with
//! their page. The caller adds the object to its owner's `subObjects`, as the game.

use rw_render::StageShader;
use rw_sim::Vec2;

use super::Menu;
use super::object::{
    ButtonBehavior, ButtonData, ButtonVariant, CursorData, DarkGradientData, HoldButtonData,
    IllustrationData, Kind, LabelData, MenuObject, ObjectId, PageData, RoundedRectData, SceneData,
    SceneId, SimpleKind, SliderData, SliderId,
};
use crate::color::{HslColor, MenuColor};
use crate::font::Font;
use crate::label::{Alignment, Label};
use crate::shared::MenuShared;
use crate::stage::{Sprite, StageColor};
use crate::tree::NodeId;

impl Menu {
    pub(super) fn push(&mut self, object: MenuObject) -> ObjectId {
        self.objects.push(object);
        ObjectId(self.objects.len() - 1)
    }

    /// `new Page(menu, null, name, index)` with its mouse cursor; the page is added to
    /// `pages`.
    pub fn add_page(&mut self, shared: &MenuShared, name: &str, index: usize) -> ObjectId {
        let page = self.push(MenuObject::new(
            None,
            Vec2::new(1.0 / 3.0, 1.0 / 3.0),
            Kind::Page(PageData {
                name: name.to_owned(),
                index,
                selectables: Vec::new(),
                last_selected: None,
                mouse_cursor: None,
            }),
        ));
        let cursor = self.add_mouse_cursor(shared, page);
        self.add_sub(page, cursor);
        self.page_data_mut(page).mouse_cursor = Some(cursor);
        self.pages.push(page);
        page
    }

    /// `new MouseCursor(menu, page, (-100, -100))`: its sprites go in the cursor
    /// container.
    fn add_mouse_cursor(&mut self, _shared: &MenuShared, page: ObjectId) -> ObjectId {
        let container = self.cursor_container;
        let mut shadow = Sprite::new(crate::AtlasManager::WHITE);
        shadow.shader = StageShader::FlatLight;
        shadow.color = StageColor::BLACK;
        shadow.transform.set_scale(4.0);
        let shadow = self.stage.add_sprite(container, shadow);
        let mut cursor = Sprite::new("Cursor");
        cursor.anchor = [0.0, 1.0];
        let cursor = self.stage.add_sprite(container, cursor);
        let id = self.push(MenuObject::new(
            Some(page),
            Vec2::new(-100.0, -100.0),
            Kind::MouseCursor(CursorData {
                cursor,
                shadow,
                fade: 0.0,
                last_fade: 0.0,
            }),
        ));
        self.objects[id.0].my_container = Some(container);
        id
    }

    /// `new MenuLabel(menu, owner, text, pos, size, bigText)`: centred, parked off
    /// screen until drawn.
    pub fn add_label(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        text: &str,
        pos: Vec2,
        size: Vec2,
        big_text: bool,
    ) -> ObjectId {
        let font = if big_text {
            Font::DISPLAY_FONT
        } else {
            Font::FONT
        };
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        self.objects[id.0].size = size;
        let container = self.container_of(id);
        let font = shared
            .atlases
            .font(font)
            .cloned()
            .expect("MenuShared::load loads the fonts");
        let mut label = Label::new(font, text);
        label.set_alignment(Alignment::Center);
        label.transform.y = -10000.0;
        let node = self.stage.add_label(container, label);
        self.objects[id.0].kind = Kind::Label(LabelData { label: node });
        id
    }

    /// `new RoundedRect(menu, owner, pos, size, filled)`, which updates and draws itself
    /// once at once.
    pub fn add_rounded_rect(
        &mut self,
        owner: ObjectId,
        pos: Vec2,
        size: Vec2,
        filled: bool,
    ) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        self.objects[id.0].size = size;
        let container = self.container_of(id);
        let count = if filled { 17 } else { 8 };
        let mut sprites: Vec<Sprite> = (0..count).map(|_| Sprite::new("pixel")).collect();
        let mut data = RoundedRectData {
            add_size: Vec2::ZERO,
            last_add_size: Vec2::ZERO,
            fill_alpha: 0.0,
            last_fill_alpha: 0.0,
            sprites: Vec::new(),
            filled,
            border_color: None,
        };
        for i in 0..4 {
            sprites[data.side(i)].transform.set_scale(2.0);
            sprites[data.corner(i)] = Sprite::new("UIroundedCorner");
            if filled {
                sprites[i].transform.set_scale(6.0);
                sprites[4 + i] = Sprite::new("UIroundedCornerInside");
            }
        }
        sprites[data.side(0)].anchor[1] = 0.0;
        sprites[data.side(2)].anchor[1] = 0.0;
        sprites[data.side(1)].anchor[0] = 0.0;
        sprites[data.side(3)].anchor[0] = 0.0;
        sprites[data.corner(0)].transform.scale_y = -1.0;
        sprites[data.corner(2)].transform.scale_x = -1.0;
        sprites[data.corner(3)].transform.scale_y = -1.0;
        sprites[data.corner(3)].transform.scale_x = -1.0;
        if filled {
            sprites[RoundedRectData::MAIN_FILL].anchor = [0.0, 0.0];
            sprites[0].anchor[1] = 0.0;
            sprites[2].anchor[1] = 0.0;
            sprites[1].anchor[0] = 0.0;
            sprites[3].anchor[0] = 0.0;
            sprites[4].transform.scale_y = -1.0;
            sprites[6].transform.scale_x = -1.0;
            sprites[7].transform.scale_y = -1.0;
            sprites[7].transform.scale_x = -1.0;
            for s in &mut sprites[..9] {
                s.color = StageColor::BLACK;
                s.transform.alpha = 0.75;
            }
        }
        data.sprites = sprites
            .into_iter()
            .map(|s| self.stage.add_sprite(container, s))
            .collect();
        self.objects[id.0].kind = Kind::RoundedRect(data);
        self.update_rounded_rect_bookkeeping(id);
        self.graf_rounded_rect(id, 0.0);
        id
    }

    /// The `ButtonTemplate` base: the behaviour and the page's selectable.
    fn add_button_template(
        &mut self,
        owner: ObjectId,
        pos: Vec2,
        size: Vec2,
        signal_text: &str,
    ) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        self.objects[id.0].size = size;
        let page = self.page_of(id);
        self.page_data_mut(page).selectables.push(id);
        self.objects[id.0].kind = Kind::Button(ButtonData {
            behav: ButtonBehavior::default(),
            black: 0.0,
            rect_color: None,
            signal_text: signal_text.to_owned(),
            variant: ButtonVariant::Simple {
                rounded_rect: id,
                select_rect: id,
                menu_label: id,
                label_color: MenuColor::MediumGrey.hsl(),
                fade_alpha: 1.0,
                sub: SimpleKind::Plain,
            },
        });
        id
    }

    fn set_variant(&mut self, id: ObjectId, variant: ButtonVariant) {
        if let Some(b) = self.button_mut(id) {
            b.variant = variant;
        }
    }

    /// `new SimpleButton(menu, owner, displayText, signalText, pos, size)`.
    pub fn add_simple_button(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        display_text: &str,
        signal_text: &str,
        pos: Vec2,
        size: Vec2,
    ) -> ObjectId {
        let id = self.add_button_template(owner, pos, size, signal_text);
        let rounded_rect = self.add_rounded_rect(id, Vec2::ZERO, size, true);
        self.add_sub(id, rounded_rect);
        let select_rect = self.add_rounded_rect(id, Vec2::ZERO, size, false);
        self.add_sub(id, select_rect);
        let menu_label = self.add_label(shared, id, display_text, Vec2::ZERO, size, false);
        self.add_sub(id, menu_label);
        self.set_variant(
            id,
            ButtonVariant::Simple {
                rounded_rect,
                select_rect,
                menu_label,
                label_color: MenuColor::MediumGrey.hsl(),
                fade_alpha: 1.0,
                sub: SimpleKind::Plain,
            },
        );
        id
    }

    fn set_simple_kind(&mut self, id: ObjectId, kind: SimpleKind) {
        if let Some(ButtonData {
            variant: ButtonVariant::Simple { sub, .. },
            ..
        }) = self.button_mut(id)
        {
            *sub = kind;
        }
    }

    /// `new CustomMessageButton(menu, owner, displayText, signalText, pos, size, message)`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_custom_message_button(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        display_text: &str,
        signal_text: &str,
        pos: Vec2,
        size: Vec2,
        message: &str,
    ) -> ObjectId {
        let id = self.add_simple_button(shared, owner, display_text, signal_text, pos, size);
        self.set_simple_kind(id, SimpleKind::CustomMessage(message.to_owned()));
        id
    }

    /// `new SelectOneButton(menu, owner, displayText, signalText, pos, size, buttonArray,
    /// buttonArrayIndex)`: the series is the signal text.
    #[allow(clippy::too_many_arguments)]
    pub fn add_select_one_button(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        display_text: &str,
        series: &str,
        pos: Vec2,
        size: Vec2,
        index: i32,
    ) -> ObjectId {
        let id = self.add_simple_button(shared, owner, display_text, series, pos, size);
        let outer_rect = self.add_rounded_rect(id, Vec2::ZERO, size, false);
        self.add_sub(id, outer_rect);
        self.set_simple_kind(
            id,
            SimpleKind::SelectOne {
                index,
                outer_rect,
                selected_col: 0.0,
                last_selected_col: 0.0,
            },
        );
        id
    }

    /// `new ControlsButton(menu, owner, pos, displayName)`: a 110 x 80 simple button
    /// signalling `INPUT` that makes a second pair of rectangles (the first stay as
    /// made), the gamepad icon in the middle and its label 50 below.
    pub fn add_controls_button(
        &mut self,
        shared: &mut MenuShared,
        owner: ObjectId,
        pos: Vec2,
        display_name: &str,
    ) -> ObjectId {
        let size = Vec2::new(110.0, 80.0);
        let id = self.add_simple_button(shared, owner, display_name, "INPUT", pos, size);
        let rounded = self.add_rounded_rect(id, Vec2::ZERO, size, true);
        self.add_sub(id, rounded);
        let select = self.add_rounded_rect(id, Vec2::ZERO, size, false);
        self.add_sub(id, select);
        let image = self.add_illustration(shared, id, "", "GamepadIcon", size / 2.0, true, true);
        self.add_sub(id, image);
        if let Some(ButtonData {
            variant:
                ButtonVariant::Simple {
                    rounded_rect,
                    select_rect,
                    menu_label,
                    sub,
                    ..
                },
            ..
        }) = self.button_mut(id)
        {
            *rounded_rect = rounded;
            *select_rect = select;
            *sub = SimpleKind::Controls {
                device_image: image,
            };
            let label = *menu_label;
            self.objects[label.0].pos.y -= 50.0;
            self.objects[label.0].last_pos.y -= 50.0;
        }
        id
    }

    /// `new HorizontalSlider(menu, owner, text, pos, size, ID, subtleSlider: false)`.
    pub fn add_horizontal_slider(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        text: &str,
        pos: Vec2,
        size: Vec2,
        slider: SliderId,
    ) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        self.objects[id.0].size = size;
        let container = self.container_of(id);
        let mut lines: [Sprite; 4] = std::array::from_fn(|_| Sprite::new("pixel"));
        lines[0].transform.scale_x = 2.0;
        lines[0].transform.scale_y = 6.0;
        lines[1].transform.scale_y = 2.0;
        lines[1].anchor[0] = 0.0;
        lines[2].transform.scale_y = 2.0;
        lines[2].anchor[0] = 1.0;
        lines[3].transform.scale_x = 2.0;
        lines[3].transform.scale_y = 6.0;
        let line_sprites = lines.map(|l| self.stage.add_sprite(container, l));
        let knob = Vec2::new(20.0, 30.0);
        let corner = Vec2::new(0.01, 0.01);
        let rounded_rect = self.add_rounded_rect(id, corner, knob, true);
        self.add_sub(id, rounded_rect);
        let select_rect = self.add_rounded_rect(id, corner, knob, false);
        self.add_sub(id, select_rect);
        let menu_label =
            self.add_label(shared, id, text, Vec2::new(size.x + 10.0, 0.0), size, false);
        if let Kind::Label(l) = &self.objects[menu_label.0].kind {
            let node = l.label;
            self.stage.label_mut(node).set_alignment(Alignment::Left);
        }
        self.add_sub(id, menu_label);
        let page = self.page_of(id);
        self.page_data_mut(page).selectables.push(id);
        self.objects[id.0].kind = Kind::Slider(Box::new(SliderData {
            behav: ButtonBehavior::default(),
            id: slider,
            rounded_rect,
            select_rect,
            menu_label,
            line_sprites,
            anchor_point: pos + corner,
            length: size.x,
            mov_speed: 0.0,
            graphic_in_dragged_mode: 0.0,
            mouse_dragged: false,
            mouse_drag_offset: 0.0,
            black: 0.0,
        }));
        id
    }

    /// `new BigArrowButton(menu, owner, signalText, pos, direction)`.
    pub fn add_big_arrow_button(
        &mut self,
        owner: ObjectId,
        signal_text: &str,
        pos: Vec2,
        direction: i32,
    ) -> ObjectId {
        let size = Vec2::new(50.0, 50.0);
        let id = self.add_button_template(owner, pos, size, signal_text);
        let rounded_rect = self.add_rounded_rect(id, Vec2::ZERO, size, true);
        self.add_sub(id, rounded_rect);
        let select_rect = self.add_rounded_rect(id, Vec2::ZERO, size, false);
        self.add_sub(id, select_rect);
        let mut symbol = Sprite::new("Big_Menu_Arrow");
        symbol.transform.rotation = direction as f32 * 90.0;
        let container = self.container_of(id);
        let symbol = self.stage.add_sprite(container, symbol);
        self.set_variant(
            id,
            ButtonVariant::BigArrow {
                rounded_rect,
                select_rect,
                symbol,
            },
        );
        id
    }

    /// `new CheckBox(menu, owner, reportTo, pos, textWidth, displayText, IDString,
    /// textOnRight)`; the screen answers `Checked`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_check_box(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        pos: Vec2,
        text_width: f32,
        display_text: &str,
        id_string: &str,
        text_on_right: bool,
    ) -> ObjectId {
        let size = Vec2::new(24.0, 24.0);
        let id = self.add_button_template(owner, pos, size, id_string);
        let rounded_rect = self.add_rounded_rect(id, Vec2::ZERO, size, true);
        self.add_sub(id, rounded_rect);
        let label_x = if text_on_right {
            9.0
        } else {
            -text_width * 1.5
        };
        let label = self.add_label(
            shared,
            id,
            display_text,
            Vec2::new(label_x, 3.0),
            Vec2::new(text_width, 20.0),
            false,
        );
        if let Kind::Label(l) = &self.objects[label.0].kind {
            let node = l.label;
            let label = self.stage.label_mut(node);
            label.set_alignment(Alignment::Left);
            label.set_anchor([0.0, label.anchor()[1]]);
        }
        self.add_sub(id, label);
        let container = self.container_of(id);
        let symbol = self
            .stage
            .add_sprite(container, Sprite::new("Menu_Symbol_CheckBox"));
        self.set_variant(
            id,
            ButtonVariant::CheckBox {
                rounded_rect,
                symbol,
                label,
                symbol_half_visible: 0.0,
                last_symbol_half_visible: 0.0,
                selectable: true,
                bright_color: true,
            },
        );
        id
    }

    /// `HoldButton`'s constructor rule for its label: a line break replaces each space
    /// that comes more than nine characters after the last break.
    #[must_use]
    pub fn hold_button_text(display_text: &str) -> String {
        let mut out = String::new();
        let mut run = 0;
        for c in display_text.encode_utf16() {
            run += 1;
            if c == u16::from(b' ') && run > 9 {
                run = 0;
                out.push_str("\r\n");
            } else {
                out.push_str(&String::from_utf16_lossy(&[c]));
            }
        }
        out
    }

    /// `new HoldButton(menu, owner, displayText, signalText, pos, fillTime)`: radius 55.
    #[allow(clippy::too_many_arguments)]
    pub fn add_hold_button(
        &mut self,
        shared: &MenuShared,
        owner: ObjectId,
        display_text: &str,
        signal_text: &str,
        pos: Vec2,
        fill_time: f32,
    ) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        self.objects[id.0].rad = 55.0;
        let page = self.page_of(id);
        self.page_data_mut(page).selectables.push(id);
        let container = self.container_of(id);
        let shaders = [
            StageShader::VectorCircleFadable,
            StageShader::VectorCircle,
            StageShader::HoldButtonCircle,
            StageShader::VectorCircle,
            StageShader::VectorCircleFadable,
        ];
        let circles = shaders.map(|shader| {
            let mut sprite = Sprite::new(crate::AtlasManager::WHITE);
            sprite.shader = shader;
            self.stage.add_sprite(container, sprite)
        });
        let text = Self::hold_button_text(display_text);
        let menu_label = self.add_label(
            shared,
            id,
            &text,
            Vec2::new(-50.0, -15.0),
            Vec2::new(100.0, 30.0),
            false,
        );
        self.add_sub(id, menu_label);
        self.objects[id.0].kind = Kind::HoldButton(HoldButtonData {
            behav: ButtonBehavior::default(),
            menu_label,
            circles,
            filled: 0.0,
            last_filled: 0.0,
            fill_time,
            has_signalled: false,
            released_counter: 0,
            pulse: 0.0,
            last_pulse: 0.0,
            signal_text: signal_text.to_owned(),
            controlled_from_outside: false,
            held: false,
            last_held: false,
            sound_loop: None,
            warning_mode: false,
        });
        id
    }

    /// `new MenuContainer` / `new GradientsContainer(menu, owner, pos, alpha)`: its own
    /// container, added to the owner's.
    pub fn add_menu_container(
        &mut self,
        owner: ObjectId,
        pos: Vec2,
        gradients_alpha: Option<f32>,
    ) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container { gradients_alpha },
        ));
        let parent = self.container_of(owner);
        let container = self.stage.add_container(parent);
        self.objects[id.0].my_container = Some(container);
        id
    }

    /// `new DarkGradient(menu, owner, centerPos, horizontalRad, verticalRad, alpha)`.
    pub fn add_dark_gradient(
        &mut self,
        owner: ObjectId,
        center: Vec2,
        horizontal_rad: f32,
        vertical_rad: f32,
        alpha: f32,
    ) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            center - Vec2::new(horizontal_rad, vertical_rad),
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        self.objects[id.0].size = Vec2::new(horizontal_rad * 2.0, vertical_rad * 2.0);
        let mut sprite = Sprite::new(crate::AtlasManager::WHITE);
        sprite.transform.scale_x = horizontal_rad / 8.0;
        sprite.transform.scale_y = vertical_rad / 8.0;
        sprite.shader = StageShader::FlatLight;
        sprite.color = StageColor::BLACK;
        sprite.transform.alpha = alpha;
        let container = self.container_of(id);
        let sprite = self.stage.add_sprite(container, sprite);
        self.objects[id.0].kind = Kind::DarkGradient(DarkGradientData { sprite, alpha });
        id
    }

    /// `GradientsContainer.CreateButtonGradients`: a gradient behind each simple button
    /// and hold button among the container's owner's objects.
    pub fn create_button_gradients(&mut self, gradients: ObjectId) {
        let Kind::Container {
            gradients_alpha: Some(alpha),
        } = self.objects[gradients.0].kind
        else {
            return;
        };
        let Some(owner) = self.objects[gradients.0].owner else {
            return;
        };
        let subs = self.objects[owner.0].sub_objects.clone();
        for sub in subs {
            let o = &self.objects[sub.0];
            let gradient = match &o.kind {
                Kind::Button(ButtonData {
                    variant: ButtonVariant::Simple { .. },
                    ..
                }) => Some((o.pos + o.size / 2.0, 105.0, 50.0, alpha)),
                Kind::HoldButton(_) => Some((o.pos, 200.0, 200.0, alpha.powf(0.65))),
                _ => None,
            };
            if let Some((center, hr, vr, a)) = gradient {
                let g = self.add_dark_gradient(gradients, center, hr, vr, a);
                self.add_sub(gradients, g);
            }
        }
    }

    /// `new MenuIllustration(menu, owner, folderName, fileName, pos, crispPixels,
    /// anchorCenter)`: loads `<folder>/<file>.png` (`Illustrations` when the folder is
    /// empty) unless an atlas of that name is loaded.
    #[allow(clippy::too_many_arguments)]
    pub fn add_illustration(
        &mut self,
        shared: &mut MenuShared,
        owner: ObjectId,
        folder: &str,
        file: &str,
        pos: Vec2,
        crisp_pixels: bool,
        anchor_center: bool,
    ) -> ObjectId {
        let folder = if folder.is_empty() {
            "Illustrations"
        } else {
            folder
        };
        let size = shared
            .atlases
            .load_illustration(&shared.assets, folder, file, crisp_pixels)
            .inspect_err(|e| log::warn!("illustration {folder}/{file}: {e}"))
            .unwrap_or([1.0, 1.0]);
        let id = self.push(MenuObject::new(
            Some(owner),
            pos,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        let mut sprite = Sprite::new(file);
        if !anchor_center {
            sprite.anchor = [0.0, 0.0];
        }
        sprite.transform.alpha = 0.0;
        let container = self.container_of(id);
        let sprite = self.stage.add_sprite(container, sprite);
        self.objects[id.0].size = Vec2::from(size);
        self.objects[id.0].kind = Kind::Illustration(IllustrationData {
            sprite,
            file_name: file.to_owned(),
            alpha: 1.0,
            last_alpha: 1.0,
            set_alpha: None,
            depth: None,
        });
        id
    }

    /// `new InteractiveMenuScene(menu, owner, sceneID)`: the scene's illustrations
    /// (depth layers unless in flat mode or the port has no depth table for it yet,
    /// D13), then the idle camera's first random draws.
    pub fn add_scene(
        &mut self,
        shared: &mut MenuShared,
        owner: ObjectId,
        scene: SceneId,
    ) -> ObjectId {
        self.add_page_scene(shared, owner, scene, false).0
    }

    /// `add_scene` for a slugcat page, whose `HasGlow` decides whether the scene's
    /// `AddGlow` call adds the glow sprites (returned: B, then A).
    pub fn add_page_scene(
        &mut self,
        shared: &mut MenuShared,
        owner: ObjectId,
        scene: SceneId,
        glow: bool,
    ) -> (ObjectId, Option<[NodeId; 2]>) {
        let id = self.push(MenuObject::new(
            Some(owner),
            Vec2::ZERO,
            Kind::Scene(Box::new(SceneData::new(scene))),
        ));
        let depth = if shared.flat_illustrations {
            None
        } else {
            scene.depth_scene(shared)
        };
        let mut glow_sprites = None;
        if let Some(table) = depth {
            glow_sprites = self.build_depth_scene(shared, id, &table, true, glow);
        } else {
            self.build_flat_scene(shared, id, scene);
        }
        let rng = &mut shared.random;
        let get_to_focus = rng.value();
        let dir = Vec2::random_unit(rng);
        let cam_get_to = dir * rng.value();
        if let Some(s) = self.scene_mut(id) {
            s.get_to_focus = get_to_focus;
            s.cam_get_to = cam_get_to;
            s.cam_pos = cam_get_to;
            s.last_cam_pos = cam_get_to;
            s.cam_get_to2 = cam_get_to;
        }
        (id, glow_sprites)
    }

    /// The flat branch of `BuildScene`: one image (two for the death scene) at the
    /// screen centre.
    fn build_flat_scene(&mut self, shared: &mut MenuShared, id: ObjectId, scene: SceneId) {
        for (folder, file) in scene.flat_files(shared) {
            let flat = self.add_illustration(
                shared,
                id,
                &folder,
                &file,
                Vec2::new(683.0, 384.0),
                false,
                true,
            );
            if let Some(s) = self.scene_mut(id) {
                s.flat_illustrations.push(flat);
            }
            self.add_sub(id, flat);
        }
    }

    /// `new SlideShowMenuScene(menu, owner, sceneID)`: its depth layers, or its flat
    /// illustrations in flat mode (the title is flat in both), then its camera path and
    /// alphas; no random draws.
    pub fn add_slide_show_scene(
        &mut self,
        shared: &mut MenuShared,
        owner: ObjectId,
        scene: SceneId,
    ) -> ObjectId {
        let mut data = SceneData::new(scene);
        data.interactive = false;
        data.flat_mode = shared.flat_illustrations;
        let id = self.push(MenuObject::new(
            Some(owner),
            Vec2::ZERO,
            Kind::Scene(Box::new(data)),
        ));
        let depth = if shared.flat_illustrations {
            None
        } else {
            scene.slide_show_depth_scene()
        };
        let flats: Vec<ObjectId> = match (depth, scene.slide_show_layers()) {
            (Some(table), _) => {
                self.build_depth_scene(shared, id, &table, false, false);
                Vec::new()
            }
            (None, FlatLayers::None) => Vec::new(),
            (None, FlatLayers::Title) => vec![self.add_illustration(
                shared,
                id,
                "",
                "MainTitle",
                Vec2::new(433.0, 340.0),
                true,
                false,
            )],
            (None, FlatLayers::Scene { folder, files }) => files
                .iter()
                .map(|file| {
                    self.add_illustration(
                        shared,
                        id,
                        folder,
                        file,
                        Vec2::new(683.0, 384.0),
                        false,
                        true,
                    )
                })
                .collect(),
        };
        for flat in flats {
            if let Some(s) = self.scene_mut(id) {
                s.flat_illustrations.push(flat);
            }
            self.add_sub(id, flat);
        }
        self.init_slide_show_scene(&shared.assets, id);
        id
    }

    /// Takes a scene off its owner and the stage and frees its images
    /// (`RemoveSprites`, `subObjects.Remove`, `UnloadImages`).
    pub fn remove_scene(&mut self, scene: ObjectId, shared: &mut MenuShared) {
        let illustrations: Vec<ObjectId> = match &self.objects[scene.0].kind {
            Kind::Scene(s) => s
                .depth_illustrations
                .iter()
                .chain(&s.flat_illustrations)
                .copied()
                .collect(),
            _ => return,
        };
        for illustration in illustrations {
            if let Kind::Illustration(i) = &self.objects[illustration.0].kind {
                let (sprite, file) = (i.sprite, i.file_name.clone());
                self.stage.remove_from_container(sprite);
                shared.atlases.unload_atlas(&file);
            }
        }
        if let Some(owner) = self.objects[scene.0].owner {
            self.objects[owner.0].sub_objects.retain(|&o| o != scene);
        }
        if self.scene == Some(scene) {
            self.scene = None;
        }
    }

    /// `obj.RemoveSprites()` then `owner.RemoveSubObject(obj)`: its sprites leave the
    /// stage, it and its sub-objects stop being selectable, and its owner drops it.
    pub fn remove_sub_object(&mut self, id: ObjectId) {
        for node in self.object_nodes(id) {
            self.stage.remove_from_container(node);
        }
        let page = self.page_of(id);
        let tree = self.object_tree(id);
        self.page_data_mut(page)
            .selectables
            .retain(|s| !tree.contains(s));
        if let Some(owner) = self.objects[id.0].owner {
            self.objects[owner.0].sub_objects.retain(|&o| o != id);
        }
    }

    /// The object and its sub-objects, recursively.
    fn object_tree(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut tree = vec![id];
        let mut i = 0;
        while let Some(&o) = tree.get(i) {
            tree.extend(self.objects[o.0].sub_objects.iter().copied());
            i += 1;
        }
        tree
    }

    /// The stage nodes of an object and its sub-objects.
    fn object_nodes(&self, id: ObjectId) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        for o in self.object_tree(id) {
            match &self.objects[o.0].kind {
                Kind::Label(l) => nodes.push(l.label),
                Kind::RoundedRect(r) => nodes.extend(r.sprites.iter().copied()),
                Kind::Illustration(i) => nodes.push(i.sprite),
                Kind::DarkGradient(g) => nodes.push(g.sprite),
                Kind::HoldButton(h) => nodes.extend(h.circles),
                Kind::Slider(sl) => nodes.extend(sl.line_sprites),
                Kind::Button(b) => match &b.variant {
                    ButtonVariant::BigArrow { symbol, .. }
                    | ButtonVariant::CheckBox { symbol, .. } => nodes.push(*symbol),
                    ButtonVariant::Simple { .. } => {}
                },
                _ => {}
            }
            if let Some(c) = self.objects[o.0].my_container {
                nodes.push(c);
            }
        }
        nodes
    }

    /// The game's `label.textRect.width` of a menu label.
    #[must_use]
    pub fn label_text_width(&self, label: ObjectId) -> f32 {
        match &self.objects[label.0].kind {
            Kind::Label(l) => self.stage.label(l.label).text_rect().width,
            _ => 0.0,
        }
    }
}

impl SceneId {
    /// `MenuScene.BuildScene` in flat mode: the folder and file of each flat image.
    /// The sleep screen's slugcat is `progression.PlayingAsSlugcat`: the scene is built
    /// after the switch, when the main loop is no longer the game.
    #[must_use]
    pub fn flat_files(self, shared: &MenuShared) -> Vec<(String, String)> {
        match self {
            Self::MainMenuDownpour if shared.dlcs.downpour => {
                Self::single("Scenes/Main Menu - Downpour", "Main Menu - Downpour - Flat")
            }
            Self::MainMenu | Self::MainMenuDownpour => {
                Self::single("Scenes/Main Menu", "Main Menu - Flat")
            }
            Self::SlugcatWhite => Self::single("Scenes/Slugcat - White", "Slugcat - White - Flat"),
            Self::SlugcatYellow => {
                Self::single("Scenes/Slugcat - Yellow", "Slugcat - Yellow - Flat")
            }
            Self::SlugcatRed if shared.progression.red_unlocked => {
                Self::single("Scenes/Slugcat - Red", "Slugcat - Red - Flat")
            }
            Self::SlugcatRed => Self::single("Scenes/Slugcat - Red", "Slugcat - Red Dark - Flat"),
            Self::SlugcatDeadRed => Self::single("Scenes/Dead Red", "Dead Red - Flat"),
            Self::GhostWhite => {
                Self::single("Scenes/White Ghost Slugcat", "White Ghost Slugcat - Flat")
            }
            Self::GhostYellow => {
                Self::single("Scenes/Yellow Ghost Slugcat", "Yellow Ghost Slugcat - Flat")
            }
            Self::GhostRed => Self::single("Scenes/Red Ghost Slugcat", "Red Ghost Slugcat - Flat"),
            Self::LandscapeSu => Self::single("Scenes/Landscape - SU", "Landscape - SU - Flat"),
            Self::SleepScreen => {
                let slugcat = shared.playing_as_slugcat().value();
                Self::single(
                    &format!("Scenes/Sleep Screen - {slugcat}"),
                    &format!("Sleep Screen - {slugcat} - Flat"),
                )
            }
            Self::NewDeath => vec![
                ("Scenes/New Death".to_owned(), "New Death - Flat".to_owned()),
                (
                    "Scenes/New Death".to_owned(),
                    "New Death Flower - Flat".to_owned(),
                ),
            ],
            Self::StarveScreen => Self::single("Scenes/Starve Screen", "Starve Screen - Flat"),
            _ => match self.slide_show_layers() {
                FlatLayers::Scene { folder, files } => files
                    .first()
                    .map(|f| Self::single(folder, f))
                    .unwrap_or_default(),
                FlatLayers::Title => Self::single("", "MainTitle"),
                FlatLayers::None => Vec::new(),
            },
        }
    }

    /// One flat image.
    fn single(folder: &str, file: &str) -> Vec<(String, String)> {
        vec![(folder.to_owned(), file.to_owned())]
    }

    /// `MenuScene.BuildScene` in flat mode for the slideshow scenes.
    #[must_use]
    pub fn slide_show_layers(self) -> FlatLayers {
        let (folder, files): (&'static str, &'static [&'static str]) = match self {
            Self::Intro1Tree => ("Scenes/Intro 1 - Tree", &["Intro 1 - Tree - Flat"]),
            Self::Intro2Branch => ("Scenes/Intro 2 - Branch", &["Intro 2 - Branch - Flat"]),
            Self::Intro3InTree => ("Scenes/Intro 3 - In Tree", &["Intro 3 - In Tree - Flat"]),
            Self::Intro4Walking => ("Scenes/Intro 4 - Walking", &["Intro 4 - Walking - Flat"]),
            Self::Intro5Hunting => ("Scenes/Intro 5 - Hunting", &["Intro 5 - Hunting - Flat"]),
            Self::Intro67RainDrop => (
                "Scenes/Intro 6 - 7 - Rain Drop",
                &[
                    "Intro 6 - 7 - Rain Drop - Flat",
                    "Intro 6 - 7 - Rain Drop - Flat2",
                ],
            ),
            Self::Intro8Climbing => ("Scenes/Intro 8 - Climbing", &["Intro 8 - Climbing - Flat"]),
            Self::Intro9RainyClimb => (
                "Scenes/Intro 9 - Rainy Climb",
                &["Intro 9 - Rainy Climb - Flat"],
            ),
            Self::Intro10Fall => ("Scenes/Intro 10 - Fall", &["Intro 10 - Fall - Flat"]),
            Self::Intro105Separation => (
                "Scenes/Intro 10p5 - Separation",
                &["Intro 10p5 - Separation - Flat"],
            ),
            Self::Intro11Drowning => (
                "Scenes/Intro 11 - Drowning",
                &["Intro 11 - Drowning - Flat"],
            ),
            Self::Intro12Waking => ("Scenes/Intro 12 - Waking", &["Intro 12 - Waking - Flat"]),
            Self::Intro13Alone => ("Scenes/Intro 13 - Alone", &["Intro 13 - Alone - Flat"]),
            Self::YellowIntroA => (
                "Scenes/Yellow Intro A",
                &["Yellow Intro A - Flat", "Yellow Intro A - Flat2"],
            ),
            Self::YellowIntroB => ("Scenes/Yellow Intro B", &["Yellow Intro B - Flat"]),
            Self::Intro14Title => return FlatLayers::Title,
            _ => return FlatLayers::None,
        };
        FlatLayers::Scene { folder, files }
    }
}

/// The flat illustrations a slideshow scene builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatLayers {
    /// `Empty`: nothing.
    None,
    /// `<folder>/<file>.png` per layer at the screen centre (not crisp).
    Scene {
        folder: &'static str,
        files: &'static [&'static str],
    },
    /// `Intro_14_Title`: `Illustrations/MainTitle.png`, crisp, at (433, 340) by its
    /// bottom-left corner.
    Title,
}

impl SceneData {
    #[must_use]
    pub fn new(id: SceneId) -> Self {
        Self {
            id,
            interactive: true,
            flat_mode: true,
            display_time: 0.0,
            camera_path: super::CameraPath::default(),
            depth_illustrations: Vec::new(),
            flat_illustrations: Vec::new(),
            idle_depths: Vec::new(),
            blur_min: 0.25,
            blur_max: 0.8,
            hidden: false,
            focus: 0.0,
            last_focus: 0.0,
            cam_pos: Vec2::ZERO,
            last_cam_pos: Vec2::ZERO,
            camera_range: 1.0,
            get_to_focus: 0.0,
            cam_vel: Vec2::ZERO,
            cam_get_to: Vec2::ZERO,
            cam_get_to2: Vec2::ZERO,
            idle_cam_speed: 0.0,
            idle_cam_speed_get_to: 0.0,
            mouse_active: 0.0,
            game_pad_active: 0.0,
            mouse_inactive_counter: 0,
            game_pad_inactive_counter: 0,
            time_since_mouse_on_button_counter: 0,
            timer: -1,
        }
    }
}

impl HslColor {
    /// `ButtonTemplate.InterpColor`-style lerp toward black by `black`.
    #[must_use]
    pub fn blacken(self, black: f32) -> Self {
        self.lerp(MenuColor::Black.hsl(), black)
    }
}
