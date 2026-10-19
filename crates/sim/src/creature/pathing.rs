//! The templates' pathing data from `StaticWorld.InitStaticWorld` and
//! `LizardBreeds.BreedTemplate`: tile and connection resistances
//! (`pathingPreferencesTiles`/`Connections`, built by the `CreatureTemplate`
//! constructor from `TileTypeResistance`/`TileConnectionResistance` lists), `canSwim`,
//! `waterPathingResistance`, `doubleReachUpConnectionParams`, `isTooCloseToTerrain`
//! and the `preBakedPathingCreatures` list. Base game only; Remix's deer setting is off.
//!
//! Mirrors: StaticWorld.cs (InitStaticWorld), CreatureTemplate.cs, LizardBreeds.cs

use std::sync::LazyLock;

use rw_assets::CreatureType;

use super::CreatureTemplate;
use crate::FloatExt;
use crate::world::{Accessibility, AiMap, Legality, MovementType, PathCost};

/// `isTooCloseToTerrain`: 1 rejects the tile, 2 accepts it, 0 goes on to the
/// accessibility class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooCloseToTerrain {
    None,
    /// Rejects tiles whose terrain proximity is below the value (vultures 2, the
    /// leviathan 4).
    ProximityBelow(i32),
    /// Proximity below 3 or a smoothed floor altitude above 17.
    Deer,
    /// Accepts shortcut entrances; rejects proximity below 2 or above 11.
    LongLegs,
    /// Proximity below 2, or high above the floor for its proximity.
    MirosBird,
}

impl TooCloseToTerrain {
    /// The delegate's verdict at `tile` (0, 1 or 2).
    #[must_use]
    pub fn verdict(self, map: &AiMap, tile: (i32, i32)) -> i32 {
        let prox = map.terrain_proximity(tile.0, tile.1);
        match self {
            Self::None => 0,
            Self::ProximityBelow(n) => i32::from(prox < n),
            Self::Deer => {
                i32::from(prox < 3 || map.tile(tile.0, tile.1).smoothed_floor_altitude > 17)
            }
            Self::LongLegs => {
                if map.is_shortcut_entrance(tile) {
                    2
                } else {
                    i32::from(!(2..=11).contains(&prox))
                }
            }
            Self::MirosBird => {
                if prox < 2 {
                    return 1;
                }
                let t = map.tile(tile.0, tile.1);
                let limit = (prox as f32).lerp_map(2.0, 6.0, 6.0, 4.0) * 2.0;
                i32::from(
                    t.smoothed_floor_altitude > 2
                        && (t.smoothed_floor_altitude + t.floor_altitude) as f32 > limit,
                )
            }
        }
    }
}

/// One template's pathing data.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplatePathing {
    tiles: [PathCost; 10],
    connections: [PathCost; 24],
    pub can_swim: bool,
    pub water_pathing_resistance: f32,
    /// `doubleReachUpConnectionParams`: the most open class allowed at the ground tile,
    /// the two tiles between and the destination.
    pub double_reach_up: Option<[Accessibility; 3]>,
    pub too_close_to_terrain: TooCloseToTerrain,
}

/// A resistance list entry.
type Entry<K> = (K, f32, Legality);
/// A lizard breed: its tile and connection resistances and its water pathing resistance.
type Breed = (
    CreatureType,
    Vec<Entry<Accessibility>>,
    Vec<Entry<MovementType>>,
    f32,
);

const A: Legality = Legality::Allowed;

impl TemplatePathing {
    /// The `CreatureTemplate` constructor's part: the ancestor's tables (or the
    /// defaults: illegal tiles at 10 per class, Solid 100, illegal connections at 100),
    /// the template's own entries, then every class up to the most open allowed one of
    /// its own list capped at that one's cost. Other fields are inherited.
    fn build(
        ancestor: Option<&Self>,
        tiles: &[Entry<Accessibility>],
        connections: &[Entry<MovementType>],
    ) -> Self {
        let mut t = ancestor.cloned().unwrap_or_else(|| Self {
            tiles: std::array::from_fn(|i| PathCost::new(10.0 * i as f32, Legality::IllegalTile)),
            connections: [PathCost::new(100.0, Legality::IllegalConnection); 24],
            can_swim: false,
            water_pathing_resistance: 1.0,
            double_reach_up: None,
            too_close_to_terrain: TooCloseToTerrain::None,
        });
        if ancestor.is_none() {
            t.tiles[Accessibility::Solid.index()] = PathCost::new(100.0, Legality::SolidTile);
        }
        t.too_close_to_terrain = TooCloseToTerrain::None;
        let mut max = 0;
        for &(acc, resistance, legality) in tiles {
            let cost = PathCost::new(resistance, legality);
            t.tiles[acc.index()] = cost;
            if cost.legality == Legality::Allowed && max < acc.index() && acc != Accessibility::Sand
            {
                max = acc.index();
            }
        }
        for m in 0..=max {
            if t.tiles[m].greater_than(t.tiles[max]) {
                t.tiles[m] = t.tiles[max];
            }
        }
        for &(kind, resistance, legality) in connections {
            t.connections[kind.index()] = PathCost::new(resistance, legality);
        }
        t
    }

    fn swim(mut self, can_swim: bool, water_pathing_resistance: f32) -> Self {
        self.can_swim = can_swim;
        self.water_pathing_resistance = water_pathing_resistance;
        self
    }

    fn reach_up(mut self, params: [Accessibility; 3]) -> Self {
        self.double_reach_up = Some(params);
        self
    }

    fn too_close(mut self, rule: TooCloseToTerrain) -> Self {
        self.too_close_to_terrain = rule;
        self
    }

    /// `AccessibilityResistance`.
    #[must_use]
    pub fn accessibility_resistance(&self, acc: Accessibility) -> PathCost {
        self.tiles[acc.index()]
    }

    /// `ConnectionResistance`.
    #[must_use]
    pub fn connection_resistance(&self, kind: MovementType) -> PathCost {
        self.connections[kind.index()]
    }

    /// The base game's templates, built in `InitStaticWorld` order.
    #[allow(clippy::too_many_lines)]
    fn base_table() -> Vec<(CreatureType, Self)> {
        use Accessibility as Ac;
        use CreatureType as T;
        use MovementType as M;
        let mut v: Vec<(CreatureType, Self)> = Vec::new();
        let standard = Self::build(
            None,
            &[
                (Ac::OffScreen, 1.0, A),
                (Ac::Floor, 1.0, A),
                (Ac::Corridor, 1.0, A),
                (Ac::Climb, 2.5, A),
            ],
            &[
                (M::Standard, 1.0, A),
                (M::OpenDiagonal, 3.0, A),
                (M::ReachOverGap, 3.0, A),
                (M::ReachUp, 2.0, A),
                (M::ReachDown, 2.0, A),
                (M::SemiDiagonalReach, 2.0, A),
                (M::DropToFloor, 20.0, A),
                (M::DropToWater, 20.0, A),
                (M::ShortCut, 1.5, A),
                (M::NpcTransportation, 25.0, A),
                (M::OffScreenMovement, 1.0, A),
                (M::BetweenRooms, 10.0, A),
                (M::Slope, 1.5, A),
            ],
        )
        .swim(true, 1.0);
        v.push((T::StandardGroundCreature, standard));
        let slugcat = Self::build(
            None,
            &[
                (Ac::Floor, 1.0, A),
                (Ac::Corridor, 1.2, A),
                (Ac::Climb, 1.5, A),
            ],
            &[
                (M::Standard, 1.0, A),
                (M::OpenDiagonal, 2.0, A),
                (M::ReachOverGap, 3.0, A),
                (M::ReachUp, 2.0, A),
                (M::ReachDown, 2.0, A),
                (M::DropToFloor, 3000.0, A),
                (M::DropToClimb, 6000.0, A),
                (M::DropToWater, 4000.0, A),
                (M::ShortCut, 0.9, A),
            ],
        )
        .swim(true, 2.0);
        v.push((T::Slugcat, slugcat));
        let lizard = Self::build(
            None,
            &[
                (Ac::OffScreen, 1.0, A),
                (Ac::Floor, 1.0, A),
                (Ac::Corridor, 1.0, A),
                (Ac::CurvedFloor, 1.0, A),
            ],
            &[
                (M::Standard, 1.0, A),
                (M::OpenDiagonal, 3.0, A),
                (M::ReachOverGap, 3.0, A),
                (M::ReachUp, 2.0, A),
                (M::DoubleReachUp, 2.0, A),
                (M::ReachDown, 2.0, A),
                (M::SemiDiagonalReach, 2.0, A),
                (M::DropToFloor, 20.0, A),
                (M::DropToWater, 20.0, A),
                (M::ShortCut, 1.5, A),
                (M::NpcTransportation, 25.0, A),
                (M::OffScreenMovement, 1.0, A),
                (M::BetweenRooms, 10.0, A),
                (M::LizardTurn, 10.0, A),
                (M::Slope, 1.5, A),
            ],
        )
        .reach_up([Ac::Floor, Ac::Wall, Ac::Floor])
        .swim(true, 1.0);
        v.push((T::LizardTemplate, lizard.clone()));
        let climber = [
            (Ac::Floor, 1.0, A),
            (Ac::Corridor, 1.0, A),
            (Ac::Climb, 2.5, A),
        ];
        let wall_climber = [
            (Ac::Floor, 1.0, A),
            (Ac::Corridor, 1.2, A),
            (Ac::Climb, 0.8, A),
            (Ac::Wall, 1.0, A),
            (Ac::Ceiling, 1.2, A),
        ];
        let breeds: [Breed; 9] = [
            (
                T::PinkLizard,
                climber.to_vec(),
                vec![(M::DropToClimb, 40.0, A)],
                3.0,
            ),
            (
                T::GreenLizard,
                vec![(Ac::Floor, 1.0, A), (Ac::Corridor, 2.0, A)],
                vec![(M::DropToFloor, 40.0, A), (M::LizardTurn, 60.0, A)],
                1.5,
            ),
            (
                T::BlueLizard,
                wall_climber.to_vec(),
                vec![
                    (M::DropToFloor, 20.0, A),
                    (M::DropToClimb, 2.0, A),
                    (M::ShortCut, 15.0, A),
                    (M::ReachOverGap, 1.1, A),
                    (M::ReachUp, 1.1, A),
                    (M::ReachDown, 1.1, A),
                    (M::CeilingSlope, 2.0, A),
                ],
                5.0,
            ),
            (
                T::YellowLizard,
                climber.to_vec(),
                vec![(M::DropToFloor, 4.0, A), (M::DropToClimb, 8.0, A)],
                2.5,
            ),
            (
                T::WhiteLizard,
                vec![
                    (Ac::Floor, 1.0, A),
                    (Ac::Corridor, 1.2, A),
                    (Ac::Climb, 0.8, A),
                    (Ac::Wall, 1.0, A),
                ],
                vec![
                    (M::DropToFloor, 10.0, A),
                    (M::DropToClimb, 10.0, A),
                    (M::ShortCut, 2.0, A),
                    (M::ReachOverGap, 1.1, A),
                    (M::ReachUp, 1.1, A),
                    (M::ReachDown, 1.1, A),
                    (M::CeilingSlope, 20.0, A),
                ],
                8.0,
            ),
            (
                T::RedLizard,
                vec![
                    (Ac::Floor, 1.0, A),
                    (Ac::Corridor, 1.0, A),
                    (Ac::Climb, 1.0, A),
                ],
                vec![(M::DropToClimb, 4.0, A)],
                2.0,
            ),
            (
                T::BlackLizard,
                climber.to_vec(),
                vec![(M::DropToClimb, 40.0, A)],
                3.5,
            ),
            (
                T::Salamander,
                climber.to_vec(),
                vec![(M::DropToClimb, 40.0, A)],
                0.5,
            ),
            (
                T::CyanLizard,
                wall_climber.to_vec(),
                vec![
                    (M::DropToFloor, 2.0, A),
                    (M::DropToClimb, 2.0, A),
                    (M::ShortCut, 5.0, A),
                    (M::ReachOverGap, 1.1, A),
                    (M::ReachUp, 1.1, A),
                    (M::ReachDown, 1.1, A),
                    (M::CeilingSlope, 2.0, A),
                ],
                5.0,
            ),
        ];
        for (kind, tiles, connections, water) in &breeds {
            let breed = Self::build(Some(&lizard), tiles, connections);
            v.push((*kind, breed.swim(lizard.can_swim, *water)));
        }
        let flyer = [
            (M::Standard, 1.0, A),
            (M::ShortCut, 1.0, A),
            (M::NpcTransportation, 10.0, A),
        ];
        let air = [(Ac::Air, 1.0, A)];
        v.push((T::Fly, Self::build(None, &air, &flyer)));
        let leech = Self::build(None, &air, &flyer).swim(true, 1.0);
        v.push((T::SeaLeech, Self::build(Some(&leech), &air, &flyer)));
        v.push((T::Leech, leech));
        let crawler_tiles = [
            (Ac::Floor, 1.0, A),
            (Ac::Corridor, 1.0, A),
            (Ac::Climb, 1.0, A),
            (Ac::Wall, 1.0, A),
            (Ac::Ceiling, 1.0, A),
            (Ac::OffScreen, 1.0, A),
        ];
        let crawler = [
            (M::Standard, 1.0, A),
            (M::OpenDiagonal, 2.0, A),
            (M::ShortCut, 0.2, A),
            (M::NpcTransportation, 20.0, A),
            (M::Slope, 1.6, A),
            (M::CeilingSlope, 1.6, A),
            (M::DropToFloor, 10.0, A),
            (M::OffScreenMovement, 1.0, A),
            (M::BetweenRooms, 10.0, A),
        ];
        v.push((T::Snail, Self::build(None, &crawler_tiles, &crawler)));
        let sky_tiles = [(Ac::Air, 1.0, A), (Ac::OffScreen, 1.0, A)];
        let sky = [
            (M::Standard, 1.0, A),
            (M::OutsideRoom, 1.0, A),
            (M::SkyHighway, 1.0, A),
            (M::OffScreenMovement, 1.0, A),
            (M::BetweenRooms, 10.0, A),
        ];
        let vulture = Self::build(None, &sky_tiles, &sky);
        v.push((
            T::KingVulture,
            Self::build(Some(&vulture), &sky_tiles, &sky)
                .too_close(TooCloseToTerrain::ProximityBelow(2)),
        ));
        let temple_guard_tiles = sky_tiles;
        v.push((
            T::Vulture,
            vulture
                .clone()
                .too_close(TooCloseToTerrain::ProximityBelow(2)),
        ));
        v.push((T::GarbageWorm, Self::build(None, &[], &[])));
        v.push((
            T::LanternMouse,
            Self::build(
                None,
                &[
                    (Ac::OffScreen, 1.0, A),
                    (Ac::Floor, 1.0, A),
                    (Ac::Corridor, 1.0, A),
                    (Ac::Climb, 2.5, A),
                ],
                &[
                    (M::Standard, 1.0, A),
                    (M::OpenDiagonal, 3.0, A),
                    (M::ReachOverGap, 3.0, A),
                    (M::ReachUp, 2.0, A),
                    (M::ReachDown, 2.0, A),
                    (M::SemiDiagonalReach, 2.0, A),
                    (M::DropToFloor, 20.0, A),
                    (M::DropToWater, 20.0, A),
                    (M::ShortCut, 1.5, A),
                    (M::NpcTransportation, 25.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                    (M::Slope, 1.5, A),
                ],
            )
            .swim(true, 1.0),
        ));
        let cicada = Self::build(
            None,
            &air,
            &[
                (M::Standard, 1.0, A),
                (M::ShortCut, 1.0, A),
                (M::NpcTransportation, 10.0, A),
                (M::OffScreenMovement, 1.0, A),
                (M::BetweenRooms, 10.0, A),
            ],
        );
        v.push((T::CicadaB, Self::build(Some(&cicada), &[], &[])));
        v.push((T::CicadaA, cicada));
        v.push((T::Spider, Self::build(None, &crawler_tiles, &crawler)));
        v.push((
            T::JetFish,
            Self::build(
                None,
                &air,
                &[
                    (M::Standard, 1.0, A),
                    (M::ShortCut, 1.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                    (M::NpcTransportation, 10.0, A),
                ],
            )
            .swim(true, 1.0),
        ));
        v.push((
            T::BigEel,
            Self::build(
                None,
                &air,
                &[
                    (M::Standard, 1.0, A),
                    (M::OutsideRoom, 1.0, A),
                    (M::SeaHighway, 100_000.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                ],
            )
            .swim(true, 1.0)
            .too_close(TooCloseToTerrain::ProximityBelow(4)),
        ));
        v.push((
            T::Deer,
            Self::build(
                None,
                &air,
                &[
                    (M::Standard, 1.0, A),
                    (M::OutsideRoom, 1.0, A),
                    (M::SideHighway, 100_000.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                ],
            )
            .too_close(TooCloseToTerrain::Deer),
        ));
        v.push((
            T::TubeWorm,
            Self::build(
                None,
                &[
                    (Ac::OffScreen, 1.0, A),
                    (Ac::Floor, 1.0, A),
                    (Ac::Corridor, 1.0, A),
                ],
                &[
                    (M::Standard, 1.0, A),
                    (M::OpenDiagonal, 3.0, A),
                    (M::ReachOverGap, 3.0, A),
                    (M::ReachUp, 2.0, A),
                    (M::DoubleReachUp, 2.0, A),
                    (M::ReachDown, 2.0, A),
                    (M::SemiDiagonalReach, 2.0, A),
                    (M::DropToFloor, 1.0, A),
                    (M::DropToWater, 1.0, A),
                    (M::ShortCut, 1.5, A),
                    (M::NpcTransportation, 25.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                    (M::Slope, 1.5, A),
                ],
            )
            .reach_up([Ac::Floor, Ac::Wall, Ac::Floor]),
        ));
        let daddy = Self::build(
            None,
            &air,
            &[
                (M::Standard, 1.0, A),
                (M::ShortCut, 1.0, A),
                (M::BigCreatureShortCutSqueeze, 10.0, A),
                (M::OffScreenMovement, 1.0, A),
                (M::BetweenRooms, 10.0, A),
            ],
        );
        v.push((
            T::BrotherLongLegs,
            Self::build(Some(&daddy), &[], &[]).too_close(TooCloseToTerrain::LongLegs),
        ));
        v.push((
            T::DaddyLongLegs,
            daddy.too_close(TooCloseToTerrain::LongLegs),
        ));
        let tentacle = Self::build(None, &[], &[]);
        v.push((T::PoleMimic, Self::build(Some(&tentacle), &[], &[])));
        v.push((T::TentaclePlant, tentacle));
        v.push((
            T::MirosBird,
            Self::build(
                None,
                &air,
                &[
                    (M::Standard, 1.0, A),
                    (M::OutsideRoom, 1.0, A),
                    (M::SideHighway, 100.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                ],
            )
            .too_close(TooCloseToTerrain::MirosBird),
        ));
        v.push((
            T::TempleGuard,
            Self::build(
                None,
                &temple_guard_tiles,
                &[
                    (M::Standard, 1.0, A),
                    (M::BetweenRooms, 10.0, Legality::Unallowed),
                ],
            ),
        ));
        let blue_reach = v
            .iter()
            .find(|(k, _)| *k == T::BlueLizard)
            .and_then(|(_, t)| t.double_reach_up);
        let mut centipede = Self::build(
            None,
            &[
                (Ac::OffScreen, 1.0, A),
                (Ac::Floor, 1.0, A),
                (Ac::Corridor, 1.0, A),
                (Ac::Climb, 1.0, A),
                (Ac::Wall, 1.0, A),
                (Ac::Ceiling, 1.0, A),
            ],
            &[
                (M::Standard, 1.0, A),
                (M::OpenDiagonal, 3.0, A),
                (M::ReachOverGap, 3.0, A),
                (M::DoubleReachUp, 2.0, A),
                (M::SemiDiagonalReach, 2.0, A),
                (M::NpcTransportation, 25.0, A),
                (M::OffScreenMovement, 1.0, A),
                (M::BetweenRooms, 10.0, A),
                (M::Slope, 1.5, A),
                (M::DropToFloor, 5.0, A),
                (M::DropToClimb, 5.0, A),
                (M::ShortCut, 1.0, A),
                (M::ReachUp, 1.1, A),
                (M::ReachDown, 1.1, A),
                (M::CeilingSlope, 2.0, A),
            ],
        )
        .swim(true, 1.0);
        centipede.double_reach_up = blue_reach;
        v.push((T::RedCentipede, Self::build(Some(&centipede), &[], &[])));
        v.push((T::SmallCentipede, Self::build(Some(&centipede), &[], &[])));
        v.push((
            T::Centiwing,
            Self::build(
                Some(&centipede),
                &air,
                &[
                    (M::Standard, 1.0, A),
                    (M::ShortCut, 1.0, A),
                    (M::NpcTransportation, 10.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 10.0, A),
                ],
            ),
        ));
        v.push((T::Centipede, centipede));
        v.push((
            T::Scavenger,
            Self::build(
                None,
                &[
                    (Ac::OffScreen, 1.0, A),
                    (Ac::Floor, 1.0, A),
                    (Ac::Corridor, 1.0, A),
                    (Ac::Climb, 2.5, A),
                ],
                &[
                    (M::Standard, 1.0, A),
                    (M::OpenDiagonal, 3.0, A),
                    (M::ReachOverGap, 3.0, A),
                    (M::ReachUp, 2.0, A),
                    (M::DoubleReachUp, 2.0, A),
                    (M::ReachDown, 2.0, A),
                    (M::SemiDiagonalReach, 2.0, A),
                    (M::DropToFloor, 10.0, A),
                    (M::DropToClimb, 20.0, A),
                    (M::DropToWater, 10.0, A),
                    (M::ShortCut, 2.5, A),
                    (M::NpcTransportation, 225.0, A),
                    (M::OffScreenMovement, 1.2, A),
                    (M::BetweenRooms, 15.0, A),
                    (M::Slope, 1.5, A),
                    (M::RegionTransportation, 400.0, A),
                ],
            )
            .reach_up([Ac::Climb, Ac::Air, Ac::Climb])
            .swim(true, 1.0),
        ));
        v.push((T::Overseer, Self::build(None, &[], &[])));
        let grub = Self::build(None, &[], &[]);
        v.push((T::Hazer, Self::build(Some(&grub), &[], &[])));
        v.push((T::VultureGrub, grub));
        v.push((
            T::EggBug,
            Self::build(
                None,
                &[
                    (Ac::OffScreen, 1.0, A),
                    (Ac::Floor, 1.0, A),
                    (Ac::Corridor, 1.0, A),
                    (Ac::Climb, 2.5, A),
                    (Ac::Wall, 50.0, A),
                ],
                &[
                    (M::Standard, 1.0, A),
                    (M::OpenDiagonal, 3.0, A),
                    (M::ReachOverGap, 3.0, A),
                    (M::ReachUp, 2.0, A),
                    (M::ReachDown, 2.0, A),
                    (M::SemiDiagonalReach, 2.0, A),
                    (M::DropToFloor, 10.0, A),
                    (M::DropToWater, 10.0, Legality::Unwanted),
                    (M::DropToClimb, 10.0, Legality::Unwanted),
                    (M::ShortCut, 1.5, A),
                    (M::NpcTransportation, 3.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 5.0, A),
                    (M::Slope, 1.5, A),
                ],
            )
            .swim(true, 40.0),
        ));
        let spider_tiles = [
            (Ac::OffScreen, 1.0, A),
            (Ac::Floor, 1.0, A),
            (Ac::Corridor, 1.0, A),
            (Ac::Climb, 1.5, A),
            (Ac::Wall, 3.0, A),
            (Ac::Ceiling, 3.0, A),
        ];
        let spider = [
            (M::Standard, 1.0, A),
            (M::OpenDiagonal, 3.0, A),
            (M::ReachOverGap, 3.0, A),
            (M::ReachUp, 2.0, A),
            (M::ReachDown, 2.0, A),
            (M::SemiDiagonalReach, 2.0, A),
            (M::DropToFloor, 10.0, A),
            (M::DropToWater, 10.0, Legality::Unwanted),
            (M::DropToClimb, 10.0, Legality::Unwanted),
            (M::ShortCut, 1.5, A),
            (M::NpcTransportation, 3.0, A),
            (M::OffScreenMovement, 1.0, A),
            (M::BetweenRooms, 5.0, A),
            (M::Slope, 1.5, A),
            (M::CeilingSlope, 1.5, A),
        ];
        let big_spider = Self::build(None, &spider_tiles, &spider).swim(true, 5.0);
        // The spitter's constructor gets the same lists again.
        v.push((
            T::SpitterSpider,
            Self::build(Some(&big_spider), &spider_tiles, &spider),
        ));
        v.push((T::BigSpider, big_spider));
        let needle = [
            (M::Standard, 1.0, A),
            (M::OpenDiagonal, 1.0, A),
            (M::ShortCut, 1.0, A),
            (M::NpcTransportation, 10.0, A),
            (M::OffScreenMovement, 1.0, A),
            (M::BetweenRooms, 1.0, A),
        ];
        let big_needle = Self::build(None, &air, &needle);
        v.push((
            T::SmallNeedleWorm,
            Self::build(Some(&big_needle), &air, &needle),
        ));
        v.push((T::BigNeedleWorm, big_needle));
        v.push((
            T::DropBug,
            Self::build(
                None,
                &spider_tiles,
                &[
                    (M::Standard, 1.0, A),
                    (M::OpenDiagonal, 3.0, A),
                    (M::ReachOverGap, 3.0, A),
                    (M::ReachUp, 2.0, A),
                    (M::ReachDown, 2.0, A),
                    (M::SemiDiagonalReach, 2.0, A),
                    (M::DropToFloor, 10.0, A),
                    (M::DropToWater, 10.0, A),
                    (M::ShortCut, 1.5, A),
                    (M::NpcTransportation, 3.0, A),
                    (M::OffScreenMovement, 1.0, A),
                    (M::BetweenRooms, 5.0, A),
                    (M::Slope, 1.5, A),
                    (M::CeilingSlope, 1.5, A),
                ],
            )
            .swim(true, 2.0),
        ));
        v
    }

    /// The table for `kind`; unregistered types get the stand-in the game fills empty
    /// slots with (a `StandardGroundCreature` built from empty lists).
    #[must_use]
    pub fn of(kind: CreatureType) -> &'static Self {
        static TABLE: LazyLock<Vec<(CreatureType, TemplatePathing)>> =
            LazyLock::new(TemplatePathing::base_table);
        static STAND_IN: LazyLock<TemplatePathing> =
            LazyLock::new(|| TemplatePathing::build(None, &[], &[]));
        TABLE
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(&*STAND_IN, |(_, t)| t)
    }
}

impl CreatureTemplate {
    /// `StaticWorld.preBakedPathingCreatures`: the templates with `doPreBakedPathing`,
    /// in type order; the index is `PreBakedPathingIndex` and the room files' layer.
    pub const PRE_BAKED_PATHING_CREATURES: [CreatureType; 15] = [
        CreatureType::StandardGroundCreature,
        CreatureType::PinkLizard,
        CreatureType::GreenLizard,
        CreatureType::BlueLizard,
        CreatureType::WhiteLizard,
        CreatureType::Salamander,
        CreatureType::Fly,
        CreatureType::Leech,
        CreatureType::Snail,
        CreatureType::Vulture,
        CreatureType::BigEel,
        CreatureType::Deer,
        CreatureType::DaddyLongLegs,
        CreatureType::MirosBird,
        CreatureType::Scavenger,
    ];

    #[must_use]
    pub fn pathing(&self) -> &'static TemplatePathing {
        TemplatePathing::of(self.kind)
    }

    /// `AccessibilityResistance`.
    #[must_use]
    pub fn accessibility_resistance(&self, acc: Accessibility) -> PathCost {
        self.pathing().accessibility_resistance(acc)
    }

    /// `ConnectionResistance`.
    #[must_use]
    pub fn connection_resistance(&self, kind: MovementType) -> PathCost {
        self.pathing().connection_resistance(kind)
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::{ActiveMods, NodeType};

    use super::*;

    fn template(kind: CreatureType) -> CreatureTemplate {
        CreatureTemplate::of(kind, ActiveMods::NONE)
    }

    /// The green lizard: its own Floor and Corridor, the lizard template's OffScreen
    /// and CurvedFloor, Climb left at the default illegal cost (40) since neither list
    /// allows it.
    #[test]
    fn green_lizard_tables() {
        let g = template(CreatureType::GreenLizard);
        let p = g.pathing();
        assert_eq!(
            g.accessibility_resistance(Accessibility::Floor),
            PathCost::new(1.0, A)
        );
        assert_eq!(
            g.accessibility_resistance(Accessibility::Corridor),
            PathCost::new(2.0, A)
        );
        assert_eq!(
            g.accessibility_resistance(Accessibility::OffScreen),
            PathCost::new(1.0, A)
        );
        assert_eq!(
            g.accessibility_resistance(Accessibility::CurvedFloor),
            PathCost::new(1.0, A)
        );
        assert_eq!(
            g.accessibility_resistance(Accessibility::Climb),
            PathCost::new(40.0, Legality::IllegalTile)
        );
        assert_eq!(
            g.accessibility_resistance(Accessibility::Solid),
            PathCost::new(100.0, Legality::SolidTile)
        );
        assert_eq!(
            g.connection_resistance(MovementType::DropToFloor),
            PathCost::new(40.0, A)
        );
        assert_eq!(
            g.connection_resistance(MovementType::LizardTurn),
            PathCost::new(60.0, A)
        );
        assert_eq!(
            g.connection_resistance(MovementType::ShortCut),
            PathCost::new(1.5, A)
        );
        assert!(!g.connection_resistance(MovementType::DropToClimb).allowed());
        assert!(
            !g.connection_resistance(MovementType::CeilingSlope)
                .allowed()
        );
        assert_eq!(p.water_pathing_resistance, 1.5);
        assert!(p.can_swim);
        assert_eq!(
            p.double_reach_up,
            Some([
                Accessibility::Floor,
                Accessibility::Wall,
                Accessibility::Floor
            ])
        );
    }

    /// The Fly allows only Air in its list; the cap makes every less open class cost
    /// what Air does, so all but Solid and Sand are allowed.
    #[test]
    fn the_fly_allows_every_class_below_air() {
        let f = template(CreatureType::Fly);
        for acc in &Accessibility::ALL[..8] {
            assert_eq!(
                f.accessibility_resistance(*acc),
                PathCost::new(1.0, A),
                "{acc:?}"
            );
        }
        assert!(!f.accessibility_resistance(Accessibility::Solid).allowed());
        assert!(!f.accessibility_resistance(Accessibility::Sand).allowed());
    }

    /// The constructor's node types follow the connection resistances (`SetNodeType`),
    /// which the abstract layer already reads from the template's own flags.
    #[test]
    fn mapped_node_types_follow_the_connections() {
        for kind in &CreatureType::registered(ActiveMods::NONE) {
            let t = template(*kind);
            let p = t.pathing();
            if *kind == CreatureType::GarbageWorm {
                continue;
            }
            let shortcut = p.connection_resistance(MovementType::ShortCut).allowed();
            assert_eq!(t.maps_node(NodeType::Exit), shortcut, "{kind:?}");
            assert_eq!(
                t.maps_node(NodeType::SkyExit),
                p.connection_resistance(MovementType::SkyHighway).allowed(),
                "{kind:?}"
            );
            assert_eq!(
                t.maps_node(NodeType::SeaExit),
                p.connection_resistance(MovementType::SeaHighway).allowed(),
                "{kind:?}"
            );
            assert_eq!(
                t.maps_node(NodeType::SideExit),
                p.connection_resistance(MovementType::SideHighway).allowed(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn the_pre_baked_list_matches_the_pathing_indices() {
        for (i, kind) in CreatureTemplate::PRE_BAKED_PATHING_CREATURES
            .iter()
            .enumerate()
        {
            assert_eq!(
                template(*kind).pre_baked_pathing_index(),
                i32::try_from(i).expect("index")
            );
        }
    }
}
