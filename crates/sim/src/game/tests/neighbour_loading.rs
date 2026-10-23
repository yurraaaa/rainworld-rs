//! A second room realized during a tick under the game's minimum loading schedule: its
//! stages at the end of the next two ticks, its creatures with `ReadyForAI`, the lobby's
//! `CreatureAllowedInRoom` at each stage, `AbstractCreature.Abstractize` in a room not
//! `readyForAI`, a room abstracted while it loads, and `Room.Loaded`'s items after its
//! effects. Expected values from `Room.cs` (`ShortCutsReady`, `ReadyForAI`, `Loaded`),
//! `RoomPreparer.cs`, `ShortcutHandler.cs` (`VesselAllowedInRoom`,
//! `CreatureAllowedInRoom`) and `AbstractCreature.cs` (`Abstractize`,
//! `RequiresAIMapToEnterRoom`).
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{
    AbstractObjectType, ActiveMods, CreatureType, PlacedObject, RoomEffect, RoomSettings,
};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::CreatureId;

use crate::tests::room;
use crate::world::World;
use crate::{Input, RainWorldGame, Sounds, Start};

/// A 40 x 20 room walled round, with three rows of floor.
fn rows() -> Vec<String> {
    let mut rows: Vec<String> = vec![format!("#{}#", ".".repeat(38)); 20];
    rows[0] = "#".repeat(40);
    for row in &mut rows[17..] {
        *row = "#".repeat(40);
    }
    rows
}

/// A Survivor story in a room (global 0) with `settings`, and a copy of it as global room
/// 2 (1 is the off-screen den), the player at (2, 3); no tick run.
fn story(settings: RoomSettings) -> RainWorldGame {
    let rows = rows();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let room = room(&refs);
    let mut data = WorldData::from_room(&room);
    let mut second = data.rooms[0].clone();
    second.name = "ROOM_B".to_owned();
    second.index = 2;
    data.rooms.push(second);
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 3)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    )
}

fn plain() -> RoomSettings {
    RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    }
}

/// [`story`] in plain rooms, the start room loaded.
fn loaded_story() -> RainWorldGame {
    let mut g = story(plain());
    g.tick_until_ready(&mut ());
    g
}

impl RainWorldGame {
    fn progress_of(&self, index: i32) -> u8 {
        self.world
            .realized(index)
            .expect("realized")
            .loading_progress()
    }

    /// An abstract creature of `kind` at (20, 3) of room 2.
    fn creature_in_second_room(&mut self, kind: CreatureType) -> CreatureId {
        let id = self
            .dev_spawn_creature(kind, 2, (20, 3), None, &mut ())
            .expect("spawns");
        assert!(!self.world.creature(id).is_realized());
        id
    }

    /// `VesselAllowedInRoom` into room 2 of a vessel carrying each of `ids`.
    fn allowed_into_second_room(&self, ids: [CreatureId; 3]) -> [bool; 3] {
        ids.map(|id| self.world.vessel_allowed_in_room(id, 2))
    }

    fn dandelion_places(&self) -> Vec<crate::Vec2> {
        self.world
            .realized(0)
            .unwrap()
            .particles
            .dandelions
            .iter()
            .map(|d| d.pos)
            .collect()
    }
}

/// A room with the `SkyDandelions` effect (a patch: the cycle's start scatters none) and,
/// with `item`, a `MultiplayerItem` rock (chance 1).
fn dandelions(item: bool) -> RoomSettings {
    RoomSettings {
        effects: vec![RoomEffect {
            kind: "SkyDandelions".to_owned(),
            amount: 0.0,
        }],
        placed_objects: PlacedObject::parse_list(if item {
            "DandelionPatch><200><70><100~0, MultiplayerItem><100><70><Rock~0~0~1"
        } else {
            "DandelionPatch><200><70><100~0"
        }),
        ..plain()
    }
}

/// Room 2 realized before tick N's loading step (as the realizer or the lobby does
/// mid-tick): that step starts its thread, N+1's hands over `ShortCutsReady`, N+2's
/// `ReadyForAI`, which realizes the lizard waiting there.
#[test]
fn a_neighbour_loads_a_stage_a_tick_and_its_creatures_come_with_stage_two() {
    let mut g = loaded_story();
    let lizard = g.creature_in_second_room(CreatureType::GreenLizard);
    g.activate_room(2).expect("loads");
    let mut seen = Vec::new();
    for _ in 0..3 {
        g.tick(Input::NONE, &mut ());
        seen.push((g.progress_of(2), g.world.creature(lizard).is_realized()));
    }
    assert_eq!(seen, [(0, false), (1, false), (3, true)]);
}

/// `VesselAllowedInRoom` into room 2 at each stage. Stage 1 in a room without a creature
/// needing the AI map opens it to creatures without `AI` (`ReadyForPlayer` for the
/// slugcat); a lizard (`requireAImap`) waits for `readyForAI`; a vulture grub (no `AI`, no
/// `requireAImap`) enters with stage 1.
#[test]
fn the_lobby_holds_a_creature_needing_the_ai_map_until_ready_for_ai() {
    let mut g = loaded_story();
    let lizard = g.creature_in_second_room(CreatureType::GreenLizard);
    let grub = g.creature_in_second_room(CreatureType::VultureGrub);
    // Out of the room, so stage 1 does not wait for it (`flag2`).
    g.world
        .move_creature(lizard, crate::WorldCoordinate::new(0, 20, 3, -1));
    let ids = [g.player_id, lizard, grub];
    assert_eq!(g.allowed_into_second_room(ids), [false; 3]);
    g.activate_room(2).expect("loads");
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.allowed_into_second_room(ids), [false; 3]);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.progress_of(2), 1);
    assert_eq!(g.allowed_into_second_room(ids), [true, false, true]);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.allowed_into_second_room(ids), [true; 3]);
}

/// `Abstractize` of a room whose AI map is not ready: a lizard's body is just dropped
/// (`realizedCreature = null`, no `Move`, no draw).
#[test]
fn abstractizing_a_room_not_ready_for_ai_drops_the_bodies() {
    let mut g = loaded_story();
    let lizard = g.creature_in_second_room(CreatureType::GreenLizard);
    g.activate_room(2).expect("loads");
    g.tick(Input::NONE, &mut ());
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.progress_of(2), 1);
    assert!(g.world.realize_lizard(lizard, &mut g.rng));
    let pos = g.world.creature(lizard).pos;
    let state = g.rng.state();
    g.world.abstractize(2, &mut g.rng);
    let c = g.world.creature(lizard);
    assert!(!c.is_realized());
    assert_eq!(c.pos, pos);
    assert_eq!(g.rng.state(), state);
}

/// A room abstracted after its thread started keeps its preparer, whose stages realize
/// nothing (D14): the lizard waiting there stays abstract, and the ticks draw as in a
/// game that never realized the room.
#[test]
fn a_room_abstracted_while_it_loads_realizes_nothing() {
    let mut g = loaded_story();
    let mut never = loaded_story();
    let lizard = g.creature_in_second_room(CreatureType::GreenLizard);
    never.creature_in_second_room(CreatureType::GreenLizard);
    g.activate_room(2).expect("loads");
    g.tick(Input::NONE, &mut ());
    never.tick(Input::NONE, &mut ());
    g.world.abstractize(2, &mut g.rng);
    assert_eq!(g.world.orphaned_preparers().count(), 1);
    // From here the two games differ only by the orphaned preparer.
    never.rng = g.rng.clone();
    for _ in 0..4 {
        g.tick(Input::NONE, &mut ());
        never.tick(Input::NONE, &mut ());
        assert_eq!(g.rng.state(), never.rng.state());
    }
    assert!(!g.world.creature(lizard).is_realized());
    assert!(g.world.realized(2).is_none());
    assert_eq!(g.world.orphaned_preparers().count(), 0);
}

/// `Room.Loaded`'s effects loop (the sky dandelions' scatter draws) comes before its
/// placed-object loop (a story's `MultiplayerItem`, one draw against its chance, an ID):
/// the dandelions fall where they do without the item, and the item's draw follows.
#[test]
fn loaded_makes_the_effects_before_the_items() {
    let (a, b) = (story(dandelions(true)), story(dandelions(false)));
    assert!(!b.dandelion_places().is_empty());
    assert_eq!(a.dandelion_places(), b.dandelion_places());
    let mut expect = b.rng.clone();
    expect.value();
    assert_eq!(a.rng.state(), expect.state());
    let rocks = a
        .world
        .room_objects(0)
        .filter(|&id| a.world.object(id).kind == AbstractObjectType::Rock)
        .count();
    assert_eq!(rocks, 1);
}
