//! The colour an entrance flashes in (`ShortcutGraphics.ColorEntrance`): the
//! creature's `ShortCutColor()`, from `Creature.NewTile` (`Creature.cs` 806) and the
//! between-rooms lobby (`ShortcutHandler.cs` 278). `Lizard.ShortCutColor` is its
//! `effectColor`; the default is the template's `shortcutColor` (`CreatureTemplate`
//! constructor: white; `StaticWorld.InitStaticWorld` sets DaddyLongLegs blue,
//! BrotherLongLegs (0.7, 0.7, 0.4), RedCentipede red, BigNeedleWorm (1, 0.5, 0.5),
//! which SmallNeedleWorm inherits).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::{CreatureId, CreatureTemplate, ShortCutColor, WorldCoordinate};
use crate::game::shortcut::{CreatureVessel, LobbyVessel, ShortcutVessel, VesselOwner};
use crate::tests::room;
use crate::world::World;
use crate::{EntranceColor, Input, RainWorldGame, Sounds, Start};

/// A room whose exit at (0, 3) (entrance (1, 3), hole right) leads back into the same
/// room, so the room behind an exit is the camera's.
const LOOP: [&str; 6] = [
    "####################",
    "#..................#",
    "XE.................#",
    "##.................#",
    "#..................#",
    "####################",
];

/// A Survivor story in [`LOOP`] with the player at (15, 3), after 20 ticks.
fn game() -> RainWorldGame {
    let room = room(&LOOP);
    let mut data = WorldData::from_room(&room);
    data.rooms[0].connections[0] = 0;
    let settings = RoomSettings {
        random_item_density: 0.0,
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
            tile: Some((15, 3)),
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
    /// A creature of `kind` at `tile` of room 0, realized and placed when a lizard.
    fn colour_test_creature(&mut self, kind: CreatureType, tile: (i32, i32)) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(kind, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let at = WorldCoordinate::new(0, tile.0, tile.1, -1);
        let c = self
            .world
            .new_creature(template, at, id, &session, &mut self.ids, &mut self.rng);
        let l = self.world.insert_creature(c);
        self.world.add_entity(l);
        if template.top_ancestor() == CreatureType::LizardTemplate {
            assert!(self.world.realize_lizard(l, &mut self.rng));
            let mut cx = crate::world::LizardRoomCx {
                rng: &mut self.rng,
                observer: &mut (),
                camera_room: -1,
            };
            self.world.place_lizard(l, 0, &mut cx);
        }
        l
    }
}

#[test]
fn a_lizard_stepping_toward_a_room_exit_flashes_it_in_its_effect_colour() {
    let mut g = game();
    let l = g.colour_test_creature(CreatureType::GreenLizard, (3, 3));
    let effect = g.world.creature(l).lizard().expect("realized").effect_color;
    assert_ne!(effect, [1.0; 3]);
    let last = WorldCoordinate::new(0, 4, 3, -1);
    let c = g.world.new_tile(l, last, g.camera_room);
    assert_eq!(
        c,
        Some(EntranceColor {
            room: 0,
            entrance: 0,
            color: ShortCutColor::Rgb(effect),
        })
    );
    // Stepping away from the entrance blinks nothing.
    g.world.creature_mut(l).pos = WorldCoordinate::new(0, 4, 3, -1);
    let away = g
        .world
        .new_tile(l, WorldCoordinate::new(0, 3, 3, -1), g.camera_room);
    assert_eq!(away, None);
}

#[test]
fn a_lizard_leaving_the_lobby_flashes_its_entrance_in_its_effect_colour() {
    let mut g = game();
    let l = g.colour_test_creature(CreatureType::GreenLizard, (8, 3));
    let effect = g.world.creature(l).lizard().expect("realized").effect_color;
    g.take_entrance_colors();
    let v = CreatureVessel {
        creature: l,
        room: -1,
        vessel: ShortcutVessel::new((0, 0)),
        entrance_node: 0,
    };
    let lobby = LobbyVessel {
        room: 0,
        entrance_node: 0,
        updates: 0,
        owner: VesselOwner::Shortcut(v),
    };
    assert!(g.creature_lobby_step(v, lobby));
    assert_eq!(
        g.take_entrance_colors(),
        [EntranceColor {
            room: 0,
            entrance: 0,
            color: ShortCutColor::Rgb(effect),
        }]
    );
}

#[test]
fn other_creatures_take_their_templates_shortcut_colour() {
    let mut g = game();
    let cases = [
        (CreatureType::Fly, [1.0, 1.0, 1.0]),
        (CreatureType::Overseer, [1.0, 1.0, 1.0]),
        (CreatureType::DaddyLongLegs, [0.0, 0.0, 1.0]),
        (CreatureType::BrotherLongLegs, [0.7, 0.7, 0.4]),
        (CreatureType::RedCentipede, [1.0, 0.0, 0.0]),
        (CreatureType::Centipede, [1.0, 1.0, 1.0]),
        (CreatureType::BigNeedleWorm, [1.0, 0.5, 0.5]),
        (CreatureType::SmallNeedleWorm, [1.0, 0.5, 0.5]),
        // `BigEel.ShortCutColor` and `Deer.ShortCutColor` are constants.
        (CreatureType::BigEel, [1.0, 0.0, 1.0]),
        (CreatureType::Deer, [0.0, 0.0, 0.0]),
    ];
    for (kind, rgb) in cases {
        let c = g.colour_test_creature(kind, (10, 3));
        assert_eq!(
            g.world.creature(c).short_cut_color(),
            ShortCutColor::Rgb(rgb),
            "{kind:?}"
        );
    }
    let player = g.player_id;
    assert_eq!(
        g.world.creature(player).short_cut_color(),
        ShortCutColor::Slugcat
    );
}
