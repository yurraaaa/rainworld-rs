//! What a shelter marker shows of the shelter's contents: one icon per item or creature
//! in the room (`ShelterMarker.ItemInShelterMarker`), laid out in centred rows under
//! the marker when the marker shows (`RevealSymbols`), a dead creature's pulsing to
//! dark grey. A player's map reads the room live from the world (`GetItemInShelter`),
//! a sleep or death screen's the items its map data took (`UpdateData`'s
//! `putItemsInShelters`).
//!
//! Mirrors: HUD/Map.cs (ShelterMarker, ItemInShelterMarker, GetItemInShelter,
//!   GetItemInShelterFromWorld)

use rw_menu::{MenuColor, NodeId, StageTree};
use rw_sim::creature::EntityId;
use rw_sim::item::Entity;
use rw_sim::world::World;
use rw_sim::{FloatExt, Vec2};

use crate::icon_symbol::{IconSymbol, IconSymbolData, SymbolWidths};

/// `GetItemInShelter` looks at this many entities of a shelter.
const MAX_ITEMS: usize = 50;

/// `ItemInShelterMarker.ItemInShelterData`: the symbol, the object's ID and its status
/// (1 for a dead creature or a bubble grass out of air; the game's -1 "no more" entry
/// ends a list here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemInShelterData {
    pub symbol: IconSymbolData,
    pub id: EntityId,
    pub status: i32,
}

impl ItemInShelterData {
    /// `GetItemInShelterFromWorld` for indices 0 to 49 until the room's entities run
    /// out, each through `DataFromAbstractPhysical`: none for the slugcat and
    /// quantified creatures (and objects `destroyOnAbstraction`, which the port's never
    /// are), a creature by `SymbolDataFromCreature`, an item by `SymbolDataFromItem`.
    /// Saved objects the port keeps as text are not among the entities (D15, D16).
    #[must_use]
    pub fn list_from_world(world: &World, room: i32) -> Vec<Self> {
        if world.abstract_room(room).is_none() {
            return Vec::new();
        }
        world
            .room_entities(room)
            .iter()
            .take(MAX_ITEMS)
            .filter_map(|&e| Self::of_entity(world, e))
            .collect()
    }

    fn of_entity(world: &World, e: Entity) -> Option<Self> {
        match e {
            Entity::Creature(id) => {
                let c = world.creature(id);
                if c.kind() == rw_assets::CreatureType::Slugcat || c.template.quantified {
                    return None;
                }
                Some(Self {
                    symbol: IconSymbolData::of_creature(c),
                    id: c.id,
                    status: i32::from(!c.state.alive),
                })
            }
            // A bubble grass's `oxygenLeft` is not kept: always full.
            Entity::Object(id) => {
                let o = world.object(id);
                Some(Self {
                    symbol: IconSymbolData::of_item(o)?,
                    id: o.id,
                    status: 0,
                })
            }
        }
    }
}

/// One shelter's items, as a map reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelterItems {
    pub room: i32,
    pub items: Vec<ItemInShelterData>,
}

impl ShelterItems {
    /// Every shelter room of `world`, read live (a player's map in a room).
    #[must_use]
    pub fn of_world(world: &World) -> Vec<Self> {
        world
            .data()
            .shelters
            .iter()
            .map(|&room| Self {
                room,
                items: ItemInShelterData::list_from_world(world, room),
            })
            .collect()
    }
}

/// `ItemInShelterMarker`.
#[derive(Debug, Clone)]
pub struct ItemInShelterMarker {
    pub data: ItemInShelterData,
    pub symbol: IconSymbol,
    /// Offset from the shelter marker.
    pub pos: Vec2,
}

impl ItemInShelterMarker {
    /// `Draw`: a dead creature's colour pulses towards very dark grey, `sin_add` ticks
    /// out of step with the ones before it.
    fn draw(&self, stage: &mut StageTree, fade: f32, at: Vec2, t: f32, counter: i32, sin_add: f32) {
        self.symbol.draw(stage, t, at + self.pos);
        let s = stage.sprite_mut(self.symbol.symbol);
        if self.data.status == 1 {
            let pulse = (0.5 + 0.5 * ((counter as f32 - sin_add + t) / 14.0).sin()).clamp(0.0, 1.0);
            s.color = MenuColor::VeryDarkGrey
                .rgb()
                .lerp(s.color, pulse.mathf_pow(3.0));
        }
        s.transform.alpha = fade;
        for id in self.symbol.shadows {
            stage.transform_mut(id).alpha = fade;
        }
    }
}

/// The shelter marker's part of `ShelterMarker`: its item markers and row widths.
#[derive(Debug, Clone, Default)]
pub struct ShelterSymbols {
    pub items: Vec<ItemInShelterMarker>,
    rows_width: Vec<f32>,
    last_visible: bool,
    pub symbols_active: bool,
}

/// What a shelter marker draws its items with.
pub struct ShelterDraw<'a> {
    /// `fade > 0 && lastFade > 0 && map.visible`.
    pub shown: bool,
    pub map_visible: bool,
    /// `Lerp(map.lastFade, map.fade, t) * Lerp(lastFade, fade, t)`.
    pub alpha: f32,
    pub screen_pos: Vec2,
    pub counter: i32,
    /// This shelter's items (`GetItemInShelter`), read when they show.
    pub items: &'a [ItemInShelterData],
    pub widths: &'a SymbolWidths,
    /// The map's `inFrontContainer`.
    pub container: NodeId,
}

impl ShelterSymbols {
    /// `ShelterMarker.Update`'s items: they tick while the map shows them.
    pub fn update(&mut self, map_visible: bool, fade: f32) {
        if map_visible && self.symbols_active && fade > 0.0 {
            for item in &mut self.items {
                item.symbol.update();
            }
        }
    }

    /// `ShelterMarker.Draw`'s items: revealed as the marker shows, hidden as it stops;
    /// drawn at `InverseLerp(0.5, 1, alpha)`.
    pub fn draw(&mut self, stage: &mut StageTree, d: &ShelterDraw<'_>, t: f32) {
        if d.shown && !self.last_visible {
            self.reveal(stage, d);
        } else if !d.shown && self.last_visible {
            self.hide(stage);
        }
        self.last_visible = d.shown;
        if !d.map_visible {
            return;
        }
        let fade = d.alpha.inverse_lerp(0.5, 1.0);
        for (i, item) in self.items.iter().enumerate() {
            item.draw(stage, fade, d.screen_pos, t, d.counter, i as f32 * 5.0);
        }
    }

    /// `RevealSymbols`: an item marker per item, then their places in rows of about
    /// 200 px, 30 px apart under the marker, each row centred (laid out twice, the
    /// second time with the first pass's row widths).
    fn reveal(&mut self, stage: &mut StageTree, d: &ShelterDraw<'_>) {
        self.symbols_active = true;
        self.items.clear();
        self.rows_width.clear();
        for &data in d.items.iter().take(MAX_ITEMS) {
            self.items.push(ItemInShelterMarker {
                data,
                symbol: IconSymbol::show(data.symbol, d.widths, stage, d.container),
                pos: Vec2::ZERO,
            });
        }
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() - 1;
        for _ in 0..2 {
            let mut x = 0.0;
            let mut row = 0;
            for k in 0..self.items.len() {
                if self.rows_width.len() <= row {
                    self.rows_width.push(0.0);
                }
                let width = self.items[k].symbol.graph_width;
                self.items[k].pos = Vec2::new(
                    x + (width - self.rows_width[row]) / 2.0,
                    (row + 1) as f32 * -30.0,
                );
                x += width;
                if x > 200.0 || k == last {
                    self.rows_width[row] = x;
                }
                if x > 200.0 {
                    x = 0.0;
                    row += 1;
                } else {
                    x += 5.0;
                }
            }
        }
    }

    /// `HideSymbols`.
    fn hide(&mut self, stage: &mut StageTree) {
        self.symbols_active = false;
        for item in &self.items {
            item.symbol.remove_sprites(stage);
        }
        self.items.clear();
        self.rows_width.clear();
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::{AbstractObjectType, CreatureType, PngImage};
    use rw_menu::{AtlasManager, StageColor};
    use rw_render::{TextureFilter, TextureSampling};

    use super::*;

    /// Atlases with a 14 px rock and a 30 px green lizard symbol.
    fn widths() -> SymbolWidths {
        let mut atlases = AtlasManager::new();
        let sampling = TextureSampling {
            filter: TextureFilter::Bilinear,
            clamp: true,
        };
        for (name, w) in [("Symbol_Rock", 14_usize), ("Kill_Green_Lizard", 30)] {
            let image = PngImage::from_pixels(w, 10, vec![[255; 4]; w * 10]);
            atlases.add_image_atlas(name, image, sampling).unwrap();
        }
        SymbolWidths::of(&atlases)
    }

    fn rock(n: i32) -> ItemInShelterData {
        ItemInShelterData {
            symbol: IconSymbolData {
                crit_type: CreatureType::StandardGroundCreature,
                item_type: AbstractObjectType::Rock,
                int_data: 0,
            },
            id: EntityId::new(-1, n),
            status: 0,
        }
    }

    fn dead_lizard() -> ItemInShelterData {
        ItemInShelterData {
            symbol: IconSymbolData {
                crit_type: CreatureType::GreenLizard,
                item_type: AbstractObjectType::Creature,
                int_data: 0,
            },
            id: EntityId::new(3, 9),
            status: 1,
        }
    }

    struct Rig {
        stage: StageTree,
        container: NodeId,
        widths: SymbolWidths,
        symbols: ShelterSymbols,
    }

    impl Rig {
        fn new() -> Self {
            let mut stage = StageTree::new();
            let container = stage.add_container(StageTree::ROOT);
            Self {
                stage,
                container,
                widths: widths(),
                symbols: ShelterSymbols::default(),
            }
        }

        fn draw(&mut self, shown: bool, items: &[ItemInShelterData], counter: i32) {
            let d = ShelterDraw {
                shown,
                map_visible: shown,
                alpha: 1.0,
                screen_pos: Vec2::new(500.0, 400.0),
                counter,
                items,
                widths: &self.widths,
                container: self.container,
            };
            self.symbols.draw(&mut self.stage, &d, 0.0);
        }

        fn positions(&self) -> Vec<(f32, f32)> {
            self.symbols
                .items
                .iter()
                .map(|i| (i.pos.x, i.pos.y))
                .collect()
        }
    }

    #[test]
    fn a_rock_and_a_dead_lizard_centre_on_one_row() {
        // `RevealSymbols`: the first pass puts them at x 7 and 34 (row width 49), the
        // second centres them on it: 0 + (14 - 49) / 2 and 19 + (30 - 49) / 2, 30 px
        // under the marker.
        let mut rig = Rig::new();
        rig.draw(true, &[rock(1), dead_lizard()], 0);
        assert_eq!(rig.positions(), vec![(-17.5, -30.0), (9.5, -30.0)]);
        assert_eq!(rig.symbols.items[0].symbol.sprite_name, "Symbol_Rock");
        assert_eq!(rig.symbols.items[1].symbol.sprite_name, "Kill_Green_Lizard");
        // At the marker plus the offset; the shadows two left one down, one left one up.
        let s = rig.stage.sprite(rig.symbols.items[0].symbol.symbol);
        assert_eq!((s.transform.x, s.transform.y), (482.5, 370.0));
        let [a, b] = rig.symbols.items[0].symbol.shadows;
        let a = rig.stage.sprite(a);
        assert_eq!(
            (a.transform.x, a.transform.y, a.color),
            (480.5, 369.0, StageColor::BLACK)
        );
        let b = rig.stage.sprite(b);
        assert_eq!((b.transform.x, b.transform.y), (481.5, 371.0));
    }

    #[test]
    fn rows_wrap_past_200_px() {
        // Fourteen 14 px rocks, 19 px apart: the eleventh takes x to 204, closing row 0;
        // the last three make row 1 (x = 52).
        let mut rig = Rig::new();
        let items: Vec<_> = (0..14).map(rock).collect();
        rig.draw(true, &items, 0);
        let p = rig.positions();
        assert_eq!(rig.symbols.rows_width, vec![204.0, 52.0]);
        assert_eq!(p[0], (7.0 - 102.0, -30.0));
        assert_eq!(p[10], (190.0 + 7.0 - 102.0, -30.0));
        assert_eq!(p[11], (7.0 - 26.0, -60.0));
        assert_eq!(p[13], (38.0 + 7.0 - 26.0, -60.0));
    }

    #[test]
    fn read_when_shown_dropped_when_hidden() {
        let mut rig = Rig::new();
        rig.draw(true, &[rock(1)], 0);
        assert!(rig.symbols.symbols_active);
        rig.draw(true, &[rock(1), rock(2)], 0);
        // Still the items read when it showed.
        assert_eq!(rig.symbols.items.len(), 1);
        rig.draw(false, &[], 0);
        assert!(rig.symbols.items.is_empty() && !rig.symbols.symbols_active);
        rig.draw(true, &[rock(1), rock(2)], 0);
        assert_eq!(rig.symbols.items.len(), 2);
    }

    #[test]
    fn a_dead_creature_pulses_to_very_dark_grey() {
        // At full flash the symbol draws white; status 1 lerps from very dark grey to
        // it by `Pow(Clamp01(0.5 + 0.5 * Sin((counter - 5 * i + t) / 14)), 3)`.
        let mut rig = Rig::new();
        rig.draw(true, &[rock(1), dead_lizard()], 30);
        let pulse = (0.5_f32 + 0.5 * (25.0_f32 / 14.0).sin()).powf(3.0);
        let expected = MenuColor::VeryDarkGrey.rgb().lerp(StageColor::WHITE, pulse);
        let s = rig.stage.sprite(rig.symbols.items[1].symbol.symbol);
        assert!((s.color.r - expected.r).abs() < 1e-5);
        assert!((s.color.g - expected.g).abs() < 1e-5);
        assert_eq!(
            rig.stage.sprite(rig.symbols.items[0].symbol.symbol).color,
            StageColor::WHITE
        );
    }
}
