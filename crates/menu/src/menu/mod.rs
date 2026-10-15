//! `Menu`: pages of menu objects, keyboard and mouse selection, the press and hold of
//! buttons and the background sound loop. Screens (`MainMenu`, `SlugcatSelectMenu`)
//! own a [`Menu`] and answer its signals through [`MenuScreen`]. Behaviour:
//! `rainworld-re/notes/menu.md` ("Menu framework").

mod build;
mod depth;
mod draw;
mod object;
mod rain;
mod scene;
mod slide_show_scene;
mod update;

use rw_sim::Vec2;

use crate::input::UiInput;
use crate::process::{ProcessId, ProcessRequests};
use crate::shared::MenuShared;
use crate::sound::{LoopId, MenuSoundId};
use crate::tree::{NodeId, StageTree};

pub use build::FlatLayers;
pub use depth::{DepthLayer, DepthScene, MenuShader};
pub use object::{
    ButtonBehavior, ButtonData, ButtonVariant, CursorData, DarkGradientData, DepthData,
    HoldButtonData, IllustrationData, Kind, LabelData, MenuObject, ObjectId, PageData,
    RoundedRectData, SceneData, SceneId, SimpleKind, SliderData, SliderId,
};
pub use rain::{RainDrop, RainEffectData};
pub use slide_show_scene::CameraPath;

/// What a menu's update reaches outside itself: the shared state (input, sound,
/// randomness, atlases) and the process manager.
pub struct MenuCx<'a> {
    pub shared: &'a mut MenuShared,
    pub requests: &'a mut ProcessRequests,
}

impl MenuCx<'_> {
    /// `Menu.PlaySound(id)`.
    pub fn play_sound(&mut self, id: MenuSoundId) {
        self.play_sound_with(id, [0.0, 1.0, 1.0]);
    }

    /// `Menu.PlaySound(id, pan, vol, pitch)`.
    pub fn play_sound_with(&mut self, id: MenuSoundId, args: [f32; 3]) {
        self.shared.play_menu_sound(id, args);
    }

    /// `Menu.PlayLoop`: nothing without a menu microphone.
    pub fn play_loop(&mut self, id: MenuSoundId, args: [f32; 3], is_bkg: bool) -> Option<LoopId> {
        let shared = &mut *self.shared;
        if !shared.mic_attached {
            return None;
        }
        shared.mic.play_loop(id, args, is_bkg, &mut shared.random)
    }
}

/// The parts of a screen the base `Menu` calls back into (its virtual methods).
pub trait MenuScreen {
    /// `Singal`: a button (or other object) sent `message`.
    fn signal(&mut self, menu: &mut Menu, cx: &mut MenuCx<'_>, sender: ObjectId, message: &str);

    /// The screen's part of `Init`, after the base's.
    fn init(&mut self, _menu: &mut Menu) {}

    /// The screen's part of `FreezeMenuFunctions`.
    fn freeze(&self) -> bool {
        false
    }

    /// A page's own `Update`, after its sub-objects'.
    fn page_update(&mut self, _menu: &mut Menu, _page: ObjectId, _cx: &mut MenuCx<'_>) {}

    /// A `SlideShowMenuScene`'s own `Update`, after its sub-objects'.
    fn scene_update(&mut self, _menu: &mut Menu, _scene: ObjectId) {}

    /// A page's own `GrafUpdate`, after its sub-objects'.
    fn page_graf_update(&self, _menu: &mut Menu, _page: ObjectId, _time_stacker: f32) {}

    /// `ShowCursor`, where a screen overrides it to always hide the cursor.
    fn shows_cursor(&self) -> bool {
        true
    }

    /// `CheckBox.IOwnCheckBox.GetChecked`.
    fn checked(&self, _menu: &Menu, _check_box: ObjectId) -> bool {
        false
    }

    /// `CheckBox.IOwnCheckBox.SetChecked`.
    fn set_checked(&mut self, _menu: &mut Menu, _check_box: ObjectId, _checked: bool) {}

    /// `UpdateInfoText` for the selected object.
    fn info_text(&self, _menu: &Menu, _cx: &MenuCx<'_>) -> String {
        String::new()
    }

    /// `SelectOneButtonOwner.GetCurrentlySelectedOfSeries`.
    fn selected_of_series(&self, _cx: &MenuCx<'_>, _series: &str) -> i32 {
        -1
    }

    /// `SelectOneButtonOwner.SetCurrentlySelectedOfSeries`.
    fn set_selected_of_series(
        &mut self,
        _menu: &mut Menu,
        _cx: &mut MenuCx<'_>,
        _series: &str,
        _to: i32,
    ) {
    }

    /// `ValueOfSlider`.
    fn slider_value(&self, _cx: &MenuCx<'_>, _slider: SliderId) -> f32 {
        0.0
    }

    /// `SliderSetValue`.
    fn set_slider_value(
        &mut self,
        _menu: &mut Menu,
        _cx: &mut MenuCx<'_>,
        _slider: ObjectId,
        _value: f32,
    ) {
    }
}

/// The base menu.
pub struct Menu {
    pub id: ProcessId,
    pub objects: Vec<MenuObject>,
    pub pages: Vec<ObjectId>,
    pub stage: StageTree,
    /// `container`: on the stage from the start.
    pub container: NodeId,
    /// `cursorContainer`: added on top in `Init`.
    pub cursor_container: NodeId,
    info_label: Option<NodeId>,
    pub scene: Option<ObjectId>,
    /// The menu scene shader globals as a scene's `GrafUpdate` last set them.
    pub scene_globals: Option<rw_render::MenuSceneGlobals>,
    pub gradients_container: Option<ObjectId>,
    pub selected: Option<ObjectId>,
    pub back_object: Option<ObjectId>,
    pub press_button: bool,
    pub hold_button: bool,
    pub last_hold_button: bool,
    pub input: UiInput,
    pub last_input: UiInput,
    pub mouse_pos: Vec2,
    pub last_mouse_pos: Vec2,
    pub mouse_down: bool,
    pub last_mouse_down: bool,
    mode_switch: bool,
    pub current_page: usize,
    scroll_init_delay: i32,
    scroll_delay: i32,
    /// `soundLoop`: the background loop once found or started.
    pub sound_loop: Option<LoopId>,
    /// `mySoundLoopID`.
    pub my_sound_loop: Option<MenuSoundId>,
    all_alien_loops_gone: bool,
    info_label_fade: f32,
    last_info_label_fade: f32,
    info_label_sin: f32,
    info_label_dirty: bool,
    pub mouse_scroll_wheel_movement: i32,
    last_scroll_wheel: i32,
    float_scroll_wheel: f32,
    init: bool,
    allow_select_move: bool,
    /// `FreezeMenuFunctions` as of this tick's start (`ShowCursor` reads it).
    frozen: bool,
    /// This tick's input when another menu took it first (a dialog over a menu reads
    /// the same `RWInput`).
    pub given_input: Option<UiInput>,
    screen_size: [f32; 2],
}

impl Menu {
    /// The menu constructor: its container on the stage, the info label at the bottom
    /// (all but `Initialization`) and the cursor container, not yet shown.
    #[must_use]
    pub fn new(id: ProcessId, shared: &MenuShared) -> Self {
        let mut stage = StageTree::new();
        let container = stage.add_container(StageTree::ROOT);
        let cursor_container = stage.new_container();
        let info_label = (id != ProcessId::Initialization)
            .then(|| shared.atlases.font(crate::Font::FONT).cloned())
            .flatten()
            .map(|font| {
                let mut label = crate::Label::new(font, "");
                label.transform.x = shared.screen_size[0] / 2.0 + 0.01;
                label.transform.y = 20.01_f32.max(0.01);
                stage.add_label(StageTree::ROOT, label)
            });
        Self {
            id,
            objects: Vec::new(),
            pages: Vec::new(),
            stage,
            container,
            cursor_container,
            info_label,
            scene: None,
            scene_globals: None,
            gradients_container: None,
            selected: None,
            back_object: None,
            press_button: false,
            hold_button: false,
            last_hold_button: false,
            input: UiInput::default(),
            last_input: UiInput::default(),
            mouse_pos: Vec2::ZERO,
            last_mouse_pos: Vec2::ZERO,
            mouse_down: false,
            last_mouse_down: false,
            mode_switch: false,
            current_page: 0,
            scroll_init_delay: 0,
            scroll_delay: 0,
            sound_loop: None,
            my_sound_loop: None,
            all_alien_loops_gone: false,
            info_label_fade: 0.0,
            last_info_label_fade: 0.0,
            info_label_sin: 0.0,
            info_label_dirty: false,
            mouse_scroll_wheel_movement: 0,
            last_scroll_wheel: 0,
            float_scroll_wheel: 0.0,
            init: false,
            allow_select_move: true,
            frozen: false,
            given_input: None,
            screen_size: shared.screen_size,
        }
    }

    #[must_use]
    pub fn object(&self, id: ObjectId) -> &MenuObject {
        &self.objects[id.0]
    }

    pub fn object_mut(&mut self, id: ObjectId) -> &mut MenuObject {
        &mut self.objects[id.0]
    }

    /// `subObjects.Add`.
    pub fn add_sub(&mut self, owner: ObjectId, sub: ObjectId) {
        self.objects[owner.0].sub_objects.push(sub);
    }

    #[must_use]
    pub fn screen_size(&self) -> [f32; 2] {
        self.screen_size
    }

    /// `Menu.HorizontalMoveToGetCentered`.
    #[must_use]
    pub fn horizontal_move_to_get_centered(&self) -> f32 {
        (1366.0 - self.screen_size[0]) / 2.0
    }

    /// The page an object is on (`MenuObject.page`).
    #[must_use]
    pub fn page_of(&self, id: ObjectId) -> ObjectId {
        let mut id = id;
        loop {
            if matches!(self.objects[id.0].kind, Kind::Page(_)) {
                return id;
            }
            match self.objects[id.0].owner {
                Some(owner) => id = owner,
                None => return id,
            }
        }
    }

    #[must_use]
    pub fn page_data(&self, page: ObjectId) -> &PageData {
        match &self.objects[page.0].kind {
            Kind::Page(p) => p,
            _ => panic!("menu object {page:?} is not a page"),
        }
    }

    pub fn page_data_mut(&mut self, page: ObjectId) -> &mut PageData {
        match &mut self.objects[page.0].kind {
            Kind::Page(p) => p,
            _ => panic!("menu object {page:?} is not a page"),
        }
    }

    /// `Container`: the object's own container, else its owner's, else the menu's.
    #[must_use]
    pub fn container_of(&self, id: ObjectId) -> NodeId {
        let mut id = id;
        loop {
            let object = &self.objects[id.0];
            if let Some(c) = object.my_container {
                return c;
            }
            match object.owner {
                Some(owner) => id = owner,
                None => return self.container,
            }
        }
    }

    /// `ScreenPos`: the position plus the owners' while they are positioned.
    #[must_use]
    pub fn screen_pos(&self, id: ObjectId) -> Vec2 {
        let object = &self.objects[id.0];
        match object.owner {
            Some(owner) if self.objects[owner.0].kind.positioned() => {
                self.screen_pos(owner) + object.pos
            }
            _ => object.pos,
        }
    }

    /// `ScreenLastPos`.
    #[must_use]
    pub fn screen_last_pos(&self, id: ObjectId) -> Vec2 {
        let object = &self.objects[id.0];
        match object.owner {
            Some(owner) if self.objects[owner.0].kind.positioned() => {
                self.screen_last_pos(owner) + object.last_pos
            }
            _ => object.last_pos,
        }
    }

    /// `DrawPos`.
    #[must_use]
    pub fn draw_pos(&self, id: ObjectId, t: f32) -> Vec2 {
        self.screen_last_pos(id).lerp(self.screen_pos(id), t)
    }

    /// `DrawSize`.
    #[must_use]
    pub fn draw_size(&self, id: ObjectId, t: f32) -> Vec2 {
        let o = &self.objects[id.0];
        o.last_size.lerp(o.size, t)
    }

    /// `Selected`.
    #[must_use]
    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selected == Some(id)
    }

    /// `ResetSelection`.
    pub fn reset_selection(&mut self) {
        self.selected = None;
        for i in 0..self.pages.len() {
            let page = self.pages[i];
            let first = self.page_data(page).selectables.first().copied();
            if first.is_some() {
                self.page_data_mut(page).last_selected = first;
            }
        }
    }

    /// `MenuLabel.text = text`.
    pub fn set_label_text(&mut self, label: ObjectId, text: &str) {
        if let Kind::Label(l) = &self.objects[label.0].kind {
            let node = l.label;
            self.stage.label_mut(node).set_text(text);
        }
    }

    /// The `ButtonBehavior` of a button or hold button.
    pub fn behavior_mut(&mut self, id: ObjectId) -> Option<&mut ButtonBehavior> {
        match &mut self.objects[id.0].kind {
            Kind::Button(b) => Some(&mut b.behav),
            Kind::HoldButton(h) => Some(&mut h.behav),
            Kind::Slider(s) => Some(&mut s.behav),
            _ => None,
        }
    }

    #[must_use]
    pub fn behavior(&self, id: ObjectId) -> Option<&ButtonBehavior> {
        match &self.objects[id.0].kind {
            Kind::Button(b) => Some(&b.behav),
            Kind::HoldButton(h) => Some(&h.behav),
            Kind::Slider(s) => Some(&s.behav),
            _ => None,
        }
    }

    /// `HoldButton` state.
    pub fn hold_button_mut(&mut self, id: ObjectId) -> Option<&mut HoldButtonData> {
        match &mut self.objects[id.0].kind {
            Kind::HoldButton(h) => Some(h),
            _ => None,
        }
    }

    #[must_use]
    pub fn hold_button(&self, id: ObjectId) -> Option<&HoldButtonData> {
        match &self.objects[id.0].kind {
            Kind::HoldButton(h) => Some(h),
            _ => None,
        }
    }

    #[must_use]
    pub fn button(&self, id: ObjectId) -> Option<&ButtonData> {
        match &self.objects[id.0].kind {
            Kind::Button(b) => Some(b),
            _ => None,
        }
    }

    pub fn button_mut(&mut self, id: ObjectId) -> Option<&mut ButtonData> {
        match &mut self.objects[id.0].kind {
            Kind::Button(b) => Some(b),
            _ => None,
        }
    }

    #[must_use]
    pub fn slider(&self, id: ObjectId) -> Option<&SliderData> {
        match &self.objects[id.0].kind {
            Kind::Slider(s) => Some(s),
            _ => None,
        }
    }

    pub fn slider_mut(&mut self, id: ObjectId) -> Option<&mut SliderData> {
        match &mut self.objects[id.0].kind {
            Kind::Slider(s) => Some(s),
            _ => None,
        }
    }

    /// A `SimpleButton`'s subclass.
    #[must_use]
    pub fn simple_kind(&self, id: ObjectId) -> Option<&SimpleKind> {
        match &self.button(id)?.variant {
            ButtonVariant::Simple { sub, .. } => Some(sub),
            _ => None,
        }
    }

    /// The text of a label object.
    #[must_use]
    pub fn label_text(&self, label: ObjectId) -> Option<&str> {
        match &self.objects[label.0].kind {
            Kind::Label(l) => Some(self.stage.label(l.label).text()),
            _ => None,
        }
    }

    /// `infoLabel.text = text; infoLabelSin = 0; infoLabelFade = 1` (a slider moved).
    pub fn flash_info_text(&mut self, text: &str) {
        if let Some(node) = self.info_label {
            self.stage.label_mut(node).set_text(text);
        }
        self.info_label_sin = 0.0;
        self.info_label_fade = 1.0;
    }

    /// `infoLabel.y = y`.
    pub fn set_info_label_y(&mut self, y: f32) {
        if let Some(node) = self.info_label {
            self.stage.label_mut(node).transform.y = y;
        }
    }

    /// `infoLabel.text = text; infolabelDirty = false; infoLabelSin = 0` (a screen's
    /// `RefreshInfoLabel`).
    pub fn refresh_info_label(&mut self, text: &str) {
        if let Some(node) = self.info_label {
            self.stage.label_mut(node).set_text(text);
            self.info_label_dirty = false;
            self.info_label_sin = 0.0;
        }
    }

    /// The info label's text.
    #[must_use]
    pub fn info_text(&self) -> &str {
        self.info_label
            .map_or("", |node| self.stage.label(node).text())
    }

    /// `CheckBox.selectable`.
    pub fn set_check_box_selectable(&mut self, id: ObjectId, value: bool) {
        if let Some(ButtonData {
            variant: ButtonVariant::CheckBox { selectable, .. },
            ..
        }) = self.button_mut(id)
        {
            *selectable = value;
        }
    }

    /// `CheckBox.label`.
    #[must_use]
    pub fn check_box_label(&self, id: ObjectId) -> Option<ObjectId> {
        match self.button(id)?.variant {
            ButtonVariant::CheckBox { label, .. } => Some(label),
            _ => None,
        }
    }

    pub fn scene_mut(&mut self, id: ObjectId) -> Option<&mut SceneData> {
        match &mut self.objects[id.0].kind {
            Kind::Scene(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn illustration(&self, id: ObjectId) -> Option<&IllustrationData> {
        match &self.objects[id.0].kind {
            Kind::Illustration(i) => Some(i),
            _ => None,
        }
    }

    pub fn illustration_mut(&mut self, id: ObjectId) -> Option<&mut IllustrationData> {
        match &mut self.objects[id.0].kind {
            Kind::Illustration(i) => Some(i),
            _ => None,
        }
    }

    /// Whether `Init` has run (on the first update).
    #[must_use]
    pub fn initialized(&self) -> bool {
        self.init
    }

    /// The illustration atlases this menu loaded, to free on shut-down
    /// (`HardCleanFutileAssets`).
    #[must_use]
    pub fn illustration_files(&self) -> Vec<String> {
        self.objects
            .iter()
            .filter_map(|o| match &o.kind {
                Kind::Illustration(i) => Some(i.file_name.clone()),
                _ => None,
            })
            .collect()
    }

    /// `Menu.ShutDownProcess`: while the manager has a menu microphone, its loops that
    /// are not background loops are destroyed; then the illustrations are freed
    /// (`AssetManager.HardCleanFutileAssets`).
    pub fn shut_down(&mut self, shared: &mut MenuShared) {
        if shared.mic_attached {
            let handles: Vec<LoopId> = shared
                .mic
                .loops()
                .iter()
                .filter(|l| !l.is_bkg)
                .map(|l| l.handle)
                .collect();
            for handle in handles {
                shared.mic.destroy_loop(handle);
            }
        }
        for file in self.illustration_files() {
            shared.atlases.unload_atlas(&file);
        }
    }

    /// `Menu.CommunicateWithUpcomingProcess`: a menu with the same background loop takes
    /// this one's over; otherwise it is destroyed.
    pub fn communicate_with_upcoming(&mut self, next: Option<&mut Menu>, shared: &mut MenuShared) {
        match next {
            Some(next)
                if self.my_sound_loop.is_some() && next.my_sound_loop == self.my_sound_loop =>
            {
                next.sound_loop = self.sound_loop;
            }
            _ => {
                if let Some(handle) = self.sound_loop {
                    shared.mic.destroy_loop(handle);
                }
            }
        }
    }
}
