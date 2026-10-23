//! `PlayerGraphics` tests, one child module a subject, and the player helpers they share.

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, Room, RoomSettings, Timeline};
use rw_random::UnityRandom;
use rw_sim::world::World;
use rw_sim::{HEAD, HIPS, Input, Player, PlayerSurroundings, Sounds, Vec2};

use crate::tests::room;
use crate::{PlayerGraphics, SlugcatName};

mod corridor;
mod frames;
mod malnourished;
mod poles;
mod zero_g;

/// A player lying on the flat room's ground, head to the right of the hips, with
/// its graphics.
pub(crate) fn grounded_player(rng: &mut UnityRandom) -> (Player, PlayerGraphics) {
    let mut player = Player::new();
    player.body.chunks[HIPS].reset_at(Vec2::new(100.0, 48.0));
    player.body.chunks[HEAD].reset_at(Vec2::new(117.0, 49.0));
    let graphics = PlayerGraphics::new(&player, SlugcatName::White, rng);
    (player, graphics)
}

pub(crate) fn run(
    player: &mut Player,
    graphics: &mut PlayerGraphics,
    room: &Room,
    input: Input,
    ticks: usize,
    rng: &mut UnityRandom,
) {
    for _ in 0..ticks {
        player.update(
            room,
            None,
            input,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        update_alone(graphics, player, room, rng);
    }
}

/// `PlayerGraphics::update` with nothing else in the room and nothing held.
pub(crate) fn update_alone(g: &mut PlayerGraphics, p: &Player, r: &Room, rng: &mut UnityRandom) {
    let source = FixedRoom(LoadedRoom {
        room: r.clone(),
        settings: RoomSettings::ANCESTOR,
    });
    let world = World::new(
        WorldData::from_room(r),
        Arc::new(source),
        Timeline::White,
        ActiveMods::NONE,
        false,
    );
    let around = PlayerSurroundings {
        world: &world,
        grasps: [None; 2],
        stomach: false,
        room: 0,
        objects: Vec::new(),
        hungry: true,
        heavy_carry: None,
    };
    g.update(p, r, None, &around, rng);
}

/// Moves the chunks back to `at` without updating the player.
pub(crate) fn place(p: &mut Player, (head, hips): (Vec2, Vec2)) {
    p.body.chunks[HEAD].reset_at(head);
    p.body.chunks[HIPS].reset_at(hips);
}

/// 30 x 20 tiles, ground top at y = 40; a pole in column 10 from the ground (tile
/// row 2) up to tile row 10, and one in column 20 hanging from tile row 15 down to
/// tile row 8.
pub(crate) fn pole_room() -> Room {
    let mut rows = Vec::new();
    for y in (0..20).rev() {
        let mut cells = vec![b'.'; 30];
        if (2..=10).contains(&y) {
            cells[10] = b'|';
        }
        if (8..=15).contains(&y) {
            cells[20] = b'|';
        }
        if y < 2 {
            cells.fill(b'#');
        }
        rows.push(String::from_utf8(cells).unwrap());
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}
