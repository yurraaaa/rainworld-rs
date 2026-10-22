//! `CollectToken.TokenStalk`'s sprites without a token (base game, no MSC satellite
//! flasher): base, five arm pieces, elbow joint, socket, head, lamp and one pixel per
//! rope segment, all in `Midground` in the palette's black but the lamp, whose flicker
//! makes one draw a frame on the cosmetic stream (D1).
//!
//! Mirrors: CollectToken.cs (TokenStalk.InitiateSprites, DrawSprites, ApplyPalette, AddToContainer), RainWorld.cs (GoldRGB)

use rw_random::UnityRandom;
use rw_sim::{FloatExt, TokenStalk, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// `RainWorld.GoldRGB`.
const GOLD_RGB: Color = Color::rgb(0.529, 0.365, 0.184);

/// The stalk's sprites.
pub trait TokenStalkSprites {
    /// `DrawSprites` at `t` between ticks in the palette's `black` (`ApplyPalette`);
    /// draws once from `rng`.
    fn draw(&self, black: Color, t: f32, rng: &mut UnityRandom) -> DrawList;
}

/// One sprite of the stalk's leaser.
struct StalkSprite {
    index: usize,
    element: &'static str,
    pos: Vec2,
    rotation: f32,
    scale: Vec2,
    anchor_y: f32,
}

impl StalkSprite {
    fn new(index: usize, element: &'static str, pos: Vec2, rotation: f32, scale: Vec2) -> Self {
        Self {
            index,
            element,
            pos,
            rotation,
            scale,
            anchor_y: 0.5,
        }
    }

    /// `anchorY = 0`: drawn from `pos` along its rotation.
    fn anchored_at_bottom(mut self) -> Self {
        self.anchor_y = 0.0;
        self
    }

    fn item(self, color: Color) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index: self.index,
            element: self.element,
            pos: self.pos,
            rotation: self.rotation,
            scale: self.scale,
            anchor: Vec2::new(0.5, self.anchor_y),
            color,
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }

    /// `Custom.AimFromOneVectorToAnother`.
    fn aim(from: Vec2, to: Vec2) -> f32 {
        (to - from).to_degrees()
    }
}

impl TokenStalkSprites for TokenStalk {
    fn draw(&self, black: Color, t: f32, rng: &mut UnityRandom) -> DrawList {
        let base = self.base_pos;
        let head = self.last_head.lerp(self.head, t);
        let head_dir = self.last_head_dir.slerp(self.head_dir, t);
        let arm = self.last_arm_pos.lerp(self.arm_pos, t);
        let elbow = self.elbow(arm);
        let first = self
            .coord
            .first()
            .map_or(arm, |c| c.last_pos.lerp(c.pos, t));
        let socket = elbow
            .direction_to(arm)
            .slerp(arm.direction_to(first), 0.4)
            .to_degrees();
        let mut p = base.lerp(elbow, 0.3);
        let p2 = elbow.lerp(arm, 0.4);
        let mut sprites = vec![
            StalkSprite::new(
                0,
                "Circle20",
                base,
                self.main_dir.to_degrees(),
                Vec2::new(0.5, 0.7),
            ),
            StalkSprite::new(
                1,
                "pixel",
                base,
                StalkSprite::aim(base, elbow),
                Vec2::new(4.0, base.distance(elbow)),
            )
            .anchored_at_bottom(),
            StalkSprite::new(
                2,
                "pixel",
                elbow,
                StalkSprite::aim(elbow, arm),
                Vec2::new(3.0, elbow.distance(arm)),
            )
            .anchored_at_bottom(),
            StalkSprite::new(
                3,
                "pixel",
                p,
                StalkSprite::aim(p, p2),
                Vec2::new(1.5, self.arm_length * 0.6),
            )
            .anchored_at_bottom(),
            StalkSprite::new(4, "pixel", p2, StalkSprite::aim(p, p2), Vec2::new(3.0, 8.0)),
        ];
        p += base.direction_to(elbow) * (self.arm_length * 0.1 + 2.0);
        sprites.push(StalkSprite::new(
            5,
            "pixel",
            p,
            StalkSprite::aim(base, elbow),
            Vec2::new(6.0, 8.0),
        ));
        sprites.push(StalkSprite::new(
            6,
            "JetFishEyeA",
            elbow,
            0.0,
            Vec2::new(1.0, 1.0),
        ));
        sprites.push(StalkSprite::new(
            7,
            "pixel",
            arm,
            socket,
            Vec2::new(5.0, 9.0),
        ));
        sprites.push(StalkSprite::new(
            8,
            "pixel",
            head,
            head_dir.to_degrees(),
            Vec2::new(4.0, 6.0),
        ));
        let lamp_pos = p;
        let mut prev = arm;
        for (i, c) in self.coord.iter().enumerate() {
            let at = c.last_pos.lerp(c.pos, t);
            let width = if i % 2 == 0 { 2.0 } else { 3.0 };
            sprites.push(StalkSprite::new(
                10 + i,
                "pixel",
                at,
                StalkSprite::aim(prev, at),
                Vec2::new(width, 5.0),
            ));
            prev = at;
        }
        let lamp_off = black.lerp(Color::WHITE, 0.15);
        let lamp_on = GOLD_RGB.lerp(Color::WHITE, 0.5);
        let power = self.last_lamp_power.lerp(self.lamp_power, t);
        let sin = (f64::from(self.last_sin_counter.lerp(self.sin_counter, t) / 6.0).sin()) as f32;
        let flicker = power * rng.value().mathf_pow(0.5) * (0.5 + 0.5 * sin);
        let lamp = StalkSprite::new(9, "tinyStar", lamp_pos, 0.0, Vec2::new(1.0, 1.0))
            .item(lamp_off.lerp(lamp_on, flicker));
        let mut items: Vec<DrawItem> = sprites.into_iter().map(|s| s.item(black)).collect();
        items.insert(9, lamp);
        DrawList { items }
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;
    use rw_sim::{TokenStalk, Vec2};

    use super::TokenStalkSprites;
    use crate::draw::{Color, DrawItem};

    /// Ten sprites plus one per rope segment, in leaser order, all black but the lamp;
    /// with the lamp off (`lampPower` 0) the lamp is `Lerp(black, white, 0.15)`. One
    /// cosmetic draw a frame (`Pow(Random.value, 0.5)`), drawn even with the lamp off.
    #[test]
    fn dead_stalk_sprites_and_lamp_draw() {
        let s = TokenStalk::new(
            0,
            Vec2::new(200.0, 250.0),
            Vec2::new(200.0, 200.0),
            &mut UnityRandom::new(1),
        );
        let black = Color::rgb(0.1, 0.05, 0.0);
        let mut rng = UnityRandom::new(4);
        let list = s.draw(black, 0.5, &mut rng);
        let mut one = UnityRandom::new(4);
        one.value();
        assert_eq!(rng.state(), one.state());
        assert_eq!(list.items.len(), 10 + s.coord.len());
        for (i, item) in list.items.iter().enumerate() {
            let DrawItem::Sprite(sprite) = item else {
                panic!("sprite {i}");
            };
            assert_eq!(sprite.index, i);
            let expected = if i == 9 {
                black.lerp(Color::WHITE, 0.15)
            } else {
                black
            };
            assert_eq!(sprite.color, expected, "sprite {i}");
        }
    }
}
