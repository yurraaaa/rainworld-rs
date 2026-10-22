//! What the lizards' jaws hold, drawn inside their containers. In the game each object's
//! sprites are a Futile node tree: `AddObjectToInternalContainer` moves the held
//! object's sprites, its own `containers[0]` among them, into the holder's container, so
//! what a held lizard holds stays nested inside it, inside its holder, whatever the
//! update order. What each container holds comes from the camera's record
//! ([`crate::sprite_order::SpriteOrder::held_by`]). A hold cycle (A holds B, B holds A;
//! two lizards biting each other in one collision) is cut where it is reached again, so
//! the whole cycle draws nested in the first of it in the list; in the game none of it
//! draws until one lets go (D20).
//!
//! Mirrors: GraphicsModule.cs (AddObjectToInternalContainer), LizardGraphics.cs (AddToContainer), PlayerGraphics.cs (AddToContainer), FContainer.cs (AddChild, RemoveChild)

use rw_graphics::{DrawItem, DrawList, InternalContainer, Layer};

use crate::sprite_order::Drawable;

/// A lizard's jaw container this frame and what it holds, in the order put there.
#[derive(Debug, Clone, PartialEq)]
pub struct JawHolder {
    pub container: InternalContainer,
    pub held: Vec<Drawable>,
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
    player: Drawable,
    visit: Vec<Visit>,
}

impl Jaws {
    #[must_use]
    pub fn new(holders: Vec<Option<JawHolder>>, player: Drawable) -> Self {
        Self {
            visit: vec![Visit::NotDone; holders.len()],
            holders,
            player,
        }
    }

    /// Moves every held object's sprites into its holder's list (`objects[i]` has
    /// `holders[i]`). A held lizard's own holds go into its list first. The player's
    /// `Foreground` sprites stay in its own list (`PlayerGraphics.AddToContainer` puts
    /// only its `Midground` ones in the given container).
    pub fn contain(mut self, objects: &mut [(Drawable, DrawList)]) {
        for i in 0..objects.len() {
            self.contain_one(i, objects);
        }
    }

    fn contain_one(&mut self, i: usize, objects: &mut [(Drawable, DrawList)]) {
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
            let Some(j) = objects.iter().position(|(o, _)| *o == e) else {
                continue;
            };
            match self.visit[j] {
                // A hold cycle: its sprites stay where they are (D20).
                Visit::InProgress => continue,
                Visit::NotDone => self.contain_one(j, objects),
                Visit::Done => {}
            }
            let mut other = std::mem::take(&mut objects[j].1);
            let layers: Vec<Layer> = if e == self.player {
                vec![Layer::Midground]
            } else {
                other.items.iter().map(DrawItem::layer).collect()
            };
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
