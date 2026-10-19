//! `HUD.KarmaMeter`: the karma symbol in the bottom-left corner, its glow, the
//! reinforced ring and when it shows. The scavenger-karma flash (Artificer), the void
//! sea and gate blinking, ripple symbols (Watcher) and the Saint's echo fade are DLC or
//! not ported (`rainworld-re/notes/hud.md`).
//!
//! Mirrors: HUD/KarmaMeter.cs

use std::f32::consts::PI;

use rw_menu::{AtlasManager, NodeId, Sprite, StageColor, StageTree};
use rw_render::StageShader;
use rw_sim::{FloatExt, SoundId, Vec2};

use crate::{CustomFloat, CustomInt, HudFx};

/// `KarmaMeter.KarmaSymbolSprite(small, (karma, cap))`: `karma0`..`karma4`, or for
/// karma 5 and up `karma<k>-<cap>`; `small` prefixes `small`.
#[must_use]
pub fn karma_symbol_sprite(small: bool, karma: i32, cap: i32) -> String {
    let prefix = if small { "smallKarma" } else { "karma" };
    if karma < 5 {
        // More Slugcats lets the small symbol go down to -1.
        format!("{prefix}{}", karma.int_clamp(0, 4))
    } else {
        format!(
            "{prefix}{}-{}",
            karma.int_clamp(5, 9),
            cap.int_clamp(karma, 9)
        )
    }
}

/// `HUD.KarmaMeter`.
#[derive(Debug, Clone)]
pub struct KarmaMeter {
    pub karma_sprite: NodeId,
    pub dark_fade: NodeId,
    pub ring_sprite: Option<NodeId>,
    pub vector_ring_sprite: Option<NodeId>,
    pub glow_sprite: NodeId,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub force_visible_counter: i32,
    pub fade: f32,
    pub last_fade: f32,
    pub rad: f32,
    pub last_rad: f32,
    /// `displayKarma`: (karma, cap), as given (the constructor clamps only the copy
    /// it picks the symbol with).
    pub display_karma: (i32, i32),
    /// The symbol's atlas element.
    pub symbol: String,
    pub show_as_reinforced: bool,
    pub reinforce_animation: i32,
    pub glowy_fac: f32,
    pub last_glowy_fac: f32,
    pub reinforcement_cycle: f32,
    pub last_reinforcement_cycle: f32,
    pub blink_red: bool,
    pub blink_red_counter: i32,
    base_color: StageColor,
    pub timer: i32,
}

/// What the karma meter reads of the rest of the HUD during its update.
#[derive(Debug, Clone, Copy)]
pub struct KarmaNeighbours {
    pub food_down_in_corner: f32,
    pub food_fade: f32,
    /// `hud.map.fade` (the map's fade is kept without the map).
    pub map_fade: f32,
    /// `textPrompt.foodVisibleMode == 0`, and then `LowerBorderHeight(1)`.
    pub text_food_visible_mode_zero: bool,
    pub text_lower_border: f32,
    pub reveal_map: bool,
    pub show_karma_food_rain: bool,
    pub hide_general_hud: bool,
    /// The owner is a slugcat select page (`OwnerType.CharacterSelect`): always shown.
    pub character_select: bool,
    /// The save state's karma, cap and `reinforcedKarma`, which the reinforcement
    /// animation shows at its end.
    pub save_karma: (i32, i32),
    pub save_reinforced: bool,
}

impl KarmaMeter {
    /// The resting position (`SafeScreenOffset` is zero).
    pub const REST: Vec2 = Vec2::new(55.01, 45.01);

    /// The constructor: dark fade, symbol and glow added to `container`.
    pub fn new(
        stage: &mut StageTree,
        container: NodeId,
        karma: i32,
        cap: i32,
        reinforced: bool,
    ) -> Self {
        let symbol = karma_symbol_sprite(true, karma.int_clamp(0, cap), cap);
        let mut dark = Sprite::new(AtlasManager::WHITE);
        dark.shader = StageShader::FlatLight;
        dark.color = StageColor::BLACK;
        let dark_fade = stage.add_sprite(container, dark);
        let base_color = StageColor::WHITE;
        let mut sprite = Sprite::new(&symbol);
        sprite.color = base_color;
        let karma_sprite = stage.add_sprite(container, sprite);
        let mut glow = Sprite::new(AtlasManager::WHITE);
        glow.shader = StageShader::FlatLight;
        glow.color = base_color;
        let glow_sprite = stage.add_sprite(container, glow);
        Self {
            karma_sprite,
            dark_fade,
            ring_sprite: None,
            vector_ring_sprite: None,
            glow_sprite,
            pos: Self::REST,
            last_pos: Self::REST,
            force_visible_counter: 0,
            fade: 0.0,
            last_fade: 0.0,
            rad: 22.5,
            last_rad: 22.5,
            display_karma: (karma, cap),
            symbol,
            show_as_reinforced: reinforced,
            reinforce_animation: -1,
            glowy_fac: 0.0,
            last_glowy_fac: 0.0,
            reinforcement_cycle: 0.0,
            last_reinforcement_cycle: 0.0,
            blink_red: false,
            blink_red_counter: 0,
            base_color,
            timer: 0,
        }
    }

    /// `Radius` for a meter of radius `rad`, with the food meter's `forceSleep`.
    #[must_use]
    pub fn radius_of(rad: f32, reinforced: bool, force_sleep: f32) -> f32 {
        let ring = if reinforced {
            8.0 * (1.0 - force_sleep.inverse_lerp(0.2, 0.4))
        } else {
            0.0
        };
        rad + ring
    }

    /// `Show`: the map button, the meters' forced display, or blinking red.
    #[must_use]
    pub fn show(&self, reveal_map: bool, show_karma_food_rain: bool) -> bool {
        reveal_map || show_karma_food_rain || self.blink_red
    }

    /// `AnyVisibility`.
    #[must_use]
    pub fn any_visibility(&self, reveal_map: bool, show_karma_food_rain: bool) -> bool {
        self.show(reveal_map, show_karma_food_rain)
            || self.fade > 0.0
            || self.reinforce_animation > 0
            || self.force_visible_counter > 0
    }

    #[must_use]
    pub fn draw_pos(&self, t: f32) -> Vec2 {
        self.last_pos.lerp(self.pos, t)
    }

    /// `UpdateGraphic(karma, cap)`.
    pub fn update_graphic(&mut self, karma: i32, cap: i32) {
        self.display_karma = (karma, cap);
        self.symbol = karma_symbol_sprite(true, karma, cap);
    }

    pub fn update(&mut self, n: &KarmaNeighbours, fx: &mut HudFx<'_>) {
        self.last_pos = self.pos;
        self.last_fade = self.fade;
        self.last_rad = self.rad;
        self.last_glowy_fac = self.glowy_fac;
        if self.blink_red_counter > 0 {
            self.blink_red_counter -= 1;
        }
        self.pos = Self::REST;
        if n.text_food_visible_mode_zero {
            self.pos.y += n.text_lower_border;
        }
        if self.fade > 0.0 {
            let to = self.fade * if self.show_as_reinforced { 1.0 } else { 0.9 };
            self.glowy_fac = self.glowy_fac.lerp_and_tick(to, 0.1, 1.0 / 30.0);
            self.timer += 1;
        } else {
            self.glowy_fac = 0.0;
        }
        self.last_reinforcement_cycle = self.reinforcement_cycle;
        self.reinforcement_cycle += 1.0 / 90.0;
        if n.food_down_in_corner > 0.0 {
            self.fade = (self.fade - 0.05).max(0.0);
        } else if n.character_select || self.show(n.reveal_map, n.show_karma_food_rain) {
            let forced: f32 = if self.force_visible_counter > 0 {
                1.0
            } else {
                0.0
            };
            let mut target = forced.max(0.25 + 0.75 * n.map_fade);
            if n.show_karma_food_rain {
                target = 1.0;
            }
            self.fade = if self.fade < target {
                (self.fade + 0.1).min(target)
            } else {
                (self.fade - 0.1).max(target)
            };
        } else {
            if self.force_visible_counter > 0 {
                self.force_visible_counter -= 1;
                self.fade = (self.fade + 0.1).min(1.0);
            } else {
                self.fade = (self.fade - 0.0125).max(0.0);
            }
            if n.food_fade > 0.0 && self.fade > 0.0 {
                self.fade = (self.fade + 0.1).min(n.food_fade);
            }
        }
        // Base game: no scavenger flash, void sea or region gate blinking.
        self.blink_red = self.blink_red_counter > 0;
        if n.hide_general_hud {
            self.fade = 0.0;
        }
        let rest = self.fade.lerp_map_pow(0.0, 0.15, 17.0, 22.5, 1.3);
        self.rad = self.rad.lerp_and_tick(rest, 0.12, 0.1);
        if self.blink_red && (16..20).contains(&(self.timer % 30)) {
            self.rad *= 0.98;
        }
        if self.reinforce_animation > -1 {
            self.update_reinforce_animation(n, fx);
        }
    }

    /// The colour the symbol, ring and glow take this tick.
    #[must_use]
    pub fn color(&self) -> StageColor {
        if self.blink_red && self.timer % 30 > 15 {
            StageColor::rgb(1.0, 0.0, 0.0)
        } else {
            self.base_color
        }
    }

    /// The reinforcement animation, from `reinforceAnimation` 0 (set when karma is
    /// reinforced; not reached in the port yet).
    fn update_reinforce_animation(&mut self, n: &KarmaNeighbours, fx: &mut HudFx<'_>) {
        self.rad = self.fade.lerp_map_pow(0.0, 0.15, 17.0, 22.5, 1.3);
        self.force_visible_counter = self.force_visible_counter.max(200);
        self.reinforce_animation += 1;
        let a = self.reinforce_animation;
        let af = a as f32;
        if a == 20 {
            fx.play_sound(SoundId::HudKarmaReinforceFlicker);
        }
        if a > 20 && a < 100 {
            let jitter = (-1.0_f32).lerp(1.0, fx.rng.value());
            self.glowy_fac = 1.0 + jitter * 0.03 * af.inverse_lerp(20.0, 100.0);
        } else if a == 104 {
            fx.add_fade_circle(self.rad, 11.0, 0.82, 50.0, 4.0, self.pos);
            fx.play_sound(SoundId::HudKarmaReinforceSmallCircle);
            fx.play_sound(SoundId::HudKarmaReinforceContract);
        } else if a > 104 && a < 130 {
            let s = (af.inverse_lerp(104.0, 130.0) * PI).sin().mathf_pow(0.5);
            self.rad -= s * 2.0;
            self.fade = 1.0 - s * 0.5;
        } else if a > 130 {
            self.fade = 1.0;
            self.rad += (af.inverse_lerp(130.0, 140.0).mathf_pow(0.2) * PI).sin() * 5.0;
            if a == 134 {
                self.glowy_fac = 1.7;
            } else if a == 135 {
                self.update_graphic(n.save_karma.0, n.save_karma.1);
                self.show_as_reinforced = n.save_reinforced;
                fx.add_fade_circle(self.rad, 16.0, 0.92, 100.0, 8.0, self.pos);
                fx.play_sound(SoundId::HudKarmaReinforceBump);
                self.reinforce_animation = -1;
            }
        }
    }

    pub fn draw(&mut self, stage: &mut StageTree, container: NodeId, force_sleep: f32, t: f32) {
        let mut fade = self.last_fade.lerp(self.fade, t);
        if self.blink_red {
            fade *= 0.65 + 0.35 * ((self.timer as f32 + t) / 30.0 * PI * 2.0).sin();
        }
        let pos = self.draw_pos(t);
        let rad = self.last_rad.lerp(self.rad, t);
        let color = self.color();
        let sprite = stage.sprite_mut(self.karma_sprite);
        self.symbol.clone_into(&mut sprite.element);
        sprite.color = color;
        sprite.transform.x = pos.x;
        sprite.transform.y = pos.y;
        sprite.transform.set_scale(rad / 22.5);
        sprite.transform.alpha = fade;
        if self.show_as_reinforced {
            self.draw_reinforced_ring(stage, container, pos, fade, force_sleep, t);
        } else {
            if let Some(ring) = self.ring_sprite.take() {
                stage.remove_from_container(ring);
            }
            if let Some(ring) = self.vector_ring_sprite.take() {
                stage.remove_from_container(ring);
            }
        }
        let dark = stage.sprite_mut(self.dark_fade);
        dark.transform.x = pos.x;
        dark.transform.y = pos.y;
        dark.transform.scale_x = 18.75;
        dark.transform.scale_y = 18.75;
        dark.transform.alpha = 0.2 * fade.mathf_pow(2.0);
        let cycle = self
            .last_reinforcement_cycle
            .lerp(self.reinforcement_cycle, t);
        let glowy = self.last_glowy_fac.lerp(self.glowy_fac, t);
        let pulse = (0.7 + 0.3 * (PI * 2.0 * cycle).sin()) * glowy.inverse_lerp(0.9, 1.0);
        let glow = stage.sprite_mut(self.glow_sprite);
        glow.color = color;
        glow.transform.x = pos.x;
        glow.transform.y = pos.y;
        glow.transform
            .set_scale((60.0 + 10.0 * pulse) * glowy / 8.0);
        glow.transform.alpha = (0.2 + 0.2 * pulse) * glowy * fade.mathf_pow(2.0);
    }

    /// The reinforced ring, made on the first frame it shows (added on top of
    /// `container`), and the vector ring that opens while force-sleeping.
    fn draw_reinforced_ring(
        &mut self,
        stage: &mut StageTree,
        container: NodeId,
        pos: Vec2,
        fade: f32,
        force_sleep: f32,
        t: f32,
    ) {
        let color = self.color();
        let ring = *self.ring_sprite.get_or_insert_with(|| {
            let mut s = Sprite::new("smallKarmaRingReinforced");
            s.color = color;
            stage.add_sprite(container, s)
        });
        let sleep = force_sleep.inverse_lerp(0.1, 0.85);
        let sprite = stage.sprite_mut(ring);
        sprite.color = color;
        sprite.transform.x = pos.x;
        sprite.transform.y = pos.y;
        sprite
            .transform
            .set_scale(self.last_rad.lerp(self.rad, t) / 22.5);
        sprite.transform.alpha = fade * sleep.inverse_lerp(0.2, 0.0);
        if sleep > 0.0 {
            let vector = *self.vector_ring_sprite.get_or_insert_with(|| {
                let mut s = Sprite::new(AtlasManager::WHITE);
                s.shader = StageShader::VectorCircleFadable;
                stage.add_sprite(container, s)
            });
            // The game lerps by `timer`, not the time stacker.
            let rad = self.last_rad.lerp(self.rad, self.timer as f32)
                + 8.0
                + 100.0 * sleep.inverse_lerp(0.2, 1.0).s_curve(0.75);
            let width = 2.0 * sleep.inverse_lerp(0.4, 0.2).mathf_pow(2.0)
                + 2.0 * sleep.inverse_lerp(1.0, 0.2).mathf_pow(0.5);
            let s = stage.sprite_mut(vector);
            s.transform.visible = true;
            s.transform.x = pos.x;
            s.transform.y = pos.y;
            s.transform.set_scale(rad / 8.0);
            s.color = StageColor {
                r: 0.0,
                g: 1.0,
                b: fade * sleep.inverse_lerp(1.0, 0.2).mathf_pow(3.0),
                a: width / rad,
            };
        } else if let Some(vector) = self.vector_ring_sprite.take() {
            stage.remove_from_container(vector);
        }
    }
}
