//! The map's `FadeInMarker`s that a base-game story map has: gates, shelters, death
//! positions and the karma flower.
//!
//! Mirrors: HUD/Map.cs (FadeInMarker)

use rw_menu::{AtlasManager, MenuColor, NodeId, Sprite, StageColor, StageTree};
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

use super::GateRequirement;

/// What a marker shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerKind {
    /// `GateMarker`: the gate's karma symbol, red while closed to the player.
    Gate { show_as_open: bool },
    /// `ShelterMarker` (the items stored in the shelter are not ported).
    Shelter,
    /// `DeathMarker`: bones, red the cycle it happened, greyer as it ages.
    Death {
        /// Cycles since the death (the saved coordinate's `abstractNode`).
        age: i32,
        /// `InverseLerp(6, 2, age)`.
        age_fade: f32,
    },
    /// `FlowerMarker`: the karma flower, pulsing gold.
    Flower,
}

/// `Map.FadeInMarker`: a symbol over a dark glow that fades in once the reveal reaches
/// it.
#[derive(Debug, Clone)]
pub struct FadeInMarker {
    pub kind: MarkerKind,
    pub room: i32,
    pub in_room_pos: Vec2,
    pub fade: f32,
    pub last_fade: f32,
    pub fade_in_speed: f32,
    /// In discovery pixels.
    pub fade_in_rad: f32,
    bkg_fade: NodeId,
    /// The death marker's black copy of the bones behind them.
    shade: Option<NodeId>,
    symbol: NodeId,
}

/// The map's state a marker draws with.
pub struct MarkerView {
    pub visible: bool,
    /// `Lerp(map.lastFade, map.fade, t)`.
    pub map_fade: f32,
    /// `map.counter + timeStacker`.
    pub time: f32,
    /// `RoomToMapPos(inRoomPos, room, t)`.
    pub screen_pos: Vec2,
    /// The map's owner is a death screen.
    pub death_screen: bool,
}

impl FadeInMarker {
    /// `GateMarker`: at (480, 240) in the gate room, with the symbol
    /// `smallKarmaNoRing<karma - 1>` (`-1` for an unlocked gate, the value itself for a
    /// lock that is not a number).
    pub fn gate(
        stage: &mut StageTree,
        container: NodeId,
        room: i32,
        karma: Option<GateRequirement>,
        show_as_open: bool,
    ) -> Self {
        let element = match karma {
            None => "smallKarmaNoRing-1".to_owned(),
            Some(k) => match k.karma() {
                Some(n) => format!("smallKarmaNoRing{}", (n - 1).clamp(-1, 4)),
                None => format!("smallKarmaNoRing{}", k.value()),
            },
        };
        Self::new(
            stage,
            container,
            MarkerKind::Gate { show_as_open },
            room,
            Vec2::new(480.0, 240.0),
            &element,
        )
    }

    /// `ShelterMarker`.
    pub fn shelter(stage: &mut StageTree, container: NodeId, room: i32, pos: Vec2) -> Self {
        Self::new(
            stage,
            container,
            MarkerKind::Shelter,
            room,
            pos,
            "ShelterMarker",
        )
    }

    /// `DeathMarker`: at the death's tile, aged `age` cycles.
    pub fn death(stage: &mut StageTree, container: NodeId, room: i32, pos: Vec2, age: i32) -> Self {
        let kind = MarkerKind::Death {
            age,
            age_fade: (age as f32).inverse_lerp(6.0, 2.0),
        };
        let mut marker = Self::glow(stage, container, kind, room, pos, 3.0);
        let mut shade = Sprite::new("Multiplayer_Bones");
        shade.color = StageColor::BLACK;
        shade.transform.visible = false;
        marker.shade = Some(stage.add_sprite(container, shade));
        marker.symbol = Self::add_symbol(stage, container, "Multiplayer_Bones");
        marker
    }

    /// `FlowerMarker`: fades in from 5 discovery pixels away.
    pub fn flower(stage: &mut StageTree, container: NodeId, room: i32, pos: Vec2) -> Self {
        let mut marker = Self::glow(stage, container, MarkerKind::Flower, room, pos, 5.0);
        marker.symbol = Self::add_symbol(stage, container, "FlowerMarker");
        marker
    }

    /// The constructor of a gate or shelter marker: the glow, then the symbol.
    fn new(
        stage: &mut StageTree,
        container: NodeId,
        kind: MarkerKind,
        room: i32,
        in_room_pos: Vec2,
        element: &str,
    ) -> Self {
        let mut marker = Self::glow(stage, container, kind, room, in_room_pos, 3.0);
        marker.symbol = Self::add_symbol(stage, container, element);
        marker
    }

    /// `FadeInMarker`'s constructor: a hidden black `FlatLight` glow in `container`
    /// (the map's `inFrontContainer`); the subclass adds its symbol after it.
    fn glow(
        stage: &mut StageTree,
        container: NodeId,
        kind: MarkerKind,
        room: i32,
        in_room_pos: Vec2,
        fade_in_rad: f32,
    ) -> Self {
        let mut bkg = Sprite::new(AtlasManager::WHITE);
        bkg.shader = StageShader::FlatLight;
        bkg.color = StageColor::BLACK;
        bkg.transform.visible = false;
        let bkg_fade = stage.add_sprite(container, bkg);
        Self {
            kind,
            room,
            in_room_pos,
            fade: 0.0,
            last_fade: 0.0,
            fade_in_speed: 0.0,
            fade_in_rad,
            bkg_fade,
            shade: None,
            symbol: bkg_fade,
        }
    }

    fn add_symbol(stage: &mut StageTree, container: NodeId, element: &str) -> NodeId {
        let mut symbol = Sprite::new(element);
        symbol.transform.visible = false;
        stage.add_sprite(container, symbol)
    }

    pub fn update(&mut self) {
        self.last_fade = self.fade;
        self.fade = (self.fade + self.fade_in_speed).min(1.0);
    }

    /// `FadeIn(fdSpd)`: starts fading in over `ticks` unless already fading.
    pub fn fade_in(&mut self, ticks: f32) {
        if self.fade_in_speed <= 0.0 {
            self.fade_in_speed = 1.0 / ticks;
        }
    }

    /// `SetInvisible`.
    pub fn set_invisible(&mut self) {
        self.fade_in_speed = 0.0;
        self.fade = 0.0;
    }

    pub fn draw(&self, stage: &mut StageTree, view: &MarkerView, t: f32) {
        stage.transform_mut(self.bkg_fade).visible = view.visible;
        stage.transform_mut(self.symbol).visible = view.visible;
        if let Some(shade) = self.shade {
            stage.transform_mut(shade).visible = view.visible;
        }
        if !view.visible {
            return;
        }
        let a = view.map_fade * self.last_fade.lerp(self.fade, t);
        match self.kind {
            MarkerKind::Death { age, age_fade } => {
                self.draw_death(stage, view, a * (0.5 + 0.5 * age_fade), age, age_fade);
                return;
            }
            MarkerKind::Flower => {
                let pulse = 0.5 + 0.5 * (view.time / 7.0).sin();
                let color = StageColor::WHITE.lerp(Self::GOLD, pulse);
                self.draw_symbol(stage, view.screen_pos, a, 10.0, color);
                return;
            }
            MarkerKind::Gate { .. } | MarkerKind::Shelter => {}
        }
        let (scale, base) = match self.kind {
            MarkerKind::Gate { show_as_open } => (
                12.5,
                if show_as_open {
                    MenuColor::DarkGrey.rgb()
                } else {
                    StageColor::rgb(1.0, 0.0, 0.0)
                },
            ),
            _ => (10.0, MenuColor::DarkGrey.rgb()),
        };
        let pulse = 0.5 + 0.5 * (view.time / 14.0).sin();
        let color = base.lerp(MenuColor::White.rgb(), pulse);
        self.draw_symbol(stage, view.screen_pos, a, scale, color);
    }

    /// `RainWorld.GoldRGB`.
    const GOLD: StageColor = StageColor::rgb(0.529, 0.365, 0.184);

    /// The glow at half `alpha` and `scale`, the symbol at `alpha` in `color`.
    fn draw_symbol(
        &self,
        stage: &mut StageTree,
        at: Vec2,
        alpha: f32,
        scale: f32,
        color: StageColor,
    ) {
        let bkg = stage.sprite_mut(self.bkg_fade);
        bkg.transform.x = at.x;
        bkg.transform.y = at.y;
        bkg.transform.alpha = alpha * 0.5;
        bkg.transform.set_scale(scale);
        let symbol = stage.sprite_mut(self.symbol);
        symbol.transform.x = at.x;
        symbol.transform.y = at.y;
        symbol.transform.alpha = alpha;
        symbol.color = color;
    }

    /// `DeathMarker.Draw`: the shade one pixel left and two down; this cycle's death is
    /// red on the death screen, pulsing red elsewhere; older ones fade to dark grey.
    fn draw_death(
        &self,
        stage: &mut StageTree,
        view: &MarkerView,
        a: f32,
        age: i32,
        age_fade: f32,
    ) {
        let red = StageColor::rgb(1.0, 0.0, 0.0);
        let color = if age == 0 {
            if view.death_screen {
                red
            } else {
                let pulse = 0.25 + 0.25 * (view.time / 7.0).sin();
                MenuColor::White.rgb().lerp(red, pulse)
            }
        } else {
            let pulse = 0.25 + 0.25 * (view.time / 14.0).sin();
            MenuColor::MediumGrey
                .rgb()
                .lerp(red, pulse)
                .lerp(MenuColor::DarkGrey.rgb(), 1.0 - age_fade)
        };
        self.draw_symbol(stage, view.screen_pos, a, (40.0 + 10.0 * a) / 8.0, color);
        if let Some(shade) = self.shade {
            let tr = stage.transform_mut(shade);
            tr.x = view.screen_pos.x - 1.0;
            tr.y = view.screen_pos.y - 2.0;
            tr.alpha = a;
        }
    }

    /// The symbol's element, for tests and tools.
    #[must_use]
    pub fn symbol<'a>(&self, stage: &'a StageTree) -> &'a str {
        &stage.sprite(self.symbol).element
    }
}
