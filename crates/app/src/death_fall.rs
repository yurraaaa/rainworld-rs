//! The camera's room's `DeathFallGraphic`: whether the room has one (an open bottom),
//! its height, which `RoomCamera.DrawUpdate` reads for `_waterLevel`, and its sprite
//! (the `DeathFall` shader in `Bloom`).
//!
//! Mirrors: DeathFallGraphic.cs (RoomRequiresDeathFall, InitiateSprites, Update, DrawSprites, AddToContainer, ApplyPalette), Room.cs (Loaded: DeathFallFocus)

use rw_assets::{PlacedObject, PlacedObjectType, Room};
use rw_graphics::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use rw_render::SpriteAtlas;
use rw_sim::{CameraView, Vec2, Water};

/// A room's `DeathFallGraphic`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeathFall {
    /// The lowest camera's bottom (`defaultHeight`, set in `InitiateSprites`).
    default_height: f32,
    /// `room.deathFallFocalPoints`: the active `DeathFallFocus` objects' positions.
    focal_points: Vec<Vec2>,
    pub height: f32,
    /// Ticks updated (`Update`), the phase of the sprite's alpha pulse.
    pub counter: i32,
}

impl DeathFall {
    /// `Room.Loaded`'s `DeathFallGraphic`, when `RoomRequiresDeathFall`: not in an
    /// unbroken shelter, and some bottom-row tile open (no water) or under water below
    /// -1200 px. The `RoomWrap` (DLC) and terrain-curve checks are off in the base game.
    /// The height is `InitiateSprites`' (made on each camera entry): the lowest camera's
    /// bottom, focal points ignored until the next `Update`.
    #[must_use]
    pub fn new(
        room: &Room,
        water: Option<&Water>,
        unbroken_shelter: bool,
        placed: &[PlacedObject],
    ) -> Option<Self> {
        if unbroken_shelter {
            return None;
        }
        let open = (0..room.width()).any(|x| match water {
            None => !room.tile(x, 0).is_solid(),
            Some(w) => w.level_at(x as f32 * 20.0 + 10.0) < -1200.0,
        });
        if !open {
            return None;
        }
        let default_height = room.cameras().iter().map(|c| c[1]).fold(f32::MAX, f32::min);
        Some(Self {
            default_height,
            focal_points: placed
                .iter()
                .filter(|o| o.active && o.kind == PlacedObjectType::DeathFallFocus.value())
                .map(|o| Vec2::from(o.pos))
                .collect(),
            height: default_height,
            counter: 0,
        })
    }

    /// `Update`: one more tick of the counter; the height is the default, or the bottom
    /// of the first camera of the room (`CameraViewingPoint`, 1366x768 rects) holding
    /// the first focal point camera 0 shows.
    pub fn update(&mut self, room: &Room, index: i32, view: &CameraView) {
        self.counter += 1;
        self.height = self.default_height;
        for &p in &self.focal_points {
            if view.viewed(index, room, p, 0.0)
                && let Some(cam) = Self::camera_viewing_point(room, p)
            {
                self.height = cam[1];
                break;
            }
        }
    }

    /// `DrawSprites`' alpha: `0.85 + 0.15 sin((counter + timeStacker) / 25)`.
    #[must_use]
    pub fn alpha(&self, time_stacker: f32) -> f32 {
        0.85 + 0.15 * ((self.counter as f32 + time_stacker) / 25.0).sin()
    }

    /// The sprite (`InitiateSprites`, `DrawSprites`): `Futile_White` scaled to
    /// `sSize.x + 20` by 180 px, anchored at its bottom centre on the screen's middle
    /// column at `height`, white at [`Self::alpha`], with the `DeathFall` shader in
    /// `Bloom`. The game takes the room's `RainMask_` element instead when the rain has
    /// made one (with the rain); the shader reads only the quad's uv, 0..1 either way.
    /// `DeathFallHeavy` is Remix's (`cfgClearerDeathGradients`). `ApplyPalette` is
    /// empty.
    #[must_use]
    pub fn draw_list(&self, screen_width: f32, cam_pos: Vec2, time_stacker: f32) -> DrawList {
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: SpriteAtlas::FUTILE_WHITE,
                pos: Vec2::new(cam_pos.x + screen_width / 2.0, self.height),
                rotation: 0.0,
                scale: Vec2::new((screen_width + 20.0) / 16.0, 180.0 / 16.0),
                anchor: Vec2::new(0.5, 0.0),
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: self.alpha(time_stacker).clamp(0.0, 1.0),
                },
                shader: Shader::DeathFall,
                layer: Layer::Bloom,
            })],
        }
    }

    /// `Room.CameraViewingPoint`.
    fn camera_viewing_point(room: &Room, p: Vec2) -> Option<[f32; 2]> {
        room.cameras()
            .iter()
            .copied()
            .find(|c| p.x > c[0] && p.x < c[0] + 1366.0 && p.y > c[1] && p.y < c[1] + 768.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 20 x 12 room of air, its bottom row solid if asked, with cameras at file
    /// positions (0, 0) and (0, 300): room y -560 and -860.
    fn room(solid_bottom: bool) -> Room {
        let (w, h) = (20, 12);
        let column = format!(
            "{}{}",
            "0|".repeat(h - 1),
            if solid_bottom { "1|" } else { "0|" }
        );
        let text = format!(
            "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0|0,300\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{}\n",
            column.repeat(w)
        );
        Room::parse(&text).expect("test room parses")
    }

    fn focus(pos: [f32; 2]) -> PlacedObject {
        PlacedObject {
            kind: PlacedObjectType::DeathFallFocus.value().to_owned(),
            pos,
            data: String::new(),
            active: true,
        }
    }

    /// `RoomRequiresDeathFall`: an open bottom tile and not an unbroken shelter; the
    /// height starts at the lowest camera's bottom.
    #[test]
    fn open_bottom_rooms_have_a_death_fall() {
        let fall = DeathFall::new(&room(false), None, false, &[]).expect("open bottom");
        assert_eq!(fall.height, -860.0);
        assert!(DeathFall::new(&room(true), None, false, &[]).is_none());
        assert!(DeathFall::new(&room(false), None, true, &[]).is_none());
    }

    /// `Update`: a focal point the camera shows sets the height to the bottom of the
    /// first camera rect (1366x768) holding it; out of view, the default. Before the first
    /// `Update` the height is `InitiateSprites`' default, whatever the focal points.
    #[test]
    fn a_viewed_focal_point_moves_the_height() {
        let r = room(false);
        let mut fall = DeathFall::new(&r, None, false, &[focus([100.0, 0.0])]).unwrap();
        let view = CameraView {
            room: 3,
            current: 0,
            next: 0,
        };
        assert_eq!(
            fall.height, -860.0,
            "InitiateSprites: the default until Update"
        );
        assert_eq!(fall.counter, 0);
        fall.update(&r, 3, &view);
        assert_eq!(fall.height, -560.0);
        fall.update(&r, 4, &view);
        assert_eq!(fall.height, -860.0, "camera in another room");
    }

    /// `DrawSprites`: a 1386 x 180 quad (`(sSize.x + 20) / 16`, `180 / 16` of the 16 px
    /// `Futile_White`) anchored at its bottom centre, x the screen's middle (stage
    /// `sSize.x / 2`, room `camPos.x + 683`), y the height (stage `height - camPos.y`);
    /// alpha `0.85 + 0.15 sin((counter + timeStacker) / 25)`.
    #[test]
    fn the_sprite_spans_the_screen_bottom_up_from_the_height() {
        let r = room(false);
        let mut fall = DeathFall::new(&r, None, false, &[]).unwrap();
        let view = CameraView {
            room: 3,
            current: 0,
            next: 0,
        };
        for _ in 0..30 {
            fall.update(&r, 3, &view);
        }
        let list = fall.draw_list(1366.0, Vec2::new(-40.0, -900.0), 0.5);
        let [DrawItem::Sprite(s)] = list.items.as_slice() else {
            panic!("one sprite: {list:?}");
        };
        assert_eq!(s.element, SpriteAtlas::FUTILE_WHITE);
        assert_eq!(s.pos, Vec2::new(643.0, -860.0));
        assert_eq!(s.scale, Vec2::new(1386.0 / 16.0, 180.0 / 16.0));
        assert_eq!(s.anchor, Vec2::new(0.5, 0.0));
        assert_eq!((s.shader, s.layer), (Shader::DeathFall, Layer::Bloom));
        let a = 0.85 + 0.15 * (30.5f32 / 25.0).sin();
        assert!((s.color.a - a).abs() < 1e-6, "{} vs {a}", s.color.a);
        assert_eq!([s.color.r, s.color.g, s.color.b], [1.0; 3]);
    }
}
