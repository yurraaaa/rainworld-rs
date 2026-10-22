//! Camera 0's `ShortcutGraphics` (`rw_graphics::ShortcutGraphics`) fed from the game:
//! made for the camera's room on `NewRoom`, faded each camera tick, coloured by the
//! game's `ColorEntrance` calls and drawn with the room's blink state and the player's
//! vessel.
//!
//! Mirrors: ShortcutGraphics.cs

use rw_assets::{PlacedObjectType, RoomEffectType, SlugcatName};
use rw_graphics::{
    DrawList, ExitBehind, ShortcutFrame, ShortcutGraphics, ShortcutPalette, ShortcutRoom,
    ShortcutVessel,
};
use rw_random::UnityRandom;
use rw_sim::{RainWorldGame, ShortCutColor, Vec2};

/// The shortcut graphics of the room camera 0 shows.
#[derive(Default)]
pub struct CameraShortcuts {
    /// The room they were made for, and them.
    made: Option<(i32, ShortcutGraphics)>,
}

impl CameraShortcuts {
    /// `StaticWorld`: the slugcat's `shortcutSegments`.
    const SLUGCAT_SEGMENTS: i32 = 2;

    /// `ShortcutGraphics.Update`, from `RoomCamera.Update` before the rooms update.
    pub fn update(&mut self) {
        if let Some((_, g)) = &mut self.made {
            g.update();
        }
    }

    /// After the game's tick: new graphics when camera 0 moved to another room
    /// (`NewRoom`), then the tick's `ColorEntrance` calls on that room.
    pub fn sync(&mut self, sim: &mut RainWorldGame, slugcat: SlugcatName) {
        let room = sim.camera_room();
        let colors = sim.take_entrance_colors();
        let Some(g) = self.graphics(sim) else {
            return;
        };
        for c in colors.into_iter().filter(|c| c.room == room) {
            g.color_entrance(
                c.entrance,
                ShortcutGraphics::creature_color(c.color, slugcat),
            );
        }
    }

    /// The graphics of camera 0's room, made anew when it changed.
    fn graphics(&mut self, sim: &RainWorldGame) -> Option<&mut ShortcutGraphics> {
        let room = sim.camera_room();
        if self.made.as_ref().is_none_or(|(r, _)| *r != room) {
            self.made = Self::generate(sim).map(|g| (room, g));
        }
        self.made.as_mut().map(|(_, g)| g)
    }

    /// The graphics of camera 0's room.
    #[cfg(test)]
    pub fn current(&mut self, sim: &RainWorldGame) -> Option<&ShortcutGraphics> {
        self.graphics(sim).map(|g| &*g)
    }

    /// `GenerateSprites` for camera 0's room.
    fn generate(sim: &RainWorldGame) -> Option<ShortcutGraphics> {
        let world = sim.world();
        let index = sim.camera_room();
        let realized = world.realized(index)?;
        let data = world.abstract_room(index)?;
        let mods = world.mods();
        let behind: Vec<Option<ExitBehind>> = data
            .connections
            .iter()
            .map(|&c| {
                let r = world.abstract_room(c)?;
                let shelter = r.shelter_index > -1;
                Some(ExitBehind {
                    shelter,
                    broken: shelter
                        && usize::try_from(r.shelter_index)
                            .ok()
                            .and_then(|i| world.data().broken_shelters.get(i))
                            .copied()
                            .unwrap_or(false),
                    ancient: r.ancient_shelter,
                })
            })
            .collect();
        let placed: Vec<(PlacedObjectType, (i32, i32))> = realized
            .settings
            .placed_objects_for(sim.campaign().timeline, mods)
            .iter()
            .filter(|o| o.active)
            .filter_map(|o| {
                let tile = (
                    (o.pos[0] / rw_assets::TILE_SIZE).floor() as i32,
                    (o.pos[1] / rw_assets::TILE_SIZE).floor() as i32,
                );
                Some((o.object_type(mods)?, tile))
            })
            .collect();
        let gate = data.gate_index > -1;
        Some(ShortcutGraphics::new(&ShortcutRoom {
            room: &realized.room,
            water: realized.water.as_ref(),
            single_room_world: world.single_room(),
            gate,
            gate_room: gate || realized.settings.effect_amount(RoomEffectType::FakeGate) > 0.0,
            connections: &data.connections,
            behind: &behind,
            placed: &placed,
        }))
    }

    /// The player's vessel when it is in the camera's room.
    fn player_vessel(sim: &RainWorldGame, slugcat: SlugcatName) -> Option<ShortcutVessel> {
        let p = sim
            .player()
            .filter(|_| sim.player_in_camera_room() && sim.lobby().is_none())?;
        Some(ShortcutVessel {
            pos: p.shortcut_vessel_tile()?,
            last_positions: p.shortcut_vessel_last_tile().into_iter().collect(),
            color: ShortcutGraphics::creature_color(ShortCutColor::Slugcat, slugcat),
            segments: Self::SLUGCAT_SEGMENTS,
        })
    }

    /// `ShortcutGraphics.Draw` for camera 0, whose image sits at `camera` in the room.
    pub fn draw(
        &mut self,
        sim: &RainWorldGame,
        slugcat: SlugcatName,
        palette: ShortcutPalette,
        camera: Vec2,
        rng: &mut UnityRandom,
    ) -> Option<DrawList> {
        let world = sim.world();
        let realized = world.realized(sim.camera_room())?;
        let blinking = realized.shortcuts_blinking.as_ref()?.rows();
        let room = sim.camera_room();
        let vessels: Vec<ShortcutVessel> = sim
            .transport_vessels()
            .filter_map(|v| match v {
                None => Self::player_vessel(sim, slugcat),
                Some(v) if v.room == room => {
                    let c = world.creature(v.creature);
                    let (pos, last_positions) = v.positions();
                    let color = ShortcutGraphics::creature_color(c.short_cut_color(), slugcat);
                    Some(ShortcutVessel {
                        pos,
                        last_positions,
                        color,
                        segments: c.template.shortcut_segments(),
                    })
                }
                Some(_) => None,
            })
            .collect();
        let g = self.graphics(sim)?;
        Some(g.draw(
            &ShortcutFrame {
                room: &realized.room,
                water: realized.water.as_ref(),
                blinking,
                palette,
                camera,
                vessels: &vessels,
            },
            rng,
        ))
    }
}
