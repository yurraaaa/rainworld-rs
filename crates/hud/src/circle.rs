//! `HUDCircle` (a ring drawn with `VectorCircleFadable`, or a ready-made atlas element
//! once it settles on its snap size) and `FadeCircle` (an expanding ring that fades).

use rw_menu::{AtlasManager, NodeId, Sprite, StageColor, StageTree};
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

/// `HUDCircle.SnapToGraphic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapGraphic {
    None,
    FoodCircleA,
    FoodCircleB,
    DeerEyeB,
    SmallEmptyCircle,
    KarmaRing,
    Circle4,
}

impl SnapGraphic {
    /// The atlas element (`snapGraphic.ToString()`).
    #[must_use]
    pub fn element(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::FoodCircleA => "FoodCircleA",
            Self::FoodCircleB => "FoodCircleB",
            Self::DeerEyeB => "deerEyeB",
            Self::SmallEmptyCircle => "smallEmptyCircle",
            Self::KarmaRing => "karmaRing",
            Self::Circle4 => "Circle4",
        }
    }

    /// `snapRad` and `snapThickness` (-1 is a filled disc).
    #[must_use]
    pub fn snap(self) -> (f32, f32) {
        match self {
            Self::FoodCircleA => (11.5, 2.0),
            Self::FoodCircleB => (5.5, -1.0),
            Self::DeerEyeB => (2.5, -1.0),
            Self::SmallEmptyCircle => (6.0, 1.0),
            Self::KarmaRing => (31.5, 4.0),
            Self::Circle4 => (2.0, -1.0),
            Self::None => (-1.0, -1.0),
        }
    }
}

/// `Custom.FadableVectorCircleColors` (`RainWorld.GoldRGB` and `RippleColor` as the
/// `VectorCircleFadable` shader has them).
pub const FADABLE_COLORS: [StageColor; 8] = [
    StageColor::rgb(1.0, 1.0, 1.0),
    StageColor::rgb(1.0, 0.0, 0.0),
    StageColor::rgb(0.0, 1.0, 0.0),
    StageColor::rgb(0.0, 0.0, 1.0),
    StageColor::rgb(0.529, 0.365, 0.184),
    StageColor::rgb(0.0, 0.0, 0.0),
    StageColor::rgb(1.0, 0.7, 0.35),
    StageColor::rgb(0.373, 0.11, 0.831),
];

/// `HUD.HUDCircle`.
#[derive(Debug, Clone)]
pub struct HudCircle {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub rad: f32,
    pub last_rad: f32,
    pub thickness: f32,
    pub last_thickness: f32,
    pub sprite: NodeId,
    pub snap_graphic: SnapGraphic,
    pub snap_rad: f32,
    pub snap_thickness: f32,
    pub visible: bool,
    pub fade: f32,
    pub last_fade: f32,
    /// Index into [`FADABLE_COLORS`].
    pub color: usize,
    pub force_color: Option<StageColor>,
}

impl HudCircle {
    /// The constructor: an invisible `Futile_White` sprite added to `container`, at its
    /// snap size, at (-100, -100).
    pub fn new(
        stage: &mut StageTree,
        container: NodeId,
        snap_graphic: SnapGraphic,
        color: usize,
    ) -> Self {
        let mut sprite = Sprite::new(AtlasManager::WHITE);
        sprite.shader = StageShader::VectorCircleFadable;
        sprite.transform.visible = false;
        let sprite = stage.add_sprite(container, sprite);
        let (snap_rad, snap_thickness) = snap_graphic.snap();
        let pos = Vec2::new(-100.0, -100.0);
        Self {
            pos,
            last_pos: pos,
            rad: snap_rad,
            last_rad: 0.0,
            thickness: snap_thickness,
            last_thickness: 0.0,
            sprite,
            snap_graphic,
            snap_rad,
            snap_thickness,
            visible: true,
            fade: 1.0,
            last_fade: 0.0,
            color,
            force_color: None,
        }
    }

    pub fn update(&mut self) {
        self.last_pos = self.pos;
        self.last_rad = self.rad;
        self.last_thickness = self.thickness;
        self.last_fade = self.fade;
    }

    pub fn draw(&self, stage: &mut StageTree, t: f32) {
        let pos = self.last_pos.lerp(self.pos, t);
        let rad = self.last_rad.lerp(self.rad, t);
        let mut thickness = self.last_thickness.lerp(self.thickness, t);
        let sprite = stage.sprite_mut(self.sprite);
        if rad <= 0.0 || !self.visible || (self.last_fade == 0.0 && self.fade == 0.0) {
            sprite.transform.visible = false;
            return;
        }
        sprite.transform.visible = true;
        if thickness > rad {
            thickness = rad;
        }
        sprite.transform.x = pos.x;
        sprite.transform.y = pos.y;
        let fade = self.last_fade.lerp(self.fade, t);
        if rad == self.snap_rad && thickness == self.snap_thickness {
            self.snap_graphic.element().clone_into(&mut sprite.element);
            sprite.transform.set_scale(1.0);
            sprite.shader = StageShader::Basic;
            sprite.transform.alpha = fade;
            sprite.color = FADABLE_COLORS[self.color];
        } else {
            AtlasManager::WHITE.clone_into(&mut sprite.element);
            sprite.transform.set_scale(rad / 8.0);
            sprite.transform.alpha = if thickness == -1.0 {
                1.0
            } else if rad > 0.0 {
                thickness / rad
            } else {
                0.0
            };
            sprite.shader = StageShader::VectorCircleFadable;
            sprite.color = StageColor::rgb(self.color as f32 / 255.0, 0.0, fade);
        }
        if let Some(c) = self.force_color {
            sprite.color = c;
        }
    }
}

/// `HUD.FadeCircle`.
#[derive(Debug, Clone)]
pub struct FadeCircle {
    pub circle: HudCircle,
    pub rad: f32,
    pub rad_speed: f32,
    pub life_time: f32,
    pub life: f32,
    pub thickness: f32,
    pub rad_slow_down: f32,
    pub alpha_multiply: f32,
    pub fade_thickness: bool,
}

impl FadeCircle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        stage: &mut StageTree,
        container: NodeId,
        rad: f32,
        rad_speed: f32,
        rad_slow_down: f32,
        life_time: f32,
        thickness: f32,
        pos: Vec2,
    ) -> Self {
        let mut circle = HudCircle::new(stage, container, SnapGraphic::None, 0);
        circle.pos = pos;
        circle.thickness = thickness;
        Self {
            circle,
            rad,
            rad_speed,
            life_time,
            life: 1.0,
            thickness,
            rad_slow_down,
            alpha_multiply: 0.15,
            fade_thickness: true,
        }
    }

    pub fn update(&mut self) {
        self.circle.last_rad = self.rad;
        self.rad += self.rad_speed;
        self.rad_speed *= self.rad_slow_down;
        self.life -= 1.0 / self.life_time;
        self.circle.update();
        self.circle.rad = self.rad;
        self.circle.fade = self.life.mathf_pow(0.5) * self.alpha_multiply;
        if self.fade_thickness {
            self.circle.thickness = self.life * self.thickness;
        }
    }
}
