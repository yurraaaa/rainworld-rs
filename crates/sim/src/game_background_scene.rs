//! The game's side of a room's `BackgroundScene`s: `Room.Loaded` making an
//! `AboveCloudsView` or `RoofTopView` for each such effect, a scene's first turn adding
//! its elements to the room's update list, and the turns of the `DistantLightning`s
//! among them (shared-stream draws, D1). Types: [`crate::BackgroundScene`].
//!
//! Mirrors: Room.cs (Loaded: AboveCloudsView, RoofTopView), BackgroundScene.cs (Update)

use super::{RainWorldGame, RoomObject};
use crate::above_clouds_view::AboveCloudsContext;
use crate::background_scene::{BackgroundScene, ElementKind};
use crate::{DustpuffSpawner, Vec2};

impl RainWorldGame {
    /// `AddObject(new AboveCloudsView(this, effect))` for room `index`'s effect of
    /// `amount`.
    pub(super) fn add_above_clouds_view(&mut self, index: i32, amount: f32) {
        let Some(data) = self.world.abstract_room(index) else {
            return;
        };
        let room_name = data.name.clone();
        let origin = self.world.room_to_world_pos(Vec2::ZERO, index);
        let params = &self.world.properties().params;
        let clouds = (params.clouds_start, params.clouds_end);
        let region = self.world.region_name().map(str::to_owned);
        let mut scene = BackgroundScene::above_clouds_view(&AboveCloudsContext {
            region: region.as_deref(),
            room_name: &room_name,
            room_origin: origin,
            arena: !self.world.story,
            effect_amount: amount,
            clouds,
        });
        scene.update_order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.background_scenes.push(scene);
        }
    }

    /// `AddObject(new RoofTopView(this, effect))` for room `index`.
    pub(super) fn add_roof_top_view(&mut self, index: i32) {
        let Some(data) = self.world.abstract_room(index) else {
            return;
        };
        let size = data.size;
        let origin = self.world.room_to_world_pos(Vec2::ZERO, index);
        // The constructor's `room.AddObject(new DustpuffSpawner())`, before the scene's own.
        let spawner = DustpuffSpawner {
            update_order: self.world.next_update_seq(),
        };
        if let Some(r) = self.world.realized_mut(index) {
            r.dust_spawners.push(spawner);
        }
        let mut scene = BackgroundScene::roof_top_view(origin, size, &mut self.rng);
        scene.update_order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.background_scenes.push(scene);
        }
    }

    /// The scenes and their added `DistantLightning`s in room `index`'s update list.
    pub(super) fn room_background_scene_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let mut objects = Vec::new();
        if let Some(r) = self.world.realized(index) {
            objects.extend(
                r.dust_spawners
                    .iter()
                    .map(|s| (s.update_order, RoomObject::DustpuffSpawner(s.update_order))),
            );
            objects.extend(
                r.dust_puffs
                    .iter()
                    .map(|p| (p.update_order, RoomObject::DustPuff(p.update_order))),
            );
        }
        for scene in self
            .world
            .realized(index)
            .map_or(&[][..], |r| r.background_scenes.as_slice())
        {
            objects.push((
                scene.update_order,
                RoomObject::BackgroundScene(scene.update_order),
            ));
            objects.extend(
                scene
                    .elements
                    .iter()
                    .filter(|e| {
                        e.update_order != 0 && matches!(e.kind, ElementKind::DistantLightning(_))
                    })
                    .map(|e| (e.update_order, RoomObject::SceneElement(e.update_order))),
            );
        }
        objects
    }

    /// `BackgroundScene.Update`: on its first turn the elements, far to near, take
    /// their places at the end of the update list.
    pub(super) fn run_background_scene(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let Some(scene) = r.background_scenes.iter().find(|s| s.update_order == order) else {
            return;
        };
        if scene.elements_added {
            return;
        }
        let add_order = scene.add_order();
        let screen = self.world.screen_size();
        let places: Vec<u64> = add_order
            .iter()
            .map(|_| self.world.next_update_seq())
            .collect();
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        if let Some(scene) = r
            .background_scenes
            .iter_mut()
            .find(|s| s.update_order == order)
        {
            for (&element, place) in add_order.iter().zip(places) {
                scene.elements[element].update_order = place;
            }
            scene.convergence_point = BackgroundScene::convergence_for(screen);
            scene.elements_added = true;
        }
    }

    /// A `DistantLightning`'s `Update` in its slot of room `index`'s update list.
    pub(super) fn run_scene_element(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        for scene in &mut r.background_scenes {
            if let Some(e) = scene.elements.iter_mut().find(|e| e.update_order == order) {
                if let ElementKind::DistantLightning(l) = &mut e.kind {
                    e.depth += l.update(&mut self.rng);
                }
                return;
            }
        }
    }

    /// `DustpuffSpawner.Update` in its slot of room `index`'s update list: the player's
    /// chunks, if it is in the room, make `DustPuff`s at the end of the list.
    pub(super) fn run_dustpuff_spawner(&mut self, index: i32, _order: u64) {
        let in_room = self.pos().room == index;
        let Some(player) = self.player.as_ref().filter(|p| in_room && !p.in_shortcut()) else {
            return;
        };
        let puffs = DustpuffSpawner::update(&player.body.chunks, &mut self.rng);
        for mut puff in puffs {
            puff.update_order = self.world.next_update_seq();
            if let Some(r) = self.world.realized_mut(index) {
                r.dust_puffs.push(puff);
            }
        }
    }

    /// A `DustPuff`'s `Update`; a destroyed one leaves the room.
    pub(super) fn run_dust_puff(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        if let Some(p) = r.dust_puffs.iter_mut().find(|p| p.update_order == order) {
            p.update();
        }
        r.dust_puffs.retain(|p| !p.slated);
    }
}
