//! `RainEffect`: the menus' falling rain, lightning and the edge darkening over them
//! (the intro roll's and, outside flat mode, the slugcat select menu's).
//!
//! Mirrors: Menu/RainEffect.cs

use rw_graphics::CustomVec;
use rw_random::UnityRandom;
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

use super::Menu;
use super::object::{Kind, MenuObject, ObjectId};
use crate::stage::{Sprite, StageColor};
use crate::tree::{NodeId, StageTree};

/// `RainEffect.RainDrop`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainDrop {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub last_last_pos: Vec2,
    pub depth: f32,
}

/// `RainEffect`'s state and sprites.
#[derive(Debug, Clone)]
pub struct RainEffectData {
    pub drops: Vec<RainDrop>,
    /// `rainDirs[j, 0]` and `rainDirs[j, 1]`.
    pub rain_dirs: [[Vec2; 2]; 2],
    pub highlight_pos: f32,
    pub lightning: f32,
    pub last_lightning: f32,
    pub lightning_intensity: f32,
    pub li_drop_off: f32,
    pub extra_lightning_chance: f32,
    pub rain_fade: f32,
    pub bkg: NodeId,
    pub lightsource: NodeId,
    pub sprites: Vec<NodeId>,
    pub fade_sprite: NodeId,
}

impl RainDrop {
    pub const COUNT: usize = 500;

    /// The constructor: `Reset`, then a start anywhere on screen with a trail above.
    fn new(rng: &mut UnityRandom) -> Self {
        let mut drop = Self {
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            last_last_pos: Vec2::ZERO,
            depth: 0.0,
        };
        drop.reset(rng);
        let x = 1366.0 * rng.value();
        let y = 900.0 * rng.value();
        drop.pos = Vec2::new(x, y);
        drop.last_pos = drop.pos + Vec2::new(0.0, 300.0) / drop.depth;
        drop.last_last_pos = drop.last_pos + Vec2::new(0.0, 300.0) / drop.depth;
        drop
    }

    /// `Reset`: above the screen, at a new depth.
    fn reset(&mut self, rng: &mut UnityRandom) {
        let x = (-100.0_f32).lerp(1466.0, rng.value());
        let y = 770.0 + rng.value() * 100.0;
        self.pos = Vec2::new(x, y);
        self.last_pos = self.pos;
        self.last_last_pos = self.pos;
        self.depth = 0.5_f32.lerp(7.0, rng.value().mathf_pow(0.5));
    }

    fn update(&mut self, rain_dir: Vec2, rng: &mut UnityRandom) {
        self.last_last_pos = self.last_pos;
        self.last_pos = self.pos;
        let jitter = Vec2::random_unit(rng) * rng.value() * 14.0;
        self.pos += (rain_dir + jitter) / self.depth;
        if self.last_pos.y < 0.0 {
            self.reset(rng);
        }
    }
}

impl RainEffectData {
    /// `RainDir(dp)`.
    #[must_use]
    pub fn rain_dir(&self, dp: f32) -> Vec2 {
        Vec2::new(0.0, -300.0) + self.rain_dirs[0][0].lerp(self.rain_dirs[1][0], dp)
    }

    /// `LightningSpike`.
    pub fn lightning_spike(&mut self, new_int: f32, drop_off_frames: f32) {
        self.lightning_intensity = self.lightning_intensity.max(new_int);
        self.extra_lightning_chance = 1.0;
        self.li_drop_off = drop_off_frames;
    }

    /// `RainEffect.Update`.
    pub fn update(&mut self, rng: &mut UnityRandom) {
        self.last_lightning = self.lightning;
        for i in 0..self.drops.len() {
            let dir = self.rain_dir(self.drops[i].depth.inverse_lerp(0.5, 7.0));
            self.drops[i].update(dir, rng);
        }
        for dirs in &mut self.rain_dirs {
            let kick = Vec2::random_unit(rng) * rng.value().mathf_pow(3.0) * 5.0;
            dirs[0] += kick + dirs[1];
            dirs[0] = dirs[0].clamp_length(30.0);
            dirs[1] *= 0.9;
            dirs[1] -= dirs[0] * 0.04;
            dirs[1] += Vec2::random_unit(rng) * rng.value().mathf_pow(15.0) * 5.0;
            dirs[1] = dirs[1].clamp_length(10.0);
        }
        self.highlight_pos -= 40.0;
        if self.highlight_pos < -500.0 {
            self.highlight_pos = 1000.0_f32.lerp(3000.0, rng.value());
        }
        self.lightning = self.lightning.lerp_and_tick(
            0.0,
            0.06,
            1.0 / 8.0_f32.lerp(14.0, self.lightning_intensity),
        );
        if rng.value() < self.lightning_intensity * 1.0_f32.lerp(2.0, self.extra_lightning_chance) {
            self.lightning = self.lightning.max(rng.value() * self.lightning_intensity);
        }
        if self.li_drop_off > 0.0 {
            self.lightning_intensity = (self.lightning_intensity - 1.0 / self.li_drop_off).max(0.0);
        }
        self.extra_lightning_chance = (self.extra_lightning_chance - 0.1).max(0.0);
    }

    /// `RainEffect.GrafUpdate`.
    pub fn graf_update(&self, stage: &mut StageTree, t: f32) {
        let flash = self
            .last_lightning
            .lerp(self.lightning, t)
            .max(0.0)
            .mathf_pow(1.5);
        let fade = self.rain_fade.lerp(1.0, flash);
        for (drop, &node) in self.drops.iter().zip(&self.sprites) {
            let tail = drop.last_pos.lerp(drop.last_last_pos, 0.2);
            let along = (drop.pos.y + drop.pos.x * 0.23 - self.highlight_pos).abs();
            let f = along.inverse_lerp(2000.0, 50.0).mathf_pow(14.0).max(flash);
            let depth_alpha = 1.0_f32.lerp(
                0.1,
                drop.depth
                    .inverse_lerp(0.5, 7.0)
                    .mathf_pow(1.0_f32.lerp(0.5, f)),
            );
            let tr = stage.transform_mut(node);
            tr.x = drop.pos.x;
            tr.y = drop.pos.y;
            tr.scale_x = 2.2 / drop.depth.lerp(1.0, 0.7);
            tr.rotation = drop.pos.aim_to(tail);
            tr.scale_y = drop.pos.distance(tail);
            tr.alpha = depth_alpha * (0.3 * fade).lerp(0.8 * fade + 0.2 * flash, f);
        }
        let tr = stage.transform_mut(self.fade_sprite);
        tr.alpha = 0.37_f32.lerp(0.17, flash);
        tr.scale_x = 1600.0_f32.lerp(2100.0, flash) / 16.0;
        tr.scale_y = 1300.0_f32.lerp(1800.0, flash) / 16.0;
        stage.transform_mut(self.bkg).alpha = flash.mathf_pow(1.1) * 0.5;
        let tr = stage.transform_mut(self.lightsource);
        tr.alpha = flash.mathf_pow(0.9) * 0.5;
        tr.x = 800.0;
        tr.y = 600.0;
        tr.scale_x = 75.0;
        tr.scale_y = 68.75;
    }
}

impl Menu {
    /// `new RainEffect(menu, owner)`: the white flash quad, the light source, 500 drops
    /// (five draws each), the wind (eight draws) and the edge darkening, in that order.
    pub fn add_rain_effect(&mut self, owner: ObjectId, rng: &mut UnityRandom) -> ObjectId {
        let id = self.push(MenuObject::new(
            Some(owner),
            Vec2::ZERO,
            Kind::Container {
                gradients_alpha: None,
            },
        ));
        let container = self.container_of(id);
        let [w, h] = self.screen_size();
        let mut bkg = Sprite::new(crate::AtlasManager::WHITE);
        bkg.transform.scale_x = 1400.0;
        bkg.transform.scale_y = 800.0;
        bkg.transform.x = w / 2.0;
        bkg.transform.y = h / 2.0;
        let bkg = self.stage.add_sprite(container, bkg);
        let mut light = Sprite::new(crate::AtlasManager::WHITE);
        light.shader = StageShader::FlatLightNoisy;
        let lightsource = self.stage.add_sprite(container, light);
        let mut drops = Vec::with_capacity(RainDrop::COUNT);
        let mut sprites = Vec::with_capacity(RainDrop::COUNT);
        for _ in 0..RainDrop::COUNT {
            drops.push(RainDrop::new(rng));
            let mut sprite = Sprite::new("pixel");
            sprite.anchor[1] = 0.0;
            sprites.push(self.stage.add_sprite(container, sprite));
        }
        let mut rain_dirs = [[Vec2::ZERO; 2]; 2];
        for dirs in &mut rain_dirs {
            dirs[0] = Vec2::random_unit(rng) * rng.value() * 20.0;
            dirs[1] = Vec2::random_unit(rng) * rng.value() * 10.0;
        }
        let mut fade = Sprite::new(crate::AtlasManager::WHITE);
        fade.transform.x = w / 2.0;
        fade.transform.y = h / 2.0;
        fade.shader = StageShader::EdgeFade;
        fade.color = StageColor::BLACK;
        let fade_sprite = self.stage.add_sprite(container, fade);
        self.objects[id.0].kind = Kind::RainEffect(Box::new(RainEffectData {
            drops,
            rain_dirs,
            highlight_pos: 0.0,
            lightning: 0.0,
            last_lightning: 0.0,
            lightning_intensity: 0.0,
            li_drop_off: 1.0,
            extra_lightning_chance: 0.0,
            rain_fade: 0.0,
            bkg,
            lightsource,
            sprites,
            fade_sprite,
        }));
        id
    }

    pub fn rain_effect_mut(&mut self, id: ObjectId) -> Option<&mut RainEffectData> {
        match &mut self.objects[id.0].kind {
            Kind::RainEffect(r) => Some(r),
            _ => None,
        }
    }

    #[must_use]
    pub fn rain_effect(&self, id: ObjectId) -> Option<&RainEffectData> {
        match &self.objects[id.0].kind {
            Kind::RainEffect(r) => Some(r),
            _ => None,
        }
    }
}
