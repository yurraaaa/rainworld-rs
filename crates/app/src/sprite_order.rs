//! Where camera 0 draws the sprites of the objects a graphics module let out of its
//! internal container: `MoveObjectToContainer(obj, null)` re-adds them at the top of
//! their default containers (`FContainer.AddChild` moves an existing child to the end),
//! and objects whose sprites join the room's containers after that go above them.
//! Everything else keeps the port's order by kind (D19).
//!
//! Mirrors: RoomCamera.cs (MoveObjectToContainer), GraphicsModule.cs (ReleaseAllInternallyContainedSprites, ReleaseSpecificInternallyContainedObjectSprites)

use rw_sim::item::Entity;

/// The objects drawn above the port's kind order in camera 0's room, bottom first.
#[derive(Debug, Default)]
pub struct SpriteOrder {
    /// The camera's room as of the last update; a new room's sprites are all added
    /// afresh in the kind order.
    room: Option<i32>,
    /// The objects drawn at the last update.
    present: Vec<Entity>,
    /// Re-added at the top, or added after such a re-add, in the order it happened.
    on_top: Vec<Entity>,
    /// Re-adds since the last update, in order (each object once, at its last re-add).
    pending: Vec<Entity>,
}

impl SpriteOrder {
    /// The sim let the sprites of `moved` out of an internal container, in order.
    pub fn moved_to_front(&mut self, moved: Vec<Entity>) {
        for e in moved {
            self.pending.retain(|&p| p != e);
            self.pending.push(e);
        }
    }

    /// The frame draws `now` in camera 0's `room`: objects new since the last update go
    /// on top while anything is there, gone ones leave it, then the pending re-adds go
    /// on top in order. Returns the objects on top, bottom first.
    pub fn update(&mut self, room: i32, now: &[Entity]) -> &[Entity] {
        if self.room != Some(room) {
            self.room = Some(room);
            self.on_top.clear();
            self.pending.clear();
            self.present = now.to_vec();
            return &self.on_top;
        }
        for &e in now {
            if !self.present.contains(&e) && !self.on_top.is_empty() {
                self.on_top.push(e);
            }
        }
        self.on_top.retain(|e| now.contains(e));
        for e in std::mem::take(&mut self.pending) {
            if now.contains(&e) {
                self.on_top.retain(|&o| o != e);
                self.on_top.push(e);
            }
        }
        self.present = now.to_vec();
        &self.on_top
    }
}

#[cfg(test)]
mod tests {
    use rw_sim::creature::CreatureId;
    use rw_sim::item::{Entity, ObjectId};

    use super::SpriteOrder;

    fn creature(i: usize) -> Entity {
        Entity::Creature(CreatureId(i))
    }

    #[test]
    fn released_objects_go_on_top_and_later_arrivals_above_them() {
        let (player, lizard, rock) = (creature(0), creature(1), Entity::Object(ObjectId(0)));
        let mut order = SpriteOrder::default();
        assert!(order.update(3, &[player, lizard]).is_empty());
        // `ReleaseAllInternallyContainedSprites` walks the list from its end.
        order.moved_to_front(vec![rock, player]);
        assert_eq!(order.update(3, &[player, lizard]), &[player]);
        let new_lizard = creature(2);
        assert_eq!(
            order.update(3, &[player, lizard, new_lizard]),
            &[player, new_lizard]
        );
        assert_eq!(order.update(3, &[lizard, new_lizard]), &[new_lizard]);
        order.moved_to_front(vec![lizard]);
        assert!(order.update(4, &[lizard]).is_empty());
    }

    #[test]
    fn without_a_release_arrivals_keep_the_kind_order() {
        let mut order = SpriteOrder::default();
        order.update(1, &[creature(0)]);
        assert!(order.update(1, &[creature(0), creature(1)]).is_empty());
    }
}
