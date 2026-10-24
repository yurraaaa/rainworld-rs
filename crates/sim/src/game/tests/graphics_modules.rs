//! The graphics modules across camera moves (`RoomCamera.ChangeRoom`): `Room.NoLongerViewed`
//! drops the modules in the old room's `drawableObjects`, `Room.NowViewed` runs
//! `InitiateGraphicsModule` over `physicalObjects[0]`, then `[1]` (`Fly`, `Overseer`:
//! `collisionLayer = 0`; `Lizard`: 1), and `CleanOutObjectNotInThisRoom` takes a module
//! out of the room its owner left without dropping it. Room 1 stands for a room the
//! camera leaves; it has no objects.

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::fly::{FlyRoomTick, FlyView};
use crate::creature::lizard::{LizardGraphicsLink, LizardView};
use crate::creature::{
    CreatureId, CreatureTemplate, EntityId, Grasp, RealizedCreature, Shareability, WorldCoordinate,
};
use crate::game::OverseerCall;
use crate::item::Entity;
use crate::tests::room;
use crate::world::{LizardRoomCx, World};
use crate::{
    Input, Player, PlayerObserver, PlayerSurroundings, RainWorldGame, Sounds, Start, Water,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    FlyAdded(EntityId),
    FlyRemoved(EntityId),
    /// The module's serial as the call left it.
    LizardAdded(EntityId, u64),
    LizardRemoved(EntityId),
    OverseerRemoved(EntityId),
}

/// Records the module calls; a lizard link without a serial is a new module.
#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    serial: u64,
}

impl PlayerObserver for Recorder {
    fn created(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn updated(
        &mut self,
        _: &Player,
        _: &Room,
        _: Option<&Water>,
        _: &PlayerSurroundings<'_>,
        _: &mut UnityRandom,
    ) {
    }
    fn spat_out(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn fly_added(&mut self, view: &FlyView<'_>, _: &mut UnityRandom) {
        self.calls.push(Call::FlyAdded(view.id));
    }
    fn fly_removed(&mut self, id: EntityId) {
        self.calls.push(Call::FlyRemoved(id));
    }
    fn builds_lizard_graphics(&self) -> bool {
        true
    }
    fn lizard_added(
        &mut self,
        view: &LizardView<'_>,
        link: &mut LizardGraphicsLink,
        _: &mut UnityRandom,
    ) {
        if link.module == 0 {
            self.serial += 1;
            link.module = self.serial;
        }
        self.calls.push(Call::LizardAdded(view.id, link.module));
    }
    fn lizard_removed(&mut self, id: EntityId) {
        self.calls.push(Call::LizardRemoved(id));
    }
    fn overseer_removed(&mut self, id: EntityId) -> Option<crate::OwnerKey> {
        self.calls.push(Call::OverseerRemoved(id));
        None
    }
}

/// A Survivor story in a 20 x 8 walled room, the player at (2, 1), after 20 ticks.
fn game() -> RainWorldGame {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        danger_type: Some(rw_assets::DangerType::None),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

impl RainWorldGame {
    fn abstract_creature(&mut self, kind: CreatureType, tile: (i32, i32)) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(kind, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let at = WorldCoordinate::new(0, tile.0, tile.1, -1);
        let c = self
            .world
            .new_creature(template, at, id, &session, &mut self.ids, &mut self.rng);
        let c = self.world.insert_creature(c);
        self.world.add_entity(c);
        c
    }

    /// A green lizard placed in room 0 (`PlaceInRoom`) while room `viewed` is viewed.
    fn module_test_lizard(&mut self, viewed: i32, observer: &mut Recorder) -> CreatureId {
        let l = self.abstract_creature(CreatureType::GreenLizard, (10, 3));
        assert!(self.world.realize_lizard(l, &mut self.rng));
        let mut cx = LizardRoomCx {
            rng: &mut self.rng,
            observer,
            camera_room: viewed,
        };
        self.world.place_lizard(l, 0, &mut cx);
        l
    }

    /// A fly realized in room 0 while room `viewed` is viewed.
    fn module_test_fly(&mut self, viewed: i32, observer: &mut Recorder) -> CreatureId {
        let f = self.abstract_creature(CreatureType::Fly, (14, 3));
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer,
            camera_room: viewed,
        };
        self.world.realize_fly_in_room(f, 0, &mut cx);
        f
    }
}

/// The camera comes to room 0 from room 1: the lizard joined the room before the fly,
/// but `NowViewed` builds the fly's module (layer 0) before the lizard's (layer 1).
#[test]
fn now_viewed_builds_layer_0_before_layer_1() {
    let mut g = game();
    let mut rec = Recorder::default();
    let l = g.module_test_lizard(1, &mut rec);
    let f = g.module_test_fly(1, &mut rec);
    assert!(
        g.world.lizard_ref(l).update_order < g.world.fly_ref(f).update_order,
        "the lizard joined first"
    );
    assert!(rec.calls.is_empty(), "no module in a room not viewed");
    g.modules_no_longer_viewed(1, &mut rec);
    g.modules_now_viewed(0, &mut rec);
    let (lid, fid) = (g.world.creature(l).id, g.world.creature(f).id);
    assert_eq!(
        rec.calls,
        vec![Call::FlyAdded(fid), Call::LizardAdded(lid, 1)]
    );
    assert_eq!(g.now_viewed_modules(), &[f, l]);
}

/// A lizard with a module leaves viewed room 1 (`CleanOutObjectNotInThisRoom` takes the
/// module out of room 1's `drawableObjects`) and joins room 0 (`AddObject`: the module
/// is room 0's drawable). The camera's move from 1 to 0 neither drops nor rebuilds it:
/// `InitiateGraphicsModule` only resets it, and it joined room 0 with `AddObject`.
#[test]
fn a_lizard_keeps_its_module_into_another_room() {
    let mut g = game();
    let mut rec = Recorder::default();
    let l = g.module_test_lizard(1, &mut rec);
    let lid = g.world.creature(l).id;
    g.world.lizard_at(l).graphics = Some(LizardGraphicsLink {
        module: 7,
        drawn_in: vec![1],
        ..LizardGraphicsLink::default()
    });
    g.world.lizard_at(l).room = Some(1);
    g.world.lizard_at(l).leave_room();
    assert!(
        g.world
            .lizard_ref(l)
            .graphics
            .as_ref()
            .unwrap()
            .drawn_in
            .is_empty()
    );
    let mut cx = LizardRoomCx {
        rng: &mut g.rng,
        observer: &mut rec,
        camera_room: 1,
    };
    g.world.lizard_add_object(l, 0, &mut cx);
    assert!(rec.calls.is_empty(), "AddObject keeps the module");
    g.modules_no_longer_viewed(1, &mut rec);
    g.modules_now_viewed(0, &mut rec);
    assert_eq!(rec.calls, vec![Call::LizardAdded(lid, 7)]);
    assert_eq!(
        g.world.lizard_ref(l).graphics.as_ref().unwrap().drawn_in,
        vec![0]
    );
    assert!(g.now_viewed_modules().is_empty());
}

/// A lizard still in the room the camera leaves loses its module there
/// (`RemoveGraphicsModule`), and coming back builds a new one.
#[test]
fn a_lizard_in_the_left_room_loses_its_module() {
    let mut g = game();
    let mut rec = Recorder::default();
    let l = g.module_test_lizard(0, &mut rec);
    let lid = g.world.creature(l).id;
    assert_eq!(
        rec.calls,
        vec![Call::LizardAdded(lid, 1)],
        "AddObject in a viewed room"
    );
    g.modules_no_longer_viewed(0, &mut rec);
    g.modules_now_viewed(1, &mut rec);
    assert!(g.world.lizard_ref(l).graphics.is_none());
    g.modules_no_longer_viewed(1, &mut rec);
    g.modules_now_viewed(0, &mut rec);
    assert_eq!(
        rec.calls[1..],
        [Call::LizardRemoved(lid), Call::LizardAdded(lid, 2)]
    );
}

/// A fly that came into room 0 with its module keeps it, and is not rebuilt (no
/// `FlyGraphics` constructor draws); one still in the left room is dropped.
#[test]
fn a_fly_keeps_its_module_into_another_room() {
    let mut g = game();
    let mut rec = Recorder::default();
    let f = g.module_test_fly(0, &mut rec);
    let fid = g.world.creature(f).id;
    assert_eq!(rec.calls, vec![Call::FlyAdded(fid)]);
    g.modules_no_longer_viewed(1, &mut rec);
    g.modules_now_viewed(0, &mut rec);
    assert_eq!(rec.calls.len(), 1, "kept: neither dropped nor rebuilt");
    assert!(g.now_viewed_modules().is_empty());
    g.modules_no_longer_viewed(0, &mut rec);
    g.modules_now_viewed(1, &mut rec);
    assert_eq!(rec.calls[1..], [Call::FlyRemoved(fid)]);
}

/// `Creature.PlaceInRoom` in a viewed room: `AddObject` of the lizard (its module), its
/// chunks' draws, then `AddObject` of the fly it holds: a later place in the update list
/// and the fly's module.
#[test]
fn a_lizard_placed_holding_a_fly_adds_the_fly_after_it() {
    let mut g = game();
    let mut rec = Recorder::default();
    let f = g.abstract_creature(CreatureType::Fly, (10, 3));
    g.world.realize_fly(f, &mut g.rng);
    let l = g.abstract_creature(CreatureType::GreenLizard, (10, 3));
    assert!(g.world.realize_lizard(l, &mut g.rng));
    g.world.creature_mut(l).grasps[0] = Some(Grasp {
        serial: 0,
        grabber: l,
        grabbed: Entity::Creature(f),
        grasp_used: 0,
        chunk_grabbed: 0,
        shareability: Shareability::CanNotShare,
        dominance: 0.5,
        pacifying: false,
    });
    let mut cx = LizardRoomCx {
        rng: &mut g.rng,
        observer: &mut rec,
        camera_room: 0,
    };
    g.world.place_lizard(l, 0, &mut cx);
    let (lid, fid) = (g.world.creature(l).id, g.world.creature(f).id);
    assert_eq!(
        rec.calls,
        vec![Call::LizardAdded(lid, 1), Call::FlyAdded(fid)]
    );
    assert_eq!(g.world.fly_ref(f).room, Some(0));
    assert!(g.world.fly_ref(f).update_order > g.world.lizard_ref(l).update_order);
}

/// An overseer out of its room in a teleport lobby has no place in `drawableObjects`
/// and keeps its module at `NoLongerViewed`; a destroyed one's module goes with it.
#[test]
fn an_overseer_in_the_lobby_keeps_its_module_and_a_destroyed_one_does_not() {
    let mut g = game();
    let mut rec = Recorder::default();
    let o = g.abstract_creature(CreatureType::Overseer, (10, 4));
    assert!(g.world.realize_overseer(o, 0, &mut g.rng));
    g.run_overseer(o, OverseerCall::Place, &mut ());
    let oid = g.world.creature(o).id;
    let Some(RealizedCreature::Overseer(b)) = &mut g.world.creature_mut(o).realized else {
        panic!("realized");
    };
    b.in_room = false;
    g.modules_no_longer_viewed(0, &mut rec);
    g.modules_now_viewed(1, &mut rec);
    assert!(rec.calls.is_empty(), "in the lobby: kept");
    let Some(RealizedCreature::Overseer(b)) = &mut g.world.creature_mut(o).realized else {
        panic!("realized");
    };
    b.slated = true;
    g.modules_no_longer_viewed(0, &mut rec);
    g.modules_now_viewed(1, &mut rec);
    assert_eq!(rec.calls, vec![Call::OverseerRemoved(oid)]);
}

/// `MoveCamera` only queues the move (`loadingRoom`); `ChangeRoom`, so `NoLongerViewed`,
/// runs from `ApplyPositionChange` in the camera's next update, the head of the next tick.
/// Until then the old room's creatures keep their modules.
#[test]
fn a_camera_move_drops_the_old_rooms_modules_at_the_next_tick() {
    let mut g = game();
    let mut rec = Recorder::default();
    let l = g.module_test_lizard(0, &mut rec);
    let f = g.module_test_fly(0, &mut rec);
    let (lid, fid) = (g.world.creature(l).id, g.world.creature(f).id);
    assert_eq!(
        rec.calls,
        vec![Call::LizardAdded(lid, 1), Call::FlyAdded(fid)]
    );
    g.camera_moves_to(1, 0);
    assert!(
        g.world.lizard_ref(l).graphics.is_some(),
        "still drawn the move's tick"
    );
    assert!(g.world.fly_ref(f).graphics);
    assert_eq!(rec.calls.len(), 2, "nothing dropped yet");
    g.tick(Input::NONE, &mut rec);
    assert!(g.world.lizard_ref(l).graphics.is_none());
    assert!(!g.world.fly_ref(f).graphics);
    assert!(rec.calls.contains(&Call::LizardRemoved(lid)));
    assert!(rec.calls.contains(&Call::FlyRemoved(fid)));
}
