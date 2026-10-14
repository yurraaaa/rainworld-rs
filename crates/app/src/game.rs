//! Simulation state owned by the app: the room, the player and its graphics module,
//! the shortcut divider and the RNGs.
//!
//! This is the only place the app touches the simulation.

use rand::SeedableRng;
use rand::rngs::StdRng;
use rw_assets::Room;
use rw_graphics::{Color, DrawList, PlayerGraphics, SlugcatName};
use rw_render::{BodyStatus, Follow};
use rw_sim::{Input, PhysicalObject, Player, ShortcutHandler};

/// A room with (at most) one player in it.
pub struct Game {
    room: Room,
    /// `None` once the body fell out of the room and was destroyed.
    player: Option<Player>,
    /// The player's graphics module; replaced with the player.
    graphics: Option<PlayerGraphics>,
    shortcuts: ShortcutHandler,
    /// Spawn positions.
    rng: StdRng,
    /// Cosmetic randomness, separate so graphics never shift the spawn sequence.
    graphics_rng: StdRng,
    /// `--spawn`: place the player on this tile instead of a random spawn tile.
    spawn_tile: Option<(i32, i32)>,
    tick: u64,
}

impl Game {
    /// Spawns the player into `room` at a spawn tile chosen with `seed`, or on
    /// `spawn_tile` when given.
    #[must_use]
    pub fn new(room: Room, seed: u64, spawn_tile: Option<(i32, i32)>) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut graphics_rng = StdRng::seed_from_u64(seed.wrapping_add(1));
        let player = Self::spawn_player(&room, spawn_tile, &mut rng);
        let graphics = PlayerGraphics::new(&player, SlugcatName::White, &mut graphics_rng);
        log::info!("spawned player");
        Self {
            room,
            player: Some(player),
            graphics: Some(graphics),
            shortcuts: ShortcutHandler::new(),
            rng,
            graphics_rng,
            spawn_tile,
            tick: 0,
        }
    }

    /// Replaces the player with a fresh one at a new spawn tile (continuing the seeded
    /// sequence).
    pub fn respawn(&mut self) {
        let player = Self::spawn_player(&self.room, self.spawn_tile, &mut self.rng);
        self.graphics = Some(PlayerGraphics::new(
            &player,
            SlugcatName::White,
            &mut self.graphics_rng,
        ));
        self.player = Some(player);
        log::info!("respawned player");
    }

    fn spawn_player(room: &Room, tile: Option<(i32, i32)>, rng: &mut StdRng) -> Player {
        match tile {
            Some(tile) => {
                let mut player = Player::new();
                player.place(tile, rng);
                player
            }
            None => Player::spawn(room, rng),
        }
    }

    /// One 40 Hz simulation tick: shortcut vessels, then the player and right after it
    /// its graphics module, as the game orders them. A dead player is frozen until [`Game::respawn`]; a destroyed one is gone
    /// (docs/DIVERGENCES.md D4).
    pub fn tick(&mut self, input: Input) {
        if let Some(player) = &mut self.player
            && !player.is_dead()
        {
            let was_in_shortcut = player.in_shortcut();
            self.shortcuts.update(&self.room, player);
            let was_stunned = player.is_stunned();
            player.update(&self.room, input);
            if let Some(graphics) = &mut self.graphics {
                graphics.update(player, &self.room, &mut self.graphics_rng);
            }
            if player.is_destroyed() {
                log::info!("player fell out of the room and was destroyed; R respawns");
                self.player = None;
                self.graphics = None;
            } else if player.is_dead() {
                log::info!("player died; R respawns");
            } else if player.is_stunned() && !was_stunned {
                log::info!("player stunned for {} ticks", player.stun_ticks());
            } else if player.in_shortcut() != was_in_shortcut {
                log::info!(
                    "player {} a shortcut",
                    if was_in_shortcut { "left" } else { "entered" }
                );
            }
        }
        self.tick += 1;
    }

    /// The slugcat's sprites at `alpha` between ticks, eyes in `face_color` (the room
    /// palette's `blackColor`); `None` once the player is destroyed.
    pub fn draw_list(&mut self, alpha: f32, face_color: Color) -> Option<DrawList> {
        let (player, graphics) = (self.player.as_ref()?, self.graphics.as_mut()?);
        Some(graphics.draw(player, alpha, face_color))
    }

    /// Whether the player is dead and frozen.
    #[must_use]
    pub fn player_dead(&self) -> bool {
        self.player.as_ref().is_some_and(Player::is_dead)
    }

    #[must_use]
    pub fn status(&self) -> BodyStatus {
        BodyStatus {
            stunned: self.player.as_ref().is_some_and(Player::is_stunned),
            dead: self.player_dead(),
        }
    }

    #[must_use]
    pub fn room(&self) -> &Room {
        &self.room
    }

    /// The player's body while it is in the room (not destroyed, not in a shortcut).
    #[must_use]
    pub fn body(&self) -> Option<&PhysicalObject> {
        self.player
            .as_ref()
            .filter(|p| !p.in_shortcut())
            .map(|p| &p.body)
    }

    /// The shortcut tile the player is travelling through, if any.
    #[must_use]
    pub fn vessel_tile(&self) -> Option<(i32, i32)> {
        self.player.as_ref().and_then(Player::shortcut_vessel_tile)
    }

    /// What the room camera follows: the player's first chunk and input, or the centre
    /// of its shortcut tile while it travels through one. `None` once destroyed.
    #[must_use]
    pub fn camera_follow(&self) -> Option<Follow> {
        let player = self.player.as_ref()?;
        let chunk = &player.body.chunks[0];
        let input = player.input_history()[0];
        Some(Follow {
            pos: chunk.pos.to_array(),
            vel: chunk.vel.to_array(),
            input: [input.x, input.y],
            in_shortcut_at: player
                .shortcut_vessel_tile()
                .map(|(x, y)| rw_assets::tile_centre(x, y)),
        })
    }

    /// Ticks simulated since start.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An all-Air 20 x 12 room.
    fn open_room() -> Room {
        let (w, h) = (20, 12);
        let cells = "0|".repeat(w * h);
        let text = format!(
            "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{cells}\n"
        );
        Room::parse(&text).expect("test room parses")
    }

    impl Game {
        fn player(&self) -> &Player {
            self.player.as_ref().expect("player present")
        }

        fn head_y(&self) -> f32 {
            self.body().expect("body in room").chunks[0].pos.y
        }
    }

    #[test]
    fn player_starts_on_all_fours_and_falls() {
        let mut game = Game::new(open_room(), 1, None);
        assert!(!game.player().is_standing());
        let before = game.head_y();
        for _ in 0..5 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.ticks(), 5);
        assert!(game.head_y() < before, "gravity pulls it down");
    }

    #[test]
    fn falling_out_removes_the_player_until_respawn() {
        // The all-Air room has an open bottom: the player falls out and is destroyed.
        let mut game = Game::new(open_room(), 1, None);
        for _ in 0..400 {
            if game.player.is_none() {
                break;
            }
            game.tick(Input::NONE);
        }
        assert!(game.player.is_none());
        assert!(game.body().is_none());
        assert!(!game.player_dead());
        game.tick(Input::NONE);
        game.respawn();
        assert!(game.body().is_some());
    }

    #[test]
    fn dead_player_freezes_until_respawn() {
        let mut game = Game::new(open_room(), 1, None);
        game.player.as_mut().expect("player").die();
        let frozen = game.body().expect("body").clone();
        for _ in 0..10 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.body(), Some(&frozen));
        game.respawn();
        assert!(!game.player_dead());
    }

    #[test]
    fn respawn_gives_a_fresh_player() {
        let mut game = Game::new(open_room(), 1, None);
        for _ in 0..10 {
            game.tick(Input {
                x: 1,
                y: 1,
                jump: false,
            });
        }
        game.respawn();
        assert!(!game.player().is_standing());
        assert_eq!(
            game.player().input_history(),
            &[Input::NONE; rw_sim::INPUT_HISTORY]
        );
        assert_eq!(game.body().expect("body").chunks[0].vel, rw_sim::Vec2::ZERO);
    }
}
