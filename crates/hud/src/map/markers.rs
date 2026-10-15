//! The map's `FadeInMarker`s that a base-game story map has: gates and shelters.

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

    /// The constructor: a hidden black `FlatLight` glow, then the hidden symbol, both
    /// in `container` (the map's `inFrontContainer`). Fade-in radius 3 for both kinds.
    fn new(
        stage: &mut StageTree,
        container: NodeId,
        kind: MarkerKind,
        room: i32,
        in_room_pos: Vec2,
        element: &str,
    ) -> Self {
        let mut bkg = Sprite::new(AtlasManager::WHITE);
        bkg.shader = StageShader::FlatLight;
        bkg.color = StageColor::BLACK;
        bkg.transform.visible = false;
        let bkg_fade = stage.add_sprite(container, bkg);
        let mut symbol = Sprite::new(element);
        symbol.transform.visible = false;
        let symbol = stage.add_sprite(container, symbol);
        Self {
            kind,
            room,
            in_room_pos,
            fade: 0.0,
            last_fade: 0.0,
            fade_in_speed: 0.0,
            fade_in_rad: 3.0,
            bkg_fade,
            symbol,
        }
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
        if !view.visible {
            return;
        }
        let a = view.map_fade * self.last_fade.lerp(self.fade, t);
        let (scale, base) = match self.kind {
            MarkerKind::Gate { show_as_open } => (
                12.5,
                if show_as_open {
                    MenuColor::DarkGrey.rgb()
                } else {
                    StageColor::rgb(1.0, 0.0, 0.0)
                },
            ),
            MarkerKind::Shelter => (10.0, MenuColor::DarkGrey.rgb()),
        };
        let pulse = 0.5 + 0.5 * (view.time / 14.0).sin();
        let bkg = stage.sprite_mut(self.bkg_fade);
        bkg.transform.x = view.screen_pos.x;
        bkg.transform.y = view.screen_pos.y;
        bkg.transform.alpha = a * 0.5;
        bkg.transform.set_scale(scale);
        let symbol = stage.sprite_mut(self.symbol);
        symbol.transform.x = view.screen_pos.x;
        symbol.transform.y = view.screen_pos.y;
        symbol.transform.alpha = a;
        symbol.color = base.lerp(MenuColor::White.rgb(), pulse);
    }

    /// The symbol's element, for tests and tools.
    #[must_use]
    pub fn symbol<'a>(&self, stage: &'a StageTree) -> &'a str {
        &stage.sprite(self.symbol).element
    }
}
