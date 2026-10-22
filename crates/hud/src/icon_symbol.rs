//! `IconSymbol` and its two kinds: a creature's (`CreatureSymbol`, whose sprite and
//! colour tables are [`CreatureSymbol`]'s) or an item's (`ItemSymbol`), each a sprite,
//! a colour and two black shadows, flashing white when shown. The map's shelter
//! markers show them. More Slugcats', the shared DLC's and the Watcher's branches are
//! left out (their objects never exist with the DLCs off; docs/divergences/ D15).
//!
//! Mirrors: IconSymbol.cs, ItemSymbol.cs, CreatureSymbol.cs (SymbolDataFromCreature)

use std::collections::HashMap;

use rw_assets::{AbstractObjectType, CreatureType};
use rw_menu::{AtlasManager, HslColor, MenuColor, NodeId, Sprite, StageColor, StageTree};
use rw_sim::creature::AbstractCreature;
use rw_sim::item::AbstractObject;
use rw_sim::{FloatExt, Vec2};

use crate::CreatureSymbol;

/// `IconSymbol.IconSymbolData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconSymbolData {
    pub crit_type: CreatureType,
    pub item_type: AbstractObjectType,
    pub int_data: i32,
}

impl IconSymbolData {
    /// `CreatureSymbol.SymbolDataFromCreature` with the Watcher off: a slugcat's player
    /// number (0, the port's only player), a centipede's size class.
    #[must_use]
    pub fn of_creature(c: &AbstractCreature) -> Self {
        let int_data = if c.kind() == CreatureType::Centipede {
            let size = c.centipede_generate_size();
            if size < 0.255 {
                1
            } else if size < 0.6 {
                2
            } else {
                3
            }
        } else {
            0
        };
        Self {
            crit_type: c.kind(),
            item_type: AbstractObjectType::Creature,
            int_data,
        }
    }

    /// `ItemSymbol.SymbolDataFromItem` with the DLCs off: none for a spear stuck in a
    /// wall and for the kinds without a symbol. The port's untyped objects (only rocks
    /// and spears carry their fields) read as the constructors leave them: an ordinary
    /// mask, unrotted fruit, pearl colour or type 0 (docs/divergences/ D15).
    #[must_use]
    pub fn of_item(o: &AbstractObject) -> Option<Self> {
        use AbstractObjectType as T;
        let int_data = match o.kind {
            T::Spear => {
                let spear = o.spear()?;
                if spear.stuck_in_wall() {
                    return None;
                }
                i32::from(spear.explosive)
            }
            T::AttachedBee
            | T::Creature
            | T::DartMaggot
            | T::KarmaFlower
            | T::Oracle
            | T::VoidSpawn => return None,
            _ => 0,
        };
        Some(Self {
            crit_type: CreatureType::StandardGroundCreature,
            item_type: o.kind,
            int_data,
        })
    }

    fn creature(self) -> CreatureSymbol {
        CreatureSymbol {
            kind: self.crit_type,
            int_data: self.int_data,
        }
    }

    /// `CreateIconSymbol`: a `CreatureSymbol` for a creature, else an `ItemSymbol`.
    fn is_creature(self) -> bool {
        self.item_type == AbstractObjectType::Creature
    }

    /// The symbol's `spriteName`.
    #[must_use]
    pub fn sprite_name(self) -> String {
        if self.is_creature() {
            self.creature().sprite_name()
        } else {
            self.item_sprite_name().to_owned()
        }
    }

    /// The symbol's `myColor`.
    #[must_use]
    pub fn color(self) -> StageColor {
        if self.is_creature() {
            self.creature().color()
        } else {
            self.item_color()
        }
    }

    /// `ItemSymbol.SpriteNameForItem` (base game).
    fn item_sprite_name(self) -> &'static str {
        use AbstractObjectType as T;
        let rotted = self.int_data == 1;
        match self.item_type {
            T::Rock => "Symbol_Rock",
            T::Spear if self.int_data == 1 => "Symbol_FireSpear",
            T::Spear => "Symbol_Spear",
            T::ScavengerBomb => "Symbol_StunBomb",
            T::SporePlant => "Symbol_SporePlant",
            T::Lantern => "Symbol_Lantern",
            T::FlareBomb => "Symbol_FlashBomb",
            T::PuffBall => "Symbol_PuffBall",
            T::WaterNut => "Symbol_WaterNut",
            T::FirecrackerPlant => "Symbol_Firecracker",
            T::DangleFruit if rotted => "Symbol_RotFruit",
            T::DangleFruit => "Symbol_DangleFruit",
            T::SeedCob if rotted => "Symbol_RotcornPlant",
            T::SeedCob => "Symbol_SeedCob",
            T::BubbleGrass => "Symbol_BubbleGrass",
            T::SlimeMold => "Symbol_SlimeMold",
            T::Mushroom => "Symbol_Mushroom",
            T::JellyFish => "Symbol_JellyFish",
            T::VultureMask if self.int_data == 1 => "Kill_KingVulture",
            T::VultureMask => "Kill_Vulture",
            T::FlyLure => "Symbol_FlyLure",
            T::SlOracleSwarmer | T::SsOracleSwarmer | T::NshSwarmer => "Symbol_Neuron",
            T::EggBugEgg => "Symbol_EggBugEgg",
            T::OverseerCarcass => "Kill_Overseer",
            T::DataPearl | T::PebblesPearl => "Symbol_Pearl",
            T::NeedleEgg => "needleEggSymbol",
            T::Pomegranate => "Symbol_Pomegranate",
            _ => AtlasManager::WHITE,
        }
    }

    /// `ItemSymbol.ColorForItem` (base game). A data pearl of a unique type
    /// (`intData > 1`) is not reached: the port's pearls have no type (type 0).
    fn item_color(self) -> StageColor {
        use AbstractObjectType as T;
        let rgb = StageColor::rgb;
        match self.item_type {
            T::SporePlant | T::FirecrackerPlant => rgb(174.0 / 255.0, 40.0 / 255.0, 30.0 / 255.0),
            T::SeedCob if self.int_data == 1 => rgb(0.3, 0.0, 1.0),
            T::SeedCob => rgb(174.0 / 255.0, 40.0 / 255.0, 30.0 / 255.0),
            T::ScavengerBomb => rgb(230.0 / 255.0, 14.0 / 255.0, 14.0 / 255.0),
            T::Spear if self.int_data == 1 => rgb(230.0 / 255.0, 14.0 / 255.0, 14.0 / 255.0),
            T::Spear => MenuColor::MediumGrey.rgb(),
            T::Lantern => rgb(1.0, 146.0 / 255.0, 81.0 / 255.0),
            T::FlareBomb => rgb(11.0 / 15.0, 174.0 / 255.0, 1.0),
            T::Pomegranate | T::BubbleGrass => rgb(14.0 / 255.0, 178.0 / 255.0, 60.0 / 255.0),
            T::GraffitiBomb => HslColor::new(13.0 / 18.0, 1.0, 0.7).rgb(),
            T::SlimeMold => rgb(1.0, 0.6, 0.0),
            T::DangleFruit if self.int_data == 1 => rgb(0.3, 0.0, 1.0),
            T::DangleFruit => rgb(0.0, 0.0, 1.0),
            T::Mushroom | T::SsOracleSwarmer => rgb(1.0, 1.0, 1.0),
            T::WaterNut => rgb(0.05, 0.3, 0.7),
            T::EggBugEgg => rgb(0.0, 1.0, 120.0 / 255.0),
            T::FlyLure => rgb(173.0 / 255.0, 4.0 / 15.0, 54.0 / 255.0),
            T::NshSwarmer => rgb(0.0, 1.0, 0.3),
            T::NeedleEgg => rgb(147.0 / 255.0, 41.0 / 255.0, 64.0 / 255.0),
            T::PebblesPearl => match self.int_data {
                1 => rgb(0.7, 0.7, 0.7),
                2 => MenuColor::DarkGrey.rgb(),
                n if n < 0 => rgb(0.0, 116.0 / 255.0, 163.0 / 255.0),
                _ => rgb(1.0, 122.0 / 255.0, 2.0 / 255.0),
            },
            T::DataPearl if self.int_data == 1 => rgb(1.0, 0.6, 0.9),
            T::DataPearl => rgb(0.7, 0.7, 0.7),
            _ => MenuColor::MediumGrey.rgb(),
        }
    }
}

/// The source widths of the atlas elements a symbol can take (`graphWidth` is the
/// element's `sourcePixelSize.x`), read once from the atlases: the map makes its
/// symbols while drawing, where it holds no atlases.
#[derive(Debug, Clone, Default)]
pub struct SymbolWidths(HashMap<String, f32>);

impl SymbolWidths {
    /// Every sprite name of a base-game creature or item symbol.
    #[must_use]
    pub fn of(atlases: &AtlasManager) -> Self {
        let creatures = CreatureType::ALL.iter().flat_map(|&kind| {
            (0..=3).map(move |int_data| IconSymbolData {
                crit_type: kind,
                item_type: AbstractObjectType::Creature,
                int_data,
            })
        });
        let items = AbstractObjectType::ALL.iter().flat_map(|&kind| {
            (0..=2).map(move |int_data| IconSymbolData {
                crit_type: CreatureType::StandardGroundCreature,
                item_type: kind,
                int_data,
            })
        });
        let mut widths = HashMap::new();
        for data in creatures.chain(items) {
            let name = data.sprite_name();
            if let Some(e) = atlases.element(&name) {
                widths.insert(name, e.source_pixel_size[0]);
            }
        }
        Self(widths)
    }

    /// The width of element `name` (0 for one not loaded).
    #[must_use]
    pub fn width(&self, name: &str) -> f32 {
        self.0.get(name).copied().unwrap_or(0.0)
    }
}

/// `IconSymbol` shown (`Show(showShadowSprites: true)`): the two black shadows, then
/// the symbol.
#[derive(Debug, Clone)]
pub struct IconSymbol {
    pub data: IconSymbolData,
    pub sprite_name: String,
    pub my_color: StageColor,
    pub graph_width: f32,
    pub show_flash: f32,
    pub last_show_flash: f32,
    pub symbol: NodeId,
    pub shadows: [NodeId; 2],
}

impl IconSymbol {
    /// `CreateIconSymbol(data, container)` then `Show(true)`.
    pub fn show(
        data: IconSymbolData,
        widths: &SymbolWidths,
        stage: &mut StageTree,
        container: NodeId,
    ) -> Self {
        let sprite_name = data.sprite_name();
        let shadows = [0, 1].map(|_| {
            let mut s = Sprite::new(&sprite_name);
            s.color = StageColor::BLACK;
            stage.add_sprite(container, s)
        });
        let symbol = stage.add_sprite(container, Sprite::new(&sprite_name));
        Self {
            data,
            my_color: data.color(),
            graph_width: widths.width(&sprite_name),
            sprite_name,
            show_flash: 1.0,
            last_show_flash: 1.0,
            symbol,
            shadows,
        }
    }

    pub fn update(&mut self) {
        self.last_show_flash = self.show_flash;
        self.show_flash = self.show_flash.lerp_and_tick(0.0, 0.08, 0.1);
    }

    /// `Draw(timeStacker, drawPos)`: the symbol whitened by the flash, the shadows two
    /// left and one down, one left and one up.
    pub fn draw(&self, stage: &mut StageTree, t: f32, at: Vec2) {
        let flash = self.last_show_flash.lerp(self.show_flash, t);
        let s = stage.sprite_mut(self.symbol);
        s.color = self.my_color.lerp(StageColor::WHITE, flash.mathf_pow(3.0));
        s.transform.x = at.x;
        s.transform.y = at.y;
        for (id, d) in self.shadows.into_iter().zip([(-2.0, -1.0), (-1.0, 1.0)]) {
            let tr = stage.transform_mut(id);
            tr.x = at.x + d.0;
            tr.y = at.y + d.1;
        }
    }

    pub fn remove_sprites(&self, stage: &mut StageTree) {
        stage.remove_from_container(self.symbol);
        for id in self.shadows {
            stage.remove_from_container(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: AbstractObjectType, int_data: i32) -> IconSymbolData {
        IconSymbolData {
            crit_type: CreatureType::StandardGroundCreature,
            item_type: kind,
            int_data,
        }
    }

    #[test]
    fn item_sprites_and_colours() {
        use AbstractObjectType as T;
        assert_eq!(item(T::Rock, 0).sprite_name(), "Symbol_Rock");
        assert_eq!(item(T::Rock, 0).color(), MenuColor::MediumGrey.rgb());
        assert_eq!(item(T::Spear, 0).sprite_name(), "Symbol_Spear");
        assert_eq!(item(T::Spear, 1).sprite_name(), "Symbol_FireSpear");
        assert_eq!(
            item(T::Spear, 1).color(),
            StageColor::rgb(230.0 / 255.0, 14.0 / 255.0, 14.0 / 255.0)
        );
        assert_eq!(item(T::VultureMask, 1).sprite_name(), "Kill_KingVulture");
        assert_eq!(item(T::NeedleEgg, 0).sprite_name(), "needleEggSymbol");
        assert_eq!(item(T::CollisionField, 0).sprite_name(), "Futile_White");
    }

    #[test]
    fn creature_symbols_use_the_kill_tables() {
        let green = IconSymbolData {
            crit_type: CreatureType::GreenLizard,
            item_type: AbstractObjectType::Creature,
            int_data: 0,
        };
        assert_eq!(green.sprite_name(), "Kill_Green_Lizard");
        assert_eq!(green.color(), StageColor::rgb(0.2, 1.0, 0.0));
    }

    #[test]
    fn flash_ticks_down_from_one() {
        let mut stage = StageTree::new();
        let c = stage.add_container(StageTree::ROOT);
        let mut s = IconSymbol::show(
            item(AbstractObjectType::Rock, 0),
            &SymbolWidths::default(),
            &mut stage,
            c,
        );
        s.update();
        // `LerpAndTick(1, 0, 0.08, 0.1)`: lerp to 0.92, then 0.1 closer.
        assert!((s.show_flash - 0.82).abs() < 1e-6);
        assert_eq!(s.last_show_flash, 1.0);
    }
}
