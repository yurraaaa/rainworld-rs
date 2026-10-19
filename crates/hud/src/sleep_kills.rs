//! `SleepAndDeathScreen.SleepScreenKills` (551238): the sleep screen's row of kill
//! trophies at the top left. After a wait the crossbones and "~" appear, then each
//! creature that earns a trophy (`CreatureSymbol.DoesCreatureEarnATrophy`) shows every
//! eighth tick with a tick sound, kills of the same kind collapsed behind each other
//! (`PlayerResultBox.KillTrophy`, `CollapseCreatureSymbols`); holding the map button
//! hurries it. The food meter waits for it (`AllowFoodMeterTick`).
//!
//! Mirrors: Menu/SleepScreenKills.cs, Menu/PlayerResultBox.cs (KillTrophy)

use rw_assets::{ActiveMods, CreatureType};
use rw_menu::menu::{Kind, Menu, ObjectId};
use rw_menu::{
    Alignment, AtlasManager, HslColor, MenuColor, MenuShared, MenuSoundId, NodeId, Sprite,
    StageColor,
};
use rw_save::session::KillRecord;
use rw_sim::creature::CreatureTemplate;
use rw_sim::{FloatExt, Vec2};

/// `CreatureSymbol` of a kill: its `IconSymbolData` (type and `intData`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatureSymbol {
    pub kind: CreatureType,
    pub int_data: i32,
}

impl CreatureSymbol {
    /// `DoesCreatureEarnATrophy` with Remix, More Slugcats and Watcher off.
    #[must_use]
    pub fn earns_trophy(kind: CreatureType) -> bool {
        use CreatureType as T;
        !CreatureTemplate::of(kind, ActiveMods::NONE).quantified
            && !matches!(
                kind,
                T::VultureGrub
                    | T::Hazer
                    | T::TubeWorm
                    | T::SmallNeedleWorm
                    | T::SmallCentipede
                    | T::StandardGroundCreature
            )
    }

    /// `PlayerResultBox.CollapseCreatureSymbols`.
    #[must_use]
    pub fn collapses_with(self, b: Self) -> bool {
        use CreatureType as T;
        let a = self;
        if a.kind == T::Centipede && b.kind == T::SmallCentipede && a.int_data == 1 {
            return true;
        }
        if b.kind == T::Centipede && a.kind == T::SmallCentipede && b.int_data == 1 {
            return true;
        }
        if a.kind == b.kind {
            if matches!(a.kind, T::Slugcat | T::Centipede) {
                return a.int_data == b.int_data;
            }
            return true;
        }
        false
    }

    /// `LizardSpriteName` (the rot and proto lizards are Watcher's).
    fn lizard_sprite(self, default: &'static str) -> String {
        match self.int_data {
            2 => "Kill_ProtoLizard",
            1 => "Kill_RotLizard",
            _ => default,
        }
        .to_owned()
    }

    /// `SpriteNameOfCreature` (base game).
    #[must_use]
    pub fn sprite_name(self) -> String {
        use CreatureType as T;
        let name = match self.kind {
            T::Slugcat => "Kill_Slugcat",
            T::GreenLizard => return self.lizard_sprite("Kill_Green_Lizard"),
            T::PinkLizard | T::BlueLizard | T::CyanLizard | T::RedLizard => {
                return self.lizard_sprite("Kill_Standard_Lizard");
            }
            T::WhiteLizard => return self.lizard_sprite("Kill_White_Lizard"),
            T::BlackLizard => return self.lizard_sprite("Kill_Black_Lizard"),
            T::YellowLizard => return self.lizard_sprite("Kill_Yellow_Lizard"),
            T::Salamander => return self.lizard_sprite("Kill_Salamander"),
            T::Scavenger => "Kill_Scavenger",
            T::Vulture => "Kill_Vulture",
            T::KingVulture => "Kill_KingVulture",
            T::CicadaA | T::CicadaB => "Kill_Cicada",
            T::Snail => "Kill_Snail",
            T::Centiwing => "Kill_Centiwing",
            T::SmallCentipede => "Kill_Centipede1",
            T::Centipede => return format!("Kill_Centipede{}", self.int_data.clamp(1, 3)),
            T::RedCentipede => "Kill_Centipede3",
            T::BrotherLongLegs | T::DaddyLongLegs => "Kill_Daddy",
            T::LanternMouse => "Kill_Mouse",
            T::GarbageWorm => "Kill_Garbageworm",
            T::Fly => "Kill_Bat",
            T::Leech | T::SeaLeech => "Kill_Leech",
            T::Spider => "Kill_SmallSpider",
            T::JetFish => "Kill_Jetfish",
            T::BigEel => "Kill_BigEel",
            T::Deer => "Kill_RainDeer",
            T::TubeWorm => "Kill_Tubeworm",
            T::TentaclePlant => "Kill_TentaclePlant",
            T::PoleMimic => "Kill_PoleMimic",
            T::MirosBird => "Kill_MirosBird",
            T::Overseer => "Kill_Overseer",
            T::VultureGrub => "Kill_VultureGrub",
            T::EggBug => "Kill_EggBug",
            T::BigSpider | T::SpitterSpider => "Kill_BigSpider",
            T::BigNeedleWorm => "Kill_NeedleWorm",
            T::SmallNeedleWorm => "Kill_SmallNeedleWorm",
            T::DropBug => "Kill_DropBug",
            T::Hazer => "Kill_Hazer",
            _ => AtlasManager::WHITE,
        };
        name.to_owned()
    }

    /// `ColorOfCreature` (base game; a slugcat by its player number's arena colour).
    #[must_use]
    pub fn color(self) -> StageColor {
        use CreatureType as T;
        match self.kind {
            T::Slugcat => match self.int_data {
                1 => StageColor::rgb(1.0, 1.0, 115.0 / 255.0),
                2 => StageColor::rgb(1.0, 115.0 / 255.0, 115.0 / 255.0),
                3 => HslColor::new(0.630_555_6, 0.54, 0.2).rgb(),
                _ => StageColor::rgb(1.0, 1.0, 1.0),
            },
            T::GreenLizard => StageColor::rgb(0.2, 1.0, 0.0),
            T::PinkLizard => StageColor::rgb(1.0, 0.0, 1.0),
            T::BlueLizard => StageColor::rgb(0.0, 0.5, 1.0),
            T::WhiteLizard | T::CicadaA => StageColor::rgb(1.0, 1.0, 1.0),
            T::RedLizard | T::RedCentipede => {
                StageColor::rgb(230.0 / 255.0, 14.0 / 255.0, 14.0 / 255.0)
            }
            T::BlackLizard | T::CicadaB => {
                StageColor::rgb(94.0 / 255.0, 94.0 / 255.0, 111.0 / 255.0)
            }
            T::YellowLizard | T::SmallCentipede | T::Centipede => StageColor::rgb(1.0, 0.6, 0.0),
            T::CyanLizard | T::Overseer => StageColor::rgb(0.0, 232.0 / 255.0, 230.0 / 255.0),
            T::Salamander => StageColor::rgb(14.0 / 15.0, 199.0 / 255.0, 228.0 / 255.0),
            T::SpitterSpider | T::Leech => {
                StageColor::rgb(174.0 / 255.0, 40.0 / 255.0, 30.0 / 255.0)
            }
            T::SeaLeech | T::TubeWorm => StageColor::rgb(0.05, 0.3, 0.7),
            T::Centiwing => StageColor::rgb(14.0 / 255.0, 178.0 / 255.0, 60.0 / 255.0),
            T::BrotherLongLegs => StageColor::rgb(116.0 / 255.0, 134.0 / 255.0, 78.0 / 255.0),
            T::DaddyLongLegs => StageColor::rgb(0.0, 0.0, 1.0),
            T::VultureGrub => StageColor::rgb(212.0 / 255.0, 202.0 / 255.0, 111.0 / 255.0),
            T::EggBug => StageColor::rgb(0.0, 1.0, 120.0 / 255.0),
            T::BigNeedleWorm | T::SmallNeedleWorm => {
                StageColor::rgb(1.0, 152.0 / 255.0, 152.0 / 255.0)
            }
            T::Hazer => StageColor::rgb(54.0 / 255.0, 202.0 / 255.0, 99.0 / 255.0),
            _ => MenuColor::MediumGrey.rgb(),
        }
    }
}

/// `PlayerResultBox.KillTrophy` with its `CreatureSymbol`'s sprites.
#[derive(Debug, Clone)]
pub struct KillTrophy {
    pub symbol: CreatureSymbol,
    sprite_name: String,
    color: StageColor,
    /// `graphWidth`: the sprite element's width.
    pub graph_width: f32,
    pub relative_pos: Vec2,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub collapsed: bool,
    pub prev_collapsed: bool,
    pub show_flash: f32,
    pub last_show_flash: f32,
    /// The symbol sprite and, behind a collapsed one, its two black shadows (from
    /// `Show`).
    pub sprites: Option<(NodeId, Option<[NodeId; 2]>)>,
}

impl KillTrophy {
    /// `MyWidth(compress)`.
    #[must_use]
    pub fn my_width(&self, compress: bool) -> f32 {
        if compress && self.collapsed {
            7.0
        } else {
            self.graph_width + 5.0
        }
    }
}

/// The crossbones `SymbolAndLabel` ("Multiplayer_Bones" and " ~"), never lit here.
#[derive(Debug, Clone, Copy)]
struct CrossBones {
    sprite: NodeId,
}

/// `SleepScreenKills`, at `pos` on the page.
#[derive(Debug, Clone)]
pub struct SleepScreenKills {
    pub pos: Vec2,
    pub trophies: Vec<KillTrophy>,
    cross_bones: Option<CrossBones>,
    display_value: i32,
    counter: i32,
    pub counted_and_done: bool,
    right_most_kill_symbol: f32,
    right_most_kill_symbol_width: f32,
    kill_symbol_rows: i32,
    pub started: bool,
    wait: i32,
    /// `AvailableXDist`: the continue and exit buttons' x less twice `pos.x`.
    available_x_dist: f32,
}

impl SleepScreenKills {
    /// The constructor: a trophy for each kill that earns one, collapsed with the next
    /// of the same kind, laid out from x 48 in rows 25 px apart within the available
    /// width.
    #[must_use]
    pub fn new(atlases: &AtlasManager, pos: Vec2, buttons_x: f32, kills: &[KillRecord]) -> Self {
        let mut trophies: Vec<KillTrophy> = kills
            .iter()
            .filter_map(|k| {
                let kind = CreatureType::from_value(&k.creature, ActiveMods::NONE)?;
                CreatureSymbol::earns_trophy(kind).then_some(CreatureSymbol {
                    kind,
                    int_data: k.int_data,
                })
            })
            .map(|symbol| {
                let sprite_name = symbol.sprite_name();
                let graph_width = atlases
                    .element(&sprite_name)
                    .map_or(0.0, |e| e.source_pixel_size[0]);
                KillTrophy {
                    symbol,
                    color: symbol.color(),
                    sprite_name,
                    graph_width,
                    relative_pos: Vec2::ZERO,
                    pos: Vec2::ZERO,
                    last_pos: Vec2::ZERO,
                    collapsed: false,
                    prev_collapsed: false,
                    show_flash: 0.0,
                    last_show_flash: 0.0,
                    sprites: None,
                }
            })
            .collect();
        for k in 0..trophies.len().saturating_sub(1) {
            if trophies[k].symbol.collapses_with(trophies[k + 1].symbol) {
                trophies[k].collapsed = true;
                trophies[k + 1].prev_collapsed = true;
            }
        }
        let mut this = Self {
            pos,
            trophies: Vec::new(),
            cross_bones: None,
            display_value: 0,
            counter: 0,
            counted_and_done: false,
            right_most_kill_symbol: 0.0,
            right_most_kill_symbol_width: 0.0,
            kill_symbol_rows: 0,
            started: false,
            wait: 40,
            available_x_dist: buttons_x - pos.x * 2.0,
        };
        let mut at = Vec2::new(48.0, 0.0);
        for l in 0..trophies.len() {
            let width = trophies[l].graph_width;
            trophies[l].relative_pos = Vec2::new(width / 2.0 + at.x, at.y);
            at.x += trophies[l].my_width(true);
            let next = trophies.get(l + 1).map_or(0.0, |t| t.my_width(false));
            if at.x + next > this.available_x_dist {
                at = Vec2::new(0.0, at.y - 25.0);
                this.kill_symbol_rows += 1;
            }
            let right = trophies[l].relative_pos.x + width / 2.0;
            if right > this.right_most_kill_symbol {
                this.right_most_kill_symbol = right;
                this.right_most_kill_symbol_width = width;
            }
        }
        if this.kill_symbol_rows == 0
            && let Some(last) = trophies.last()
        {
            this.right_most_kill_symbol = this.available_x_dist;
            this.right_most_kill_symbol_width = last.graph_width;
        }
        this.counted_and_done = trophies.is_empty();
        this.trophies = trophies;
        this
    }

    /// `Update` (`map` is the map button, `RWInput.PlayerInput(0).mp`): the trophies'
    /// flash, then the wait (four times as fast with the map button) with the
    /// crossbones at 24, then a trophy shown every eighth tick (every tick with the
    /// map button).
    pub fn update(&mut self, menu: &mut Menu, shared: &mut MenuShared, page: ObjectId, map: bool) {
        for t in &mut self.trophies {
            t.last_pos = t.pos;
            t.last_show_flash = t.show_flash;
            t.show_flash = t.show_flash.lerp_and_tick(0.0, 0.08, 0.1);
        }
        if !self.started || self.counted_and_done {
            return;
        }
        if self.wait > 0 {
            if self.wait <= 24 && self.cross_bones.is_none() {
                shared.play_menu_sound(MenuSoundId::PlayerResultBoxKillTick, [0.0, 1.0, 1.0]);
                self.add_cross_bones(menu, shared, page);
            }
            self.wait -= if map { 4 } else { 1 };
            return;
        }
        self.counter += 1;
        let (right, width, avail) = (
            self.right_most_kill_symbol,
            self.right_most_kill_symbol_width,
            self.available_x_dist,
        );
        for t in &mut self.trophies {
            t.pos.x = t
                .relative_pos
                .x
                .lerp_map(0.0, right, 0.0, avail - width / 2.0);
            t.pos.y = t.relative_pos.y;
        }
        if self.display_value <= -1 || !(self.counter % 8 == 0 || map) {
            return;
        }
        let index = usize::try_from(self.display_value).unwrap_or(usize::MAX);
        if let Some(t) = self.trophies.get_mut(index) {
            shared.play_menu_sound(MenuSoundId::PlayerResultBoxKillTick, [0.0, 1.0, 1.0]);
            Self::show(t, menu);
        } else {
            self.counted_and_done = true;
        }
        self.display_value += 1;
    }

    /// `IconSymbol.Show`: behind a collapsed symbol two black shadows, then the symbol,
    /// flashing white.
    fn show(t: &mut KillTrophy, menu: &mut Menu) {
        let container = menu.container;
        let shadows = t.prev_collapsed.then(|| {
            [0, 1].map(|_| {
                let mut s = Sprite::new(&t.sprite_name);
                s.color = StageColor::BLACK;
                menu.stage.add_sprite(container, s)
            })
        });
        let symbol = menu
            .stage
            .add_sprite(container, Sprite::new(&t.sprite_name));
        t.sprites = Some((symbol, shadows));
        t.show_flash = 1.0;
        t.last_show_flash = 1.0;
    }

    /// The crossbones `SymbolAndLabel` at (10, 0): the bones sprite in medium grey and
    /// " ~" in big text, left-aligned 39 px to its left.
    fn add_cross_bones(&mut self, menu: &mut Menu, shared: &MenuShared, page: ObjectId) {
        let mut s = Sprite::new("Multiplayer_Bones");
        s.color = MenuColor::MediumGrey.rgb();
        s.transform.y = -10000.0;
        let sprite = menu.stage.add_sprite(menu.container, s);
        let at = self.pos + Vec2::new(10.0, 0.0) + Vec2::new(-39.0, -9.0);
        let label = menu.add_label(shared, page, " ~", at, Vec2::new(100.0, 20.0), true);
        if let Kind::Label(l) = &menu.object(label).kind {
            let node = l.label;
            let label = menu.stage.label_mut(node);
            label.set_alignment(Alignment::Left);
            label.color = MenuColor::MediumGrey.rgb();
        }
        menu.add_sub(page, label);
        self.cross_bones = Some(CrossBones { sprite });
    }

    /// `GrafUpdate` with the page drawn at `page`: the crossbones and each shown
    /// trophy (`IconSymbol.Draw`), a collapsed one's symbol sliding in from half its
    /// width as it flashes.
    pub fn draw(&self, menu: &mut Menu, page: Vec2, t: f32) {
        let origin = page + self.pos;
        if let Some(c) = self.cross_bones {
            let at = origin + Vec2::new(10.0, 0.0);
            let tr = &mut menu.stage.sprite_mut(c.sprite).transform;
            tr.x = at.x;
            tr.y = at.y;
        }
        for trophy in &self.trophies {
            let Some((symbol, shadows)) = trophy.sprites else {
                continue;
            };
            let flash = trophy.last_show_flash.lerp(trophy.show_flash, t);
            let mut at = origin + trophy.last_pos.lerp(trophy.pos, t);
            if trophy.prev_collapsed {
                at.x += trophy.graph_width / 2.0 * flash;
            }
            let s = menu.stage.sprite_mut(symbol);
            s.color = trophy.color.lerp(StageColor::WHITE, flash.mathf_pow(3.0));
            s.transform.x = at.x;
            s.transform.y = at.y;
            if let Some([a, b]) = shadows {
                let tr = &mut menu.stage.sprite_mut(a).transform;
                tr.x = at.x - 2.0;
                tr.y = at.y - 1.0;
                let tr = &mut menu.stage.sprite_mut(b).transform;
                tr.x = at.x - 1.0;
                tr.y = at.y + 1.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kill(kind: CreatureType) -> KillRecord {
        KillRecord::new(kind, 0, 1, false)
    }

    #[test]
    fn trophies_skip_quantified_and_small_kills_and_collapse_repeats() {
        // `DoesCreatureEarnATrophy` (Remix off: an overseer earns one); a fly is
        // quantified. Two overseers in a row collapse: the first is 7 px wide.
        assert!(CreatureSymbol::earns_trophy(CreatureType::Overseer));
        assert!(!CreatureSymbol::earns_trophy(CreatureType::Fly));
        assert!(!CreatureSymbol::earns_trophy(CreatureType::VultureGrub));
        let o = CreatureSymbol {
            kind: CreatureType::Overseer,
            int_data: 0,
        };
        assert_eq!(o.sprite_name(), "Kill_Overseer");
        assert_eq!(
            o.color(),
            StageColor::rgb(0.0, 232.0 / 255.0, 230.0 / 255.0)
        );
        let kills = [
            kill(CreatureType::Fly),
            kill(CreatureType::Overseer),
            kill(CreatureType::Overseer),
            kill(CreatureType::PinkLizard),
        ];
        let k = SleepScreenKills::new(&AtlasManager::new(), Vec2::new(70.0, 728.0), 1000.0, &kills);
        assert_eq!(k.trophies.len(), 3);
        assert!(k.trophies[0].collapsed && k.trophies[1].prev_collapsed);
        // Without the atlas the symbols are 0 wide: x 48, then +7 (collapsed), +5.
        let xs: Vec<f32> = k.trophies.iter().map(|t| t.relative_pos.x).collect();
        assert_eq!(xs, [48.0, 55.0, 60.0]);
        assert!(!k.counted_and_done);
        let none = SleepScreenKills::new(&AtlasManager::new(), Vec2::ZERO, 1000.0, &kills[..1]);
        assert!(none.counted_and_done, "a fly alone shows nothing");
    }
}
