//! Simulation state owned by the app: the room, the player and the spawn RNG.
//!
//! This is the only place the app touches the simulation.

use rand::SeedableRng;
use rand::rngs::StdRng;
use rw_assets::Room;
use rw_sim::{Input, PhysicalObject, Player};

/// A room with one player in it.
pub struct Game {
    room: Room,
    player: Player,
    rng: StdRng,
    tick: u64,
}

impl Game {
    /// Spawns the player into `room` at a spawn tile chosen with `seed`.
    #[must_use]
    pub fn new(room: Room, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let player = Player::spawn(&room, &mut rng);
        log::info!("spawned player");
        Self {
            room,
            player,
            rng,
            tick: 0,
        }
    }

    /// Replaces the player with a fresh one at a new spawn tile (continuing the seeded
    /// sequence).
    pub fn respawn(&mut self) {
        self.player = Player::spawn(&self.room, &mut self.rng);
        log::info!("respawned player");
    }

    /// One 40 Hz simulation tick. A dead player is frozen until [`Game::respawn`]
    /// (docs/DIVERGENCES.md D4).
    pub fn tick(&mut self, input: Input) {
        if !self.player.is_dead() {
            let was_stunned = self.player.is_stunned();
            self.player.update(&self.room, input);
            if self.player.is_dead() {
                log::info!("player died; R respawns");
            } else if self.player.is_stunned() && !was_stunned {
                log::info!("player stunned for {} ticks", self.player.stun_ticks());
            }
        }
        self.tick += 1;
    }

    /// Whether the player is dead and frozen.
    #[must_use]
    pub fn player_dead(&self) -> bool {
        self.player.is_dead()
    }

    #[must_use]
    pub fn room(&self) -> &Room {
        &self.room
    }

    #[must_use]
    pub fn body(&self) -> &PhysicalObject {
        &self.player.body
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

    #[test]
    fn player_starts_on_all_fours_and_falls() {
        let mut game = Game::new(open_room(), 1);
        assert!(!game.player.is_standing());
        let before = game.body().chunks[0].pos.y;
        for _ in 0..5 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.ticks(), 5);
        assert!(
            game.body().chunks[0].pos.y < before,
            "gravity pulls it down"
        );
    }

    #[test]
    fn dead_player_freezes_until_respawn() {
        // The all-Air room has an open bottom: the player falls out and dies.
        let mut game = Game::new(open_room(), 1);
        for _ in 0..400 {
            if game.player_dead() {
                break;
            }
            game.tick(Input::NONE);
        }
        assert!(game.player_dead());
        let frozen = game.body().clone();
        for _ in 0..10 {
            game.tick(Input::NONE);
        }
        assert_eq!(game.body(), &frozen);
        game.respawn();
        assert!(!game.player_dead());
    }

    #[test]
    fn respawn_gives_a_fresh_player() {
        let mut game = Game::new(open_room(), 1);
        for _ in 0..10 {
            game.tick(Input {
                x: 1,
                y: 1,
                jump: false,
            });
        }
        game.respawn();
        assert!(!game.player.is_standing());
        assert_eq!(
            game.player.input_history(),
            &[Input::NONE; rw_sim::INPUT_HISTORY]
        );
        assert_eq!(game.body().chunks[0].vel, rw_sim::Vec2::ZERO);
    }
}
