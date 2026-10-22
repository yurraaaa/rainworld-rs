//! What the lizards' jaws hold, drawn inside their containers. In the game each object's
//! sprites are a Futile node tree: `AddObjectToInternalContainer` moves the held
//! object's sprites, its own `containers[0]` among them, into the holder's container, so
//! what a held lizard holds stays nested inside it, inside its holder, whatever the
//! update order. A hold cycle (A holds B, B holds A; two lizards biting each other in
//! one collision) is cut where it is reached again, so the whole cycle draws nested in
//! the first of it in the list; in the game none of it draws until one lets go (D20).
//!
//! Mirrors: GraphicsModule.cs (AddObjectToInternalContainer), LizardGraphics.cs (AddToContainer), FContainer.cs (AddChild, RemoveChild)

use rw_graphics::{DrawItem, DrawList, InternalContainer, Layer};
use rw_sim::item::Entity;

/// A lizard's jaw container this frame and what it holds, in the order put there.
#[derive(Debug, Clone, PartialEq)]
pub struct JawHolder {
    pub container: InternalContainer,
    pub held: Vec<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visit {
    NotDone,
    InProgress,
    Done,
}

/// One frame's nesting of held sprites into their holders' draw lists.
pub struct Jaws {
    /// Per object of the frame's list, its jaws (lizards with a drawn container).
    holders: Vec<Option<JawHolder>>,
    player: Entity,
    visit: Vec<Visit>,
}

impl Jaws {
    #[must_use]
    pub fn new(holders: Vec<Option<JawHolder>>, player: Entity) -> Self {
        Self {
            visit: vec![Visit::NotDone; holders.len()],
            holders,
            player,
        }
    }

    /// Moves every held object's sprites into its holder's list (`objects[i]` has
    /// `holders[i]`). A held lizard's own holds go into its list first. The player's
    /// `Foreground` sprites stay in `player` (`PlayerGraphics.AddToContainer` puts only
    /// its `Midground` ones in the given container).
    pub fn contain(
        mut self,
        mut player: Option<&mut DrawList>,
        objects: &mut [(Entity, DrawList)],
    ) {
        for i in 0..objects.len() {
            self.contain_one(i, player.as_deref_mut(), objects);
        }
    }

    fn contain_one(
        &mut self,
        i: usize,
        mut player: Option<&mut DrawList>,
        objects: &mut [(Entity, DrawList)],
    ) {
        if self.visit[i] != Visit::NotDone {
            return;
        }
        let Some(JawHolder {
            mut container,
            held,
        }) = self.holders[i].clone()
        else {
            self.visit[i] = Visit::Done;
            return;
        };
        self.visit[i] = Visit::InProgress;
        for e in held {
            if e == self.player {
                if let Some(p) = player.as_deref_mut() {
                    container = objects[i]
                        .1
                        .take_into_container(container, p, &[Layer::Midground]);
                }
                continue;
            }
            let Some(j) = objects.iter().position(|(o, _)| *o == e) else {
                continue;
            };
            match self.visit[j] {
                // A hold cycle: its sprites stay where they are (D20).
                Visit::InProgress => continue,
                Visit::NotDone => self.contain_one(j, player.as_deref_mut(), objects),
                Visit::Done => {}
            }
            let mut other = std::mem::take(&mut objects[j].1);
            let layers: Vec<Layer> = other.items.iter().map(DrawItem::layer).collect();
            container = objects[i]
                .1
                .take_into_container(container, &mut other, &layers);
            objects[j].1 = other;
        }
        self.visit[i] = Visit::Done;
    }
}

#[cfg(test)]
#[path = "jaws_tests.rs"]
mod tests;
