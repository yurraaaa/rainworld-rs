//! `TutorialControlsPage` and its `ControlMap` for the keyboard preset: the picture of
//! the keyboard with the default bindings' labels, shown over the dimmed game when a
//! new game's slugcat first stands up in SU_C04, until it walks past x 800. The page is
//! a `Menu` whose page container sits in the HUD's second container; the port keeps its
//! sprites in the HUD stage. The gamepad presets' pictures, the pick-up instructions
//! and the special-button rows (More Slugcats, Watcher) are not ported.

use std::sync::Arc;

use rw_assets::StreamingAssets;
use rw_menu::{
    Alignment, AtlasManager, Font, Label, MenuColor, NodeId, Sprite, StageTree, Translator,
};
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

use crate::CustomFloat;

/// One `MenuIllustration` of the map: its sprite, position (relative to the control
/// map), texture size and alpha.
#[derive(Debug, Clone)]
struct Illustration {
    sprite: NodeId,
    pos: Vec2,
    size: Vec2,
    alpha: f32,
    last_alpha: f32,
    set_alpha: Option<f32>,
}

impl Illustration {
    /// `new MenuIllustration(menu, owner, "", file, pos, crispPixels: true,
    /// anchorCenter: false)`; the control map starts it at alpha 0.
    fn new(
        stage: &mut StageTree,
        container: NodeId,
        shared: &mut IllustrationSource<'_>,
        file: &str,
        pos: Vec2,
    ) -> Self {
        let size = shared
            .atlases
            .load_illustration(shared.assets, "Illustrations", file, true)
            .inspect_err(|e| log::warn!("illustration {file}: {e}"))
            .unwrap_or([1.0, 1.0]);
        let mut sprite = Sprite::new(file);
        sprite.anchor = [0.0, 0.0];
        sprite.transform.alpha = 0.0;
        Self {
            sprite: stage.add_sprite(container, sprite),
            pos,
            size: Vec2::from(size),
            alpha: 0.0,
            last_alpha: 0.0,
            set_alpha: None,
        }
    }

    /// `MenuIllustration.Update`.
    fn update(&mut self) {
        self.last_alpha = self.alpha;
        if let Some(a) = self.set_alpha.take() {
            self.alpha = a;
        }
    }
}

/// Where the page's illustrations are loaded from and into.
pub struct IllustrationSource<'a> {
    pub assets: &'a StreamingAssets,
    pub atlases: &'a mut AtlasManager,
}

/// What the page's text needs.
pub struct PageText<'a> {
    pub font: Option<&'a Arc<Font>>,
    pub translator: &'a Translator,
}

impl PageText<'_> {
    /// `"<key> - <action>"`, both translated.
    fn row(&self, key: &str, action: &str) -> String {
        let t = self.translator;
        format!("{} - {}", t.translate(key), t.translate(action))
    }
}

/// `Menu.ControlMap` for `Preset.KeyboardSinglePlayer` with no special button.
#[derive(Debug, Clone)]
pub struct ControlMap {
    /// `ScreenPos`: the page's position plus the map's.
    pub pos: Vec2,
    black_fade: NodeId,
    /// `None` without the font.
    labels: [Option<NodeId>; 7],
    label_positions: [Vec2; 7],
    /// `controlsMap`, `controlsMap2`, `controlsMap3`.
    maps: [Illustration; 3],
    pub fade: f32,
    pub last_fade: f32,
    pub counter: i32,
}

impl ControlMap {
    fn new(
        stage: &mut StageTree,
        container: NodeId,
        pos: Vec2,
        source: &mut IllustrationSource<'_>,
        text: &PageText<'_>,
    ) -> Self {
        let mut fade = Sprite::new(AtlasManager::WHITE);
        fade.shader = StageShader::FlatLight;
        fade.color = MenuColor::Black.rgb();
        let black_fade = stage.add_sprite(container, fade);
        // "Z" and "X" are not translated.
        let texts = [
            text.row("Arrows", "Move"),
            text.row("Hold Space", "Map"),
            text.row("Esc", "Pause"),
            text.row("Shift", "Pick up / Eat"),
            format!("Z - {}", text.translator.translate("Jump")),
            format!("X - {}", text.translator.translate("Throw")),
            String::new(),
        ];
        let labels = texts.map(|s| {
            text.font.map(|font| {
                let mut label = Label::new(Arc::clone(font), &s);
                label.set_alignment(Alignment::Left);
                label.transform.y = -10000.0;
                stage.add_label(container, label)
            })
        });
        let label_positions = [
            (742.0, 42.0),
            (300.0, 1.0),
            (20.0, 216.0),
            (20.0, 90.0),
            (20.0, 45.0),
            (20.0, 1.0),
            (0.0, 0.0),
        ]
        .map(|(x, y)| Vec2::new(x, y + 1.0));
        let keyboard = Vec2::new(-220.0, 0.0);
        let mut maps = [
            Illustration::new(stage, container, source, "keyboard", keyboard),
            Illustration::new(
                stage,
                container,
                source,
                "keyboard2",
                keyboard + Vec2::new(-134.0, -27.0),
            ),
            Illustration::new(
                stage,
                container,
                source,
                "keyboard3",
                keyboard + Vec2::new(16.0, 11.0),
            ),
        ];
        stage.sprite_mut(maps[0].sprite).color = MenuColor::MediumGrey.rgb();
        let size = maps[0].size;
        let s = &mut stage.sprite_mut(black_fade).transform;
        s.scale_x = (size.x * 2.0 + 200.0) / 16.0;
        s.scale_y = (size.y * 2.0 + 200.0) / 16.0;
        for m in &mut maps {
            m.alpha = 0.0;
            m.last_alpha = 0.0;
        }
        Self {
            pos,
            black_fade,
            labels,
            label_positions,
            maps,
            fade: 0.0,
            last_fade: 0.0,
            counter: 0,
        }
    }

    /// `ControlMap.Update` inside the tutorial page, with the page's `blackFade` and the
    /// HUD map's fade.
    fn update(&mut self, black_fade: f32, map_fade: f32) {
        for m in &mut self.maps {
            m.update();
        }
        self.counter += 1;
        self.last_fade = self.fade;
        self.fade = black_fade * (1.0 - map_fade);
        let later = (self.counter as f32).inverse_lerp(5.0, 80.0).s_curve(0.8);
        self.maps[0].set_alpha = Some(self.fade);
        self.maps[1].set_alpha = Some(self.fade.min(later));
        self.maps[2].set_alpha = Some(self.fade.min(later) * 0.5);
    }

    fn draw(&self, stage: &mut StageTree, t: f32) {
        let fade = self.last_fade.lerp(self.fade, t).s_curve(0.6);
        for m in &self.maps {
            let tr = &mut stage.sprite_mut(m.sprite).transform;
            let pos = self.pos + m.pos;
            tr.x = pos.x;
            tr.y = pos.y;
            tr.alpha = m.last_alpha.lerp(m.alpha, t);
        }
        let map = &self.maps[0];
        let tr = &mut stage.sprite_mut(self.black_fade).transform;
        tr.x = self.pos.x + map.pos.x + map.size.x * 0.5;
        tr.y = self.pos.y + map.pos.y + map.size.y * 0.5;
        tr.alpha = fade * 0.5;
        let anchor = self.pos + self.maps[1].pos;
        let alpha = self.maps[1].last_alpha.lerp(self.maps[1].alpha, t);
        for (i, (label, offset)) in self.labels.iter().zip(self.label_positions).enumerate() {
            let Some(label) = label else { continue };
            let mut x = anchor.x + offset.x;
            let mut y = anchor.y + offset.y + 6.0;
            match i {
                3..=6 => x -= 80.0,
                1 => y -= 20.0,
                _ => x -= 20.0,
            }
            let l = stage.label_mut(*label);
            l.transform.x = x;
            l.transform.y = y;
            l.transform.alpha = alpha;
        }
    }

    /// The labels' texts.
    #[must_use]
    pub fn texts(&self, stage: &StageTree) -> Vec<String> {
        self.labels
            .iter()
            .flatten()
            .map(|l| stage.label(*l).text().to_owned())
            .collect()
    }
}

/// `Menu.TutorialControlsPage`, run by its `TutorialControlsPageOwner`.
#[derive(Debug, Clone)]
pub struct ControlsPage {
    /// `pages[0].Container`, in the HUD's second container.
    pub container: NodeId,
    black_sprite: NodeId,
    pub black_fade: f32,
    pub last_black_fade: f32,
    pub counter: i32,
    pub want_to_continue: bool,
    /// `ShutDownProcess` ran and the owner destroyed itself.
    pub done: bool,
    pub control_map: ControlMap,
}

impl ControlsPage {
    /// `Page` position (1/3, 1/3) less `HorizontalMoveToGetCentered` (applied in
    /// `Init`, on the first update; the port applies it at once, nothing being drawn
    /// before).
    fn page_pos(screen: [f32; 2]) -> Vec2 {
        Vec2::new(1.0 / 3.0 - (1366.0 - screen[0]) / 2.0, 1.0 / 3.0)
    }

    /// The constructor: a page container on `hud_container`, the dimming sprite and the
    /// keyboard control map.
    pub fn new(
        stage: &mut StageTree,
        hud_container: NodeId,
        screen: [f32; 2],
        source: &mut IllustrationSource<'_>,
        text: &PageText<'_>,
    ) -> Self {
        let container = stage.add_container(hud_container);
        let mut black = Sprite::new("pixel");
        black.color = MenuColor::Black.rgb();
        black.transform.scale_x = 1400.0;
        black.transform.scale_y = 800.0;
        black.transform.x = screen[0] / 2.0;
        black.transform.y = screen[1] / 2.0;
        black.transform.alpha = 0.5;
        let black_sprite = stage.add_sprite(container, black);
        // The keyboard's picture sits 450 px up (350 for the PlayStation pads).
        let map_x = (screen[0] / 3.0).trunc() + 170.2 + ((1366.0 - screen[0]) / 2.0).trunc();
        let pos = Self::page_pos(screen) + Vec2::new(map_x, 450.0);
        let control_map = ControlMap::new(stage, container, pos, source, text);
        Self {
            container,
            black_sprite,
            black_fade: 0.0,
            last_black_fade: 0.0,
            counter: 0,
            want_to_continue: false,
            done: false,
            control_map,
        }
    }

    /// `Update`: fades in at 1/16 per tick, out at 1/8 once asked to continue, then
    /// shuts down (its sprites are removed from `stage`).
    pub fn update(&mut self, stage: &mut StageTree, map_fade: f32) {
        self.counter += 1;
        self.last_black_fade = self.black_fade;
        if self.want_to_continue {
            self.black_fade = (self.black_fade - 0.125).max(0.0);
            if self.black_fade <= 0.0 && !self.done {
                self.done = true;
                stage.remove_from_container(self.container);
            }
        } else {
            self.black_fade = (self.black_fade + 0.0625).min(1.0);
        }
        self.control_map.update(self.black_fade, map_fade);
    }

    /// `GrafUpdate`.
    pub fn draw(&self, stage: &mut StageTree, t: f32) {
        let fade = self.last_black_fade.lerp(self.black_fade, t);
        stage.sprite_mut(self.black_sprite).transform.alpha = fade.s_curve(0.6) * 0.25;
        self.control_map.draw(stage, t);
    }
}
