//! Simulation state owned by the app: the room, its water, the player and its
//! graphics module, the shortcut divider and the game's random generator.
//!
//! This is the only place the app touches the simulation.

use rw_assets::{Room, RoomSettings, StreamingAssets};
use rw_graphics::{Color, DrawList, PlayerGraphics, SlugcatName};
use rw_random::UnityRandom;
use rw_render::{BodyStatus, ChunkInterpolation, Follow, PlayerLight, WaterSurface};
use rw_sim::{
    Input, PhysicalObject, Player, ShortcutHandler, Vec2, Water, determine_safe_spawn_tile,
};

/// A room with (at most) one player in it.
pub struct Game {
    room: Room,
    /// The room's settings (the water's wave values).
    settings: RoomSettings,
    /// The room's water surface, `None` without water.
    water: Option<Water>,
    /// `None` once the body fell out of the room and was destroyed.
    player: Option<Player>,
    /// The player's graphics module; replaced with the player.
    graphics: Option<PlayerGraphics>,
    shortcuts: ShortcutHandler,
    /// The game's single `UnityEngine.Random` stream: spawning, the player, the water
    /// and the graphics draw from it in the game's order.
    rng: UnityRandom,
    /// `--spawn`: place the player on this tile instead of a random spawn tile.
    spawn_tile: Option<(i32, i32)>,
    tick: u64,
}

impl Game {
    /// Spawns the player into `room` (water waves from `settings`) at a spawn tile
    /// drawn from `UnityRandom::new(seed)` (`Random.InitState`, low 32 bits), or on
    /// `spawn_tile` when given.
    #[must_use]
    pub fn new(
        room: Room,
        settings: &RoomSettings,
        seed: u64,
        spawn_tile: Option<(i32, i32)>,
    ) -> Self {
        let mut rng = UnityRandom::new(seed as i32);
        let (player, graphics) = Self::spawn_player(&room, spawn_tile, &mut rng);
        log::info!("spawned player");
        Self {
            water: Water::new(&room, settings),
            settings: settings.clone(),
            room,
            player: Some(player),
            graphics: Some(graphics),
            shortcuts: ShortcutHandler::new(),
            rng,
            spawn_tile,
            tick: 0,
        }
    }

    /// The settings of `room` (Survivor timeline), or the defaults when they cannot be
    /// read.
    #[must_use]
    pub fn room_settings(assets: &StreamingAssets, room: &Room) -> RoomSettings {
        let region = assets.region_of_room(&room.name().to_lowercase());
        RoomSettings::load(assets, room.name(), region.as_deref(), Some("white"))
            .inspect_err(|e| log::warn!("room settings ({e}); using the defaults"))
            .unwrap_or(RoomSettings::ANCESTOR)
    }

    #[must_use]
    pub fn settings(&self) -> &RoomSettings {
        &self.settings
    }

    /// Replaces the player with a fresh one at a new spawn tile (continuing the random
    /// sequence).
    pub fn respawn(&mut self) {
        let (player, graphics) = Self::spawn_player(&self.room, self.spawn_tile, &mut self.rng);
        self.player = Some(player);
        self.graphics = Some(graphics);
        log::info!("respawned player");
    }

    /// The game's order of draws: `DetermineSafeSpawnTile`, the `Player` constructor,
    /// then `Creature.PlaceInRoom`, whose `AddObject` builds the graphics module (in a
    /// viewed room) before the chunk offsets.
    fn spawn_player(
        room: &Room,
        tile: Option<(i32, i32)>,
        rng: &mut UnityRandom,
    ) -> (Player, PlayerGraphics) {
        let tile = tile.unwrap_or_else(|| determine_safe_spawn_tile(room, rng));
        let mut player = Player::create(rng);
        let graphics = PlayerGraphics::new(&player, SlugcatName::White, rng);
        player.place(tile, rng);
        (player, graphics)
    }

    /// One 40 Hz simulation tick: shortcut vessels, the water surface (pushed by the
    /// body), then the player and right after it its graphics module, as the game
    /// orders them. A dead player is frozen until [`Game::respawn`]; a destroyed one is gone
    /// (docs/DIVERGENCES.md D4).
    pub fn tick(&mut self, input: Input) {
        if let Some(player) = &mut self.player
            && !player.is_dead()
        {
            let was_in_shortcut = player.in_shortcut();
            self.shortcuts.update(&self.room, player);
            let was_stunned = player.is_stunned();
            if let Some(water) = &mut self.water {
                let chunks = if player.in_shortcut() {
                    &mut [][..]
                } else {
                    &mut player.body.chunks[..]
                };
                water.update(&self.room, chunks.iter_mut(), &mut self.rng);
            }
            player.update(&self.room, self.water.as_ref(), input, &mut self.rng);
            if let Some(graphics) = &mut self.graphics {
                graphics.update(player, &self.room, self.water.as_ref(), &mut self.rng);
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

    /// The water surface as drawn at `alpha` between ticks; `None` without water.
    #[must_use]
    pub fn water_surface(&self, alpha: f32) -> Option<WaterSurface> {
        let water = self.water.as_ref()?;
        let front = water.draw_points(alpha).map(Vec2::to_array).collect();
        let back = water.back_draw_points(alpha).map(Vec2::to_array).collect();
        Some(WaterSurface::from_rows(front, back, water.still_level()))
    }

    /// `PreviousPoint(x)` on the water's front points this tick.
    #[must_use]
    pub fn water_previous_point(&self, x: f32) -> Option<usize> {
        self.water.as_ref().map(|w| w.previous_point(x))
    }

    /// `_waterDepth` (`RoomCamera.MoveCamera`): 0 with the water in front of the
    /// terrain, 1/31 behind it, 2/31 in gates.
    #[must_use]
    pub fn water_depth(&self) -> f32 {
        let gate = self.room.name().to_ascii_uppercase().starts_with("GATE_");
        let depth = if gate {
            2.0
        } else if self.room.water_in_front() {
            0.0
        } else {
            1.0
        };
        depth / 31.0
    }

    /// `Room.waterGlitterCycle` between ticks: it drops by 1/60 every room tick.
    #[must_use]
    pub fn water_glitter(&self, alpha: f32) -> f32 {
        let now = -(self.tick as f32) / 60.0;
        let last = -(self.tick.saturating_sub(1) as f32) / 60.0;
        last + (now - last) * alpha
    }

    /// The followed player's light in the water: its main chunk at `alpha` and
    /// `outOfBreathFac`; `None` when dead, destroyed or in a shortcut (out of the room).
    #[must_use]
    pub fn player_light(&self, alpha: f32) -> Option<PlayerLight> {
        let player = self.player.as_ref()?;
        if player.is_dead() || player.in_shortcut() {
            return None;
        }
        Some(PlayerLight {
            pos: player.body.chunks[0].interpolated_pos(alpha),
            out_of_breath: player.out_of_breath(),
        })
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

    /// The player, unless destroyed.
    #[must_use]
    pub fn live_player(&self) -> Option<&Player> {
        self.player.as_ref()
    }

    /// Moves the player (a new one if it was destroyed) so its chunks sit 1 px above
    /// and below `pos`, moving at `vel`, as `PlaceInRoom` puts them around a tile
    /// centre. Movement state is kept; the graphics module starts afresh.
    pub fn set_player(&mut self, pos: Vec2, vel: Vec2) {
        let player = self.player.get_or_insert_with(Player::new);
        // `place` resets the stuck-feet state; its random offsets are overwritten below.
        player.place(pos.tile(), &mut self.rng);
        for (chunk, dy) in player.body.chunks.iter_mut().zip([1.0, -1.0]) {
            chunk.reset_at(pos + Vec2::new(0.0, dy));
            chunk.vel = vel;
        }
        self.graphics = Some(PlayerGraphics::new(
            player,
            SlugcatName::White,
            &mut self.rng,
        ));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An all-Air 20 x 12 room.
    pub(crate) fn open_room() -> Room {
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
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
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
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
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
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
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
        let mut game = Game::new(open_room(), &RoomSettings::ANCESTOR, 1, None);
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
