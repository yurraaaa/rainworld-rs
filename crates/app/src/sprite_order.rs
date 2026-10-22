//! Camera 0's sprite containers as the game fills them. Each drawable's sprites join
//! the room camera's containers when its sprite leaser is made (`NewObjectInRoom`:
//! `InitiateSprites` ends with `AddToContainer(null)`), and `FContainer.AddChild` puts
//! them on top, also when a node already in the container is added again. So within a
//! container the sprites draw in the order of each drawable's last (re)add:
//!
//! - a camera moving to a room makes leasers for `room.drawableObjects` in order
//!   (`ChangeRoom`): the drawables added with `AddObject` (items, cosmetics, lights, the
//!   shelter door, a player whose `PlayerGraphics` outlived its last room), then the
//!   creatures' graphics modules `NowViewed` builds, collision layer by layer, then the
//!   shortcut graphics (`ShortcutGraphics.NewRoom`);
//! - `AddObject` in the viewed room makes a leaser on top;
//! - `MoveObjectToContainer` re-adds on top: back to the defaults (a jaws release,
//!   `BringSpritesToFront`, `Fly.Grabbed`), or `Weapon.ChangeOverlap` into `Items` or
//!   `Background`;
//! - `MoveObjectToInternalContainer` moves them into a lizard's jaw container (the
//!   player's `Foreground` sprites stay out, re-added on top); a holder whose leaser goes
//!   takes them off the stage with its container.
//!
//! - `ApplyPalette` re-adds the camera's full-screen effect sprite on top: at the end of
//!   a move to a room or camera (`ApplyPositionChange`, after the leasers and shortcut
//!   graphics), and whenever `ApplyFade` rebuilds the palette.
//!
//! Every re-add here moves all of a drawable's sprites, so one order over the drawables
//! stands for every container's.
//!
//! Mirrors: RoomCamera.cs (NewObjectInRoom, ChangeRoom, ApplyPositionChange, SetUpFullScreenEffect, MoveObjectToContainer, MoveObjectToInternalContainer), Room.cs (AddObject, NowViewed), GraphicsModule.cs (AddObjectToInternalContainer, ReleaseAllInternallyContainedSprites, ReleaseSpecificInternallyContainedObjectSprites, BringSpritesToFront), Weapon.cs (ChangeOverlap, AddToContainer), FContainer.cs (AddChild)

use rw_sim::item::Entity;

/// A sprite leaser's `drawableObject` in camera 0's room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawable {
    /// A creature's graphics module, the player's included, or a rock or spear.
    Entity(Entity),
    ShelterDoor,
    Drip(u64),
    /// A `Spark`, `ExplosionSpikes`, `LizardBubble` or other cosmetic, by id.
    Cosmetic(u64),
    HoloLight(u64),
    LightSource(u64),
    /// `ShortcutGraphics` (not a leaser: its sprites join the containers after the
    /// room's leasers).
    Shortcuts,
    /// The room camera's `fullScreenEffect` sprite (not a leaser: re-added on top of
    /// its container by every `ApplyPalette`).
    FullScreenEffect,
}

/// Where a drawable's leaser goes when the camera moves to its room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joined {
    /// In `drawableObjects` since its `AddObject`.
    AddObject,
    /// A graphics module `NowViewed` builds, from `physicalObjects[layer]`.
    NowViewed { layer: u8 },
}

/// A drawable in camera 0's room this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Present {
    pub drawable: Drawable,
    /// Its place in the room's update list (`AddObject`); a new place is a new leaser.
    pub seq: u64,
    pub joined: Joined,
    pub overlap: Option<Overlap>,
}

/// A weapon's `ChangeOverlap` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlap {
    /// Moves so far (each changed `inFrontOfObjects`).
    pub moves: u32,
    pub in_front: bool,
    /// The update-list counter at the last move.
    pub seq: u64,
}

impl Present {
    /// A drawable `AddObject` put in `drawableObjects`.
    #[must_use]
    pub fn added(drawable: Drawable, seq: u64) -> Self {
        Self {
            drawable,
            seq,
            joined: Joined::AddObject,
            overlap: None,
        }
    }

    /// A creature's graphics module in `physicalObjects[layer]`.
    #[must_use]
    pub fn module(drawable: Drawable, seq: u64, layer: u8) -> Self {
        Self {
            joined: Joined::NowViewed { layer },
            ..Self::added(drawable, seq)
        }
    }
}

/// A re-add of a drawable's sprites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// `MoveObjectToContainer(obj, null)`.
    ToDefault(Drawable),
    /// `MoveObjectToInternalContainer(obj, holder, 0)`.
    IntoJaws { obj: Drawable, holder: Drawable },
    /// `Weapon.ChangeOverlap`: `MoveObjectToContainer(obj, Items or Background)`.
    Overlap { obj: Drawable, in_front: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parent {
    Room,
    /// In this holder's jaw container, the `n`th put into a container.
    Holder(Drawable, u64),
    /// In a container that left the stage with its holder's leaser.
    Detached,
}

#[derive(Debug, Clone, Copy)]
struct Leaser {
    drawable: Drawable,
    seq: u64,
    parent: Parent,
    /// A weapon `ChangeOverlap` put in `Background`.
    behind: bool,
    overlap_moves: u32,
}

/// Camera 0's leasers in container order, bottom first.
#[derive(Debug, Default)]
pub struct SpriteOrder {
    room: Option<i32>,
    leasers: Vec<Leaser>,
    /// Internal-container adds so far (orders a holder's contents).
    contained: u64,
}

impl SpriteOrder {
    /// Whether the camera has shown a room before (the player's graphics module, made
    /// in its first viewed room, lives on: `Player.RemoveGraphicsModule` is empty).
    #[must_use]
    pub fn had_room(&self) -> bool {
        self.room.is_some()
    }

    /// One tick's changes in camera 0's `room`: `present` is every drawable there now,
    /// `moves` the internal-container moves with the update-list counter at each, and
    /// `late` re-adds placed after the tick's other changes; the weapons'
    /// `ChangeOverlap`s come from `present`, placed by their counter too. A new room
    /// makes its leasers afresh (`ChangeRoom`, all weapons in `Items`) and drops the
    /// moves.
    pub fn update(&mut self, room: i32, present: &[Present], moves: &[(u64, Move)], late: &[Move]) {
        if self.room != Some(room) {
            self.room = Some(room);
            self.change_room(present);
            return;
        }
        self.drop_gone(present);
        let mut arrivals: Vec<&Present> = present
            .iter()
            .filter(|p| !self.leasers.iter().any(|l| l.drawable == p.drawable))
            .collect();
        arrivals.sort_by_key(|p| p.seq);
        let mut moves = moves.to_vec();
        moves.extend(self.overlap_moves(present));
        moves.sort_by_key(|&(at, _)| at);
        let mut arrivals = arrivals.into_iter().peekable();
        for (at, m) in moves {
            while let Some(p) = arrivals.next_if(|p| p.seq <= at) {
                self.arrive(p);
            }
            self.apply(m);
        }
        for p in arrivals {
            self.arrive(p);
        }
        for &m in late {
            self.apply(m);
        }
        for p in present {
            if let (Some(o), Some(i)) = (p.overlap, self.position(p.drawable)) {
                self.leasers[i].overlap_moves = o.moves;
            }
        }
    }

    fn change_room(&mut self, present: &[Present]) {
        let mut sorted: Vec<&Present> = present.iter().collect();
        sorted.sort_by_key(|p| {
            let group = match p.joined {
                Joined::AddObject => 0,
                Joined::NowViewed { layer } => 1 + layer,
            };
            (group, p.seq)
        });
        self.leasers.clear();
        for p in sorted {
            self.arrive(p);
        }
        for drawable in [Drawable::Shortcuts, Drawable::FullScreenEffect] {
            self.leasers.push(Leaser {
                drawable,
                seq: 0,
                parent: Parent::Room,
                behind: false,
                overlap_moves: 0,
            });
        }
    }

    /// `ApplyPalette`'s `SetUpFullScreenEffect`: the effect sprite on top.
    pub fn raise_full_screen_effect(&mut self) {
        if self.position(Drawable::FullScreenEffect).is_some() {
            self.re_add(Drawable::FullScreenEffect, Parent::Room, false);
        }
    }

    /// Leasers whose drawable left the room, or came back to it (a new `AddObject`),
    /// are dropped; what their jaw containers held leaves the stage with them.
    fn drop_gone(&mut self, present: &[Present]) {
        let gone: Vec<Drawable> = self
            .leasers
            .iter()
            .filter(|l| {
                !matches!(l.drawable, Drawable::Shortcuts | Drawable::FullScreenEffect)
                    && !present
                        .iter()
                        .any(|p| p.drawable == l.drawable && p.seq == l.seq)
            })
            .map(|l| l.drawable)
            .collect();
        self.leasers.retain(|l| !gone.contains(&l.drawable));
        for l in &mut self.leasers {
            if let Parent::Holder(h, _) = l.parent
                && gone.contains(&h)
            {
                l.parent = Parent::Detached;
            }
        }
    }

    fn arrive(&mut self, p: &Present) {
        self.leasers.push(Leaser {
            drawable: p.drawable,
            seq: p.seq,
            parent: Parent::Room,
            behind: false,
            overlap_moves: p.overlap.map_or(0, |o| o.moves),
        });
    }

    fn position(&self, d: Drawable) -> Option<usize> {
        self.leasers.iter().position(|l| l.drawable == d)
    }

    /// Re-adds `d`'s leaser on top, with `parent` and `behind`.
    fn re_add(&mut self, d: Drawable, parent: Parent, behind: bool) {
        let Some(i) = self.position(d) else {
            return;
        };
        let mut l = self.leasers.remove(i);
        l.parent = parent;
        l.behind = behind;
        self.leasers.push(l);
    }

    /// `MoveObjectToContainer` finds no leaser for a drawable the camera does not hold;
    /// `MoveObjectToInternalContainer` with a holder it does not hold passes `null`.
    fn apply(&mut self, m: Move) {
        match m {
            Move::ToDefault(d) => self.re_add(d, Parent::Room, false),
            Move::IntoJaws { obj, holder } => {
                if self.position(holder).is_none() {
                    self.re_add(obj, Parent::Room, false);
                } else {
                    self.contained += 1;
                    self.re_add(obj, Parent::Holder(holder, self.contained), false);
                }
            }
            Move::Overlap { obj, in_front } => self.re_add(obj, Parent::Room, !in_front),
        }
    }

    /// The weapons' `ChangeOverlap`s since the last update: one whose count of moves
    /// changed, or one that arrived (a new leaser in `Items`) and moved after its
    /// `AddObject`. Several moves in a tick end as the last one does.
    fn overlap_moves(&self, present: &[Present]) -> Vec<(u64, Move)> {
        present
            .iter()
            .filter_map(|p| {
                let o = p.overlap?;
                let moved = match self.leasers.iter().find(|l| l.drawable == p.drawable) {
                    Some(l) => l.overlap_moves != o.moves,
                    None => o.moves > 0 && o.seq >= p.seq,
                };
                moved.then_some((
                    o.seq,
                    Move::Overlap {
                        obj: p.drawable,
                        in_front: o.in_front,
                    },
                ))
            })
            .collect()
    }

    /// The drawables in container order, bottom first.
    pub fn order(&self) -> impl Iterator<Item = Drawable> + '_ {
        self.leasers.iter().map(|l| l.drawable)
    }

    /// What `holder`'s jaw container holds, in the order put there.
    #[must_use]
    pub fn held_by(&self, holder: Drawable) -> Vec<Drawable> {
        let mut held: Vec<(u64, Drawable)> = self
            .leasers
            .iter()
            .filter_map(|l| match l.parent {
                Parent::Holder(h, n) if h == holder => Some((n, l.drawable)),
                _ => None,
            })
            .collect();
        held.sort_by_key(|&(n, _)| n);
        held.into_iter().map(|(_, d)| d).collect()
    }

    /// The jaw container `d`'s sprites are in, if any (`Detached` ones included).
    #[must_use]
    pub fn holder(&self, d: Drawable) -> Option<Drawable> {
        self.leasers
            .iter()
            .find(|l| l.drawable == d)
            .and_then(|l| match l.parent {
                Parent::Holder(h, _) => Some(h),
                Parent::Room | Parent::Detached => None,
            })
    }

    /// Whether a weapon's sprites are in `Background` rather than `Items`.
    #[must_use]
    pub fn behind(&self, d: Drawable) -> bool {
        self.leasers.iter().any(|l| l.drawable == d && l.behind)
    }

    /// Whether `d`'s sprites are on the stage: in the room's containers, or in a jaw
    /// container that is. A hold cycle counts as shown (D20).
    #[must_use]
    pub fn shown(&self, d: Drawable) -> bool {
        let mut at = d;
        for _ in 0..=self.leasers.len() {
            let Some(l) = self.leasers.iter().find(|l| l.drawable == at) else {
                return false;
            };
            match l.parent {
                Parent::Room => return true,
                Parent::Detached => return false,
                Parent::Holder(h, _) => at = h,
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "sprite_order_tests.rs"]
mod tests;
