//! `Menu.EndgameTokens` (531650-531908): a token for each fulfilled, unused passage at
//! the screen's bottom left, sliding into a row as their meters show them, and when the
//! PASSAGE button is due. `Passage` (the token's activation and the black fade before
//! the custom end-game screen) is not ported: the button is disabled (D13).

use rw_menu::{AtlasManager, MenuColor, NodeId, Sprite, StageTree};
use rw_random::UnityRandom;
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

use crate::EndgameMeter;

/// `EndgameTokens.Token`.
#[derive(Debug, Clone)]
pub struct Token {
    /// The meter's index on the ladder.
    pub meter: usize,
    pub index: usize,
    /// Relative to the tokens' position.
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub fade: f32,
    pub last_fade: f32,
    get_to: Vec2,
    super_glow: f32,
    sprites: [NodeId; 3],
}

/// The tokens.
#[derive(Debug, Clone)]
pub struct EndgameTokens {
    /// Relative to the page.
    pub pos: Vec2,
    pub tokens: Vec<Token>,
    /// `forceShowTokenAdd`: a passage was fulfilled on this screen.
    pub force_show_token_add: bool,
    add_passage_button_when_token_becomes_visible: bool,
}

impl Token {
    /// `WantToBeVisible`.
    fn wants_to_be_visible(&self, meters: &[EndgameMeter]) -> bool {
        meters[self.meter].show_as_fulfilled > 0.0
    }

    /// `InPosition`.
    #[must_use]
    pub fn in_position(&self) -> bool {
        (self.pos.x - self.get_to.x).abs() < 5.0 && (self.pos.y - self.get_to.y).abs() < 5.0
    }

    /// The row slot of the `visible_before`-th visible token.
    fn slot(visible_before: usize) -> Vec2 {
        Vec2::new(
            20.0 + 40.0 * (visible_before % 5) as f32,
            15.0 + 40.0 * (visible_before as f32 / 5.0).floor(),
        )
    }
}

impl EndgameTokens {
    /// The constructor: a token for each fulfilled meter whose passage is unused, in
    /// `container` (the menu's). Returns whether the PASSAGE button is added at once
    /// (a passage fulfilled before this screen).
    pub fn new(
        stage: &mut StageTree,
        container: NodeId,
        pos: Vec2,
        meters: &[EndgameMeter],
    ) -> (Self, bool) {
        let mut this = Self {
            pos,
            tokens: Vec::new(),
            force_show_token_add: false,
            add_passage_button_when_token_becomes_visible: false,
        };
        let mut already = false;
        for (m, meter) in meters.iter().enumerate() {
            if meter.fulfilled_now {
                this.add_passage_button_when_token_becomes_visible = true;
            }
            if meter.tracker.goal_fulfilled() && !meter.tracker.consumed {
                already |= meter.tracker.goal_already_fulfilled();
                let index = this.tokens.len();
                let visible = this.visible_before(index, meters);
                let sprites = [
                    Sprite::new(&format!("{}A", meter.tracker.id.value())),
                    Sprite::new("EndGameCircle"),
                    Sprite {
                        shader: StageShader::FlatLight,
                        ..Sprite::new(AtlasManager::WHITE)
                    },
                ]
                .map(|s| stage.add_sprite(container, s));
                let at = Token::slot(visible);
                this.tokens.push(Token {
                    meter: m,
                    index,
                    pos: at,
                    // The constructor sets `pos` after `lastPos` took the default.
                    last_pos: Vec2::ZERO,
                    fade: 0.0,
                    last_fade: 0.0,
                    get_to: Vec2::ZERO,
                    super_glow: 0.0,
                    sprites,
                });
            }
            if meter.fulfilled_now {
                this.force_show_token_add = true;
            }
        }
        if already {
            this.add_passage_button_when_token_becomes_visible = false;
        }
        (this, already)
    }

    fn visible_before(&self, index: usize, meters: &[EndgameMeter]) -> usize {
        self.tokens[..index]
            .iter()
            .filter(|t| t.wants_to_be_visible(meters))
            .count()
    }

    /// `AllAnimationsDone`.
    #[must_use]
    pub fn all_animations_done(&self) -> bool {
        self.tokens.iter().all(|t| t.fade >= 0.9 && t.in_position())
    }

    /// `Update`: the tokens slide to their slots and fade in as their meters show them
    /// fulfilled; true when the PASSAGE button is to be added (black) now.
    pub fn update(&mut self, meters: &[EndgameMeter]) -> bool {
        let count = self.tokens.len();
        for i in 0..count {
            let visible = self.visible_before(i, meters);
            let next_in_position = self.tokens.get(i + 1).is_some_and(Token::in_position);
            let t = &mut self.tokens[i];
            t.last_pos = t.pos;
            t.last_fade = t.fade;
            t.get_to = Token::slot(visible);
            t.pos.x = t.pos.x.lerp_and_tick(t.get_to.x, 0.05, 1.0 / 30.0);
            t.pos.y = t.pos.y.lerp_and_tick(t.get_to.y, 0.05, 1.0 / 30.0);
            let meter = &meters[t.meter];
            if meter.tracker.goal_already_fulfilled() {
                t.fade = t.fade.lerp_and_tick(1.0, 0.05, 1.0 / 30.0);
            } else if i == count - 1 || next_in_position {
                t.fade = t
                    .fade
                    .lerp_and_tick(meter.show_as_fulfilled, 0.05, 1.0 / 30.0);
            }
        }
        if !self.add_passage_button_when_token_becomes_visible {
            return false;
        }
        let shown = self.tokens.iter().all(|t| {
            let m = &meters[t.meter];
            !(m.fulfilled_now && m.show_as_fulfilled < 0.9)
        });
        if shown {
            self.add_passage_button_when_token_becomes_visible = false;
        }
        shown
    }

    /// `GrafUpdate` of each token; `page` is the page's draw position. The glow's size
    /// makes one draw per token and frame (from `cosmetic`, D1).
    pub fn draw(
        &self,
        stage: &mut StageTree,
        page: Vec2,
        t: f32,
        meters: &[EndgameMeter],
        cosmetic: &mut UnityRandom,
    ) {
        for token in &self.tokens {
            let at = page + self.pos + token.last_pos.lerp(token.pos, t);
            let fade = token.last_fade.lerp(token.fade, t);
            let now = if meters[token.meter].fulfilled_now {
                1.0
            } else {
                0.0
            };
            let glow = now.lerp(1.0, token.super_glow);
            let color = MenuColor::DarkGrey.rgb().lerp(
                MenuColor::MediumGrey
                    .rgb()
                    .lerp(MenuColor::White.rgb(), token.super_glow),
                glow,
            );
            let [symbol, circle, light] = token.sprites;
            for s in token.sprites {
                let s = stage.sprite_mut(s);
                s.transform.x = at.x;
                s.transform.y = at.y;
                s.color = color;
            }
            stage.sprite_mut(symbol).transform.alpha = fade;
            stage.sprite_mut(circle).transform.alpha = fade;
            let s = stage.sprite_mut(light);
            s.transform.set_scale(
                3.0_f32.lerp(5.0 + glow, fade)
                    + token.super_glow * 0.75_f32.lerp(1.0, cosmetic.value()),
            );
            s.transform.alpha = 0.0_f32.lerp(0.3, glow) * fade;
        }
    }
}

#[cfg(test)]
mod tests;
